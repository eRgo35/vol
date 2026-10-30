//! Whole-screen rendering (08 §1, §2, §6, §7; VOL-VIS-001…004, 008…010).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};
use strum::IntoEnumIterator;
use vol::{
    app::{App, Options},
    backend::{BackendHandle, DisconnectReason, StoreEvent},
    model::{ConnState, ObjectId, Serial},
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

fn disconnected(reason: DisconnectReason, retry_in: Duration, attempt: u32) -> App {
    let mut app = app(true);
    let retry_at = app.ui.now + retry_in;
    app.store.set_conn(ConnState::Disconnected {
        reason,
        retry_at,
        attempt,
    });
    app
}

fn connected() -> App {
    let mut app = app(true);
    app.store.set_conn(ConnState::Connected {
        server_version: "1.6.9".to_owned(),
    });
    app
}

/// Line `y` of the screen, without trailing blanks.
fn line(buf: &Buffer, y: u16) -> String {
    text(buf)
        .lines()
        .nth(usize::from(y))
        .unwrap()
        .trim_end()
        .to_owned()
}

#[test]
fn disconnected_no_daemon_banner_80x24() {
    let app = disconnected(DisconnectReason::NoDaemon, Duration::from_secs(4), 3);
    let buf = render(&app, 80, 24);
    // Body = lines 2…22; the banner is at body.y + body.height / 3 (08 §3).
    assert_eq!(
        line(&buf, 9),
        "             ○ PipeWire not available — retrying in 4 s (attempt 3)"
    );
    assert_eq!(line(&buf, 1), " Show: Applications (f)", "no count");
    assert!(line(&buf, 23).starts_with(" ○ disconnected │"));
    insta::assert_snapshot!(framed(&buf));
}

#[test]
fn disconnected_connection_lost_text() {
    // The countdown rounds up and comes from the injected clock.
    let app = disconnected(
        DisconnectReason::ConnectionLost,
        Duration::from_millis(1200),
        2,
    );
    let screen = text(&render(&app, 80, 24));
    assert!(
        screen.contains("○ Connection to PipeWire lost — retrying in 2 s "),
        "{screen}"
    );
    assert!(!screen.contains("attempt"), "{screen}");
    let mut later = app;
    later.ui.now += Duration::from_millis(1300);
    let screen = text(&render(&later, 80, 24));
    assert!(screen.contains("retrying in 0 s"), "{screen}");

    let denied = disconnected(DisconnectReason::PermissionDenied, Duration::ZERO, 1);
    let screen = text(&render(&denied, 80, 24));
    assert!(
        screen.contains("○ Permission denied connecting to PipeWire"),
        "{screen}"
    );
}

#[test]
fn connecting_text() {
    let buf = render(&app(true), 80, 24);
    assert_eq!(line(&buf, 9).trim_start(), "Connecting to PipeWire…");
    assert!(line(&buf, 23).starts_with(" ○ connecting… │"));
}

#[test]
fn empty_playback_hint_text() {
    let buf = render(&connected(), 80, 24);
    assert_eq!(
        line(&buf, 9),
        "                              No playback streams"
    );
    assert_eq!(
        line(&buf, 10),
        "                   Applications that play audio appear here."
    );
    assert!(buf[(30, 9)].modifier.contains(Modifier::DIM));
    assert_eq!(
        line(&buf, 1).trim_end(),
        format!(" Show: Applications (f){}0 streams", " ".repeat(46))
    );
    insta::assert_snapshot!(framed(&buf));
}

#[test]
fn empty_with_hidden_streams_text() {
    let mut app = connected();
    for id in [1, 2] {
        app.store.apply(StoreEvent::NodeAdded {
            id: ObjectId(id),
            serial: Serial(u64::from(id)),
            props: [
                ("media.class", "Stream/Output/Audio"),
                ("node.virtual", "true"),
            ]
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        });
    }
    let screen = text(&render(&app, 80, 24));
    assert!(
        screen.contains("No streams match this filter (2 hidden)"),
        "{screen}"
    );
    assert!(screen.contains("Press f to change the filter."), "{screen}");
}

/// The `default_system` fixture (02 §2.3 mockup streams) on a `width`×`height`
/// terminal, all initial events applied; the first row is selected.
#[cfg(feature = "mock")]
fn fixture(width: u16, height: u16) -> App {
    use vol::{
        backend::{fixtures::default_system, mock::spawn_mock},
        event::Event,
    };
    let (handle, _control) = spawn_mock(default_system());
    let options = Options {
        color: true,
        mouse: false,
    };
    let mut app = App::new(options, handle, Instant::now());
    app.resize(width, height);
    while let Ok(event) = app.backend.events.try_recv() {
        app.on_event(Event::Backend(event));
    }
    app
}

#[cfg(feature = "mock")]
#[test]
fn playback_80x24_locked_rows() {
    let buf = render(&fixture(80, 24), 80, 24);
    assert_eq!(
        line(&buf, 3),
        "▌ [ ] ━━━━━━━━━━━━━━━━━━━━━━━━──────────┃────────────────  72%  -8.6 dB"
    );
    insta::assert_snapshot!(framed(&buf));
}

#[cfg(feature = "mock")]
#[test]
fn playback_60x12_mid_tier() {
    insta::assert_snapshot!(framed(&render(&fixture(60, 12), 60, 12)));
}

#[cfg(feature = "mock")]
#[test]
fn playback_40x16_narrow_tier() {
    insta::assert_snapshot!(framed(&render(&fixture(40, 16), 40, 16)));
}

#[cfg(feature = "mock")]
#[test]
fn over_100_percent_row_style() {
    // Spotify, 120 %: line 2 of the second row. G = 51, tick at bar cell 34
    // (column 40), fill 41 cells: cells 35…40 (columns 41…46) are over 100 %.
    let buf = render(&fixture(80, 24), 80, 24);
    assert_eq!(buf[(40, 6)].symbol(), "┃");
    for x in 41..=46 {
        assert_eq!(buf[(x, 6)].symbol(), "═", "column {x}");
        assert_eq!(buf[(x, 6)].fg, Color::Red, "column {x}");
    }
    assert_eq!(buf[(47, 6)].symbol(), "─");
    assert_ne!(
        buf[(39, 6)].fg,
        Color::Red,
        "fill left of the tick is not red"
    );
    for x in 58..=61 {
        assert_eq!(buf[(x, 6)].fg, Color::Red, "percent text, column {x}");
    }
}

#[cfg(feature = "mock")]
#[test]
fn muted_row_is_dim() {
    // mpv, muted and idle: the third row (lines 8…10).
    let buf = render(&fixture(80, 24), 80, 24);
    assert!(line(&buf, 9).starts_with("  [M] ━━━"));
    for y in 8..=10 {
        for x in 0..80 {
            let cell = &buf[(x, y)];
            if y == 9 && (2..5).contains(&x) {
                assert!(!cell.modifier.contains(Modifier::DIM), "[M] at {x}");
                assert_eq!(cell.fg, Color::Red, "[M] at {x}");
            } else {
                assert!(cell.modifier.contains(Modifier::DIM), "({x},{y})");
            }
        }
    }
    // The row above is not dimmed.
    assert!(!buf[(10, 6)].modifier.contains(Modifier::DIM));
}
