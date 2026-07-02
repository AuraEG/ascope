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

use ascope::{app, fs, shell, ui};

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

fn teardown_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::cursor::Show
    );
}

/// Configure the terminal for raw TUI mode, run the event loop, then
/// unconditionally restore the terminal before returning to the caller.
fn run_tui(root: PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        teardown_terminal();
        default_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, root);

    // Restore the terminal regardless of how the event loop exited.
    teardown_terminal();

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
    ascope::plugin::engine::set_current_app_state(&mut state as *mut app::AppState);
    // Configurable tick rate of 16ms (~60fps) for smooth animations and updates.
    let events = ui::event::EventHandler::new(Duration::from_millis(16));

    loop {
        let size = terminal
            .size()
            .unwrap_or_else(|_| ratatui::prelude::Rect::new(0, 0, 80, 24));
        let layout = ui::layout::build_layout(size, true, state.search_mode || state.rename_mode);
        let panes = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(50),
            ])
            .split(layout.main_area);

        let preview_w = panes[1].width.saturating_sub(2);
        let preview_h = panes[1].height.saturating_sub(2);

        if state.last_selection_time.elapsed().as_millis() > 50 {
            state.update_preview_cache(preview_w, preview_h);
        } else {
            // Poll async preview updates even if we don't fully refresh the cache,
            // to make sure background workers can still deliver results.
            state.poll_preview_updates();
        }
        state.poll_search_updates();
        state.poll_shell_updates();
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
                let mut intercepted = false;
                if state.modal_mode == ascope::app::ModalMode::None && !state.rename_mode {
                    if let Some(ref engine) = state.plugin_engine {
                        let dkb_guard = engine.dynamic_keybindings.borrow();
                        let current_char = match key.code {
                            KeyCode::Char(c) => Some(c),
                            _ => None,
                        };

                        if let Some(c) = current_char {
                            let has_ctrl = key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL);
                            let has_alt = key.modifiers.contains(crossterm::event::KeyModifiers::ALT);

                            let key_str = if has_ctrl {
                                format!("ctrl-{}", c)
                            } else if has_alt {
                                format!("alt-{}", c)
                            } else {
                                c.to_string()
                            };

                            let proposed_seq = if state.pending_key_sequence.is_empty() {
                                key_str.clone()
                            } else {
                                format!("{} {}", state.pending_key_sequence, key_str)
                            };

                            enum ActionOrCallback<'a> {
                                Event(String),
                                Callback(&'a mlua::RegistryKey),
                            }

                            let mut complete_match = None;
                            let mut prefix_match = false;

                            for kb in &engine.keybindings {
                                let kb_key_normalized = kb.key.trim().to_lowercase();
                                let proposed_seq_normalized = proposed_seq.trim().to_lowercase();

                                if kb_key_normalized == proposed_seq_normalized {
                                    complete_match = Some(ActionOrCallback::Event(kb.event.clone()));
                                } else if kb_key_normalized.starts_with(&format!("{} ", proposed_seq_normalized))
                                    || kb_key_normalized.starts_with(&proposed_seq_normalized)
                                {
                                    prefix_match = true;
                                }
                            }

                            for dkb in &*dkb_guard {
                                let kb_key_normalized = dkb.key.trim().to_lowercase();
                                let proposed_seq_normalized = proposed_seq.trim().to_lowercase();

                                if kb_key_normalized == proposed_seq_normalized {
                                    complete_match = Some(ActionOrCallback::Callback(&dkb.callback));
                                } else if kb_key_normalized.starts_with(&format!("{} ", proposed_seq_normalized))
                                    || kb_key_normalized.starts_with(&proposed_seq_normalized)
                                {
                                    prefix_match = true;
                                }
                            }

                            if let Some(act) = complete_match {
                                match act {
                                    ActionOrCallback::Event(event) => {
                                        let _ = engine.trigger_event(&event, String::new());
                                    }
                                    ActionOrCallback::Callback(callback_key) => {
                                        let _ = engine.execute_key_callback(callback_key);
                                    }
                                }
                                state.pending_key_sequence.clear();
                                intercepted = true;
                            } else if prefix_match {
                                state.pending_key_sequence = proposed_seq;
                                intercepted = true;
                            } else {
                                state.pending_key_sequence.clear();

                                let fresh_seq = key_str;
                                let fresh_seq_normalized = fresh_seq.trim().to_lowercase();
                                let mut fresh_complete_match = None;
                                let mut fresh_prefix_match = false;

                                for kb in &engine.keybindings {
                                    let kb_key_normalized = kb.key.trim().to_lowercase();
                                    if kb_key_normalized == fresh_seq_normalized {
                                        fresh_complete_match = Some(ActionOrCallback::Event(kb.event.clone()));
                                    } else if kb_key_normalized.starts_with(&format!("{} ", fresh_seq_normalized))
                                        || kb_key_normalized.starts_with(&fresh_seq_normalized)
                                    {
                                        fresh_prefix_match = true;
                                    }
                                }

                                for dkb in &*dkb_guard {
                                    let kb_key_normalized = dkb.key.trim().to_lowercase();
                                    if kb_key_normalized == fresh_seq_normalized {
                                        fresh_complete_match = Some(ActionOrCallback::Callback(&dkb.callback));
                                    } else if kb_key_normalized.starts_with(&format!("{} ", fresh_seq_normalized))
                                        || kb_key_normalized.starts_with(&fresh_seq_normalized)
                                    {
                                        fresh_prefix_match = true;
                                    }
                                }

                                if let Some(act) = fresh_complete_match {
                                    match act {
                                        ActionOrCallback::Event(event) => {
                                            let _ = engine.trigger_event(&event, String::new());
                                        }
                                        ActionOrCallback::Callback(callback_key) => {
                                            let _ = engine.execute_key_callback(callback_key);
                                        }
                                    }
                                    intercepted = true;
                                } else if fresh_prefix_match {
                                    state.pending_key_sequence = fresh_seq;
                                    intercepted = true;
                                }
                            }
                        } else {
                            state.pending_key_sequence.clear();
                        }
                    }
                }

                if intercepted {
                    // Intercepted by plugin keybinding, do nothing
                } else if state.show_help {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('?') => {
                            state.show_help = false;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let count = crate::ui::widgets::help_items_len();
                            state.help_selected_index = (state.help_selected_index + 1) % count;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let count = crate::ui::widgets::help_items_len();
                            state.help_selected_index =
                                (state.help_selected_index + count - 1) % count;
                        }
                        _ => {}
                    }
                } else if state.rename_mode {
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
                } else if state.modal_mode != ascope::app::ModalMode::None {
                    if state.modal_mode == ascope::app::ModalMode::PluginOverlay {
                        match key.code {
                            KeyCode::Esc => {
                                state.modal_mode = ascope::app::ModalMode::None;
                                if let Some(ref engine) = state.plugin_engine {
                                    let _ = engine.clear_modal_callback();
                                }
                            }
                            KeyCode::Up | KeyCode::Char('p')
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                let len = state.plugin_modal_filtered_items.len();
                                if len > 0 {
                                    state.plugin_modal_selected_index =
                                        (state.plugin_modal_selected_index + len - 1) % len;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('n')
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                let len = state.plugin_modal_filtered_items.len();
                                if len > 0 {
                                    state.plugin_modal_selected_index =
                                        (state.plugin_modal_selected_index + 1) % len;
                                }
                            }
                            KeyCode::Up => {
                                let len = state.plugin_modal_filtered_items.len();
                                if len > 0 {
                                    state.plugin_modal_selected_index =
                                        (state.plugin_modal_selected_index + len - 1) % len;
                                }
                            }
                            KeyCode::Down => {
                                let len = state.plugin_modal_filtered_items.len();
                                if len > 0 {
                                    state.plugin_modal_selected_index =
                                        (state.plugin_modal_selected_index + 1) % len;
                                }
                            }
                            KeyCode::Enter => {
                                let idx = state.plugin_modal_selected_index;
                                if let Some(item) = state.plugin_modal_filtered_items.get(idx).cloned() {
                                    state.modal_mode = ascope::app::ModalMode::None;
                                    if let Some(ref engine) = state.plugin_engine {
                                        let _ = engine.trigger_modal_select(item.value, "select".to_string());
                                    }
                                } else {
                                    state.modal_mode = ascope::app::ModalMode::None;
                                    if let Some(ref engine) = state.plugin_engine {
                                        let _ = engine.clear_modal_callback();
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                state.plugin_modal_input.pop();
                                state.update_plugin_modal_filtering();
                            }
                            KeyCode::Char(c) => {
                                state.plugin_modal_input.push(c);
                                state.update_plugin_modal_filtering();
                            }
                            _ => {}
                        }
                    } else if state.modal_mode == ascope::app::ModalMode::CommandPalette {
                        if state.command_palette_focused {
                            match key.code {
                                KeyCode::Esc => {
                                    state.modal_mode = ascope::app::ModalMode::None;
                                    state.command_palette_input.clear();
                                    state.command_palette_cursor_index = 0;
                                    state.command_palette_focused = true;
                                }
                                KeyCode::Up | KeyCode::Char('p')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    state.command_palette_focused = false;
                                    let len = state.command_palette_results.len();
                                    if len > 0 {
                                        state.command_palette_selected_index =
                                            (state.command_palette_selected_index + len - 1) % len;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('n')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    state.command_palette_focused = false;
                                    let len = state.command_palette_results.len();
                                    if len > 0 {
                                        state.command_palette_selected_index =
                                            (state.command_palette_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Up => {
                                    state.command_palette_focused = false;
                                    let len = state.command_palette_results.len();
                                    if len > 0 {
                                        state.command_palette_selected_index =
                                            (state.command_palette_selected_index + len - 1) % len;
                                    }
                                }
                                KeyCode::Down => {
                                    state.command_palette_focused = false;
                                    let len = state.command_palette_results.len();
                                    if len > 0 {
                                        state.command_palette_selected_index =
                                            (state.command_palette_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Left => {
                                    if state.command_palette_cursor_index > 0 {
                                        state.command_palette_cursor_index -= 1;
                                    }
                                }
                                KeyCode::Right => {
                                    if state.command_palette_cursor_index
                                        < state.command_palette_input.chars().count()
                                    {
                                        state.command_palette_cursor_index += 1;
                                    }
                                }
                                KeyCode::Backspace => {
                                    if state.command_palette_cursor_index > 0 {
                                        let char_idx = state.command_palette_cursor_index - 1;
                                        if let Some((byte_idx, _)) =
                                            state.command_palette_input.char_indices().nth(char_idx)
                                        {
                                            state.command_palette_input.remove(byte_idx);
                                            state.command_palette_cursor_index -= 1;
                                            state.update_command_palette_results();
                                        }
                                    }
                                }
                                KeyCode::Delete => {
                                    if state.command_palette_cursor_index
                                        < state.command_palette_input.chars().count()
                                    {
                                        let char_idx = state.command_palette_cursor_index;
                                        if let Some((byte_idx, _)) =
                                            state.command_palette_input.char_indices().nth(char_idx)
                                        {
                                            state.command_palette_input.remove(byte_idx);
                                            state.update_command_palette_results();
                                        }
                                    }
                                }
                                KeyCode::Char(c)
                                    if !key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL)
                                        && !key
                                            .modifiers
                                            .contains(crossterm::event::KeyModifiers::ALT) =>
                                {
                                    let char_idx = state.command_palette_cursor_index;
                                    let byte_idx = state
                                        .command_palette_input
                                        .char_indices()
                                        .nth(char_idx)
                                        .map(|(i, _)| i)
                                        .unwrap_or(state.command_palette_input.len());
                                    state.command_palette_input.insert(byte_idx, c);
                                    state.command_palette_cursor_index += 1;
                                    state.update_command_palette_results();
                                }
                                KeyCode::Enter => {
                                    if let Some(target) = state
                                        .command_palette_results
                                        .get(state.command_palette_selected_index)
                                        .cloned()
                                    {
                                        if !target.cmd.is_empty() {
                                            state.modal_mode = ascope::app::ModalMode::None;
                                            state.command_palette_input.clear();
                                            state.command_palette_cursor_index = 0;
                                            state.command_palette_focused = true;

                                            if target.cmd == "reload_plugins" {
                                                if let Some(ref mut engine) = state.plugin_engine {
                                                    let _ = engine.load_plugins();
                                                }
                                            } else {
                                                disable_raw_mode()?;
                                                execute!(
                                                    terminal.backend_mut(),
                                                    LeaveAlternateScreen,
                                                    DisableMouseCapture
                                                )?;
                                                terminal.show_cursor()?;

                                                println!("\x1b[35m=== Executing Action ===\x1b[0m");
                                                println!("Command: {}\n", target.cmd);

                                                #[cfg(unix)]
                                                let status = std::process::Command::new("sh")
                                                    .arg("-c")
                                                    .arg(&target.cmd)
                                                    .status();

                                                #[cfg(windows)]
                                                let status = std::process::Command::new("cmd")
                                                    .arg("/C")
                                                    .arg(&target.cmd)
                                                    .status();

                                                match status {
                                                    Ok(s) => println!("\n\x1b[32mCommand finished with status: {}\x1b[0m", s),
                                                    Err(e) => println!("\n\x1b[31mFailed to run command: {}\x1b[0m", e),
                                                }

                                                println!("\nPress Enter to return to AuraScope...");
                                                let mut input = String::new();
                                                let _ = std::io::stdin().read_line(&mut input);

                                                enable_raw_mode()?;
                                                execute!(
                                                    terminal.backend_mut(),
                                                    EnterAlternateScreen,
                                                    DisableMouseCapture
                                                )?;
                                                terminal.hide_cursor()?;
                                                terminal.clear()?;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    state.modal_mode = ascope::app::ModalMode::None;
                                    state.command_palette_input.clear();
                                    state.command_palette_cursor_index = 0;
                                    state.command_palette_focused = true;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if state.command_palette_selected_index == 0 {
                                        state.command_palette_focused = true;
                                    } else {
                                        let len = state.command_palette_results.len();
                                        if len > 0 {
                                            state.command_palette_selected_index =
                                                (state.command_palette_selected_index + len - 1)
                                                    % len;
                                        }
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let len = state.command_palette_results.len();
                                    if len > 0 {
                                        state.command_palette_selected_index =
                                            (state.command_palette_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Char('i') | KeyCode::Char('a') | KeyCode::Insert => {
                                    state.command_palette_focused = true;
                                }
                                KeyCode::Backspace
                                | KeyCode::Delete
                                | KeyCode::Left
                                | KeyCode::Right => {
                                    state.command_palette_focused = true;
                                }
                                KeyCode::Char(c)
                                    if !key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL)
                                        && !key
                                            .modifiers
                                            .contains(crossterm::event::KeyModifiers::ALT) =>
                                {
                                    state.command_palette_focused = true;
                                    let char_idx = state.command_palette_cursor_index;
                                    let byte_idx = state
                                        .command_palette_input
                                        .char_indices()
                                        .nth(char_idx)
                                        .map(|(i, _)| i)
                                        .unwrap_or(state.command_palette_input.len());
                                    state.command_palette_input.insert(byte_idx, c);
                                    state.command_palette_cursor_index += 1;
                                    state.update_command_palette_results();
                                }
                                KeyCode::Enter => {
                                    if let Some(target) = state
                                        .command_palette_results
                                        .get(state.command_palette_selected_index)
                                        .cloned()
                                    {
                                        if !target.cmd.is_empty() {
                                            state.modal_mode = ascope::app::ModalMode::None;
                                            state.command_palette_input.clear();
                                            state.command_palette_cursor_index = 0;
                                            state.command_palette_focused = true;

                                            if target.cmd == "reload_plugins" {
                                                if let Some(ref mut engine) = state.plugin_engine {
                                                    let _ = engine.load_plugins();
                                                }
                                            } else {
                                                disable_raw_mode()?;
                                                execute!(
                                                    terminal.backend_mut(),
                                                    LeaveAlternateScreen,
                                                    DisableMouseCapture
                                                )?;
                                                terminal.show_cursor()?;

                                                println!("\x1b[35m=== Executing Action ===\x1b[0m");
                                                println!("Command: {}\n", target.cmd);

                                                #[cfg(unix)]
                                                let status = std::process::Command::new("sh")
                                                    .arg("-c")
                                                    .arg(&target.cmd)
                                                    .status();

                                                #[cfg(windows)]
                                                let status = std::process::Command::new("cmd")
                                                    .arg("/C")
                                                    .arg(&target.cmd)
                                                    .status();

                                                match status {
                                                    Ok(s) => println!("\n\x1b[32mCommand finished with status: {}\x1b[0m", s),
                                                    Err(e) => println!("\n\x1b[31mFailed to run command: {}\x1b[0m", e),
                                                }

                                                println!("\nPress Enter to return to AuraScope...");
                                                let mut input = String::new();
                                                let _ = std::io::stdin().read_line(&mut input);

                                                enable_raw_mode()?;
                                                execute!(
                                                    terminal.backend_mut(),
                                                    EnterAlternateScreen,
                                                    DisableMouseCapture
                                                )?;
                                                terminal.hide_cursor()?;
                                                terminal.clear()?;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else if state.modal_mode == ascope::app::ModalMode::SearchOverlay {
                        if state.search_overlay_focused {
                            match key.code {
                                KeyCode::Esc => {
                                    state.modal_mode = ascope::app::ModalMode::None;
                                    state.search_overlay_input.clear();
                                    state.search_overlay_results.clear();
                                    state.search_overlay_cursor_index = 0;
                                    state.search_overlay_focused = true;
                                }
                                KeyCode::Tab => {
                                    if state.search_overlay_mode
                                        == ascope::app::SearchOverlayMode::FuzzyFiles
                                    {
                                        state.search_overlay_mode =
                                            ascope::app::SearchOverlayMode::LiveGrep;
                                    } else {
                                        state.search_overlay_mode =
                                            ascope::app::SearchOverlayMode::FuzzyFiles;
                                    }
                                    state.update_search_overlay_results();
                                }
                                KeyCode::Up => {
                                    state.search_overlay_focused = false;
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + len - 1) % len;
                                    }
                                }
                                KeyCode::Down => {
                                    state.search_overlay_focused = false;
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Char('p')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    state.search_overlay_focused = false;
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + len - 1) % len;
                                    }
                                }
                                KeyCode::Char('n')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    state.search_overlay_focused = false;
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Char('k')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    state.search_overlay_focused = false;
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + len - 1) % len;
                                    }
                                }
                                KeyCode::Char('j')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    state.search_overlay_focused = false;
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Left => {
                                    if state.search_overlay_cursor_index > 0 {
                                        state.search_overlay_cursor_index -= 1;
                                    }
                                }
                                KeyCode::Right => {
                                    if state.search_overlay_cursor_index
                                        < state.search_overlay_input.chars().count()
                                    {
                                        state.search_overlay_cursor_index += 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(target) = state
                                        .search_overlay_results
                                        .get(state.search_overlay_selected_index)
                                        .cloned()
                                    {
                                        state.modal_mode = ascope::app::ModalMode::None;
                                        state.search_overlay_input.clear();
                                        state.search_overlay_results.clear();
                                        state.search_overlay_cursor_index = 0;
                                        state.search_overlay_focused = true;

                                        state.jump_to_path(target.path.clone());

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

                                        if editor.contains("nvim") || editor.contains("vim") {
                                            if let Some(line) = target.line_number {
                                                cmd.arg(format!("+{}", line));
                                            }
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
                                KeyCode::Backspace => {
                                    if state.search_overlay_cursor_index > 0 {
                                        let char_idx = state.search_overlay_cursor_index - 1;
                                        if let Some((byte_idx, _)) =
                                            state.search_overlay_input.char_indices().nth(char_idx)
                                        {
                                            state.search_overlay_input.remove(byte_idx);
                                            state.search_overlay_cursor_index -= 1;
                                            state.update_search_overlay_results();
                                        }
                                    }
                                }
                                KeyCode::Delete => {
                                    if state.search_overlay_cursor_index
                                        < state.search_overlay_input.chars().count()
                                    {
                                        let char_idx = state.search_overlay_cursor_index;
                                        if let Some((byte_idx, _)) =
                                            state.search_overlay_input.char_indices().nth(char_idx)
                                        {
                                            state.search_overlay_input.remove(byte_idx);
                                            state.update_search_overlay_results();
                                        }
                                    }
                                }
                                KeyCode::Char(c)
                                    if !key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL)
                                        && !key
                                            .modifiers
                                            .contains(crossterm::event::KeyModifiers::ALT) =>
                                {
                                    let char_idx = state.search_overlay_cursor_index;
                                    let byte_idx = state
                                        .search_overlay_input
                                        .char_indices()
                                        .nth(char_idx)
                                        .map(|(i, _)| i)
                                        .unwrap_or(state.search_overlay_input.len());
                                    state.search_overlay_input.insert(byte_idx, c);
                                    state.search_overlay_cursor_index += 1;
                                    state.update_search_overlay_results();
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Esc => {
                                    state.modal_mode = ascope::app::ModalMode::None;
                                    state.search_overlay_input.clear();
                                    state.search_overlay_results.clear();
                                    state.search_overlay_cursor_index = 0;
                                    state.search_overlay_focused = true;
                                }
                                KeyCode::Tab => {
                                    if state.search_overlay_mode
                                        == ascope::app::SearchOverlayMode::FuzzyFiles
                                    {
                                        state.search_overlay_mode =
                                            ascope::app::SearchOverlayMode::LiveGrep;
                                    } else {
                                        state.search_overlay_mode =
                                            ascope::app::SearchOverlayMode::FuzzyFiles;
                                    }
                                    state.update_search_overlay_results();
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + len - 1) % len;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Char('p')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + len - 1) % len;
                                    }
                                }
                                KeyCode::Char('n')
                                    if key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                                {
                                    let len = state.search_overlay_results.len();
                                    if len > 0 {
                                        state.search_overlay_selected_index =
                                            (state.search_overlay_selected_index + 1) % len;
                                    }
                                }
                                KeyCode::Char('i') | KeyCode::Char('a') => {
                                    state.search_overlay_focused = true;
                                }
                                KeyCode::Enter => {
                                    if let Some(target) = state
                                        .search_overlay_results
                                        .get(state.search_overlay_selected_index)
                                        .cloned()
                                    {
                                        state.modal_mode = ascope::app::ModalMode::None;
                                        state.search_overlay_input.clear();
                                        state.search_overlay_results.clear();
                                        state.search_overlay_cursor_index = 0;
                                        state.search_overlay_focused = true;

                                        state.jump_to_path(target.path.clone());

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

                                        if editor.contains("nvim") || editor.contains("vim") {
                                            if let Some(line) = target.line_number {
                                                cmd.arg(format!("+{}", line));
                                            }
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
                                KeyCode::Left
                                | KeyCode::Right
                                | KeyCode::Backspace
                                | KeyCode::Delete => {
                                    state.search_overlay_focused = true;
                                }
                                KeyCode::Char(c)
                                    if !key
                                        .modifiers
                                        .contains(crossterm::event::KeyModifiers::CONTROL)
                                        && !key
                                            .modifiers
                                            .contains(crossterm::event::KeyModifiers::ALT) =>
                                {
                                    state.search_overlay_focused = true;
                                    let char_idx = state.search_overlay_cursor_index;
                                    let byte_idx = state
                                        .search_overlay_input
                                        .char_indices()
                                        .nth(char_idx)
                                        .map(|(i, _)| i)
                                        .unwrap_or(state.search_overlay_input.len());
                                    state.search_overlay_input.insert(byte_idx, c);
                                    state.search_overlay_cursor_index += 1;
                                    state.update_search_overlay_results();
                                }
                                _ => {}
                            }
                        }
                    } else if state.modal_mode == ascope::app::ModalMode::DeleteConfirmation {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                                state.modal_mode = ascope::app::ModalMode::None;
                                state.delete_targets.clear();
                            }
                            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                                state.confirm_delete();
                            }
                            _ => {}
                        }
                    } else if state.modal_mode == ascope::app::ModalMode::SizeDetails {
                        if key.code == KeyCode::Esc {
                            state.close_size_details_popup();
                        }
                    } else if state.modal_mode == ascope::app::ModalMode::OpenConfirmation {
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
                                state.modal_mode = ascope::app::ModalMode::None;
                                state.modal_input.clear();
                            }
                            KeyCode::Char('n') => {
                                if let Some(path) = state.modal_target_path.take() {
                                    state.open_tab(path);
                                }
                                state.modal_mode = ascope::app::ModalMode::None;
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
                                state.modal_mode = ascope::app::ModalMode::None;
                                state.modal_input.clear();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                state.modal_mode = ascope::app::ModalMode::None;
                                state.modal_input.clear();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let len = match state.modal_mode {
                                    ascope::app::ModalMode::Bookmarks => {
                                        state.config.bookmarks.len()
                                    }
                                    ascope::app::ModalMode::Recent => state.config.recent.len(),
                                    _ => 0,
                                };
                                if len > 0 {
                                    state.modal_selected_index =
                                        (state.modal_selected_index + len - 1) % len;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let len = match state.modal_mode {
                                    ascope::app::ModalMode::Bookmarks => {
                                        state.config.bookmarks.len()
                                    }
                                    ascope::app::ModalMode::Recent => state.config.recent.len(),
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
                                            ascope::app::ModalMode::Bookmarks => {
                                                state.config.bookmarks.get(idx).cloned()
                                            }
                                            ascope::app::ModalMode::Recent => {
                                                state.config.recent.get(idx).cloned()
                                            }
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    match state.modal_mode {
                                        ascope::app::ModalMode::Bookmarks => state
                                            .config
                                            .bookmarks
                                            .get(state.modal_selected_index)
                                            .cloned(),
                                        ascope::app::ModalMode::Recent => state
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
                                    state.modal_mode = ascope::app::ModalMode::OpenConfirmation;
                                    state.modal_confirm_new_tab = false;
                                } else {
                                    state.modal_mode = ascope::app::ModalMode::None;
                                    state.modal_input.clear();
                                }
                            }
                            KeyCode::Char('D') => {
                                if state.modal_mode == ascope::app::ModalMode::Bookmarks {
                                    state.remove_bookmark(state.modal_selected_index);
                                } else if state.modal_mode == ascope::app::ModalMode::Recent {
                                    state.remove_recent(state.modal_selected_index);
                                }
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() => {
                                state.modal_input.push(c);
                                if let Ok(idx) = state.modal_input.parse::<usize>() {
                                    let idx = idx.saturating_sub(1);
                                    let len = match state.modal_mode {
                                        ascope::app::ModalMode::Bookmarks => {
                                            state.config.bookmarks.len()
                                        }
                                        ascope::app::ModalMode::Recent => state.config.recent.len(),
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
                                            ascope::app::ModalMode::Bookmarks => {
                                                state.config.bookmarks.len()
                                            }
                                            ascope::app::ModalMode::Recent => {
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
                        KeyCode::Char('/')
                            if state.navigation.filter_query().unwrap_or("").is_empty() =>
                        {
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
                        KeyCode::Char('k')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            state.trigger_size_details_popup();
                        }
                        KeyCode::Char('K') => {
                            state.trigger_size_details_popup();
                        }
                        KeyCode::Up | KeyCode::Char('k') => state.move_selection(-1),
                        KeyCode::Enter => {
                            if let Some(target) = state.selected_item() {
                                if target.entry_type == ascope::fs::walker::EntryType::Directory {
                                    state.navigate_in();
                                } else if target.entry_type == ascope::fs::walker::EntryType::File {
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
                                    let query = state.navigation.filter_query().unwrap_or("");
                                    if (editor.contains("nvim") || editor.contains("vim"))
                                        && !query.is_empty()
                                    {
                                        cmd.arg(format!("+/{}", query));
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
                            if state.navigation.filter_query().unwrap_or("").is_empty() {
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
                            state.modal_mode = ascope::app::ModalMode::Bookmarks;
                            state.modal_selected_index = 0;
                            state.modal_input.clear();
                        }
                        KeyCode::Char('R') => {
                            state.modal_mode = ascope::app::ModalMode::Recent;
                            state.modal_selected_index = 0;
                            state.modal_input.clear();
                        }
                        KeyCode::Char(' ') => state.toggle_select(),
                        KeyCode::Char('f')
                            if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                        {
                            state.modal_mode = ascope::app::ModalMode::SearchOverlay;
                            state.search_overlay_mode = ascope::app::SearchOverlayMode::FuzzyFiles;
                            state.search_overlay_input.clear();
                            state.search_overlay_selected_index = 0;
                            state.search_overlay_cursor_index = 0;
                            state.update_search_overlay_results();
                        }
                        KeyCode::Char('g')
                            if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                        {
                            state.modal_mode = ascope::app::ModalMode::SearchOverlay;
                            state.search_overlay_mode = ascope::app::SearchOverlayMode::LiveGrep;
                            state.search_overlay_input.clear();
                            state.search_overlay_selected_index = 0;
                            state.search_overlay_cursor_index = 0;
                            state.update_search_overlay_results();
                        }
                        KeyCode::Char('y') => state.yank_full_path(),
                        KeyCode::Char('Y') => state.yank_filename(),
                        KeyCode::Char('X') => state.cut_file(),
                        KeyCode::Char('v') => state.paste_files(),
                        KeyCode::Char('o') => state.open_in_system(),
                        KeyCode::Char('d') => state.request_delete(),
                        KeyCode::Char('r') => state.request_rename(),
                        KeyCode::Char('p')
                            if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) =>
                        {
                            state.modal_mode = ascope::app::ModalMode::CommandPalette;
                            state.command_palette_input.clear();
                            state.command_palette_selected_index = 0;
                            state.command_palette_cursor_index = 0;
                            state.command_palette_focused = true;
                            state.rebuild_command_palette_candidates();
                            state.update_command_palette_results();
                        }
                        KeyCode::Char(':') => {
                            state.modal_mode = ascope::app::ModalMode::CommandPalette;
                            state.command_palette_input.clear();
                            state.command_palette_selected_index = 0;
                            state.command_palette_cursor_index = 0;
                            state.command_palette_focused = true;
                            state.rebuild_command_palette_candidates();
                            state.update_command_palette_results();
                        }
                        KeyCode::Char('?') => {
                            state.show_help = true;
                            state.help_selected_index = 0;
                        }
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

    if let Some(ref engine) = state.plugin_engine {
        let _ = engine.trigger_event("on_shutdown", String::new());
    }

    ascope::plugin::engine::clear_current_app_state();
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
                    println!(
                        "Total Size: {} bytes ({})",
                        stats.total_size,
                        fs::walker::format_size(stats.total_size)
                    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_teardown_is_idempotent() {
        teardown_terminal();
        teardown_terminal();
    }

    #[test]
    fn test_panic_teardown_restores_terminal() {
        use std::sync::{Arc, Mutex};
        let old_hook = Arc::new(Mutex::new(Some(std::panic::take_hook())));
        let old_hook_clone = Arc::clone(&old_hook);
        std::panic::set_hook(Box::new(move |info| {
            teardown_terminal();
            if let Some(ref hook) = *old_hook_clone.lock().unwrap() {
                hook(info);
            }
        }));

        let result = std::panic::catch_unwind(|| {
            let _ = enable_raw_mode();
            assert!(crossterm::terminal::is_raw_mode_enabled().unwrap());
            panic!("test panic for terminal teardown");
        });

        // Restore old hook
        let _ = std::panic::take_hook();
        if let Some(hook) = old_hook.lock().unwrap().take() {
            std::panic::set_hook(hook);
        }

        assert!(result.is_err());
        assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    }
}
