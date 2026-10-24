//! Whole-screen rendering (08 §1, §2, §6; VOL-VIS-001…004, 009, 010).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};
use strum::IntoEnumIterator;
use vol::{
    app::{App, Options},
    backend::BackendHandle,
    ui::{self, SelectedTab},
};

fn app(color: bool) -> App {
    let options = Options {
        color,
        mouse: false,
    };
    App::new(options, BackendHandle::inert(), Instant::now())
}

fn render(app: &App, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    ui::render(app, area, &mut buf);
    buf
}

/// The buffer's symbols, one line per row.
fn text(buf: &Buffer) -> String {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// [`text`] with every line quoted, so snapshots keep trailing blanks.
fn framed(buf: &Buffer) -> String {
    text(buf)
        .lines()
        .map(|line| format!("\"{line}\""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn render_80x24_snapshot() {
    insta::assert_snapshot!(framed(&render(&app(true), 80, 24)));
}

#[test]
fn render_60x24_snapshot() {
    insta::assert_snapshot!(framed(&render(&app(true), 60, 24)));
}

#[test]
fn render_40x12_is_not_too_small() {
    let buf = render(&app(true), 40, 12);
    let screen = text(&buf);
    assert!(!screen.contains("Terminal too small"), "{screen}");
    assert!(screen.starts_with(" Playback │"), "{screen}");
    insta::assert_snapshot!(framed(&buf));
}

#[test]
fn render_39x12_shows_too_small_message() {
    let buf = render(&app(true), 39, 12);
    let screen = text(&buf);
    assert!(screen.contains("need 40x12, have 39x12"), "{screen}");
    assert!(!screen.contains("Playback"), "{screen}");
    insta::assert_snapshot!(framed(&buf));
}

#[test]
fn render_40x11_shows_too_small_message() {
    let buf = render(&app(true), 40, 11);
    let screen = text(&buf);
    assert!(screen.contains("Terminal too small"), "{screen}");
    assert!(screen.contains("need 40x12, have 40x11"), "{screen}");
    assert!(!screen.contains("Playback"), "{screen}");
    insta::assert_snapshot!(framed(&buf));
}

#[test]
fn render_zero_and_one_cell_areas_do_not_panic() {
    for (w, h) in [(0, 0), (0, 24), (80, 0), (1, 1), (1, 24), (80, 1), (5, 2)] {
        let screen = text(&render(&app(true), w, h));
        // Narrower than the message: truncated, never wrapped.
        if w > 0 && h > 1 {
            assert_eq!(screen.lines().count(), usize::from(h));
        }
    }
    let narrow = text(&render(&app(true), 10, 12));
    assert!(narrow.contains("Terminal t"), "{narrow}");
    assert!(narrow.contains("need 40x12"), "{narrow}");
}

#[test]
fn no_cell_written_outside_area() {
    let outer = Rect::new(0, 0, 100, 30);
    for (w, h) in [(80, 24), (39, 12), (40, 12), (3, 2)] {
        let inner = Rect::new(7, 3, w, h);
        let mut buf = Buffer::empty(outer);
        for y in outer.top()..outer.bottom() {
            for x in outer.left()..outer.right() {
                buf[(x, y)].set_symbol("#");
            }
        }
        let mut app = app(true);
        for tab in SelectedTab::iter() {
            app.ui.tab = tab;
            ui::render(&app, inner, &mut buf);
            for y in outer.top()..outer.bottom() {
                for x in outer.left()..outer.right() {
                    if !inner.contains((x, y).into()) {
                        let cell = &buf[(x, y)];
                        assert_eq!(cell.symbol(), "#", "({x},{y}) at {w}x{h}");
                        assert_eq!(cell.fg, Color::Reset, "({x},{y}) at {w}x{h}");
                    }
                }
            }
        }
    }
}

#[test]
fn selected_tab_is_bold_yellow() {
    let mut app = app(true);
    app.ui.tab = SelectedTab::RecordingTab;
    let buf = render(&app, 80, 24);
    // " Playback │ Recording": "R" is at column 12.
    let cell = &buf[(12, 0)];
    assert_eq!(cell.symbol(), "R");
    assert_eq!(cell.fg, Color::Yellow);
    assert!(cell.modifier.contains(Modifier::BOLD));
    let other = &buf[(1, 0)];
    assert_eq!(other.symbol(), "P");
    assert_eq!(other.fg, Color::Reset);
    assert!(!other.modifier.contains(Modifier::BOLD));
}

#[test]
fn mono_theme_has_no_colors_anywhere() {
    let mut app = app(false);
    for (w, h) in [(80, 24), (39, 12)] {
        for tab in SelectedTab::iter() {
            app.ui.tab = tab;
            let buf = render(&app, w, h);
            for cell in &buf.content {
                assert_eq!(cell.fg, Color::Reset, "{tab} at {w}x{h}");
                assert_eq!(cell.bg, Color::Reset, "{tab} at {w}x{h}");
            }
        }
    }
}
