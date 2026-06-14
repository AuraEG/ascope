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
mod config;
mod fs;
mod git;
mod shell;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
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

    /// Write the final selected directory to this file before exit.
    #[arg(long)]
    export_target: Option<PathBuf>,
}

// --------------------------------------------------------------------------
// [SECTION] TUI Bootstrap
// --------------------------------------------------------------------------

/// Configure the terminal for raw TUI mode, run the event loop, then
/// unconditionally restore the terminal before returning to the caller.
fn run_tui(root: PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut state = app::AppState::new(root);
    // Configurable tick rate of 16ms (~60fps) for smooth animations and updates.
    let events = ui::event::EventHandler::new(Duration::from_millis(16));

    loop {
        state.update_preview_cache();
        terminal.draw(|f| ui::widgets::render_dashboard(f, &state))?;

        match events.next()? {
            ui::event::AppEvent::Tick => {
                state.poll_scan();
                if let Some((_, timestamp)) = &state.notification {
                    if timestamp.elapsed() >= Duration::from_secs(3) {
                        state.notification = None;
                    }
                }
            }
            ui::event::AppEvent::Key(key) => {
                if state.rename_mode {
                    match key.code {
                        KeyCode::Esc => {
                            state.rename_mode = false;
                            state.rename_input.clear();
                        }
                        KeyCode::Enter => {
                            state.confirm_rename();
                        }
                        KeyCode::Backspace => {
                            state.rename_input.pop();
                        }
                        KeyCode::Char(c) => {
                            state.rename_input.push(c);
                        }
                        _ => {}
                    }
                } else if state.modal_mode != crate::app::ModalMode::None {
                    if state.modal_mode == crate::app::ModalMode::DeleteConfirmation {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                                state.modal_mode = crate::app::ModalMode::None;
                                state.delete_targets.clear();
                            }
                            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                                state.confirm_delete();
                            }
                            _ => {}
                        }
                    } else if state.modal_mode == crate::app::ModalMode::OpenConfirmation {
                        match key.code {
                            KeyCode::Esc => {
                                state.modal_mode = state.modal_confirm_prev;
                                state.modal_target_path = None;
                            }
                            KeyCode::Left
                            | KeyCode::Right
                            | KeyCode::Char('h')
                            | KeyCode::Char('l')
                            | KeyCode::Tab => {
                                state.modal_confirm_new_tab = !state.modal_confirm_new_tab;
                            }
                            KeyCode::Char('s') => {
                                if let Some(path) = state.modal_target_path.take() {
                                    state.jump_to_path(path);
                                }
                                state.modal_mode = crate::app::ModalMode::None;
                                state.modal_input.clear();
                            }
                            KeyCode::Char('n') => {
                                if let Some(path) = state.modal_target_path.take() {
                                    state.open_tab(path);
                                }
                                state.modal_mode = crate::app::ModalMode::None;
                                state.modal_input.clear();
                            }
                            KeyCode::Enter => {
                                if let Some(path) = state.modal_target_path.take() {
                                    if state.modal_confirm_new_tab {
                                        state.open_tab(path);
                                    } else {
                                        state.jump_to_path(path);
                                    }
                                }
                                state.modal_mode = crate::app::ModalMode::None;
                                state.modal_input.clear();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                state.modal_mode = crate::app::ModalMode::None;
                                state.modal_input.clear();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let len = match state.modal_mode {
                                    crate::app::ModalMode::Bookmarks => {
                                        state.config.bookmarks.len()
                                    }
                                    crate::app::ModalMode::Recent => state.config.recent.len(),
                                    _ => 0,
                                };
                                if len > 0 {
                                    state.modal_selected_index =
                                        (state.modal_selected_index + len - 1) % len;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let len = match state.modal_mode {
                                    crate::app::ModalMode::Bookmarks => {
                                        state.config.bookmarks.len()
                                    }
                                    crate::app::ModalMode::Recent => state.config.recent.len(),
                                    _ => 0,
                                };
                                if len > 0 {
                                    state.modal_selected_index =
                                        (state.modal_selected_index + 1) % len;
                                }
                            }
                            KeyCode::Enter => {
                                let target_path = if !state.modal_input.is_empty() {
                                    if let Ok(idx) = state.modal_input.parse::<usize>() {
                                        let idx = idx.saturating_sub(1);
                                        match state.modal_mode {
                                            crate::app::ModalMode::Bookmarks => {
                                                state.config.bookmarks.get(idx).cloned()
                                            }
                                            crate::app::ModalMode::Recent => {
                                                state.config.recent.get(idx).cloned()
                                            }
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    match state.modal_mode {
                                        crate::app::ModalMode::Bookmarks => state
                                            .config
                                            .bookmarks
                                            .get(state.modal_selected_index)
                                            .cloned(),
                                        crate::app::ModalMode::Recent => state
                                            .config
                                            .recent
                                            .get(state.modal_selected_index)
                                            .cloned(),
                                        _ => None,
                                    }
                                };

                                if let Some(path) = target_path {
                                    state.modal_target_path = Some(path);
                                    state.modal_confirm_prev = state.modal_mode;
                                    state.modal_mode = crate::app::ModalMode::OpenConfirmation;
                                    state.modal_confirm_new_tab = false;
                                } else {
                                    state.modal_mode = crate::app::ModalMode::None;
                                    state.modal_input.clear();
                                }
                            }
                            KeyCode::Char('D') => {
                                if state.modal_mode == crate::app::ModalMode::Bookmarks {
                                    state.remove_bookmark(state.modal_selected_index);
                                } else if state.modal_mode == crate::app::ModalMode::Recent {
                                    state.remove_recent(state.modal_selected_index);
                                }
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() => {
                                state.modal_input.push(c);
                                if let Ok(idx) = state.modal_input.parse::<usize>() {
                                    let idx = idx.saturating_sub(1);
                                    let len = match state.modal_mode {
                                        crate::app::ModalMode::Bookmarks => {
                                            state.config.bookmarks.len()
                                        }
                                        crate::app::ModalMode::Recent => state.config.recent.len(),
                                        _ => 0,
                                    };
                                    if idx < len {
                                        state.modal_selected_index = idx;
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                state.modal_input.pop();
                                if !state.modal_input.is_empty() {
                                    if let Ok(idx) = state.modal_input.parse::<usize>() {
                                        let idx = idx.saturating_sub(1);
                                        let len = match state.modal_mode {
                                            crate::app::ModalMode::Bookmarks => {
                                                state.config.bookmarks.len()
                                            }
                                            crate::app::ModalMode::Recent => {
                                                state.config.recent.len()
                                            }
                                            _ => 0,
                                        };
                                        if idx < len {
                                            state.modal_selected_index = idx;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else if state.search_mode {
                    match key.code {
                        KeyCode::Esc => state.toggle_search_mode(),
                        KeyCode::Enter => state.toggle_search_mode(),
                        KeyCode::Backspace => state.pop_search_char(),
                        KeyCode::Char('/') if state.search_query.is_empty() => {
                            state.toggle_search_mode();
                        }
                        KeyCode::Char(c) => state.push_search_char(c),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('/') => state.toggle_search_mode(),
                        KeyCode::Down | KeyCode::Char('j') => state.move_selection(1),
                        KeyCode::Up | KeyCode::Char('k') => state.move_selection(-1),
                        KeyCode::Enter => {
                            if let Some(target) = state.selected_item() {
                                if target.entry_type == crate::fs::walker::EntryType::Directory {
                                    state.navigate_in();
                                } else if target.entry_type == crate::fs::walker::EntryType::File {
                                    disable_raw_mode()?;
                                    execute!(
                                        terminal.backend_mut(),
                                        LeaveAlternateScreen,
                                        DisableMouseCapture
                                    )?;
                                    terminal.show_cursor()?;

                                    let editor = std::env::var("EDITOR")
                                        .unwrap_or_else(|_| "nvim".to_string());
                                    let mut cmd = std::process::Command::new(&editor);
                                    if (editor.contains("nvim") || editor.contains("vim"))
                                        && !state.search_query.is_empty()
                                    {
                                        cmd.arg(format!("+/{}", state.search_query));
                                    }
                                    cmd.arg(&target.path);

                                    let mut child = cmd.spawn()?;
                                    let _status = child.wait()?;

                                    enable_raw_mode()?;
                                    execute!(
                                        terminal.backend_mut(),
                                        EnterAlternateScreen,
                                        EnableMouseCapture
                                    )?;
                                    terminal.hide_cursor()?;
                                    terminal.clear()?;
                                }
                            }
                        }
                        KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                            if state.search_query.is_empty() {
                                state.navigate_out();
                            } else {
                                state.clear_search();
                            }
                        }
                        KeyCode::Char('s') => state.cycle_sort_mode(),
                        KeyCode::Char('e') => state.toggle_expand(),
                        KeyCode::Char('t') => state.open_tab(state.current_path.clone()),
                        KeyCode::Char('T') => state.open_home_tab(),
                        KeyCode::Tab => state.next_tab(),
                        KeyCode::BackTab => state.prev_tab(),
                        KeyCode::Char('x') => state.close_tab(),
                        KeyCode::Char('m') => state.add_bookmark(),
                        KeyCode::Char('b') => {
                            state.modal_mode = crate::app::ModalMode::Bookmarks;
                            state.modal_selected_index = 0;
                            state.modal_input.clear();
                        }
                        KeyCode::Char('R') => {
                            state.modal_mode = crate::app::ModalMode::Recent;
                            state.modal_selected_index = 0;
                            state.modal_input.clear();
                        }
                        KeyCode::Char(' ') => state.toggle_select(),
                        KeyCode::Char('y') => state.yank_full_path(),
                        KeyCode::Char('Y') => state.yank_filename(),
                        KeyCode::Char('X') => state.cut_file(),
                        KeyCode::Char('v') => state.paste_files(),
                        KeyCode::Char('o') => state.open_in_system(),
                        KeyCode::Char('d') => state.request_delete(),
                        KeyCode::Char('r') => state.request_rename(),
                        _ => {}
                    }
                }
            }
            ui::event::AppEvent::Resize(_, _) => {
                // The terminal backend automatically updates its size representation;
                // next iteration will redraw at the correct size.
            }
            ui::event::AppEvent::Mouse(_) => {}
        }
    }

    Ok(state.current_path)
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
    } else {
        match run_tui(args.path) {
            Ok(final_path) => {
                if let Some(export_file) = args.export_target {
                    if let Err(e) = shell::write_export_target(&export_file, &final_path) {
                        eprintln!("[x] Export target error: {e}");
                    }
                }
            }
            Err(e) => eprintln!("[x] TUI error: {e}"),
        }
    }
}
