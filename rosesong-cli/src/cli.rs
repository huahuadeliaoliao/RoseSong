use std::env;
use std::fs;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use ratatui::crossterm::ExecutableCommand;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode},
        terminal::{disable_raw_mode, enable_raw_mode, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};

use crate::error::ApplicationError;

pub enum CliMessage {
    Quit,
}

enum View {
    Playlists(usize),
    Favorites(usize),
}

impl View {
    fn index_mut(&mut self) -> Option<&mut usize> {
        match self {
            View::Playlists(ref mut index) | View::Favorites(ref mut index) => Some(index),
        }
    }
}

pub fn run_cli(_tx: Sender<CliMessage>, _rx: Receiver<CliMessage>) -> Result<(), ApplicationError> {
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut should_quit = false;
    let mut current_view = View::Playlists(0);

    while !should_quit {
        terminal.draw(|frame| {
            let dir = match &current_view {
                View::Playlists(_) => "playlists",
                View::Favorites(_) => "favorites",
            };
            let content = fetch_directory_content(dir).unwrap_or_else(|_| vec![]);
            if let Err(e) = ui(frame, dir, &current_view, &content) {
                eprintln!("UI error: {}", e);
            }
        })?;
        should_quit = handle_events(&mut current_view)?;
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn handle_events(current_view: &mut View) -> Result<bool, ApplicationError> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            match key {
                event::KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                }
                | event::KeyEvent {
                    code: KeyCode::Esc, ..
                } => return Ok(true),
                event::KeyEvent {
                    code: KeyCode::Char('f'),
                    ..
                } => *current_view = View::Favorites(0),
                event::KeyEvent {
                    code: KeyCode::Char('p'),
                    ..
                } => *current_view = View::Playlists(0),
                event::KeyEvent {
                    code: KeyCode::Char('j'),
                    ..
                } => {
                    if let Some(index) = current_view.index_mut() {
                        *index += 1;
                    }
                }
                event::KeyEvent {
                    code: KeyCode::Char('k'),
                    ..
                } => {
                    if let Some(index) = current_view.index_mut() {
                        if *index > 0 {
                            *index -= 1;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(false)
}

fn ui<'a>(
    frame: &mut Frame<'a>,
    directory: &str,
    view: &View,
    content: &[String],
) -> Result<(), ApplicationError> {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(frame.size());

    let selected_index = match view {
        View::Playlists(index) | View::Favorites(index) => *index,
    };

    let text_lines: Vec<Line> = content
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            if idx == selected_index {
                Line::styled(line.clone(), Style::new().add_modifier(Modifier::REVERSED))
            } else {
                Line::raw(line.clone())
            }
        })
        .collect();

    let paragraph =
        Paragraph::new(text_lines).block(Block::default().borders(Borders::ALL).title(directory));
    frame.render_widget(paragraph, main_layout[0]);

    Ok(())
}

fn fetch_directory_content(dir_name: &str) -> Result<Vec<String>, ApplicationError> {
    let home_dir = env::var("HOME").map_err(|e| {
        ApplicationError::IoError(io::Error::new(io::ErrorKind::NotFound, e).to_string())
    })?;
    let dir_path = PathBuf::from(home_dir)
        .join(".config/rosesong")
        .join(dir_name);

    let entries = fs::read_dir(dir_path)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            // Only include files with a .toml extension
            if path.extension().and_then(std::ffi::OsStr::to_str) == Some("toml") {
                path.file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .map(String::from)
            } else {
                None
            }
        })
        .collect();

    Ok(entries)
}
