// ==========================================================================
// File    : ui/widgets.rs
// Project : AuraScope
// Layer   : TUI
// Purpose : Renders the split-pane dashboard: directory tree on the left,
//           proportional size-distribution bars on the right.
//
// Author  : Ahmed Ashour
// Created : 2026-06-13
// ==========================================================================

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::AppState;

// --------------------------------------------------------------------------
// [SECTION] Dashboard Renderer
// --------------------------------------------------------------------------

/// Draw the full TUI dashboard into the current frame.
///
/// The screen is split 50/50 horizontally: the left pane shows the directory
/// tree with the active selection highlighted; the right pane shows a bar for
/// each entry proportional to its share of the total scanned size.
pub fn render_dashboard(f: &mut Frame, state: &AppState) {
    let frame = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(if state.search_mode { 3 } else { 0 }),
        ])
        .split(f.size());

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(frame[0]);

    render_tree(f, state, panes[0]);
    render_size_bars(f, state, panes[1]);

    if state.search_mode {
        render_search_overlay(f, state, frame[1]);
    }
}

// --------------------------------------------------------------------------
// [SECTION] Left Pane -- Directory Tree
// --------------------------------------------------------------------------

fn render_tree(f: &mut Frame, state: &AppState, area: Rect) {
    let visible = state.visible_items();
    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(idx, (path, size))| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            #[allow(clippy::cast_precision_loss)]
            let label = format!(" {name} ({:.2} MB)", *size as f64 / 1_000_000.0);

            // Selected row uses a contrasting pair so it remains legible on any theme.
            let style = if idx == state.selected_index {
                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(label).style(style)
        })
        .collect();

    #[allow(clippy::cast_precision_loss)]
    let total_mb = state.active_stats.total_size as f64 / 1_000_000.0;
    let block = Block::default()
        .title(format!(
            " {} [{total_mb:.2} MB] ",
            state.current_path.display()
        ))
        .borders(Borders::ALL);

    f.render_widget(List::new(items).block(block), area);
}

// --------------------------------------------------------------------------
// [SECTION] Right Pane -- Size Distribution Bars
// --------------------------------------------------------------------------

fn render_size_bars(f: &mut Frame, state: &AppState, area: Rect) {
    let visible = state.visible_items();
    // The largest entry anchors the scale so all bars are relative to it.
    let max_size = visible.first().map(|x| x.1).unwrap_or(1);

    let items: Vec<ListItem> = visible
        .iter()
        .map(|(_, size)| {
            #[allow(clippy::cast_precision_loss)]
            let ratio = (*size as f64 / max_size as f64).clamp(0.0, 1.0);
            let filled = ((ratio * 20.0) as usize).min(20);
            let bar = format!("|{}{}|", "█".repeat(filled), "░".repeat(20 - filled));
            ListItem::new(bar).style(Style::default().fg(Color::LightCyan))
        })
        .collect();

    let block = Block::default()
        .title(" Size Distribution ")
        .borders(Borders::ALL);

    f.render_widget(List::new(items).block(block), area);
}

// --------------------------------------------------------------------------
// [SECTION] Bottom Overlay -- Search Input
// --------------------------------------------------------------------------

fn render_search_overlay(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let prompt = Paragraph::new(format!("Search: {}", state.search_query)).block(block);
    f.render_widget(Clear, area);
    f.render_widget(prompt, area);
}
