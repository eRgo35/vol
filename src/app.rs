//! Application state and the draw loop (01 §3, VOL-ARCH-033).

use std::time::Instant;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::Event as CrosstermEvent;

use crate::{
    action::Action,
    backend::AudioBackend,
    event::{Event, EventHandler},
    keymap::map_key,
    ui::{self, SelectedTab},
    ui_state::{Overlay, UiState},
};

/// Minimum terminal width (08 §6).
pub const MIN_WIDTH: u16 = 40;
/// Minimum terminal height (08 §6).
pub const MIN_HEIGHT: u16 = 12;

/// Startup options derived from the command line and environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Use colors (`false` = mono theme, `NO_COLOR`/`--no-color`).
    pub color: bool,
    /// `--mouse` was passed (mouse capture arrives in M8).
    pub mouse: bool,
}

/// Application state. Pure: no terminal, no runtime, no clock reads.
pub struct App {
    /// Cleared by [`Action::Quit`] and [`Event::Quit`].
    pub running: bool,
    /// UI-only state.
    pub ui: UiState,
    /// Command side of the audio backend.
    pub backend: Box<dyn AudioBackend>,
    /// Something changed since the last draw.
    pub dirty: bool,
    /// Startup options.
    pub options: Options,
}

impl App {
    /// A running app on the Playback tab that still needs its first frame.
    pub fn new(options: Options, backend: Box<dyn AudioBackend>, now: Instant) -> Self {
        Self {
            running: true,
            ui: UiState::new(now),
            backend,
            dirty: true,
            options,
        }
    }

    /// Whether the next tick should draw.
    pub fn needs_draw(&self) -> bool {
        self.dirty
    }

    /// Applies one event to the state.
    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::Tick => {}
            Event::Crossterm(CrosstermEvent::Key(key)) => {
                if let Some(action) = map_key(&self.ui, key) {
                    self.update(action);
                }
            }
            Event::Crossterm(CrosstermEvent::Resize(width, height)) => self.resize(width, height),
            Event::Crossterm(_) => {}
            // The store arrives in M1; for now any backend event may change the screen.
            Event::Backend(_) => self.dirty = true,
            Event::Quit => self.running = false,
        }
    }

    /// Applies one user action to the state.
    pub fn update(&mut self, action: Action) {
        match action {
            Action::Quit => self.running = false,
            Action::NextTab => self.go_tab(self.ui.tab.next()),
            Action::PrevTab => self.go_tab(self.ui.tab.previous()),
            Action::GoTab(tab) => self.go_tab(tab),
            Action::CloseOverlay => {
                if self.ui.overlay == Overlay::None {
                    self.ui.status = None;
                } else {
                    self.ui.overlay = Overlay::None;
                }
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// Records a new terminal size (VOL-ARCH-064, VOL-VIS-001).
    pub fn resize(&mut self, width: u16, height: u16) {
        self.ui.terminal_too_small = width < MIN_WIDTH || height < MIN_HEIGHT;
        self.dirty = true;
    }

    /// Runs until quit: handles every event as it arrives and draws only on
    /// ticks, and only when something changed (VOL-ARCH-033).
    ///
    /// # Errors
    ///
    /// Fails if the terminal cannot be queried or drawn, or if every event
    /// source stopped.
    pub async fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
        events: &mut EventHandler,
    ) -> color_eyre::Result<()> {
        let size = terminal.size()?;
        self.resize(size.width, size.height);
        while self.running {
            let event = events.next().await?;
            let tick = matches!(event, Event::Tick);
            self.on_event(event);
            if tick && self.running && self.needs_draw() {
                terminal.draw(|frame| ui::render(self, frame.area(), frame.buffer_mut()))?;
                self.dirty = false;
            }
        }
        Ok(())
    }

    fn go_tab(&mut self, tab: SelectedTab) {
        self.ui.tab = tab;
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use strum::IntoEnumIterator;

    use super::*;
    use crate::backend::{BackendEvent, BackendHandle};

    fn app() -> App {
        let options = Options {
            color: true,
            mouse: false,
        };
        App::new(options, BackendHandle::inert().commands, Instant::now())
    }

    /// An app whose first frame has been drawn.
    fn drawn() -> App {
        let mut app = app();
        app.dirty = false;
        app
    }

    fn key(app: &mut App, code: KeyCode) {
        app.on_event(Event::Crossterm(CrosstermEvent::Key(KeyEvent::new(
            code,
            KeyModifiers::NONE,
        ))));
    }

    fn resize(app: &mut App, width: u16, height: u16) {
        app.on_event(Event::Crossterm(CrosstermEvent::Resize(width, height)));
    }

    #[test]
    fn q_stops_running() {
        let mut app = app();
        key(&mut app, KeyCode::Char('q'));
        assert!(!app.running);
    }

    #[test]
    fn esc_keeps_running() {
        let mut app = app();
        key(&mut app, KeyCode::Esc);
        assert!(app.running);
    }

    #[test]
    fn esc_without_overlay_clears_status_message() {
        let mut app = drawn();
        app.ui.status = Some(crate::ui_state::StatusMsg {
            text: "Channels locked".to_owned(),
            kind: crate::ui_state::StatusKind::Info,
            expires_at: app.ui.now,
        });
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.ui.status, None);
        assert!(app.needs_draw());
    }

    #[test]
    fn esc_closes_overlay_and_keeps_status() {
        let mut app = drawn();
        app.ui.overlay = Overlay::Help { scroll: 0 };
        app.ui.status = Some(crate::ui_state::StatusMsg {
            text: "x".to_owned(),
            kind: crate::ui_state::StatusKind::Info,
            expires_at: app.ui.now,
        });
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.ui.overlay, Overlay::None);
        assert!(app.ui.status.is_some());
        assert!(app.running);
    }

    #[test]
    fn tab_switch_marks_dirty_and_changes_tab() {
        let mut app = drawn();
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.ui.tab, SelectedTab::RecordingTab);
        assert!(app.needs_draw());
    }

    #[test]
    fn tabs_wrap_around() {
        let mut app = app();
        key(&mut app, KeyCode::BackTab);
        assert_eq!(app.ui.tab, SelectedTab::ConfigurationTab);
        key(&mut app, KeyCode::Char(']'));
        assert_eq!(app.ui.tab, SelectedTab::PlaybackTab);
    }

    #[test]
    fn tick_without_change_does_not_need_draw() {
        let mut app = drawn();
        app.on_event(Event::Tick);
        assert!(!app.needs_draw());
    }

    #[test]
    fn first_frame_needs_draw() {
        assert!(app().needs_draw());
    }

    #[test]
    fn resize_marks_dirty() {
        let mut app = drawn();
        resize(&mut app, 100, 30);
        assert!(app.needs_draw());
    }

    #[test]
    fn resize_below_40x12_sets_too_small() {
        for (w, h) in [(39, 12), (40, 11), (0, 0)] {
            let mut app = app();
            resize(&mut app, w, h);
            assert!(app.ui.terminal_too_small, "{w}x{h}");
        }
    }

    #[test]
    fn resize_to_40x12_clears_too_small() {
        let mut app = app();
        resize(&mut app, 39, 11);
        resize(&mut app, 40, 12);
        assert!(!app.ui.terminal_too_small);
    }

    #[test]
    fn backend_event_marks_dirty() {
        let mut app = drawn();
        app.on_event(Event::Backend(BackendEvent::Batch(Vec::new())));
        assert!(app.needs_draw());
    }

    #[test]
    fn quit_event_stops_running() {
        let mut app = app();
        app.on_event(Event::Quit);
        assert!(!app.running);
    }

    #[test]
    fn default_tab_is_playback() {
        assert_eq!(app().ui.tab, SelectedTab::PlaybackTab);
    }

    #[test]
    fn f_keys_switch_tabs() {
        let mut app = app();
        for (n, tab) in (1..=5).zip(SelectedTab::iter()) {
            key(&mut app, KeyCode::F(n));
            assert_eq!(app.ui.tab, tab, "F{n}");
        }
    }
}
