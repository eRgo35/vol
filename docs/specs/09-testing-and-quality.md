# 09 — Testing, quality, platforms, packaging

Area prefix `VOL-TEST` (testing), `VOL-QA` (local gate: lint/MSRV/hooks/release), `VOL-PLAT` (platforms), `VOL-PKG` (packaging).
**Verification is local.** The gate is `just check` (fmt, clippy, test, doc, deny, msrv), enforced before every push by the versioned git hook `scripts/hooks/pre-push` (installed with `scripts/install-hooks.sh`). GitHub Actions is used **only** to build and publish tagged releases (`.github/workflows/release.yml`, trigger `v*.*.*`); no workflow verifies pull requests or branches. The recipes are owned by the tooling, not by this spec; the requirements below say what the gate MUST enforce.

## 1. Test layers

| Layer | Where | Needs PipeWire? | What |
|---|---|---|---|
| L1 pure unit | `#[cfg(test)]` in `model/*`, `backend/coalesce.rs`, `keymap.rs` | no | reducer, volume math, view derivation, coalescing, key map |
| L2 property | `proptest` in `model/volume.rs`, `backend/coalesce.rs` | no | invariants (below) |
| L3 render snapshot | `src/ui/tests` or `tests/render.rs` with `ratatui::backend::TestBackend` | no | exact buffer text + style assertions |
| L4 scenario | `tests/scenarios.rs` with `MockBackend` | no | key sequences + scripted backend events → recorded `Command`s and screens |
| L5 live (manual or opt-in) | `tests/live.rs`, `#[ignore]`, env `VOL_TEST_PIPEWIRE=1` | yes | backend against a real/private daemon; the VERIFY list |

Dev-dependencies to add: `proptest`, `insta` (snapshots; `cargo insta` optional — snapshots are plain `.snap` files committed under `tests/snapshots/`). No `mockall`: the mock backend is hand-written (`backend/mock.rs`) because scripts are order-sensitive.

## 2. Mock backend

```rust
pub struct MockScript { pub initial: Vec<BackendEvent> }       // delivered right after spawn
pub struct MockControl {
    pub fn push(&self, ev: BackendEvent);                       // simulate daemon traffic
    pub fn commands(&self) -> Vec<Command>;                     // everything the app sent, in order
    pub fn fail_next(&self, err: BackendError);                 // next command yields CommandFailed
}
pub fn fixtures::default_system() -> MockScript;                // 2 sinks, 2 sources, 1 card, 4 playback streams (3 applications + 1 virtual), 1 capture stream; stereo except one mono source and the capture stream; defaults set
pub fn fixtures::empty_connected() -> MockScript;
pub fn fixtures::disconnected(NoDaemon) -> MockScript;
```

`fixtures::default_system()` mirrors the observed live graph shapes (property names/values from `pw-dump` of PipeWire 1.6.9: sink `alsa_output.pci-0000_28_00.4.analog-stereo` with `device.id`, `card.profile.device`, `Props.channelVolumes=[0.658492,0.658492]`; device with `EnumProfile`/`EnumRoute`/`Route`; metadata `default.audio.sink`, `default.configured.audio.sink`).

## 3. Required tests

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-TEST-001 | MUST | Reducer: for each `StoreEvent` kind a table test covering add, update, remove, unknown-id, duplicate; plus VOL-ARCH-022 cascade (node removal drops its ports/links/target). | Implemented |
| VOL-TEST-002 | MUST | Reducer: `Disconnected` clears maps; `Connected` sets `conn`; `revision` increments iff `apply` returned `true`. | Implemented |
| VOL-TEST-003 | MUST | Coalescing (L2): for any random event vector the backend can produce (an `*Added` never repeats an id that is still live), `coalesce(events)` applied to a `Store` yields the **same final `Store`** as applying `events` one by one (equivalence property), except `revision`. | Implemented |
| VOL-TEST-004 | MUST | Volume math (L2): VOL-VM-005 (round trip for 0..=150), VOL-VM-006 (dB anchors, monotonic), step clamp (`p+Δ` within 0..=150), locked scaling keeps channel ratios within `1e-5` and lands the max channel exactly on the target percent, unlocked touches one channel only. | Missing |
| VOL-TEST-005 | MUST | `Level`/gain write path: percent 87 on a node with `volume=1.0` writes `channelVolumes = [0.658503; 2]` (= 0.87³ as `f32`, tolerance 1e-6); on `volume=0.5` writes `0.658503/0.5`; on `volume=0` writes `master=Some(1.0)`. | Missing |
| VOL-TEST-006 | MUST | View derivation: title fallback chains (02 §3.2, 04 §3, 06 §2), `is_virtual_stream`, `is_hardware`, filter results, sort order, port resolution incl. unplugged/none/one-candidate, stream→sink target label incl. monitor ports, vol's own meter streams hidden. Fixtures derive from observed props. | Missing |
| VOL-TEST-007 | MUST | Default detection from metadata JSON: `{"name":"x"}` ok; malformed/other-type values → `None`, no panic. | Implemented |
| VOL-TEST-020 | MUST | Keymap table test: every row of 07 §2–§4 (mode × key → `Action`), incl. `Ctrl-m ≠ m`, `Release` ignored, `SHIFT`-insensitive capitals, popup-mode `q` = close. | Missing |
| VOL-TEST-030 | MUST | Render snapshots (L3), text only, at **80×24**, **60×10**, **40×16**, **40×12**, and too-small **39×12**/**40×11**: Playback (locked, unlocked, muted, > 100 %, `--` volume), Recording (monitor source label), Output (default, unplugged port, popup), Input, Configuration (popup, unavailable profile), help overlay, disconnected, empty states. The mockups in docs/specs MUST match the snapshots character for character for the same fixtures (or the docs be regenerated). | Missing |
| VOL-TEST-031 | MUST | Style assertions: selected row has `▌` with cyan fg; > 100 % cells use `bar_over` red; muted row has DIM; mono mode has no non-reset fg/bg (VOL-VIS-007). | Missing |
| VOL-TEST-032 | MUST | Bar geometry: for G ∈ {3, 29, 40, 47, 51} and p ∈ {0, 1, 50, 99, 100, 101, 150}: fill and tick positions match VOL-VIS formulas (table test). | Implemented |
| VOL-TEST-033 | MUST | Meter mapping: `peak 1.0 → D=1.0`; `0.001 → 0.0` (−60 dBFS); decay 20 dB/s verified over simulated ticks; glyph for each `steps ∈ 0..=64`. | Missing |
| VOL-TEST-040 | MUST | Scenario: press `l` on the selected stream → exactly one `SetVolumes` after ≤16 ms coalescing, value `(0.77)³` for 72→77 %; pressing 10 times in one frame yields one command with the final value (VOL-ARCH-035). | Missing |
| VOL-TEST-041 | MUST | Scenario: `m` → `SetMute{mute: !current}`; `d` on Output tab → `SetDefault{Sink, node.name}`; `d` on Playback → status `Streams have no default`, no command. | Missing |
| VOL-TEST-042 | MUST | Scenario: open move popup, remove the stream node via `MockControl::push`, popup closes with `Stream ended`; selection moves per VOL-ROW-004. | Missing |
| VOL-TEST-043 | MUST | Scenario: `Disconnected{NoDaemon}` at startup → banner rendered, no panic; later `Connected` + `Batch` → rows appear; `SetMeters` re-sent (VOL-ARCH-045). | Missing |
| VOL-TEST-044 | MUST | Scenario: `fail_next(NodeGone)` → status message rendered for 4 s (time injected), then cleared. | Missing |
| VOL-TEST-045 | MUST | Scenario: tab switch changes the desired meter set; leaving a tab sends `SetMeters` without that tab's targets. | Missing |
| VOL-TEST-050 | SHOULD | L5 live tests (ignored by default): (a) connect and receive `Connected` with ≥ 1 node; (b) set volume on a `support.null-audio-sink` created for the test and read back `channelVolumes` within 200 ms; (c) kill the private daemon → `Disconnected{ConnectionLost}` ≤ 2 s, restart → `Connected` ≤ 12 s; (d) meter on a null sink playing `pw-cat --playback` of a −6 dBFS sine reports peak ≈ 0.5 ± 0.05. | Missing |

## 4. Manual acceptance checklist (live PipeWire + WirePlumber)

Each item is binary. "Compare" items use black-box CLIs only.

| ID | Check |
|---|---|
| MA-01 | Start with `PIPEWIRE_REMOTE=/nonexistent` → UI shows banner, no panic; start the daemon → rows appear ≤ 12 s. |
| MA-02 | Default sink row: vol % == `round(100 × wpctl get-volume @DEFAULT_AUDIO_SINK@)`; dB == `pactl get-sink-volume @DEFAULT_SINK@` dB ±0.05. |
| MA-03 | `l` ×1 raises by 5 points; `wpctl get-volume` agrees within one frame; Plasma/GNOME volume applet follows. |
| MA-04 | `m` toggles mute; `wpctl get-volume` prints `[MUTED]`. |
| MA-05 | 150 % reachable (`End`), bar shows `═` beyond the tick; `wpctl get-volume` prints 1.50. |
| MA-06 | `d` on a non-default sink: `wpctl status` star moves; vol star follows; survives `systemctl --user restart wireplumber` per WirePlumber's own persistence. |
| MA-07 | Move a playing stream (`pw-cat --playback`) between two sinks via popup; `pw-link -l` shows the new link ≤ 1 s; choose "Default device" → returns to the default sink. |
| MA-08 | Port popup on a card with 2 output routes switches the route (`pw-cli enum-params <dev> Route` changes `index`). |
| MA-09 | Profile popup on a card switches profile; sinks/sources appear/disappear live in tabs 3–4. |
| MA-10 | Meters: play a sine; Output row meter moves ≈ 25 Hz, falls ~20 dB/s after stopping; Recording meter follows source level. |
| MA-11 | Kill `pipewire` while running → banner ≤ 2 s; no stuck terminal state; `q` exits cleanly (cursor, alternate screen restored). |
| MA-12 | `NO_COLOR=1 vol` shows no color; narrow terminal 40×12 works; 39×12 shows too-small message. |
| MA-13 | `vol` leaves no `vol.meter.*` nodes behind after exit (`pw-cli ls Node` output contains no `vol.meter`) and after `kill -TERM`. |
| MA-14 | Every `VERIFY:` item (see SPEC.md §VERIFY index) recorded as confirmed/refuted in the PR that implements it. |

## 5. Quality gates

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-QA-001 | MUST | `cargo fmt --all -- --check` clean. | Implemented |
| VOL-QA-002 | MUST | `cargo clippy --all-targets --all-features --locked -- -D warnings` clean on stable and on MSRV. Additional crate-level lints: `#![deny(unsafe_code)]` (exception: `backend/pw/**` with a `// SAFETY:` comment per unsafe block, only if `libspa`'s safe pod API proves insufficient), `clippy::unwrap_used`/`expect_used` = warn outside tests, `clippy::dbg_macro`/`todo`/`print_stdout`/`print_stderr` = deny (stdout/stderr are the terminal). | Implemented (`[lints]` in `Cargo.toml`) |
| VOL-QA-003 | MUST | MSRV **1.88** (required by `ratatui 0.30`; raised from 1.85, 2026-10-24); `rust-version = "1.88"` in `Cargo.toml`; `just msrv` runs `cargo +1.88 check --locked --all-features --all-targets`. Verified 2026-10-24: `pipewire 0.10.1`/`libspa 0.10.1`, `proptest 1.12`, `insta 1.49` compile on 1.88. | Implemented |
| VOL-QA-004 | MUST | `just test` (`cargo test --locked --all-features --all-targets`) runs on the developer's Linux machine with libpipewire headers installed, **without** requiring a PipeWire daemon. L5 tests are `#[ignore]`. | Implemented |
| VOL-QA-005 | MUST | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --locked` clean. | Implemented |
| VOL-QA-006 | SHOULD | `cargo deny check` (licenses permissive-only; `pipewire`/`libspa` are MIT). | Implemented (duplicate-version warnings only) |
| VOL-QA-007 | MUST | No test or code path touches the user's real audio session unless `VOL_TEST_PIPEWIRE=1`. | Implemented |
| VOL-QA-008 | MUST | Public behavior changes update `README.md` (usage, keys, flags, requirements) in the same change; the template README is replaced. | Missing |
| VOL-QA-009 | MUST | The pre-push hook (`scripts/hooks/pre-push`) runs `just check` and is the enforcement point for VOL-QA-001…006; `git push --no-verify` is reserved for emergencies and MUST NOT be used to land a milestone. | Implemented (hook and installer exist) |
| VOL-QA-010 | MUST | GitHub Actions runs **only** on tags `v*.*.*` (build + checksum + GitHub Release); no PR/branch verification workflow exists or is proposed. | Implemented (`release.yml`) |

## 6. Supported platforms

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-PLAT-001 | MUST | **Linux only** (x86_64, aarch64). macOS/Windows are unsupported; the crate need not build there and no gate or workflow targets them (`pipewire-sys` fails to build without libpipewire). | Implemented (README, CONTRIBUTING) |
| VOL-PLAT-002 | MUST | Build requires `libpipewire-0.3` development files (pkg-config `libpipewire-0.3`), `libspa-0.2`, and libclang (bindgen). | Implemented (README build table, `flake.nix`) |
| VOL-PLAT-003 | MUST | Runtime: a PipeWire daemon (`pipewire`, protocol native socket) and WirePlumber for defaults/moves/profile persistence. Developed and observed against PipeWire **1.6.9** / WirePlumber of the same release. Lower bounds for features used are `[INFERENCE]`: Route/EnumRoute/Profile params and `target.object` metadata ≥ PipeWire 0.3.65 / WirePlumber 0.4.12 — VERIFY by running MA-02…MA-09 on the oldest supported distro (Debian 12: PipeWire 0.3.65). | Missing |
| VOL-PLAT-004 | MUST | No PulseAudio fallback: if only `pipewire-pulse`/PulseAudio exists without a native PipeWire socket, the app shows the disconnected banner. | Implemented (vol only speaks the native protocol; no socket → `NoDaemon` banner, `tests/render.rs::disconnected_no_daemon_banner_80x24`) |
| VOL-PLAT-005 | SHOULD | Works inside a terminal multiplexer; requires `TERM` with 8 basic colors + UTF-8. 256/truecolor not required. | Missing |

## 7. Packaging notes

| Target | Build dependencies | Runtime |
|---|---|---|
| Debian/Ubuntu | `build-essential pkg-config libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev` | `libpipewire-0.3-0`, `pipewire`, `wireplumber` |
| Fedora | `pipewire-devel clang-devel pkgconf-pkg-config` | `pipewire`, `wireplumber` |
| Arch | `pipewire clang pkgconf` (+ `base-devel`) | `pipewire`, `wireplumber` |
| Nix (`flake.nix`) | add to `devShells.default.packages`: `pipewire`, `llvmPackages.libclang`, `pkg-config` (already), and `env.LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib"`; for `rustPlatform.buildRustPackage` use `nativeBuildInputs = [ pkg-config rustPlatform.bindgenHook ]`, `buildInputs = [ pipewire ]`. Drop `x86_64-darwin`/`aarch64-darwin` from `supportedSystems`; drop `openssl` (unused). | `pipewire`, `wireplumber` |
| Release workflow (tag `v*.*.*`, ubuntu-latest; already installs these) | `sudo apt-get update && sudo apt-get install -y libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev pkg-config` | none |

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-PKG-001 | MUST | `README.md` documents the table above, runtime requirements, flags (`--mouse`, `--no-color`, `--no-meters`), env (`NO_COLOR`, `VOL_LOG`, `PIPEWIRE_REMOTE`), and keys (07). | Missing |
| VOL-PKG-002 | MUST | `flake.nix` dev shell can `cargo build` and `cargo test` without host packages (§7 Nix row). | Stub (`flake.nix` shell written but unverified: no Nix on the author machine; `just msrv` needs rustup; moved to M9) |
| VOL-PKG-003 | SHOULD | `Cargo.toml` gains `rust-version`, `repository`, `readme`, `keywords`, `categories = ["command-line-utilities", "multimedia::audio"]`; `description` stays "A simpler TUI version of pavucontrol". | Missing |
