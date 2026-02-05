use ratatui::{
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::{
    model::{Class, Model},
    view::{color_from_index, format_duration, terminal_width, truncate_string, CUTOFF},
};

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

    for (count, (class, duration, class_index)) in rows.iter().take(CUTOFF).enumerate() {
        let _ = count; // (remove if unused later)
        total_duration += *duration;

        let percent = if total == 0 {
            0.0
        } else {
            100.0 * (*duration as f64 / total as f64)
        };
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

        let class_cell = Cell::from(truncate_string(class, max_string_length)).style(class_style);

        let dur_cell = {
            let mut s = Style::default();
            if !class_has_selection {
                s = s.fg(color_from_index(*class_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            let duration_string = format_duration(*duration);
            if duration_string.len() > max_duration_width {
                max_duration_width = duration_string.len();
            }

            Cell::from(duration_string).style(s)
        };

        let pct_cell = {
            let mut s = Style::default();
            if !class_has_selection {
                s = s.fg(color_from_index(*class_index));
            }
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
        Constraint::Length(width as u16 - (max_duration_width + 9) as u16),
        Constraint::Length(max_duration_width as u16),
        Constraint::Length(9),
    ];

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
        // None => model
        //     .classes
        //     .iter()
        //     .enumerate()
        //     .flat_map(|(class_idx, class)| {
        //         class
        //             .titles
        //             .iter()
        //             .enumerate()
        //             .map(move |(title_idx, title)| {
        //                 (
        //                     title.title.as_str(),
        //                     title.total_duration(&model.logs),
        //                     title_idx,
        //                     class_idx,
        //                 )
        //             })
        //     })
        //     .collect(),
        None => model
            .titles
            .iter()
            .map(|(class_idx, title_idx)| {
                (
                    model.classes[*class_idx].titles[*title_idx].title.as_str(),
                    model.classes[*class_idx].titles[*title_idx].total_duration(&model.logs),
                    *title_idx,
                    *class_idx,
                )
            })
            .collect(),
    };

    // rows.sort_by(|a, b| b.1.cmp(&a.1));
    // let rows = rows.into_iter().take(CUTOFF).collect::<Vec<_>>();

    let total: u64 = rows.iter().map(|(_, dur, _, _)| *dur).sum();

    let mut max_title_width = rows
        .iter()
        .map(|(title, _, _, _)| title.len())
        .max()
        .unwrap_or(0);

    let max_string_length = terminal_width().saturating_sub(20);
    max_title_width = max_title_width.min(max_string_length);
    let mut max_duration_width = "Duration".len();

    let mut table_rows: Vec<Row<'static>> = Vec::new();
    let mut total_percentage = 0.0;
    let mut total_duration: u64 = 0;

    for (title, duration, title_index, class_index) in rows.iter() {
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

        let title_cell = Cell::from(truncate_string(title, max_string_length)).style(title_style);

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
            let duration_string = format_duration(*duration);
            if duration_string.len() > max_duration_width {
                max_duration_width = duration_string.len();
            }

            Cell::from(duration_string).style(s)
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
        Constraint::Length(width as u16 - (max_duration_width + 9) as u16),
        Constraint::Length(max_duration_width as u16),
        Constraint::Length(9),
    ];

    Table::new(table_rows, widths)
        .header(
            Row::new(vec!["Title", "Duration", " Percent"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::BOTTOM | Borders::RIGHT))
        .column_spacing(1)
}
