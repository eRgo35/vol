//! Style roles (08 §8) and the `NO_COLOR` policy (08 §9).
//!
//! Named ANSI colors only, so the user's terminal palette applies. In mono
//! mode no role sets a foreground or background color; only modifiers.

use ratatui::style::{Color, Modifier, Style};

/// Styles by role. Build with [`Theme::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Color mode (`false` = mono, `NO_COLOR`). In mono mode error messages
    /// are additionally prefixed with `! ` by the renderer.
    pub color: bool,
    /// Selected tab title.
    pub tab_selected: Style,
    /// Other tab titles.
    pub tab_normal: Style,
    /// Selection bar `▌`.
    pub sel_bar: Style,
    /// Title of the selected row.
    pub title_selected: Style,
    /// Title of a corked/suspended stream.
    pub title_idle: Style,
    /// Volume bar, filled cells up to 100 %.
    pub bar_fill: Style,
    /// Volume bar, filled cells above 100 %.
    pub bar_over: Style,
    /// Volume bar, empty cells.
    pub bar_empty: Style,
    /// Volume bar, filled cells of a muted row.
    pub bar_muted: Style,
    /// 100 % tick `┃`.
    pub tick: Style,
    /// `[M]`.
    pub mute_on: Style,
    /// `[ ]`.
    pub mute_off: Style,
    /// Percent text above 100 %.
    pub pct_over: Style,
    /// Default marker `★`.
    pub default_star: Style,
    /// Running state `●`.
    pub state_running: Style,
    /// Idle state `○`.
    pub state_idle: Style,
    /// Peak meter, low cells.
    pub meter_low: Style,
    /// Peak meter, middle cell.
    pub meter_mid: Style,
    /// Peak meter, high cell.
    pub meter_high: Style,
    /// Peak meter, empty cells.
    pub meter_empty: Style,
    /// Popup border.
    pub popup_border: Style,
    /// Selected popup item.
    pub popup_sel: Style,
    /// Current-value check `✓` in popups.
    pub popup_check: Style,
    /// Connected `●` in the status line.
    pub status_ok: Style,
    /// Disconnected `○` in the status line.
    pub status_bad: Style,
    /// Error message in the status line.
    pub msg_error: Style,
    /// Status hint and empty-state hint.
    pub hint: Style,
}

impl Theme {
    /// Theme for color mode (`true`) or mono mode (`false`).
    pub fn new(color: bool) -> Self {
        // `pick(color_mode, mono_mode)`, one line per row of the 08 §8 table.
        let pick = |c: Style, m: Style| if color { c } else { m };
        let d = Style::new();
        let fg = |c: Color| Style::new().fg(c);
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let dim = Style::new().add_modifier(Modifier::DIM);
        let reversed = Style::new().add_modifier(Modifier::REVERSED);
        Self {
            color,
            tab_selected: pick(
                fg(Color::Yellow).add_modifier(Modifier::BOLD),
                bold.patch(reversed),
            ),
            tab_normal: d,
            sel_bar: pick(fg(Color::Cyan).add_modifier(Modifier::BOLD), bold),
            title_selected: bold,
            title_idle: dim,
            bar_fill: pick(fg(Color::Green), d),
            bar_over: pick(fg(Color::Red), d),
            bar_empty: pick(fg(Color::DarkGray), dim),
            bar_muted: pick(fg(Color::DarkGray), dim),
            tick: bold,
            mute_on: pick(fg(Color::Red).add_modifier(Modifier::BOLD), bold),
            mute_off: dim,
            pct_over: pick(fg(Color::Red), bold),
            default_star: pick(fg(Color::Yellow), d),
            state_running: pick(fg(Color::Green), d),
            state_idle: dim,
            meter_low: pick(fg(Color::Green), d),
            meter_mid: pick(fg(Color::Yellow), d),
            meter_high: pick(fg(Color::Red), d),
            meter_empty: pick(fg(Color::DarkGray), dim),
            popup_border: pick(fg(Color::Cyan), d),
            popup_sel: reversed,
            popup_check: pick(fg(Color::Green), bold),
            status_ok: pick(fg(Color::Green), d),
            status_bad: pick(fg(Color::Red), d),
            msg_error: pick(fg(Color::Red), bold),
            hint: dim,
        }
    }
}

/// Whether colors are used: disabled iff `--no-color` was passed or
/// `NO_COLOR` is set and non-empty (<https://no-color.org>).
pub fn color_enabled(no_color_flag: bool, env_no_color: Option<&str>) -> bool {
    !(no_color_flag || env_no_color.is_some_and(|v| !v.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every role, by name. Destructuring keeps this exhaustive.
    fn roles(theme: Theme) -> Vec<(&'static str, Style)> {
        let Theme {
            color: _,
            tab_selected,
            tab_normal,
            sel_bar,
            title_selected,
            title_idle,
            bar_fill,
            bar_over,
            bar_empty,
            bar_muted,
            tick,
            mute_on,
            mute_off,
            pct_over,
            default_star,
            state_running,
            state_idle,
            meter_low,
            meter_mid,
            meter_high,
            meter_empty,
            popup_border,
            popup_sel,
            popup_check,
            status_ok,
            status_bad,
            msg_error,
            hint,
        } = theme;
        vec![
            ("tab_selected", tab_selected),
            ("tab_normal", tab_normal),
            ("sel_bar", sel_bar),
            ("title_selected", title_selected),
            ("title_idle", title_idle),
            ("bar_fill", bar_fill),
            ("bar_over", bar_over),
            ("bar_empty", bar_empty),
            ("bar_muted", bar_muted),
            ("tick", tick),
            ("mute_on", mute_on),
            ("mute_off", mute_off),
            ("pct_over", pct_over),
            ("default_star", default_star),
            ("state_running", state_running),
            ("state_idle", state_idle),
            ("meter_low", meter_low),
            ("meter_mid", meter_mid),
            ("meter_high", meter_high),
            ("meter_empty", meter_empty),
            ("popup_border", popup_border),
            ("popup_sel", popup_sel),
            ("popup_check", popup_check),
            ("status_ok", status_ok),
            ("status_bad", status_bad),
            ("msg_error", msg_error),
            ("hint", hint),
        ]
    }

    #[test]
    fn mono_theme_sets_no_fg_or_bg_on_any_role() {
        let theme = Theme::new(false);
        assert!(!theme.color);
        for (name, style) in roles(theme) {
            assert_eq!(style.fg, None, "{name}");
            assert_eq!(style.bg, None, "{name}");
            assert_eq!(style.underline_color, None, "{name}");
        }
    }

    #[test]
    fn color_theme_tab_selected_is_bold_yellow() {
        let theme = Theme::new(true);
        assert!(theme.color);
        assert_eq!(
            theme.tab_selected,
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            Theme::new(false).tab_selected,
            Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        );
    }

    #[test]
    fn muted_roles_use_dim_in_mono() {
        let theme = Theme::new(false);
        for (name, style) in [
            ("title_idle", theme.title_idle),
            ("bar_empty", theme.bar_empty),
            ("bar_muted", theme.bar_muted),
            ("mute_off", theme.mute_off),
            ("state_idle", theme.state_idle),
            ("meter_empty", theme.meter_empty),
            ("hint", theme.hint),
        ] {
            assert!(style.add_modifier.contains(Modifier::DIM), "{name}");
        }
    }

    #[test]
    fn color_theme_matches_role_table() {
        let t = Theme::new(true);
        let fg = |c| Style::new().fg(c);
        let m = |m| Style::new().add_modifier(m);
        assert_eq!(t.tab_normal, Style::new());
        assert_eq!(t.sel_bar, fg(Color::Cyan).add_modifier(Modifier::BOLD));
        assert_eq!(t.title_selected, m(Modifier::BOLD));
        assert_eq!(t.title_idle, m(Modifier::DIM));
        assert_eq!(t.bar_fill, fg(Color::Green));
        assert_eq!(t.bar_over, fg(Color::Red));
        assert_eq!(t.bar_empty, fg(Color::DarkGray));
        assert_eq!(t.bar_muted, fg(Color::DarkGray));
        assert_eq!(t.tick, m(Modifier::BOLD));
        assert_eq!(t.mute_on, fg(Color::Red).add_modifier(Modifier::BOLD));
        assert_eq!(t.mute_off, m(Modifier::DIM));
        assert_eq!(t.pct_over, fg(Color::Red));
        assert_eq!(t.default_star, fg(Color::Yellow));
        assert_eq!(t.state_running, fg(Color::Green));
        assert_eq!(t.state_idle, m(Modifier::DIM));
        assert_eq!(t.meter_low, fg(Color::Green));
        assert_eq!(t.meter_mid, fg(Color::Yellow));
        assert_eq!(t.meter_high, fg(Color::Red));
        assert_eq!(t.meter_empty, fg(Color::DarkGray));
        assert_eq!(t.popup_border, fg(Color::Cyan));
        assert_eq!(t.popup_sel, m(Modifier::REVERSED));
        assert_eq!(t.popup_check, fg(Color::Green));
        assert_eq!(t.status_ok, fg(Color::Green));
        assert_eq!(t.status_bad, fg(Color::Red));
        assert_eq!(t.msg_error, fg(Color::Red));
        assert_eq!(t.hint, m(Modifier::DIM));
    }

    #[test]
    fn mono_theme_matches_role_table() {
        let t = Theme::new(false);
        let d = Style::new();
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let dim = Style::new().add_modifier(Modifier::DIM);
        assert_eq!(t.tab_normal, d);
        assert_eq!(t.sel_bar, bold);
        assert_eq!(t.title_selected, bold);
        assert_eq!(t.bar_fill, d);
        assert_eq!(t.bar_over, d);
        assert_eq!(t.tick, bold);
        assert_eq!(t.mute_on, bold);
        assert_eq!(t.pct_over, bold);
        assert_eq!(t.default_star, d);
        assert_eq!(t.state_running, d);
        assert_eq!(t.meter_low, d);
        assert_eq!(t.meter_mid, d);
        assert_eq!(t.meter_high, d);
        assert_eq!(t.meter_empty, dim);
        assert_eq!(t.popup_border, d);
        assert_eq!(t.popup_sel, Style::new().add_modifier(Modifier::REVERSED));
        assert_eq!(t.popup_check, bold);
        assert_eq!(t.status_ok, d);
        assert_eq!(t.status_bad, d);
        assert_eq!(t.msg_error, bold);
        assert_eq!(t.hint, dim);
    }

    #[test]
    fn no_color_unset_keeps_color() {
        assert!(color_enabled(false, None));
    }

    #[test]
    fn no_color_empty_string_keeps_color() {
        assert!(color_enabled(false, Some("")));
    }

    #[test]
    fn no_color_nonempty_disables() {
        assert!(!color_enabled(false, Some("1")));
        assert!(!color_enabled(false, Some("0")));
    }

    #[test]
    fn flag_disables_even_if_env_unset() {
        assert!(!color_enabled(true, None));
        assert!(!color_enabled(true, Some("")));
    }
}
