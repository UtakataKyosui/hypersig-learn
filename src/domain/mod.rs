//! Validated medication values and pure scheduling in Asia/Tokyo (UTC+09:00).
use std::{fmt, num::NonZeroU32, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidValue;
impl fmt::Display for InvalidValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("入力値が不正です。")
    }
}
impl std::error::Error for InvalidValue {}

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);
        impl FromStr for $name {
            type Err = InvalidValue;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if s.is_empty()
                    || s.len() > 128
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    return Err(InvalidValue);
                }
                Ok(Self(s.into()))
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
string_id!(MedicationId);
string_id!(DoseSlotId);

/// Gregorian ordinal, restricted to 0001-01-01 through 9999-12-31.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalDate(i32);
fn leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
fn month_days(y: i32, m: usize) -> i32 {
    if m == 2 && leap(y) {
        29
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1]
    }
}
fn year_start(y: i32) -> i32 {
    let n = y - 1;
    365 * n + n / 4 - n / 100 + n / 400
}
impl FromStr for LocalDate {
    type Err = InvalidValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 10
            || s.as_bytes()[4] != b'-'
            || s.as_bytes()[7] != b'-'
            || !s
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
        {
            return Err(InvalidValue);
        }
        let y: i32 = s[..4].parse().map_err(|_| InvalidValue)?;
        let m: usize = s[5..7].parse().map_err(|_| InvalidValue)?;
        let d: i32 = s[8..].parse().map_err(|_| InvalidValue)?;
        if !(1..=9999).contains(&y)
            || !(1..=12).contains(&m)
            || !(1..=month_days(y, m)).contains(&d)
        {
            return Err(InvalidValue);
        }
        Ok(Self(
            year_start(y) + (1..m).map(|m| month_days(y, m)).sum::<i32>() + d - 1,
        ))
    }
}
impl fmt::Display for LocalDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut y = 1;
        while y < 9999 && year_start(y + 1) <= self.0 {
            y += 1;
        }
        let mut d = self.0 - year_start(y);
        let mut m = 1;
        while d >= month_days(y, m) {
            d -= month_days(y, m);
            m += 1;
        }
        write!(f, "{y:04}-{m:02}-{:02}", d + 1)
    }
}
impl LocalDate {
    fn next(self) -> Option<Self> {
        (self.0 < year_start(10000) - 1).then_some(Self(self.0 + 1))
    }
    /// Monday = 0, Sunday = 6.
    /// # Panics
    /// Only if the private nonnegative ordinal invariant is broken.
    #[must_use]
    pub fn weekday(self) -> u8 {
        u8::try_from(self.0 % 7).expect("validated ordinal is nonnegative")
    }
    #[must_use]
    pub fn at(self, time: LocalTime) -> UtcInstant {
        UtcInstant(
            (i64::from(self.0) - 719_162) * 86_400_000 + i64::from(time.0) * 60_000 - 32_400_000,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalTime(u16);
impl FromStr for LocalTime {
    type Err = InvalidValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 5
            || s.as_bytes()[2] != b':'
            || !s
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 2 || b.is_ascii_digit())
        {
            return Err(InvalidValue);
        }
        let h: u16 = s[..2].parse().map_err(|_| InvalidValue)?;
        let m: u16 = s[3..].parse().map_err(|_| InvalidValue)?;
        if h > 23 || m > 59 {
            return Err(InvalidValue);
        }
        Ok(Self(h * 60 + m))
    }
}
impl fmt::Display for LocalTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.0 / 60, self.0 % 60)
    }
}

/// Hundredths, never a floating point value. Canonical display has two decimals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoseAmount(u32);
impl DoseAmount {
    #[must_use]
    pub fn hundredths(self) -> u32 {
        self.0
    }
}
impl FromStr for DoseAmount {
    type Err = InvalidValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (whole, fraction) = s.split_once('.').unwrap_or((s, ""));
        if whole.is_empty()
            || whole.len() > 4
            || !whole.bytes().all(|b| b.is_ascii_digit())
            || fraction.len() > 2
            || !fraction.bytes().all(|b| b.is_ascii_digit())
            || (s.contains('.') && fraction.is_empty())
        {
            return Err(InvalidValue);
        }
        let whole: u32 = whole.parse().map_err(|_| InvalidValue)?;
        let fraction = if fraction.is_empty() {
            0
        } else {
            fraction.parse::<u32>().map_err(|_| InvalidValue)?
                * if fraction.len() == 1 { 10 } else { 1 }
        };
        let amount = whole * 100 + fraction;
        if !(1..=100_000).contains(&amount) {
            return Err(InvalidValue);
        }
        Ok(Self(amount))
    }
}
impl fmt::Display for DoseAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recurrence {
    Daily,
    Weekdays(Weekdays),
    Every(NonZeroU32),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weekdays(u8);
impl Weekdays {
    /// # Errors
    /// Rejects empty weekday sets or reversed date ranges.
    pub fn new(bits: u8) -> Result<Self, InvalidValue> {
        if bits == 0 || bits > 127 {
            Err(InvalidValue)
        } else {
            Ok(Self(bits))
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PausePeriod {
    start: LocalDate,
    end: LocalDate,
}
impl PausePeriod {
    /// # Errors
    /// Rejects empty weekday sets or reversed date ranges.
    pub fn new(start: LocalDate, end: LocalDate) -> Result<Self, InvalidValue> {
        if start > end {
            Err(InvalidValue)
        } else {
            Ok(Self { start, end })
        }
    }
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.start <= other.end && other.start <= self.end
    }
    #[must_use]
    pub fn contains(self, date: LocalDate) -> bool {
        self.start <= date && date <= self.end
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoseSlot {
    pub id: DoseSlotId,
    pub time: LocalTime,
    pub amount: DoseAmount,
    pub unit: DoseUnit,
    pub timing: Timing,
    pub note: String,
}
/// A required, bounded free-text unit such as 錠, 包, mL or mg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoseUnit(String);
impl FromStr for DoseUnit {
    type Err = InvalidValue;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.trim().is_empty() || value.chars().count() > 20 {
            return Err(InvalidValue);
        }
        Ok(Self(value.trim().into()))
    }
}
impl fmt::Display for DoseUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    Unspecified,
    BeforeMeal,
    AfterMeal,
    BetweenMeals,
    Bedtime,
    Other,
}
#[derive(Debug, Clone)]
pub struct Medication {
    pub id: MedicationId,
    pub name: String,
    pub enabled: bool,
    pub schedule: Schedule,
    pub slots: Vec<DoseSlot>,
    pub pauses: Vec<PausePeriod>,
    pub saved_at: UtcInstant,
}
#[derive(Debug, Clone)]
pub struct Schedule {
    start: LocalDate,
    end: Option<LocalDate>,
    recurrence: Recurrence,
}
impl Schedule {
    /// # Errors
    /// Rejects empty weekday sets or reversed date ranges.
    pub fn new(
        start: LocalDate,
        end: Option<LocalDate>,
        recurrence: Recurrence,
    ) -> Result<Self, InvalidValue> {
        if end.is_some_and(|end| end < start) {
            Err(InvalidValue)
        } else {
            Ok(Self {
                start,
                end,
                recurrence,
            })
        }
    }
    fn includes(&self, date: LocalDate) -> bool {
        date >= self.start
            && self.end.is_none_or(|end| date <= end)
            && match self.recurrence {
                Recurrence::Daily => true,
                Recurrence::Weekdays(days) => days.0 & (1 << date.weekday()) != 0,
                Recurrence::Every(n) => {
                    u32::try_from(date.0 - self.start.0).is_ok_and(|days| days % n.get() == 0)
                }
            }
    }

    /// First matching date at or after the lower bound, without scanning elapsed days.
    fn next_date(&self, lower: LocalDate) -> Option<LocalDate> {
        let lower = lower.max(self.start);
        let ordinal = match self.recurrence {
            Recurrence::Daily => i64::from(lower.0),
            Recurrence::Every(interval) => {
                let elapsed = i64::from(lower.0 - self.start.0);
                let interval = i64::from(interval.get());
                i64::from(self.start.0) + ((elapsed + interval - 1) / interval) * interval
            }
            Recurrence::Weekdays(days) => {
                let offset = (0..7).find(|offset| {
                    days.0 & (1 << ((i32::from(lower.weekday()) + offset) % 7)) != 0
                })?;
                i64::from(lower.0) + i64::from(offset)
            }
        };
        let limit = self.end.map_or(year_start(10000) - 1, |end| end.0);
        (ordinal <= i64::from(limit))
            .then(|| LocalDate(i32::try_from(ordinal).expect("bounded calendar ordinal")))
    }
}
/// Unix milliseconds at the persistence boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtcInstant(pub i64);
impl UtcInstant {
    /// # Errors
    /// Returns an error when the clock is outside the supported calendar (years 1–9999).
    pub fn tokyo_date(self) -> Result<LocalDate, InvalidValue> {
        let ordinal = self.0.div_euclid(86_400_000)
            + (self.0.rem_euclid(86_400_000) + 32_400_000) / 86_400_000
            + 719_162;
        if !(0..i64::from(year_start(10000))).contains(&ordinal) {
            return Err(InvalidValue);
        }
        Ok(LocalDate(i32::try_from(ordinal).map_err(|_| InvalidValue)?))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub slot_id: DoseSlotId,
    pub date: LocalDate,
    pub at: UtcInstant,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextDose {
    Disabled,
    Ended,
    NoUpcoming,
    At(Occurrence),
}
impl Medication {
    /// Same predicate for the screen and notification candidate generation.
    #[must_use]
    pub fn on_date(&self, date: LocalDate) -> Vec<Occurrence> {
        if !self.enabled
            || !self.schedule.includes(date)
            || self.pauses.iter().any(|p| p.contains(date))
        {
            return vec![];
        }
        let mut result: Vec<_> = self
            .slots
            .iter()
            .map(|s| Occurrence {
                slot_id: s.id.clone(),
                date,
                at: date.at(s.time),
            })
            .collect();
        result.sort_by_key(|o| o.at);
        result
    }
    /// # Errors
    /// Returns an error when the clock is outside the supported calendar (years 1–9999).
    pub fn next(&self, now: UtcInstant) -> Result<NextDose, InvalidValue> {
        if !self.enabled {
            return Ok(NextDose::Disabled);
        }
        let mut date = now
            .tokyo_date()?
            .max(self.saved_at.tokyo_date()?)
            .max(self.schedule.start);
        if self.schedule.end.is_some_and(|end| date > end) {
            return Ok(NextDose::Ended);
        }
        if self.slots.is_empty() {
            return Ok(NextDose::NoUpcoming);
        }
        while let Some(candidate) = self.schedule.next_date(date) {
            date = candidate;
            if let Some(end) = self
                .pauses
                .iter()
                .filter(|pause| pause.contains(date))
                .map(|pause| pause.end)
                .max()
            {
                let Some(after_pause) = end.next() else { break };
                date = after_pause;
                continue;
            }
            if let Some(item) = self
                .on_date(date)
                .into_iter()
                .find(|o| o.at >= now && o.at >= self.saved_at)
            {
                return Ok(NextDose::At(item));
            }
            let Some(next) = date.next() else { break };
            date = next;
        }
        Ok(NextDose::NoUpcoming)
    }
    /// Due within the inclusive 15 minute delivery window; never before first save.
    /// # Errors
    /// Returns an error when the clock is outside the supported calendar (years 1–9999).
    pub fn due(&self, now: UtcInstant) -> Result<Vec<Occurrence>, InvalidValue> {
        let last = now.tokyo_date()?;
        if self.pauses.iter().any(|pause| pause.contains(last)) {
            return Ok(vec![]);
        }
        let earliest = UtcInstant(now.0.saturating_sub(900_000));
        let first = earliest.tokyo_date()?;
        let mut candidates = self.on_date(first);
        if first != last {
            candidates.extend(self.on_date(last));
        }
        candidates.retain(|o| o.at >= earliest && o.at <= now && o.at >= self.saved_at);
        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn date(s: &str) -> LocalDate {
        s.parse().unwrap()
    }
    fn at(d: &str, t: &str) -> UtcInstant {
        date(d).at(t.parse().unwrap())
    }
    fn medication(recurrence: Recurrence, start: &str, end: Option<&str>) -> Medication {
        Medication {
            id: "9007199254740993".parse().unwrap(),
            name: "薬".into(),
            enabled: true,
            schedule: Schedule::new(date(start), end.map(date), recurrence).unwrap(),
            slots: vec![DoseSlot {
                id: "slot".parse().unwrap(),
                time: "08:00".parse().unwrap(),
                amount: "0.5".parse().unwrap(),
                unit: "錠".parse().unwrap(),
                timing: Timing::AfterMeal,
                note: String::new(),
            }],
            pauses: vec![],
            saved_at: at(start, "00:00"),
        }
    }
    #[test]
    fn strict_values_round_trip() {
        for s in ["0001-01-01", "2000-02-29", "2024-02-29", "9999-12-31"] {
            assert_eq!(date(s).to_string(), s);
        }
        for s in [
            "",
            "0000-01-01",
            "1900-02-29",
            "2025-02-29",
            "2024-04-31",
            "2024-1-01",
            "２０２４-01-01",
        ] {
            assert!(s.parse::<LocalDate>().is_err(), "{s}");
        }
        for s in ["00:00", "23:59"] {
            assert_eq!(s.parse::<LocalTime>().unwrap().to_string(), s);
        }
        for s in ["24:00", "12:60", "1:00", "00:00:00", ""] {
            assert!(s.parse::<LocalTime>().is_err());
        }
        for s in ["0.01", "0.50", "1000.00"] {
            let a = s.parse::<DoseAmount>().unwrap();
            assert_eq!(a.to_string(), s);
            assert_eq!(a.to_string().parse(), Ok(a));
        }
        for s in [
            "", "0", "-1", "+1", "1e2", "1.001", "1000.01", "1.", ".5", " 1", "NaN",
        ] {
            assert!(s.parse::<DoseAmount>().is_err(), "{s}");
        }
        assert_eq!(
            "9007199254740993"
                .parse::<MedicationId>()
                .unwrap()
                .to_string(),
            "9007199254740993"
        );
    }
    #[test]
    fn interval_anchor_survives_inclusive_pause() {
        let mut m = medication(
            Recurrence::Every(NonZeroU32::new(2).unwrap()),
            "2026-10-01",
            None,
        );
        m.pauses
            .push(PausePeriod::new(date("2026-10-03"), date("2026-10-05")).unwrap());
        for d in ["2026-10-03", "2026-10-04", "2026-10-05", "2026-10-06"] {
            assert_eq!(m.on_date(date(d)).len(), 0);
        }
        assert!(
            matches!(m.next(at("2026-10-01","08:01")).unwrap(),NextDose::At(o) if o.at==at("2026-10-07","08:00"))
        );
        assert_eq!(m.on_date(date("2026-10-07")).len(), 1);
    }
    #[test]
    fn calendar_and_weekday_boundaries() {
        for (start, next) in [
            ("2024-02-28", "2024-02-29"),
            ("2025-02-28", "2025-03-01"),
            ("2026-12-31", "2027-01-01"),
            ("2026-04-30", "2026-05-01"),
        ] {
            let m = medication(Recurrence::Daily, start, Some(next));
            assert_eq!(m.on_date(date(start)).len(), 1);
            assert!(
                matches!(m.next(at(start,"09:00")).unwrap(),NextDose::At(o) if o.date==date(next))
            );
            assert_eq!(m.next(at(next, "09:00")).unwrap(), NextDose::NoUpcoming);
        }
        let m = medication(
            Recurrence::Weekdays(Weekdays::new(1 | 64).unwrap()),
            "2026-10-25",
            Some("2026-10-27"),
        );
        assert_eq!(date("2026-10-25").weekday(), 6);
        assert_eq!(m.on_date(date("2026-10-25")).len(), 1);
        assert_eq!(m.on_date(date("2026-10-26")).len(), 1);
        assert_eq!(m.on_date(date("2026-10-27")).len(), 0);
        assert_eq!(m.next(at("2026-10-28", "00:00")).unwrap(), NextDose::Ended);
    }
    #[test]
    fn due_window_save_cutoff_and_multiple_slots() {
        let mut m = medication(Recurrence::Daily, "2026-10-01", None);
        let mut second = m.slots[0].clone();
        second.id = "second".parse().unwrap();
        second.time = "20:00".parse().unwrap();
        m.slots.push(second);
        assert_eq!(m.on_date(date("2026-10-01")).len(), 2);
        assert_eq!(m.due(at("2026-10-01", "08:15")).unwrap().len(), 1);
        assert_eq!(
            m.due(UtcInstant(at("2026-10-01", "08:15").0 + 1))
                .unwrap()
                .len(),
            0
        );
        m.saved_at = at("2026-10-01", "08:01");
        assert_eq!(m.due(at("2026-10-01", "08:02")).unwrap().len(), 0);
        assert!(
            matches!(m.next(m.saved_at).unwrap(),NextDose::At(o) if o.at==at("2026-10-01","20:00"))
        );
        m.enabled = false;
        assert_eq!(m.next(m.saved_at).unwrap(), NextDose::Disabled);
        assert_eq!(m.due(m.saved_at).unwrap().len(), 0);
    }
    #[test]
    fn utc_and_jst_midnight_are_the_same_instant() {
        assert_eq!(at("1970-01-01", "09:00").0, 0);
        assert_eq!(at("1970-01-01", "00:00").0, -32_400_000);
        assert_eq!(UtcInstant(-1).tokyo_date().unwrap(), date("1970-01-01"));
        let mut m = medication(Recurrence::Daily, "2026-10-01", None);
        m.slots[0].time = "00:00".parse().unwrap();
        let now = at("2026-10-01", "00:01");
        assert_eq!(m.due(now).unwrap().len(), 1);
        let view =
            crate::schedule_view::query(&m, hypersig::Timestamp::from_millis(now.0)).unwrap();
        assert_eq!(
            view.today[0].timestamp.as_millis(),
            at("2026-10-01", "00:00").0
        );
        assert_eq!(view.today[0].local_date, "2026-10-01");
    }

    #[test]
    fn pause_suppresses_every_slot_through_the_last_minute() {
        let mut m = medication(Recurrence::Daily, "2026-10-01", None);
        m.slots[0].time = "00:00".parse().unwrap();
        let mut late = m.slots[0].clone();
        late.id = "late".parse().unwrap();
        late.time = "23:59".parse().unwrap();
        m.slots.push(late);
        m.pauses
            .push(PausePeriod::new(date("2026-10-03"), date("2026-10-05")).unwrap());
        assert_eq!(m.due(at("2026-10-02", "23:59")).unwrap().len(), 1);
        for day in ["2026-10-03", "2026-10-05"] {
            assert_eq!(m.on_date(date(day)).len(), 0);
            assert_eq!(m.due(at(day, "00:15")).unwrap().len(), 0);
            assert_eq!(m.due(at(day, "23:59")).unwrap().len(), 0);
        }
        assert_eq!(
            m.due(at("2026-10-06", "00:00")).unwrap()[0]
                .slot_id
                .to_string(),
            "slot"
        );
        assert!(
            matches!(m.next(at("2026-10-03", "00:00")).unwrap(), NextDose::At(o) if o.at == at("2026-10-06", "00:00"))
        );
    }

    #[test]
    fn weekdays_resume_on_the_next_selected_day_and_respect_end() {
        let mut m = medication(
            Recurrence::Weekdays(Weekdays::new(1).unwrap()),
            "2026-10-01",
            Some("2026-10-12"),
        );
        m.pauses
            .push(PausePeriod::new(date("2026-10-05"), date("2026-10-06")).unwrap());
        assert!(
            matches!(m.next(at("2026-10-04", "00:00")).unwrap(), NextDose::At(o) if o.at == at("2026-10-12", "08:00"))
        );
        assert_eq!(m.on_date(date("2026-09-28")).len(), 0);
        assert_eq!(m.due(at("2026-10-12", "08:00")).unwrap().len(), 1);
        assert_eq!(m.on_date(date("2026-10-19")).len(), 0);
    }

    #[test]
    fn save_and_delivery_boundaries_are_inclusive_without_future_candidates() {
        let mut m = medication(Recurrence::Daily, "2026-10-01", None);
        m.saved_at = at("2026-10-01", "08:00");
        assert_eq!(m.due(UtcInstant(m.saved_at.0 - 1)).unwrap().len(), 0);
        assert_eq!(m.due(m.saved_at).unwrap().len(), 1);
        assert!(matches!(m.next(m.saved_at).unwrap(), NextDose::At(o) if o.at == m.saved_at));
        m.slots.clear();
        assert_eq!(m.next(m.saved_at).unwrap(), NextDose::NoUpcoming);
        for clock in [UtcInstant(i64::MIN), UtcInstant(i64::MAX)] {
            assert!(clock.tokyo_date().is_err());
            assert!(m.due(clock).is_err());
        }
    }

    #[test]
    fn date_round_trips_cover_a_complete_gregorian_cycle() {
        let mut day = date("2000-01-01");
        for _ in 0..146_097 {
            assert_eq!(day.to_string().parse(), Ok(day));
            assert_eq!(day.at("00:00".parse().unwrap()).tokyo_date(), Ok(day));
            day = day.next().unwrap();
        }
        assert_eq!(day.to_string(), "2400-01-01");
    }

    #[test]
    fn view_preserves_large_ids_and_next_day_utc_milliseconds() {
        let mut m = medication(Recurrence::Daily, "2026-10-01", None);
        m.slots[0].id = "9007199254740993".parse().unwrap();
        m.slots[0].time = "00:00".parse().unwrap();
        let view = crate::schedule_view::query(
            &m,
            hypersig::Timestamp::from_millis(at("2026-10-01", "23:59").0),
        )
        .unwrap();
        assert_eq!(view.today[0].slot_id, "9007199254740993");
        let crate::schedule_view::NextPlannedDose::At(next) = view.next else {
            panic!("expected next dose")
        };
        assert_eq!(next.slot_id, "9007199254740993");
        assert_eq!(next.local_date, "2026-10-02");
        // 2026-10-01 15:00:00 UTC, independently fixed Unix milliseconds.
        assert_eq!(next.timestamp.as_millis(), 1_790_866_800_000);
    }

    #[test]
    fn pause_start_blocks_previous_days_delayed_notification() {
        let mut m = medication(Recurrence::Daily, "2026-10-01", None);
        m.slots[0].time = "23:59".parse().unwrap();
        m.pauses
            .push(PausePeriod::new(date("2026-10-03"), date("2026-10-05")).unwrap());
        assert_eq!(m.due(at("2026-10-02", "23:59")).unwrap().len(), 1);
        assert_eq!(m.due(at("2026-10-03", "00:00")).unwrap().len(), 0);
        assert_eq!(m.due(at("2026-10-03", "00:14")).unwrap().len(), 0);
        assert_eq!(m.due(at("2026-10-06", "00:00")).unwrap().len(), 0);
        assert_eq!(m.due(at("2026-10-06", "23:59")).unwrap().len(), 1);
        m.pauses.clear();
        assert_eq!(m.due(at("2026-10-03", "00:00")).unwrap().len(), 1);
    }

    #[test]
    fn next_skips_long_intervals_and_pauses_without_moving_the_anchor() {
        let m = medication(
            Recurrence::Every(NonZeroU32::new(u32::MAX).unwrap()),
            "2026-10-01",
            None,
        );
        assert_eq!(
            m.next(at("2026-10-01", "08:01")).unwrap(),
            NextDose::NoUpcoming
        );
        let mut m = medication(
            Recurrence::Every(NonZeroU32::new(2).unwrap()),
            "2026-10-01",
            None,
        );
        // Deliberately unsorted adjacent pauses must both be skipped.
        m.pauses = vec![
            PausePeriod::new(date("9999-12-28"), date("9999-12-29")).unwrap(),
            PausePeriod::new(date("2026-10-02"), date("9999-12-27")).unwrap(),
        ];
        assert!(
            matches!(m.next(at("2026-10-01", "08:01")).unwrap(), NextDose::At(o) if o.date == date("9999-12-30"))
        );
        m.pauses
            .push(PausePeriod::new(date("9999-12-30"), date("9999-12-31")).unwrap());
        assert_eq!(
            m.next(at("2026-10-01", "08:01")).unwrap(),
            NextDose::NoUpcoming
        );
    }

    #[test]
    fn optimized_next_matches_bounded_day_by_day_reference() {
        let recurrences = [
            Recurrence::Daily,
            Recurrence::Weekdays(Weekdays::new(1 | 8 | 64).unwrap()),
            Recurrence::Every(NonZeroU32::new(3).unwrap()),
        ];
        for recurrence in recurrences {
            let mut m = medication(recurrence, "2024-02-20", Some("2024-03-10"));
            m.pauses = vec![PausePeriod::new(date("2024-02-27"), date("2024-03-02")).unwrap()];
            let mut day = date("2024-02-19");
            while day <= date("2024-03-11") {
                for time in ["00:00", "08:00", "08:01"] {
                    let now = day.at(time.parse().unwrap());
                    let mut scan = day.max(m.schedule.start);
                    let mut expected = NextDose::NoUpcoming;
                    if scan > m.schedule.end.unwrap() {
                        expected = NextDose::Ended;
                    } else {
                        while scan <= m.schedule.end.unwrap() {
                            if let Some(o) = m
                                .on_date(scan)
                                .into_iter()
                                .find(|o| o.at >= now && o.at >= m.saved_at)
                            {
                                expected = NextDose::At(o);
                                break;
                            }
                            scan = scan.next().unwrap();
                        }
                    }
                    assert_eq!(m.next(now).unwrap(), expected);
                }
                day = day.next().unwrap();
            }
        }
    }
}
