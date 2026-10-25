//! What a medication's screens say about today and the next notification.
//! The list and the edit page read the same `assess`, which builds on the
//! schedule computation the notifications use.
use crate::{
    domain::{InvalidValue, LocalDate, Medication, UtcInstant},
    schedule_view::{self, NextPlannedDose, PlannedDose, TIMEZONE_LABEL, tokyo_text},
};
use hypersig::Timestamp;

/// Why a medication has, or has not, a next notification. When several
/// apply, the first of disabled, ended, empty and paused wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No dose slot is registered.
    Empty,
    /// Notifications are switched off.
    Disabled,
    /// The end date has passed.
    Ended,
    /// Today is inside a pause period (休薬中).
    Paused,
    Active,
}

impl State {
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Disabled => "disabled",
            Self::Ended => "ended",
            Self::Paused => "paused",
            Self::Active => "active",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "服薬枠なし",
            Self::Disabled => "通知オフ",
            Self::Ended => "終了",
            Self::Paused => "休薬中",
            Self::Active => "通知中",
        }
    }
}

pub struct Assessment {
    pub state: State,
    /// The pause period that holds today, whatever the state.
    pub paused: Option<(LocalDate, LocalDate)>,
    pub today: Vec<PlannedDose>,
    pub next: NextPlannedDose,
}

/// # Errors
/// Rejects a clock outside the supported Gregorian calendar.
pub fn assess(medication: &Medication, now: UtcInstant) -> Result<Assessment, InvalidValue> {
    let view = schedule_view::query(medication, Timestamp::from_millis(now.0))?;
    let today = now.tokyo_date()?;
    let paused = medication
        .pauses
        .iter()
        .find(|period| period.contains(today))
        .map(|period| (period.start(), period.end()));
    let state = if !medication.enabled {
        State::Disabled
    } else if matches!(view.next, NextPlannedDose::Ended) {
        State::Ended
    } else if medication.slots.is_empty() {
        State::Empty
    } else if paused.is_some() {
        State::Paused
    } else {
        State::Active
    };
    Ok(Assessment {
        state,
        paused,
        today: view.today,
        next: view.next,
    })
}

/// The sentence about the next notification, with the zone named.
#[must_use]
pub fn next_text(assessment: &Assessment) -> String {
    match (&assessment.state, &assessment.next) {
        (State::Disabled, _) => "通知が無効のため、次回の予定はありません。".to_owned(),
        (State::Ended, _) => "服薬期間が終了しています。".to_owned(),
        (State::Empty, _) => "服薬枠がないため、次回の予定はありません。".to_owned(),
        (_, NextPlannedDose::At(dose)) => {
            format!(
                "次回の通知: {}（{TIMEZONE_LABEL}）",
                tokyo_text(dose.timestamp)
            )
        }
        _ => "次回の予定はありません。".to_owned(),
    }
}

/// The list page's entry for `editor`.
#[must_use]
pub fn summary(
    editor: &crate::medications::Editor,
    name: &str,
    assessment: &Assessment,
) -> crate::medications::Summary {
    let status = match (assessment.state, assessment.paused) {
        (State::Paused, Some((start, end))) => format!("休薬中（{start} 〜 {end}）"),
        (state, _) => state.label().to_owned(),
    };
    crate::medications::Summary {
        id: editor.id.clone(),
        name: name.to_owned(),
        revision: editor.revision,
        state: assessment.state.key().to_owned(),
        status,
        next: next_text(assessment),
    }
}

/// The sentence that says today is paused, or `""`.
#[must_use]
pub fn paused_text(assessment: &Assessment) -> String {
    assessment.paused.map_or_else(String::new, |(start, end)| {
        format!("今日は休薬中です（{start} 〜 {end}）。")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DoseSlot, Schedule};

    fn at(date: &str, time: &str) -> UtcInstant {
        date.parse::<LocalDate>().unwrap().at(time.parse().unwrap())
    }

    fn medication() -> Medication {
        Medication {
            id: "m".parse().unwrap(),
            name: "薬".into(),
            enabled: true,
            schedule: Schedule::new(
                "2026-10-01".parse().unwrap(),
                Some("2026-10-31".parse().unwrap()),
                crate::domain::Recurrence::Daily,
            )
            .unwrap(),
            slots: vec![slot("b", "20:00"), slot("a", "08:00")],
            pauses: vec![],
            saved_at: at("2026-10-01", "00:00"),
        }
    }

    fn slot(id: &str, time: &str) -> DoseSlot {
        DoseSlot {
            id: id.parse().unwrap(),
            time: time.parse().unwrap(),
            amount: "1".parse().unwrap(),
            unit: "錠".parse().unwrap(),
            timing: crate::domain::Timing::Unspecified,
            note: String::new(),
        }
    }

    #[test]
    fn states_are_told_apart_and_ordered() {
        let now = at("2026-10-10", "12:00");
        let mut med = medication();
        assert_eq!(assess(&med, now).unwrap().state, State::Active);

        med.pauses = vec![
            crate::domain::PausePeriod::new(
                "2026-10-10".parse().unwrap(),
                "2026-10-12".parse().unwrap(),
            )
            .unwrap(),
        ];
        let paused = assess(&med, now).unwrap();
        assert_eq!(paused.state, State::Paused);
        assert_eq!(
            paused_text(&paused),
            "今日は休薬中です（2026-10-10 〜 2026-10-12）。"
        );
        assert!(paused.today.is_empty());
        assert_eq!(
            next_text(&paused),
            "次回の通知: 2026-10-13 08:00（Asia/Tokyo）"
        );

        let slots = std::mem::take(&mut med.slots);
        assert_eq!(assess(&med, now).unwrap().state, State::Empty);
        med.slots = slots;

        assert_eq!(
            assess(&med, at("2026-11-01", "09:00")).unwrap().state,
            State::Ended
        );
        med.enabled = false;
        assert_eq!(assess(&med, now).unwrap().state, State::Disabled);
        // Disabled outranks ended.
        assert_eq!(
            assess(&med, at("2026-11-01", "09:00")).unwrap().state,
            State::Disabled
        );
    }

    #[test]
    fn today_comes_in_time_order_in_tokyo_around_the_utc_boundary() {
        let med = medication();
        // 2026-10-10 00:30 in Tokyo is still 2026-10-09 in UTC.
        let early = assess(&med, at("2026-10-10", "00:30")).unwrap();
        let times: Vec<_> = early
            .today
            .iter()
            .map(|dose| tokyo_text(dose.timestamp))
            .collect();
        assert_eq!(times, ["2026-10-10 08:00", "2026-10-10 20:00"]);
        assert_eq!(
            next_text(&early),
            "次回の通知: 2026-10-10 08:00（Asia/Tokyo）"
        );
        // 23:30 in Tokyo is already 14:30 UTC: tomorrow's first dose is next.
        let late = assess(&med, at("2026-10-10", "23:30")).unwrap();
        assert_eq!(
            next_text(&late),
            "次回の通知: 2026-10-11 08:00（Asia/Tokyo）"
        );
    }

    #[test]
    fn a_pause_does_not_hide_why_notifications_are_off() {
        let mut med = medication();
        med.enabled = false;
        med.pauses = vec![
            crate::domain::PausePeriod::new(
                "2026-10-10".parse().unwrap(),
                "2026-10-10".parse().unwrap(),
            )
            .unwrap(),
        ];
        let off = assess(&med, at("2026-10-10", "12:00")).unwrap();
        assert_eq!(off.state, State::Disabled);
        assert!(off.paused.is_some());
        assert!(next_text(&off).contains("無効"));
    }
}
