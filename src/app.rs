//! Application state and the draw loop (01 §3, VOL-ARCH-033).

use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::Event as CrosstermEvent;

use crate::{
    action::Action,
    backend::{BackendError, BackendEvent, BackendHandle, CommandKind, StoreEvent},
    event::{Event, EventHandler},
    keymap::map_key,
    log,
    model::{
        ConnState, Defaults, NodeKind, Store, parse_default_name,
        view::{RowKey, UiPending, playback_rows},
    },
    ui::{self, SelectedTab},
    ui_state::{Overlay, StatusKind, UiState},
    widgets::{row_list::LOCKED_ROW_HEIGHT, statusline::Msg},
};

/// Minimum terminal width (08 §6).
pub const MIN_WIDTH: u16 = 40;
/// Minimum terminal height (08 §6).
pub const MIN_HEIGHT: u16 = 12;

/// Minimum time between two store summaries in the debug log.
const SUMMARY_INTERVAL: Duration = Duration::from_secs(1);

/// Startup options derived from the command line and environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Use colors (`false` = mono theme, `NO_COLOR`/`--no-color`).
    pub color: bool,
    /// `--mouse` was passed (mouse capture arrives in M8).
    pub mouse: bool,
}

/// Application state. Pure: no terminal, no runtime, no clock reads (time
/// arrives with [`Event::Tick`]).
pub struct App {
    /// Cleared by [`Action::Quit`] and [`Event::Quit`].
    pub running: bool,
    /// UI-only state.
    pub ui: UiState,
    /// PipeWire-derived state (VOL-ARCH-020).
    pub store: Store,
    /// The audio backend: commands go out through it, and `main` shuts it
    /// down on exit.
    pub backend: BackendHandle,
    /// The latest `CommandFailed`, kept for the status line (M3).
    pub last_command_failure: Option<(CommandKind, BackendError)>,
    /// When the last store summary was logged, and at which store revision.
    pub last_summary: Option<(Instant, u64)>,
    /// Something changed since the last draw.
    pub dirty: bool,
    /// Startup options.
    pub options: Options,
}

impl App {
    /// A running app on the Playback tab that still needs its first frame.
    pub fn new(options: Options, backend: BackendHandle, now: Instant) -> Self {
        Self {
            running: true,
            ui: UiState::new(now),
            store: Store::default(),
            backend,
            last_command_failure: None,
            last_summary: None,
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
            Event::Tick(now) => self.tick(now),
            Event::Crossterm(CrosstermEvent::Key(key)) => {
                if let Some(action) = map_key(&self.ui, key) {
                    self.update(action);
                }
            }
            Event::Crossterm(CrosstermEvent::Resize(width, height)) => self.resize(width, height),
            Event::Crossterm(_) => {}
            Event::Backend(event) => self.on_backend(event),
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
            Action::RowNext => self.move_selection(|i| i.saturating_add(1)),
            Action::RowPrev => self.move_selection(|i| i.saturating_sub(1)),
            Action::RowFirst => self.move_selection(|_| 0),
            Action::RowLast => self.move_selection(|_| usize::MAX),
            Action::FilterNext => self.cycle_filter(true),
            Action::FilterPrev => self.cycle_filter(false),
            _ => {}
        }
    }

    /// Records a new terminal size (VOL-ARCH-064, VOL-VIS-001).
    pub fn resize(&mut self, width: u16, height: u16) {
        self.ui.terminal_too_small = width < MIN_WIDTH || height < MIN_HEIGHT;
        // Tab bar, filter bar and status line take one line each (08 §1).
        self.ui.body_height = height.saturating_sub(3);
        self.sync_list();
        self.dirty = true;
    }

    /// Keys of the active tab's rows, in display order. Only Playback has
    /// rows so far.
    pub fn row_keys(&self) -> Vec<RowKey> {
        match self.ui.tab {
            SelectedTab::PlaybackTab => {
                playback_rows(&self.store, self.ui.playback_filter, &UiPending)
                    .into_iter()
                    .map(|row| row.key)
                    .collect()
            }
            _ => Vec::new(),
        }
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
            let tick = matches!(event, Event::Tick(_));
            self.on_event(event);
            if tick && self.running && self.needs_draw() {
                terminal.draw(|frame| ui::render(self, frame.area(), frame.buffer_mut()))?;
                self.dirty = false;
            }
        }
        Ok(())
    }

    /// Advances the injected clock, drops an expired status message (so the
    /// hint is redrawn), redraws when the disconnected countdown changes
    /// (08 §7) and logs the store summary at most once per second, and only
    /// when the store changed.
    fn tick(&mut self, now: Instant) {
        let banner = ui::banner_text(&self.store.conn, self.ui.now);
        self.ui.now = now;
        self.dirty |= ui::banner_text(&self.store.conn, now) != banner;
        if self.ui.status.as_ref().is_some_and(|m| now >= m.expires_at) {
            self.ui.status = None;
            self.dirty = true;
        }
        let due = self.last_summary.is_none_or(|(at, revision)| {
            revision != self.store.revision && now.duration_since(at) >= SUMMARY_INTERVAL
        });
        if due {
            log::debug(format_args!("{}", store_summary(&self.store)));
            self.last_summary = Some((now, self.store.revision));
        }
    }

    /// Applies a backend message to the store (VOL-ARCH-012, VOL-ARCH-027).
    fn on_backend(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::Batch(events) => {
                for event in events {
                    if let Some((key, value)) = unparsable_default(&event) {
                        log::debug(format_args!(
                            "metadata {key}: value {value:?} is not {{\"name\": string}}"
                        ));
                    }
                    self.dirty |= self.store.apply(event);
                }
            }
            BackendEvent::Connected { server_version, .. } => {
                self.dirty |= self.store.set_conn(ConnState::Connected { server_version });
            }
            BackendEvent::Disconnected {
                reason,
                retry_in,
                attempt,
            } => {
                let conn = ConnState::Disconnected {
                    reason,
                    retry_at: self.ui.now + retry_in.unwrap_or_default(),
                    attempt,
                };
                let conn_changed = self.store.set_conn(conn);
                let cleared = self.store.clear();
                self.dirty |= conn_changed | cleared;
                // Popups are bound to rows, which are gone (VOL-ARCH-044);
                // the help overlay stays.
                if matches!(self.ui.overlay, Overlay::Popup(_)) {
                    self.ui.overlay = Overlay::None;
                    self.dirty = true;
                }
            }
            // Meters are drawn from M7 on.
            BackendEvent::Meters(_) => {}
            BackendEvent::CommandFailed { cmd, error } => {
                self.last_command_failure = Some((cmd, error));
            }
        }
        self.sync_list();
    }

    fn go_tab(&mut self, tab: SelectedTab) {
        self.ui.tab = tab;
        self.sync_list();
        self.dirty = true;
    }

    /// Re-finds the active tab's selection in its current rows and scrolls
    /// it into view (VOL-ROW-002, VOL-ROW-004).
    fn sync_list(&mut self) {
        let keys = self.row_keys();
        let list = self.ui.lists.entry(self.ui.tab).or_default();
        let before = list.clone();
        list.reselect(&keys);
        list.scroll_into_view(&vec![LOCKED_ROW_HEIGHT; keys.len()], self.ui.body_height);
        self.dirty |= *list != before;
    }

    /// Moves the selection to `to(current index)`, clamped (VOL-KEY-004).
    /// No-op on an empty list.
    fn move_selection(&mut self, to: impl Fn(usize) -> usize) {
        let keys = self.row_keys();
        let list = self.ui.lists.entry(self.ui.tab).or_default();
        if list.selected.is_none() {
            return;
        }
        let before = list.clone();
        list.select_index(&keys, to(list.last_index));
        list.scroll_into_view(&vec![LOCKED_ROW_HEIGHT; keys.len()], self.ui.body_height);
        self.dirty |= *list != before;
    }

    /// `f`/`F`: cycles the active tab's filter, wrapping (VOL-FLT-001). Tabs
    /// without filters show `No filters on this tab`.
    fn cycle_filter(&mut self, forward: bool) {
        match self.ui.tab {
            SelectedTab::PlaybackTab => {
                let filter = self.ui.playback_filter;
                self.ui.playback_filter = if forward {
                    filter.next()
                } else {
                    filter.prev()
                };
                self.sync_list();
            }
            _ => self.ui.set_status(Msg::NO_FILTERS, StatusKind::Info),
        }
        self.dirty = true;
    }
}

/// One-line store summary for the debug log: nodes by kind, devices, ports,
/// links and the effective defaults.
fn store_summary(store: &Store) -> String {
    let nodes = |kind: NodeKind| store.nodes.values().filter(|n| n.kind == kind).count();
    let name = |name: &Option<String>| name.clone().unwrap_or_else(|| "-".to_owned());
    format!(
        "store: sinks={} sources={} playback={} capture={} devices={} ports={} links={} \
         default.sink={} default.source={}",
        nodes(NodeKind::Sink),
        nodes(NodeKind::Source),
        nodes(NodeKind::PlaybackStream),
        nodes(NodeKind::CaptureStream),
        store.devices.len(),
        store.ports.len(),
        store.links.len(),
        name(&store.defaults.sink),
        name(&store.defaults.source),
    )
}

/// Key and value of a `default.*` metadata event whose value the store will
/// read as `None` because it is not JSON `{"name": string}` (VOL-ARCH-025).
fn unparsable_default(event: &StoreEvent) -> Option<(&str, &str)> {
    let StoreEvent::Metadata {
        subject: 0,
        key,
        value: Some(value),
        ..
    } = event
    else {
        return None;
    };
    let is_default_key = Defaults::default().slot_mut(key).is_some();
    (is_default_key && parse_default_name(value).is_none()).then_some((key, value))
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use strum::IntoEnumIterator;

    use super::*;
    use crate::backend::mock::{
        MockControl, MockScript, disconnected, empty_connected, spawn_mock,
    };
    use crate::backend::{Command, DisconnectReason};
    use crate::model::{ObjectId, PropMap, Serial};

    const OPTIONS: Options = Options {
        color: true,
        mouse: false,
    };

    fn app() -> App {
        App::new(OPTIONS, BackendHandle::inert(), Instant::now())
    }

    fn stream_added(id: u32) -> StoreEvent {
        StoreEvent::NodeAdded {
            id: ObjectId(id),
            serial: Serial(u64::from(id) + 100),
            props: props(&[("media.class", "Stream/Output/Audio"), ("node.name", "s")]),
        }
    }

    fn batch(app: &mut App, events: Vec<StoreEvent>) {
        app.on_event(Event::Backend(BackendEvent::Batch(events)));
    }

    fn selected(app: &App) -> Option<RowKey> {
        app.ui.list(app.ui.tab).selected
    }

    fn stream_key(id: u32) -> Option<RowKey> {
        Some(RowKey::Stream(Serial(u64::from(id) + 100)))
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

    fn props(pairs: &[(&str, &str)]) -> PropMap {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn sink_added(id: u32) -> StoreEvent {
        StoreEvent::NodeAdded {
            id: ObjectId(id),
            serial: Serial(u64::from(id) + 100),
            props: props(&[("media.class", "Audio/Sink"), ("node.name", "sink")]),
        }
    }

    /// An app driven by a mock backend, with the mock's initial events
    /// applied and its first frame drawn.
    fn mocked(script: MockScript) -> (App, MockControl) {
        let (handle, control) = spawn_mock(script);
        let mut app = App::new(OPTIONS, handle, Instant::now());
        pump(&mut app);
        app.dirty = false;
        (app, control)
    }

    /// Feeds every event the backend has queued into the app.
    fn pump(app: &mut App) {
        while let Ok(event) = app.backend.events.try_recv() {
            app.on_event(Event::Backend(event));
        }
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
        let now = app.ui.now;
        app.on_event(Event::Tick(now));
        assert!(!app.needs_draw());
    }

    #[test]
    fn tick_advances_injected_clock() {
        let mut app = drawn();
        let later = app.ui.now + Duration::from_millis(40);
        app.on_event(Event::Tick(later));
        assert_eq!(app.ui.now, later);
        assert!(!app.needs_draw());
    }

    #[test]
    fn tick_drops_expired_status_and_marks_dirty() {
        let mut app = drawn();
        app.ui
            .set_status("Channels locked", crate::ui_state::StatusKind::Info);
        let set_at = app.ui.now;
        app.on_event(Event::Tick(set_at + Duration::from_millis(3999)));
        assert!(app.ui.status.is_some());
        assert!(!app.needs_draw());
        app.on_event(Event::Tick(set_at + crate::ui_state::STATUS_TTL));
        assert_eq!(app.ui.status, None);
        assert!(app.needs_draw());
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
    fn row_navigation_moves_over_playback_rows_and_clamps() {
        let mut app = app();
        // 12 lines: a 9-line body holds three 3-line rows.
        resize(&mut app, 80, 12);
        batch(&mut app, (1..=4).map(stream_added).collect());
        assert_eq!(selected(&app), stream_key(1));
        for _ in 0..3 {
            key(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(selected(&app), stream_key(4));
        assert_eq!(app.ui.list(app.ui.tab).offset, 1);
        app.dirty = false;
        key(&mut app, KeyCode::Down);
        assert_eq!(selected(&app), stream_key(4));
        assert!(!app.needs_draw(), "clamped move changes nothing");
        key(&mut app, KeyCode::Char('g'));
        assert_eq!(selected(&app), stream_key(1));
        assert_eq!(app.ui.list(app.ui.tab).offset, 0);
        assert!(app.needs_draw());
        key(&mut app, KeyCode::Up);
        assert_eq!(selected(&app), stream_key(1));
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(selected(&app), stream_key(4));
        key(&mut app, KeyCode::Char('k'));
        assert_eq!(selected(&app), stream_key(3));
    }

    #[test]
    fn selection_follows_key_when_streams_change() {
        let mut app = app();
        batch(&mut app, (1..=3).map(stream_added).collect());
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(selected(&app), stream_key(2));
        // A row above disappears: the selection stays on its stream.
        batch(&mut app, vec![StoreEvent::NodeRemoved { id: ObjectId(1) }]);
        assert_eq!(selected(&app), stream_key(2));
        // The selected stream disappears: the row now at its index is selected.
        batch(&mut app, vec![StoreEvent::NodeRemoved { id: ObjectId(2) }]);
        assert_eq!(selected(&app), stream_key(3));
        batch(&mut app, vec![StoreEvent::NodeRemoved { id: ObjectId(3) }]);
        assert_eq!(selected(&app), None);
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(selected(&app), None);
    }

    #[test]
    fn filter_cycle_wraps_and_marks_dirty() {
        use crate::model::view::StreamFilter;

        let mut app = drawn();
        let forward = [
            StreamFilter::All,
            StreamFilter::Virtual,
            StreamFilter::Applications,
        ];
        for expected in forward {
            key(&mut app, KeyCode::Char('f'));
            assert_eq!(app.ui.playback_filter, expected);
            assert!(app.needs_draw());
            app.dirty = false;
        }
        key(&mut app, KeyCode::Char('F'));
        assert_eq!(app.ui.playback_filter, StreamFilter::Virtual);
        assert!(app.needs_draw());

        // The filter is per tab: switching away and back keeps it; a tab
        // without filters says so.
        key(&mut app, KeyCode::F(5));
        key(&mut app, KeyCode::Char('f'));
        assert_eq!(
            app.ui.status.as_ref().map(|m| m.text.as_str()),
            Some(Msg::NO_FILTERS)
        );
        key(&mut app, KeyCode::F(1));
        assert_eq!(app.ui.playback_filter, StreamFilter::Virtual);
    }

    #[test]
    fn filter_change_reselects_rows() {
        let mut app = app();
        let mut virtual_stream = stream_added(2);
        if let StoreEvent::NodeAdded { props, .. } = &mut virtual_stream {
            props.insert("node.virtual".to_owned(), "true".to_owned());
        }
        batch(&mut app, vec![stream_added(1), virtual_stream]);
        assert_eq!(app.row_keys(), [stream_key(1).unwrap()]);
        // Applications → All Streams → Virtual Streams.
        key(&mut app, KeyCode::Char('f'));
        key(&mut app, KeyCode::Char('G'));
        assert_eq!(selected(&app), stream_key(2));
        key(&mut app, KeyCode::Char('f'));
        assert_eq!(selected(&app), stream_key(2));
        key(&mut app, KeyCode::Char('f'));
        assert_eq!(selected(&app), stream_key(1));
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
    fn batch_event_applies_to_store_and_marks_dirty() {
        let (mut app, control) = mocked(empty_connected());
        let revision = app.store.revision;
        control
            .push(BackendEvent::Batch(vec![sink_added(40), sink_added(41)]))
            .unwrap();
        pump(&mut app);
        assert_eq!(app.store.nodes.len(), 2);
        assert!(app.store.revision > revision);
        assert!(app.needs_draw());
    }

    #[test]
    fn identical_batch_does_not_mark_dirty() {
        let (mut app, control) = mocked(empty_connected());
        let batch = BackendEvent::Batch(vec![sink_added(40)]);
        control.push(batch.clone()).unwrap();
        pump(&mut app);
        app.dirty = false;
        let revision = app.store.revision;
        control.push(batch).unwrap();
        control.push(BackendEvent::Batch(Vec::new())).unwrap();
        pump(&mut app);
        assert_eq!(app.store.revision, revision);
        assert!(!app.needs_draw());
    }

    #[test]
    fn disconnected_clears_store() {
        let (mut app, control) = mocked(empty_connected());
        control
            .push(BackendEvent::Batch(vec![sink_added(40)]))
            .unwrap();
        pump(&mut app);
        app.dirty = false;
        control
            .push(BackendEvent::Disconnected {
                reason: DisconnectReason::ConnectionLost,
                retry_in: Some(Duration::from_secs(2)),
                attempt: 3,
            })
            .unwrap();
        pump(&mut app);
        assert!(app.store.nodes.is_empty());
        assert_eq!(
            app.store.conn,
            ConnState::Disconnected {
                reason: DisconnectReason::ConnectionLost,
                retry_at: app.ui.now + Duration::from_secs(2),
                attempt: 3,
            }
        );
        assert!(app.needs_draw());
    }

    #[test]
    fn disconnected_event_clears_rows_and_keeps_tab_and_filter() {
        use crate::model::view::StreamFilter;
        use crate::ui_state::PopupState;

        let (mut app, control) = mocked(empty_connected());
        batch(&mut app, (1..=2).map(stream_added).collect());
        key(&mut app, KeyCode::Char('f'));
        app.ui.overlay = Overlay::Popup(PopupState::default());
        assert_eq!(app.row_keys().len(), 2);
        control
            .push(BackendEvent::Disconnected {
                reason: DisconnectReason::NoDaemon,
                retry_in: Some(Duration::from_secs(1)),
                attempt: 1,
            })
            .unwrap();
        pump(&mut app);
        assert!(app.row_keys().is_empty());
        assert_eq!(selected(&app), None);
        assert_eq!(app.ui.tab, SelectedTab::PlaybackTab);
        assert_eq!(app.ui.playback_filter, StreamFilter::All);
        assert_eq!(app.ui.overlay, Overlay::None, "popups close");

        // The help overlay is not bound to rows and stays open.
        app.ui.overlay = Overlay::Help { scroll: 0 };
        control
            .push(BackendEvent::Disconnected {
                reason: DisconnectReason::NoDaemon,
                retry_in: Some(Duration::from_secs(2)),
                attempt: 2,
            })
            .unwrap();
        pump(&mut app);
        assert_eq!(app.ui.overlay, Overlay::Help { scroll: 0 });
    }

    #[test]
    fn disconnected_countdown_redraws_when_seconds_change() {
        let (mut app, _control) = mocked(disconnected(DisconnectReason::NoDaemon));
        resize(&mut app, 80, 24);
        app.dirty = false;
        let start = app.ui.now;
        app.on_event(Event::Tick(start + Duration::from_millis(400)));
        assert!(!app.needs_draw(), "still 1 s");
        app.on_event(Event::Tick(start + Duration::from_millis(600)));
        assert!(app.needs_draw(), "1 s → 0 s");
    }

    #[test]
    fn connected_sets_conn() {
        let (mut app, control) = mocked(disconnected(DisconnectReason::NoDaemon));
        assert!(matches!(app.store.conn, ConnState::Disconnected { .. }));
        control
            .push(BackendEvent::Connected {
                server_version: "1.6.9".to_owned(),
                socket: "pipewire-0".to_owned(),
            })
            .unwrap();
        pump(&mut app);
        assert_eq!(
            app.store.conn,
            ConnState::Connected {
                server_version: "1.6.9".to_owned()
            }
        );
        assert!(app.needs_draw());
    }

    #[test]
    fn command_failed_is_kept() {
        let (mut app, control) = mocked(empty_connected());
        control.fail_next(BackendError::NodeGone(ObjectId(7)));
        app.backend
            .commands
            .send(Command::SetMute {
                node: ObjectId(7),
                mute: true,
            })
            .unwrap();
        pump(&mut app);
        assert_eq!(
            app.last_command_failure,
            Some((CommandKind::SetMute, BackendError::NodeGone(ObjectId(7))))
        );
    }

    #[test]
    fn store_summary_is_logged_at_most_once_per_second() {
        let (mut app, control) = mocked(empty_connected());
        let start = app.ui.now;
        app.on_event(Event::Tick(start));
        let first = app.store.revision;
        assert_eq!(app.last_summary, Some((start, first)));
        control
            .push(BackendEvent::Batch(vec![sink_added(40)]))
            .unwrap();
        pump(&mut app);
        app.on_event(Event::Tick(start + Duration::from_millis(500)));
        assert_eq!(app.last_summary, Some((start, first)));
        let second = start + SUMMARY_INTERVAL;
        app.on_event(Event::Tick(second));
        assert_eq!(app.last_summary, Some((second, app.store.revision)));
        // Unchanged store: no new line, however long it stays unchanged.
        app.on_event(Event::Tick(second + Duration::from_secs(5)));
        assert_eq!(app.last_summary, Some((second, app.store.revision)));
    }

    #[test]
    fn store_summary_counts_objects_and_defaults() {
        let mut store = Store::default();
        store.apply(sink_added(40));
        store.apply(StoreEvent::Metadata {
            subject: 0,
            key: "default.audio.sink".to_owned(),
            type_: Some("Spa:String:JSON".to_owned()),
            value: Some(r#"{"name":"sink"}"#.to_owned()),
        });
        assert_eq!(
            store_summary(&store),
            "store: sinks=1 sources=0 playback=0 capture=0 devices=0 ports=0 links=0 \
             default.sink=sink default.source=-"
        );
    }

    #[test]
    fn unparsable_default_value_is_detected() {
        let metadata = |subject: u32, key: &str, value: &str| StoreEvent::Metadata {
            subject,
            key: key.to_owned(),
            type_: Some("Spa:String:JSON".to_owned()),
            value: Some(value.to_owned()),
        };
        let bad = metadata(0, "default.audio.sink", "garbage");
        assert_eq!(
            unparsable_default(&bad),
            Some(("default.audio.sink", "garbage"))
        );
        let good = metadata(0, "default.audio.sink", r#"{"name":"x"}"#);
        assert_eq!(unparsable_default(&good), None);
        assert_eq!(unparsable_default(&metadata(0, "other", "garbage")), None);
        assert_eq!(unparsable_default(&metadata(5, "target.object", "x")), None);
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
