//! Input Devices tab (stub until its milestone).

use ratatui::{buffer::Buffer, layout::Rect};

use crate::{app::App, ui::SelectedTab};

/// Renders the Input Devices tab body into `area`.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    super::render_stub(app, SelectedTab::InputDevicesTab, area, buf);
}
