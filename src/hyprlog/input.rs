use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use color_eyre::eyre::Context;
use color_eyre::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::tui::{set_follow, App, DragState};

pub fn event_loop(app: &mut App) -> Result<()> {
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_millis(250) {
            return Ok(());
        }
        if app.force_render.load(Ordering::SeqCst) {
            return Ok(());
        }

        if event::poll(Duration::ZERO).context("event poll failed")? {
            match event::read().context("event read failed")? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Char('m') => {
                                app.settings.multi_timeline = !app.settings.multi_timeline
                            }
                            KeyCode::Char('f') => {
                                set_follow(app, !app.follow);
                            }
                            KeyCode::Up => {
                                if let None = app.selected_class.as_ref() {
                                    app.selected_class =
                                        Some((app.model.classes[0].class.clone(), 0));
                                } else if let Some((class, index)) = app.selected_class.as_mut() {
                                    if let Some((title, index)) = app.selected_title.as_mut() {
                                        if *index > 0 {
                                            *index = *index - 1;
                                            *title = app.model.get_class_mut(class.clone()).titles
                                                [*index]
                                                .title
                                                .clone();
                                        }
                                    } else if *index > 0 {
                                        *index = *index - 1;
                                        *class = app.model.classes[*index].class.clone();
                                    }
                                }
                            }
                            KeyCode::Down => {
                                if let None = app.selected_class.as_ref() {
                                    app.selected_class =
                                        Some((app.model.classes[0].class.clone(), 0));
                                } else if let Some((class, index)) = app.selected_class.as_mut() {
                                    if let Some((title, index)) = app.selected_title.as_mut() {
                                        *index = (*index + 1).min(
                                            app.model.get_class_mut(class.clone()).titles.len() - 1,
                                        );
                                        *title = app.model.get_class_mut(class.clone()).titles
                                            [*index]
                                            .title
                                            .clone();
                                    } else {
                                        *index = (*index + 1).min(app.model.classes.len() - 1);
                                        *class = app.model.classes[*index].class.clone();
                                    }
                                }
                            }
                            KeyCode::Left => {
                                app.settings.focused_interval.pan_days(1, false);
                            }
                            KeyCode::Right => {
                                app.settings.focused_interval.pan_days(1, true);
                            }
                            KeyCode::Esc => {
                                if app.selected_title.is_some() {
                                    app.selected_title = None;
                                    app.title_scroll = 0;
                                } else if app.selected_class.is_some() {
                                    app.selected_class = None;
                                    app.settings.class_arg = String::from("");
                                    app.class_scroll = 0;
                                    app.title_scroll = 0;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => handle_mouse_event(app, mouse),
                _ => {}
            }
            return Ok(());
        }
    }
}

fn handle_mouse_event(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if timeline_contains(app.timeline_area, mouse.column, mouse.row) {
                app.drag_state = Some(DragState {
                    start_col: mouse.column,
                    start_interval: app.settings.focused_interval.clone(),
                    area_width: app.timeline_area.width,
                });
            } else {
                app.drag_state = None;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(state) = app.drag_state.as_ref() {
                if state.area_width > 0 {
                    let delta_cols = mouse.column as i64 - state.start_col as i64;
                    let interval_ms = state.start_interval.width() as i64;
                    let ms_per_column = interval_ms / state.area_width as i64;
                    if ms_per_column != 0 {
                        let delta_ms = -delta_cols * ms_per_column;
                        let mut next_interval = state.start_interval.clone();
                        next_interval.pan_millis(delta_ms);
                        app.settings.focused_interval = next_interval;
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.drag_state = None;
        }
        _ => {}
    }
}

fn timeline_contains(area: Rect, column: u16, row: u16) -> bool {
    if area.width == 0 || area.height == 0 {
        return false;
    }
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}
