//! Shared expression rules plus authoritative server parsing. No database access.
use crate::domain::{
    DoseAmount, DoseSlot, DoseSlotId, InvalidValue, LocalDate, PausePeriod, Recurrence, Schedule,
    Timing, Weekdays,
};
use hypersig::{FieldErrors, Form, Rules, Sig, Signals, Validate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct MedicationInput {
    pub name: String,
    pub start: String,
    pub end: String,
    /// daily / weekdays / interval
    pub repeat: String,
    pub interval: i32,
    pub weekdays: i32,
}
impl Validate for MedicationInput {
    fn rules<S>(f: Sig<S, Self>, r: &mut Rules<S, Self>) {
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
        .check(
            Self::weekdays(),
            f.at(Self::repeat()).not_equals("weekdays").or(f
                .at(Self::weekdays())
                .ge(1)
                .and(f.at(Self::weekdays()).le(127))),
            "曜日を選択してください。",
        );
    }
}
impl MedicationInput {
    /// Invoke after shared validation; all failures retain their field identity.
    /// # Errors
    /// Returns field errors for failed shared rules or invalid domain values.
    pub fn parse(&self) -> Result<(String, Schedule), FieldErrors<Self>> {
        Form::validate(self)?;
        let start = self
            .start
            .parse::<LocalDate>()
            .map_err(|_| FieldErrors::new().add(Self::start(), "日付が不正です。"))?;
        let end = if self.end.is_empty() {
            None
        } else {
            Some(
                self.end
                    .parse::<LocalDate>()
                    .map_err(|_| FieldErrors::new().add(Self::end(), "日付が不正です。"))?,
            )
        };
        let recurrence = match self.repeat.as_str() {
            "daily" => Recurrence::Daily,
            "weekdays" => {
                Recurrence::Weekdays(
                    Weekdays::new(u8::try_from(self.weekdays).map_err(|_| {
                        FieldErrors::new().add(Self::weekdays(), "曜日が不正です。")
                    })?)
                    .map_err(|_| FieldErrors::new().add(Self::weekdays(), "曜日が不正です。"))?,
                )
            }
            _ => Recurrence::Every(
                u32::try_from(self.interval)
                    .ok()
                    .and_then(std::num::NonZeroU32::new)
                    .ok_or_else(|| FieldErrors::new().add(Self::interval(), "間隔が不正です。"))?,
            ),
        };
        let schedule = Schedule::new(start, end, recurrence)
            .map_err(|_| FieldErrors::new().add(Self::end(), "終了日は開始日以降です。"))?;
        Ok((self.name.trim().into(), schedule))
    }
}

#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct DoseSlotInput {
    pub time: String,
    pub amount: String,
    pub unit: String,
    pub timing: String,
    pub note: String,
}
impl Validate for DoseSlotInput {
    fn rules<S>(f: Sig<S, Self>, r: &mut Rules<S, Self>) {
        r.check(
            Self::time(),
            !f.at(Self::time()).is_empty(),
            "時刻は必須です。",
        )
        .check(
            Self::amount(),
            !f.at(Self::amount()).is_empty(),
            "量は必須です。",
        )
        .check(
            Self::unit(),
            !f.at(Self::unit()).trim().is_empty(),
            "単位は必須です。",
        )
        .check(
            Self::unit(),
            f.at(Self::unit()).char_count().le(20),
            "単位は20文字以内です。",
        )
        .check(
            Self::note(),
            f.at(Self::note()).char_count().le(200),
            "メモは200文字以内です。",
        );
    }
}
impl DoseSlotInput {
    /// # Errors
    /// Returns field errors for failed shared rules or invalid domain values.
    pub fn parse(&self, id: DoseSlotId) -> Result<DoseSlot, FieldErrors<Self>> {
        Form::validate(self)?;
        let time = self
            .time
            .parse()
            .map_err(|_| FieldErrors::new().add(Self::time(), "時刻が不正です。"))?;
        let amount = self.amount.parse::<DoseAmount>().map_err(|_| {
            FieldErrors::new().add(Self::amount(), "量は0.01〜1000、小数2桁以内です。")
        })?;
        let timing = match self.timing.as_str() {
            "unspecified" => Timing::Unspecified,
            "before_meal" => Timing::BeforeMeal,
            "after_meal" => Timing::AfterMeal,
            "between_meals" => Timing::BetweenMeals,
            "bedtime" => Timing::Bedtime,
            "other" => Timing::Other,
            _ => return Err(FieldErrors::new().add(Self::timing(), "タイミングが不正です。")),
        };
        Ok(DoseSlot {
            id,
            time,
            amount,
            unit: self
                .unit
                .parse()
                .map_err(|_| FieldErrors::new().add(Self::unit(), "単位が不正です。"))?,
            timing,
            note: self.note.clone(),
        })
    }
}

#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct PauseInput {
    pub start: String,
    pub end: String,
}
impl Validate for PauseInput {
    fn rules<S>(f: Sig<S, Self>, r: &mut Rules<S, Self>) {
        r.check(
            Self::start(),
            !f.at(Self::start()).is_empty(),
            "開始日は必須です。",
        )
        .check(
            Self::end(),
            !f.at(Self::end()).is_empty(),
            "終了日は必須です。",
        );
    }
}
impl PauseInput {
    /// Repository supplies other pauses for this medication, excluding the edited row.
    /// CRUD must read/check/write under its own concurrency control.
    /// # Errors
    /// Returns field errors for failed shared rules or invalid domain values.
    pub fn parse(&self, others: &[PausePeriod]) -> Result<PausePeriod, FieldErrors<Self>> {
        Form::validate(self)?;
        let start = self
            .start
            .parse()
            .map_err(|_: InvalidValue| FieldErrors::new().add(Self::start(), "日付が不正です。"))?;
        let end = self
            .end
            .parse()
            .map_err(|_: InvalidValue| FieldErrors::new().add(Self::end(), "日付が不正です。"))?;
        let period = PausePeriod::new(start, end)
            .map_err(|_| FieldErrors::new().add(Self::end(), "終了日は開始日以降です。"))?;
        if others.iter().any(|other| period.overlaps(*other)) {
            return Err(FieldErrors::new().add(Self::start(), "休薬期間が重複しています。"));
        }
        Ok(period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_rules_and_server_calendar_errors_are_separate() {
        let mut input = MedicationInput {
            name: "薬".into(),
            start: "2026-02-30".into(),
            end: String::new(),
            repeat: "daily".into(),
            interval: 1,
            weekdays: 0,
        };
        assert!(Form::validate(&input).is_ok());
        assert!(
            input
                .parse()
                .err()
                .unwrap()
                .message(MedicationInput::start())
                .is_some()
        );
        input.start = "2026-03-01".into();
        input.end = "2026-02-28".into();
        assert!(
            input
                .parse()
                .err()
                .unwrap()
                .message(MedicationInput::end())
                .is_some()
        );
        input.repeat = "interval".into();
        input.interval = 0;
        assert!(Form::validate(&input).is_err());
        input.repeat = "weekdays".into();
        assert!(Form::validate(&input).is_err());
        input.weekdays = 1;
        input.end.clear();
        assert!(input.parse().is_ok());
        input.name = " ".into();
        assert!(Form::validate(&input).is_err());
        input.name = "薬".repeat(101);
        assert!(Form::validate(&input).is_err());
    }
    #[test]
    fn amount_and_pause_errors_keep_field_identity() {
        let mut input = DoseSlotInput {
            time: "08:00".into(),
            amount: "0".into(),
            unit: "錠".into(),
            timing: "after_meal".into(),
            note: String::new(),
        };
        assert!(Form::validate(&input).is_ok());
        assert!(
            input
                .parse("slot".parse().unwrap())
                .err()
                .unwrap()
                .message(DoseSlotInput::amount())
                .is_some()
        );
        input.amount = "0.5".into();
        assert!(input.parse("slot".parse().unwrap()).is_ok());
        let pause = PauseInput {
            start: "2026-10-03".into(),
            end: "2026-10-05".into(),
        };
        let existing = pause.parse(&[]).unwrap();
        let touching = PauseInput {
            start: "2026-10-05".into(),
            end: "2026-10-06".into(),
        };
        assert!(
            touching
                .parse(&[existing])
                .err()
                .unwrap()
                .message(PauseInput::start())
                .is_some()
        );
        let adjacent = PauseInput {
            start: "2026-10-06".into(),
            end: "2026-10-06".into(),
        };
        assert!(adjacent.parse(&[existing]).is_ok());
    }

    #[test]
    fn dose_field_errors_and_unicode_length_limits() {
        let mut input = DoseSlotInput {
            time: "23:59".into(),
            amount: "1000".into(),
            unit: "単".repeat(20),
            timing: "other".into(),
            note: "💊".repeat(200),
        };
        assert!(input.parse("slot".parse().unwrap()).is_ok());
        input.unit.push('位');
        assert!(
            Form::validate(&input)
                .unwrap_err()
                .message(DoseSlotInput::unit())
                .is_some()
        );
        input.unit = "錠".into();
        input.note.push('💊');
        assert!(
            Form::validate(&input)
                .unwrap_err()
                .message(DoseSlotInput::note())
                .is_some()
        );
        input.note.clear();
        input.time = "24:00".into();
        assert!(
            input
                .parse("slot".parse().unwrap())
                .unwrap_err()
                .message(DoseSlotInput::time())
                .is_some()
        );
        input.time = "08:00".into();
        input.timing = "unknown".into();
        assert!(
            input
                .parse("slot".parse().unwrap())
                .unwrap_err()
                .message(DoseSlotInput::timing())
                .is_some()
        );
    }

    #[test]
    fn pause_invalid_dates_reversed_ranges_and_empty_fields() {
        for (start, end, field) in [
            ("", "2026-10-01", PauseInput::start()),
            ("2026-02-29", "2026-10-01", PauseInput::start()),
            ("2026-10-01", "", PauseInput::end()),
            ("2026-10-01", "2026-13-01", PauseInput::end()),
            ("2026-10-02", "2026-10-01", PauseInput::end()),
        ] {
            let input = PauseInput {
                start: start.into(),
                end: end.into(),
            };
            assert!(input.parse(&[]).unwrap_err().message(field).is_some());
        }
    }
}
