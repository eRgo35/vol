//! Status line: connection, transient message or hint, default device,
//! `? help` (07 §6).

use std::time::Instant;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    model::{ConnState, NodeKind, Store},
    theme::Theme,
    ui::SelectedTab,
    ui_state::{StatusKind, StatusMsg},
};

/// Always the last 9 columns (07 §6).
const HELP: &str = " │ ? help";
/// Below this width the default label is dropped.
const DEFAULT_MIN_WIDTH: u16 = 60;
/// Below this width `? help` is dropped.
const HELP_MIN_WIDTH: u16 = 24;
/// Maximum width of the default label, `out: …` included.
const DEFAULT_MAX: usize = 24;

/// The transient message catalogue (07 §6), exact strings.
pub struct Msg;

impl Msg {
    /// Set-default was not echoed within 1 s.
    pub const DEFAULT_NOT_APPLIED: &str = "Default not applied (WirePlumber running?)";
    /// No `default` metadata object.
    pub const NO_METADATA: &str = "No default-device metadata (is WirePlumber running?)";
    /// Move was not applied by the session manager.
    pub const MOVE_NOT_APPLIED: &str = "Move not applied by the session manager";
    /// The selected stream disappeared.
    pub const STREAM_ENDED: &str = "Stream ended";
    /// The selected device disappeared.
    pub const DEVICE_REMOVED: &str = "Device removed";
    /// The selected card disappeared.
    pub const CARD_REMOVED: &str = "Card removed";
    /// `d` on a stream tab.
    pub const STREAMS_HAVE_NO_DEFAULT: &str = "Streams have no default";
    /// Volume key before the `Props` param arrived.
    pub const VOLUME_NOT_READY: &str = "Volume not available yet";
    /// Channel focus on a mono node.
    pub const NEEDS_2_CHANNELS: &str = "Needs 2 or more channels";
    /// Channel focus while locked.
    pub const UNLOCK_FIRST: &str = "Unlock channels first (c)";
    /// Lock toggled on.
    pub const CHANNELS_LOCKED: &str = "Channels locked";
    /// Lock toggled off.
    pub const CHANNELS_UNLOCKED: &str = "Channels unlocked";
    /// Port picker with no candidates.
    pub const NO_PORTS: &str = "No selectable ports";
    /// Profile picker with no candidates.
    pub const NO_PROFILES: &str = "No profiles";
    /// `f` on a tab without filters.
    pub const NO_FILTERS: &str = "No filters on this tab";
    /// Key that does not apply to the active tab.
    pub const NOT_APPLICABLE: &str = "Not applicable here";
    /// `CommandFailed` with `NotConnected`.
    pub const NOT_CONNECTED: &str = "Not connected to PipeWire";
    /// `CommandFailed` with `PermissionDenied`.
    pub const PERMISSION_DENIED: &str = "Permission denied by PipeWire";

    /// Default output changed (echo).
    pub fn default_output(title: &str) -> String {
        format!("Default output: {title}")
    }

    /// Default input changed (echo).
    pub fn default_input(title: &str) -> String {
        format!("Default input: {title}")
    }

    /// Move issued.
    pub fn moving(title: &str, label: &str) -> String {
        format!("Moving \"{title}\" → {label}")
    }

    /// Chosen port is unplugged.
    pub fn unplugged(port: &str) -> String {
        format!("{port} is unplugged")
    }

    /// Chosen profile is unavailable.
    pub fn unavailable(profile: &str) -> String {
        format!("{profile} is unavailable")
    }

    /// `CommandFailed` with `Pipewire(errno, msg)`.
    pub fn pipewire_error(msg: &str) -> String {
        format!("PipeWire error: {msg}")
    }
}

/// The hint shown when no message is active (07 §6).
pub fn hint(tab: SelectedTab) -> &'static str {
    match tab {
        SelectedTab::PlaybackTab | SelectedTab::RecordingTab => "h/l vol  m mute  c lock  p move",
        SelectedTab::OutputDevicesTab | SelectedTab::InputDevicesTab => {
            "h/l vol  m mute  d default  p port"
        }
        SelectedTab::ConfigurationTab => "p profile",
    }
}

/// `out: <default sink title>` / `in: <default source title>` for `tab`,
/// cut to 24 columns; `None` on Configuration or when the default node is
/// not known. Device title: `node.description` → `node.nick` → `node.name`.
pub fn default_label(store: &Store, tab: SelectedTab) -> Option<String> {
    let (prefix, kind, name) = match tab {
        SelectedTab::PlaybackTab | SelectedTab::OutputDevicesTab => {
            ("out", NodeKind::Sink, store.defaults.sink.as_deref()?)
        }
        SelectedTab::RecordingTab | SelectedTab::InputDevicesTab => {
            ("in", NodeKind::Source, store.defaults.source.as_deref()?)
        }
        SelectedTab::ConfigurationTab => return None,
    };
    let node = store
        .nodes
        .values()
        .find(|n| n.kind == kind && n.props.get("node.name").map(String::as_str) == Some(name))?;
    let title = ["node.description", "node.nick", "node.name"]
        .iter()
        .find_map(|key| node.props.get(*key).filter(|v| !v.is_empty()))?;
    Some(truncate(&format!("{prefix}: {title}"), DEFAULT_MAX))
}

/// The status line for one frame. Pure: time comes from `now`.
#[derive(Debug, Clone)]
pub struct StatusLine<'a> {
    /// Connection state.
    pub conn: &'a ConnState,
    /// Latest transient message; shown while `now < expires_at`.
    pub message: Option<&'a StatusMsg>,
    /// The injected clock.
    pub now: Instant,
    /// Active tab (selects the hint).
    pub tab: SelectedTab,
    /// From [`default_label`].
    pub default: Option<String>,
    /// Styles.
    pub theme: &'a Theme,
}

impl Widget for StatusLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme;
        let (glyph, glyph_style, conn) = match self.conn {
            ConnState::Connected { server_version } => {
                ("●", theme.status_ok, format!(" PipeWire {server_version}"))
            }
            ConnState::Disconnected { .. } => ("○", theme.status_bad, " disconnected".to_owned()),
            ConnState::Connecting => ("○", theme.status_bad, " connecting…".to_owned()),
        };
        let message = match self.message.filter(|m| self.now < m.expires_at) {
            Some(StatusMsg {
                text,
                kind: StatusKind::Error,
                ..
            }) if !theme.color => (format!("! {text}"), theme.msg_error),
            Some(StatusMsg {
                text,
                kind: StatusKind::Error,
                ..
            }) => (text.clone(), theme.msg_error),
            Some(StatusMsg { text, .. }) => (text.clone(), Style::new()),
            None => (hint(self.tab).to_owned(), theme.hint),
        };
        let left = [
            (" ".to_owned(), Style::new()),
            (glyph.to_owned(), glyph_style),
            (conn, Style::new()),
            (" │ ".to_owned(), Style::new()),
            message,
        ];

        let width = area.width;
        let help = if width >= HELP_MIN_WIDTH { HELP } else { "" };
        let right = match &self.default {
            Some(default) if width >= DEFAULT_MIN_WIDTH => format!(" {default}{help}"),
            _ => help.to_owned(),
        };
        let left_max = usize::from(width).saturating_sub(right.width());
        let mut spans = fit(&left, left_max);
        let used: usize = spans.iter().map(Span::width).sum();
        spans.push(Span::raw(" ".repeat(left_max - used)));
        spans.push(Span::raw(right));
        Line::from(spans).render(area, buf);
    }
}

/// `segments` cut to `max` cells in total, ending in `…` if anything was cut.
fn fit(segments: &[(String, Style)], max: usize) -> Vec<Span<'static>> {
    let total: usize = segments.iter().map(|(text, _)| text.width()).sum();
    if total <= max {
        return segments
            .iter()
            .map(|(text, style)| Span::styled(text.clone(), *style))
            .collect();
    }
    let mut left = max.saturating_sub(1);
    let mut spans = Vec::new();
    for (text, style) in segments {
        let cut = take_width(text, left);
        left -= cut.width();
        if cut.len() < text.len() {
            if max > 0 {
                spans.push(Span::styled(format!("{cut}…"), *style));
            }
            break;
        }
        spans.push(Span::styled(cut, *style));
    }
    spans
}

/// `text` cut to `max` cells, ending in `…` if anything was cut.
pub(super) fn truncate(text: &str, max: usize) -> String {
    if text.width() <= max {
        return text.to_owned();
    }
    let mut out = take_width(text, max.saturating_sub(1));
    out.push('…');
    out
}

/// The longest prefix of `text` at most `max` cells wide.
fn take_width(text: &str, max: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used <= max
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::style::Color;

    use super::*;
    use crate::{
        backend::DisconnectReason,
        model::{Node, NodeState, ObjectId, Serial},
        ui_state::STATUS_TTL,
    };

    fn connected() -> ConnState {
        ConnState::Connected {
            server_version: "1.4.7".to_owned(),
        }
    }

    fn line(conn: &ConnState, message: Option<&StatusMsg>, now: Instant, width: u16) -> String {
        line_with(conn, message, now, width, Some("out: Speakers".to_owned()))
    }

    fn line_with(
        conn: &ConnState,
        message: Option<&StatusMsg>,
        now: Instant,
        width: u16,
        default: Option<String>,
    ) -> String {
        let theme = Theme::new(true);
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        StatusLine {
            conn,
            message,
            now,
            tab: SelectedTab::PlaybackTab,
            default,
            theme: &theme,
        }
        .render(area, &mut buf);
        (0..width).map(|x| buf[(x, 0)].symbol()).collect()
    }

    fn msg(text: &str, kind: StatusKind, expires_at: Instant) -> StatusMsg {
        StatusMsg {
            text: text.to_owned(),
            kind,
            expires_at,
        }
    }

    #[test]
    fn connected_text_with_version() {
        let now = Instant::now();
        let text = line(&connected(), None, now, 80);
        assert_eq!(
            text,
            " ● PipeWire 1.4.7 │ h/l vol  m mute  c lock  p move       out: Speakers │ ? help"
        );
    }

    #[test]
    fn disconnected_text() {
        let now = Instant::now();
        let conn = ConnState::Disconnected {
            reason: DisconnectReason::NoDaemon,
            retry_at: now,
            attempt: 1,
        };
        let text = line_with(&conn, None, now, 80, None);
        assert!(text.starts_with(" ○ disconnected │ "), "{text}");
        assert!(text.ends_with(" │ ? help"), "{text}");
        let text = line_with(&ConnState::Connecting, None, now, 80, None);
        assert!(text.starts_with(" ○ connecting… │ "), "{text}");
    }

    #[test]
    fn message_overrides_hint() {
        let now = Instant::now();
        let m = msg("Channels locked", StatusKind::Info, now + STATUS_TTL);
        let text = line(&connected(), Some(&m), now, 80);
        assert!(
            text.starts_with(" ● PipeWire 1.4.7 │ Channels locked "),
            "{text}"
        );
        assert!(!text.contains("h/l vol"), "{text}");
    }

    #[test]
    fn error_message_styled_and_prefixed_in_mono() {
        let now = Instant::now();
        let m = msg(Msg::NOT_CONNECTED, StatusKind::Error, now + STATUS_TTL);
        for color in [true, false] {
            let theme = Theme::new(color);
            let area = Rect::new(0, 0, 80, 1);
            let mut buf = Buffer::empty(area);
            StatusLine {
                conn: &connected(),
                message: Some(&m),
                now,
                tab: SelectedTab::PlaybackTab,
                default: None,
                theme: &theme,
            }
            .render(area, &mut buf);
            let text: String = (0..80).map(|x| buf[(x, 0)].symbol()).collect();
            let expected = if color {
                " ● PipeWire 1.4.7 │ Not connected"
            } else {
                " ● PipeWire 1.4.7 │ ! Not connected"
            };
            assert!(text.starts_with(expected), "{text}");
            let cell = &buf[(20, 0)];
            assert_eq!(cell.fg, theme.msg_error.fg.unwrap_or(Color::Reset));
            assert_eq!(cell.modifier, theme.msg_error.add_modifier);
        }
    }

    #[test]
    fn message_expires_after_4s() {
        let set_at = Instant::now();
        let m = msg("Channels locked", StatusKind::Info, set_at + STATUS_TTL);
        assert_eq!(STATUS_TTL, Duration::from_secs(4));
        let before = line(
            &connected(),
            Some(&m),
            set_at + Duration::from_millis(3999),
            80,
        );
        assert!(before.contains("Channels locked"), "{before}");
        let after = line(&connected(), Some(&m), set_at + Duration::from_secs(4), 80);
        assert!(!after.contains("Channels locked"), "{after}");
        assert!(after.contains("h/l vol"), "{after}");
    }

    #[test]
    fn default_label_dropped_below_60_cols() {
        let now = Instant::now();
        let at_60 = line(&connected(), None, now, 60);
        assert!(at_60.ends_with(" out: Speakers │ ? help"), "{at_60}");
        assert_eq!(at_60.chars().count(), 60);
        let at_59 = line(&connected(), None, now, 59);
        assert!(!at_59.contains("out:"), "{at_59}");
        assert_eq!(
            at_59,
            " ● PipeWire 1.4.7 │ h/l vol  m mute  c lock  p mo… │ ? help"
        );
    }

    #[test]
    fn help_hint_always_visible_at_40() {
        let now = Instant::now();
        let long = msg(&"x".repeat(100), StatusKind::Info, now + STATUS_TTL);
        for message in [None, Some(&long)] {
            let text = line(&connected(), message, now, 40);
            assert_eq!(text.chars().count(), 40, "{text}");
            assert!(text.ends_with("… │ ? help"), "{text}");
        }
        assert_eq!(
            line(&connected(), None, now, 40),
            " ● PipeWire 1.4.7 │ h/l vol  m… │ ? help"
        );
    }

    #[test]
    fn hints_per_tab() {
        assert_eq!(
            hint(SelectedTab::RecordingTab),
            hint(SelectedTab::PlaybackTab)
        );
        assert_eq!(
            hint(SelectedTab::InputDevicesTab),
            "h/l vol  m mute  d default  p port"
        );
        assert_eq!(hint(SelectedTab::ConfigurationTab), "p profile");
    }

    fn device(id: u32, kind: NodeKind, props: &[(&str, &str)]) -> Node {
        Node {
            id: ObjectId(id),
            serial: Serial(u64::from(id)),
            kind,
            props: props
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            state: NodeState::Idle,
            audio: None,
            process_latency_ns: None,
        }
    }

    #[test]
    fn default_label_from_store() {
        let mut store = Store::default();
        assert_eq!(default_label(&store, SelectedTab::PlaybackTab), None);
        store.defaults.sink = Some("alsa_output.speakers".to_owned());
        store.defaults.source = Some("alsa_input.mic".to_owned());
        // Default name not in the store: omitted.
        assert_eq!(default_label(&store, SelectedTab::PlaybackTab), None);
        let sink = device(
            40,
            NodeKind::Sink,
            &[
                ("node.name", "alsa_output.speakers"),
                ("node.description", "Starship/Matisse HD Audio Controller"),
            ],
        );
        let source = device(
            41,
            NodeKind::Source,
            &[("node.name", "alsa_input.mic"), ("node.nick", "Mic")],
        );
        store.nodes.insert(sink.id, sink);
        store.nodes.insert(source.id, source);
        let out = default_label(&store, SelectedTab::OutputDevicesTab);
        assert_eq!(out.as_deref(), Some("out: Starship/Matisse H…"));
        assert_eq!(out.map(|s| s.width()), Some(24));
        assert_eq!(
            default_label(&store, SelectedTab::RecordingTab).as_deref(),
            Some("in: Mic")
        );
        assert_eq!(default_label(&store, SelectedTab::ConfigurationTab), None);
    }

    #[test]
    fn wide_glyphs_are_not_split() {
        assert_eq!(truncate("音楽音楽", 5), "音楽…");
        assert_eq!(truncate("音楽", 4), "音楽");
    }

    #[test]
    fn catalogue_strings_match_spec() {
        let table: &[(&str, &str)] = &[
            (&Msg::default_output("Speakers"), "Default output: Speakers"),
            (&Msg::default_input("Mic"), "Default input: Mic"),
            (
                Msg::DEFAULT_NOT_APPLIED,
                "Default not applied (WirePlumber running?)",
            ),
            (
                Msg::NO_METADATA,
                "No default-device metadata (is WirePlumber running?)",
            ),
            (
                &Msg::moving("Firefox", "Speakers"),
                "Moving \"Firefox\" → Speakers",
            ),
            (
                Msg::MOVE_NOT_APPLIED,
                "Move not applied by the session manager",
            ),
            (Msg::STREAM_ENDED, "Stream ended"),
            (Msg::DEVICE_REMOVED, "Device removed"),
            (Msg::CARD_REMOVED, "Card removed"),
            (Msg::STREAMS_HAVE_NO_DEFAULT, "Streams have no default"),
            (Msg::VOLUME_NOT_READY, "Volume not available yet"),
            (Msg::NEEDS_2_CHANNELS, "Needs 2 or more channels"),
            (Msg::UNLOCK_FIRST, "Unlock channels first (c)"),
            (Msg::CHANNELS_LOCKED, "Channels locked"),
            (Msg::CHANNELS_UNLOCKED, "Channels unlocked"),
            (&Msg::unplugged("Headphones"), "Headphones is unplugged"),
            (&Msg::unavailable("Pro Audio"), "Pro Audio is unavailable"),
            (Msg::NO_PORTS, "No selectable ports"),
            (Msg::NO_PROFILES, "No profiles"),
            (Msg::NO_FILTERS, "No filters on this tab"),
            (Msg::NOT_APPLICABLE, "Not applicable here"),
            (Msg::NOT_CONNECTED, "Not connected to PipeWire"),
            (Msg::PERMISSION_DENIED, "Permission denied by PipeWire"),
            (
                &Msg::pipewire_error("No such entity"),
                "PipeWire error: No such entity",
            ),
        ];
        for (got, want) in table {
            assert_eq!(got, want);
        }
    }
}
