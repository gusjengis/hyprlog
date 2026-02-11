use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use color_eyre::eyre::Context;
use color_eyre::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};

use crate::input::event_loop;
use crate::interval::Interval;
use crate::interval_change::handle_interval_change;
use crate::log_reader::LogReader;
use crate::model::{Log, Model};
use crate::model_building::{build_model, filter_class};
use crate::stream_client::{StreamClient, StreamEvent};
use crate::tables::{build_class_table, build_title_table};
use crate::view::{format_short_duration, header, render_log};
use crate::Settings;

pub struct App {
    pub settings: Settings,
    pub model: Model,
    pub(crate) should_quit: bool,
    pub(crate) selected_class: Option<(String, usize)>,
    pub(crate) selected_title: Option<(String, usize)>,
    pub(crate) class_scroll: usize,
    pub(crate) title_scroll: usize,
    pub(crate) follow: bool,
    last_frame_end: Option<Instant>,
    update_time: Duration,
    timeline_time: Duration,
    classes_time: Duration,
    titles_time: Duration,
    stream_client: Option<StreamClient>,
    next_expected_seq: u64,
    pending_logs: Vec<Log>,
    needs_full_rebuild: bool,
    pub(crate) force_render: std::sync::Arc<AtomicBool>,
    pub(crate) timeline_area: Rect,
    pub(crate) drag_state: Option<DragState>,
}

pub(crate) struct DragState {
    pub(crate) start_col: u16,
    pub(crate) start_interval: Interval,
    pub(crate) area_width: u16,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        let stream_client = StreamClient::connect().ok();
        let force_render = stream_client
            .as_ref()
            .map(|c| c.force_render().clone())
            .unwrap_or_else(|| std::sync::Arc::new(AtomicBool::new(false)));

        let mut app = Self {
            settings,
            model: Model::new(),
            should_quit: false,
            selected_class: None,
            selected_title: None,
            class_scroll: 0,
            title_scroll: 0,
            follow: false,
            last_frame_end: None,
            update_time: Duration::ZERO,
            timeline_time: Duration::ZERO,
            classes_time: Duration::ZERO,
            titles_time: Duration::ZERO,
            stream_client,
            next_expected_seq: 0,
            pending_logs: Vec::new(),
            needs_full_rebuild: false,
            force_render,
            timeline_area: Rect::default(),
            drag_state: None,
        };

        if let Some(ref mut client) = app.stream_client {
            while let Ok(event) = client.try_recv() {
                match event {
                    StreamEvent::Welcome { current_seq } => {
                        app.next_expected_seq = current_seq;
                    }
                    StreamEvent::Log {
                        seq,
                        timestamp,
                        class,
                        title,
                    } => {
                        if seq == app.next_expected_seq {
                            app.next_expected_seq += 1;
                            app.pending_logs.push(Log::new(
                                timestamp as u64,
                                None,
                                filter_class(class, &app.settings),
                                title,
                            ));
                        } else if seq > app.next_expected_seq {
                            app.needs_full_rebuild = true;
                            app.next_expected_seq = seq + 1;
                        }
                    }
                    StreamEvent::Gap { .. } => {
                        app.needs_full_rebuild = true;
                    }
                    StreamEvent::Disconnected => {
                        app.needs_full_rebuild = true;
                    }
                }
            }
        }

        app
    }
}

pub fn start_tui(settings: Settings) -> Result<()> {
    color_eyre::install()?;
    let mut app = App::new(settings);

    let mut reader = LogReader::new(&app.settings);

    if !reader.is_empty() {
        build_model(&mut app.model, &mut reader, &app.settings).unwrap();
    }

    let _ = update(&mut app);
    crossterm::execute!(stdout(), EnableMouseCapture).context("failed to enable mouse capture")?;
    let result = ratatui::run(|terminal| run(terminal, &mut app)).context("failed to run app");
    let _ = crossterm::execute!(stdout(), DisableMouseCapture);
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, app))?;

        event_loop(app)?;

        if app.should_quit {
            break;
        }

        app.force_render.store(false, Ordering::SeqCst);

        app.last_frame_end = Some(Instant::now());

        process_stream_events(app);
        let update_start = Instant::now();
        update(app);
        app.update_time = update_start.elapsed();
    }
    Ok(())
}

fn process_stream_events(app: &mut App) {
    if let Some(ref mut client) = app.stream_client {
        while let Ok(event) = client.try_recv() {
            match event {
                StreamEvent::Welcome { current_seq } => {
                    app.next_expected_seq = current_seq;
                }
                StreamEvent::Log {
                    seq,
                    timestamp,
                    class,
                    title,
                } => {
                    if seq == app.next_expected_seq {
                        app.next_expected_seq += 1;
                        app.pending_logs.push(Log::new(
                            timestamp as u64,
                            None,
                            filter_class(class, &app.settings),
                            title,
                        ));
                    } else if seq > app.next_expected_seq {
                        app.needs_full_rebuild = true;
                        app.next_expected_seq = seq + 1;
                    }
                }
                StreamEvent::Gap { .. } => {
                    app.needs_full_rebuild = true;
                }
                StreamEvent::Disconnected => {
                    app.needs_full_rebuild = true;
                }
            }
        }
    }
}

fn update(app: &mut App) {
    if app.settings.focused_interval.changed {
        handle_interval_change(app);
        app.settings.focused_interval.changed = false;
    }
    if app.needs_full_rebuild {
        app.model = Model::new();
        build_model(
            &mut app.model,
            &mut LogReader::new(&app.settings),
            &app.settings,
        )
        .unwrap();
        app.pending_logs.clear();
        app.needs_full_rebuild = false;
    } else if !app.pending_logs.is_empty() {
        for log in app.pending_logs.drain(..) {
            app.model.add_log(log, false);
        }
    }

    if !app.model.logs.is_empty() {
        app.model.maintain_order(vec![app.model.logs.len() - 1]);
    }

    if let Some((class, index)) = app.selected_class.as_mut() {
        if let Some(class_index) = app.model.index_of(&class) {
            if class_index != *index {
                *index = class_index;
            }
            if let Some((title, index)) = app.selected_title.as_mut() {
                if let Some(title_index) = app.model.index_of(&title) {
                    if title_index != *index {
                        *index = title_index;
                    }
                } else {
                    let class_struct = &app.model.get_class_mut(class.clone());
                    if *index >= class_struct.titles.len() {
                        *index = class_struct.titles.len() - 1;
                    }

                    if !class_struct.titles.is_empty() {
                        *title = class_struct.titles[*index].title.clone();
                    } else {
                        app.selected_title = None;
                    }
                }
            }
        } else {
            if *index >= app.model.classes.len() {
                *index = app.model.classes.len() - 1;
            }
            if !app.model.classes.is_empty() {
                *class = app.model.classes[*index].class.clone();
            }
            app.selected_title = None;
        }
    }
    if app.follow {
        if now_is_visible(app) {
            let newest_log = app.model.logs.last().unwrap();
            let class_index = app.model.index_of(&newest_log.class).unwrap();
            app.selected_class = Some((newest_log.class.clone(), class_index));
            app.selected_title = Some((
                newest_log.title.clone(),
                app.model.classes[class_index]
                    .index_of(&newest_log.title)
                    .unwrap(),
            ));
        } else {
            set_follow(app, false);
        }
    }

    if let Some((class, _)) = &app.selected_class {
        app.settings.class_arg = class.clone();
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    let timeline_start = Instant::now();
    let (timelines_text, center_timeline) = match render_log(&app.model, &app.settings) {
        Ok(text) => (text, false),
        Err(message) => {
            let empty_text: Text<'static> = Text::from(Line::from(message));
            (empty_text, true)
        }
    };
    app.timeline_time = timeline_start.elapsed();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(timelines_text.height() as u16),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    app.timeline_area = chunks[0];

    let mut timeline_paragraph = Paragraph::new(timelines_text).wrap(Wrap { trim: false });
    if center_timeline {
        timeline_paragraph = timeline_paragraph.alignment(Alignment::Center);
    }
    frame.render_widget(timeline_paragraph, chunks[0]);

    frame.render_widget(Paragraph::new(header(&app.settings)), chunks[1]);

    let table_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(2),
            Constraint::Fill(1),
        ])
        .split(chunks[2]);

    // Table layout:
    // - 1 row for header
    // - 1 row for bottom border (both tables draw Borders::BOTTOM)
    // - 1 pinned total row
    // Everything else is scrollable body.
    let class_body_capacity = table_cols[0].height.saturating_sub(3) as usize;
    let title_body_capacity = table_cols[2].height.saturating_sub(3) as usize;

    adjust_scroll(
        &mut app.class_scroll,
        app.selected_class.as_ref().map(|(_, idx)| *idx),
        app.model.classes.len(),
        class_body_capacity,
    );

    if app.selected_class.is_none() {
        app.model.ensure_titles_sorted();
    }

    let (title_len, title_selected) = match app.selected_class.as_ref() {
        Some((_, class_idx)) => app
            .model
            .classes
            .get(*class_idx)
            .map(|c| {
                (
                    c.titles.len(),
                    app.selected_title.as_ref().map(|(_, idx)| *idx),
                )
            })
            .unwrap_or((0, None)),
        None => (app.model.titles.len(), None),
    };
    adjust_scroll(
        &mut app.title_scroll,
        title_selected,
        title_len,
        title_body_capacity,
    );

    let classes_start = Instant::now();
    let class_table = build_class_table(
        &app.model,
        &app.selected_class,
        table_cols[0].width,
        app.class_scroll,
        class_body_capacity,
    );
    app.classes_time = classes_start.elapsed();
    let titles_start = Instant::now();
    let title_table = build_title_table(
        &app.model,
        &app.selected_class,
        &app.selected_title,
        table_cols[2].width,
        app.title_scroll,
        title_body_capacity,
    );
    app.titles_time = titles_start.elapsed();
    frame.render_widget(class_table, table_cols[0]);
    draw_inner_border(frame, table_cols[1], Style::default());
    frame.render_widget(title_table, table_cols[2]);

    frame.render_widget(Paragraph::new(footer_line(app)), chunks[3]);
}

fn adjust_scroll(scroll: &mut usize, selected: Option<usize>, len: usize, body_capacity: usize) {
    if len == 0 || body_capacity == 0 {
        *scroll = 0;
        return;
    }

    *scroll = (*scroll).min(len - 1);
    let Some(sel) = selected.map(|s| s.min(len - 1)) else {
        return;
    };

    // Keep `sel` within the real data rows (excluding ellipsis rows).
    for _ in 0..4 {
        if sel < *scroll {
            *scroll = sel;
            continue;
        }

        let top_ellipsis = *scroll > 0;
        let mut slots = body_capacity;
        if top_ellipsis {
            if slots == 0 {
                return;
            }
            slots -= 1;
        }

        let remaining = len.saturating_sub(*scroll);
        let real = if remaining <= slots {
            remaining
        } else if slots == 0 {
            0
        } else {
            // Reserve one slot for the bottom ellipsis row.
            slots - 1
        };

        if real == 0 {
            return;
        }

        if sel >= *scroll + real {
            *scroll = sel + 1 - real;
            *scroll = (*scroll).min(len - 1);
            continue;
        }

        break;
    }
}

fn draw_inner_border(frame: &mut Frame, area: Rect, style: Style) {
    let x0 = area.x;
    let x1 = area.x + 1;

    let bottom_y = area.y + area.height.saturating_sub(1);

    for y in area.y..bottom_y {
        frame.buffer_mut().set_string(x0, y, "▕", style);
        frame.buffer_mut().set_string(x1, y, "▏", style);
    }

    if area.height > 0 {
        frame.buffer_mut().set_string(x0, bottom_y, "─", style);
        frame.buffer_mut().set_string(x1, bottom_y, "─", style);
    }
}

fn toggle_span(label: &str, on: bool) -> Span<'static> {
    let base = Style::default().add_modifier(Modifier::BOLD);
    let on_style = base.add_modifier(Modifier::REVERSED);
    Span::styled(format!(" {} ", label), if on { on_style } else { base })
}

fn key_span(key: &str) -> Span<'static> {
    Span::styled(
        format!(" {} ", key),
        Style::default().add_modifier(Modifier::BOLD),
    )
}

fn footer_line(app: &App) -> Line<'static> {
    let frame_time = app.last_frame_end.map_or_else(
        || "—".to_string(),
        |end| format_short_duration(end.elapsed()),
    );

    let mut spans = vec![
        key_span("q"),
        Span::raw("quit  •  "),
        key_span("m"),
        toggle_span("multi-timeline", app.settings.multi_timeline),
        Span::raw("  •  "),
    ];

    if now_is_visible(app) {
        spans.push(key_span("f"));
        spans.push(toggle_span("follow", app.follow));
        spans.push(Span::raw("  •  "));
    }

    spans.extend([
        key_span("↑/↓"),
        Span::raw("move  •  "),
        key_span("+/-"),
        Span::raw("zoom  •  "),
        key_span("esc"),
        Span::raw("back  •  "),
        Span::styled(
            format!("ft: {}", frame_time),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw("  •  "),
        Span::styled(
            format!("update: {}", format_short_duration(app.update_time)),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw("  •  "),
        Span::styled(
            format!("timeline: {}", format_short_duration(app.timeline_time)),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw("  •  "),
        Span::styled(
            format!("classes: {}", format_short_duration(app.classes_time)),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw("  •  "),
        Span::styled(
            format!("titles: {}", format_short_duration(app.titles_time)),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ]);

    Line::from(spans)
}

pub(crate) fn set_follow(app: &mut App, follow: bool) {
    app.follow = follow;
    app.selected_title = None;
}

fn now_is_visible(app: &App) -> bool {
    app.settings
        .focused_interval
        .contains_utc_timestamp_millis(Utc::now().timestamp_millis() as u64)
}
