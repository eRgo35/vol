use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Tabs, Widget},
};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use crate::{
    app::App,
    tabs::{
        configuration::configuration_tab, input_devices::input_devices_tab,
        output_devices::output_devices_tab, playback::playback_tab, recording::recording_tab,
    },
};

#[derive(Default, Debug, Clone, Copy, Display, FromRepr, EnumIter)]
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

impl Widget for &App {
    /// Renders the user interface widgets.
    ///
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui/ratatui/tree/master/examples
    fn render(self, area: Rect, buf: &mut Buffer) {
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
            .select(self.selected_tab as usize)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            );

        tabs.render(chunks[0], buf);

        // Render the content for the selected tab
        let content_block = Block::new();

        let text = match self.selected_tab {
            SelectedTab::PlaybackTab => playback_tab(self),
            SelectedTab::RecordingTab => recording_tab(self),
            SelectedTab::OutputDevicesTab => output_devices_tab(self),
            SelectedTab::InputDevicesTab => input_devices_tab(self),
            SelectedTab::ConfigurationTab => configuration_tab(self),
        };

        let paragraph = Paragraph::new(text).block(content_block).centered();

        paragraph.render(chunks[1], buf);
    }
}
