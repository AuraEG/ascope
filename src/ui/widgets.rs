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
    widgets::{Block, Borders, List, ListItem},
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
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(f.size());

    render_tree(f, state, panes[0]);
    render_size_bars(f, state, panes[1]);
}

// --------------------------------------------------------------------------
// [SECTION] Left Pane -- Directory Tree
// --------------------------------------------------------------------------

fn render_tree(f: &mut Frame, state: &AppState, area: Rect) {
    let items: Vec<ListItem> = state
        .items
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
    // The largest entry anchors the scale so all bars are relative to it.
    let max_size = state.items.first().map(|x| x.1).unwrap_or(1);

    let items: Vec<ListItem> = state
        .items
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
