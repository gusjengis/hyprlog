use std::time::{Duration, Instant};

use color_eyre::eyre::Context;
use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};

use crate::log_reader::LogReader;
use crate::model::{Log, Model};
use crate::model_building::{build_model, filter_class};
use crate::stream_client::{StreamClient, StreamEvent};
use crate::view::{
    build_class_table, build_title_table, format_short_duration, header, render_log,
};
use crate::Settings;

pub struct App {
    settings: Settings,
    model: Model,
    should_quit: bool,
    selected_class: Option<(String, usize)>,
    selected_title: Option<(String, usize)>,
    follow: bool,
    last_frame_end: Option<Instant>,
    update_time: Duration,
    timeline_time: Duration,
    classes_time: Duration,
    titles_time: Duration,
    stream_client: Option<StreamClient>,
    next_expected_seq: u64,
    pending_logs: Vec<Log>,
    needs_full_rebuild: bool,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        let stream_client = StreamClient::connect().ok();
        let mut app = Self {
            settings,
            model: Model::new(),
            should_quit: false,
            selected_class: None,
            selected_title: None,
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
                    StreamEvent::Gap { expected, received } => {
                        app.needs_full_rebuild = true;
                        app.next_expected_seq = received + 1;
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

    let _ = update(&mut app, true);
    ratatui::run(|terminal| run(terminal, &mut app)).context("failed to run app")
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let mut force_render = true;
    loop {
        let had_events = process_stream_events(app);
        if had_events {
            force_render = true;
        }

        terminal.draw(|f| render(f, app))?;

        if !force_render {
            event_loop(app)?;
        }
        if app.should_quit {
            break;
        }

        if force_render {
            force_render = false;
        }

        app.last_frame_end = Some(Instant::now());

        let update_start = Instant::now();
        update(app, force_render);
        app.update_time = update_start.elapsed();
    }
    Ok(())
}

fn process_stream_events(app: &mut App) -> bool {
    let mut had_events = false;
    if let Some(ref mut client) = app.stream_client {
        while let Ok(event) = client.try_recv() {
            had_events = true;
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
    had_events
}

fn update(app: &mut App, force_render: bool) {
    if app.needs_full_rebuild || force_render {
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
            app.model.add_log(log);
        }
        app.model.sort();
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
                    *title = class_struct.titles[*index].title.clone();
                }
            }
        } else {
            if *index >= app.model.classes.len() {
                *index = app.model.classes.len() - 1;
            }
            *class = app.model.classes[*index].class.clone();
            app.selected_title = None;
        }
    }
    if app.follow {
        let newest_log = app.model.logs.last().unwrap();
        let class_index = app.model.index_of(&newest_log.class).unwrap();
        app.selected_class = Some((newest_log.class.clone(), class_index));
        app.selected_title = Some((
            newest_log.title.clone(),
            app.model.classes[class_index]
                .index_of(&newest_log.title)
                .unwrap(),
        ));
    }

    if let Some((class, _)) = &app.selected_class {
        app.settings.class_arg = class.clone();
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    let timeline_start = Instant::now();
    let Some(timelines_text) = render_log(&app.model, &app.settings) else {
        let msg = Paragraph::new("No log data for this interval. (press 'q' to quit)");
        frame.render_widget(msg, frame.area());
        return;
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

    frame.render_widget(
        Paragraph::new(timelines_text).wrap(Wrap { trim: false }),
        chunks[0],
    );

    frame.render_widget(Paragraph::new(header(&app.settings)), chunks[1]);

    let table_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(2),
            Constraint::Fill(1),
        ])
        .split(chunks[2]);

    let classes_start = Instant::now();
    let class_table = build_class_table(&app.model, &app.selected_class, table_cols[0].width);
    app.classes_time = classes_start.elapsed();
    let titles_start = Instant::now();
    let title_table = build_title_table(&app.model, &app.selected_class, &app.selected_title);
    app.titles_time = titles_start.elapsed();
    frame.render_widget(class_table, table_cols[0]);
    draw_inner_border(frame, table_cols[1], Style::default());
    frame.render_widget(title_table, table_cols[2]);

    frame.render_widget(Paragraph::new(footer_line(app)), chunks[3]);
}

fn event_loop(app: &mut App) -> Result<()> {
    if event::poll(Duration::from_millis(250)).context("event poll failed")? {
        if let event::Event::Key(key) = event::read().context("event read failed")? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char('m') => {
                        app.settings.multi_timeline = !app.settings.multi_timeline
                    }
                    KeyCode::Char('f') => {
                        app.follow = !app.follow;
                        app.selected_title = None;
                    }
                    KeyCode::Up => {
                        if let None = app.selected_class.as_ref() {
                            app.selected_class = Some((app.model.classes[0].class.clone(), 0));
                        } else if let Some((class, index)) = app.selected_class.as_mut() {
                            if let Some((title, index)) = app.selected_title.as_mut() {
                                if *index > 0 {
                                    *index = (*index - 1);
                                    *title = app.model.get_class_mut(class.clone()).titles[*index]
                                        .title
                                        .clone();
                                }
                            } else if *index > 0 {
                                *index = (*index - 1);
                                *class = app.model.classes[*index].class.clone();
                            }
                        }
                    }
                    KeyCode::Down => {
                        if let None = app.selected_class.as_ref() {
                            app.selected_class = Some((app.model.classes[0].class.clone(), 0));
                        } else if let Some((class, index)) = app.selected_class.as_mut() {
                            if let Some((title, index)) = app.selected_title.as_mut() {
                                *index = (*index + 1)
                                    .min(app.model.get_class_mut(class.clone()).titles.len() - 1);
                                *title = app.model.get_class_mut(class.clone()).titles[*index]
                                    .title
                                    .clone();
                            } else {
                                *index = (*index + 1).min(app.model.classes.len() - 1);
                                *class = app.model.classes[*index].class.clone();
                            }
                        }
                    }
                    KeyCode::Esc => {
                        if app.selected_title.is_some() {
                            app.selected_title = None;
                        } else if app.selected_class.is_some() {
                            app.selected_class = None;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
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

    Line::from(vec![
        key_span("q"),
        Span::raw("quit  •  "),
        key_span("m"),
        toggle_span("multi-timeline", app.settings.multi_timeline),
        Span::raw("  •  "),
        key_span("f"),
        toggle_span("follow", app.follow),
        Span::raw("  •  "),
        key_span("↑/↓"),
        Span::raw("move  •  "),
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
    ])
}
