use crate::model::Model;
use crate::ticks::{overlay_line_with_offset, tick_row};
use crate::timeline_cache::{TimelineCache, TimelineCacheEntry, TimelineCacheKey, TimelineMode};
use crate::timeline_sections::{timeline_for_interval, TimelineCharacter};
use crate::Settings;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use std::collections::HashMap;
use std::time::Duration;
use terminal_size::Width;

pub fn render_log(
    model: &Model,
    settings: &Settings,
    cache: &mut TimelineCache,
) -> Result<Text<'static>, String> {
    let labels = get_labels(model, settings);

    if labels.is_empty() {
        return Err("Empty log".to_string());
    }

    if settings.multi_timeline {
        return Ok(crate::multi_timeline::render_multi_timelines(
            model, labels, settings, cache,
        ));
    }

    let colors = key_to_color_map(&labels);
    let timelines = render_timelines(model, &colors, settings, cache);

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
    settings: &Settings,
    cache: &mut TimelineCache,
) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(Line::from(""));
    lines.push(build_timeline(model, colors, settings, None, cache));
    lines.push(Line::from("\n"));
    return Text::from(lines);
}

fn build_timeline(
    model: &Model,
    colors: &HashMap<String, Color>,
    settings: &Settings,
    label: Option<&String>,
    cache: &mut TimelineCache,
) -> Line<'static> {
    let width = terminal_width();
    if width == 0 {
        return Line::from("");
    }

    let ms_per_section_u64 = settings.focused_interval.width() / width as u64;
    if ms_per_section_u64 == 0 {
        return Line::from("");
    }

    let start_ms = settings.focused_interval.start.timestamp_millis() as u64;
    let end_ms = start_ms + ms_per_section_u64 * width as u64;
    let key = TimelineCacheKey {
        ms_per_character: ms_per_section_u64,
        mode: single_mode(settings),
        label: if settings.class_arg.is_empty() {
            None
        } else {
            Some(settings.class_arg.clone())
        },
    };

    let (timeline, section_labels) =
        materialize_single_timeline(model, settings, cache, &key, start_ms, end_ms, width, label);

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run_text = String::new();
    let mut run_style: Option<Style> = None;

    let timeline_chars: Vec<char> = timeline.chars().collect();
    for (idx, ch) in timeline_chars.iter().enumerate() {
        let section_label = if settings.multi_timeline {
            label.expect("label required when multi_timeline")
        } else {
            section_labels.get(idx).map_or("", |s| s.as_str())
        };

        let color = *colors.get(section_label).unwrap_or(&Color::White);
        let (ch, style) = if color == Color::Black {
            (
                ' ',
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::CROSSED_OUT),
            )
        } else {
            (
                *ch,
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::CROSSED_OUT),
            )
        };

        if let Some(s) = run_style {
            if s == style {
                run_text.push(ch);
                continue;
            }

            spans.push(Span::styled(std::mem::take(&mut run_text), s));
        }

        run_style = Some(style);
        run_text.push(ch);
    }

    if let Some(s) = run_style {
        spans.push(Span::styled(run_text, s));
    }

    Line::from(spans)
}

pub(crate) fn choose_character(section_data: &TimelineCharacter, ms_per_section: f64) -> char {
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

fn single_mode(settings: &Settings) -> TimelineMode {
    match (settings.class_arg.is_empty(), settings.full) {
        (true, false) => TimelineMode::SingleAll,
        (true, true) => TimelineMode::SingleAllFull,
        (false, false) => TimelineMode::SingleClass,
        (false, true) => TimelineMode::SingleClassFull,
    }
}

fn dynamic_column_range(
    start_ms: u64,
    width: usize,
    ms_per_char: u64,
    has_dynamic: bool,
) -> (usize, usize) {
    if !has_dynamic || width == 0 || ms_per_char == 0 {
        return (0, 0);
    }

    let end_ms = start_ms + ms_per_char * width as u64;
    let now_ms = chrono::Utc::now().timestamp_millis() as u64;
    if now_ms < start_ms || now_ms >= end_ms {
        return (0, 0);
    }

    let col = ((now_ms - start_ms) / ms_per_char) as usize;
    let col_start = col.saturating_sub(1);
    let col_end = (col + 2).min(width);
    (col_start, col_end)
}

fn cacheable_column_ranges(width: usize, dynamic_range: (usize, usize)) -> Vec<(usize, usize)> {
    let (dyn_start, dyn_end) = dynamic_range;
    if dyn_end > dyn_start {
        let mut ranges = Vec::new();
        if dyn_start > 0 {
            ranges.push((0, dyn_start));
        }
        if dyn_end < width {
            ranges.push((dyn_end, width));
        }
        ranges
    } else {
        vec![(0, width)]
    }
}

fn single_has_dynamic_open_log(model: &Model, settings: &Settings) -> bool {
    let Some(last) = model.logs.last() else {
        return false;
    };
    if last.end.is_some() {
        return false;
    }

    settings.class_arg.is_empty() || settings.full || last.class == settings.class_arg
}

fn materialize_single_timeline(
    model: &Model,
    settings: &Settings,
    cache: &mut TimelineCache,
    key: &TimelineCacheKey,
    start_ms: u64,
    end_ms: u64,
    width: usize,
    label: Option<&String>,
) -> (String, Vec<String>) {
    let ms_per_char = key.ms_per_character;
    let dynamic_range = dynamic_column_range(
        start_ms,
        width,
        ms_per_char,
        single_has_dynamic_open_log(model, settings),
    );

    if dynamic_range.1 <= dynamic_range.0 {
        if let Some(entry) = cache.exact_entry(key, start_ms, end_ms) {
            return (entry.timeline.clone(), entry.section_labels.clone());
        }
    }

    let mut chars: Vec<Option<char>> = vec![None; width];
    let mut labels: Vec<Option<String>> = vec![None; width];

    if let Some(entries) = cache.entries_for_key(key) {
        for entry in entries {
            let overlap_start = start_ms.max(entry.start_ms);
            let overlap_end = end_ms.min(entry.end_ms);
            if overlap_start >= overlap_end {
                continue;
            }

            let mut col_start = ((overlap_start - start_ms) / ms_per_char) as usize;
            let mut col_end = ((overlap_end - start_ms) / ms_per_char) as usize;
            if col_start >= width {
                continue;
            }
            col_end = col_end.min(width);
            if col_end <= col_start {
                continue;
            }

            let (dyn_start, dyn_end) = dynamic_range;
            if dyn_end > dyn_start && col_start < dyn_end && col_end > dyn_start {
                if col_start < dyn_start {
                    col_end = dyn_start;
                } else {
                    col_start = dyn_end;
                }
            }

            if col_end <= col_start {
                continue;
            }

            let dst_start_ms = start_ms + (col_start as u64) * ms_per_char;
            if dst_start_ms < entry.start_ms {
                let advance_cols = (entry.start_ms - dst_start_ms).div_ceil(ms_per_char) as usize;
                col_start = (col_start + advance_cols).min(width);
                if col_end <= col_start {
                    continue;
                }
            }

            let src_start = ((start_ms + (col_start as u64) * ms_per_char - entry.start_ms)
                / ms_per_char) as usize;
            let entry_chars: Vec<char> = entry.timeline.chars().collect();
            for i in 0..(col_end - col_start) {
                let dst = col_start + i;
                let src = src_start + i;
                if dst >= width || src >= entry_chars.len() {
                    break;
                }
                if chars[dst].is_none() {
                    chars[dst] = Some(entry_chars[src]);
                    labels[dst] = Some(entry.section_labels.get(src).cloned().unwrap_or_default());
                }
            }
        }
    }

    let mut gap_start: Option<usize> = None;
    for i in 0..=width {
        let missing = i < width && chars[i].is_none();
        if missing {
            if gap_start.is_none() {
                gap_start = Some(i);
            }
            continue;
        }

        if let Some(s) = gap_start {
            let e = i;
            let seg_width = e - s;
            if seg_width > 0 {
                let seg_start_ms = start_ms + (s as u64) * ms_per_char;
                let seg_end_ms = start_ms + (e as u64) * ms_per_char;
                let interval = crate::interval::Interval {
                    start: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
                        seg_start_ms as i64,
                    )
                    .expect("invalid start timestamp"),
                    end: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(seg_end_ms as i64)
                        .expect("invalid end timestamp"),
                    changed: false,
                };
                let sections = timeline_for_interval(model, seg_width, &interval, settings, label);

                let mut seg_chars = Vec::with_capacity(seg_width);
                let mut seg_labels = Vec::with_capacity(seg_width);
                for section in &sections {
                    seg_chars.push(choose_character(section, ms_per_char as f64));
                    seg_labels.push(section.label.clone());
                }

                for j in 0..seg_width {
                    chars[s + j] = Some(seg_chars[j]);
                    labels[s + j] = Some(seg_labels[j].clone());
                }

                let local_dynamic_range = (
                    dynamic_range.0.saturating_sub(s).min(seg_width),
                    dynamic_range.1.saturating_sub(s).min(seg_width),
                );
                for (cache_s, cache_e) in cacheable_column_ranges(seg_width, local_dynamic_range) {
                    if cache_e <= cache_s {
                        continue;
                    }
                    let mut timeline = String::with_capacity(cache_e - cache_s);
                    for ch in &seg_chars[cache_s..cache_e] {
                        timeline.push(*ch);
                    }
                    let section_labels = seg_labels[cache_s..cache_e].to_vec();

                    cache.insert(
                        key.clone(),
                        TimelineCacheEntry {
                            start_ms: seg_start_ms + (cache_s as u64) * ms_per_char,
                            end_ms: seg_start_ms + (cache_e as u64) * ms_per_char,
                            timeline,
                            section_labels,
                        },
                    );
                }
            }
            gap_start = None;
        }
    }

    let mut timeline = String::with_capacity(width);
    let mut section_labels = Vec::with_capacity(width);
    for i in 0..width {
        timeline.push(chars[i].unwrap_or(' '));
        section_labels.push(labels[i].clone().unwrap_or_default());
    }

    (timeline, section_labels)
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
