use std::task::Wake;

use color_eyre::{
    Result,
    owo_colors::{OwoColorize, colors::css::LemonChiffon},
};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::{FutureExt, StreamExt, future::Lazy};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize, palette::tailwind},
    symbols,
    text::Line,
    widgets::{Block, Borders, Gauge, LineGauge, Padding, Tabs},
};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = App::new().run(terminal).await;
    ratatui::restore();
    result
}

#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    // Event stream.
    event_stream: EventStream,
    selected_tab: SelectedTab,
    active_output: usize,
    volume: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy, Display, FromRepr, EnumIter)]
enum SelectedTab {
    #[default]
    #[strum(to_string = "App Outputs")]
    Tab1,

    #[strum(to_string = "App Inputs")]
    Tab2,

    #[strum(to_string = "Device Outputs")]
    Tab3,

    #[strum(to_string = "Device Inputs")]
    Tab4,
}

#[derive(Debug, Default, Clone, Copy, Display, FromRepr, EnumIter)]
enum SelectedOutput {
    #[default]
    Output1,
    Output2,
    Output3,
}

impl SelectedOutput {
    fn previous(self) -> Self {
        let current_index: usize = self as usize;
        let previous_index = current_index.saturating_sub(1);
        Self::from_repr(previous_index).unwrap_or(self)
    }

    fn next(self) -> Self {
        let current_index: usize = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
    }
}

impl SelectedTab {
    fn previous(self) -> Self {
        let current_index: usize = self as usize;
        let previous_index = current_index.saturating_sub(1);
        Self::from_repr(previous_index).unwrap_or(self)
    }

    fn next(self) -> Self {
        let current_index: usize = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
    }
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        self.volume = vec![50, 75, 100];
        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_crossterm_events().await?;
        }
        Ok(())
    }

    /// Renders the user interface.
    ///
    /// This is where you add new widgets. See the following resources for more information:
    /// - <https://docs.rs/ratatui/latest/ratatui/widgets/index.html>
    /// - <https://github.com/ratatui/ratatui/tree/master/examples>
    fn draw(&mut self, frame: &mut Frame) {
        // let titles = SelectedTab::iter().map(SelectedTab::title);

        use Constraint::{Length, Min, Ratio};
        let layout = Layout::vertical([Length(4), Constraint::Fill(1), Length(2)])
            .spacing(1)
            .margin(1);
        let [title_area, body_area, footer_area] = layout.areas(frame.area());

        let layout = Layout::vertical([Length(4); 3]).spacing(1);
        let [vol1, vol2, vol3] = layout.areas(body_area);

        let layout = Layout::vertical([Length(2), Length(2)]);
        let [header, menu] = layout.areas(title_area);

        // let titles = SelectedTab::iter().map(SelectedTab::title);
        // let highlight_style = (Color::default(), self.selected_tab.palette().c700);
        // let selected_tab_index = self.selected_tab as usize;
        /*
                let tabs1 = Tabs::new(titles)
                    .highlight_style(highlight_style)
                    .select(selected_tab_index)
                    .padding("", "")
                    .divider(" ");
                let tabs2 = Tabs::new(titles)
                    .highlight_style(highlight_style)
                    .select(selected_tab_index)
                    .padding("", "")
                    .divider(" ");
                let tabs3 = Tabs::new(titles)
                    .highlight_style(highlight_style)
                    .select(selected_tab_index)
                    .padding("", "")
                    .divider(" ");
        */
        let title = Line::from("Volume Control").centered().bold();
        let footer = Line::from("Some Audio Output Information")
            .alignment(Alignment::Right)
            .bold();

        let name1 = Line::from("Speakers Lorem Ipsum");
        let name2 = Line::from("Headphones dolor sit");
        let name3 = Line::from("Digital Output amet");

        let mut name = [name1, name2, name3];

        let newName = name[self.active_output].clone().bold();
        name[self.active_output] = newName;

        let volume_bar1 = LineGauge::default()
            .block(title_block(name[0].clone()))
            .filled_style(Style::new().white().on_black().bold())
            .line_set(symbols::line::DOUBLE)
            .ratio(self.volume[0] as f64 / 100.0);

        let volume_bar2 = LineGauge::default()
            .block(title_block(name[1].clone()))
            .filled_style(Style::new().white().on_black().bold())
            .line_set(symbols::line::DOUBLE)
            .ratio(self.volume[1] as f64 / 100.0);

        let volume_bar3 = LineGauge::default()
            .block(title_block(name[2].clone()))
            .filled_style(Style::new().white().on_black().bold())
            .line_set(symbols::line::DOUBLE)
            .ratio(self.volume[2] as f64 / 100.0);

        frame.render_widget(title, header);
        frame.render_widget(volume_bar1, vol1);
        frame.render_widget(volume_bar2, vol2);
        frame.render_widget(volume_bar3, vol3);
        frame.render_widget(footer, footer_area);
    }

    /// Reads the crossterm events and updates the state of [`App`].
    async fn handle_crossterm_events(&mut self) -> Result<()> {
        tokio::select! {
            event = self.event_stream.next().fuse() => {
                match event {
                    Some(Ok(evt)) => {
                        match evt {
                            Event::Key(key)
                                if key.kind == KeyEventKind::Press
                                    => self.on_key_event(key),
                            Event::Mouse(_) => {}
                            Event::Resize(_, _) => {}
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Sleep for a short duration to avoid busy waiting.
            }
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            (_, KeyCode::Char('l')) => self.next_tab(),
            (_, KeyCode::Char('h')) => self.previous_tab(),
            (_, KeyCode::Char('j') | KeyCode::Up) => self.previous_output(),
            (_, KeyCode::Char('k') | KeyCode::Down) => self.next_output(),
            (_, KeyCode::Right) => self.volume_up(),
            (_, KeyCode::Left) => self.volume_down(),
            // Add other key handlers here.
            _ => {}
        }
    }

    pub fn next_tab(&mut self) {
        self.selected_tab = self.selected_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.selected_tab = self.selected_tab.previous();
    }

    pub fn next_output(&mut self) {
        if self.active_output < 2 {
            self.active_output += 1;
        }
    }

    pub fn previous_output(&mut self) {
        if self.active_output > 0 {
            self.active_output -= 1;
        }
    }

    pub fn volume_up(&mut self) {
        if self.volume[self.active_output] < 100 {
            self.volume[self.active_output] += 5;
        }
    }

    pub fn volume_down(&mut self) {
        if self.volume[self.active_output] > 0 {
            self.volume[self.active_output] -= 5;
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}

fn title_block(title: Line) -> Block {
    Block::new()
        .borders(Borders::NONE)
        .padding(Padding::vertical(1))
        .title(title)
        .fg(tailwind::SLATE.c200)
}
