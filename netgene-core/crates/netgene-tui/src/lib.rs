//! # NetGene TUI
//!
//! Interactive terminal dashboard built with ratatui + crossterm.

pub mod app;
pub mod ui;
pub mod events;

pub use app::App;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

/// Launch the TUI dashboard.
pub async fn run_tui() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new().await;

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        // Handle input with 100ms tick
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Tab, _) => app.next_tab(),
                    (KeyCode::BackTab, _) => app.prev_tab(),
                    (KeyCode::Char('r'), _) => app.refresh().await,
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.scroll_down(),
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.scroll_up(),
                    _ => {}
                }
            }
        }

        // Tick update
        app.tick().await;
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
