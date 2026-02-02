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
            Cell::from(format_duration(*duration)).style(s)
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

        let class_has_selection = selected_class.is_some();

        let mut title_style = Style::default();
        if class_has_selection {
            title_style = title_style.fg(color_from_index(*title_index));
        }
        if is_selected {
            title_style = title_style.add_modifier(Modifier::REVERSED);
        }

        let title_cell = Cell::from(truncate_string(title, max_string_length)).style(title_style);

        let dur_cell = {
            let mut s = Style::default();
            if class_has_selection {
                s = s.fg(color_from_index(*title_index));
            }
            if is_selected {
                s = s.add_modifier(Modifier::REVERSED);
            }
            Cell::from(format_duration(*duration)).style(s)
        };

        let pct_cell = {
            let mut s = Style::default();
            if class_has_selection {
                s = s.fg(color_from_index(*title_index));
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
