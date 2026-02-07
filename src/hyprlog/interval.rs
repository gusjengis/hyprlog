use chrono::{DateTime, Days, Local, NaiveTime, TimeDelta, TimeZone, Utc};

use crate::model::Log;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interval {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub changed: bool,
}

impl Default for Interval {
    fn default() -> Self {
        let today = Local::now().date_naive();
        let tomorrow = today + TimeDelta::days(1);

        Interval {
            start: local_midnight_to_utc(today),
            end: local_midnight_to_utc(tomorrow),
            changed: false,
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
            changed: false,
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

    pub fn contains_interval(&self, interval: &Interval) -> bool {
        self.start <= interval.start && self.end >= interval.end
    }

    pub fn pan_days(&mut self, days: u64, forward: bool) {
        let original_start = self.start;
        let original_end = self.end;

        let start_date = self.start.with_timezone(&Local).date_naive();
        let end_date = self.end.with_timezone(&Local).date_naive();
        let span_days = (end_date - start_date).num_days().max(0) as u64;

        let today = Local::now().date_naive();
        let tonight_midnight_date = today + TimeDelta::days(1);

        let desired_start_date = if forward {
            start_date + Days::new(days)
        } else {
            start_date - Days::new(days)
        };
        let desired_end_date = desired_start_date + Days::new(span_days);

        let (next_start_date, next_end_date) =
            if forward && desired_end_date >= tonight_midnight_date {
                let capped_end_date = tonight_midnight_date;
                let capped_start_date = capped_end_date - Days::new(span_days);
                (capped_start_date, capped_end_date)
            } else {
                (desired_start_date, desired_end_date)
            };

        self.start = local_midnight_to_utc(next_start_date);
        self.end = local_midnight_to_utc(next_end_date);
        self.changed = self.start != original_start || self.end != original_end;
    }

    pub fn expand_to_include(&mut self, focused_interval: &Interval) {
        self.start = focused_interval.start.min(self.start);
        self.end = focused_interval.end.min(self.end);
    }

    pub fn overlap(&self, other_interval: &Interval) -> Option<Interval> {
        if self.start < other_interval.end && self.end > other_interval.start {
            Some(Interval {
                start: self.start.max(other_interval.start),
                end: self.end.min(other_interval.end),
                changed: false,
            })
        } else {
            None
        }
    }

    pub fn contains_log(&self, log: &Log) -> bool {
        let end;
        if let Some(end_) = log.end {
            end = end_;
        } else {
            end = chrono::Utc::now().timestamp_millis() as u64;
        }
        log.start >= self.start.timestamp_millis() as u64
            && end <= self.end.timestamp_millis() as u64
    }
}

fn local_midnight_to_utc(date: chrono::NaiveDate) -> DateTime<Utc> {
    let local_dt = Local
        .from_local_datetime(&date.and_time(NaiveTime::MIN))
        .single()
        .expect("invalid local datetime (DST issue)");

    local_dt.with_timezone(&Utc)
}
