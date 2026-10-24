//! User intents produced by the key map (07 §1).

use crate::ui::SelectedTab;

/// A user intent. Produced by [`crate::keymap::map_key`], consumed by the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Quit the application.
    Quit,
    /// Next tab (wraps).
    NextTab,
    /// Previous tab (wraps).
    PrevTab,
    /// Go to a specific tab.
    GoTab(SelectedTab),
    /// Select the next row (clamped).
    RowNext,
    /// Select the previous row (clamped).
    RowPrev,
    /// Select the first row.
    RowFirst,
    /// Select the last row.
    RowLast,
    /// Relative volume change in integer percent.
    VolumeDelta(i32),
    /// Absolute volume in percent, `0..=150`.
    VolumeSet(i32),
    /// Toggle mute of the selected row.
    ToggleMute,
    /// Toggle channel lock of the selected row.
    ToggleLock,
    /// Focus the previous channel of an unlocked row.
    ChannelPrev,
    /// Focus the next channel of an unlocked row.
    ChannelNext,
    /// Make the selected device the default.
    SetDefault,
    /// Open the move/port/profile picker.
    OpenPicker,
    /// Next filter of the active tab.
    FilterNext,
    /// Previous filter of the active tab.
    FilterPrev,
    /// Open or close the help overlay.
    ToggleHelp,
    /// Close the open overlay; with none open, clear the transient status
    /// message (07 §2.1, `Esc`).
    CloseOverlay,
    /// Select the next popup item (clamped).
    PopupNext,
    /// Select the previous popup item (clamped).
    PopupPrev,
    /// Select the first popup item.
    PopupFirst,
    /// Select the last popup item.
    PopupLast,
    /// Apply the selected popup item and close the popup.
    PopupApply,
    /// Select popup item by 0-based index.
    PopupJump(usize),
}
