use crate::log_parsing::{compute_durations, timeline};
use crate::log_reader::LogReader;
use crate::model::{Class, Model};
use crate::Settings;
use std::time::Duration;
// use colored::{Color, Colorize};
use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use std::collections::HashMap;
use std::fmt::Write;
use terminal_size::Width;

pub fn render_log(model: &Model, settings: &Settings) -> Option<Text<'static>> {
    let mut reader = LogReader::new(settings);
    if !reader.is_empty() {
        match compute_durations(&mut reader, settings) {
            Ok((durations, total)) => {
                if durations.is_empty() {
                    if &settings.class_arg == "" {
                        println!("Empty log.");
                    } else {
                        println!("Class \"{}\" not found in log.", &settings.class_arg);
                    }
                    return None;
                }

                // println!("{}", durations);
                let colors = key_to_color_map(&durations);
                let labels: Vec<String> = durations.iter().map(|(s, _)| s.clone()).collect();
                // header(settings);

                let timelines = render_timelines(model, &colors, labels, settings);

                return Some(timelines);
            }
            Err(e) => {
                eprintln!("Failed to compute durations: {e:?}");
            }
        }
    } else {
        println!(
            "Log files not found in the following interval.\n{:?}",
            settings.interval
        );
    }
    return None;
}

pub fn header(settings: &Settings) -> String {
    let mut res = String::from("");
    let date_str = settings.interval.date_str();
    let term_width = terminal_width();

    let inner_width = date_str.len() + 2;
    let box_width = inner_width + 2;

    if box_width > term_width {
        res.push_str(&date_str);
        return res;
    }

    let start_column = ((term_width - box_width) as f32 / 2.0).round() as usize;
    let start_column_int_div = (term_width - box_width) / 2;
    let rounded_up = start_column == start_column_int_div;
    let pad = " ".repeat(start_column);
    let top_border_left = "─".repeat(start_column - 1);
    let top_border_right = "─".repeat(start_column - (1 + if rounded_up { 0 } else { 1 }));

    res.push_str(&format!("{}╭{}╮\n", pad, "─".repeat(inner_width)));
    res.push_str(&format!("{}│ {} │\n", pad, date_str));
    res.push_str(&format!(
        "┌{}┴{}┴{}┐\n",
        top_border_left,
        "─".repeat(inner_width),
        top_border_right
    ));
    res.push_str(&format!("\n"));

    return res;
}

const FANCY_TIMELINE: bool = true;
const CUTOFF: usize = usize::MAX; // not doing anything but the setting is here

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
            if count >= CUTOFF {
                break;
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

    for section_data in sections {
        let key = if settings.multi_timeline {
            label.expect("label required when multi_timeline")
        } else {
            &section_data.0
        };

        if let Some(color) = colors.get(key) {
            // Old behavior was effectively:
            //   STRIKE_ON + (colored glyph) + STRIKE_OFF
            //
            // In Ratatui, "strike + color" must be part of the SAME style on the Span.
            // To match your old look, when FANCY_TIMELINE is enabled we apply CROSSED_OUT
            // broadly (so the decoration stays colored with the fg).
            let mut base_style = Style::default().fg(*color);
            if FANCY_TIMELINE {
                base_style = base_style.add_modifier(Modifier::CROSSED_OUT);
            }

            let (s, style) = if *color == Color::Black {
                if FANCY_TIMELINE {
                    (
                        " ".to_string(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD | Modifier::CROSSED_OUT),
                    )
                } else {
                    ("—".to_string(), Style::default().fg(Color::White))
                }
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

fn choose_character(section_data: (String, i64, i64, bool, bool), settings: &Settings) -> char {
    let width = terminal_width();
    let ms_per_section = (settings.interval.width() / (width as u64)) as f64;
    let fullness = section_data.2 as f64 / ms_per_section as f64;
    if FANCY_TIMELINE {
        if section_data.3 && section_data.4 {
            // there is activity near both the left and right side of a section
            return '█';
        } else if section_data.3 {
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
        } else if section_data.4 {
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
    } else {
        if fullness == 0.0 {
            return ' ';
        }
        return '█';
    }
}

pub fn build_class_table(
    model: &Model,
    selected_class: &Option<(String, usize)>,
    width: u16,
) -> Table<'static> {
    let mut rows: Vec<(&str, u64, usize)> = model
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| (c.class.as_str(), c.total_duration(&model.logs), i))
        .collect();

    // If you want to enforce the same cutoff semantics as before,
    // you can keep sorting here (or assume model.sort() already did it).
    // rows.sort_by(|a, b| b.1.cmp(&a.1)); // optional

    let total: u64 = rows.iter().map(|(_, dur, _)| *dur).sum();

    let mut max_class_width = rows
        .iter()
        .map(|(class, _, _)| class.len())
        .max()
        .unwrap_or(0);

    // Cap by terminal width so it doesn't explode.
    let max_string_length = terminal_width().saturating_sub(20);
    max_class_width = max_class_width.min(max_string_length);

    let mut table_rows: Vec<Row<'static>> = Vec::new();
    let mut total_percentage = 0.0;
    let mut total_duration: u64 = 0;

    for (count, (class, duration, class_index)) in rows.iter().take(CUTOFF).enumerate() {
        let _ = count; // (remove if unused later)
        total_duration += *duration;

        let percent = if total == 0 {
            0.0
        } else {
            100.0 * (*duration as f64 / total as f64)
        };
        total_percentage += percent;

        // Selection styling
        let is_selected = selected_class
            .as_ref()
            .map(|(name, idx)| *idx == *class_index && name == class)
            .unwrap_or(false);

        // Color placeholder for now (since theme/colors aren’t wired in yet)
        // Later: let color = theme.color_for_class(class);
        let mut class_style = Style::default().fg(color_from_index(*class_index));
        if is_selected {
            class_style = class_style.add_modifier(Modifier::REVERSED);
        }

        let class_cell = Cell::from(truncate_string(class, max_string_length)).style(class_style);

        let dur_cell = {
            let mut s = Style::default().fg(color_from_index(*class_index));
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            Cell::from(format_duration(*duration)).style(s)
        };

        let pct_cell = {
            let mut s = Style::default().fg(color_from_index(*class_index));
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            Cell::from(format!("{:>7.2}%", percent)).style(s)
        };

        table_rows.push(Row::new(vec![class_cell, dur_cell, pct_cell]));
    }

    // Add "Total" row (bold)
    table_rows.push(
        Row::new(vec![
            Cell::from(truncate_string("Total", max_string_length))
                .style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(format_duration(total_duration))
                .style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(format!("{:>7.2}%", total_percentage))
                .style(Style::default().add_modifier(Modifier::BOLD)),
        ])
        .top_margin(1),
    );

    let widths = [
        Constraint::Length(width as u16 - 20),
        Constraint::Length(11),
        Constraint::Length(9),
    ];

    Table::new(table_rows, widths)
        .header(
            Row::new(vec!["Class", "Duration", "Percent"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::BOTTOM | Borders::LEFT))
        .column_spacing(1)
}

pub fn build_title_table(
    model: &Model,
    selected_class: &Option<(String, usize)>,
    selected_title: &Option<(String, usize)>,
) -> Table<'static> {
    // Pick the class we’re showing titles for.
    // If none selected, show nothing (just Total=0).
    let class_opt: Option<&Class> = selected_class
        .as_ref()
        .and_then(|(_name, idx)| model.classes.get(*idx));

    // Build rows: (title, duration, title_index)
    let mut rows: Vec<(&str, u64, usize)> = match class_opt {
        Some(class) => class
            .titles
            .iter()
            .enumerate()
            .map(|(i, t)| (t.title.as_str(), t.total_duration(&model.logs), i))
            .collect(),
        None => Vec::new(),
    };

    // Optional: if you want local sorting independent of model.sort()
    // rows.sort_by(|a, b| b.1.cmp(&a.1));

    let total: u64 = rows.iter().map(|(_, dur, _)| *dur).sum();

    let mut max_title_width = rows
        .iter()
        .map(|(title, _, _)| title.len())
        .max()
        .unwrap_or(0);

    // Cap by terminal width so it doesn't explode.
    let max_string_length = terminal_width().saturating_sub(20);
    max_title_width = max_title_width.min(max_string_length);

    let mut table_rows: Vec<Row<'static>> = Vec::new();
    let mut total_percentage = 0.0;
    let mut total_duration: u64 = 0;

    for (title, duration, title_index) in rows.iter().take(CUTOFF) {
        total_duration += *duration;

        let percent = if total == 0 {
            0.0
        } else {
            100.0 * (*duration as f64 / total as f64)
        };
        total_percentage += percent;

        // Selection styling (title selection is within the selected class)
        let is_selected = selected_title
            .as_ref()
            .map(|(name, idx)| *idx == *title_index && name == title)
            .unwrap_or(false);

        let mut title_style = Style::default();
        if is_selected {
            title_style = title_style.add_modifier(Modifier::REVERSED);
        }

        let title_cell = Cell::from(truncate_string(title, max_string_length)).style(title_style);

        let dur_cell = {
            let mut s = Style::default();
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            Cell::from(format_duration(*duration)).style(s)
        };

        let pct_cell = {
            let mut s = Style::default();
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            Cell::from(format!("{:>7.2}%", percent)).style(s)
        };

        table_rows.push(Row::new(vec![title_cell, dur_cell, pct_cell]));
    }

    // Add "Total" row (bold)
    table_rows.push(
        Row::new(vec![
            Cell::from(truncate_string("Total", max_string_length))
                .style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(format_duration(total_duration))
                .style(Style::default().add_modifier(Modifier::BOLD)),
            Cell::from(format!("{:>7.2}%", total_percentage))
                .style(Style::default().add_modifier(Modifier::BOLD)),
        ])
        .top_margin(1),
    );

    let widths = [
        Constraint::Length(max_title_width as u16),
        Constraint::Length(10),
        Constraint::Length(9),
    ];

    Table::new(table_rows, widths)
        .header(
            Row::new(vec!["Title", "Duration", "Percent"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::BOTTOM | Borders::RIGHT))
        .column_spacing(1)
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        ".".repeat(max_len) // handles silly small max_len
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

fn terminal_width() -> usize {
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

fn key_to_color_map(list: &Vec<(String, u64)>) -> HashMap<String, Color> {
    let mut res: HashMap<String, Color> = HashMap::new();
    res.insert(String::from(""), Color::Black);
    let mut color_index = 0;
    for entry in list {
        if !res.contains_key(&entry.0) {
            res.insert(entry.0.clone(), color_from_index(color_index));
            color_index += 1;
        }
    }

    return res;
}
