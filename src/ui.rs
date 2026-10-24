use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Tabs, Widget},
};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use crate::{
    app::App,
    tabs::{
        configuration::configuration_tab, input_devices::input_devices_tab,
        output_devices::output_devices_tab, playback::playback_tab, recording::recording_tab,
    },
};

/// The five tabs, in tab-bar order. Names and display strings are stable
/// (VOL-ARCH-004).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Display, FromRepr, EnumIter)]
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

/// Renders the whole screen.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    // Split the layout into tabs area and content area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(area);

    // Render the tabs
    let tab_titles: Vec<_> = SelectedTab::iter()
        .map(|tab| Line::from(Span::styled(tab.to_string(), Style::default())))
        .collect();

    let tabs = Tabs::new(tab_titles)
        .block(Block::default())
        .select(app.ui.tab as usize)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        );

    tabs.render(chunks[0], buf);

    // Render the content for the selected tab
    let content_block = Block::new();

    let text = match app.ui.tab {
        SelectedTab::PlaybackTab => playback_tab(app),
        SelectedTab::RecordingTab => recording_tab(app),
        SelectedTab::OutputDevicesTab => output_devices_tab(app),
        SelectedTab::InputDevicesTab => input_devices_tab(app),
        SelectedTab::ConfigurationTab => configuration_tab(app),
    };

    let paragraph = Paragraph::new(text).block(content_block).centered();

    paragraph.render(chunks[1], buf);
}
