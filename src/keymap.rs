//! Pure key map: `KeyEvent` + UI state → [`Action`] (07 §1–§3).
//!
//! Implemented so far: quit, `Esc`, tab switching, row navigation. Volume,
//! popup and help keys return `None` until their milestones. Digits are reserved for
//! absolute volume and never select tabs (Q2). `Ctrl-Z` is ignored
//! (VOL-ARCH-065).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    action::Action,
    ui::SelectedTab,
    ui_state::{Mode, UiState},
};

/// Maps a key event to an action for the current mode. Pure.
///
/// Only `Press` and `Repeat` are handled. `SHIFT` is ignored on characters
/// (and on `BackTab`, which terminals report with it); any other modifier
/// makes a key match only an explicit entry, so `Ctrl-q` is not `q`.
pub fn map_key(ui: &UiState, key: KeyEvent) -> Option<Action> {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }
    let modifiers = match key.code {
        KeyCode::Char(_) | KeyCode::BackTab => key.modifiers.difference(KeyModifiers::SHIFT),
        _ => key.modifiers,
    };
    if modifiers == KeyModifiers::CONTROL && matches!(key.code, KeyCode::Char('c' | 'C')) {
        return Some(Action::Quit);
    }
    if modifiers != KeyModifiers::NONE {
        return None;
    }
    match ui.mode() {
        Mode::TooSmall => match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        },
        Mode::Help | Mode::Popup => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Action::CloseOverlay),
            _ => None,
        },
        Mode::Normal => match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Esc => Some(Action::CloseOverlay),
            KeyCode::Tab | KeyCode::Char(']') => Some(Action::NextTab),
            KeyCode::BackTab | KeyCode::Char('[') => Some(Action::PrevTab),
            KeyCode::F(n @ 1..=5) => SelectedTab::from_repr(usize::from(n - 1)).map(Action::GoTab),
            KeyCode::Char('j') | KeyCode::Down => Some(Action::RowNext),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::RowPrev),
            KeyCode::Char('g') => Some(Action::RowFirst),
            KeyCode::Char('G') => Some(Action::RowLast),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyEventState, KeyModifiers};

    use super::*;
    use crate::{
        ui::SelectedTab,
        ui_state::{Overlay, PopupState},
    };

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn key_kind(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind_and_state(code, KeyModifiers::NONE, kind, KeyEventState::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        key_mod(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn normal() -> UiState {
        UiState::new(Instant::now())
    }

    fn help() -> UiState {
        let mut ui = normal();
        ui.overlay = Overlay::Help { scroll: 0 };
        ui
    }

    fn popup() -> UiState {
        let mut ui = normal();
        ui.overlay = Overlay::Popup(PopupState::default());
        ui
    }

    fn too_small() -> UiState {
        let mut ui = normal();
        ui.terminal_too_small = true;
        ui
    }

    fn overlay_states() -> [UiState; 2] {
        [help(), popup()]
    }

    #[test]
    fn q_quits_in_normal_mode() {
        assert!(matches!(
            map_key(&normal(), key(KeyCode::Char('q'))),
            Some(Action::Quit)
        ));
    }

    #[test]
    fn q_closes_overlay_instead_of_quitting() {
        for ui in overlay_states() {
            assert!(matches!(
                map_key(&ui, key(KeyCode::Char('q'))),
                Some(Action::CloseOverlay)
            ));
        }
    }

    #[test]
    fn ctrl_c_quits_in_every_mode() {
        for ui in [normal(), help(), popup(), too_small()] {
            assert!(matches!(map_key(&ui, ctrl('c')), Some(Action::Quit)));
        }
    }

    #[test]
    fn esc_does_not_quit() {
        for ui in [normal(), help(), popup(), too_small()] {
            assert!(!matches!(
                map_key(&ui, key(KeyCode::Esc)),
                Some(Action::Quit)
            ));
        }
        // With no overlay, Esc clears the status message (handled by the app).
        assert!(matches!(
            map_key(&normal(), key(KeyCode::Esc)),
            Some(Action::CloseOverlay)
        ));
    }

    #[test]
    fn esc_closes_overlay() {
        for ui in overlay_states() {
            assert!(matches!(
                map_key(&ui, key(KeyCode::Esc)),
                Some(Action::CloseOverlay)
            ));
        }
    }

    #[test]
    fn release_events_ignored() {
        for code in [
            KeyCode::Char('q'),
            KeyCode::Tab,
            KeyCode::F(1),
            KeyCode::Esc,
        ] {
            assert!(map_key(&normal(), key_kind(code, KeyEventKind::Release)).is_none());
        }
    }

    #[test]
    fn repeat_events_handled() {
        assert!(matches!(
            map_key(&normal(), key_kind(KeyCode::Tab, KeyEventKind::Repeat)),
            Some(Action::NextTab)
        ));
        assert!(matches!(
            map_key(
                &normal(),
                key_kind(KeyCode::Char('q'), KeyEventKind::Repeat)
            ),
            Some(Action::Quit)
        ));
    }

    #[test]
    fn tab_cycles_forward() {
        assert!(matches!(
            map_key(&normal(), key(KeyCode::Tab)),
            Some(Action::NextTab)
        ));
    }

    #[test]
    fn backtab_cycles_backward() {
        // crossterm reports Shift-Tab as BackTab, usually with SHIFT set.
        assert!(matches!(
            map_key(&normal(), key(KeyCode::BackTab)),
            Some(Action::PrevTab)
        ));
        assert!(matches!(
            map_key(&normal(), key_mod(KeyCode::BackTab, KeyModifiers::SHIFT)),
            Some(Action::PrevTab)
        ));
    }

    #[test]
    fn brackets_cycle_tabs() {
        assert!(matches!(
            map_key(&normal(), key(KeyCode::Char(']'))),
            Some(Action::NextTab)
        ));
        assert!(matches!(
            map_key(&normal(), key(KeyCode::Char('['))),
            Some(Action::PrevTab)
        ));
    }

    #[test]
    fn f1_to_f5_go_to_tabs() {
        let expected = [
            (1, SelectedTab::PlaybackTab),
            (2, SelectedTab::RecordingTab),
            (3, SelectedTab::OutputDevicesTab),
            (4, SelectedTab::InputDevicesTab),
            (5, SelectedTab::ConfigurationTab),
        ];
        for (n, tab) in expected {
            match map_key(&normal(), key(KeyCode::F(n))) {
                Some(Action::GoTab(got)) => assert_eq!(got as usize, tab as usize, "F{n}"),
                other => panic!("F{n} mapped to {other:?}"),
            }
        }
        assert!(map_key(&normal(), key(KeyCode::F(6))).is_none());
    }

    #[test]
    fn ctrl_q_is_not_q() {
        for ui in [normal(), help(), popup(), too_small()] {
            assert!(map_key(&ui, ctrl('q')).is_none());
        }
        assert!(map_key(&normal(), key_mod(KeyCode::Char('q'), KeyModifiers::ALT)).is_none());
        assert!(
            map_key(
                &normal(),
                key_mod(KeyCode::Char(']'), KeyModifiers::CONTROL)
            )
            .is_none()
        );
    }

    #[test]
    fn ctrl_z_is_ignored() {
        for ui in [normal(), help(), popup(), too_small()] {
            assert!(map_key(&ui, ctrl('z')).is_none());
        }
    }

    #[test]
    fn shift_modifier_ignored_for_chars() {
        assert!(matches!(
            map_key(&normal(), key_mod(KeyCode::Char('q'), KeyModifiers::SHIFT)),
            Some(Action::Quit)
        ));
        assert!(matches!(
            map_key(&normal(), key_mod(KeyCode::Char(']'), KeyModifiers::SHIFT)),
            Some(Action::NextTab)
        ));
        assert!(matches!(
            map_key(
                &normal(),
                key_mod(
                    KeyCode::Char('c'),
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT
                )
            ),
            Some(Action::Quit)
        ));
    }

    #[test]
    fn too_small_mode_allows_only_quit() {
        let ui = too_small();
        assert!(matches!(
            map_key(&ui, key(KeyCode::Char('q'))),
            Some(Action::Quit)
        ));
        assert!(matches!(map_key(&ui, ctrl('c')), Some(Action::Quit)));
        for code in [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Char(']'),
            KeyCode::Char('['),
            KeyCode::F(1),
            KeyCode::F(5),
            KeyCode::Esc,
            KeyCode::Char('l'),
            KeyCode::Char('5'),
            KeyCode::Enter,
        ] {
            assert!(map_key(&ui, key(code)).is_none(), "{code:?}");
        }
    }

    #[test]
    fn digits_do_not_select_tabs() {
        // Q2: digits are reserved for absolute volume (not wired yet).
        for c in ['1', '2', '3', '4', '5'] {
            assert!(!matches!(
                map_key(&normal(), key(KeyCode::Char(c))),
                Some(Action::GoTab(_))
            ));
        }
    }

    #[test]
    #[allow(non_snake_case)] // test name from the M2 plan; `G` is the key itself
    fn j_k_g_G_arrows_row_navigation() {
        let expected = [
            (KeyCode::Char('j'), Action::RowNext),
            (KeyCode::Down, Action::RowNext),
            (KeyCode::Char('k'), Action::RowPrev),
            (KeyCode::Up, Action::RowPrev),
            (KeyCode::Char('g'), Action::RowFirst),
            (KeyCode::Char('G'), Action::RowLast),
        ];
        for (code, action) in expected {
            assert_eq!(map_key(&normal(), key(code)), Some(action), "{code:?}");
        }
        // Terminals report `G` with SHIFT; `Home`/`End` are volume keys, not rows.
        assert_eq!(
            map_key(&normal(), key_mod(KeyCode::Char('G'), KeyModifiers::SHIFT)),
            Some(Action::RowLast)
        );
        for code in [KeyCode::Home, KeyCode::End] {
            assert!(map_key(&normal(), key(code)).is_none(), "{code:?}");
        }
        for ui in [help(), popup(), too_small()] {
            assert_ne!(map_key(&ui, key(KeyCode::Char('j'))), Some(Action::RowNext));
        }
    }

    #[test]
    fn overlays_ignore_tab_keys() {
        for ui in overlay_states() {
            for code in [
                KeyCode::Tab,
                KeyCode::BackTab,
                KeyCode::F(2),
                KeyCode::Char(']'),
            ] {
                assert!(map_key(&ui, key(code)).is_none(), "{code:?}");
            }
        }
    }
}
