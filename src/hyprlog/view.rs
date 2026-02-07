use crate::model::Model;
use crate::timeline_sections::{timeline, TimelineCharacter};
use crate::Settings;
use chrono::{Datelike, Local, Months, NaiveTime, TimeDelta, TimeZone, Timelike, Utc};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use std::collections::HashMap;
use std::time::Duration;
use terminal_size::Width;

pub fn render_log(model: &Model, settings: &Settings) -> Result<Text<'static>, String> {
    let labels = get_labels(model, settings);

    if labels.is_empty() {
        return Err("Empty log".to_string());
    }

    let colors = key_to_color_map(&labels);
    let timelines = render_timelines(model, &colors, labels, settings);

    Ok(timelines)
}

pub fn header(settings: &Settings) -> String {
    let mut res = String::from("");
    let date_str = settings.focused_interval.date_str();
    let term_width = terminal_width();

    let inner_width = date_str.len() + 2;
    let box_width = inner_width + 2;

    if box_width > term_width {
        res.push_str(&date_str);
        return res;
    }

    let tick_line = tick_row(settings, term_width, false);

    let start_column = ((term_width - box_width) as f32 / 2.0).round() as usize;
    let start_column_int_div = (term_width - box_width) / 2;
    let rounded_up = start_column == start_column_int_div;
    let pad = " ".repeat(start_column);
    let top_border_left = "─".repeat(start_column - 1);
    let top_border_right = "─".repeat(start_column - (1 + if rounded_up { 0 } else { 1 }));

    let top_line = format!("{}╭{}╮", pad, "─".repeat(inner_width));
    let middle_line = format!("│ {} │", date_str);
    let bottom_line = format!(
        "┌{}┴{}┴{}┐",
        top_border_left,
        "─".repeat(inner_width),
        top_border_right
    );

    res.push_str(&format!("{}\n", top_line));
    res.push_str(&format!(
        "{}\n",
        overlay_line_with_offset(&tick_line, &middle_line, start_column)
    ));
    res.push_str(&format!("{}\n", bottom_line));
    res.push_str(&format!("\n"));

    res
}

pub fn render_timelines(
    model: &Model,
    colors: &HashMap<String, Color>,
    labels: Vec<String>,
    settings: &Settings,
) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(Line::from(""));
    if !settings.multi_timeline {
        lines.push(build_timeline(model, colors, settings, None));
        lines.push(Line::from("\n"));
    } else {
        let mut count = 0;
        for label in labels {
            if label.len() == 0 {
                continue;
            }
            lines.push(build_timeline(model, colors, settings, Some(&label)));
            lines.push(Line::from("\n"));
            count += 1;
        }
    }
    return Text::from(lines);
}

fn build_timeline(
    model: &Model,
    colors: &HashMap<String, Color>,
    settings: &Settings,
    label: Option<&String>,
) -> Line<'static> {
    let width = terminal_width();
    let sections = timeline(model, width, settings, label);

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(sections.len());

    for section_data in sections.iter() {
        let key = if settings.multi_timeline {
            label.expect("label required when multi_timeline")
        } else {
            &section_data.label
        };

        if let Some(color) = colors.get(key) {
            // Old behavior was effectively:
            //   STRIKE_ON + (colored glyph) + STRIKE_OFF
            //
            // In Ratatui, "strike + color" must be part of the SAME style on the Span.
            // To match your old look, when FANCY_TIMELINE is enabled we apply CROSSED_OUT
            // broadly (so the decoration stays colored with the fg).
            let mut base_style = Style::default().fg(*color);
            base_style = base_style.add_modifier(Modifier::CROSSED_OUT);

            let (s, style) = if *color == Color::Black {
                (
                    " ".to_string(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD | Modifier::CROSSED_OUT),
                )
            } else {
                (
                    choose_character(section_data, settings).to_string(),
                    base_style,
                )
            };

            spans.push(Span::styled(s, style)); // <-- owns the String
        }
    }

    Line::from(spans)
}

fn choose_character(section_data: &TimelineCharacter, settings: &Settings) -> char {
    let width = terminal_width();
    let ms_per_section = (settings.focused_interval.width() / (width as u64)) as f64;
    let fullness = section_data.total as f64 / ms_per_section as f64;
    if section_data.activity_at_left_edge && section_data.activity_at_right_edge {
        // there is activity near both the left and right side of a section
        return '█';
    } else if section_data.activity_at_left_edge {
        // there is activity near the left side of a section
        return match fullness {
            f64::MIN..=0.00 => ' ',
            0.0..=0.1250000 => '▏',
            0.125..=0.25000 => '▎',
            0.25..=0.375000 => '▍',
            0.375..=0.50000 => '▌',
            0.5..=0.6250000 => '▋',
            0.625..=0.75000 => '▊',
            0.75..=f64::MAX => '█',
            _ => '—',
        };
    } else if section_data.activity_at_right_edge {
        // there is activity near the right side of a section
        return match fullness {
            f64::MIN..=0.00 => ' ',
            0.0..=0.1250000 => '🮇',
            0.125..=0.25000 => '🮈',
            0.25..=0.375000 => '▐',
            0.375..=0.50000 => '🮉',
            0.5..=0.6250000 => '🮊',
            0.625..=0.75000 => '🮋',
            0.75..=f64::MAX => '█',
            _ => ' ',
        };
    } else {
        return match fullness {
            f64::MIN..=0.00 => ' ',
            0.00..=0.333333 => '│',
            0.333333..=0.66 => '┃',
            0.66..=f64::MAX => '█',
            _ => ' ',
        };
    }
}

#[derive(Clone, Copy)]
enum TickScale {
    Hours(u32),
    Days(i64),
    Weeks(i64),
    Months(u32),
    Years(i32),
}

fn tick_row(settings: &Settings, width: usize, labels: bool) -> String {
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

fn overlay_line_with_offset(base: &str, overlay: &str, offset: usize) -> String {
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

pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let cut_point = s.floor_char_boundary(max_len - 3);
        format!("{}...", &s[..cut_point])
    } else {
        ".".repeat(max_len)
    }
}

pub fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        return format!("{ms}ms");
    }

    const SEC_PER_MIN: u64 = 60;
    const MIN_PER_HOUR: u64 = 60;
    const HOUR_PER_DAY: u64 = 24;
    const DAY_PER_YEAR: u64 = 365;

    let total_secs = ms / 1_000;

    let seconds = total_secs % SEC_PER_MIN;
    let total_mins = total_secs / SEC_PER_MIN;

    let minutes = total_mins % MIN_PER_HOUR;
    let total_hours = total_mins / MIN_PER_HOUR;

    let hours = total_hours % HOUR_PER_DAY;
    let total_days = total_hours / HOUR_PER_DAY;

    let days = total_days % DAY_PER_YEAR;
    let years = total_days / DAY_PER_YEAR;

    if years > 0 {
        format!(
            "{years}y {days}d {:02}:{:02}:{:02}",
            hours, minutes, seconds
        )
    } else if days > 0 {
        format!("{days}d {:02}:{:02}:{:02}", hours, minutes, seconds)
    } else if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

pub fn format_short_duration(duration: Duration) -> String {
    if duration.as_secs_f64() < 0.001 {
        format!("{}µs", duration.as_micros())
    } else if duration.as_secs_f64() < 1.0 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{}s", duration.as_secs_f64())
    }
}

pub fn terminal_width() -> usize {
    return match terminal_size::terminal_size() {
        Some((Width(w), _)) => w as usize,
        None => 120,
    };
}

pub fn color_from_index(index: usize) -> Color {
    return match index {
        0 => Color::Green,
        1 => Color::Red,
        2 => Color::Blue,
        3 => Color::Magenta,
        4 => Color::Yellow,
        5 => Color::Cyan,
        6 => Color::LightRed,
        7 => Color::LightGreen,
        8 => Color::LightBlue,
        9 => Color::LightMagenta,
        10 => Color::LightYellow,
        11 => Color::LightCyan,
        _ => Color::White,
    };
}

fn key_to_color_map(list: &Vec<String>) -> HashMap<String, Color> {
    let mut res: HashMap<String, Color> = HashMap::new();
    res.insert(String::from(""), Color::Black);
    let mut color_index = 0;
    for entry in list {
        if !res.contains_key(entry) {
            res.insert(entry.clone(), color_from_index(color_index));
            color_index += 1;
        }
    }

    return res;
}

pub fn get_labels(model: &Model, settings: &Settings) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();

    if settings.full {
        for class in &model.classes {
            for title in &class.titles {
                let label = format!("{}: {}", class.class, title.title);
                labels.push(label);
            }
        }
    } else if settings.class_arg == "" {
        for class in &model.classes {
            labels.push(class.class.clone());
        }
    } else {
        if let Some(class) = model.classes.iter().find(|c| c.class == settings.class_arg) {
            for title in &class.titles {
                labels.push(title.title.clone());
            }
        }
    }

    labels
}
