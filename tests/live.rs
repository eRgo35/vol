//! L5 live tests against a **private** PipeWire daemon (VOL-QA-007).
//!
//! # Running
//!
//! `just test-live` (= `VOL_TEST_PIPEWIRE=1 cargo test --test live --
//! --ignored --test-threads=1`). Every test is `#[ignore]`d, so `just check`
//! never runs them, and each returns early unless `VOL_TEST_PIPEWIRE=1`.
//! Needs `pipewire` and `pw-cli`/`pw-dump` on `PATH`; `wireplumber` is
//! optional (tests that need metadata skip without it).
//!
//! Each test starts its own daemon with `common::PrivateDaemon::start`, which
//! creates `$TMPDIR/vol-pw-<pid>-<n>/`, spawns `pipewire` (and `wireplumber`)
//! there, checks with `pw-cli info 0` that the socket is served by that child,
//! and on drop kills the daemons and removes the directory. The user's own
//! session is never contacted: every PipeWire process gets `env_clear()` plus
//! `PrivateDaemon::env()`.
//!
//! # Verified environment recipe (PipeWire 1.6.9, WirePlumber 0.5.18, 2026-10-24)
//!
//! * Socket location: the daemon creates `pipewire-0` (+ `pipewire-0-manager`)
//!   in `$PIPEWIRE_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR`. Clients resolve
//!   `$PIPEWIRE_REMOTE` (an absolute socket path works), else
//!   `$PIPEWIRE_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR`; each one alone was
//!   enough. With none of them set the client fails with "Host is down"
//!   (no fallback to `/run/user/<uid>`). The helper sets both runtime vars.
//! * `PIPEWIRE_REMOTE` beats an explicit remote: with it set to a bogus path,
//!   `pw-cli -r <private socket>` fails. In-process clients must therefore run
//!   with `PIPEWIRE_REMOTE` unset (or pointing at the private socket).
//! * The socket path must fit `sun_path` (≤ 107 bytes): a long `TMPDIR`
//!   makes the daemon exit with "File name too long".
//! * D-Bus: both bus addresses point at a missing socket. `pipewire` then
//!   logs errors for rtkit, the portal module and RAOP discovery and runs
//!   fine; this also keeps network sinks (RAOP/Avahi) out of the session.
//! * `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `HOME` point inside the temp
//!   dir so neither the user's config drop-ins are read nor WirePlumber's
//!   restore state (`state/wireplumber/…`) is written to the real home.
//! * WirePlumber runs privately without a seat or D-Bus (optional D-Bus,
//!   portal, logind and MPRIS components are skipped). Its hardware monitors
//!   are disabled by a config drop-in (`hardware.audio`/`bluetooth`/
//!   `video-capture = disabled`) so it never opens the real sound cards. It
//!   still provides the `default` metadata and sets `default.audio.sink` to
//!   the null sink.
//! * Null sink: `pw-cli create-node adapter '{ factory.name=support.null-audio-sink
//!   node.name=<n> media.class=Audio/Sink object.linger=true audio.position=[FL FR] }'`;
//!   `object.linger` keeps it after `pw-cli` exits.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::thread::sleep;
use std::time::{Duration, Instant};

use common::{PrivateDaemon, require_live};

/// Polls `pw-dump` until `pred` holds, for at most `timeout`.
fn wait_for_dump(daemon: &PrivateDaemon, timeout: Duration, pred: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        let dump = daemon.pw_dump();
        if pred(&dump) || Instant::now() >= deadline {
            return dump;
        }
        sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "live: needs PipeWire; run with `just test-live`"]
fn private_daemon_starts_and_has_null_sink() {
    if !require_live() {
        return;
    }
    let mut daemon = PrivateDaemon::start(true);
    daemon.create_null_sink("vol-test-sink");

    let dump = wait_for_dump(&daemon, Duration::from_secs(3), |d| {
        d.contains("\"node.name\": \"vol-test-sink\"")
    });
    assert!(dump.contains("\"node.name\": \"vol-test-sink\""), "{dump}");
    assert!(dump.contains("\"media.class\": \"Audio/Sink\""), "{dump}");
    assert!(
        !dump.contains("\"media.class\": \"Audio/Device\""),
        "real hardware leaked into the private session: {dump}"
    );

    if daemon.has_wireplumber() {
        let dump = wait_for_dump(&daemon, Duration::from_secs(3), |d| {
            d.contains("\"default.audio.sink\"")
        });
        assert!(
            dump.contains(r#""key": "default.audio.sink", "type": "Spa:String:JSON", "value": { "name": "vol-test-sink" }"#),
            "{dump}"
        );
    }
}
