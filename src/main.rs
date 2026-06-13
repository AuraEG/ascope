// ==========================================================================
// File    : main.rs
// Project : AuraScope
// Layer   : Entry
// Purpose : CLI entrypoint; dispatches to TUI navigation mode or the
//           non-interactive stats-only output path.
//
// Author  : Ahmed Ashour
// Created : 2026-06-13
// ==========================================================================

mod app;
mod fs;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

// --------------------------------------------------------------------------
// [SECTION] CLI Arguments
// --------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(author, version, about = "Blazingly fast terminal workspace inspector")]
struct Args {
    /// Root path to inspect (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Print disk-usage statistics without launching the TUI
    #[arg(long)]
    stats: bool,

    /// Emit statistics as JSON (requires --stats)
    #[arg(long)]
    json: bool,
}

// --------------------------------------------------------------------------
// [SECTION] TUI Bootstrap
// --------------------------------------------------------------------------

/// Configure the terminal for raw TUI mode, run the event loop, then
/// unconditionally restore the terminal before returning to the caller.
fn run_tui(root: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, root);

    // Restore the terminal regardless of how the event loop exited.
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

// --------------------------------------------------------------------------
// [SECTION] Event Loop
// --------------------------------------------------------------------------

/// Drive the render → poll → handle cycle until the user presses q or Escape.
fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    root: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state = app::AppState::new(root);

    loop {
        terminal.draw(|f| ui::widgets::render_dashboard(f, &state))?;

        // Short timeout keeps the loop responsive without burning CPU.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => state.move_selection(1),
                    KeyCode::Up | KeyCode::Char('k') => state.move_selection(-1),
                    KeyCode::Enter => state.navigate_in(),
                    KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                        state.navigate_out();
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

// --------------------------------------------------------------------------
// [SECTION] Entry
// --------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    if args.stats {
        match fs::walker::scan_path(&args.path) {
            Ok(stats) => {
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&stats).unwrap());
                } else {
                    println!("Scan Path : {:?}", args.path);
                    println!("Total Size: {} bytes", stats.total_size);
                    println!("File Count: {}", stats.file_count);
                }
            }
            Err(e) => eprintln!("[x] Scan error: {e}"),
        }
    } else if let Err(e) = run_tui(args.path) {
        eprintln!("[x] TUI error: {e}");
    }
}
