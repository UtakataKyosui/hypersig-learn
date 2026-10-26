//! The medication edit page: the medication's fields, its dose slots, and
//! the actions that save them.
//!
//! Each action loads the stored medication, changes only the part it owns
//! (the medication's own fields, or one dose slot) and writes it back under
//! the revision the page last saw. What it does not own, such as the pause
//! periods, is kept as stored.
use crate::{
    AppState,
    auth::Member,
    domain::UtcInstant,
    medication_form::{MedicationForm, REPEATS, SlotView, TIMINGS, blank_slot},
    medication_input::{DoseSlotInput, MedicationInput, PauseInput},
    medication_style::{STYLE, Tab, head, tab_bar, theme},
    medications::{self, Editor, Error, Slot},
    pause_edit::{self, PauseView},
};
use hypersig::html::{Class, El, a, div, each_keyed, fieldset, h1, legend, li, main, p, span, ul};
use hypersig::{Action, FieldErrors, Form, Page, Patches, SignalPatch, Signals, if_else};
use hypersig_ui::{
    AlertVariant, ButtonSize, ButtonVariant, Dialog, Level, NumberField, TextField, alert,
    alert_description, alert_title, button, card, card_content, card_description, card_header,
    card_title, checkbox_in_form, dialog_footer, radio_group_in_form, select_field_in_form,
    submit_button, switch_in_form,
};
use serde::{Deserialize, Serialize};
use std::ops::Not as _;

/// The medication being edited and the revision the page last saw of it.
/// Every save sends it, and the server refuses a save made on an older one.
#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct Target {
    pub id: String,
    /// Zero until the medication is first saved.
    pub revision: i32,
}

#[derive(Signals, Serialize, Deserialize)]
pub struct EditPage {
    pub target: Target,
    pub medication: Form<MedicationForm>,
    /// The dose slots, earliest time first.
    pub slots: Vec<SlotView>,
    /// The dose slot form: adds a slot, or edits the one named by `editing`.
    pub slot: Form<DoseSlotInput>,
    /// The id of the dose slot `slot` edits, or `""` while it adds one.
    pub editing: String,
    /// The summary line of that slot, for the delete dialog.
    pub editing_label: String,
    pub busy: bool,
    pub slot_busy: bool,
    pub notice: String,
    pub slot_notice: String,
    /// The text of a failed call, shown apart from the field messages.
    pub problem: String,
    /// The HTTP status of a failed call, `0` while none failed.
    pub status: i32,
    /// The pause periods, earliest first.
    pub pauses: Vec<PauseView>,
    /// The pause form: adds a period, or edits the one named by `pause_editing`.
    pub pause: Form<PauseInput>,
    /// The id of the pause period `pause` edits, or `""` while it adds one.
    pub pause_editing: String,
    pub pause_busy: bool,
    pub pause_notice: String,
    /// The id and the label of the pause period the delete dialog asks about.
    pub removing: String,
    pub removing_label: String,
    /// Says that today is paused, or `""` while it is not.
    pub pause_now: String,
    /// When the next dose is planned.
    pub next_dose: String,
}

pub const SAVE_MEDICATION: Action<EditPage, (Target, MedicationForm)> =
    Action::post("/medications/edit/save");
pub const SAVE_SLOT: Action<EditPage, (Target, String, DoseSlotInput)> =
    Action::post("/medications/edit/slot/save");
pub const REMOVE_SLOT: Action<EditPage, (Target, String)> =
    Action::post("/medications/edit/slot/remove");

/// The most dose slots one medication holds.
const MAX_SLOTS: usize = 64;

/// A medication that has not been saved yet.
#[must_use]
pub fn new_editor(id: String, start: String) -> Editor {
    Editor {
        id,
        revision: 0,
        enabled: true,
        medication: MedicationForm::blank(start).to_input(),
        slots: vec![],
        pauses: vec![],
    }
}

pub async fn new_page(_: AppState, _: Member, (): ()) -> Result<Page<EditPage>, Error> {
    let now = hypersig::Timestamp::now().as_millis();
    let start = UtcInstant(now)
        .tokyo_date()
        .map_err(|_| Error::Invalid)?
        .to_string();
    Ok(render(&new_editor(medications::random_id()?, start)))
}

pub async fn page(state: AppState, member: Member, id: String) -> Result<Page<EditPage>, Error> {
    let row = crate::repository::load(&state.db, member.owner(), &id).await?;
    Ok(render(&row.editor()?))
}

/// The edit page for `editor`.
#[must_use]
pub fn render(editor: &Editor) -> Page<EditPage> {
    let standing = pause_edit::standing(editor, hypersig::Timestamp::now().as_millis());
    let state = EditPage {
        target: Target {
            id: editor.id.clone(),
            revision: editor.revision,
        },
        medication: Form::new(MedicationForm::from_input(
            &editor.medication,
            editor.enabled,
        )),
        slots: slot_views(editor),
        slot: Form::new(blank_slot()),
        editing: String::new(),
        editing_label: String::new(),
        busy: false,
        slot_busy: false,
        notice: String::new(),
        slot_notice: String::new(),
        problem: String::new(),
        status: 0,
        pauses: pause_edit::views(editor),
        pause: Form::new(pause_edit::blank_pause()),
        pause_editing: String::new(),
        pause_busy: false,
        pause_notice: String::new(),
        removing: String::new(),
        removing_label: String::new(),
        pause_now: standing.paused,
        next_dose: standing.next,
    };
    let title = if editor.revision == 0 {
        "お薬を登録"
    } else {
        "お薬を編集"
    };
    Page::new(title, state)
        .global(hypersig_ui::global())
        .theme(hypersig_ui::theme())
        .theme(theme())
        .style(STYLE)
        .child(view())
}

/// The dose slots of `editor`, earliest time first. The times are `HH:mm`,
/// so the text order is the clock order.
#[must_use]
pub fn slot_views(editor: &Editor) -> Vec<SlotView> {
    let mut views: Vec<_> = editor.slots.iter().cloned().map(SlotView::from).collect();
    views.sort_by(|a, b| (&a.input.time, &a.id).cmp(&(&b.input.time, &b.id)));
    views
}

/// Asks for confirmation before a dose slot is deleted.
const REMOVE_SLOT_DIALOG: Dialog<EditPage> = Dialog::new("remove-slot");

fn view() -> El<EditPage> {
    main()
        .class(Class::new("med-shell"))
        .child(head(
            h1().bind_text(if_else(
                EditPage::target().at(Target::revision()).ge(1),
                "お薬を編集",
                "お薬を登録",
            )),
            p().text("あなたのペースで、忘れずに。"),
        ))
        .child(
            a().href(crate::MEDICATIONS.link())
                .class(Class::new("med-link"))
                .text("← 今日の予定へ戻る"),
        )
        .child(crate::failure::alerts(
            EditPage::problem(),
            EditPage::status(),
        ))
        .child(pause_edit::standing_banner())
        .child(medication_card())
        .child(slots_card())
        .child(pause_edit::pauses_card())
        .child(
            div()
                .class(Class::new("med-actions"))
                .child(crate::auth::sign_out_button()),
        )
        .child(tab_bar(Tab::Medications))
}

fn medication_card() -> El<EditPage> {
    let form = EditPage::medication();
    let repeat = form.field(MedicationForm::repeat());
    let busy = EditPage::busy();
    let save = SAVE_MEDICATION
        .call_with((EditPage::target(), form.value()))
        .indicator(busy)
        .error(EditPage::problem())
        .status(EditPage::status());
    let required =
        |kind, caption, field| crate::form_fields::temporal_field(kind, caption, form, field, true);
    card()
        .class(Class::new("med-card"))
        .child(
            card_header()
                .child(card_title(Level::H2).text("お薬と繰り返し"))
                .child(card_description().text("開始日から、設定した繰り返しで服薬します。")),
        )
        .child(
            card_content().child(
                form.form(save)
                    .class(Class::new("med-form"))
                    .child(
                        alert(AlertVariant::Destructive)
                            .show(form.is_submitted().and(form.is_valid().not()))
                            .child(alert_title().text("入力内容を確認してください"))
                            .child(alert_description().text("赤字の項目を直すと保存できます。")),
                    )
                    .child(
                        TextField::in_form("お薬の名前", form, MedicationForm::name())
                            .required()
                            .render(),
                    )
                    .child(
                        radio_group_in_form(
                            "繰り返し",
                            form,
                            MedicationForm::repeat(),
                            REPEATS.map(|(value, label)| (value.to_owned(), label)),
                        )
                        .class(Class::new("med-seg")),
                    )
                    .child(
                        div().show(repeat.equals("interval")).child(
                            NumberField::in_form(
                                "間隔（日ごと）",
                                form,
                                MedicationForm::interval(),
                            )
                            .min(1)
                            .required()
                            .render(),
                        ),
                    )
                    .child(weekdays().show(repeat.equals("weekdays")))
                    .child(required(
                        crate::form_fields::Temporal::Date,
                        "開始日",
                        MedicationForm::start(),
                    ))
                    .child(crate::form_fields::date_field(
                        "終了日（任意）",
                        form,
                        MedicationForm::end(),
                    ))
                    .child(switch_in_form(
                        "通知を有効にする",
                        form,
                        MedicationForm::enabled(),
                    ))
                    .child(
                        p().class(Class::new("med-muted"))
                            .show(form.is_dirty().and(busy.not()))
                            .text("未保存の変更があります。"),
                    )
                    .child(
                        alert(AlertVariant::Default)
                            .show(
                                EditPage::notice()
                                    .is_empty()
                                    .not()
                                    .and(form.is_dirty().not()),
                            )
                            .child(alert_title().bind_text(EditPage::notice())),
                    )
                    .child(
                        submit_button(ButtonVariant::Default, ButtonSize::Lg)
                            .disabled(busy)
                            .bind_text(if_else(busy, "保存中…", "この内容で保存")),
                    ),
            ),
        )
}

fn weekdays() -> El<EditPage> {
    let form = EditPage::medication();
    let days = [
        ("月", MedicationForm::mon()),
        ("火", MedicationForm::tue()),
        ("水", MedicationForm::wed()),
        ("木", MedicationForm::thu()),
        ("金", MedicationForm::fri()),
        ("土", MedicationForm::sat()),
        ("日", MedicationForm::sun()),
    ];
    days.into_iter().fold(
        fieldset()
            .class(Class::new("med-days"))
            .child(legend().text("曜日")),
        |group, (label, field)| group.child(checkbox_in_form(label, form, field)),
    )
}

fn slots_card() -> El<EditPage> {
    let saved = EditPage::target().at(Target::revision()).ge(1);
    card()
        .class(Class::new("med-card"))
        .child(
            card_header()
                .child(card_title(Level::H2).text("服薬枠"))
                .child(
                    card_description()
                        .text("1日に複数回飲む場合は、時刻と量ごとに枠を追加します。"),
                ),
        )
        .child(
            card_content()
                .child(
                    p().class(Class::new("med-pad"))
                        .show(saved.clone().not())
                        .text("薬を保存すると、服薬枠を追加できます。"),
                )
                .child(
                    p().class(Class::new("med-pad"))
                        .show(saved.clone().and(EditPage::slots().is_empty()))
                        .text("服薬枠がまだありません。下のフォームから追加してください。"),
                )
                .child(slot_list().show(saved.clone()))
                .child(slot_form().show(saved.clone()))
                .child(remove_slot_dialog().show(saved)),
        )
}

fn slot_list() -> El<EditPage> {
    let slot = EditPage::slot();
    ul().class(Class::new("med-list")).child(each_keyed(
        EditPage::slots(),
        |item| item.at(SlotView::id()),
        |item| {
            li().class(Class::new("med-item"))
                .class(Class::new("med-entry"))
                .child(
                    span()
                        .class(Class::new("med-item-text"))
                        .child(span().bind_text(item.at(SlotView::summary())))
                        .child(
                            span()
                                .class(Class::new("med-muted"))
                                .show(
                                    item.at(SlotView::input())
                                        .at(DoseSlotInput::note())
                                        .is_empty()
                                        .not(),
                                )
                                .bind_text(item.at(SlotView::input()).at(DoseSlotInput::note())),
                        ),
                )
                .child(
                    button(ButtonVariant::Outline, ButtonSize::Sm)
                        .on_click(
                            slot.reset()
                                .then(slot.value().set(item.at(SlotView::input())))
                                .then(EditPage::editing().set(item.at(SlotView::id())))
                                .then(EditPage::editing_label().set(item.at(SlotView::summary()))),
                        )
                        .text("編集"),
                )
        },
    ))
}

fn slot_form() -> El<EditPage> {
    let form = EditPage::slot();
    let editing = EditPage::editing();
    let slot_busy = EditPage::slot_busy();
    let adding = editing.is_empty();
    let save = SAVE_SLOT
        .call_with((EditPage::target(), editing, form.value()))
        .indicator(slot_busy)
        .error(EditPage::problem())
        .status(EditPage::status());
    form.form(save)
        .class(Class::new("med-form"))
        .child(card_title(Level::H3).bind_text(if_else(
            adding.clone(),
            "服薬枠を追加",
            "服薬枠を編集",
        )))
        .child(
            alert(AlertVariant::Destructive)
                .show(form.is_submitted().and(form.is_valid().not()))
                .child(alert_title().text("入力内容を確認してください"))
                .child(alert_description().text("赤字の項目を直すと保存できます。")),
        )
        .child(crate::form_fields::temporal_field(
            crate::form_fields::Temporal::Time,
            "お薬の通知時刻",
            form,
            DoseSlotInput::time(),
            true,
        ))
        .child(
            div()
                .class(Class::new("med-row2"))
                .child(
                    TextField::in_form("1回の量", form, DoseSlotInput::amount())
                        .input_mode(hypersig_ui::InputMode::Decimal)
                        .description("0.01〜1000、小数2桁まで")
                        .required()
                        .render(),
                )
                .child(
                    TextField::in_form("単位", form, DoseSlotInput::unit())
                        .description("錠・包・mL・mgなど、20文字以内")
                        .required()
                        .render(),
                ),
        )
        .child(select_field_in_form(
            "タイミング",
            form,
            DoseSlotInput::timing(),
            TIMINGS.map(|(value, label)| (value.to_owned(), label)),
        ))
        .child(TextField::in_form("メモ（任意）", form, DoseSlotInput::note()).render())
        .child(
            alert(AlertVariant::Default)
                .show(
                    EditPage::slot_notice()
                        .is_empty()
                        .not()
                        .and(form.is_dirty().not()),
                )
                .child(alert_title().bind_text(EditPage::slot_notice())),
        )
        .child(
            div()
                .class(Class::new("med-actions"))
                .child(
                    submit_button(ButtonVariant::Default, ButtonSize::Lg)
                        .disabled(slot_busy)
                        .bind_text(if_else(adding.clone(), "枠を追加", "枠を更新")),
                )
                .child(
                    button(ButtonVariant::Outline, ButtonSize::Lg)
                        .show(adding.clone().not())
                        .disabled(slot_busy)
                        .on_click(form.reset().then(editing.set("")))
                        .text("編集をやめる"),
                )
                .child(
                    REMOVE_SLOT_DIALOG
                        .trigger(ButtonVariant::Destructive, ButtonSize::Lg)
                        .show(adding.not())
                        .disabled(slot_busy)
                        .text("この枠を削除"),
                ),
        )
}

fn remove_slot_dialog() -> El<EditPage> {
    let remove = REMOVE_SLOT
        .call_with((EditPage::target(), EditPage::editing()))
        .indicator(EditPage::slot_busy())
        .error(EditPage::problem())
        .status(EditPage::status());
    REMOVE_SLOT_DIALOG.alert_dialog(
        "服薬枠を削除しますか",
        "削除すると、この枠の時刻には通知されなくなります。元に戻すには、もう一度追加します。",
        div().child(p().bind_text(EditPage::editing_label())).child(
            dialog_footer()
                .child(
                    REMOVE_SLOT_DIALOG
                        .close(ButtonVariant::Outline, ButtonSize::Default)
                        .text("やめる"),
                )
                .child(
                    REMOVE_SLOT_DIALOG
                        .close(ButtonVariant::Destructive, ButtonSize::Default)
                        .on_click(remove)
                        .text("削除する"),
                ),
        ),
    )
}

/// The page after the medication was saved: the form starts from the saved
/// values, so it is no longer dirty.
fn medication_saved(editor: &Editor) -> Patches<EditPage> {
    let form = MedicationForm::from_input(&editor.medication, editor.enabled);
    Patches::new().signals(
        EditPage::medication()
            .patch(Form::new(form))
            .and(target_patch(editor))
            .and(pause_edit::standing_patch(editor))
            .and(EditPage::notice().patch("保存しました。".to_owned())),
    )
}

pub(crate) fn target_patch(editor: &Editor) -> SignalPatch<EditPage> {
    EditPage::target().patch(Target {
        id: editor.id.clone(),
        revision: editor.revision,
    })
}

/// The dose slot form starts over, ready to add the next slot.
fn slot_form_reset() -> SignalPatch<EditPage> {
    EditPage::slot()
        .patch(Form::new(blank_slot()))
        .and(EditPage::editing().patch(String::new()))
        .and(EditPage::editing_label().patch(String::new()))
}

fn slot_saved(editor: &Editor) -> Patches<EditPage> {
    slots_changed(editor, "服薬枠を保存しました。")
}

fn slot_removed(editor: &Editor) -> Patches<EditPage> {
    slots_changed(editor, "服薬枠を削除しました。")
}

/// The whole slot list goes out again, in time order: an upsert would put a
/// new slot last, and the list keeps each row by its key across the reorder.
fn slots_changed(editor: &Editor, notice: &str) -> Patches<EditPage> {
    Patches::new().signals(
        target_patch(editor)
            .and(slot_form_reset())
            .and(EditPage::slots().patch(slot_views(editor)))
            .and(pause_edit::standing_patch(editor))
            .and(EditPage::slot_notice().patch(notice.to_owned())),
    )
}

/// Messages for the field they belong to, shown until the next submit.
fn rejected<T: hypersig::Validate>(
    form: hypersig::Sig<EditPage, Form<T>>,
    errors: &FieldErrors<T>,
) -> Patches<EditPage> {
    form.reject(errors).into()
}

/// The stored medication `target` names, or a new one when it has none yet.
/// A save made on a revision other than the stored one is a conflict.
pub(crate) async fn stored(
    state: &AppState,
    member: &Member,
    target: &Target,
) -> Result<Editor, Error> {
    if target.revision == 0 {
        return Ok(new_editor(target.id.clone(), String::new()));
    }
    let row = crate::repository::load(&state.db, member.owner(), &target.id).await?;
    if row.revision != target.revision {
        return Err(Error::Conflict);
    }
    row.editor()
}

/// `editor` with the medication's own fields replaced. Its dose slots and
/// pause periods stay as stored.
fn with_medication(mut editor: Editor, form: &MedicationForm, input: MedicationInput) -> Editor {
    editor.medication = input;
    editor.enabled = form.enabled;
    editor
}

pub async fn save_medication(
    state: AppState,
    member: Member,
    (target, form): (Target, MedicationForm),
) -> Result<Patches<EditPage>, Error> {
    let input = match form.parse() {
        Ok(input) => input,
        Err(errors) => return Ok(rejected(EditPage::medication(), &errors)),
    };
    let editor = with_medication(stored(&state, &member, &target).await?, &form, input);
    let now = hypersig::Timestamp::now().as_millis();
    let saved = medications::commit(&state, &member, editor, now).await?;
    let patches = medication_saved(&saved);
    Ok(if target.revision == 0 {
        patches.navigate(crate::MEDICATION.url(saved.id))
    } else {
        patches
    })
}

/// Adds the slot `input` to `editor`, or replaces the slot `slot_id` when it
/// names one. Input that fails the form's rules or the server's parse comes
/// back as messages for the form's fields.
fn upsert_slot(
    editor: &mut Editor,
    slot_id: &str,
    fresh_id: &str,
    input: DoseSlotInput,
) -> Result<Result<SlotView, FieldErrors<DoseSlotInput>>, Error> {
    let adding = slot_id.is_empty();
    let id = if adding { fresh_id } else { slot_id };
    if let Err(errors) = input.parse(id.parse().map_err(|_| Error::Invalid)?) {
        return Ok(Err(errors));
    }
    let view = SlotView::new(
        id.to_owned(),
        DoseSlotInput {
            unit: input.unit.trim().to_owned(),
            ..input
        },
    );
    let slot = Slot {
        id: view.id.clone(),
        input: view.input.clone(),
    };
    if adding {
        if editor.slots.len() >= MAX_SLOTS {
            return Ok(Err(FieldErrors::new().add(
                DoseSlotInput::time(),
                format!("服薬枠は{MAX_SLOTS}件までです。"),
            )));
        }
        editor.slots.push(slot);
    } else {
        let at = editor
            .slots
            .iter_mut()
            .find(|existing| existing.id == slot_id)
            .ok_or(Error::Missing)?;
        *at = slot;
    }
    Ok(Ok(view))
}

pub async fn save_slot(
    state: AppState,
    member: Member,
    (target, slot_id, input): (Target, String, DoseSlotInput),
) -> Result<Patches<EditPage>, Error> {
    if let Err(errors) = Form::validate(&input) {
        return Ok(rejected(EditPage::slot(), &errors));
    }
    // A slot belongs to a saved medication.
    if target.revision == 0 {
        return Err(Error::Missing);
    }
    let mut editor = stored(&state, &member, &target).await?;
    if let Err(errors) = upsert_slot(&mut editor, &slot_id, &medications::random_id()?, input)? {
        return Ok(rejected(EditPage::slot(), &errors));
    }
    let now = hypersig::Timestamp::now().as_millis();
    let saved = medications::commit(&state, &member, editor, now).await?;
    Ok(slot_saved(&saved))
}

/// Removes the slot `slot_id` from `editor`.
fn remove_slot_from(editor: &mut Editor, slot_id: &str) -> Result<(), Error> {
    let before = editor.slots.len();
    editor.slots.retain(|slot| slot.id != slot_id);
    if editor.slots.len() == before {
        return Err(Error::Missing);
    }
    Ok(())
}

pub async fn remove_slot(
    state: AppState,
    member: Member,
    (target, slot_id): (Target, String),
) -> Result<Patches<EditPage>, Error> {
    if target.revision == 0 {
        return Err(Error::Missing);
    }
    let mut editor = stored(&state, &member, &target).await?;
    remove_slot_from(&mut editor, &slot_id)?;
    let now = hypersig::Timestamp::now().as_millis();
    let saved = medications::commit(&state, &member, editor, now).await?;
    Ok(slot_removed(&saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::medication_input::PauseInput;
    use crate::medications::Pause;
    use axum::http::StatusCode;
    use axum::response::IntoResponse as _;

    fn saved_editor() -> Editor {
        let mut editor = new_editor("med".into(), "2026-10-05".into());
        editor.revision = 3;
        editor.medication.name = "薬".into();
        editor
    }

    fn slot(time: &str, amount: &str, timing: &str) -> DoseSlotInput {
        DoseSlotInput {
            time: time.into(),
            amount: amount.into(),
            unit: "錠".into(),
            timing: timing.into(),
            note: String::new(),
        }
    }

    fn added(editor: &mut Editor, id: &str, input: DoseSlotInput) -> SlotView {
        upsert_slot(editor, "", id, input)
            .ok()
            .unwrap()
            .ok()
            .unwrap()
    }

    #[test]
    fn page_links_and_controls_come_from_typed_items() {
        let html = render(&saved_editor()).render();
        assert!(html.contains(crate::MEDICATIONS.path()), "{html}");
        assert!(html.contains(crate::auth::LOGIN.path()), "{html}");
        assert!(html.contains(SAVE_MEDICATION.path()), "{html}");
        assert!(html.contains(SAVE_SLOT.path()), "{html}");
        assert!(html.contains(REMOVE_SLOT.path()), "{html}");
        for id in [
            "ui-medication-value-name-input",
            "ui-medication-value-start-input",
            "ui-medication-value-end-input",
            "ui-slot-value-time-input",
            "ui-slot-value-amount-input",
        ] {
            assert!(html.contains(id), "{id}\n{html}");
        }
        assert!(html.contains(r#"type="date""#), "{html}");
        assert!(html.contains(r#"type="time""#), "{html}");
        assert!(html.contains(r#"inputmode="decimal""#), "{html}");
        assert!(html.contains(r#"role="radiogroup""#), "{html}");
        assert!(html.contains("<select"), "{html}");
        assert!(html.contains(r#"type="submit""#), "{html}");
        assert!(html.contains("novalidate"), "{html}");
        assert!(html.contains("data-hs-key"), "{html}");
    }

    #[test]
    fn page_starts_from_the_stored_medication_with_a_clean_form() {
        let mut editor = saved_editor();
        editor.medication.repeat = "weekdays".into();
        editor.medication.weekdays = 1 | 4;
        editor.enabled = false;
        added(&mut editor, "slot-a", slot("08:00", "1", "after_meal"));
        let page = render(&editor);
        let form = page.value_of(EditPage::medication());
        assert!(form.value().mon && form.value().wed && !form.value().tue);
        assert!(!form.value().enabled);
        let json = serde_json::to_value(&form).unwrap();
        assert_eq!(json["initial"], json["value"]);
        assert_eq!(page.value_of(EditPage::target()).revision, 3);
        assert_eq!(page.value_of(EditPage::slots()).len(), 1);
        assert_eq!(page.value_of(EditPage::status()), 0);
    }

    #[test]
    fn saving_the_medication_clears_dirty_and_keeps_slots_and_pauses() {
        let mut stored = saved_editor();
        added(&mut stored, "slot-a", slot("08:00", "1", "after_meal"));
        stored.pauses.push(Pause {
            id: "pause".into(),
            input: PauseInput {
                start: "2026-10-10".into(),
                end: "2026-10-12".into(),
            },
        });
        let mut form = MedicationForm::from_input(&stored.medication, true);
        form.name = "  新しい薬 ".into();
        form.repeat = "interval".into();
        form.interval = 3;
        form.enabled = false;
        let input = form.parse().ok().unwrap();
        let merged = with_medication(stored, &form, input);
        assert_eq!(merged.slots.len(), 1);
        assert_eq!(merged.pauses.len(), 1);
        assert!(!merged.enabled);
        assert!(merged.domain(0).is_ok());

        let mut saved = merged;
        saved.medication.name = saved.medication.name.trim().into();
        saved.revision += 1;
        let patches = medication_saved(&saved);
        let form = patches.value_of(EditPage::medication()).unwrap();
        assert_eq!(form.value().name, "新しい薬");
        assert_eq!(form.value().interval, 3);
        let json = serde_json::to_value(&form).unwrap();
        assert_eq!(json["initial"], json["value"]);
        assert_eq!(json["submitted"], false);
        assert_eq!(patches.value_of(EditPage::target()).unwrap().revision, 4);
        assert_eq!(
            patches.value_of(EditPage::notice()).as_deref(),
            Some("保存しました。")
        );
    }

    #[test]
    fn invalid_medication_input_is_rejected_on_its_fields() {
        let mut form = MedicationForm::blank("2026-10-05".into());
        let errors = form.parse().err().unwrap();
        assert!(errors.message(MedicationForm::name()).is_some());
        form.name = "薬".into();
        form.repeat = "weekdays".into();
        let errors = form.parse().err().unwrap();
        let patches = rejected(EditPage::medication(), &errors);
        let events = patches.to_event_stream();
        assert!(events.contains("medication.errors"), "{events}");
        assert!(events.contains("曜日を選択してください。"), "{events}");
    }

    #[test]
    fn one_medication_holds_slots_that_differ_in_time_amount_and_timing() {
        let mut editor = saved_editor();
        let morning = added(&mut editor, "slot-a", slot("08:00", "1", "after_meal"));
        let noon = added(&mut editor, "slot-b", slot("12:30", "0.5", "before_meal"));
        let night = added(&mut editor, "slot-c", slot("22:00", "2", "bedtime"));
        assert_eq!(editor.slots.len(), 3);
        assert_eq!(morning.summary, "08:00 · 1 錠 · 食後");
        assert_eq!(noon.summary, "12:30 · 0.5 錠 · 食前");
        assert_eq!(night.summary, "22:00 · 2 錠 · 就寝前");
        let domain = editor.domain(0).unwrap();
        assert_eq!(domain.slots.len(), 3);

        let edited = upsert_slot(
            &mut editor,
            "slot-b",
            "unused",
            slot("13:00", "0.75", "other"),
        )
        .ok()
        .unwrap()
        .ok()
        .unwrap();
        assert_eq!(edited.id, "slot-b");
        assert_eq!(editor.slots.len(), 3);
        assert_eq!(editor.slots[1].input.time, "13:00");
        assert_eq!(editor.slots[1].input.amount, "0.75");
        assert_eq!(editor.slots[0].input.time, "08:00");
    }

    #[test]
    fn slot_input_errors_stay_on_fields_and_unknown_slots_are_missing() {
        let mut editor = saved_editor();
        let errors = upsert_slot(&mut editor, "", "slot-a", slot("08:00", "0", "after_meal"))
            .ok()
            .unwrap()
            .err()
            .unwrap();
        assert!(errors.message(DoseSlotInput::amount()).is_some());
        let errors = upsert_slot(&mut editor, "", "slot-a", slot("25:00", "1", "after_meal"))
            .ok()
            .unwrap()
            .err()
            .unwrap();
        assert!(errors.message(DoseSlotInput::time()).is_some());
        assert!(editor.slots.is_empty());
        let missing = upsert_slot(&mut editor, "nope", "x", slot("08:00", "1", "after_meal"));
        assert!(matches!(missing, Err(Error::Missing)));

        for n in 0..MAX_SLOTS {
            added(
                &mut editor,
                &format!("slot-{n}"),
                slot("08:00", "1", "unspecified"),
            );
        }
        let errors = upsert_slot(
            &mut editor,
            "",
            "slot-extra",
            slot("09:00", "1", "unspecified"),
        )
        .ok()
        .unwrap()
        .err()
        .unwrap();
        assert!(errors.message(DoseSlotInput::time()).is_some());
        assert_eq!(editor.slots.len(), MAX_SLOTS);
    }

    #[test]
    fn slot_patches_send_the_list_in_time_order_and_reset_the_slot_form() {
        let mut editor = saved_editor();
        added(&mut editor, "slot-b", slot("20:00", "1", "bedtime"));
        added(&mut editor, "slot-a", slot("08:00", "1", "after_meal"));
        editor.revision = 4;
        let patches = slot_saved(&editor);
        let list = patches.value_of(EditPage::slots()).unwrap();
        let ids: Vec<_> = list.iter().map(|view| view.id.as_str()).collect();
        assert_eq!(ids, ["slot-a", "slot-b"]);
        assert_eq!(patches.value_of(EditPage::target()).unwrap().revision, 4);
        assert_eq!(patches.value_of(EditPage::editing()).as_deref(), Some(""));
        assert_eq!(
            patches.value_of(EditPage::editing_label()).as_deref(),
            Some("")
        );
        let form = patches.value_of(EditPage::slot()).unwrap();
        let json = serde_json::to_value(&form).unwrap();
        assert_eq!(json["initial"], json["value"]);

        remove_slot_from(&mut editor, "slot-a").unwrap();
        assert!(matches!(
            remove_slot_from(&mut editor, "slot-a"),
            Err(Error::Missing)
        ));
        let patches = slot_removed(&editor);
        let list = patches.value_of(EditPage::slots()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "slot-b");
    }

    #[test]
    fn deleting_a_slot_asks_first_and_failures_have_their_alerts() {
        let mut editor = saved_editor();
        added(&mut editor, "slot-a", slot("08:00", "1", "after_meal"));
        let html = render(&editor).render();
        assert!(
            html.contains(r#"aria-labelledby="remove-slot-title""#),
            "{html}"
        );
        assert!(html.contains("服薬枠を削除しますか"), "{html}");
        assert!(html.contains(REMOVE_SLOT.path()), "{html}");
        assert!(html.contains("ログインの有効期限が切れました"), "{html}");
        assert!(html.contains("通信できませんでした"), "{html}");
        // The delete call is made by the dialog's confirm button only.
        assert_eq!(html.matches(REMOVE_SLOT.path()).count(), 1, "{html}");
    }

    #[test]
    fn input_errors_and_communication_failures_have_distinct_statuses() {
        let status = |error: Error| error.into_response().status();
        assert_eq!(status(Error::Invalid), StatusCode::BAD_REQUEST);
        assert_eq!(status(Error::Missing), StatusCode::NOT_FOUND);
        assert_eq!(status(Error::Conflict), StatusCode::CONFLICT);
        assert_eq!(status(Error::Store), StatusCode::INTERNAL_SERVER_ERROR);
        // A field message is a 200 patch, not an error response.
        let errors = FieldErrors::new().add(DoseSlotInput::time(), "x");
        assert!(
            rejected(EditPage::slot(), &errors)
                .to_event_stream()
                .contains("slot.errors")
        );
    }
}
