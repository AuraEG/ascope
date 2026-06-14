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

use once_cell::sync::Lazy;
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
use crate::fs::walker::EntryType;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

// --------------------------------------------------------------------------
// [SECTION] Dashboard Renderer
// --------------------------------------------------------------------------

/// Draw the full TUI dashboard into the current frame.
///
/// The screen is split 50/50 horizontally: the left pane shows the directory
/// tree with the active selection highlighted; the right pane shows a bar for
/// each entry proportional to its share of the total scanned size.
pub fn render_dashboard(f: &mut Frame, state: &AppState) {
    let layout = crate::ui::layout::build_layout(f.size(), true, state.search_mode);

    if layout.tab_bar.height > 0 {
        render_tab_bar(f, state, layout.tab_bar);
    }

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout.main_area);

    render_tree(f, state, panes[0]);
    render_right_pane(f, state, panes[1]);

    if state.search_mode {
        render_search_overlay(f, state, layout.search_bar);
    }

    if layout.status_bar.height > 0 {
        render_status_bar(f, state, layout.status_bar);
    }

    if state.modal_mode != crate::app::ModalMode::None {
        render_modal(f, state);
    }

    render_notification(f, state);
}

fn render_tab_bar(f: &mut Frame, state: &AppState, area: Rect) {
    let mut spans = Vec::new();
    spans.push(Span::raw(" "));

    for (i, tab) in state.tabs.iter().enumerate() {
        let path = &tab.current_path;
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        let label = format!(" {}: {} ", i + 1, name);
        if i == state.active_tab {
            let style = if state.search_mode {
                Style::default().fg(Color::Black).bg(Color::Yellow).bold()
            } else {
                Style::default().fg(Color::Black).bg(Color::Cyan).bold()
            };
            spans.push(Span::styled(label, style));
        } else {
            spans.push(Span::styled(
                label,
                Style::default().fg(Color::Gray).bg(Color::Rgb(40, 40, 40)),
            ));
        }
        spans.push(Span::raw(" "));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(paragraph, area);
}

fn render_modal(f: &mut Frame, state: &AppState) {
    if state.modal_mode == crate::app::ModalMode::None {
        return;
    }

    if state.modal_mode == crate::app::ModalMode::OpenConfirmation {
        render_open_confirmation(f, state);
        return;
    }

    let area = centered_rect(70, 60, f.size());
    let screen = f.size();

    // 1. Draw Dropshadow
    if area.width > 1 && area.height > 1 {
        let shadow_area = Rect {
            x: (area.x + 1).min(screen.width.saturating_sub(1)),
            y: (area.y + 1).min(screen.height.saturating_sub(1)),
            width: area.width.min(screen.width.saturating_sub(area.x + 1)),
            height: area.height.min(screen.height.saturating_sub(area.y + 1)),
        };
        let shadow_block = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 16)));
        f.render_widget(shadow_block, shadow_area);
    }

    // 2. Prepare Title and Footer Lines
    let title_text = match state.modal_mode {
        crate::app::ModalMode::Bookmarks => " Bookmarks persistence ",
        crate::app::ModalMode::Recent => " Recently Visited Directories ",
        crate::app::ModalMode::None | crate::app::ModalMode::OpenConfirmation => "",
    };
    let title_line = Line::from(vec![
        Span::styled(" 󰉋 ", Style::default().fg(Color::Rgb(150, 100, 220)).bold()),
        Span::styled(title_text, Style::default().fg(Color::LightCyan).bold()),
        Span::styled(" ", Style::default()),
    ]);

    let footer_line = if !state.modal_input.is_empty() {
        Line::from(vec![
            Span::styled(
                " Go to index: ",
                Style::default().fg(Color::Rgb(240, 200, 50)).bold(),
            ),
            Span::styled(
                state.modal_input.clone(),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled(
                " │ [Enter] jump │ [Esc] cancel ",
                Style::default().fg(Color::Gray),
            ),
        ])
    } else {
        match state.modal_mode {
            crate::app::ModalMode::Bookmarks => Line::from(vec![
                Span::styled(
                    " [Enter] ",
                    Style::default().fg(Color::Rgb(0, 180, 216)).bold(),
                ),
                Span::styled("jump ", Style::default().fg(Color::Gray)),
                Span::styled(
                    " [Esc] ",
                    Style::default().fg(Color::Rgb(0, 180, 216)).bold(),
                ),
                Span::styled("close ", Style::default().fg(Color::Gray)),
                Span::styled(" [D] ", Style::default().fg(Color::Rgb(220, 50, 50)).bold()),
                Span::styled("delete ", Style::default().fg(Color::Gray)),
                Span::styled(
                    " [1-9] ",
                    Style::default().fg(Color::Rgb(240, 200, 50)).bold(),
                ),
                Span::styled("direct jump ", Style::default().fg(Color::Gray)),
            ]),
            crate::app::ModalMode::Recent => Line::from(vec![
                Span::styled(
                    " [Enter] ",
                    Style::default().fg(Color::Rgb(0, 180, 216)).bold(),
                ),
                Span::styled("jump ", Style::default().fg(Color::Gray)),
                Span::styled(
                    " [Esc] ",
                    Style::default().fg(Color::Rgb(0, 180, 216)).bold(),
                ),
                Span::styled("close ", Style::default().fg(Color::Gray)),
                Span::styled(" [D] ", Style::default().fg(Color::Rgb(220, 50, 50)).bold()),
                Span::styled("delete ", Style::default().fg(Color::Gray)),
                Span::styled(
                    " [1-9] ",
                    Style::default().fg(Color::Rgb(240, 200, 50)).bold(),
                ),
                Span::styled("direct jump ", Style::default().fg(Color::Gray)),
            ]),
            crate::app::ModalMode::None | crate::app::ModalMode::OpenConfirmation => {
                Line::default()
            }
        }
    };

    let block = Block::default()
        .title(title_line)
        .title_alignment(Alignment::Center)
        .title_bottom(footer_line)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(150, 100, 220))) // sleek purple border
        .style(Style::default().bg(Color::Rgb(25, 25, 30))); // deep background

    let is_empty = match state.modal_mode {
        crate::app::ModalMode::Bookmarks => state.config.bookmarks.is_empty(),
        crate::app::ModalMode::Recent => state.config.recent.is_empty(),
        crate::app::ModalMode::None | crate::app::ModalMode::OpenConfirmation => true,
    };

    if is_empty {
        let msg = match state.modal_mode {
            crate::app::ModalMode::Bookmarks => {
                "\n\n\n  No bookmarks saved yet.\n\n  Press 'm' in the directory tree to bookmark any directory."
            }
            crate::app::ModalMode::Recent => {
                "\n\n\n  No recently visited directories."
            }
            crate::app::ModalMode::None | crate::app::ModalMode::OpenConfirmation => "",
        };
        let paragraph = Paragraph::new(msg)
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(160, 160, 160)));

        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
        return;
    }

    let list_items: Vec<ListItem> = match state.modal_mode {
        crate::app::ModalMode::Bookmarks => state
            .config
            .bookmarks
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let is_selected = i == state.modal_selected_index;
                let matches_input = if !state.modal_input.is_empty() {
                    if let Ok(idx) = state.modal_input.parse::<usize>() {
                        idx.saturating_sub(1) == i
                    } else {
                        false
                    }
                } else {
                    false
                };

                let mut spans = Vec::new();
                if matches_input {
                    spans.push(Span::styled(
                        " ⚡ ",
                        Style::default().fg(Color::Rgb(240, 200, 50)).bold(),
                    ));
                } else if is_selected {
                    spans.push(Span::styled(
                        " ➔ ",
                        Style::default().fg(Color::Rgb(0, 180, 216)).bold(),
                    ));
                } else {
                    spans.push(Span::raw("   "));
                }

                let index_style = if matches_input {
                    Style::default().fg(Color::White).bold()
                } else if is_selected {
                    Style::default().fg(Color::Rgb(240, 200, 50)).bold()
                } else {
                    Style::default().fg(Color::Rgb(150, 150, 150))
                };
                spans.push(Span::styled(format!("[{}] ", i + 1), index_style));

                let path_str = path.display().to_string();
                let path_style = if matches_input || is_selected {
                    Style::default().fg(Color::White).bold()
                } else {
                    Style::default().fg(Color::Rgb(180, 180, 180))
                };
                spans.push(Span::styled(path_str, path_style));

                let item_style = if matches_input {
                    Style::default().bg(Color::Rgb(100, 40, 180))
                } else if is_selected {
                    Style::default().bg(Color::Rgb(50, 50, 75))
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(spans)).style(item_style)
            })
            .collect(),
        crate::app::ModalMode::Recent => state
            .config
            .recent
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let is_selected = i == state.modal_selected_index;
                let matches_input = if !state.modal_input.is_empty() {
                    if let Ok(idx) = state.modal_input.parse::<usize>() {
                        idx.saturating_sub(1) == i
                    } else {
                        false
                    }
                } else {
                    false
                };

                let mut spans = Vec::new();
                if matches_input {
                    spans.push(Span::styled(
                        " ⚡ ",
                        Style::default().fg(Color::Rgb(240, 200, 50)).bold(),
                    ));
                } else if is_selected {
                    spans.push(Span::styled(
                        " ➔ ",
                        Style::default().fg(Color::Rgb(0, 180, 216)).bold(),
                    ));
                } else {
                    spans.push(Span::raw("   "));
                }

                let index_style = if matches_input {
                    Style::default().fg(Color::White).bold()
                } else if is_selected {
                    Style::default().fg(Color::Rgb(240, 200, 50)).bold()
                } else {
                    Style::default().fg(Color::Rgb(150, 150, 150))
                };
                spans.push(Span::styled(format!("[{}] ", i + 1), index_style));

                let path_str = path.display().to_string();
                let path_style = if matches_input || is_selected {
                    Style::default().fg(Color::White).bold()
                } else {
                    Style::default().fg(Color::Rgb(180, 180, 180))
                };
                spans.push(Span::styled(path_str, path_style));

                let item_style = if matches_input {
                    Style::default().bg(Color::Rgb(100, 40, 180))
                } else if is_selected {
                    Style::default().bg(Color::Rgb(50, 50, 75))
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(spans)).style(item_style)
            })
            .collect(),
        crate::app::ModalMode::None | crate::app::ModalMode::OpenConfirmation => Vec::new(),
    };

    f.render_widget(Clear, area);
    f.render_widget(List::new(list_items).block(block), area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// --------------------------------------------------------------------------
// [SECTION] Left Pane -- Directory Tree
// --------------------------------------------------------------------------

fn render_tree(f: &mut Frame, state: &AppState, area: Rect) {
    let visible = state.visible_items();
    let total_items = visible.len();
    let max_height = (area.height as usize).saturating_sub(2);
    let start_idx = if total_items <= max_height {
        0
    } else {
        let half = max_height / 2;
        if state.selected_index < half {
            0
        } else if state.selected_index >= total_items - half {
            total_items - max_height
        } else {
            state.selected_index - half
        }
    };

    let window = &visible[start_idx..total_items.min(start_idx + max_height)];

    let items: Vec<ListItem> = window
        .iter()
        .enumerate()
        .map(|(idx, (entry, score))| {
            let actual_idx = start_idx + idx;
            let path = &entry.path;
            let size = entry.size;
            let entry_type = &entry.entry_type;
            let mtime = entry.mtime;
            let raw_name = path.file_name().unwrap_or_default().to_string_lossy();
            let mut name = sanitize_line(&raw_name);
            if let Some(target) = &entry.symlink_target {
                name = format!("{} -> {}", name, target.display());
            }
            #[allow(clippy::cast_precision_loss)]
            let size_mb = size as f64 / 1_000_000.0;

            let mut spans = Vec::new();

            // Render score badge if search is active
            if !state.search_query.is_empty() && *score > 0 {
                spans.push(Span::styled(
                    format!(" [{score}]"),
                    Style::default().fg(Color::Yellow),
                ));
            }

            // Tree indentation and guides
            let depth = path
                .strip_prefix(&state.current_path)
                .map(|r| r.components().count())
                .unwrap_or(0);

            let mut indent = String::new();
            if depth > 1 {
                if let Ok(path_comps) = path.strip_prefix(&state.current_path) {
                    let path_comps: Vec<_> = path_comps.components().collect();
                    for i in 1..depth {
                        let mut ancestor_path = state.current_path.clone();
                        for comp in path_comps.iter().take(i) {
                            ancestor_path.push(comp);
                        }
                        let parent_path = ancestor_path.parent().unwrap();

                        let has_later_sibling = window[idx + 1..].iter().any(|(next_entry, _)| {
                            next_entry.path.starts_with(parent_path)
                                && !next_entry.path.starts_with(&ancestor_path)
                        });

                        if i == depth - 1 {
                            if has_later_sibling {
                                indent.push_str("  ├── ");
                            } else {
                                indent.push_str("  └── ");
                            }
                        } else if has_later_sibling {
                            indent.push_str("  │   ");
                        } else {
                            indent.push_str("      ");
                        }
                    }
                }
            } else if depth == 1 {
                indent.push(' ');
            }

            spans.push(Span::raw(indent));

            // Prefix with Nerd Font file type icon
            let icon = get_icon(path, entry_type);
            let icon_style = get_icon_style(path, entry_type);
            spans.push(Span::styled(format!("{icon} "), icon_style));

            spans.push(Span::raw(format!("{name} ({size_mb:.2} MB)")));

            let mtime_str = format_system_time(mtime);
            spans.push(Span::styled(
                format!(" [{}]", mtime_str),
                Style::default().fg(Color::DarkGray),
            ));

            let item_style = if actual_idx == state.selected_index {
                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(spans)).style(item_style)
        })
        .collect();

    let block = Block::default()
        .title(" Directory Tree ")
        .borders(Borders::ALL);

    f.render_widget(List::new(items).block(block), area);
}

// --------------------------------------------------------------------------
// [SECTION] Right Pane -- Size Bars or Preview
// --------------------------------------------------------------------------

fn render_right_pane(f: &mut Frame, state: &AppState, area: Rect) {
    if let Some(entry) = state.selected_item() {
        if entry.entry_type == EntryType::File {
            render_file_preview(f, state, area);
            return;
        }
    }

    render_size_bars(f, state, area);
}

fn render_size_bars(f: &mut Frame, state: &AppState, area: Rect) {
    let visible = state.visible_items();
    // The largest entry anchors the scale so all bars are relative to it.
    let max_size = visible.first().map(|(e, _)| e.size).unwrap_or(0).max(1);

    let total_items = visible.len();
    let max_height = (area.height as usize).saturating_sub(2);
    let start_idx = if total_items <= max_height {
        0
    } else {
        let half = max_height / 2;
        if state.selected_index < half {
            0
        } else if state.selected_index >= total_items - half {
            total_items - max_height
        } else {
            state.selected_index - half
        }
    };

    let window = &visible[start_idx..total_items.min(start_idx + max_height)];

    let items: Vec<ListItem> = window
        .iter()
        .map(|(entry, _)| {
            let size = entry.size;
            #[allow(clippy::cast_precision_loss)]
            let ratio = (size as f64 / max_size as f64).clamp(0.0, 1.0);
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

fn render_file_preview(f: &mut Frame, state: &AppState, area: Rect) {
    if let Some(entry) = state.selected_item() {
        let title = entry
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let block = Block::default()
            .title(format!(" Preview: {title} "))
            .borders(Borders::ALL);

        let lines = state.preview_lines().to_vec();
        let preview = Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false });
        f.render_widget(preview, area);
    }
}

pub fn build_preview_lines(path: &Path, query: &str) -> Vec<Line<'static>> {
    match load_preview_source(path, query) {
        Ok((source, start_line)) => highlight_preview_source(path, &source, query, start_line),
        Err(error) => {
            if error.kind() == std::io::ErrorKind::InvalidData {
                vec![Line::from("[Binary File - Preview Not Available]")]
            } else {
                vec![Line::from(format!("[x] Preview error: {error}"))]
            }
        }
    }
}

fn load_preview_source(path: &Path, query: &str) -> std::io::Result<(String, usize)> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();

    for line in reader.lines() {
        lines.push(line?);
    }

    let mut match_line = 0;
    if !query.is_empty() {
        let q_lower = query.to_lowercase();
        for (i, line) in lines.iter().enumerate() {
            if line.to_lowercase().contains(&q_lower) {
                match_line = i;
                break;
            }
        }
    }

    let start = match_line.saturating_sub(15);
    let end = (start + 100).min(lines.len());
    let start = end.saturating_sub(100).min(start);

    let mut content = String::new();
    for line in &lines[start..end] {
        content.push_str(line);
        content.push('\n');
    }
    Ok((content, start))
}

fn highlight_preview_source(
    path: &Path,
    source: &str,
    query: &str,
    start_line: usize,
) -> Vec<Line<'static>> {
    let syntax = SYNTAX_SET
        .find_syntax_for_file(path)
        .ok()
        .flatten()
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let theme = THEME_SET
        .themes
        .get("base16-ocean.dark")
        .unwrap_or_else(|| THEME_SET.themes.values().next().expect("default theme"));
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    let mut line_num = start_line + 1;
    for line in source.split_inclusive('\n') {
        let clean_line = line.trim_end_matches(&['\r', '\n'][..]);
        let sanitized = sanitize_line(clean_line);

        let mut spans = vec![Span::styled(
            format!("{:4} │ ", line_num),
            Style::default().fg(Color::DarkGray),
        )];

        match highlighter.highlight_line(&sanitized, &SYNTAX_SET) {
            Ok(ranges) => {
                for (style, text) in ranges {
                    spans.extend(highlight_spans_with_query(
                        text,
                        to_ratatui_style(style),
                        query,
                    ));
                }
            }
            Err(_) => {
                spans.extend(highlight_spans_with_query(
                    &sanitized,
                    Style::default(),
                    query,
                ));
            }
        }
        lines.push(Line::from(spans));
        line_num += 1;
    }

    if lines.is_empty() {
        lines.push(Line::from("[i] Empty file"));
    }

    lines
}

fn highlight_spans_with_query(text: &str, style: Style, query: &str) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), style)];
    }

    let mut spans = Vec::new();
    let q_len = query.len();
    let q_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();

    let mut last_idx = 0;
    let mut search_idx = 0;

    while let Some(idx) = text_lower[search_idx..].find(&q_lower) {
        let absolute_idx = search_idx + idx;

        if absolute_idx > last_idx {
            spans.push(Span::styled(
                text[last_idx..absolute_idx].to_string(),
                style,
            ));
        }

        let match_text = &text[absolute_idx..absolute_idx + q_len];
        spans.push(Span::styled(
            match_text.to_string(),
            style
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));

        last_idx = absolute_idx + q_len;
        search_idx = last_idx;
    }

    if last_idx < text.len() {
        spans.push(Span::styled(text[last_idx..].to_string(), style));
    }

    spans
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

/// Replaces tab characters with 4 spaces and strips or replaces other control
/// characters (like \r, \x1b, backspaces) to prevent TUI screen corruption.
fn sanitize_line(line: &str) -> String {
    let mut sanitized = String::with_capacity(line.len());
    for c in line.chars() {
        if c == '\t' {
            sanitized.push_str("    ");
        } else if c.is_control() {
            sanitized.push(' ');
        } else {
            sanitized.push(c);
        }
    }
    sanitized
}

pub fn get_icon(path: &Path, entry_type: &EntryType) -> &'static str {
    match entry_type {
        EntryType::Directory => "󰉋",
        EntryType::Symlink => "",
        EntryType::File => {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                match ext.to_lowercase().as_str() {
                    "rs" => "󱘗",
                    "js" | "mjs" | "cjs" => "",
                    "jsx" => "",
                    "ts" | "mts" | "cts" => "",
                    "tsx" => "",
                    "py" | "pyc" | "pyd" | "pyo" => "",
                    "go" => "",
                    "c" => "",
                    "cpp" | "cc" | "cxx" => "",
                    "h" | "hpp" => "",
                    "swift" => "",
                    "kt" | "kts" => "",
                    "java" | "class" | "jar" => "",
                    "scala" => "",
                    "hs" | "lhs" => "",
                    "zig" => "",
                    "nim" => "",
                    "ml" | "mli" => "",
                    "d" => "",
                    "rb" => "",
                    "php" => "",
                    "pl" | "pm" => "",
                    "lua" => "",
                    "wasm" | "wat" => "",
                    "html" | "htm" => "",
                    "css" => "",
                    "scss" | "sass" => "",
                    "less" => "",
                    "elm" => "",
                    "sh" | "bash" | "zsh" | "fish" | "ksh" => "",
                    "ps1" | "psm1" => "",
                    "toml" => "",
                    "yml" | "yaml" => "",
                    "json" => "",
                    "xml" => "󰗀",
                    "csv" | "tsv" => "󰈛",
                    "sql" | "db" | "sqlite" => "",
                    "graphql" | "gql" => "",
                    "tf" | "tfvars" => "",
                    "nix" => "",
                    "md" | "markdown" => "",
                    "pdf" => "",
                    "txt" | "log" | "ini" | "conf" => "󰈙",
                    "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "ico" | "bmp" | "tiff" => "󰸭",
                    "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" | "rar" => "󰿺",
                    _ => "󰈔",
                }
            } else if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                match filename.to_lowercase().as_str() {
                    "makefile" | "gemfile" | "rakefile" => "",
                    "dockerfile" => "",
                    "license" => "󰈙",
                    _ => "󰈔",
                }
            } else {
                "󰈔"
            }
        }
    }
}

pub fn get_icon_style(path: &Path, entry_type: &EntryType) -> Style {
    match entry_type {
        EntryType::Directory => Style::default().fg(Color::LightBlue),
        EntryType::Symlink => Style::default().fg(Color::LightMagenta),
        EntryType::File => {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let color = match ext.to_lowercase().as_str() {
                    "rs" => Color::Rgb(244, 91, 50),
                    "js" | "mjs" | "cjs" | "py" | "rb" => Color::Yellow,
                    "jsx" | "tsx" => Color::LightYellow,
                    "ts" | "mts" | "cts" | "go" | "lua" => Color::Cyan,
                    "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "php" | "pl" | "pm" => Color::Blue,
                    "swift" => Color::Rgb(250, 115, 50),
                    "kt" | "kts" | "hs" | "lhs" | "tf" | "tfvars" => Color::Magenta,
                    "java" | "class" | "jar" => Color::Rgb(220, 50, 50),
                    "scala" => Color::Red,
                    "zig" | "nim" => Color::Rgb(230, 160, 20),
                    "ml" | "mli" => Color::Rgb(238, 90, 36),
                    "d" => Color::Rgb(180, 50, 50),
                    "html" | "htm" | "css" | "scss" | "sass" | "less" => Color::LightRed,
                    "elm" => Color::LightCyan,
                    "sh" | "bash" | "zsh" | "fish" | "ksh" | "ps1" | "psm1" => Color::LightGreen,
                    "json" | "toml" | "yml" | "yaml" | "xml" => Color::Green,
                    "csv" | "tsv" | "sql" | "db" | "sqlite" | "graphql" | "gql" => {
                        Color::LightGreen
                    }
                    "nix" => Color::LightBlue,
                    "md" | "markdown" | "txt" | "log" | "ini" | "conf" => Color::Gray,
                    "pdf" => Color::Red,
                    "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "ico" | "bmp" | "tiff" => {
                        Color::LightMagenta
                    }
                    "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" | "rar" => Color::Rgb(190, 150, 90),
                    _ => Color::White,
                };
                Style::default().fg(color)
            } else if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                match filename.to_lowercase().as_str() {
                    "makefile" | "gemfile" | "rakefile" => Style::default().fg(Color::Yellow),
                    "dockerfile" => Style::default().fg(Color::Cyan),
                    "license" => Style::default().fg(Color::Gray),
                    _ => Style::default().fg(Color::White),
                }
            } else {
                Style::default().fg(Color::White)
            }
        }
    }
}

pub fn format_system_time(time: std::time::SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

// --------------------------------------------------------------------------
// [SECTION] Bottom Overlay -- Search Input
// --------------------------------------------------------------------------

fn render_search_overlay(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let matches_count = if state.search_query.is_empty() {
        0
    } else {
        state.visible_items().len()
    };

    let prompt = Paragraph::new(format!(
        "Search: {} ({} matches)",
        state.search_query, matches_count
    ))
    .block(block);
    f.render_widget(Clear, area);
    f.render_widget(prompt, area);
}

// --------------------------------------------------------------------------
// [SECTION] Status Bar
// --------------------------------------------------------------------------

fn render_status_bar(f: &mut Frame, state: &AppState, area: Rect) {
    let mut spans = Vec::new();

    // 1. Current Path
    let raw_path = state.current_path.to_string_lossy();
    let path = sanitize_line(&raw_path);
    spans.push(Span::styled(
        format!(" {path} "),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // 2. Entries Count / Scanning
    if state.is_scanning() {
        let label = state.scan_progress_label();
        spans.push(Span::styled(
            format!(" {label} ({} entries) ", state.items.len()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            format!(" {} entries ", state.items.len()),
            Style::default().fg(Color::White),
        ));
    }
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // 3. Total Size
    #[allow(clippy::cast_precision_loss)]
    let total_mb = state.total_size() as f64 / 1_000_000.0;
    spans.push(Span::styled(
        format!(" {:.2} MB ", total_mb),
        Style::default().fg(Color::Green),
    ));
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // 4. Sort Mode
    let sort_label = match state.sort_mode {
        crate::app::SortMode::SizeDesc => " sort: size↓ ",
        crate::app::SortMode::NameAsc => " sort: name↑ ",
        crate::app::SortMode::MtimeDesc => " sort: mtime↓ ",
    };
    spans.push(Span::styled(
        sort_label,
        Style::default().fg(Color::Magenta),
    ));
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // 5. Git Context
    if let Some(git) = &state.git_ctx {
        spans.push(Span::styled(
            format!(" branch@{} ", git.branch),
            Style::default().fg(Color::LightYellow),
        ));
        if git.dirty_count > 0 {
            spans.push(Span::styled(
                format!("●{} ", git.dirty_count),
                Style::default().fg(Color::LightRed),
            ));
        }
    } else {
        spans.push(Span::styled(" no-git ", Style::default().fg(Color::Gray)));
    }
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // 6. Help Hint
    spans.push(Span::styled(" [?]help ", Style::default().fg(Color::Gray)));

    let paragraph =
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(30, 30, 40)));

    f.render_widget(paragraph, area);
}

fn centered_rect_fixed(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}

fn render_open_confirmation(f: &mut Frame, state: &AppState) {
    let path_str = if let Some(path) = &state.modal_target_path {
        path.to_string_lossy().to_string()
    } else {
        String::new()
    };
    let display_path = if path_str.len() > 42 {
        format!("...{}", &path_str[path_str.len() - 39..])
    } else {
        path_str
    };

    let target_label = Line::from(vec![
        Span::styled(" Path: ", Style::default().fg(Color::Gray)),
        Span::styled(display_path, Style::default().fg(Color::White).bold()),
    ]);

    let prompt_label = Line::from(vec![Span::styled(
        " Open this directory in:",
        Style::default().fg(Color::Gray),
    )]);

    let same_style = if !state.modal_confirm_new_tab {
        Style::default().fg(Color::Black).bg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::White)
    };
    let same_choice = Span::styled(" Same Tab [s] ", same_style);

    let new_style = if state.modal_confirm_new_tab {
        Style::default().fg(Color::Black).bg(Color::Cyan).bold()
    } else {
        Style::default().fg(Color::White)
    };
    let new_choice = Span::styled(" New Tab [n] ", new_style);

    let choices_line = Line::from(vec![
        Span::raw("    "),
        same_choice,
        Span::raw("        "),
        new_choice,
    ]);

    let content = vec![
        Line::default(),
        target_label,
        Line::default(),
        prompt_label,
        Line::default(),
        choices_line,
    ];

    let footer_line = Line::from(vec![
        Span::styled(" [Left/Right] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("switch │ ", Style::default().fg(Color::Gray)),
        Span::styled(" [Enter] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("confirm │ ", Style::default().fg(Color::Gray)),
        Span::styled(" [Esc] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("back ", Style::default().fg(Color::Gray)),
    ]);

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" 󰉋 ", Style::default().fg(Color::Rgb(150, 100, 220)).bold()),
            Span::styled(
                " Open Directory ",
                Style::default().fg(Color::LightCyan).bold(),
            ),
            Span::styled(" ", Style::default()),
        ]))
        .title_alignment(Alignment::Center)
        .title_bottom(footer_line)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(150, 100, 220)))
        .style(Style::default().bg(Color::Rgb(25, 25, 30)));

    let area = centered_rect_fixed(50, 9, f.size());
    let screen = f.size();
    if area.width > 1 && area.height > 1 {
        let shadow_area = Rect {
            x: (area.x + 1).min(screen.width.saturating_sub(1)),
            y: (area.y + 1).min(screen.height.saturating_sub(1)),
            width: area.width.min(screen.width.saturating_sub(area.x + 1)),
            height: area.height.min(screen.height.saturating_sub(area.y + 1)),
        };
        let shadow_block = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 16)));
        f.render_widget(shadow_block, shadow_area);
    }

    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(content).block(block), area);
}

fn get_notification_area(screen: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: screen.width.saturating_sub(width + 2),
        y: screen.height.saturating_sub(height + 3),
        width: width + 2,
        height: height + 2,
    }
}

fn render_notification(f: &mut Frame, state: &AppState) {
    if let Some((msg, _)) = &state.notification {
        let text = Line::from(vec![
            Span::styled(" 󰄬 ", Style::default().fg(Color::Green).bold()),
            Span::styled(msg.clone(), Style::default().fg(Color::White)),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .style(Style::default().bg(Color::Rgb(20, 35, 30)));

        let msg_len = msg.len() as u16;
        let area = get_notification_area(f.size(), msg_len + 4, 1);
        f.render_widget(Clear, area);
        f.render_widget(Paragraph::new(text).block(block), area);
    }
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

        let (preview, start_line) = load_preview_source(&file_path, "").unwrap();
        assert!(preview.contains("fn main() {}"));
        assert_eq!(start_line, 0);
    }

    #[test]
    fn test_syntax_set_lazy_init() {
        let ext = SYNTAX_SET.find_syntax_by_extension("rs");
        assert!(ext.is_some());

        let theme = THEME_SET.themes.get("base16-ocean.dark");
        assert!(theme.is_some());
    }

    #[test]
    fn test_sanitize_line_removes_control_chars_and_tabs() {
        let line_with_tabs = "hello\tworld";
        assert_eq!(sanitize_line(line_with_tabs), "hello    world");

        let line_with_ctrl = "hello\rworld\x1b[31m";
        assert_eq!(sanitize_line(line_with_ctrl), "hello world [31m");
    }

    #[test]
    fn test_icon_lookup() {
        let path_rs = Path::new("main.rs");
        let icon_rs = get_icon(path_rs, &EntryType::File);
        assert_eq!(icon_rs, "󱘗");

        let path_dir = Path::new("src");
        let icon_dir = get_icon(path_dir, &EntryType::Directory);
        assert_eq!(icon_dir, "󰉋");
    }

    #[test]
    fn test_time_formatting() {
        let epoch = std::time::SystemTime::UNIX_EPOCH;
        let formatted = format_system_time(epoch);
        assert!(formatted.starts_with("1970") || formatted.starts_with("1969"));
    }
}
