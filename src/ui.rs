//! Top-level screen: tab bar, filter bar, body, status line (08 §1, §2, §6).
//!
//! Rendering is a pure function of `(&App, Rect, &mut Buffer)`: it never reads
//! the clock and writes no cell outside the given area (VOL-VIS-009/010).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, MIN_HEIGHT, MIN_WIDTH},
    model::ConnState,
    tabs,
    theme::Theme,
    widgets::{
        row_list::{LOCKED_ROW_HEIGHT, count_text},
        statusline::{StatusLine, default_label},
    },
};

/// The five tabs, in tab-bar order. Names and display strings are stable
/// (VOL-ARCH-004).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Display, FromRepr, EnumIter)]
pub enum SelectedTab {
    #[default]
    #[strum(to_string = "Playback")]
    PlaybackTab,
    #[strum(to_string = "Recording")]
    RecordingTab,
    #[strum(to_string = "Output Devices")]
    OutputDevicesTab,
    #[strum(to_string = "Input Devices")]
    InputDevicesTab,
    #[strum(to_string = "Configuration")]
    ConfigurationTab,
}

impl SelectedTab {
    /// The next tab, wrapping Configuration → Playback (VOL-KEY-003).
    pub fn next(self) -> Self {
        Self::from_repr((self as usize + 1) % Self::iter().count()).unwrap_or_default()
    }

    /// The previous tab, wrapping Playback → Configuration (VOL-KEY-003).
    pub fn previous(self) -> Self {
        let count = Self::iter().count();
        Self::from_repr((self as usize + count - 1) % count).unwrap_or_default()
    }
}

/// Divider between tab titles.
const DIVIDER: &str = " │ ";
/// Shown first when tabs are hidden on the left.
const MORE_LEFT: &str = "◂ ";
/// Shown last when tabs are hidden on the right.
const MORE_RIGHT: &str = " ▸";

/// One piece of the tab bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSegment {
    /// Text to draw.
    pub text: String,
    /// Index of the tab whose (possibly truncated) title this is; `None` for
    /// padding, dividers and overflow markers.
    pub tab: Option<usize>,
}

/// The tab bar as laid out for one width (08 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabBarLayout {
    /// Segments, left to right.
    pub segments: Vec<TabSegment>,
}

impl TabBarLayout {
    /// The bar as plain text.
    pub fn text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }

    fn push(&mut self, text: impl Into<String>, tab: Option<usize>) {
        self.segments.push(TabSegment {
            text: text.into(),
            tab,
        });
    }
}

/// Lays out the tab bar per 08 §2. Never wider than `width`.
///
/// If everything fits: ` A │ B │ C`. Otherwise the bar starts at the first
/// tab from which the selected title fits entirely, shows `◂ ` when tabs are
/// hidden on the left, fills the width (truncating the last title with `…`)
/// and ends with ` ▸` when tabs are hidden on the right.
pub fn tab_bar_layout(titles: &[String], selected: usize, width: u16) -> TabBarLayout {
    let width = usize::from(width);
    let count = titles.len();
    let selected = selected.min(count.saturating_sub(1));
    let cells = |s: &str| s.chars().count();
    let lead = |start: usize| if start > 0 { MORE_LEFT } else { " " };
    // Width from the lead up to the end of the selected title, plus ` ▸` if
    // tabs follow it.
    let needed = |start: usize| {
        let titles_width: usize = titles[start..=selected].iter().map(|t| cells(t)).sum();
        let dividers = (selected - start) * cells(DIVIDER);
        let more = if selected + 1 < count {
            cells(MORE_RIGHT)
        } else {
            0
        };
        cells(lead(start)) + titles_width + dividers + more
    };
    let full = 1
        + titles.iter().map(|t| cells(t)).sum::<usize>()
        + count.saturating_sub(1) * cells(DIVIDER);
    let start = if full <= width {
        0
    } else {
        (0..=selected)
            .find(|&s| needed(s) <= width)
            .unwrap_or(selected)
    };

    let mut layout = TabBarLayout {
        segments: Vec::new(),
    };
    layout.push(lead(start), None);
    let mut used = cells(lead(start));
    // Segment index of the last title drawn in full.
    let mut last_whole = None;
    let mut hidden_right = false;
    for (i, title) in titles.iter().enumerate().skip(start) {
        let divider = if i > start { DIVIDER } else { "" };
        let more = if i + 1 < count { cells(MORE_RIGHT) } else { 0 };
        let room = width.saturating_sub(used + cells(divider) + more);
        if cells(title) <= room {
            layout.push(divider, None);
            layout.push(title.as_str(), Some(i));
            last_whole = Some(layout.segments.len() - 1);
            used += cells(divider) + cells(title);
            continue;
        }
        // Cut at a word boundary; only the selected title is cut mid-word.
        let cut = word_prefix(title, room)
            .or_else(|| (i == selected && room > 0).then(|| truncate(title, room)));
        if let Some(cut) = &cut {
            layout.push(divider, None);
            layout.push(cut.as_str(), Some(i));
            last_whole = None;
        }
        hidden_right = cut.is_none() || i + 1 < count;
        break;
    }
    if hidden_right {
        // Shorten the last full title so the bar visibly continues.
        if let Some(segment) = last_whole.and_then(|k| layout.segments.get_mut(k))
            && segment.tab != Some(selected)
            && let Some(short) = word_prefix(&segment.text, cells(&segment.text).saturating_sub(1))
        {
            segment.text = short;
        }
        layout.push(MORE_RIGHT, None);
    }
    clip(layout, width)
}

/// The longest run of leading whole words of `title`, followed by `…`, that
/// is shorter than `title` and fits in `max` cells.
fn word_prefix(title: &str, max: usize) -> Option<String> {
    let words: Vec<&str> = title.split(' ').collect();
    (1..words.len())
        .rev()
        .map(|n| format!("{}…", words[..n].join(" ")))
        .find(|prefix| prefix.chars().count() <= max)
}

/// `text` cut to `max` cells, ending in `…` if anything was cut.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    if max > 0 {
        out.push('…');
    }
    out
}

/// Cuts the layout to `width` cells (only matters when even the lead and the
/// selected title do not fit).
fn clip(layout: TabBarLayout, width: usize) -> TabBarLayout {
    let mut left = width;
    let mut segments = Vec::new();
    for segment in layout.segments {
        if left == 0 {
            break;
        }
        let text: String = segment.text.chars().take(left).collect();
        left -= text.chars().count();
        segments.push(TabSegment { text, ..segment });
    }
    TabBarLayout { segments }
}

/// Renders the whole screen into `area`.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    if area.is_empty() {
        return;
    }
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(area, buf);
        return;
    }
    let theme = Theme::new(app.options.color);
    let [tab_bar, filter_bar, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);
    render_tab_bar(app, &theme, tab_bar, buf);
    render_filter_bar(app, body.height, filter_bar, buf);
    tabs::render(app.ui.tab, app, body, buf);
    StatusLine {
        conn: &app.store.conn,
        message: app.ui.status.as_ref(),
        now: app.ui.now,
        tab: app.ui.tab,
        default: default_label(&app.store, app.ui.tab),
        theme: &theme,
    }
    .render(status, buf);
}

/// Blank columns right of the filter-bar count (as in the 02 §2.3 mockups).
const COUNT_MARGIN: usize = 2;

/// Filter bar texts (VOL-FLT-001, VOL-ROW-003): `Show: <filter> (f)` for
/// tabs with a filter, and the row count (`3 streams`, or `2/9` when not all
/// rows fit in `body_height` lines) while connected. Empty when absent; only
/// Playback has rows and a filter so far.
pub fn filter_bar_text(app: &App, body_height: u16) -> (String, String) {
    if app.ui.tab != SelectedTab::PlaybackTab {
        return (String::new(), String::new());
    }
    let left = format!("Show: {} (f)", app.ui.playback_filter.name());
    let right = match app.store.conn {
        ConnState::Connected { .. } => {
            let heights = vec![LOCKED_ROW_HEIGHT; app.row_keys().len()];
            let info = app.ui.list(app.ui.tab).info(&heights, body_height);
            count_text(&info, "stream")
        }
        _ => String::new(),
    };
    (left, right)
}

/// ` <left>` at the left edge, `<right>` right-aligned before the margin.
fn render_filter_bar(app: &App, body_height: u16, area: Rect, buf: &mut Buffer) {
    let (left, right) = filter_bar_text(app, body_height);
    if !left.is_empty() {
        Line::raw(format!(" {left}")).render(area, buf);
    }
    let width = usize::from(area.width);
    let right_end = width.saturating_sub(COUNT_MARGIN);
    let x = right_end.saturating_sub(right.width());
    buf.set_stringn(
        area.x + u16::try_from(x).unwrap_or(0),
        area.y,
        &right,
        right_end - x,
        Style::new(),
    );
}

fn render_tab_bar(app: &App, theme: &Theme, area: Rect, buf: &mut Buffer) {
    let titles: Vec<String> = SelectedTab::iter().map(|t| t.to_string()).collect();
    let selected = app.ui.tab as usize;
    let spans: Vec<Span> = tab_bar_layout(&titles, selected, area.width)
        .segments
        .into_iter()
        .map(|segment| {
            let style = match segment.tab {
                Some(i) if i == selected => theme.tab_selected,
                Some(_) => theme.tab_normal,
                None => Style::new(),
            };
            Span::styled(segment.text, style)
        })
        .collect();
    Line::from(spans).render(area, buf);
}

/// `Terminal too small` / `need 40x12, have <W>x<H>` (VOL-VIS-001/002).
fn render_too_small(area: Rect, buf: &mut Buffer) {
    let have = format!(
        "need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
        area.width, area.height
    );
    render_centered(
        &[
            ("Terminal too small", Style::new()),
            (have.as_str(), Style::new()),
        ],
        area,
        buf,
    );
}

/// Draws `lines` centered in `area`; each line is cut to the area width,
/// never wrapped. Lines below the area are dropped.
pub(crate) fn render_centered(lines: &[(&str, Style)], area: Rect, buf: &mut Buffer) {
    let count = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let top = area.y + area.height.saturating_sub(count).div_ceil(2);
    for (y, (text, style)) in (top..area.bottom()).zip(lines) {
        let text = truncate_plain(text, usize::from(area.width));
        let len = u16::try_from(text.chars().count()).unwrap_or(area.width);
        let x = area.x + (area.width - len) / 2;
        buf.set_stringn(x, y, &text, usize::from(len), *style);
    }
}

/// `text` cut to `max` cells, without an ellipsis.
fn truncate_plain(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn app() -> App {
        let options = crate::app::Options {
            color: true,
            mouse: false,
        };
        App::new(
            options,
            crate::backend::BackendHandle::inert(),
            std::time::Instant::now(),
        )
    }

    fn connected_with_streams(count: u32) -> App {
        use crate::{
            backend::{BackendEvent, StoreEvent},
            event::Event,
            model::{ObjectId, Serial},
        };
        let mut app = app();
        app.store.set_conn(ConnState::Connected {
            server_version: "1.6.9".to_owned(),
        });
        let streams = (1..=count)
            .map(|id| StoreEvent::NodeAdded {
                id: ObjectId(id),
                serial: Serial(u64::from(id)),
                props: [("media.class", "Stream/Output/Audio")]
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            })
            .collect();
        app.on_event(Event::Backend(BackendEvent::Batch(streams)));
        app
    }

    fn screen_line(app: &App, width: u16, height: u16, y: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        render(app, area, &mut buf);
        (0..width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn filter_bar_text_playback_default() {
        let app = app();
        assert_eq!(
            filter_bar_text(&app, 21),
            ("Show: Applications (f)".to_owned(), String::new()),
            "no count before the first connection"
        );
        let line = screen_line(&app, 80, 24, 1);
        assert_eq!(line.trim_end(), " Show: Applications (f)");

        let mut app = connected_with_streams(0);
        assert_eq!(filter_bar_text(&app, 21).1, "0 streams");
        app.update(crate::action::Action::FilterNext);
        assert_eq!(filter_bar_text(&app, 21).0, "Show: All Streams (f)");
        app.update(crate::action::Action::FilterNext);
        assert_eq!(filter_bar_text(&app, 21).0, "Show: Virtual Streams (f)");

        // Tabs without rows yet show an empty filter bar.
        app.update(crate::action::Action::NextTab);
        assert_eq!(filter_bar_text(&app, 21), (String::new(), String::new()));
    }

    #[test]
    fn filter_bar_right_count_pluralization() {
        assert_eq!(
            filter_bar_text(&connected_with_streams(1), 21).1,
            "1 stream"
        );
        let app = connected_with_streams(3);
        assert_eq!(filter_bar_text(&app, 21).1, "3 streams");
        let line = screen_line(&app, 80, 24, 1);
        assert_eq!(
            line,
            format!(" Show: Applications (f){}3 streams  ", " ".repeat(46))
        );
        // Not all rows fit: position of the selected row.
        assert_eq!(filter_bar_text(&connected_with_streams(9), 21).1, "1/9");
        assert!(screen_line(&connected_with_streams(9), 40, 12, 1).ends_with(" 1/9  "));
    }

    fn titles() -> Vec<String> {
        SelectedTab::iter().map(|t| t.to_string()).collect()
    }

    fn bar(selected: usize, width: u16) -> String {
        tab_bar_layout(&titles(), selected, width).text()
    }

    #[test]
    fn tab_bar_fits_at_80() {
        let expected = " Playback │ Recording │ Output Devices │ Input Devices │ Configuration";
        for selected in 0..5 {
            assert_eq!(bar(selected, 80), expected);
        }
        let layout = tab_bar_layout(&titles(), 2, 80);
        let selected: Vec<_> = layout
            .segments
            .iter()
            .filter(|s| s.tab == Some(2))
            .collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].text, "Output Devices");
    }

    #[test]
    fn tab_bar_overflow_at_40_selected_first() {
        // The 08 §2 example: tabs stay hidden, so the last title is shortened.
        assert_eq!(bar(0, 40), " Playback │ Recording │ Output… ▸");
        // Narrower: "Output Devices" does not fit and is cut at a word boundary.
        assert_eq!(bar(0, 33), " Playback │ Recording │ Output… ▸");
        // Wider: "Input Devices" is cut, "Configuration" hidden.
        assert_eq!(
            bar(0, 50),
            " Playback │ Recording │ Output Devices │ Input… ▸"
        );
        assert_eq!(
            bar(0, 60),
            " Playback │ Recording │ Output Devices │ Input… ▸"
        );
    }

    #[test]
    fn tab_bar_scrolls_so_selected_fits() {
        assert_eq!(bar(4, 40), "◂ Input Devices │ Configuration");
        // "Configuration" has no word that fits: hidden. The selected title
        // is never shortened.
        assert_eq!(bar(3, 40), "◂ Output Devices │ Input Devices ▸");
        assert_eq!(bar(1, 40), " Playback │ Recording │ Output… ▸");
        // A single-word title cannot be shortened at a word boundary.
        assert_eq!(bar(0, 32), " Playback │ Recording ▸");
        // Narrower than lead + selected title: the selected title is cut.
        assert_eq!(bar(2, 10), "◂ Outpu… ▸");
    }

    #[test]
    fn selected_tab_titles_are_stable() {
        assert_eq!(
            titles(),
            [
                "Playback",
                "Recording",
                "Output Devices",
                "Input Devices",
                "Configuration"
            ]
        );
    }

    proptest! {
        #[test]
        fn tab_bar_never_exceeds_width(width in 1u16..=120, selected in 0usize..5) {
            let layout = tab_bar_layout(&titles(), selected, width);
            let text = layout.text();
            prop_assert!(Line::raw(text.as_str()).width() <= usize::from(width), "{:?}", text);
            prop_assert!(text.chars().count() <= usize::from(width), "{:?}", text);
            // From the 40-column minimum on, the selected title is always shown.
            prop_assert!(width < MIN_WIDTH || layout.segments.iter().any(|s| s.tab == Some(selected)), "{:?}", text);
        }
    }
}
