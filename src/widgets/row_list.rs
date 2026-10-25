//! Scrolling row list: selection by [`RowKey`], viewport, count (02 §2.3,
//! VOL-ROW-002/003/004).
//!
//! The list knows only row keys and row heights; each row is drawn by a
//! caller-supplied function.

use std::ops::Range;

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::model::view::RowKey;

/// Height of a row with locked channels (02 §2.3).
pub const LOCKED_ROW_HEIGHT: u16 = 3;

/// Selection and viewport of one tab's list (VOL-ROW-004). Session-only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListState {
    /// Selected row; `None` only while the list is empty.
    pub selected: Option<RowKey>,
    /// Index of the selected row (or of the last one, while empty).
    pub last_index: usize,
    /// Index of the first drawn row.
    pub offset: usize,
}

impl ListState {
    /// Re-finds the selection in `keys` (VOL-ROW-004): the selected key keeps
    /// its row wherever it moved; if it disappeared, the row at
    /// `min(last_index, len − 1)` is selected; an empty list has no selection.
    pub fn reselect(&mut self, keys: &[RowKey]) {
        let found = self
            .selected
            .and_then(|key| keys.iter().position(|k| *k == key));
        match found {
            Some(index) => self.last_index = index,
            None => self.select_index(keys, self.last_index),
        }
    }

    /// Selects the row at `index`, clamped to the list (no selection if empty).
    pub fn select_index(&mut self, keys: &[RowKey], index: usize) {
        match keys.len().checked_sub(1) {
            Some(last) => {
                self.last_index = index.min(last);
                self.selected = Some(keys[self.last_index]);
            }
            None => self.selected = None,
        }
    }

    /// Scrolls minimally so the selected row is fully visible in a body of
    /// `body_height` lines (VOL-ROW-002). `heights` holds every row's height.
    /// If rows above the viewport would fit below it, it scrolls back so the
    /// body is not left partly empty.
    pub fn scroll_into_view(&mut self, heights: &[u16], body_height: u16) {
        if self.selected.is_none() || heights.is_empty() {
            self.offset = 0;
            return;
        }
        let selected = self.last_index.min(heights.len() - 1);
        let span = |rows: &[u16]| rows.iter().map(|&h| u32::from(h)).sum::<u32>();
        let body = u32::from(body_height);
        self.offset = self.offset.min(selected);
        while self.offset < selected && span(&heights[self.offset..=selected]) > body {
            self.offset += 1;
        }
        while self.offset > 0 && span(&heights[self.offset - 1..]) <= body {
            self.offset -= 1;
        }
    }

    /// Rows drawn from [`Self::offset`]: those that fit entirely in
    /// `body_height` lines, or just the first one (clipped) if even it does
    /// not fit (VOL-ROW-002).
    pub fn visible(&self, heights: &[u16], body_height: u16) -> Range<usize> {
        let start = self.offset.min(heights.len());
        let mut used = 0u32;
        let fitting = heights[start..]
            .iter()
            .take_while(|&&h| {
                used += u32::from(h);
                used <= u32::from(body_height)
            })
            .count();
        let count = if start < heights.len() {
            fitting.max(1)
        } else {
            0
        };
        start..start + count
    }

    /// Counts for the filter bar (VOL-ROW-003).
    pub fn info(&self, heights: &[u16], body_height: u16) -> ListInfo {
        ListInfo {
            shown: self.visible(heights, body_height).len(),
            total: heights.len(),
            selected_pos: self.selected.map(|_| self.last_index + 1),
        }
    }
}

/// How much of a list is on screen (VOL-ROW-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListInfo {
    /// Rows drawn.
    pub shown: usize,
    /// Rows in the list.
    pub total: usize,
    /// 1-based position of the selected row.
    pub selected_pos: Option<usize>,
}

/// Filter-bar count: `<sel>/<total>` when not all rows are drawn, else
/// `<total> <noun>` with a plural `s` unless `total == 1` (VOL-ROW-003).
pub fn count_text(info: &ListInfo, noun: &str) -> String {
    match info.selected_pos {
        Some(pos) if info.shown < info.total => format!("{pos}/{}", info.total),
        _ if info.total == 1 => format!("1 {noun}"),
        _ => format!("{} {noun}s", info.total),
    }
}

/// Draws the visible rows of a list top to bottom into its area.
pub struct RowList<'a, F> {
    /// Selection and viewport.
    pub state: &'a ListState,
    /// Height of every row.
    pub heights: &'a [u16],
    /// Draws row `index` into `area`; the flag says whether it is selected.
    /// The area is shorter than the row only for a clipped selected row; the
    /// function must not write outside it.
    pub draw_row: F,
}

impl<F: FnMut(usize, bool, Rect, &mut Buffer)> Widget for RowList<'_, F> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut y = area.y;
        for index in self.state.visible(self.heights, area.height) {
            let height = self.heights[index].min(area.bottom() - y);
            let selected = self.state.selected.is_some() && index == self.state.last_index;
            (self.draw_row)(index, selected, Rect { y, height, ..area }, buf);
            y += height;
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::*;
    use crate::model::Serial;

    fn keys(serials: &[u64]) -> Vec<RowKey> {
        serials.iter().map(|&s| RowKey::Stream(Serial(s))).collect()
    }

    fn selected_on(serials: &[u64], index: usize) -> ListState {
        let mut state = ListState::default();
        state.select_index(&keys(serials), index);
        state
    }

    /// Renders `rows` 3-line rows into a `width`×`height` buffer; each row
    /// writes its index (and `*` if selected) on each of its lines.
    fn draw(state: &ListState, rows: usize, height: u16) -> Vec<String> {
        let heights = vec![LOCKED_ROW_HEIGHT; rows];
        let area = Rect::new(0, 0, 4, height);
        let mut buf = Buffer::empty(area);
        RowList {
            state,
            heights: &heights,
            draw_row: |index: usize, selected: bool, area: Rect, buf: &mut Buffer| {
                let mark = if selected { "*" } else { "" };
                for y in area.top()..area.bottom() {
                    buf.set_string(area.x, y, format!("{index}{mark}"), Style::new());
                }
            },
        }
        .render(area, &mut buf);
        (0..height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn selection_survives_reorder() {
        let mut state = selected_on(&[10, 20, 30], 1);
        state.reselect(&keys(&[5, 30, 10, 20]));
        assert_eq!(state.selected, Some(RowKey::Stream(Serial(20))));
        assert_eq!(state.last_index, 3);
    }

    #[test]
    fn selection_moves_to_clamped_index_when_key_vanishes() {
        let mut state = selected_on(&[10, 20, 30], 1);
        state.reselect(&keys(&[10, 30]));
        assert_eq!(state.selected, Some(RowKey::Stream(Serial(30))));
        assert_eq!(state.last_index, 1);

        let mut state = selected_on(&[10, 20, 30], 2);
        state.reselect(&keys(&[10, 20]));
        assert_eq!(state.selected, Some(RowKey::Stream(Serial(20))));
        assert_eq!(state.last_index, 1);
    }

    #[test]
    fn empty_list_has_no_selection() {
        let mut state = selected_on(&[10, 20], 1);
        state.reselect(&[]);
        assert_eq!(state.selected, None);
        state.scroll_into_view(&[], 9);
        assert_eq!(state.offset, 0);
        assert_eq!(draw(&state, 0, 6), vec![""; 6]);

        // Rows that appear later get selected again.
        state.reselect(&keys(&[7]));
        assert_eq!(state.selected, Some(RowKey::Stream(Serial(7))));
    }

    #[test]
    fn scroll_keeps_selected_fully_visible_for_3_line_rows() {
        let all = keys(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let heights = vec![LOCKED_ROW_HEIGHT; all.len()];
        let mut state = ListState::default();
        let select = |state: &mut ListState, index: usize| {
            state.select_index(&all, index);
            state.scroll_into_view(&heights, 9);
            state.offset
        };
        // Body of 9 lines = 3 rows: moving down scrolls one row at a time.
        assert_eq!(select(&mut state, 0), 0);
        assert_eq!(select(&mut state, 2), 0);
        assert_eq!(select(&mut state, 3), 1);
        assert_eq!(select(&mut state, 5), 3);
        // Moving up inside the viewport does not scroll; above it scrolls minimally.
        assert_eq!(select(&mut state, 3), 3);
        assert_eq!(select(&mut state, 2), 2);
        assert_eq!(select(&mut state, 9), 7);
        assert_eq!(draw(&state, 10, 9)[6], "9*");

        // A taller body shows the rows above again instead of leaving space empty.
        state.scroll_into_view(&heights, 30);
        assert_eq!(state.offset, 0);
    }

    #[test]
    fn partial_rows_not_drawn() {
        let state = selected_on(&[1, 2, 3, 4], 0);
        // 10 lines hold three 3-line rows; the fourth would be cut, so it is not drawn.
        let lines = draw(&state, 4, 10);
        assert_eq!(lines, ["0*", "0*", "0*", "1", "1", "1", "2", "2", "2", ""]);
        assert_eq!(state.visible(&[3; 4], 10), 0..3);

        // A selected row taller than the body is clipped at the bottom.
        assert_eq!(draw(&state, 4, 2), ["0*", "0*"]);
    }

    #[test]
    fn count_text_all_fit_vs_position() {
        let mut state = selected_on(&[1, 2, 3], 1);
        let fits = state.info(&[3; 3], 9);
        assert_eq!(fits.shown, 3);
        assert_eq!(count_text(&fits, "stream"), "3 streams");
        let one = selected_on(&[1], 0).info(&[3], 9);
        assert_eq!(count_text(&one, "stream"), "1 stream");
        let none = ListState::default().info(&[], 9);
        assert_eq!(count_text(&none, "device"), "0 devices");

        state.select_index(&keys(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 1);
        let cut = state.info(&[3; 9], 21 - 1);
        assert_eq!(
            cut,
            ListInfo {
                shown: 6,
                total: 9,
                selected_pos: Some(2)
            }
        );
        assert_eq!(count_text(&cut, "stream"), "2/9");
    }
}
