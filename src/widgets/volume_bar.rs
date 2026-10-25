//! The volume bar (`docs/specs/08-visual-design.md` §4).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::theme::Theme;

/// Displayed percent at which the bar is full.
const BAR_MAX: u32 = 150;

/// Number of filled cells of a `g`-cell bar at `percent`:
/// `⌊g·min(p, 150)/150 + 0.5⌋`, computed exactly in integers.
pub fn bar_fill(g: u16, percent: u32) -> u16 {
    let p = u64::from(percent.min(BAR_MAX));
    let fill = (2 * u64::from(g) * p + u64::from(BAR_MAX)) / (2 * u64::from(BAR_MAX));
    u16::try_from(fill).unwrap_or(g)
}

/// Column of the 100 % tick, `⌊2g/3⌋`; `None` when `g < 3`.
pub fn bar_tick(g: u16) -> Option<u16> {
    (g >= 3).then(|| u16::try_from(2 * u32::from(g) / 3).unwrap_or(g))
}

/// One-line volume bar filling the first row of its area.
pub struct VolumeBar<'a> {
    /// Displayed percent (may exceed 150; the bar clamps); `None` = no volume known (`--`).
    pub percent: Option<u32>,
    /// Muted rows draw filled cells in `bar_muted`.
    pub muted: bool,
    /// Styles.
    pub theme: &'a Theme,
}

impl Widget for VolumeBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let g = area.width;
        let Some(percent) = self.percent else {
            let dim = Style::new().add_modifier(Modifier::DIM);
            for i in 0..g {
                buf[(area.x + i, area.y)].set_char('─').set_style(dim);
            }
            let start = area.x + g.saturating_sub(2) / 2;
            buf.set_stringn(start, area.y, "--", usize::from(g), dim);
            return;
        };
        let theme = self.theme;
        let (fill_style, over_style) = if self.muted {
            (theme.bar_muted, theme.bar_muted)
        } else {
            (theme.bar_fill, theme.bar_over)
        };
        let fill = bar_fill(g, percent);
        let tick = bar_tick(g);
        for i in 0..g {
            let (glyph, style) = match tick {
                Some(t) if i == t => ('┃', theme.tick),
                Some(t) if i < fill && i > t => ('═', over_style),
                _ if i < fill => ('━', fill_style),
                _ => ('─', theme.bar_empty),
            };
            buf[(area.x + i, area.y)].set_char(glyph).set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    const WIDTHS: [u16; 5] = [3, 29, 40, 47, 51];
    const PERCENTS: [u32; 7] = [0, 1, 50, 99, 100, 101, 150];

    /// The bar rendered into a `g`×1 buffer offset by one column.
    fn render(percent: Option<u32>, muted: bool, g: u16) -> Buffer {
        let theme = Theme::new(true);
        let area = Rect::new(1, 0, g, 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, g + 2, 1));
        VolumeBar {
            percent,
            muted,
            theme: &theme,
        }
        .render(area, &mut buf);
        buf
    }

    /// The bar's cells as text.
    fn glyphs(buf: &Buffer, g: u16) -> String {
        (0..g).map(|i| buf[(i + 1, 0)].symbol()).collect()
    }

    fn fg(buf: &Buffer, i: u16) -> Color {
        buf[(i + 1, 0)].fg
    }

    #[test]
    fn fill_and_tick_table() {
        for g in WIDTHS {
            // 08 §4: t = ⌊2G/3⌋.
            let t = (2.0 * f64::from(g) / 3.0).floor();
            assert_eq!(bar_tick(g).map(f64::from), Some(t), "tick G={g}");
            for p in PERCENTS {
                // 08 §4: fill = ⌊G·p_draw/150 + 0.5⌋.
                let p_draw = f64::from(p.min(150));
                let fill = (f64::from(g) * p_draw / 150.0 + 0.5).floor();
                assert_eq!(f64::from(bar_fill(g, p)), fill, "fill G={g} p={p}");

                let expected: String = (0..g)
                    .map(|i| {
                        let i = f64::from(i);
                        if i == t {
                            '┃'
                        } else if i < fill && i < t {
                            '━'
                        } else if i < fill {
                            '═'
                        } else {
                            '─'
                        }
                    })
                    .collect();
                assert_eq!(
                    glyphs(&render(Some(p), false, g), g),
                    expected,
                    "G={g} p={p}"
                );
            }
        }
        assert_eq!(bar_fill(51, 1000), 51);
    }

    #[test]
    fn tick_column_is_two_thirds() {
        assert_eq!(bar_tick(51), Some(34));
        assert_eq!(bar_tick(40), Some(26));
        assert_eq!(bar_tick(29), Some(19));
        assert_eq!(bar_tick(3), Some(2));
        for g in WIDTHS {
            let t = bar_tick(g).unwrap();
            let text = glyphs(&render(Some(100), false, g), g);
            let expected = "━".repeat(usize::from(t)) + "┃" + &"─".repeat(usize::from(g - t - 1));
            assert_eq!(text, expected, "G={g}");
        }
    }

    #[test]
    fn over_100_uses_double_line_glyph_right_of_tick() {
        let buf = render(Some(150), false, 51);
        let text: Vec<char> = glyphs(&buf, 51).chars().collect();
        for i in 0..34 {
            assert_eq!(text[usize::from(i)], '━', "cell {i}");
            assert_eq!(fg(&buf, i), Color::Green, "cell {i}");
        }
        assert_eq!(text[34], '┃');
        for i in 35..51 {
            assert_eq!(text[usize::from(i)], '═', "cell {i}");
            assert_eq!(fg(&buf, i), Color::Red, "cell {i}");
        }
        // 120 % (the Spotify row of 02 §2.3 at G = 51): six cells over.
        assert_eq!(
            glyphs(&render(Some(120), false, 51), 51),
            "━".repeat(34) + "┃" + &"═".repeat(6) + &"─".repeat(10)
        );
        // Above 150 % the bar stays full.
        assert_eq!(
            glyphs(&render(Some(210), false, 51), 51),
            text.iter().collect::<String>()
        );
        // The first over cell at G = 51 appears at 105 % (08 §4).
        assert!(!glyphs(&render(Some(104), false, 51), 51).contains('═'));
        assert_eq!(
            glyphs(&render(Some(105), false, 51), 51)
                .matches('═')
                .count(),
            1
        );
    }

    #[test]
    fn muted_uses_muted_style() {
        let theme = Theme::new(true);
        let muted = render(Some(150), true, 51);
        let normal = render(Some(150), false, 51);
        assert_eq!(glyphs(&muted, 51), glyphs(&normal, 51));
        for i in (0..51).filter(|&i| i != 34) {
            assert_eq!(Some(fg(&muted, i)), theme.bar_muted.fg, "cell {i}");
        }
        assert_eq!(muted[(35, 0)].modifier, theme.tick.add_modifier);
        let half = render(Some(50), true, 51);
        assert_eq!(Some(fg(&half, 0)), theme.bar_muted.fg);
        assert_eq!(Some(fg(&half, 50)), theme.bar_empty.fg);
    }

    #[test]
    fn unavailable_shows_dashes() {
        let buf = render(None, false, 51);
        assert_eq!(glyphs(&buf, 51), "─".repeat(24) + "--" + &"─".repeat(25));
        for i in 0..51 {
            assert!(buf[(i + 1, 0)].modifier.contains(Modifier::DIM), "cell {i}");
        }
        assert_eq!(glyphs(&render(None, true, 4), 4), "─--─");
        assert_eq!(glyphs(&render(None, false, 1), 1), "-");
    }

    #[test]
    fn width_below_3_has_no_tick_and_no_panic() {
        assert_eq!(bar_tick(2), None);
        assert_eq!(bar_tick(1), None);
        assert_eq!(bar_tick(0), None);
        for g in [1, 2] {
            for p in PERCENTS {
                let text = glyphs(&render(Some(p), false, g), g);
                assert!(!text.contains('┃'), "G={g} p={p}: {text}");
                assert!(!text.contains('═'), "G={g} p={p}: {text}");
            }
        }
        assert_eq!(glyphs(&render(Some(150), false, 2), 2), "━━");
        assert_eq!(glyphs(&render(Some(50), false, 2), 2), "━─");
    }

    #[test]
    fn zero_width_noop() {
        let theme = Theme::new(true);
        let before = Buffer::empty(Rect::new(0, 0, 4, 2));
        for area in [Rect::new(1, 0, 0, 1), Rect::new(1, 0, 3, 0)] {
            for percent in [None, Some(100)] {
                let mut buf = before.clone();
                VolumeBar {
                    percent,
                    muted: false,
                    theme: &theme,
                }
                .render(area, &mut buf);
                assert_eq!(buf, before, "{area:?} {percent:?}");
            }
        }
    }
}
