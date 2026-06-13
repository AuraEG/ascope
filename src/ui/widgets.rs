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

use std::path::Path;

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
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
    render_right_pane(f, state, panes[1]);

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
    let total_mb = state.total_size() as f64 / 1_000_000.0;
    let progress = state.scan_progress_label();
    let scan_label = if progress.is_empty() {
        String::new()
    } else {
        format!(" {progress}")
    };
    let block = Block::default()
        .title(format!(
            " {}{scan_label} [{total_mb:.2} MB] ",
            state.current_path.display()
        ))
        .borders(Borders::ALL);

    f.render_widget(List::new(items).block(block), area);
}

// --------------------------------------------------------------------------
// [SECTION] Right Pane -- Size Bars or Preview
// --------------------------------------------------------------------------

fn render_right_pane(f: &mut Frame, state: &AppState, area: Rect) {
    if let Some((path, _)) = state.selected_item() {
        if path.is_file() {
            render_file_preview(f, &path, area);
            return;
        }
    }

    render_size_bars(f, state, area);
}

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

fn render_file_preview(f: &mut Frame, path: &Path, area: Rect) {
    let title = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let block = Block::default()
        .title(format!(" Preview: {title} "))
        .borders(Borders::ALL);

    let lines = build_preview_lines(path);
    let preview = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(preview, area);
}

fn build_preview_lines(path: &Path) -> Vec<Line<'static>> {
    match load_preview_source(path) {
        Ok(source) => highlight_preview_source(path, &source),
        Err(error) => vec![Line::from(format!("[x] Preview error: {error}"))],
    }
}

fn load_preview_source(path: &Path) -> std::io::Result<String> {
    let source = std::fs::read_to_string(path)?;
    Ok(source.lines().take(100).collect::<Vec<_>>().join("\n"))
}

fn highlight_preview_source(path: &Path, source: &str) -> Vec<Line<'static>> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let syntax = syntax_set
        .find_syntax_for_file(path)
        .ok()
        .flatten()
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let theme = theme_set
        .themes
        .get("base16-ocean.dark")
        .unwrap_or_else(|| theme_set.themes.values().next().expect("default theme"));
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    for line in source.lines() {
        let mut input = line.to_string();
        input.push('\n');
        match highlighter.highlight_line(&input, &syntax_set) {
            Ok(ranges) => {
                let spans = ranges
                    .into_iter()
                    .map(|(style, text)| Span::styled(text.to_string(), to_ratatui_style(style)))
                    .collect::<Vec<_>>();
                lines.push(Line::from(spans));
            }
            Err(_) => lines.push(Line::from(line.to_string())),
        }
    }

    if lines.is_empty() {
        lines.push(Line::from("[i] Empty file"));
    }

    lines
}

fn to_ratatui_style(style: SyntectStyle) -> Style {
    Style::default()
        .fg(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ))
        .bg(Color::Rgb(
            style.background.r,
            style.background.g,
            style.background.b,
        ))
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

// --------------------------------------------------------------------------
// [SECTION] Tests
// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_load_preview_source_reads_file_contents() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("sample.rs");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"fn main() {}\n").unwrap();

        let preview = load_preview_source(&file_path).unwrap();
        assert!(preview.contains("fn main() {}"));
    }
}
