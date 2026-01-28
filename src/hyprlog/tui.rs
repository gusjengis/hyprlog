use std::time::Duration;

use color_eyre::eyre::Context;
use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};

use crate::view::{header, render_log};
use crate::Settings;

pub struct App {
    settings: Settings,
    should_quit: bool,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            should_quit: false,
        }
    }
}

pub fn start_tui(settings: Settings) -> Result<()> {
    color_eyre::install()?;
    let mut app = App::new(settings);
    ratatui::run(|terminal| run(terminal, &mut app)).context("failed to run app")
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, app))?;
        event_loop(app)?;
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn render(frame: &mut Frame, app: &App) {
    let Some((timelines_text, table_left, _)) = render_log(&app.settings) else {
        let msg = Paragraph::new("No log data for this interval. (press 'q' to quit)");
        frame.render_widget(msg, frame.area());
        return;
    };

    // Build a second table (duplicate for now)
    let Some((_timelines_text2, _, table_right)) = render_log(&app.settings) else {
        // If this somehow fails while the first succeeded, just don't draw the second
        let msg = Paragraph::new("No log data for this interval. (press 'q' to quit)");
        frame.render_widget(msg, frame.area());
        return;
    };

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

    // (you said forget header, but leaving your line as-is)
    frame.render_widget(Paragraph::new(header(&app.settings)), chunks[1]);

    // Split the table area into left/right
    let table_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    frame.render_widget(table_left, table_cols[0]);
    frame.render_widget(table_right, table_cols[1]);

    frame.render_widget(
        Paragraph::new("q: quit  •  m: toggle multi-timeline"),
        chunks[3],
    );
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
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
