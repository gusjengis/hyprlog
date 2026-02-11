use std::time::SystemTime;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::model::{Class, Log, Model};
use crate::view::{color_from_index, terminal_width};
use crate::Settings;

#[derive(Copy, Clone, Default)]
struct SectionAcc {
    total: u64,
    flags: u8,
}

impl SectionAcc {
    #[inline]
    fn set_left(&mut self) {
        self.flags |= 0b01;
    }

    #[inline]
    fn set_right(&mut self) {
        self.flags |= 0b10;
    }

    #[inline]
    fn left(self) -> bool {
        (self.flags & 0b01) != 0
    }

    #[inline]
    fn right(self) -> bool {
        (self.flags & 0b10) != 0
    }
}

#[inline]
fn now_millis() -> u64 {
    // We already use chrono elsewhere; this avoids pulling it into the hot loop.
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(_) => 0,
    }
}

#[inline]
fn class_index(model: &Model, class: &str) -> Option<usize> {
    model.class_index(class)
}

#[inline]
fn title_index(class: &Class, title: &str) -> Option<usize> {
    class.title_index(title)
}

#[inline]
fn section_index(starting_ms: u64, ms_per_section: u64, timestamp: u64) -> usize {
    ((timestamp - starting_ms) / ms_per_section) as usize
}

#[inline]
fn char_for_section(acc: SectionAcc, ms_per_section: u64) -> char {
    let total = acc.total;
    if total == 0 {
        return ' ';
    }

    let left = acc.left();
    let right = acc.right();
    if left && right {
        return '█';
    }

    if left {
        let t8 = (total as u128) * 8;
        let m = ms_per_section as u128;
        if t8 <= m {
            '▏'
        } else if t8 <= m * 2 {
            '▎'
        } else if t8 <= m * 3 {
            '▍'
        } else if t8 <= m * 4 {
            '▌'
        } else if t8 <= m * 5 {
            '▋'
        } else if t8 <= m * 6 {
            '▊'
        } else {
            '█'
        }
    } else if right {
        let t8 = (total as u128) * 8;
        let m = ms_per_section as u128;
        if t8 <= m {
            '🮇'
        } else if t8 <= m * 2 {
            '🮈'
        } else if t8 <= m * 3 {
            '▐'
        } else if t8 <= m * 4 {
            '🮉'
        } else if t8 <= m * 5 {
            '🮊'
        } else if t8 <= m * 6 {
            '🮋'
        } else {
            '█'
        }
    } else {
        // Match existing behavior: thresholds at ~0.333 and 0.66.
        let t3 = (total as u128) * 3;
        let m = ms_per_section as u128;
        if t3 <= m {
            '│'
        } else if (total as u128) * 100 <= m * 66 {
            '┃'
        } else {
            '█'
        }
    }
}

fn style_for_color(color: Color) -> (Style, char) {
    if color == Color::Black {
        (
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::CROSSED_OUT),
            ' ',
        )
    } else {
        (
            Style::default()
                .fg(color)
                .add_modifier(Modifier::CROSSED_OUT),
            '\0',
        )
    }
}

fn label_count_for_multi(model: &Model, settings: &Settings) -> usize {
    if settings.full {
        model.classes.iter().map(|c| c.titles.len()).sum()
    } else if settings.class_arg.is_empty() {
        model.classes.len()
    } else if let Some(class) = model.get_class(&settings.class_arg) {
        class.titles.len()
    } else {
        0
    }
}

fn class_offsets(model: &Model) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(model.classes.len() + 1);
    offsets.push(0);
    let mut acc = 0;
    for c in &model.classes {
        acc += c.titles.len();
        offsets.push(acc);
    }
    offsets
}

#[inline]
fn line_index_for_log_full(model: &Model, offsets: &[usize], log: &Log) -> Option<usize> {
    let class_idx = class_index(model, log.class.as_str())?;
    let class = model.classes.get(class_idx)?;
    let title_idx = title_index(class, log.title.as_str())?;
    offsets.get(class_idx).copied().map(|off| off + title_idx)
}

#[inline]
fn line_index_for_log_class(model: &Model, log: &Log) -> Option<usize> {
    class_index(model, log.class.as_str())
}

#[inline]
fn line_index_for_log_title(class: &Class, log: &Log) -> Option<usize> {
    title_index(class, log.title.as_str())
}

fn assign_interval_to_line(
    line_sections: &mut [SectionAcc],
    starting_ms: u64,
    ms_per_section: u64,
    clamped_start: u64,
    clamped_end: u64,
) {
    if ms_per_section == 0 || line_sections.is_empty() || clamped_end <= clamped_start {
        return;
    }

    let width = line_sections.len();
    let max_index = width - 1;

    let start_idx = section_index(starting_ms, ms_per_section, clamped_start).min(max_index);
    let end_idx =
        section_index(starting_ms, ms_per_section, clamped_end.saturating_sub(1)).min(max_index);

    let pad = ms_per_section / 10;

    if start_idx == end_idx {
        let i = start_idx;
        let section_start = starting_ms + ms_per_section * i as u64;
        let section_end = section_start + ms_per_section;
        let contrib = section_end
            .min(clamped_end)
            .saturating_sub(section_start.max(clamped_start));
        let s = &mut line_sections[i];
        s.total += contrib;
        if section_start + pad >= clamped_start {
            s.set_left();
        }
        if section_end.saturating_sub(pad) <= clamped_end {
            s.set_right();
        }
        return;
    }

    // First section
    {
        let i = start_idx;
        let section_start = starting_ms + ms_per_section * i as u64;
        let section_end = section_start + ms_per_section;
        let contrib = section_end.min(clamped_end).saturating_sub(clamped_start);
        let s = &mut line_sections[i];
        s.total += contrib;
        if section_start + pad >= clamped_start {
            s.set_left();
        }
        if section_end.saturating_sub(pad) <= clamped_end {
            s.set_right();
        }
    }

    // Middle full sections
    if end_idx > start_idx + 1 {
        for s in &mut line_sections[(start_idx + 1)..end_idx] {
            s.total += ms_per_section;
            s.set_left();
            s.set_right();
        }
    }

    // Last section
    {
        let i = end_idx;
        let section_start = starting_ms + ms_per_section * i as u64;
        let section_end = section_start + ms_per_section;
        let contrib = clamped_end.saturating_sub(section_start.max(clamped_start));
        let s = &mut line_sections[i];
        s.total += contrib;
        if section_start + pad >= clamped_start {
            s.set_left();
        }
        if section_end.saturating_sub(pad) <= clamped_end {
            s.set_right();
        }
    }
}

fn lower_bound_by_start(logs: &[Log], start_ms: u64) -> usize {
    let mut lo = 0usize;
    let mut hi = logs.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if logs[mid].start < start_ms {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

pub fn render_multi_timelines(
    model: &Model,
    labels: Vec<String>,
    settings: &Settings,
) -> Text<'static> {
    let width = terminal_width();
    if width == 0 {
        return Text::from(Vec::<Line<'static>>::new());
    }

    let interval_width = settings.focused_interval.width();
    let ms_per_section = interval_width / width as u64;
    if ms_per_section == 0 {
        return Text::from(Vec::<Line<'static>>::new());
    }

    let starting_ms = settings.focused_interval.start.timestamp_millis() as u64;
    let interval_end = starting_ms + ms_per_section * width as u64;
    let now = now_millis();

    // Determine how many lines we will generate.
    let lines_len = label_count_for_multi(model, settings);
    if lines_len == 0 {
        return Text::from(Vec::<Line<'static>>::new());
    }

    // Flat storage: lines_len * width.
    let mut acc = vec![SectionAcc::default(); lines_len * width];

    let offsets = if settings.full {
        Some(class_offsets(model))
    } else {
        None
    };

    let selected_class = if !settings.full && !settings.class_arg.is_empty() {
        class_index(model, settings.class_arg.as_str()).and_then(|idx| model.classes.get(idx))
    } else {
        None
    };

    let logs = &model.logs;
    let mut i = lower_bound_by_start(logs, starting_ms);
    while i < logs.len() {
        let log = &logs[i];
        if log.start >= interval_end {
            break;
        }
        let end = log.end.unwrap_or(now);
        if end > interval_end {
            i += 1;
            continue;
        }

        // From here on, behavior matches Interval::contains_log (start >= interval_start and end <= interval_end).
        if settings.full {
            let Some(offsets) = offsets.as_ref() else {
                break;
            };

            if let Some(line_idx) = line_index_for_log_full(model, offsets, log) {
                if line_idx < lines_len {
                    let base = line_idx * width;
                    assign_interval_to_line(
                        &mut acc[base..base + width],
                        starting_ms,
                        ms_per_section,
                        log.start,
                        end,
                    );
                }
            }
        } else if settings.class_arg.is_empty() {
            if let Some(line_idx) = line_index_for_log_class(model, log) {
                if line_idx < lines_len {
                    let base = line_idx * width;
                    assign_interval_to_line(
                        &mut acc[base..base + width],
                        starting_ms,
                        ms_per_section,
                        log.start,
                        end,
                    );
                }
            }
        } else if let Some(class) = selected_class {
            if log.class == settings.class_arg {
                if let Some(line_idx) = line_index_for_log_title(class, log) {
                    if line_idx < lines_len {
                        let base = line_idx * width;
                        assign_interval_to_line(
                            &mut acc[base..base + width],
                            starting_ms,
                            ms_per_section,
                            log.start,
                            end,
                        );
                    }
                }
            }
        }

        i += 1;
    }

    // Build output lines.
    // We preserve the previous behavior of an initial blank line and an empty line after each timeline.
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(1 + lines_len * 2);
    lines.push(Line::from(""));

    // We render based on `labels` for stable coloring order.
    // If the label list doesn't match the computed line count, we fall back to best-effort.
    let render_len = std::cmp::min(labels.len(), lines_len);
    for i in 0..render_len {
        let label = &labels[i];
        if label.is_empty() {
            continue;
        }
        let color = if label.is_empty() {
            Color::Black
        } else {
            color_from_index(i)
        };
        let (style, forced_char) = style_for_color(color);

        let base = i * width;
        let mut s = String::with_capacity(width);
        if forced_char != '\0' {
            // Special-case black: render whitespace with different style.
            for _ in 0..width {
                s.push(forced_char);
            }
        } else {
            for sec in &acc[base..base + width] {
                s.push(char_for_section(*sec, ms_per_section));
            }
        }

        lines.push(Line::from(Span::styled(s, style)));
        lines.push(Line::from("\n"));
    }

    Text::from(lines)
}
