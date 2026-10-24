//! Tab bodies. Each tab renders into the body area of the screen.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::{
    app::App,
    theme::Theme,
    ui::{SelectedTab, render_centered},
};

pub mod configuration;
pub mod input_devices;
pub mod output_devices;
pub mod playback;
pub mod recording;

/// Renders the body of `tab` into `area`.
pub fn render(tab: SelectedTab, app: &App, area: Rect, buf: &mut Buffer) {
    match tab {
        SelectedTab::PlaybackTab => playback::render(app, area, buf),
        SelectedTab::RecordingTab => recording::render(app, area, buf),
        SelectedTab::OutputDevicesTab => output_devices::render(app, area, buf),
        SelectedTab::InputDevicesTab => input_devices::render(app, area, buf),
        SelectedTab::ConfigurationTab => configuration::render(app, area, buf),
    }
}

/// Placeholder body: the tab name and a dim `(not implemented yet)`, centered.
fn render_stub(app: &App, tab: SelectedTab, area: Rect, buf: &mut Buffer) {
    let theme = Theme::new(app.options.color);
    let name = tab.to_string();
    render_centered(
        &[
            (name.as_str(), Style::new()),
            ("(not implemented yet)", theme.hint),
        ],
        area,
        buf,
    );
}
