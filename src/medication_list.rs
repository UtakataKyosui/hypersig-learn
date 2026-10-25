//! The medication list: today's schedule, the next notification, and one
//! entry per medication with the state it is in. Each entry links to its edit
//! page and can be deleted after a confirmation.
use crate::{
    AppState,
    auth::Member,
    domain::{LocalDate, UtcInstant},
    medication_form::SlotView,
    medication_style::{STYLE, Tab, head, tab_bar},
    medications::{Delete, Editor, Error, Settings, Summary, TodayDose, summary},
    schedule_status,
    schedule_view::{NextPlannedDose, TIMEZONE_LABEL, day_label, tokyo_clock, tokyo_text},
};
use hypersig::html::{Class, El, a, div, each_keyed, h1, h2, li, main, p, section, span, ul};
use hypersig::{Action, Form, NumberFormat, Page, Patches, Timestamp};
use hypersig_ui::{
    AlertVariant, ButtonSize, ButtonVariant, Dialog, alert, alert_title, dialog_footer,
};
use std::ops::Not as _;

pub const REMOVE: Action<Settings, (String, i32)> = Action::post("/medications/remove");

/// Asks for confirmation before a medication is deleted.
const REMOVE_DIALOG: Dialog<Settings> = Dialog::new("remove-medication");

/// What the list page shows of all medications at one moment.
pub struct Overview {
    pub items: Vec<Summary>,
    pub today: Vec<TodayDose>,
    pub today_label: String,
    pub next: String,
    pub next_parts: NextParts,
}

/// The next notification as the hero card shows it; empty while there is none.
#[derive(Default)]
pub struct NextParts {
    pub time: String,
    pub day: String,
    pub name: String,
    pub detail: String,
}

/// The medications `member` stored, with the time each was last saved.
async fn stored(state: &AppState, member: &Member) -> Result<Vec<(Editor, i64)>, Error> {
    crate::repository::list(&state.db, member.owner())
        .await?
        .into_iter()
        .map(|row| Ok((row.editor()?, row.updated_at)))
        .collect()
}

/// The list, today's doses and the next notification at `now` (Unix
/// milliseconds), all from `schedule_status::assess`. Medications are by
/// name and today's doses by time, so the order never depends on storage.
///
/// # Errors
/// Fails for a stored medication that no longer parses, or a clock outside
/// the supported calendar.
pub fn overview(medications: &[(Editor, i64)], now: i64) -> Result<Overview, Error> {
    let today_date = UtcInstant(now).tokyo_date().map_err(|_| Error::Invalid)?;
    let slot_view = |editor: &Editor, slot_id: &str| {
        editor
            .slots
            .iter()
            .find(|slot| slot.id == slot_id)
            .map(|slot| SlotView::new(slot.id.clone(), slot.input.clone()))
    };
    let mut items = Vec::new();
    let mut today = Vec::new();
    let mut next: Option<(i64, String, NextParts)> = None;
    for (editor, saved_at) in medications {
        let medication = editor.domain(*saved_at)?;
        let assessment =
            schedule_status::assess(&medication, UtcInstant(now)).map_err(|_| Error::Invalid)?;
        for dose in &assessment.today {
            let view = slot_view(editor, &dose.slot_id);
            today.push((
                dose.timestamp.as_millis(),
                TodayDose {
                    key: format!("{}:{}", editor.id, dose.slot_id),
                    medication_id: editor.id.clone(),
                    name: medication.name.clone(),
                    summary: view
                        .as_ref()
                        .map_or_else(String::new, |v| v.summary.clone()),
                    time: tokyo_clock(dose.timestamp),
                    detail: view.as_ref().map_or_else(String::new, SlotView::detail),
                },
            ));
        }
        if let NextPlannedDose::At(dose) = &assessment.next
            && next
                .as_ref()
                .is_none_or(|(at, _, _)| dose.timestamp.as_millis() < *at)
        {
            let day = dose.local_date.parse::<LocalDate>().ok();
            next = Some((
                dose.timestamp.as_millis(),
                tokyo_text(dose.timestamp),
                NextParts {
                    time: tokyo_clock(dose.timestamp),
                    day: match day {
                        Some(date) if date != today_date => day_label(date, false),
                        _ => "今日".to_owned(),
                    },
                    name: medication.name.clone(),
                    detail: slot_view(editor, &dose.slot_id)
                        .as_ref()
                        .map_or_else(String::new, SlotView::detail),
                },
            ));
        }
        items.push(summary(editor, *saved_at, now)?);
    }
    items.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    today.sort_by(|a, b| (a.0, &a.1.name, &a.1.key).cmp(&(b.0, &b.1.name, &b.1.key)));
    let (next_text, next_parts) = match next {
        Some((_, text, parts)) => (
            format!("次回の通知: {} {text}（{TIMEZONE_LABEL}）", parts.name),
            parts,
        ),
        None if medications.is_empty() => {
            ("薬が登録されていません。".to_owned(), NextParts::default())
        }
        None => ("次回の通知はありません。".to_owned(), NextParts::default()),
    };
    Ok(Overview {
        items,
        today: today.into_iter().map(|(_, dose)| dose).collect(),
        today_label: format!("{} · {TIMEZONE_LABEL}", day_label(today_date, true)),
        next: next_text,
        next_parts,
    })
}

pub async fn page(state: AppState, member: Member, (): ()) -> Result<Page<Settings>, Error> {
    let now = Timestamp::now().as_millis();
    Ok(render(overview(&stored(&state, &member).await?, now)?))
}

/// The list page for `overview`.
#[must_use]
pub fn render(overview: Overview) -> Page<Settings> {
    let editor = crate::medication_edit::new_editor(String::new(), String::new());
    Page::new(
        "今日の予定",
        Settings {
            form: Form::new(editor),
            items: overview.items,
            today: overview.today,
            today_label: overview.today_label,
            next: overview.next,
            next_time: overview.next_parts.time,
            next_day: overview.next_parts.day,
            next_name: overview.next_parts.name,
            next_detail: overview.next_parts.detail,
            problem: String::new(),
            status: 0,
            busy: false,
            notice: String::new(),
            removing: String::new(),
            removing_revision: 0,
            removing_label: String::new(),
        },
    )
    .global(hypersig_ui::global())
    .theme(hypersig_ui::theme())
    .theme(crate::medication_style::theme())
    .style(STYLE)
    .child(
        main()
            .class(Class::new("med-shell"))
            .child(head(
                h1().text("今日の予定"),
                p().bind_text(Settings::today_label()),
            ))
            .child(crate::failure::alerts(
                Settings::problem(),
                Settings::status(),
            ))
            .child(hero())
            .child(today_section())
            .child(medications_section())
            .child(remove_dialog())
            .child(
                div()
                    .class(Class::new("med-actions"))
                    .child(crate::auth::sign_out_button()),
            )
            .child(tab_bar(Tab::Today)),
    )
}

/// The next notification: its time large, with the medication and the slot.
/// While there is none, the sentence that says why.
fn hero() -> El<Settings> {
    let upcoming = Settings::next_time().is_empty().not();
    section()
        .class(Class::new("med-hero"))
        .attr("aria-labelledby", "next-heading")
        .child(
            div()
                .class(Class::new("med-hero-top"))
                .child(span().id("next-heading").text("次のお知らせ"))
                .child(
                    span()
                        .class(Class::new("med-chip"))
                        .show(upcoming.clone())
                        .bind_text(Settings::next_day()),
                ),
        )
        .child(
            p().class(Class::new("med-hero-time"))
                .show(upcoming.clone())
                .bind_text(Settings::next_time()),
        )
        .child(
            p().class(Class::new("med-hero-name"))
                .show(upcoming.clone())
                .child(span().bind_text(Settings::next_name()))
                .child(span().text(" · "))
                .child(span().bind_text(Settings::next_detail())),
        )
        .child(
            p().class(Class::new("med-hero-note"))
                .show(upcoming.not())
                .bind_text(Settings::next()),
        )
}

fn today_section() -> El<Settings> {
    section()
        .class(Class::new("med-section"))
        .attr("aria-labelledby", "today-heading")
        .child(
            div()
                .class(Class::new("med-section-head"))
                .child(
                    h2().id("today-heading")
                        .class(Class::new("med-section-title"))
                        .text("今日のお知らせ"),
                )
                .child(
                    span()
                        .class(Class::new("med-sub"))
                        .child(
                            span().bind_text(
                                Settings::today().len().format(&NumberFormat::new()),
                            ),
                        )
                        .child(span().text("件")),
                ),
        )
        .child(
            p().class(Class::new("med-muted"))
                .show(Settings::items().is_empty().not().and(Settings::today().is_empty()))
                .text("今日の予定はありません。休薬中・通知オフ・終了・服薬枠なしの薬は、下の一覧に状態が表示されます。"),
        )
        .child(ul().class(Class::new("med-list")).child(each_keyed(
            Settings::today(),
            |dose| dose.at(TodayDose::key()),
            |dose| {
                li().class(Class::new("med-item"))
.class(Class::new("med-dose")).child(
                    span()
                        .class(Class::new("med-item-text"))
                        .child(
                            span()
                                .class(Class::new("med-dose-time"))
                                .bind_text(dose.at(TodayDose::time())),
                        )
                        .child(
                            a().class(Class::new("med-link"))
                                .href(crate::MEDICATION.link_with(dose.at(TodayDose::medication_id())))
                                .child(span().bind_text(dose.at(TodayDose::name())))
                                .child(span().text(" · "))
                                .child(span().bind_text(dose.at(TodayDose::detail()))),
                        ),
                )
                .child(span().class(Class::new("med-pill")).text("予定"))
            },
        )))
}

fn medications_section() -> El<Settings> {
    section()
        .class(Class::new("med-section"))
        .attr("aria-labelledby", "medications-heading")
        .child(
            div()
                .class(Class::new("med-section-head"))
                .child(
                    h2().id("medications-heading")
                        .class(Class::new("med-section-title"))
                        .text("登録済みのお薬"),
                )
                .child(
                    a().href(crate::NEW_MEDICATION.link())
                        .class(Class::new("med-link"))
                        .text("＋ お薬を追加"),
                ),
        )
        .child(
            p().class(Class::new("med-muted"))
                .show(Settings::items().is_empty())
                .text("まだ薬が登録されていません。「お薬を追加」から登録してください。"),
        )
        .child(
            alert(AlertVariant::Default)
                .show(Settings::notice().is_empty().not())
                .child(alert_title().bind_text(Settings::notice())),
        )
        .child(ul().class(Class::new("med-list")).child(each_keyed(
            Settings::items(),
            |item| item.at(Summary::id()),
            |item| {
                li().class(Class::new("med-item"))
                    .class(Class::new("med-dose"))
                    .child(
                        span()
                            .class(Class::new("med-item-text"))
                            .child(
                                a().class(Class::new("med-link"))
                                    .href(crate::MEDICATION.link_with(item.at(Summary::id())))
                                    .bind_text(item.at(Summary::name())),
                            )
                            .child(
                                span()
                                    .class(Class::new("med-pill"))
                                    .class(Class::new("med-status"))
                                    .bind_attr("data-state", item.at(Summary::state()))
                                    .bind_text(item.at(Summary::status())),
                            )
                            .child(
                                span()
                                    .class(Class::new("med-muted"))
                                    .bind_text(item.at(Summary::next())),
                            ),
                    )
                    .child(
                        REMOVE_DIALOG
                            .trigger(ButtonVariant::Destructive, ButtonSize::Sm)
                            .on_click(
                                Settings::removing()
                                    .set(item.at(Summary::id()))
                                    .then(
                                        Settings::removing_revision()
                                            .set(item.at(Summary::revision())),
                                    )
                                    .then(Settings::removing_label().set(item.at(Summary::name()))),
                            )
                            .text("削除"),
                    )
            },
        )))
}

fn remove_dialog() -> El<Settings> {
    let remove = REMOVE
        .call_with((Settings::removing(), Settings::removing_revision()))
        .indicator(Settings::busy())
        .error(Settings::problem())
        .status(Settings::status());
    REMOVE_DIALOG.alert_dialog(
        "薬を削除しますか",
        "服薬枠と休薬期間もまとめて削除され、通知されなくなります。元に戻すことはできません。",
        div()
            .child(p().bind_text(Settings::removing_label()))
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

/// The page after the medication `id` was deleted: its entry goes by key, and
/// today's doses and the next notification are computed again.
fn removed(id: &str, rest: Overview) -> Patches<Settings> {
    Patches::new()
        .signals(
            Settings::today()
                .patch(rest.today)
                .and(Settings::next().patch(rest.next))
                .and(Settings::next_time().patch(rest.next_parts.time))
                .and(Settings::next_day().patch(rest.next_parts.day))
                .and(Settings::next_name().patch(rest.next_parts.name))
                .and(Settings::next_detail().patch(rest.next_parts.detail))
                .and(Settings::removing().patch(String::new()))
                .and(Settings::notice().patch("薬を削除しました。".to_owned())),
        )
        .list(Settings::items().patch_remove(&[id.to_owned()], |item| item.at(Summary::id())))
}

pub async fn remove(
    state: AppState,
    member: Member,
    (id, revision): (String, i32),
) -> Result<Patches<Settings>, Error> {
    crate::repository::delete(
        &state.db,
        member.owner(),
        &Delete {
            id: id.clone(),
            revision,
        },
    )
    .await?;
    let now = Timestamp::now().as_millis();
    Ok(removed(
        &id,
        overview(&stored(&state, &member).await?, now)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::medication_input::{DoseSlotInput, PauseInput};
    use crate::medications::{Pause, Slot};

    fn at(date: &str, time: &str) -> i64 {
        date.parse::<crate::domain::LocalDate>()
            .unwrap()
            .at(time.parse().unwrap())
            .0
    }

    fn medication(id: &str, name: &str, times: &[&str]) -> (Editor, i64) {
        let mut editor = crate::medication_edit::new_editor(id.into(), "2026-10-01".into());
        editor.revision = 2;
        editor.medication.name = name.into();
        editor.slots = times
            .iter()
            .enumerate()
            .map(|(n, time)| Slot {
                id: format!("{id}-slot{n}"),
                input: DoseSlotInput {
                    time: (*time).into(),
                    amount: "1".into(),
                    unit: "錠".into(),
                    timing: "after_meal".into(),
                    note: String::new(),
                },
            })
            .collect();
        (editor, at("2026-10-01", "00:00"))
    }

    fn pause(editor: &mut Editor, start: &str, end: &str) {
        editor.pauses.push(Pause {
            id: "p".into(),
            input: PauseInput {
                start: start.into(),
                end: end.into(),
            },
        });
    }

    fn page_of(medications: &[(Editor, i64)], now: i64) -> Page<Settings> {
        render(overview(medications, now).ok().unwrap())
    }

    #[test]
    fn links_come_from_typed_routes_and_the_empty_state_is_part_of_the_page() {
        let html = page_of(&[], at("2026-10-10", "12:00")).render();
        assert!(html.contains(crate::NEW_MEDICATION.path()), "{html}");
        assert!(html.contains("まだ薬が登録されていません"), "{html}");
        assert!(html.contains("薬が登録されていません。"), "{html}");
        assert!(html.contains("data-hs-key"), "{html}");
        assert!(html.contains("data-hs-attr"), "{html}");
        assert!(html.contains("Asia/Tokyo"), "{html}");
        assert!(html.contains(REMOVE.path()), "{html}");
        assert!(html.contains(r#"role="alertdialog""#), "{html}");
        assert!(html.contains(crate::auth::LOGIN.path()), "{html}");
        assert!(html.contains(crate::NOTIFICATIONS.path()), "{html}");
        assert!(html.contains(r#"aria-current="page""#), "{html}");
        assert!(html.contains("次のお知らせ"), "{html}");
    }

    #[test]
    fn today_is_in_time_order_across_medications_with_stable_keys() {
        let now = at("2026-10-10", "05:00");
        let medications = [
            medication("b", "夕の薬", &["20:00"]),
            medication("a", "朝の薬", &["12:00", "08:00"]),
        ];
        let view = overview(&medications, now).ok().unwrap();
        let keys: Vec<_> = view.today.iter().map(|d| d.key.as_str()).collect();
        assert_eq!(keys, ["a:a-slot1", "a:a-slot0", "b:b-slot0"]);
        assert!(view.today[0].summary.starts_with("08:00"));
        let names: Vec<_> = view.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["夕の薬", "朝の薬"]);
        assert_eq!(view.today_label, "10月10日（土） · Asia/Tokyo");
        assert_eq!(view.today[0].time, "08:00");
        assert_eq!(view.today[0].detail, "1 錠 · 食後");
        assert_eq!(view.next_parts.time, "08:00");
        assert_eq!(view.next_parts.day, "今日");
        assert_eq!(view.next_parts.name, "朝の薬");
        assert_eq!(
            view.next,
            "次回の通知: 朝の薬 2026-10-10 08:00（Asia/Tokyo）"
        );
    }

    #[test]
    fn the_day_and_the_next_dose_are_tokyo_whatever_the_utc_date_is() {
        // 2026-10-10 23:30 in Tokyo is 14:30 UTC the same day; 00:30 in Tokyo
        // is still the day before in UTC.
        let medications = [medication("a", "薬", &["00:15", "08:00"])];
        let late = overview(&medications, at("2026-10-10", "23:30"))
            .ok()
            .unwrap();
        assert_eq!(late.today_label, "10月10日（土） · Asia/Tokyo");
        assert_eq!(late.next_parts.day, "10月11日");
        assert_eq!(late.next_parts.time, "00:15");
        assert_eq!(late.next, "次回の通知: 薬 2026-10-11 00:15（Asia/Tokyo）");
        let early = overview(&medications, at("2026-10-11", "00:30"))
            .ok()
            .unwrap();
        assert_eq!(early.today_label, "10月11日（日） · Asia/Tokyo");
        assert_eq!(early.today.len(), 2);
        assert_eq!(early.next, "次回の通知: 薬 2026-10-11 08:00（Asia/Tokyo）");
    }

    #[test]
    fn empty_disabled_paused_and_ended_are_told_apart() {
        let now = at("2026-10-10", "12:00");
        let empty = medication("e", "枠なし", &[]);
        let mut disabled = medication("d", "オフ", &["08:00"]);
        disabled.0.enabled = false;
        let mut paused = medication("p", "休薬", &["08:00"]);
        pause(&mut paused.0, "2026-10-09", "2026-10-11");
        let mut ended = medication("n", "終了", &["08:00"]);
        ended.0.medication.end = "2026-10-05".into();
        let active = medication("a", "通常", &["20:00"]);
        let view = overview(&[empty, disabled, paused, ended, active], now)
            .ok()
            .unwrap();
        let state = |name: &str| view.items.iter().find(|i| i.name == name).unwrap();
        assert_eq!(state("枠なし").state, "empty");
        assert_eq!(state("オフ").state, "disabled");
        assert_eq!(state("休薬").state, "paused");
        assert_eq!(state("休薬").status, "休薬中（2026-10-09 〜 2026-10-11）");
        assert_eq!(
            state("休薬").next,
            "次回の通知: 2026-10-12 08:00（Asia/Tokyo）"
        );
        assert_eq!(state("終了").state, "ended");
        assert_eq!(state("通常").state, "active");
        // Only the active medication has a dose today.
        assert_eq!(view.today.len(), 1);
        assert_eq!(view.today[0].name, "通常");
    }

    #[test]
    fn nothing_today_with_medications_says_so_apart_from_the_empty_list() {
        let mut paused = medication("p", "休薬", &["08:00"]);
        pause(&mut paused.0, "2026-10-09", "2026-10-11");
        let view = overview(&[paused], at("2026-10-10", "12:00")).ok().unwrap();
        assert!(view.today.is_empty());
        assert!(!view.items.is_empty());
        let html = render(view).render();
        assert!(html.contains("今日の予定はありません。"), "{html}");
    }

    #[test]
    fn the_next_notification_follows_changes_and_deleting_recomputes_it() {
        let now = at("2026-10-10", "09:00");
        let mut a = medication("a", "A", &["10:00"]);
        let b = medication("b", "B", &["11:00"]);
        let before = overview(&[a.clone(), b.clone()], now).ok().unwrap();
        assert_eq!(before.next, "次回の通知: A 2026-10-10 10:00（Asia/Tokyo）");
        // Pausing A moves the overall next notification to B.
        pause(&mut a.0, "2026-10-10", "2026-10-10");
        let paused = overview(&[a.clone(), b.clone()], now).ok().unwrap();
        assert_eq!(paused.next, "次回の通知: B 2026-10-10 11:00（Asia/Tokyo）");
        // Deleting B leaves A, whose next dose is after its pause.
        let rest = overview(&[a], now).ok().unwrap();
        let patches = removed("b", rest);
        let events = patches.to_event_stream();
        assert!(events.contains("remove_keys"), "{events}");
        assert_eq!(
            patches.value_of(Settings::next()).unwrap(),
            "次回の通知: A 2026-10-11 10:00（Asia/Tokyo）"
        );
        assert_eq!(patches.value_of(Settings::removing()).as_deref(), Some(""));
        // Deleting the last medication leaves the empty state.
        let none = overview(&[], now).ok().unwrap();
        let patches = removed("a", none);
        assert!(patches.value_of(Settings::today()).unwrap().is_empty());
        assert_eq!(
            patches.value_of(Settings::next()).as_deref(),
            Some("薬が登録されていません。")
        );
    }

    #[test]
    fn items_are_the_first_values_of_the_page() {
        let medications = [
            medication("a", "薬A", &["08:00"]),
            medication("b", "薬B", &[]),
        ];
        let page = page_of(&medications, at("2026-10-10", "05:00"));
        let items = page.value_of(Settings::items());
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].name, "薬B");
        assert_eq!(page.value_of(Settings::today()).len(), 1);
        assert_eq!(page.value_of(Settings::status()), 0);
    }
}
