//! Screen adapter: timestamps are Unix milliseconds; timezone is never Browser.
use crate::domain::{InvalidValue, LocalDate, Medication, NextDose, Occurrence, UtcInstant};
use hypersig::{DateFormat, DateStyle, IntlLocale, TimeStyle, TimeZone, Timestamp};
pub const TIMEZONE_LABEL: &str = "Asia/Tokyo";
pub const DATE_TIME: DateFormat =
    DateFormat::date_time(IntlLocale::Ja, DateStyle::Medium, TimeStyle::Short)
        .zone(TimeZone::Offset(540));
pub struct PlannedDose {
    pub slot_id: String,
    pub local_date: String,
    pub timestamp: Timestamp,
}
impl From<Occurrence> for PlannedDose {
    fn from(o: Occurrence) -> Self {
        Self {
            slot_id: o.slot_id.to_string(),
            local_date: o.date.to_string(),
            timestamp: Timestamp::from_millis(o.at.0),
        }
    }
}
pub struct ScheduleView {
    pub today: Vec<PlannedDose>,
    pub next: NextPlannedDose,
}
pub enum NextPlannedDose {
    Disabled,
    Ended,
    NoUpcoming,
    At(PlannedDose),
}
impl From<NextDose> for NextPlannedDose {
    fn from(next: NextDose) -> Self {
        match next {
            NextDose::Disabled => Self::Disabled,
            NextDose::Ended => Self::Ended,
            NextDose::NoUpcoming => Self::NoUpcoming,
            NextDose::At(occurrence) => Self::At(occurrence.into()),
        }
    }
}
/// # Errors
/// Rejects clock values outside the supported Gregorian calendar.
pub fn query(medication: &Medication, now: Timestamp) -> Result<ScheduleView, InvalidValue> {
    let now = UtcInstant(now.as_millis());
    Ok(ScheduleView {
        today: medication
            .on_date(now.tokyo_date()?)
            .into_iter()
            .map(Into::into)
            .collect(),
        next: medication.next(now)?.into(),
    })
}

/// `date` as `10月25日（日）`.
#[must_use]
pub fn day_label(date: LocalDate, with_weekday: bool) -> String {
    let text = date.to_string();
    let number = |range: std::ops::Range<usize>| text[range].trim_start_matches('0').to_owned();
    let weekday = ["月", "火", "水", "木", "金", "土", "日"][usize::from(date.weekday())];
    let day = format!("{}月{}日", number(5..7), number(8..10));
    if with_weekday {
        format!("{day}（{weekday}）")
    } else {
        day
    }
}

/// The time of day of `at` on the Asia/Tokyo clock as `HH:mm`, or `""` for a
/// clock outside the supported calendar.
#[must_use]
pub fn tokyo_clock(at: Timestamp) -> String {
    tokyo_text(at)
        .split_once(' ')
        .map_or_else(String::new, |(_, time)| time.to_owned())
}

/// `at` as `YYYY-MM-DD HH:mm` on the Asia/Tokyo calendar. The offset is added
/// by hand, so neither the server's nor the browser's time zone matters.
/// Empty for a clock outside the supported calendar.
#[must_use]
pub fn tokyo_text(at: Timestamp) -> String {
    let instant = UtcInstant(at.as_millis());
    let Ok(date) = instant.tokyo_date() else {
        return String::new();
    };
    let minutes = (instant.0 + 32_400_000).rem_euclid(86_400_000) / 60_000;
    format!("{date} {:02}:{:02}", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokyo_text_adds_nine_hours_whatever_the_host_zone_is() {
        // 2026-10-26 15:30 UTC is already the next day in Tokyo.
        let utc = |text: &str| {
            let (date, time) = text.split_once(' ').unwrap();
            // `at` reads its arguments as Tokyo time; nine hours on top
            // read them as UTC.
            let tokyo = date
                .parse::<crate::domain::LocalDate>()
                .unwrap()
                .at(time.parse().unwrap());
            Timestamp::from_millis(tokyo.0 + 32_400_000)
        };
        assert_eq!(tokyo_text(utc("2026-10-26 15:30")), "2026-10-27 00:30");
        assert_eq!(tokyo_text(utc("2026-10-26 14:59")), "2026-10-26 23:59");
        assert_eq!(tokyo_text(utc("2026-12-31 15:00")), "2027-01-01 00:00");
        assert_eq!(tokyo_text(Timestamp::from_millis(0)), "1970-01-01 09:00");
        assert_eq!(tokyo_text(Timestamp::from_millis(-1)), "1970-01-01 08:59");
    }

    #[test]
    fn tokyo_text_is_empty_outside_the_calendar() {
        assert_eq!(tokyo_text(Timestamp::from_millis(i64::MAX / 2)), "");
    }
}
