//! Screen adapter: timestamps are Unix milliseconds; timezone is never Browser.
use crate::domain::{InvalidValue, Medication, NextDose, Occurrence, UtcInstant};
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
