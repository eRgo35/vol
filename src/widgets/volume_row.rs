//! Row layout (`docs/specs/02-playback.md` §2.3): width tiers, column widths and the
//! 3-line locked row. Unlocked rows arrive in M3.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use super::statusline::truncate;
use super::volume_bar::VolumeBar;
use crate::model::view::{StreamRow, TargetLabel};
use crate::model::volume::fmt_db;
use crate::theme::Theme;

/// Width tier of the body (02 §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// W ≥ 72: dB and meter columns.
    Full,
    /// 56 ≤ W ≤ 71: meter column, no dB.
    Mid,
    /// W ≤ 55: neither dB nor meter.
    Narrow,
}

/// Column widths of a row at one body width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowGeometry {
    /// Width tier.
    pub tier: Tier,
    /// Volume bar width `G`.
    pub bar_w: u16,
    /// Target label maximum width `L = min(30, ⌊3W/8⌋)`.
    pub label_max: u16,
}

/// `sel ␠ mute(3) ␠`: cells before the bar on line 2.
const BAR_X: u16 = 6;
/// Percent column width (`␠` + 4).
const PCT_W: u16 = 5;
/// dB column width (`␠` + 8).
const DB_W: u16 = 9;
/// Meter column width (`␠` + 8).
const METER_W: u16 = 9;
/// Line 1 cells before the title: `sel marker ␠`.
const TITLE_X: u16 = 3;
/// The title never gets fewer cells than this while a label is shown.
const TITLE_MIN: u16 = 8;

impl RowGeometry {
    /// Geometry for body width `width`.
    pub fn new(width: u16) -> Self {
        let tier = match width {
            72.. => Tier::Full,
            56..=71 => Tier::Mid,
            _ => Tier::Narrow,
        };
        let columns = BAR_X + PCT_W + db_w(tier) + meter_w(tier);
        Self {
            tier,
            bar_w: width.saturating_sub(columns),
            label_max: 30.min(u16::try_from(u32::from(width) * 3 / 8).unwrap_or(30)),
        }
    }

    fn has_db(self) -> bool {
        self.tier == Tier::Full
    }
}

fn db_w(tier: Tier) -> u16 {
    if tier == Tier::Full { DB_W } else { 0 }
}

fn meter_w(tier: Tier) -> u16 {
    if tier == Tier::Narrow { 0 } else { METER_W }
}

/// Draws `row` as a 3-line locked row into `area` (lines that do not fit are skipped).
/// `geometry` must be built for `area.width`.
pub fn render_locked_row(
    row: &StreamRow,
    geometry: &RowGeometry,
    selected: bool,
    theme: &Theme,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.is_empty() {
        return;
    }
    let (sel, sel_style) = if selected {
        ("▌", theme.sel_bar)
    } else {
        (" ", Style::new())
    };
    for y in 0..area.height.min(3) {
        put(buf, area, 0, y, sel, sel_style);
    }
    title_line(row, geometry, selected, theme, area, buf);
    if area.height >= 2 {
        volume_line(row, geometry, theme, area, buf);
    }
    if area.height >= 3 {
        put(buf, area, 5, 2, &channel_summary(row), Style::new());
    }
    if row.audio.as_ref().is_some_and(|a| a.mute) {
        dim_except_mute(area, buf);
    }
}

/// Line 1: marker, title (+ media) and the right-aligned target label.
fn title_line(
    row: &StreamRow,
    geometry: &RowGeometry,
    selected: bool,
    theme: &Theme,
    area: Rect,
    buf: &mut Buffer,
) {
    let (marker, marker_style) = if row.running {
        ("●", theme.state_running)
    } else {
        ("○", theme.state_idle)
    };
    put(buf, area, 1, 0, marker, marker_style);

    let mut title_style = Style::new();
    if selected {
        title_style = title_style.patch(theme.title_selected);
    }
    if !row.running {
        title_style = title_style.patch(theme.title_idle);
    }
    let (label, label_style) = match &row.target {
        TargetLabel::Device(desc) => (format!("→ {desc}"), Style::new()),
        TargetLabel::NoDevice => (
            "(no device)".to_owned(),
            Style::new().add_modifier(Modifier::DIM),
        ),
    };
    let label = truncate(&label, usize::from(geometry.label_max));
    let label_w = u16::try_from(label.width()).unwrap_or(u16::MAX);
    let with_label = area.width.saturating_sub(TITLE_X + 1 + label_w);
    let title_w = if with_label >= TITLE_MIN {
        put(buf, area, area.width - label_w, 0, &label, label_style);
        with_label
    } else {
        area.width.saturating_sub(TITLE_X)
    };
    let text = match &row.media {
        Some(media) => format!("{}: {media}", row.title),
        None => row.title.clone(),
    };
    if title_w > 0 {
        let text = truncate(&text, usize::from(title_w));
        put(buf, area, TITLE_X, 0, &text, title_style);
    }
}

/// Line 2: mute marker, bar, percent and dB (the meter column stays blank until M7).
fn volume_line(
    row: &StreamRow,
    geometry: &RowGeometry,
    theme: &Theme,
    area: Rect,
    buf: &mut Buffer,
) {
    let audio = row.audio.as_ref();
    let muted = audio.is_some_and(|a| a.mute);
    let percent = audio.and_then(|a| a.master_percent());
    let (mute, mute_style) = if muted {
        ("[M]", theme.mute_on)
    } else {
        ("[ ]", theme.mute_off)
    };
    put(buf, area, 2, 1, mute, mute_style);

    let bar = Rect::new(area.x + BAR_X, area.y + 1, geometry.bar_w, 1).intersection(area);
    VolumeBar {
        percent: percent.map(|p| u32::try_from(p).unwrap_or(0)),
        muted,
        theme,
    }
    .render(bar, buf);

    let (Some(p), Some(gain)) = (percent, audio.and_then(|a| a.master_gain())) else {
        return;
    };
    let pct_style = if p > 100 {
        theme.pct_over
    } else {
        Style::new()
    };
    let x = BAR_X + geometry.bar_w;
    put(buf, area, x, 1, &format!(" {:>3}%", p.min(999)), pct_style);
    if geometry.has_db() {
        let db = fmt_db(gain);
        let db = if db.width() > 8 {
            "+99.9 dB".to_owned()
        } else {
            db
        };
        put(buf, area, x + PCT_W, 1, &format!(" {db:>8}"), Style::new());
    }
}

/// Line 3 text: `NAME p%` pairs and `locked` for two or more channels, else blank.
fn channel_summary(row: &StreamRow) -> String {
    let Some(audio) = row.audio.as_ref() else {
        return String::new();
    };
    let percents = audio.channel_percents();
    if percents.len() < 2 {
        return String::new();
    }
    let mut parts: Vec<String> = row
        .channel_names
        .iter()
        .zip(percents)
        .map(|(name, p)| format!("{name} {p}%"))
        .collect();
    parts.push("locked".to_owned());
    parts.join("  ")
}

/// Muted rows: every cell gains DIM except the `[M]` marker (08 §4).
fn dim_except_mute(area: Rect, buf: &mut Buffer) {
    for y in 0..area.height.min(3) {
        for x in 0..area.width {
            if y == 1 && (2..5).contains(&x) {
                continue;
            }
            let cell = &mut buf[(area.x + x, area.y + y)];
            cell.set_style(cell.style().add_modifier(Modifier::DIM));
        }
    }
}

/// Writes `text` at (`x`, `y`) relative to `area`, clipped to the area.
fn put(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) {
    if x >= area.width || y >= area.height {
        return;
    }
    let max = usize::from(area.width - x);
    buf.set_stringn(area.x + x, area.y + y, text, max, style);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::view::RowKey;
    use crate::model::volume::{level_to_gain, percent_to_level};
    use crate::model::{Channel, NodeAudio, ObjectId, Serial};

    fn gain(p: i32) -> f32 {
        level_to_gain(percent_to_level(p))
    }

    fn row(title: &str, media: Option<&str>, percent: i32, channels: usize) -> StreamRow {
        let names = ["FL", "FR"];
        StreamRow {
            key: RowKey::Stream(Serial(412)),
            node: ObjectId(84),
            title: title.to_owned(),
            media: media.map(str::to_owned),
            running: true,
            target: TargetLabel::Device(
                "Starship/Matisse HD Audio Controller Analog Stereo".to_owned(),
            ),
            audio: Some(NodeAudio {
                volume: 1.0,
                channel_volumes: vec![gain(percent); channels],
                mute: false,
                channel_map: (3..3 + channels as u32).map(Channel).collect(),
            }),
            channel_names: names[..channels].iter().map(|n| (*n).to_owned()).collect(),
        }
    }

    fn firefox() -> StreamRow {
        row("Firefox", Some("Playing music – Lo-fi beats"), 72, 2)
    }

    fn spotify() -> StreamRow {
        row("Spotify", Some("Song Title"), 120, 2)
    }

    fn render(row: &StreamRow, width: u16, selected: bool) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, 3));
        render_locked_row(
            row,
            &RowGeometry::new(width),
            selected,
            &Theme::new(true),
            buf.area,
            &mut buf,
        );
        buf
    }

    /// Line `y` as text; the cell hidden behind a two-cell glyph is skipped.
    fn line(buf: &Buffer, y: u16) -> String {
        let mut out = String::new();
        let mut x = 0;
        while x < buf.area.width {
            let symbol = buf[(x, y)].symbol();
            out.push_str(symbol);
            x += u16::try_from(symbol.width().max(1)).unwrap_or(1);
        }
        out
    }

    #[test]
    fn geometry_80_60_40() {
        let expect = [
            (80, Tier::Full, 51, 30),
            (60, Tier::Mid, 40, 22),
            (40, Tier::Narrow, 29, 15),
        ];
        for (w, tier, bar_w, label_max) in expect {
            assert_eq!(
                RowGeometry::new(w),
                RowGeometry {
                    tier,
                    bar_w,
                    label_max
                },
                "W={w}"
            );
        }
    }

    #[test]
    fn tier_boundaries_71_72_55_56() {
        assert_eq!(RowGeometry::new(72).tier, Tier::Full);
        assert_eq!(RowGeometry::new(72).bar_w, 72 - 29);
        assert_eq!(RowGeometry::new(71).tier, Tier::Mid);
        assert_eq!(RowGeometry::new(71).bar_w, 71 - 20);
        assert_eq!(RowGeometry::new(56).tier, Tier::Mid);
        assert_eq!(RowGeometry::new(56).bar_w, 56 - 20);
        assert_eq!(RowGeometry::new(55).tier, Tier::Narrow);
        assert_eq!(RowGeometry::new(55).bar_w, 55 - 11);
    }

    #[test]
    fn title_truncates_with_ellipsis_and_keeps_8_cells() {
        // W = 40: L = 15, label "→ Starship/Mat…", title gets 40 − 3 − 1 − 15 = 21 cells.
        let buf = render(&firefox(), 40, false);
        assert_eq!(line(&buf, 0), " ● Firefox: Playing mus… → Starship/Mat…");
        // W = 21: L = 7, title 21 − 4 − 7 = 10 cells (≥ 8: label kept).
        let buf = render(&firefox(), 21, false);
        assert_eq!(line(&buf, 0), " ● Firefox: … → Star…");
    }

    #[test]
    fn wide_glyph_title_not_split() {
        // W = 40: the 21-cell title field holds ten two-cell glyphs + `…`.
        let r = row("音楽音楽音楽音楽音楽音楽", None, 72, 2);
        let buf = render(&r, 40, false);
        assert_eq!(line(&buf, 0), " ● 音楽音楽音楽音楽音楽… → Starship/Mat…");
        // W = 22: the 10-cell field leaves 9 cells before `…`; the fifth glyph would
        // be split, so it is dropped and a blank cell remains.
        let buf = render(&r, 22, false);
        assert_eq!(line(&buf, 0), " ● 音楽音楽…  → Stars…");
    }

    #[test]
    fn label_dropped_when_title_would_drop_below_8() {
        // W = 16: L = 6; with the label the title would get 16 − 4 − 6 = 6 < 8 cells.
        let buf = render(&firefox(), 16, false);
        assert_eq!(line(&buf, 0), " ● Firefox: Pla…");
        // W = 19: L = 7; title gets 19 − 4 − 7 = 8 cells: label kept.
        let buf = render(&firefox(), 19, false);
        assert_eq!(line(&buf, 0), " ● Firefox… → Star…");
    }

    #[test]
    fn line2_columns_match_spec_at_80() {
        // 02 §2.3 mockup, meter column blank until M7.
        let buf = render(&firefox(), 80, true);
        assert_eq!(
            line(&buf, 0),
            "▌● Firefox: Playing music – Lo-fi beats           → Starship/Matisse HD Audio C…"
        );
        assert_eq!(
            line(&buf, 1),
            "▌ [ ] ━━━━━━━━━━━━━━━━━━━━━━━━──────────┃────────────────  72%  -8.6 dB         "
        );
        let buf = render(&spotify(), 80, false);
        assert_eq!(
            line(&buf, 1),
            "  [ ] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┃══════────────── 120%  +4.8 dB         "
        );
        assert_eq!(
            buf[(58, 1)].fg,
            Theme::new(true).pct_over.fg.unwrap_or_default()
        );
    }

    #[test]
    fn muted_row_shows_m_marker() {
        let mut r = row("mpv", Some("video.mkv"), 55, 2);
        if let Some(a) = r.audio.as_mut() {
            a.mute = true;
        }
        let buf = render(&r, 80, false);
        assert!(line(&buf, 1).starts_with("  [M] "));
        let theme = Theme::new(true);
        assert_eq!(buf[(2, 1)].fg, theme.mute_on.fg.unwrap_or_default());
        assert!(
            !buf[(2, 1)].modifier.contains(Modifier::DIM),
            "[M] not dimmed"
        );
        assert!(buf[(3, 0)].modifier.contains(Modifier::DIM), "title dimmed");
        assert!(buf[(6, 1)].modifier.contains(Modifier::DIM), "bar dimmed");
        assert_eq!(buf[(6, 1)].fg, theme.bar_muted.fg.unwrap_or_default());

        let buf = render(&firefox(), 80, false);
        assert!(line(&buf, 1).starts_with("  [ ] "));
    }

    #[test]
    fn channel_summary_line_for_stereo_and_blank_for_mono() {
        let buf = render(&firefox(), 80, true);
        assert_eq!(
            line(&buf, 2),
            format!("{:<80}", "▌    FL 72%  FR 72%  locked")
        );
        let buf = render(&spotify(), 80, false);
        assert_eq!(
            line(&buf, 2),
            format!("{:<80}", "     FL 120%  FR 120%  locked")
        );
        let buf = render(&row("Telegram", None, 100, 1), 80, false);
        assert_eq!(line(&buf, 2), " ".repeat(80));
    }

    #[test]
    fn unavailable_volume_shows_dashes() {
        let mut r = firefox();
        r.audio = None;
        let buf = render(&r, 40, false);
        // G = 29: `--` at column 6 + ⌊27/2⌋ = 19; no percent.
        let expected = format!("  [ ] {}--{}", "─".repeat(13), "─".repeat(14));
        assert_eq!(line(&buf, 1), format!("{expected:<40}"));
        assert_eq!(line(&buf, 2), " ".repeat(40));
    }

    #[test]
    fn short_area_draws_only_lines_that_fit() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 2));
        render_locked_row(
            &firefox(),
            &RowGeometry::new(80),
            false,
            &Theme::new(true),
            Rect::new(0, 0, 80, 1),
            &mut buf,
        );
        assert_eq!(line(&buf, 1), " ".repeat(80));
        render_locked_row(
            &firefox(),
            &RowGeometry::new(80),
            false,
            &Theme::new(true),
            Rect::new(0, 0, 0, 0),
            &mut buf,
        );
    }
}
