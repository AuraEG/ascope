// ==========================================================================
// File    : ui/layout.rs
// Project : AuraScope
// Layer   : TUI
// Purpose : Defines screen layout segments and coordinates rendering areas.
//
// Author  : Ahmed Ashour
// Created : 2026-06-14
// ==========================================================================

use ratatui::prelude::*;

/// Holds the pre-calculated screen rectangles for all TUI components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayout {
    /// Area for rendering top tabs.
    pub tab_bar: Rect,
    /// Area for rendering tree and preview panes.
    pub main_area: Rect,
    /// Area for rendering status metadata.
    pub status_bar: Rect,
    /// Area for rendering bottom search input.
    pub search_bar: Rect,
}

/// Computes the layout rectangles based on screen dimensions and feature flags.
pub fn build_layout(area: Rect, has_tabs: bool, has_search: bool) -> AppLayout {
    // According to acceptance criteria, hide the status bar if terminal height is < 10.
    let show_status = area.height >= 10;

    let constraints = [
        Constraint::Length(if has_tabs { 1 } else { 0 }),
        Constraint::Min(0),
        Constraint::Length(if show_status { 1 } else { 0 }),
        Constraint::Length(if has_search { 3 } else { 0 }),
    ];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    AppLayout {
        tab_bar: chunks[0],
        main_area: chunks[1],
        status_bar: chunks[2],
        search_bar: chunks[3],
    }
}

// --------------------------------------------------------------------------
// [SECTION] Tests
// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_calculation_standard() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = build_layout(area, true, true);

        assert_eq!(layout.tab_bar.height, 1);
        assert_eq!(layout.search_bar.height, 3);
        assert_eq!(layout.status_bar.height, 1);
        assert_eq!(layout.main_area.height, 19); // 24 - 1 - 3 - 1
    }

    #[test]
    fn test_layout_calculation_no_status_small_terminal() {
        let area = Rect::new(0, 0, 80, 8); // Height < 10
        let layout = build_layout(area, false, false);

        assert_eq!(layout.tab_bar.height, 0);
        assert_eq!(layout.search_bar.height, 0);
        assert_eq!(layout.status_bar.height, 0); // hidden
        assert_eq!(layout.main_area.height, 8);
    }
}
