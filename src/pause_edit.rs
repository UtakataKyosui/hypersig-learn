//! Pause periods on the medication edit page: the list, the add/edit form, the
//! confirmation before a delete, and the actions that save them.
//!
//! A pause period is whole days, both ends included. Each action loads the
//! stored medication, changes only its pause periods and writes it back
//! under the revision the page last saw, as the dose slot actions do.
use crate::{
    AppState,
    auth::Member,
    domain::{LocalDate, PausePeriod, UtcInstant},
    medication_edit::{EditPage, Target, target_patch},
    medication_input::PauseInput,
    medications::{self, Editor, Error, Pause},
    schedule_view::{self, NextPlannedDose, TIMEZONE_LABEL},
};
use hypersig::html::{Class, El, div, li, p, span, ul};
use hypersig::{Action, FieldErrors, Form, Patches, SignalPatch, Signals, Timestamp, if_else};
use hypersig_ui::{
    AlertVariant, ButtonSize, ButtonVariant, Dialog, Level, alert, alert_description, alert_title,
    button, card, card_content, card_description, card_header, card_title, dialog_footer,
    submit_button,
};
use serde::{Deserialize, Serialize};
use std::ops::Not as _;

/// A pause period of the list: the stored values and the line the list shows.
#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct PauseView {
    pub id: String,
    pub input: PauseInput,
    pub label: String,
}

impl PauseView {
    #[must_use]
    pub fn new(id: String, input: PauseInput) -> Self {
        let label = format!("{} 〜 {}", input.start, input.end);
        Self { id, input, label }
    }
}

impl From<Pause> for PauseView {
    fn from(pause: Pause) -> Self {
        Self::new(pause.id, pause.input)
    }
}

pub const SAVE_PAUSE: Action<EditPage, (Target, String, PauseInput)> =
    Action::post("/medications/edit/pause/save");
pub const REMOVE_PAUSE: Action<EditPage, (Target, String)> =
    Action::post("/medications/edit/pause/remove");

/// Asks for confirmation before a pause period is deleted.
const REMOVE_DIALOG: Dialog<EditPage> = Dialog::new("remove-pause");

/// The most pause periods one medication holds.
const MAX_PAUSES: usize = 128;

/// The values a new pause period starts from.
#[must_use]
pub fn blank_pause() -> PauseInput {
    PauseInput {
        start: String::new(),
        end: String::new(),
    }
}

/// The pause periods of `editor`, earliest first.
#[must_use]
pub fn views(editor: &Editor) -> Vec<PauseView> {
    let mut views: Vec<_> = editor.pauses.iter().cloned().map(PauseView::from).collect();
    sort(&mut views);
    views
}

/// Earliest start first, then earliest end. The dates are `YYYY-MM-DD`, so
/// the text order is the calendar order.
fn sort(views: &mut [PauseView]) {
    views.sort_by(|a, b| (&a.input.start, &a.input.end).cmp(&(&b.input.start, &b.input.end)));
}

/// What the page says of the saved settings: whether today is paused, and
/// when the next dose is planned. Both are empty for a medication that is
/// not saved yet.
#[derive(Debug, PartialEq, Eq)]
pub struct Standing {
    pub paused: String,
    pub next: String,
}

/// The standing of `editor` at `now` (Unix milliseconds), from the same
/// schedule computation the notifications use.
#[must_use]
pub fn standing(editor: &Editor, now: i64) -> Standing {
    let Ok(medication) = editor.domain(now) else {
        return Standing {
            paused: String::new(),
            next: String::new(),
        };
    };
    let Ok(view) = schedule_view::query(&medication, Timestamp::from_millis(now)) else {
        return Standing {
            paused: String::new(),
            next: String::new(),
        };
    };
    let paused = UtcInstant(now)
        .tokyo_date()
        .ok()
        .and_then(|today| {
            medication
                .pauses
                .iter()
                .zip(&editor.pauses)
                .find(|(period, _)| period.contains(today))
        })
        .map_or_else(String::new, |(_, pause)| {
            format!(
                "今日は休薬中です（{} 〜 {}）。",
                pause.input.start, pause.input.end
            )
        });
    let next = match view.next {
        NextPlannedDose::Disabled => "通知が無効のため、次回の予定はありません。".to_owned(),
        NextPlannedDose::Ended => "服薬期間が終了しています。".to_owned(),
        NextPlannedDose::NoUpcoming => "次回の予定はありません。".to_owned(),
        NextPlannedDose::At(dose) => {
            let time = editor
                .slots
                .iter()
                .find(|slot| slot.id == dose.slot_id)
                .map_or("", |slot| slot.input.time.as_str());
            format!("次回の通知: {} {time}（{TIMEZONE_LABEL}）", dose.local_date)
        }
    };
    Standing { paused, next }
}

/// The signals that show the standing of `editor` as of now.
pub fn standing_patch(editor: &Editor) -> SignalPatch<EditPage> {
    let standing = standing(editor, Timestamp::now().as_millis());
    EditPage::pause_now()
        .patch(standing.paused)
        .and(EditPage::next_dose().patch(standing.next))
}

/// The pause form starts over, ready to add the next period.
fn form_reset() -> SignalPatch<EditPage> {
    EditPage::pause()
        .patch(Form::new(blank_pause()))
        .and(EditPage::pause_editing().patch(String::new()))
}

fn pauses_saved(editor: &Editor) -> Patches<EditPage> {
    Patches::new().signals(
        target_patch(editor)
            .and(form_reset())
            .and(EditPage::pauses().patch(views(editor)))
            .and(EditPage::pause_notice().patch("休薬期間を保存しました。".to_owned()))
            .and(standing_patch(editor)),
    )
}

fn pauses_removed(editor: &Editor) -> Patches<EditPage> {
    Patches::new().signals(
        target_patch(editor)
            .and(form_reset())
            .and(EditPage::pauses().patch(views(editor)))
            .and(EditPage::removing().patch(String::new()))
            .and(EditPage::pause_notice().patch("休薬期間を削除しました。".to_owned()))
            .and(standing_patch(editor)),
    )
}

/// Adds the pause period `input` to `editor`, or replaces the one `pause_id`
/// names. A period that is reversed, overlaps another or fails the form's
/// rules comes back as messages for the form's fields.
fn upsert_pause(
    editor: &mut Editor,
    pause_id: &str,
    fresh_id: &str,
    input: &PauseInput,
) -> Result<Result<(), FieldErrors<PauseInput>>, Error> {
    let adding = pause_id.is_empty();
    if !adding && editor.pauses.iter().all(|pause| pause.id != pause_id) {
        return Err(Error::Missing);
    }
    let others = editor
        .pauses
        .iter()
        .filter(|pause| pause.id != pause_id)
        .map(|pause| pause.input.parse(&[]))
        .collect::<Result<Vec<PausePeriod>, _>>()
        .map_err(|_| Error::Invalid)?;
    if let Err(errors) = input.parse(&others) {
        return Ok(Err(errors));
    }
    if adding && editor.pauses.len() >= MAX_PAUSES {
        return Ok(Err(FieldErrors::new().add(
            PauseInput::start(),
            format!("休薬期間は{MAX_PAUSES}件までです。"),
        )));
    }
    let date = |text: &str| text.parse::<LocalDate>().map_err(|_| Error::Invalid);
    let input = PauseInput {
        start: date(&input.start)?.to_string(),
        end: date(&input.end)?.to_string(),
    };
    if adding {
        editor.pauses.push(Pause {
            id: fresh_id.to_owned(),
            input,
        });
    } else if let Some(pause) = editor.pauses.iter_mut().find(|pause| pause.id == pause_id) {
        pause.input = input;
    }
    editor
        .pauses
        .sort_by(|a, b| (&a.input.start, &a.input.end).cmp(&(&b.input.start, &b.input.end)));
    Ok(Ok(()))
}

/// Removes the pause period `pause_id` from `editor`.
fn remove_pause_from(editor: &mut Editor, pause_id: &str) -> Result<(), Error> {
    let before = editor.pauses.len();
    editor.pauses.retain(|pause| pause.id != pause_id);
    if editor.pauses.len() == before {
        return Err(Error::Missing);
    }
    Ok(())
}

pub async fn save_pause(
    state: AppState,
    member: Member,
    (target, pause_id, input): (Target, String, PauseInput),
) -> Result<Patches<EditPage>, Error> {
    if let Err(errors) = Form::validate(&input) {
        return Ok(EditPage::pause().reject(&errors).into());
    }
    // A pause period belongs to a saved medication.
    if target.revision == 0 {
        return Err(Error::Missing);
    }
    let mut editor = crate::medication_edit::stored(&state, &member, &target).await?;
    if let Err(errors) = upsert_pause(&mut editor, &pause_id, &medications::random_id()?, &input)? {
        return Ok(EditPage::pause().reject(&errors).into());
    }
    let now = Timestamp::now().as_millis();
    let saved = medications::commit(&state, &member, editor, now).await?;
    Ok(pauses_saved(&saved))
}

pub async fn remove_pause(
    state: AppState,
    member: Member,
    (target, pause_id): (Target, String),
) -> Result<Patches<EditPage>, Error> {
    if target.revision == 0 {
        return Err(Error::Missing);
    }
    let mut editor = crate::medication_edit::stored(&state, &member, &target).await?;
    remove_pause_from(&mut editor, &pause_id)?;
    let now = Timestamp::now().as_millis();
    let saved = medications::commit(&state, &member, editor, now).await?;
    Ok(pauses_removed(&saved))
}

/// Whether today is paused and when the next dose is planned.
pub fn standing_banner() -> El<EditPage> {
    let saved = EditPage::target().at(Target::revision()).ge(1);
    div()
        .class(Class::new("med-pad"))
        .show(saved)
        .child(
            alert(AlertVariant::Default)
                .show(EditPage::pause_now().is_empty().not())
                .child(alert_title().bind_text(EditPage::pause_now())),
        )
        .child(
            p().class(Class::new("med-muted"))
                .show(EditPage::next_dose().is_empty().not())
                .bind_text(EditPage::next_dose()),
        )
}

pub fn pauses_card() -> El<EditPage> {
    let saved = EditPage::target().at(Target::revision()).ge(1);
    card()
        .child(
            card_header()
                .child(card_title(Level::H2).text("休薬期間"))
                .child(card_description().text(
                    "休薬期間は日単位です。開始日と終了日の両端を含む終日が休薬になり、その間は通知されません。終了日の翌日から通知が再開します。日付は日本時間（Asia/Tokyo）です。",
                )),
        )
        .child(
            card_content()
                .child(
                    p().class(Class::new("med-pad"))
                        .show(saved.clone().not())
                        .text("薬を保存すると、休薬期間を追加できます。"),
                )
                .child(
                    p().class(Class::new("med-pad"))
                        .show(saved.clone().and(EditPage::pauses().is_empty()))
                        .text("休薬期間がまだありません。下のフォームから追加してください。"),
                )
                .child(pause_list().show(saved.clone()))
                .child(pause_form().show(saved.clone()))
                .child(remove_dialog().show(saved)),
        )
}

fn pause_list() -> El<EditPage> {
    let form = EditPage::pause();
    ul().class(Class::new("med-list"))
        .child(hypersig::html::each_keyed(
            EditPage::pauses(),
            |item| item.at(PauseView::id()),
            |item| {
                li().class(Class::new("med-item"))
                    .child(
                        span()
                            .class(Class::new("med-item-text"))
                            .bind_text(item.at(PauseView::label())),
                    )
                    .child(
                        div()
                            .class(Class::new("med-actions"))
                            .child(
                                button(ButtonVariant::Outline, ButtonSize::Sm)
                                    .on_click(
                                        form.reset()
                                            .then(form.value().set(item.at(PauseView::input())))
                                            .then(
                                                EditPage::pause_editing()
                                                    .set(item.at(PauseView::id())),
                                            ),
                                    )
                                    .text("編集"),
                            )
                            .child(
                                REMOVE_DIALOG
                                    .trigger(ButtonVariant::Destructive, ButtonSize::Sm)
                                    .on_click(
                                        EditPage::removing().set(item.at(PauseView::id())).then(
                                            EditPage::removing_label()
                                                .set(item.at(PauseView::label())),
                                        ),
                                    )
                                    .text("削除"),
                            ),
                    )
            },
        ))
}

fn pause_form() -> El<EditPage> {
    let form = EditPage::pause();
    let editing = EditPage::pause_editing();
    let busy = EditPage::pause_busy();
    let adding = editing.is_empty();
    let save = SAVE_PAUSE
        .call_with((EditPage::target(), editing, form.value()))
        .indicator(busy)
        .error(EditPage::problem())
        .status(EditPage::status());
    form.form(save)
        .class(Class::new("med-form"))
        .child(card_title(Level::H3).bind_text(if_else(
            adding.clone(),
            "休薬期間を追加",
            "休薬期間を編集",
        )))
        .child(
            alert(AlertVariant::Destructive)
                .show(form.is_submitted().and(form.is_valid().not()))
                .child(alert_title().text("入力内容を確認してください"))
                .child(alert_description().text("赤字の項目を直すと保存できます。")),
        )
        .child(crate::form_fields::temporal_field(
            crate::form_fields::Temporal::Date,
            "開始日",
            form,
            PauseInput::start(),
            true,
        ))
        .child(crate::form_fields::temporal_field(
            crate::form_fields::Temporal::Date,
            "終了日",
            form,
            PauseInput::end(),
            true,
        ))
        .child(
            alert(AlertVariant::Default)
                .show(
                    EditPage::pause_notice()
                        .is_empty()
                        .not()
                        .and(form.is_dirty().not()),
                )
                .child(alert_title().bind_text(EditPage::pause_notice())),
        )
        .child(
            div()
                .class(Class::new("med-actions"))
                .child(
                    submit_button(ButtonVariant::Default, ButtonSize::Lg)
                        .disabled(busy)
                        .bind_text(if_else(adding.clone(), "期間を追加", "期間を更新")),
                )
                .child(
                    button(ButtonVariant::Outline, ButtonSize::Lg)
                        .show(adding.not())
                        .disabled(busy)
                        .on_click(form.reset().then(editing.set("")))
                        .text("編集をやめる"),
                ),
        )
}

fn remove_dialog() -> El<EditPage> {
    let busy = EditPage::pause_busy();
    let remove = REMOVE_PAUSE
        .call_with((EditPage::target(), EditPage::removing()))
        .indicator(busy)
        .error(EditPage::problem())
        .status(EditPage::status());
    REMOVE_DIALOG.alert_dialog(
        "休薬期間を削除しますか",
        "削除すると、この期間にも通知が届くようになります。元に戻すには、もう一度追加します。",
        div()
            .child(p().bind_text(EditPage::removing_label()))
            .child(
                dialog_footer()
                    .child(
                        REMOVE_DIALOG
                            .close(ButtonVariant::Outline, ButtonSize::Default)
                            .text("やめる"),
                    )
                    .child(
                        REMOVE_DIALOG
                            .close(ButtonVariant::Destructive, ButtonSize::Default)
                            .on_click(remove)
                            .text("削除する"),
                    ),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::medication_edit::{new_editor, render};
    use crate::medication_input::DoseSlotInput;
    use crate::medications::Slot;

    fn range(start: &str, end: &str) -> PauseInput {
        PauseInput {
            start: start.into(),
            end: end.into(),
        }
    }

    fn editor() -> Editor {
        let mut editor = new_editor("med".into(), "2026-10-01".into());
        editor.revision = 2;
        editor.medication.name = "薬".into();
        editor.slots.push(Slot {
            id: "slot".into(),
            input: DoseSlotInput {
                time: "08:00".into(),
                amount: "1".into(),
                unit: "錠".into(),
                timing: "unspecified".into(),
                note: String::new(),
            },
        });
        editor
    }

    fn add(editor: &mut Editor, id: &str, start: &str, end: &str) {
        upsert_pause(editor, "", id, &range(start, end))
            .ok()
            .unwrap()
            .ok()
            .unwrap();
    }

    fn at(date: &str, time: &str) -> i64 {
        date.parse::<LocalDate>()
            .unwrap()
            .at(time.parse().unwrap())
            .0
    }

    #[test]
    fn periods_are_added_edited_and_kept_ordered_by_start() {
        let mut editor = editor();
        add(&mut editor, "late", "2026-10-20", "2026-10-22");
        add(&mut editor, "early", "2026-10-05", "2026-10-06");
        add(&mut editor, "mid", "2026-10-10", "2026-10-10");
        let ids: Vec<_> = views(&editor).into_iter().map(|view| view.id).collect();
        assert_eq!(ids, ["early", "mid", "late"]);
        assert_eq!(editor.pauses[0].id, "early");
        assert!(editor.domain(0).is_ok());

        upsert_pause(
            &mut editor,
            "late",
            "unused",
            &range("2026-10-01", "2026-10-02"),
        )
        .ok()
        .unwrap()
        .ok()
        .unwrap();
        let ids: Vec<_> = views(&editor).into_iter().map(|view| view.id).collect();
        assert_eq!(ids, ["late", "early", "mid"]);
        assert_eq!(editor.pauses.len(), 3);

        // A period may be moved onto its own old range.
        upsert_pause(
            &mut editor,
            "mid",
            "unused",
            &range("2026-10-10", "2026-10-12"),
        )
        .ok()
        .unwrap()
        .ok()
        .unwrap();
        assert_eq!(views(&editor)[2].label, "2026-10-10 〜 2026-10-12");

        remove_pause_from(&mut editor, "early").unwrap();
        assert!(matches!(
            remove_pause_from(&mut editor, "early"),
            Err(Error::Missing)
        ));
        assert!(matches!(
            upsert_pause(&mut editor, "nope", "x", &range("2027-01-01", "2027-01-02")),
            Err(Error::Missing)
        ));
    }

    #[test]
    fn reversed_overlapping_and_empty_ranges_are_rejected_on_their_field() {
        let mut editor = editor();
        add(&mut editor, "a", "2026-10-10", "2026-10-12");
        let before = editor.pauses.len();
        let field_of = |input: PauseInput, editor: &mut Editor| {
            let errors = upsert_pause(editor, "", "b", &input)
                .ok()
                .unwrap()
                .err()
                .unwrap();
            (
                errors.message(PauseInput::start()).is_some(),
                errors.message(PauseInput::end()).is_some(),
            )
        };
        assert_eq!(
            field_of(range("2026-10-09", "2026-10-08"), &mut editor),
            (false, true)
        );
        assert_eq!(
            field_of(range("2026-10-12", "2026-10-13"), &mut editor),
            (true, false)
        );
        assert_eq!(
            field_of(range("2026-10-01", "2026-10-31"), &mut editor),
            (true, false)
        );
        assert_eq!(
            field_of(range("", "2026-10-31"), &mut editor),
            (true, false)
        );
        assert_eq!(
            field_of(range("2026-10-31", ""), &mut editor),
            (false, true)
        );
        assert_eq!(editor.pauses.len(), before);

        // Editing a period does not collide with itself.
        assert!(
            upsert_pause(&mut editor, "a", "x", &range("2026-10-11", "2026-10-12"))
                .ok()
                .unwrap()
                .is_ok()
        );
    }

    #[test]
    fn a_rejection_keeps_the_input_in_the_form() {
        let input = range("2026-10-09", "2026-10-08");
        let errors = PauseInput::parse(&input, &[]).unwrap_err();
        let events = Patches::from(EditPage::pause().reject(&errors)).to_event_stream();
        assert!(events.contains("pause.errors"), "{events}");
        assert!(events.contains("終了日は開始日以降です。"), "{events}");
        // A rejection sets messages only: the value and the baseline stay.
        assert!(!events.contains("pause.value"), "{events}");
        assert!(!events.contains("2026-10-09"), "{events}");
    }

    #[test]
    fn the_cap_is_a_message_on_the_start_field() {
        let mut editor = editor();
        for n in 0..MAX_PAUSES {
            let day = 1 + n;
            let date = format!("{:04}-01-01", 2100 + day);
            add(&mut editor, &format!("p{n}"), &date, &date);
        }
        let errors = upsert_pause(&mut editor, "", "extra", &range("2400-01-01", "2400-01-01"))
            .ok()
            .unwrap()
            .err()
            .unwrap();
        assert!(errors.message(PauseInput::start()).is_some());
        assert_eq!(editor.pauses.len(), MAX_PAUSES);
    }

    #[test]
    fn standing_follows_the_pause_and_resumes_after_its_end() {
        let mut editor = editor();
        let on = |editor: &Editor, date: &str, time: &str| standing(editor, at(date, time));

        let open = on(&editor, "2026-10-10", "07:00");
        assert_eq!(open.paused, "");
        assert_eq!(open.next, "次回の通知: 2026-10-10 08:00（Asia/Tokyo）");

        add(&mut editor, "p", "2026-10-10", "2026-10-12");
        for (date, time) in [("2026-10-10", "07:00"), ("2026-10-12", "23:59")] {
            let paused = on(&editor, date, time);
            assert_eq!(
                paused.paused,
                "今日は休薬中です（2026-10-10 〜 2026-10-12）。"
            );
            assert_eq!(paused.next, "次回の通知: 2026-10-13 08:00（Asia/Tokyo）");
        }
        let resumed = on(&editor, "2026-10-13", "07:00");
        assert_eq!(resumed.paused, "");
        assert_eq!(resumed.next, "次回の通知: 2026-10-13 08:00（Asia/Tokyo）");

        remove_pause_from(&mut editor, "p").unwrap();
        let removed = on(&editor, "2026-10-10", "07:00");
        assert_eq!(removed.next, "次回の通知: 2026-10-10 08:00（Asia/Tokyo）");

        editor.enabled = false;
        assert!(on(&editor, "2026-10-10", "07:00").next.contains("無効"));
        let unsaved = standing(&new_editor("x".into(), String::new()), 0);
        assert_eq!(
            unsaved,
            Standing {
                paused: String::new(),
                next: String::new()
            }
        );
    }

    #[test]
    fn saved_patches_carry_the_sorted_list_the_reset_form_and_the_standing() {
        let mut editor = editor();
        add(&mut editor, "b", "2026-10-20", "2026-10-21");
        add(&mut editor, "a", "2026-10-05", "2026-10-06");
        editor.revision = 3;
        let patches = pauses_saved(&editor);
        let list = patches.value_of(EditPage::pauses()).unwrap();
        assert_eq!(list[0].id, "a");
        assert_eq!(list[1].id, "b");
        assert_eq!(patches.value_of(EditPage::target()).unwrap().revision, 3);
        assert_eq!(
            patches.value_of(EditPage::pause_editing()).as_deref(),
            Some("")
        );
        let form = patches.value_of(EditPage::pause()).unwrap();
        let json = serde_json::to_value(&form).unwrap();
        assert_eq!(json["initial"], json["value"]);
        assert!(patches.value_of(EditPage::next_dose()).is_some());

        remove_pause_from(&mut editor, "a").unwrap();
        let patches = pauses_removed(&editor);
        assert_eq!(patches.value_of(EditPage::pauses()).unwrap().len(), 1);
        assert_eq!(patches.value_of(EditPage::removing()).as_deref(), Some(""));
    }

    #[test]
    fn the_page_lists_pauses_asks_before_deleting_and_explains_the_rules() {
        let mut editor = editor();
        add(&mut editor, "a", "2026-10-05", "2026-10-06");
        let page = render(&editor);
        assert_eq!(page.value_of(EditPage::pauses()).len(), 1);
        let html = page.render();
        assert!(html.contains(SAVE_PAUSE.path()), "{html}");
        assert!(html.contains(REMOVE_PAUSE.path()), "{html}");
        assert!(html.contains(r#"role="alertdialog""#), "{html}");
        assert!(html.contains(r#"command="show-modal""#), "{html}");
        assert!(
            html.contains(r#"aria-labelledby="remove-pause-title""#),
            "{html}"
        );
        assert!(html.contains("両端を含む終日"), "{html}");
        assert!(html.contains("終了日の翌日から通知が再開"), "{html}");
        assert!(html.contains("Asia/Tokyo"), "{html}");
        assert!(html.contains("休薬期間がまだありません"), "{html}");
        assert!(html.contains("data-hs-key"), "{html}");
        for id in ["ui-pause-value-start-input", "ui-pause-value-end-input"] {
            assert!(html.contains(id), "{id}\n{html}");
        }
    }
}
