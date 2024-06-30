use std::io::{self, stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode, KeyModifiers},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};

pub enum CliMessage {
    Quit,
}

pub fn run_cli(
    running: Arc<AtomicBool>,
    _tx: Sender<CliMessage>,
    _rx: Receiver<CliMessage>,
) -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut should_quit = false;
    while !should_quit && running.load(Ordering::Relaxed) {
        terminal.draw(|frame| ui(frame))?;
        should_quit = handle_events()?;
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn handle_events() -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            match key {
                event::KeyEvent {
                    code: KeyCode::Char('q'),
                    modifiers: KeyModifiers::NONE,
                    kind: event::KeyEventKind::Press,
                    ..
                }
                | event::KeyEvent {
                    code: KeyCode::Esc,
                    kind: event::KeyEventKind::Press,
                    ..
                }
                | event::KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: event::KeyEventKind::Press,
                    ..
                } => return Ok(true),
                _ => {}
            }
        }
    }
    Ok(false)
}

fn ui<'a>(frame: &mut Frame<'a>) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
        .split(frame.size());

    let top_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
        .split(main_layout[0]);

    let left_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(top_layout[0]);

    let bottom_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
        .split(main_layout[1]);

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("导入歌曲"),
        left_layout[0],
    );

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("收藏夹"),
        top_layout[1],
    );

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("播放列表"),
        left_layout[1],
    );

    let play_status = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ]
            .as_ref(),
        )
        .split(bottom_layout[0]);

    let song_info = Paragraph::new(Line::from("当前歌曲: 示例歌曲 - 示例艺术家"))
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center);
    frame.render_widget(song_info, play_status[0]);

    let controls = Paragraph::new(Line::from(vec![
        Span::styled("⏮ ", Style::default().fg(Color::White)),
        Span::styled("⏯ ", Style::default().fg(Color::White)),
        Span::styled("⏭", Style::default().fg(Color::White)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(controls, play_status[3]);

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("播放状态"),
        bottom_layout[0],
    );

    frame.render_widget(
        Block::default().borders(Borders::ALL).title("相关歌曲"),
        bottom_layout[1],
    );
}
