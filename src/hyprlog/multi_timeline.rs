use crate::model::Model;
use crate::timeline_cache::{TimelineCache, TimelineCacheEntry, TimelineCacheKey, TimelineMode};
use crate::timeline_sections::timeline_for_interval;
use crate::view::{choose_character, color_from_index, terminal_width};
use crate::Settings;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

fn multi_mode(settings: &Settings) -> TimelineMode {
    if settings.full {
        TimelineMode::MultiFull
    } else if settings.class_arg.is_empty() {
        TimelineMode::MultiClass
    } else {
        TimelineMode::MultiTitle
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

fn multi_label_has_dynamic_open_log(
    model: &Model,
    settings: &Settings,
    mode: &TimelineMode,
    label: &str,
) -> bool {
    let Some(last) = model.logs.last() else {
        return false;
    };
    if last.end.is_some() {
        return false;
    }

    match mode {
        TimelineMode::MultiClass => last.class == label,
        TimelineMode::MultiTitle => last.class == settings.class_arg && last.title == label,
        TimelineMode::MultiFull => format!("{}: {}", last.class, last.title) == label,
        _ => false,
    }
}

fn materialize_multi_timeline(
    model: &Model,
    settings: &Settings,
    cache: &mut TimelineCache,
    key: &TimelineCacheKey,
    start_ms: u64,
    end_ms: u64,
    width: usize,
    label: &String,
    has_dynamic: bool,
) -> String {
    let ms_per_char = key.ms_per_character;
    let dynamic_range = dynamic_column_range(start_ms, width, ms_per_char, has_dynamic);

    if dynamic_range.1 <= dynamic_range.0 {
        if let Some(entry) = cache.exact_entry(key, start_ms, end_ms) {
            return entry.timeline.clone();
        }
    }

    let mut chars: Vec<Option<char>> = vec![None; width];

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
                let sections =
                    timeline_for_interval(model, seg_width, &interval, settings, Some(label));

                let mut seg_chars = Vec::with_capacity(seg_width);
                for section in &sections {
                    seg_chars.push(choose_character(section, ms_per_char as f64));
                }

                for j in 0..seg_width {
                    chars[s + j] = Some(seg_chars[j]);
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

                    cache.insert(
                        key.clone(),
                        TimelineCacheEntry {
                            start_ms: seg_start_ms + (cache_s as u64) * ms_per_char,
                            end_ms: seg_start_ms + (cache_e as u64) * ms_per_char,
                            timeline,
                            section_labels: Vec::new(),
                        },
                    );
                }
            }
            gap_start = None;
        }
    }

    let mut timeline = String::with_capacity(width);
    for ch in chars {
        timeline.push(ch.unwrap_or(' '));
    }
    timeline
}

pub fn render_multi_timelines(
    model: &Model,
    labels: Vec<String>,
    settings: &Settings,
    cache: &mut TimelineCache,
) -> Text<'static> {
    let width = terminal_width();
    if width == 0 {
        return Text::from(Vec::<Line<'static>>::new());
    }

    let interval_width = settings.focused_interval.width();
    let ms_per_character = interval_width / width as u64;
    if ms_per_character == 0 {
        return Text::from(Vec::<Line<'static>>::new());
    }

    let start_ms = settings.focused_interval.start.timestamp_millis() as u64;
    let end_ms = start_ms + ms_per_character * width as u64;
    let mode = multi_mode(settings);

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(1 + labels.len() * 2);
    lines.push(Line::from(""));

    for (idx, label) in labels.iter().enumerate() {
        if label.is_empty() {
            continue;
        }

        let key_label = if matches!(mode, TimelineMode::MultiTitle) {
            Some(format!("{}::{label}", settings.class_arg))
        } else {
            Some(label.clone())
        };

        let key = TimelineCacheKey {
            ms_per_character,
            mode: mode.clone(),
            label: key_label,
        };

        let timeline = materialize_multi_timeline(
            model,
            settings,
            cache,
            &key,
            start_ms,
            end_ms,
            width,
            label,
            multi_label_has_dynamic_open_log(model, settings, &mode, label),
        );

        let color = color_from_index(idx);
        let span = if color == Color::Black {
            Span::styled(
                " ".repeat(width),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD | Modifier::CROSSED_OUT),
            )
        } else {
            Span::styled(
                timeline,
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::CROSSED_OUT),
            )
        };

        lines.push(Line::from(span));
        lines.push(Line::from("\n"));
    }

    Text::from(lines)
}
