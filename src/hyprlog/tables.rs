use ratatui::{
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::{
    model::{Class, Model},
    view::{color_from_index, format_duration, terminal_width, truncate_string},
};

fn percent_of(duration: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * (duration as f64 / total as f64)
    }
}

fn name_cell(name: &str, style: Style, max_string_length: usize) -> Cell<'static> {
    Cell::from(truncate_string(name, max_string_length)).style(style)
}

fn duration_cell(duration: u64, style: Style) -> Cell<'static> {
    Cell::from(format_duration(duration)).style(style)
}

fn percent_cell(percent: f64, style: Style) -> Cell<'static> {
    Cell::from(format!("{:>7.2}%", percent)).style(style)
}

fn total_row(max_string_length: usize, total_duration: u64, total_percentage: f64) -> Row<'static> {
    Row::new(vec![
        Cell::from(truncate_string("Total", max_string_length))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(format_duration(total_duration))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(format!("{:>7.2}%", total_percentage))
            .style(Style::default().add_modifier(Modifier::BOLD)),
    ])
}

fn table_widths(width: u16, max_duration_width: usize) -> [Constraint; 3] {
    let name_width = width.saturating_sub((max_duration_width + 9) as u16);
    [
        Constraint::Length(name_width),
        Constraint::Length(max_duration_width as u16),
        Constraint::Length(9),
    ]
}

fn ellipsis_row(max_string_length: usize) -> Row<'static> {
    Row::new(vec![
        Cell::from(truncate_string("...", max_string_length))
            .style(Style::default().add_modifier(Modifier::DIM)),
        Cell::from(" ".to_string()).style(Style::default().add_modifier(Modifier::DIM)),
        Cell::from(" ".to_string()).style(Style::default().add_modifier(Modifier::DIM)),
    ])
}

fn blank_row() -> Row<'static> {
    Row::new(vec![
        Cell::from("".to_string()),
        Cell::from("".to_string()),
        Cell::from("".to_string()),
    ])
}

fn window_counts(scroll: usize, body_capacity: usize, len: usize) -> (bool, bool, usize) {
    if len == 0 || body_capacity == 0 {
        return (false, false, 0);
    }

    let top_ellipsis = scroll > 0;
    let mut slots = body_capacity;
    if top_ellipsis {
        if slots == 0 {
            return (true, false, 0);
        }
        slots -= 1;
    }

    let remaining = len.saturating_sub(scroll);
    if remaining <= slots {
        (top_ellipsis, false, remaining)
    } else {
        if slots == 0 {
            (top_ellipsis, true, 0)
        } else {
            // Reserve one slot for the bottom ellipsis row.
            (top_ellipsis, true, slots - 1)
        }
    }
}

pub fn build_class_table(
    model: &Model,
    selected_class: &Option<(String, usize)>,
    width: u16,
    scroll: usize,
    body_capacity: usize,
) -> Table<'static> {
    let len = model.classes.len();
    let scroll = if len == 0 { 0 } else { scroll.min(len - 1) };

    let total: u64 = model
        .classes
        .iter()
        .map(|c| c.total_duration(&model.logs))
        .sum();

    // let mut max_class_width = rows
    //     .iter()
    //     .map(|(class, _, _)| class.len())
    //     .max()
    //     .unwrap_or(0);

    // Cap by terminal width so it doesn't explode.
    let max_string_length = terminal_width().saturating_sub(20);
    let max_duration_width = std::cmp::max("Duration".len(), format_duration(total).len());
    // max_class_width = max_class_width.min(max_string_length);

    let mut table_rows: Vec<Row<'static>> = Vec::new();
    let (top_ellipsis, bottom_ellipsis, real_count) = window_counts(scroll, body_capacity, len);
    if top_ellipsis {
        table_rows.push(ellipsis_row(max_string_length));
    }

    for class_index in scroll..(scroll + real_count) {
        let class = &model.classes[class_index];
        let duration = class.total_duration(&model.logs);
        let percent = percent_of(duration, total);

        let is_selected = selected_class
            .as_ref()
            .map(|(name, idx)| *idx == class_index && name == &class.class)
            .unwrap_or(false);

        let class_has_selection = selected_class.is_some();

        let mut class_style = Style::default();
        if !class_has_selection {
            class_style = class_style.fg(color_from_index(class_index));
        }
        if is_selected {
            class_style = class_style.add_modifier(Modifier::REVERSED);
        }

        let class_cell = name_cell(class.class.as_str(), class_style, max_string_length);

        let dur_cell = {
            let mut s = Style::default();
            if !class_has_selection {
                s = s.fg(color_from_index(class_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            duration_cell(duration, s)
        };

        let pct_cell = {
            let mut s = Style::default();
            if !class_has_selection {
                s = s.fg(color_from_index(class_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            percent_cell(percent, s)
        };

        table_rows.push(Row::new(vec![class_cell, dur_cell, pct_cell]));
    }

    if bottom_ellipsis {
        table_rows.push(ellipsis_row(max_string_length));
    }

    // Pad so the pinned total row lands at the bottom.
    while table_rows.len() < body_capacity {
        table_rows.push(blank_row());
    }

    let total_percentage = if total == 0 { 0.0 } else { 100.0 };
    table_rows.push(total_row(max_string_length, total, total_percentage));

    let widths = table_widths(width, max_duration_width);

    Table::new(table_rows, widths)
        .header(
            Row::new(vec!["Class", "Duration", " Percent"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::BOTTOM | Borders::LEFT))
        .column_spacing(1)
}

pub fn build_title_table(
    model: &Model,
    selected_class: &Option<(String, usize)>,
    selected_title: &Option<(String, usize)>,
    width: u16,
    scroll: usize,
    body_capacity: usize,
) -> Table<'static> {
    let class_opt: Option<&Class> = selected_class
        .as_ref()
        .and_then(|(_name, idx)| model.classes.get(*idx));

    let len = match class_opt {
        Some(class) => class.titles.len(),
        None => model.titles.len(),
    };
    let scroll = if len == 0 { 0 } else { scroll.min(len - 1) };

    let total: u64 = match class_opt {
        Some(class) => class.total_duration(&model.logs),
        None => model
            .classes
            .iter()
            .map(|c| c.total_duration(&model.logs))
            .sum(),
    };

    let max_string_length = terminal_width().saturating_sub(20);
    let max_duration_width = std::cmp::max("Duration".len(), format_duration(total).len());

    let mut table_rows: Vec<Row<'static>> = Vec::new();
    let (top_ellipsis, bottom_ellipsis, real_count) = window_counts(scroll, body_capacity, len);
    if top_ellipsis {
        table_rows.push(ellipsis_row(max_string_length));
    }

    match class_opt {
        Some(class) => {
            for title_index in scroll..(scroll + real_count) {
                let title = &class.titles[title_index];
                let duration = title.total_duration(&model.logs);
                let percent = percent_of(duration, total);

                let is_selected = selected_title
                    .as_ref()
                    .map(|(name, idx)| *idx == title_index && name == &title.title)
                    .unwrap_or(false);

                let mut title_style = Style::default().fg(color_from_index(title_index));
                if is_selected {
                    title_style = title_style.add_modifier(Modifier::REVERSED);
                }

                let title_cell = name_cell(title.title.as_str(), title_style, max_string_length);

                let dur_cell = {
                    let mut s = Style::default().fg(color_from_index(title_index));
                    if is_selected {
                        s = s.add_modifier(Modifier::REVERSED);
                    }
                    duration_cell(duration, s)
                };

                let pct_cell = {
                    let mut s = Style::default().fg(color_from_index(title_index));
                    if is_selected {
                        s = s.add_modifier(Modifier::REVERSED);
                    }
                    percent_cell(percent, s)
                };

                table_rows.push(Row::new(vec![title_cell, dur_cell, pct_cell]));
            }
        }
        None => {
            for i in scroll..(scroll + real_count) {
                let (class_index, title_index) = model.titles[i];
                let title = &model.classes[class_index].titles[title_index];
                let duration = title.total_duration(&model.logs);
                let percent = percent_of(duration, total);

                let title_style = Style::default().fg(color_from_index(class_index));
                let title_cell = name_cell(title.title.as_str(), title_style, max_string_length);

                let dur_cell = {
                    let s = Style::default().fg(color_from_index(class_index));
                    duration_cell(duration, s)
                };

                let pct_cell = {
                    let s = Style::default().fg(color_from_index(class_index));
                    percent_cell(percent, s)
                };

                table_rows.push(Row::new(vec![title_cell, dur_cell, pct_cell]));
            }
        }
    }

    if bottom_ellipsis {
        table_rows.push(ellipsis_row(max_string_length));
    }

    // Pad so the pinned total row lands at the bottom.
    while table_rows.len() < body_capacity {
        table_rows.push(blank_row());
    }

    let total_percentage = if total == 0 { 0.0 } else { 100.0 };
    table_rows.push(total_row(max_string_length, total, total_percentage));

    let widths = table_widths(width, max_duration_width);

    Table::new(table_rows, widths)
        .header(
            Row::new(vec!["Title", "Duration", " Percent"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::BOTTOM | Borders::RIGHT))
        .column_spacing(1)
}
