use std::time::Duration;

use color_eyre::eyre::Context;
use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};

use crate::view::{header, render_log};
use crate::{view, Settings};

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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // header = 1 row (or 2 if you want)
            Constraint::Min(0),    // rest for timeline
        ])
        .split(frame.area());

    // Header
    let header = Paragraph::new(header(&app.settings));
    frame.render_widget(header, chunks[0]);

    // Timeline string (whatever you generated)
    let timeline_text = render_log(&app.settings).unwrap();

    // Render timeline
    let timeline = Paragraph::new(timeline_text).wrap(Wrap { trim: false });
    frame.render_widget(timeline, chunks[1]);
}

fn event_loop(app: &mut App) -> Result<()> {
    if event::poll(Duration::from_millis(250)).context("event poll failed")? {
        if let event::Event::Key(key) = event::read().context("event read failed")? {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                app.should_quit = true;
            }
        }
    }
    Ok(())
}
