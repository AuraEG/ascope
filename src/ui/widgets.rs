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

use crate::app::AppState;
use crate::fs::walker::EntryType;



// --------------------------------------------------------------------------
// [SECTION] Dashboard Renderer
// --------------------------------------------------------------------------

/// Draw the full TUI dashboard into the current frame.
///
/// The screen is split 50/50 horizontally: the left pane shows the directory
/// tree with the active selection highlighted; the right pane shows a bar for
/// each entry proportional to its share of the total scanned size.
pub fn render_dashboard(f: &mut Frame, state: &AppState) {
    let layout =
        crate::ui::layout::build_layout(f.size(), true, state.search_mode || state.rename_mode);

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
    } else if state.rename_mode {
        render_rename_overlay(f, state, layout.search_bar);
    }

    if layout.status_bar.height > 0 {
        render_status_bar(f, state, layout.status_bar);
    }

    if state.modal_mode != crate::app::ModalMode::None {
        render_modal(f, state);
    }

    if state.show_help {
        render_help_modal(f, state);
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

    if state.modal_mode == crate::app::ModalMode::DeleteConfirmation {
        render_delete_confirmation(f, state);
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
        _ => "",
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
            _ => Line::default(),
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
        _ => true,
    };

    if is_empty {
        let msg = match state.modal_mode {
            crate::app::ModalMode::Bookmarks => {
                "\n\n\n  No bookmarks saved yet.\n\n  Press 'm' in the directory tree to bookmark any directory."
            }
            crate::app::ModalMode::Recent => {
                "\n\n\n  No recently visited directories."
            }
            _ => "",
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
        _ => Vec::new(),
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
        let cursor = state.navigation.cursor();
        if cursor < half {
            0
        } else if cursor >= total_items - half {
            total_items - max_height
        } else {
            cursor - half
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

            let is_yanked = state.yanked_files.contains(path);
            let is_cut = state.cut_files.contains(path);
            let is_selected_for_batch = state.selected_paths.contains(path);

            let mut spans = Vec::new();

            // Render batch selection marker
            if is_selected_for_batch {
                spans.push(Span::styled("● ", Style::default().fg(Color::Green)));
            } else {
                spans.push(Span::raw("  "));
            }

            // Render score badge if search is active
            if state
                .navigation
                .filter_query()
                .is_some_and(|q| !q.is_empty())
                && *score > 0
            {
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

            if size == u64::MAX {
                spans.push(Span::raw(name));
            } else {
                spans.push(Span::raw(name));
                let size_str = crate::fs::walker::format_size(size);
                let size_style = if actual_idx == state.navigation.cursor() {
                    Style::default().fg(Color::Rgb(160, 160, 160))
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                spans.push(Span::styled(format!(" ({size_str})"), size_style));
            }

            if is_yanked {
                spans.push(Span::styled(
                    " [YANK]",
                    Style::default().fg(Color::Rgb(240, 200, 50)).bold(),
                ));
            } else if is_cut {
                spans.push(Span::styled(
                    " [CUT]",
                    Style::default().fg(Color::Rgb(128, 128, 128)).bold(),
                ));
            }

            let mtime_str = format_system_time(mtime);
            let mtime_style = if actual_idx == state.navigation.cursor() {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!(" [{}]", mtime_str), mtime_style));

            let item_style = if actual_idx == state.navigation.cursor() {
                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
            } else if is_yanked {
                Style::default().fg(Color::Rgb(240, 200, 50)) // Gold/yellow
            } else if is_cut {
                Style::default().fg(Color::Rgb(128, 128, 128)) // Dimmed gray
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
        let cursor = state.navigation.cursor();
        if cursor < half {
            0
        } else if cursor >= total_items - half {
            total_items - max_height
        } else {
            cursor - half
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

pub fn is_using_bat_previewer() -> bool {
    true
}

fn get_match_line_and_total(path: &Path, query: &str) -> std::io::Result<(usize, usize)> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut total_lines = 0;
    let mut match_line = None;
    let q_lower = query.to_lowercase();

    for line_res in reader.lines() {
        let line = line_res?;
        if match_line.is_none() && !q_lower.is_empty() && line.to_lowercase().contains(&q_lower) {
            match_line = Some(total_lines);
        }
        total_lines += 1;
    }

    Ok((match_line.unwrap_or(0), total_lines))
}

fn is_binary_file(path: &Path) -> bool {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0; 1024];
    let n = match file.read(&mut buf) {
        Ok(bytes_read) => bytes_read,
        Err(_) => return false,
    };
    if n == 0 {
        return false;
    }
    let slice = &buf[..n];
    if slice.contains(&0) {
        return true;
    }
    let mut control_count = 0;
    for &b in slice {
        if b < 32 && b != 9 && b != 10 && b != 13 {
            control_count += 1;
        }
    }
    if control_count * 100 / n > 5 {
        return true;
    }
    false
}

pub fn build_preview_lines(path: &Path, query: &str) -> Vec<Line<'static>> {
    if is_binary_file(path) {
        return vec![Line::from("[Binary File - Preview Not Available]")];
    }
    use ansi_to_tui::IntoText as _;
    use bat::assets::HighlightingAssets;
    use bat::config::{Config, VisibleLines};
    use bat::controller::Controller;
    use bat::input::Input;
    use bat::line_range::{LineRange, LineRanges, HighlightedLineRanges};
    use bat::style::{StyleComponent, StyleComponents};

    let (match_line, total_lines) = match get_match_line_and_total(path, query) {
        Ok(x) => x,
        Err(error) => {
            if error.kind() == std::io::ErrorKind::InvalidData {
                return vec![Line::from("[Binary File - Preview Not Available]")];
            } else {
                return vec![Line::from(format!("[x] Preview error: {error}"))];
            }
        }
    };

    let start = match_line.saturating_sub(15);
    let end = (start + 100).min(total_lines);
    let start = end.saturating_sub(100).min(start);

    let start_line = start + 1;
    let end_line = end.max(1);

    let assets = HighlightingAssets::from_binary();
    let highlighted_lines = if !query.is_empty() {
        HighlightedLineRanges(LineRanges::from(vec![LineRange::new(match_line + 1, match_line + 1)]))
    } else {
        HighlightedLineRanges::default()
    };
    let mut config = Config {
        colored_output: true,
        true_color: true,
        theme: "base16".to_string(),
        visible_lines: VisibleLines::Ranges(LineRanges::from(vec![LineRange::new(start_line, end_line)])),
        highlighted_lines,
        style_components: StyleComponents::new(&[StyleComponent::LineNumbers]),
        ..Default::default()
    };

    let mut mapping = bat::SyntaxMapping::builtin();
    let custom_mappings = [
        ("*.mjs", "JavaScript (Babel)"),
        ("*.cjs", "JavaScript (Babel)"),
        ("*.jsx", "JavaScript (Babel)"),
        ("*.mts", "TypeScript"),
        ("*.cts", "TypeScript"),
        ("*.tsx", "TypeScriptReact"),
        ("*.tfvars", "Terraform"),
        ("*.kts", "Kotlin"),
        ("*.pyc", "Python"),
        ("*.pyd", "Python"),
        ("*.pyo", "Python"),
        ("*.cc", "C++"),
        ("*.cxx", "C++"),
        ("*.hpp", "C++"),
        ("*.zsh", "Bourne Again Shell (bash)"),
        ("*.fish", "Bourne Again Shell (bash)"),
        ("*.db", "SQL"),
        ("*.sqlite", "SQL"),
        ("*.gql", "GraphQL"),
        ("*.markdown", "Markdown"),
        ("Gemfile", "Ruby"),
        ("Rakefile", "Ruby"),
        ("gemfile", "Ruby"),
        ("rakefile", "Ruby"),
        ("Dockerfile", "Dockerfile"),
        ("dockerfile", "Dockerfile"),
        ("LICENSE", "Plain Text"),
        ("license", "Plain Text"),
    ];

    use bat::MappingTarget;
    for &(pattern, syntax) in &custom_mappings {
        mapping.insert(pattern, MappingTarget::MapTo(syntax)).ok();
    }
    config.syntax_mapping = mapping;

    let controller = Controller::new(&config, &assets);
    let mut output_string = String::new();
    let input = Input::ordinary_file(path);

    if controller.run(vec![input], Some(&mut output_string)).is_ok() {
        if let Ok(text) = output_string.as_bytes().into_text() {
            let mut highlighted_lines = Vec::new();
            for line in text.lines {
                let mut highlighted_spans = Vec::new();
                for span in line.spans {
                    highlighted_spans.extend(highlight_spans_with_query(
                        &span.content,
                        span.style,
                        query,
                    ));
                }
                highlighted_lines.push(Line::from(highlighted_spans));
            }
            return highlighted_lines;
        }
    }

    vec![Line::from("[Error generating preview]")]
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

    let query = state.navigation.filter_query().unwrap_or("");
    let matches_count = if query.is_empty() {
        0
    } else {
        state.visible_items().len()
    };

    let prompt =
        Paragraph::new(format!("Search: {} ({} matches)", query, matches_count)).block(block);
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
    let entry_count = state.navigation.visible_items().len();
    if state.is_scanning() {
        let label = state.scan_progress_label();
        spans.push(Span::styled(
            format!(" {label} ({entry_count} entries) "),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            format!(" {entry_count} entries "),
            Style::default().fg(Color::White),
        ));
    }
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // 3. Total Size
    let total_size_str = crate::fs::walker::format_size(state.total_size());
    spans.push(Span::styled(
        format!(" {total_size_str} "),
        Style::default().fg(Color::Green),
    ));
    spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));

    // 4. Sort Mode
    let sort_label = match state.navigation.sort_mode() {
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

fn render_delete_confirmation(f: &mut Frame, state: &AppState) {
    let count = state.delete_targets.len();
    let prompt_label = Line::from(vec![Span::styled(
        " Are you sure you want to permanently delete: ",
        Style::default().fg(Color::Gray),
    )]);

    let target_label = if count == 1 {
        let path_str = state.delete_targets[0].to_string_lossy().to_string();
        let display_path = if path_str.len() > 42 {
            format!("...{}", &path_str[path_str.len() - 39..])
        } else {
            path_str
        };
        Line::from(vec![
            Span::raw("    "),
            Span::styled(display_path, Style::default().fg(Color::LightRed).bold()),
        ])
    } else {
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("{} selected items", count),
                Style::default().fg(Color::LightRed).bold(),
            ),
        ])
    };

    let warning_label = Line::from(vec![
        Span::raw("    "),
        Span::styled(
            "󰆴 This action cannot be undone!",
            Style::default().fg(Color::Rgb(220, 50, 50)).bold(),
        ),
    ]);

    let content = vec![
        Line::default(),
        prompt_label,
        Line::default(),
        target_label,
        Line::default(),
        warning_label,
    ];

    let footer_line = Line::from(vec![
        Span::styled(" [y/Enter] ", Style::default().fg(Color::Red).bold()),
        Span::styled("confirm deletion │ ", Style::default().fg(Color::Gray)),
        Span::styled(" [n/Esc] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("cancel ", Style::default().fg(Color::Gray)),
    ]);

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" 󰆴 ", Style::default().fg(Color::Red).bold()),
            Span::styled(
                " Delete Confirmation ",
                Style::default().fg(Color::LightRed).bold(),
            ),
            Span::styled(" ", Style::default()),
        ]))
        .title_alignment(Alignment::Center)
        .title_bottom(footer_line)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(Color::Rgb(25, 20, 20)));

    let area = centered_rect_fixed(50, 9, f.size());
    let screen = f.size();
    if area.width > 1 && area.height > 1 {
        let shadow_area = Rect {
            x: (area.x + 1).min(screen.width.saturating_sub(1)),
            y: (area.y + 1).min(screen.height.saturating_sub(1)),
            width: area.width.min(screen.width.saturating_sub(area.x + 1)),
            height: area.height.min(screen.height.saturating_sub(area.y + 1)),
        };
        let shadow_block = Block::default().style(Style::default().bg(Color::Rgb(12, 8, 8)));
        f.render_widget(shadow_block, shadow_area);
    }

    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(content).block(block), area);
}

fn render_rename_overlay(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title(" Rename ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let prompt = Paragraph::new(format!("Rename to: {}", state.rename_input)).block(block);
    f.render_widget(Clear, area);
    f.render_widget(prompt, area);

    // Show blinking cursor at input
    f.set_cursor(area.x + 12 + state.rename_input.len() as u16, area.y + 1);
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

struct HelpItem {
    key: &'static str,
    desc: &'static str,
    context: &'static str,
}

static HELP_ITEMS: &[HelpItem] = &[
    HelpItem {
        key: "q / Esc",
        desc: "Quit application",
        context: "General",
    },
    HelpItem {
        key: "?",
        desc: "Toggle help screen",
        context: "General",
    },
    HelpItem {
        key: "j / Down",
        desc: "Move cursor down",
        context: "Navigation",
    },
    HelpItem {
        key: "k / Up",
        desc: "Move cursor up",
        context: "Navigation",
    },
    HelpItem {
        key: "h / Left",
        desc: "Navigate out to parent folder",
        context: "Navigation",
    },
    HelpItem {
        key: "Enter",
        desc: "Enter directory / Open file in EDITOR",
        context: "Navigation",
    },
    HelpItem {
        key: "e",
        desc: "Toggle inline directory expansion",
        context: "Navigation",
    },
    HelpItem {
        key: "s",
        desc: "Cycle sort mode (size/name/mtime)",
        context: "Navigation",
    },
    HelpItem {
        key: "t",
        desc: "Open new tab at current path",
        context: "Tabs",
    },
    HelpItem {
        key: "T",
        desc: "Open new tab at home folder",
        context: "Tabs",
    },
    HelpItem {
        key: "Tab",
        desc: "Switch to next tab",
        context: "Tabs",
    },
    HelpItem {
        key: "Shift+Tab",
        desc: "Switch to previous tab",
        context: "Tabs",
    },
    HelpItem {
        key: "x",
        desc: "Close active tab (keeps at least one)",
        context: "Tabs",
    },
    HelpItem {
        key: "m",
        desc: "Bookmark current folder",
        context: "Bookmarks",
    },
    HelpItem {
        key: "b",
        desc: "Open bookmarks modal",
        context: "Bookmarks",
    },
    HelpItem {
        key: "R",
        desc: "Open recently visited history modal",
        context: "Bookmarks",
    },
    HelpItem {
        key: "Space",
        desc: "Toggle selection of current item",
        context: "File Actions",
    },
    HelpItem {
        key: "y",
        desc: "Yank full path(s) to clipboard",
        context: "File Actions",
    },
    HelpItem {
        key: "Y",
        desc: "Yank filename(s) to clipboard",
        context: "File Actions",
    },
    HelpItem {
        key: "X",
        desc: "Cut file(s) for moving",
        context: "File Actions",
    },
    HelpItem {
        key: "v",
        desc: "Paste yanked/cut files here",
        context: "File Actions",
    },
    HelpItem {
        key: "o",
        desc: "Open selected in system default application",
        context: "File Actions",
    },
    HelpItem {
        key: "d",
        desc: "Delete selected file(s)/folder(s)",
        context: "File Actions",
    },
    HelpItem {
        key: "r",
        desc: "Inline rename selected file/folder",
        context: "File Actions",
    },
    HelpItem {
        key: "/",
        desc: "Fuzzy search files and directories",
        context: "Search",
    },
    HelpItem {
        key: "Esc",
        desc: "Close current overlay / modal",
        context: "Modals",
    },
    HelpItem {
        key: "1-9",
        desc: "Direct jump by index inside modals",
        context: "Modals",
    },
    HelpItem {
        key: "D",
        desc: "Remove entry from bookmarks/recent",
        context: "Modals",
    },
];

fn render_help_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(80, 85, f.size());
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

    // 2. Prepare block title and footer
    let title_line = Line::from(vec![
        Span::styled(" 󰞋 ", Style::default().fg(Color::Rgb(150, 100, 220)).bold()),
        Span::styled(
            " Keyboard Shortcuts & Keybinding Map ",
            Style::default().fg(Color::LightCyan).bold(),
        ),
        Span::styled(" ", Style::default()),
    ]);

    let footer_line = Line::from(vec![
        Span::styled(" [j/k / Up/Down] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("scroll │ ", Style::default().fg(Color::Gray)),
        Span::styled(" [Esc / ?] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("close help ", Style::default().fg(Color::Gray)),
    ]);

    let block = Block::default()
        .title(title_line)
        .title_alignment(Alignment::Center)
        .title_bottom(footer_line)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(150, 100, 220))) // sleek purple border
        .style(Style::default().bg(Color::Rgb(25, 25, 30))); // deep background

    let header_cells = vec![
        ratatui::widgets::Cell::from(Span::styled(
            "Keybinding",
            Style::default().fg(Color::LightCyan).bold(),
        )),
        ratatui::widgets::Cell::from(Span::styled(
            "Action Description",
            Style::default().fg(Color::LightCyan).bold(),
        )),
        ratatui::widgets::Cell::from(Span::styled(
            "Section",
            Style::default().fg(Color::LightCyan).bold(),
        )),
    ];
    let header = ratatui::widgets::Row::new(header_cells)
        .style(Style::default().bg(Color::Rgb(35, 35, 45)))
        .height(1)
        .bottom_margin(1);

    let rows: Vec<ratatui::widgets::Row> = HELP_ITEMS
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == state.help_selected_index;
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan).bold()
            } else {
                Style::default().fg(Color::White)
            };

            let key_span = Span::styled(
                item.key,
                if is_selected {
                    Style::default().fg(Color::Black).bold()
                } else {
                    Style::default().fg(Color::Rgb(240, 200, 50)).bold() // Gold key highlight
                },
            );
            let desc_span = Span::raw(item.desc);
            let ctx_span = Span::styled(
                item.context,
                if is_selected {
                    Style::default().fg(Color::Black)
                } else {
                    Style::default().fg(Color::Rgb(150, 100, 220)) // Purple section tag
                },
            );

            ratatui::widgets::Row::new(vec![
                ratatui::widgets::Cell::from(key_span),
                ratatui::widgets::Cell::from(desc_span),
                ratatui::widgets::Cell::from(ctx_span),
            ])
            .style(style)
        })
        .collect();

    // Calculate column constraints
    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(55),
        Constraint::Percentage(20),
    ];

    // Scroll state management using ratatui's TableState
    let mut table_state = ratatui::widgets::TableState::default();
    table_state.select(Some(state.help_selected_index));

    let table = ratatui::widgets::Table::new(rows, widths)
        .header(header)
        .block(block)
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).bold());

    f.render_widget(Clear, area);
    f.render_stateful_widget(table, area, &mut table_state);
}

// --------------------------------------------------------------------------
// [SECTION] Tests
// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;



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

    #[test]
    fn test_help_modal_items() {
        let has_quit = HELP_ITEMS
            .iter()
            .any(|item| item.desc.to_lowercase().contains("quit"));
        let has_navigate = HELP_ITEMS.iter().any(|item| {
            item.desc.to_lowercase().contains("navigate")
                || item.desc.to_lowercase().contains("directory")
        });
        assert!(has_quit, "Help items should contain quit description");
        assert!(
            has_navigate,
            "Help items should contain navigate/directory description"
        );
    }
}
