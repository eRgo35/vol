//! UI-only state: active tab, overlays, status message, injected clock.

use std::time::Instant;

use crate::ui::SelectedTab;

/// UI state that is not derived from the PipeWire store.
#[derive(Debug, Clone)]
pub struct UiState {
    /// Active tab.
    pub tab: SelectedTab,
    /// Open overlay, if any.
    pub overlay: Overlay,
    /// Terminal is below the 40x12 minimum (08 §6).
    pub terminal_too_small: bool,
    /// Transient status-line message.
    pub status: Option<StatusMsg>,
    /// Current time, injected so rendering never reads the clock.
    pub now: Instant,
}

impl UiState {
    /// Initial state: Playback tab, no overlay, no message.
    pub fn new(now: Instant) -> Self {
        Self {
            tab: SelectedTab::default(),
            overlay: Overlay::None,
            terminal_too_small: false,
            status: None,
            now,
        }
    }

    /// Input mode derived from the state. `TooSmall` wins over any overlay.
    pub fn mode(&self) -> Mode {
        if self.terminal_too_small {
            return Mode::TooSmall;
        }
        match self.overlay {
            Overlay::None => Mode::Normal,
            Overlay::Help { .. } => Mode::Help,
            Overlay::Popup(_) => Mode::Popup,
        }
    }
}

/// Overlay drawn over the body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Overlay {
    /// No overlay.
    #[default]
    None,
    /// Help overlay with its scroll offset.
    Help {
        /// First visible line.
        scroll: u16,
    },
    /// List popup (move/port/profile picker).
    Popup(PopupState),
}

/// List popup contents (placeholder; completed in M4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PopupState {
    /// Title in the border.
    pub title: String,
    /// Item labels.
    pub items: Vec<String>,
    /// Selected item index.
    pub selected: usize,
}

/// Key dispatch mode (07 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No overlay open.
    Normal,
    /// A list popup is open.
    Popup,
    /// The help overlay is open.
    Help,
    /// Terminal below 40x12.
    TooSmall,
}

/// Transient status-line message (07 §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMsg {
    /// Message text.
    pub text: String,
    /// Styling class.
    pub kind: StatusKind,
    /// When the message disappears.
    pub expires_at: Instant,
}

/// How a status message is styled (07 §6: errors use the error style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Normal message.
    Info,
    /// Error message.
    Error,
}
