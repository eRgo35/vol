//! Playback tab: application streams (02 §3).

use ratatui::{buffer::Buffer, layout::Rect};

use crate::{
    app::App,
    model::view::{StreamFilter, UiPending, playback_rows},
    theme::Theme,
    ui::{SelectedTab, render_body_text},
};

/// Renders the Playback tab body into `area`.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    let rows = playback_rows(&app.store, app.ui.playback_filter, &UiPending);
    if rows.is_empty() {
        render_empty(app, area, buf);
        return;
    }
    super::render_stub(app, SelectedTab::PlaybackTab, area, buf);
}

/// Empty state (VOL-PB-004): `No playback streams`, or, when the filter hides
/// streams, how many.
fn render_empty(app: &App, area: Rect, buf: &mut Buffer) {
    let theme = Theme::new(app.options.color);
    let hidden = playback_rows(&app.store, StreamFilter::All, &UiPending).len();
    let (title, hint) = match hidden {
        0 => (
            "No playback streams".to_owned(),
            "Applications that play audio appear here.",
        ),
        n => (
            format!("No streams match this filter ({n} hidden)"),
            "Press f to change the filter.",
        ),
    };
    render_body_text(&[(&title, theme.hint), (hint, theme.hint)], area, buf);
}
