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

fn duration_cell(duration: u64, style: Style, max_duration_width: &mut usize) -> Cell<'static> {
    let duration_string = format_duration(duration);
    if duration_string.len() > *max_duration_width {
        *max_duration_width = duration_string.len();
    }

    Cell::from(duration_string).style(style)
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
    .top_margin(1)
}

fn table_widths(width: u16, max_duration_width: usize) -> [Constraint; 3] {
    [
        Constraint::Length(width as u16 - (max_duration_width + 9) as u16),
        Constraint::Length(max_duration_width as u16),
        Constraint::Length(9),
    ]
}

pub fn build_class_table(
    model: &Model,
    selected_class: &Option<(String, usize)>,
    width: u16,
) -> Table<'static> {
    let rows: Vec<(&str, u64, usize)> = model
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| (c.class.as_str(), c.total_duration(&model.logs), i))
        .collect();

    let total: u64 = rows.iter().map(|(_, dur, _)| *dur).sum();

    // let mut max_class_width = rows
    //     .iter()
    //     .map(|(class, _, _)| class.len())
    //     .max()
    //     .unwrap_or(0);

    // Cap by terminal width so it doesn't explode.
    let max_string_length = terminal_width().saturating_sub(20);
    let mut max_duration_width = "Duration".len();
    // max_class_width = max_class_width.min(max_string_length);

    let mut table_rows: Vec<Row<'static>> = Vec::new();
    let mut total_percentage = 0.0;
    let mut total_duration: u64 = 0;

    for (count, (class, duration, class_index)) in rows.iter().enumerate() {
        let _ = count; // (remove if unused later)
        total_duration += *duration;

        let percent = percent_of(*duration, total);
        total_percentage += percent;

        let is_selected = selected_class
            .as_ref()
            .map(|(name, idx)| *idx == *class_index && name == class)
            .unwrap_or(false);

        let class_has_selection = selected_class.is_some();

        let mut class_style = Style::default();
        if !class_has_selection {
            class_style = class_style.fg(color_from_index(*class_index));
        }
        if is_selected {
            class_style = class_style.add_modifier(Modifier::REVERSED);
        }

        let class_cell = name_cell(class, class_style, max_string_length);

        let dur_cell = {
            let mut s = Style::default();
            if !class_has_selection {
                s = s.fg(color_from_index(*class_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            duration_cell(*duration, s, &mut max_duration_width)
        };

        let pct_cell = {
            let mut s = Style::default();
            if !class_has_selection {
                s = s.fg(color_from_index(*class_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            percent_cell(percent, s)
        };

        table_rows.push(Row::new(vec![class_cell, dur_cell, pct_cell]));
    }

    table_rows.push(total_row(
        max_string_length,
        total_duration,
        total_percentage,
    ));

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
) -> Table<'static> {
    let class_opt: Option<&Class> = selected_class
        .as_ref()
        .and_then(|(_name, idx)| model.classes.get(*idx));

    let mut rows: Vec<(&str, u64, usize, usize)> = match class_opt {
        Some(class) => class
            .titles
            .iter()
            .enumerate()
            .map(|(i, t)| (t.title.as_str(), t.total_duration(&model.logs), i, 0))
            .collect(),
        None => model
            .classes
            .iter()
            .enumerate()
            .flat_map(|(class_idx, class)| {
                class
                    .titles
                    .iter()
                    .enumerate()
                    .map(move |(title_idx, title)| {
                        (
                            title.title.as_str(),
                            title.total_duration(&model.logs),
                            title_idx,
                            class_idx,
                        )
                    })
            })
            .collect(),
        // None => model
        //     .titles
        //     .iter()
        //     .map(|(class_idx, title_idx)| {
        //         (
        //             model.classes[*class_idx].titles[*title_idx].title.as_str(),
        //             model.classes[*class_idx].titles[*title_idx].total_duration(&model.logs),
        //             *title_idx,
        //             *class_idx,
        //         )
        //     })
        //     .collect(),
    };

    rows.sort_by(|a, b| b.1.cmp(&a.1));
    // let rows = rows.into_iter().take(CUTOFF).collect::<Vec<_>>();

    let total: u64 = rows.iter().map(|(_, dur, _, _)| *dur).sum();

    let max_string_length = terminal_width().saturating_sub(20);
    let mut max_duration_width = "Duration".len();

    let mut table_rows: Vec<Row<'static>> = Vec::new();
    let mut total_percentage = 0.0;
    let mut total_duration: u64 = 0;

    for (title, duration, title_index, class_index) in rows.iter() {
        total_duration += *duration;

        let percent = percent_of(*duration, total);
        total_percentage += percent;

        // Selection styling (title selection is within the selected class)
        let is_selected = selected_title
            .as_ref()
            .map(|(name, idx)| *idx == *title_index && name == title)
            .unwrap_or(false);

        let class_has_selection = selected_class.is_some();

        let mut title_style = Style::default();
        if class_has_selection {
            title_style = title_style.fg(color_from_index(*title_index));
        } else {
            title_style = title_style.fg(color_from_index(*class_index));
        }
        if is_selected {
            title_style = title_style.add_modifier(Modifier::REVERSED);
        }

        let title_cell = name_cell(title, title_style, max_string_length);

        let dur_cell = {
            let mut s = Style::default();
            if class_has_selection {
                s = s.fg(color_from_index(*title_index));
            } else {
                title_style = title_style.fg(color_from_index(*class_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            duration_cell(*duration, s, &mut max_duration_width)
        };

        let pct_cell = {
            let mut s = Style::default();
            if class_has_selection {
                s = s.fg(color_from_index(*title_index));
            } else {
                title_style = title_style.fg(color_from_index(*class_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            percent_cell(percent, s)
        };

        table_rows.push(Row::new(vec![title_cell, dur_cell, pct_cell]));
    }

    table_rows.push(total_row(
        max_string_length,
        total_duration,
        total_percentage,
    ));

    let widths = table_widths(width, max_duration_width);

    Table::new(table_rows, widths)
        .header(
            Row::new(vec!["Title", "Duration", " Percent"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::BOTTOM | Borders::RIGHT))
        .column_spacing(1)
}
