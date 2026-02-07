use chrono::{Datelike, Local, Months, NaiveTime, TimeDelta, TimeZone, Timelike, Utc};

use crate::Settings;

#[derive(Clone, Copy)]
enum TickScale {
    Hours(u32),
    Days(i64),
    Weeks(i64),
    Months(u32),
    Years(i32),
}

pub fn tick_row(settings: &Settings, width: usize, labels: bool) -> String {
    if width == 0 {
        return String::new();
    }

    let scale_ms = settings.scale();
    if scale_ms == 0 {
        return " ".repeat(width);
    }

    let tick_scale = select_tick_scale(scale_ms);
    let mut buffer = vec![' '; width];
    let start = settings.focused_interval.start;
    let end = settings.focused_interval.end;
    let mut tick = align_tick(start, tick_scale);

    while tick.with_timezone(&Utc) < end {
        let tick_ms = tick.with_timezone(&Utc).timestamp_millis() as i128;
        let start_ms = start.timestamp_millis() as i128;
        let width_i = width as i128;
        let scale_ms_i = scale_ms as i128;
        let pos = ((tick_ms - start_ms) * width_i / scale_ms_i) as i64;
        if pos >= 0 && (pos as usize) < width {
            let idx = pos as usize;
            buffer[idx] = '│';
            let label = tick_label(tick, tick_scale);
            if labels {
                write_label(&mut buffer, idx.saturating_add(1), &label);
            }
            write_label(&mut buffer, idx.saturating_add(1), &label);
        }
        tick = advance_tick(tick, tick_scale);
    }

    buffer.into_iter().collect()
}

pub fn overlay_line_with_offset(base: &str, overlay: &str, offset: usize) -> String {
    let mut buffer: Vec<char> = base.chars().collect();
    for (idx, ch) in overlay.chars().enumerate() {
        let target = offset + idx;
        if target >= buffer.len() {
            break;
        }
        buffer[target] = ch;
    }
    buffer.into_iter().collect()
}

fn select_tick_scale(scale_ms: u64) -> TickScale {
    const HOUR_MS: u64 = 3_600_000;
    const DAY_MS: u64 = 86_400_000;

    if scale_ms <= DAY_MS * 2 {
        TickScale::Hours(6)
    } else if scale_ms <= DAY_MS * 14 {
        TickScale::Days(1)
    } else if scale_ms <= DAY_MS * 90 {
        TickScale::Weeks(1)
    } else if scale_ms <= DAY_MS * 730 {
        TickScale::Months(1)
    } else {
        TickScale::Years(1)
    }
}

fn align_tick(start: chrono::DateTime<Utc>, scale: TickScale) -> chrono::DateTime<Local> {
    let start_local = start.with_timezone(&Local);
    match scale {
        TickScale::Hours(step) => {
            let step = step as i64;
            let date = start_local.date_naive();
            let hour = start_local.hour() as i64;
            let aligned_hour = hour - (hour % step);
            let mut candidate =
                local_datetime(date, aligned_hour as u32, 0, 0).unwrap_or(start_local);
            if candidate < start_local {
                candidate = candidate + TimeDelta::hours(step);
            }
            candidate
        }
        TickScale::Days(step) => {
            let mut date = start_local.date_naive();
            let mut candidate = local_midnight(date);
            if candidate < start_local {
                date += TimeDelta::days(step);
                candidate = local_midnight(date);
            }
            candidate
        }
        TickScale::Weeks(step) => {
            let mut date = start_local.date_naive();
            let days_from_monday = start_local.weekday().num_days_from_monday() as i64;
            date -= TimeDelta::days(days_from_monday);
            let mut candidate = local_midnight(date);
            if candidate < start_local {
                date += TimeDelta::days(step * 7);
                candidate = local_midnight(date);
            }
            candidate
        }
        TickScale::Months(step) => {
            let mut date = start_local
                .date_naive()
                .with_day(1)
                .expect("invalid month start");
            let mut candidate = local_midnight(date);
            if candidate < start_local {
                date = date
                    .checked_add_months(Months::new(step))
                    .expect("invalid month shift");
                candidate = local_midnight(date);
            }
            candidate
        }
        TickScale::Years(step) => {
            let mut year = start_local.year();
            let mut date = chrono::NaiveDate::from_ymd_opt(year, 1, 1).expect("invalid year start");
            let mut candidate = local_midnight(date);
            if candidate < start_local {
                year += step;
                date = chrono::NaiveDate::from_ymd_opt(year, 1, 1).expect("invalid year start");
                candidate = local_midnight(date);
            }
            candidate
        }
    }
}

fn advance_tick(current: chrono::DateTime<Local>, scale: TickScale) -> chrono::DateTime<Local> {
    match scale {
        TickScale::Hours(step) => current + TimeDelta::hours(step as i64),
        TickScale::Days(step) => current + TimeDelta::days(step),
        TickScale::Weeks(step) => current + TimeDelta::days(step * 7),
        TickScale::Months(step) => {
            let date = current
                .date_naive()
                .checked_add_months(Months::new(step))
                .expect("invalid month shift");
            local_midnight(date)
        }
        TickScale::Years(step) => {
            let date = chrono::NaiveDate::from_ymd_opt(current.year() + step, 1, 1)
                .expect("invalid year shift");
            local_midnight(date)
        }
    }
}

fn tick_label(tick: chrono::DateTime<Local>, scale: TickScale) -> String {
    match scale {
        TickScale::Hours(_) => {
            if tick.hour() == 0 {
                format!("{}{}", tick.day(), ordinal_suffix(tick.day()))
            } else {
                format_hour_label(tick.hour())
            }
        }
        TickScale::Days(_) => format!("{}{}", tick.day(), ordinal_suffix(tick.day())),
        TickScale::Weeks(_) => format!("{}{}", tick.day(), ordinal_suffix(tick.day())),
        TickScale::Months(_) => format!("{}", month_label(tick.month())),
        TickScale::Years(_) => format!("{:02}", tick.year() % 100),
    }
}

fn ordinal_suffix(day: u32) -> &'static str {
    let mod_100 = day % 100;
    if (11..=13).contains(&mod_100) {
        return "th";
    }
    match day % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

fn format_hour_label(hour: u32) -> String {
    let period = if hour < 12 { "a" } else { "p" };
    let hour12 = ((hour + 11) % 12) + 1;
    format!("{hour12}{period}")
}

fn month_label(month: u32) -> &'static str {
    match month {
        1 => "Jan.",
        2 => "Feb.",
        3 => "Mar.",
        4 => "Apr.",
        5 => "May.",
        6 => "Jun.",
        7 => "Jul.",
        8 => "Aug.",
        9 => "Sep.",
        10 => "Oct.",
        11 => "Nov.",
        12 => "Dec.",
        _ => "",
    }
}

fn write_label(buffer: &mut [char], start: usize, label: &str) {
    let mut idx = start;
    for ch in label.chars() {
        if idx >= buffer.len() {
            break;
        }
        buffer[idx] = ch;
        idx += 1;
    }
}

fn local_midnight(date: chrono::NaiveDate) -> chrono::DateTime<Local> {
    Local
        .from_local_datetime(&date.and_time(NaiveTime::MIN))
        .single()
        .expect("invalid local datetime (DST issue)")
}

fn local_datetime(
    date: chrono::NaiveDate,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<chrono::DateTime<Local>> {
    let time = NaiveTime::from_hms_opt(hour, minute, second)?;
    Local.from_local_datetime(&date.and_time(time)).single()
}
