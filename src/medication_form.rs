//! The values of the medication edit form and their conversion to the stored
//! `MedicationInput`. No database access.
//!
//! A checkbox binds a `bool`, so the weekdays are seven fields here where
//! `MedicationInput` stores one bitmask (bit 0 is Monday, bit 6 is Sunday).
use crate::medication_input::{DoseSlotInput, MedicationInput};
use hypersig::{FieldErrors, Rules, Sig, Signals, Validate};
use serde::{Deserialize, Serialize};

/// The repeat rules the form offers, as the stored value and its label.
pub const REPEATS: [(&str, &str); 3] = [
    ("daily", "毎日"),
    ("weekdays", "曜日を指定"),
    ("interval", "N日ごと"),
];
/// The timings a dose slot offers, as the stored value and its label.
pub const TIMINGS: [(&str, &str); 6] = [
    ("unspecified", "指定なし"),
    ("before_meal", "食前"),
    ("after_meal", "食後"),
    ("between_meals", "食間"),
    ("bedtime", "就寝前"),
    ("other", "その他"),
];
/// The units a dose slot offers.
pub const UNITS: [&str; 7] = ["錠", "カプセル", "包", "mL", "mg", "滴", "個"];

/// The label of a stored timing value, or the value itself when unknown.
#[must_use]
pub fn timing_label(timing: &str) -> &str {
    TIMINGS
        .iter()
        .find(|(value, _)| *value == timing)
        .map_or(timing, |(_, label)| label)
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "each weekday is a checkbox, which binds one bool"
)]
#[derive(Clone, Debug, PartialEq, Eq, Signals, Serialize, Deserialize)]
pub struct MedicationForm {
    pub name: String,
    pub start: String,
    pub end: String,
    /// daily / weekdays / interval
    pub repeat: String,
    pub interval: i32,
    pub mon: bool,
    pub tue: bool,
    pub wed: bool,
    pub thu: bool,
    pub fri: bool,
    pub sat: bool,
    pub sun: bool,
    /// Whether the medication is scheduled and notified.
    pub enabled: bool,
}

impl Validate for MedicationForm {
    fn rules<S>(f: Sig<S, Self>, r: &mut Rules<S, Self>) {
        let any_weekday = f
            .at(Self::mon())
            .or(f.at(Self::tue()))
            .or(f.at(Self::wed()))
            .or(f.at(Self::thu()))
            .or(f.at(Self::fri()))
            .or(f.at(Self::sat()))
            .or(f.at(Self::sun()));
        r.check(
            Self::name(),
            !f.at(Self::name()).trim().is_empty(),
            "薬名は必須です。",
        )
        .check(
            Self::name(),
            f.at(Self::name()).char_count().le(100),
            "薬名は100文字以内です。",
        )
        .check(
            Self::start(),
            !f.at(Self::start()).is_empty(),
            "開始日は必須です。",
        )
        .check(
            Self::repeat(),
            f.at(Self::repeat())
                .equals("daily")
                .or(f.at(Self::repeat()).equals("weekdays"))
                .or(f.at(Self::repeat()).equals("interval")),
            "繰り返しを選択してください。",
        )
        .check(
            Self::interval(),
            f.at(Self::repeat())
                .not_equals("interval")
                .or(f.at(Self::interval()).ge(1)),
            "間隔は1以上です。",
        )
        // The weekday group shows its message under the last checkbox.
        .check(
            Self::sun(),
            f.at(Self::repeat()).not_equals("weekdays").or(any_weekday),
            "曜日を選択してください。",
        );
    }
}

impl MedicationForm {
    /// A new medication that starts on `start` and repeats daily.
    #[must_use]
    pub fn blank(start: String) -> Self {
        Self {
            name: String::new(),
            start,
            end: String::new(),
            repeat: "daily".into(),
            interval: 1,
            mon: false,
            tue: false,
            wed: false,
            thu: false,
            fri: false,
            sat: false,
            sun: false,
            enabled: true,
        }
    }

    /// The form that shows a stored `input` and its `enabled` switch.
    #[must_use]
    pub fn from_input(input: &MedicationInput, enabled: bool) -> Self {
        let bit = |n: u32| input.weekdays & (1 << n) != 0;
        Self {
            name: input.name.clone(),
            start: input.start.clone(),
            end: input.end.clone(),
            repeat: input.repeat.clone(),
            interval: input.interval,
            mon: bit(0),
            tue: bit(1),
            wed: bit(2),
            thu: bit(3),
            fri: bit(4),
            sat: bit(5),
            sun: bit(6),
            enabled,
        }
    }

    /// The stored value for these fields. The weekday bits are kept only
    /// while the repeat rule is `weekdays`.
    #[must_use]
    pub fn to_input(&self) -> MedicationInput {
        let days = [
            self.mon, self.tue, self.wed, self.thu, self.fri, self.sat, self.sun,
        ];
        let bits = days
            .iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .fold(0, |bits, (n, _)| bits | (1 << n));
        MedicationInput {
            name: self.name.clone(),
            start: self.start.clone(),
            end: self.end.clone(),
            repeat: self.repeat.clone(),
            interval: self.interval,
            weekdays: if self.repeat == "weekdays" { bits } else { 0 },
        }
    }

    /// The form's own rules, then the server's parse of the stored value.
    /// Each message stays on the field the visitor edited.
    ///
    /// # Errors
    /// Returns the messages of the fields that failed.
    pub fn parse(&self) -> Result<MedicationInput, FieldErrors<Self>> {
        hypersig::Form::validate(self)?;
        let input = self.to_input();
        input.parse().map_err(|e| Self::field_errors(&e))?;
        Ok(input)
    }

    /// The messages of `errors` on the form fields they belong to.
    #[must_use]
    pub fn field_errors(errors: &FieldErrors<MedicationInput>) -> FieldErrors<Self> {
        let mut out = FieldErrors::new();
        macro_rules! carry {
            ($($from:ident => $to:ident),+) => {$(
                if let Some(message) = errors.message(MedicationInput::$from()) {
                    out = out.add(Self::$to(), message);
                }
            )+};
        }
        carry!(name => name, start => start, end => end, repeat => repeat,
            interval => interval, weekdays => sun);
        out
    }
}

/// A dose slot of the list: the stored values and the line the list shows.
#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct SlotView {
    pub id: String,
    pub input: DoseSlotInput,
    pub summary: String,
}

impl SlotView {
    #[must_use]
    pub fn new(id: String, input: DoseSlotInput) -> Self {
        let summary = format!(
            "{} · {} {} · {}",
            input.time,
            input.amount,
            input.unit.trim(),
            timing_label(&input.timing)
        );
        Self { id, input, summary }
    }
}

impl SlotView {
    /// The amount and the timing: `1 錠 · 食後`.
    #[must_use]
    pub fn detail(&self) -> String {
        format!(
            "{} {} · {}",
            self.input.amount,
            self.input.unit.trim(),
            timing_label(&self.input.timing)
        )
    }
}

impl From<crate::medications::Slot> for SlotView {
    fn from(slot: crate::medications::Slot) -> Self {
        Self::new(slot.id, slot.input)
    }
}

/// The values a new dose slot starts from.
#[must_use]
pub fn blank_slot() -> DoseSlotInput {
    DoseSlotInput {
        time: "08:00".into(),
        amount: "1".into(),
        unit: UNITS[0].into(),
        timing: "unspecified".into(),
        note: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::LocalDate;

    fn form() -> MedicationForm {
        MedicationForm {
            name: " 薬 ".into(),
            ..MedicationForm::blank("2026-10-05".into())
        }
    }

    #[test]
    fn weekday_checkboxes_round_trip_through_the_bitmask() {
        // Bit 0 is the day the domain calls weekday 0.
        let monday = "2026-10-05".parse::<LocalDate>().unwrap();
        assert_eq!(monday.weekday(), 0);
        let mut f = form();
        f.repeat = "weekdays".into();
        f.mon = true;
        f.sun = true;
        let input = f.to_input();
        assert_eq!(input.weekdays, 1 | 64);
        assert_eq!(MedicationForm::from_input(&input, true), f);
        f.repeat = "daily".into();
        assert_eq!(f.to_input().weekdays, 0);
    }

    #[test]
    fn shared_rules_and_domain_errors_stay_on_form_fields() {
        let mut f = form();
        assert!(f.parse().is_ok());
        f.repeat = "weekdays".into();
        let errors = f.parse().err().unwrap();
        assert!(errors.message(MedicationForm::sun()).is_some());
        f.mon = true;
        assert!(f.parse().is_ok());
        f.repeat = "interval".into();
        f.interval = 0;
        assert!(
            f.parse()
                .err()
                .unwrap()
                .message(MedicationForm::interval())
                .is_some()
        );
        f.interval = 3;
        f.start = "2026-02-30".into();
        assert!(
            f.parse()
                .err()
                .unwrap()
                .message(MedicationForm::start())
                .is_some()
        );
        f.start = "2026-10-05".into();
        f.end = "2026-10-04".into();
        assert!(
            f.parse()
                .err()
                .unwrap()
                .message(MedicationForm::end())
                .is_some()
        );
        f.end.clear();
        f.name = " ".into();
        assert!(
            f.parse()
                .err()
                .unwrap()
                .message(MedicationForm::name())
                .is_some()
        );
    }

    #[test]
    fn slot_view_summarizes_time_amount_unit_and_timing() {
        let mut input = blank_slot();
        input.timing = "after_meal".into();
        assert_eq!(
            SlotView::new("s".into(), input).summary,
            "08:00 · 1 錠 · 食後"
        );
    }
}
