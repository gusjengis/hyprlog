use chrono::{DateTime, Days, Local, NaiveTime, TimeDelta, TimeZone, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interval {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Default for Interval {
    fn default() -> Self {
        let today = Local::now().date_naive();
        let tomorrow = today + TimeDelta::days(1);

        Interval {
            start: local_midnight_to_utc(today),
            end: local_midnight_to_utc(tomorrow),
        }
    }
}

impl Interval {
    /// Interval covering the last `days` full local days (inclusive)
    pub fn from_day_count(days: u64) -> Self {
        let today = Local::now().date_naive();
        let start_day = today - TimeDelta::days(days as i64 - 1);
        let end_day = today + TimeDelta::days(1);

        Interval {
            start: local_midnight_to_utc(start_day),
            end: local_midnight_to_utc(end_day),
        }
    }

    pub fn width(&self) -> u64 {
        (self.end.timestamp_millis() - self.start.timestamp_millis()) as u64
    }

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn set_days(&mut self, value: u64) {
        *self = Self::from_day_count(value);
    }

    pub fn date_str(&self) -> String {
        let start = self.start.with_timezone(&Local).date_naive();
        let end = (self.end.with_timezone(&Local) - TimeDelta::seconds(1)).date_naive();

        if start == end {
            start.format("%Y-%m-%d").to_string()
        } else {
            format!("{} - {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d"))
        }
    }

    pub fn contains_utc_timestamp_millis(&self, timestamp_ms: u64) -> bool {
        let secs = (timestamp_ms / 1_000) as i64;
        let nanos = ((timestamp_ms % 1_000) * 1_000_000) as u32;

        let ts = match DateTime::<Utc>::from_timestamp(secs, nanos) {
            Some(dt) => dt,
            None => return false,
        };

        ts >= self.start && ts < self.end
    }

    pub fn pan_days(&mut self, days: u64, forward: bool) {
        let offset = Days::new(days);
        // let upper_bound =
        let today = Local::now().date_naive();
        let tomorrow = today + TimeDelta::days(1);
        local_midnight_to_utc(tomorrow);
        if forward {
            self.start = self.start + offset;
            self.end = self.end + offset;
        } else {
            self.start = self.start - offset;
            self.end = self.end - offset;
        }
    }
}

fn local_midnight_to_utc(date: chrono::NaiveDate) -> DateTime<Utc> {
    let local_dt = Local
        .from_local_datetime(&date.and_time(NaiveTime::MIN))
        .single()
        .expect("invalid local datetime (DST issue)");

    local_dt.with_timezone(&Utc)
}
