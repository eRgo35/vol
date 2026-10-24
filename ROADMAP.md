# vol — Roadmap

Last updated: 2026-10-21. Spec: [`SPEC.md`](SPEC.md) and `docs/specs/`. Execution detail: [`PLAN.md`](PLAN.md) and `docs/plans/`. Requirement → milestone map: [`docs/plans/coverage.md`](docs/plans/coverage.md).

## Ground rules

* **Verification is local.** A milestone is done when `just check` (fmt, clippy `-D warnings`, test, doc `-D warnings`, deny, msrv 1.88) passes on the author's Linux machine, the pre-push hook (`scripts/hooks/pre-push`, installed by `scripts/install-hooks.sh`) lets the push through, and the milestone's manual acceptance items pass against a live PipeWire session. GitHub Actions is used **only** to build and publish a release when a `v*.*.*` tag is pushed (`.github/workflows/release.yml`). No milestone adds, requires or relies on CI verification (VOL-GEN-008).
* Milestone status values: `Not started`, `In progress`, `Done`. A milestone is `Done` only when every requirement assigned to it has status `Implemented` in the specs (or `n/a`), its exit criteria hold, and its plan's Definition of Done is ticked.
* Each requirement ID belongs to exactly one milestone (the one that delivers its last clause). Plans note partial deliveries.
* Ordering follows the dependency chain *types → pure reducer → backend → read-only UI → writes → more tabs → meters → polish → release*. Pure code (model, volume math, views, keymap, layout) is always written and tested before the code that needs a PipeWire daemon.

## Overview

| Milestone | Title | Size | Depends on | IDs | Plan | Status |
|---|---|---|---|---|---|---|
| M0 | Foundation and refactor | M | — | 47 | [`docs/plans/00-foundation.md`](docs/plans/00-foundation.md) | Done |
| M1 | PipeWire backend and registry mirror | L | M0 | 30 | [`docs/plans/01-backend-store.md`](docs/plans/01-backend-store.md) | Done |
| M2 | Playback tab (read-only) | L | M1 | 25 | [`docs/plans/02-playback-readonly.md`](docs/plans/02-playback-readonly.md) | Not started |
| M3 | Volume and mute writes | M | M2 | 20 | [`docs/plans/03-volume-mute.md`](docs/plans/03-volume-mute.md) | Not started |
| M4 | Recording tab and stream move | M | M3 | 12 | [`docs/plans/04-recording-move.md`](docs/plans/04-recording-move.md) | Not started |
| M5 | Output and Input devices, defaults, ports | L | M4 | 19 | [`docs/plans/05-output-input.md`](docs/plans/05-output-input.md) | Not started |
| M6 | Configuration tab (card profiles) | S | M5 | 9 | [`docs/plans/06-configuration.md`](docs/plans/06-configuration.md) | Not started |
| M7 | Peak meters | M | M5 | 18 | [`docs/plans/07-meters.md`](docs/plans/07-meters.md) | Not started |
| M8 | Polish: help, mouse, NO_COLOR, snapshots | M | M7 | 10 | [`docs/plans/08-polish.md`](docs/plans/08-polish.md) | Not started |
| M9 | Packaging and release v0.1.0 | S | M8 | 5 | [`docs/plans/09-release.md`](docs/plans/09-release.md) | Not started |
| M10 | Backlog (deferred, post-0.1) | — | M9 | 4 | [`docs/plans/10-deferred.md`](docs/plans/10-deferred.md) | Not started |

Size: S ≈ 2–3 working days, M ≈ 4–6, L ≈ 8–12 (one developer, tasks ≤ ½ day each). Estimates are planning aids, not commitments.

## Milestones

### M0 — Foundation and refactor

**Status:** Done (2026-10-24) · **Size:** M · **Depends on:** — · **Plan:** [`docs/plans/00-foundation.md`](docs/plans/00-foundation.md)

**Goal.** Turn the template skeleton into the target architecture without any PipeWire code: library + binary split, template leftovers and `legacy.rs` removed, pure key map, dirty-flag draw loop, theme, CLI flags, size guard, tab bar, message/model types, `AudioBackend` trait with a scriptable mock, lints/MSRV/dev-dependencies, dev environment.

**SPEC requirement IDs (47).** `VOL-GEN-001`, `VOL-GEN-002`, `VOL-GEN-003`, `VOL-GEN-004`, `VOL-GEN-005`, `VOL-GEN-006`, `VOL-GEN-007`, `VOL-GEN-008`, `VOL-ARCH-001`, `VOL-ARCH-002`, `VOL-ARCH-003`, `VOL-ARCH-004`, `VOL-ARCH-005`, `VOL-ARCH-012`, `VOL-ARCH-033`, `VOL-ARCH-050`, `VOL-ARCH-060`, `VOL-ARCH-062`, `VOL-ARCH-063`, `VOL-ARCH-064`, `VOL-ARCH-065`, `VOL-ARCH-066`, `VOL-VIS-001`, `VOL-VIS-002`, `VOL-VIS-003`, `VOL-VIS-004`, `VOL-VIS-009`, `VOL-VIS-010`, `VOL-KEY-001`, `VOL-KEY-002`, `VOL-KEY-003`, `VOL-KEY-007`, `VOL-KEY-009`, `VOL-KEY-010`, `VOL-PB-011`, `VOL-PB-012`, `VOL-QA-001`, `VOL-QA-002`, `VOL-QA-003`, `VOL-QA-004`, `VOL-QA-005`, `VOL-QA-006`, `VOL-QA-007`, `VOL-QA-009`, `VOL-QA-010`, `VOL-PLAT-001`, `VOL-PLAT-002`

**Exit criteria.**

- `just check` passes (fmt, clippy `-D warnings`, test, doc `-D warnings`, deny, msrv 1.88) and the pre-push hook is installed.
- `src/legacy.rs`, `App.counter`, `AppEvent::Increment/Decrement` and the `fn xxx_tab(&App) -> String` stubs no longer exist.
- `vol --help`/`--version` print and exit 0; an unknown flag exits 2 before raw mode.
- Running `vol`: five tabs switch with `Tab`/`Shift-Tab`/`[`/`]`/`F1`–`F5`; `q`/`Ctrl-C` quit, `Esc` does not; terminal < 40×12 shows the too-small message; `kill -TERM` restores the terminal.
- `MockBackend` can deliver scripted `BackendEvent`s to `App` and records `Command`s (unit-tested).
- Render snapshot tests exist for 80×24, 40×12, 39×12, 40×11.

### M1 — PipeWire backend and registry mirror

**Status:** Done (2026-10-24) · **Size:** L · **Depends on:** M0 · **Plan:** [`docs/plans/01-backend-store.md`](docs/plans/01-backend-store.md)

**Goal.** A `vol-pw` thread that mirrors nodes, devices, ports, links and the `default` metadata into a pure `Store` via a reducer, with coalescing, reconnect/backoff, shutdown and file logging. Headless: verified by unit tests and opt-in live tests, no new UI.

**SPEC requirement IDs (30).** `VOL-ARCH-010`, `VOL-ARCH-011`, `VOL-ARCH-013`, `VOL-ARCH-014`, `VOL-ARCH-015`, `VOL-ARCH-016`, `VOL-ARCH-020`, `VOL-ARCH-021`, `VOL-ARCH-022`, `VOL-ARCH-023`, `VOL-ARCH-025`, `VOL-ARCH-026`, `VOL-ARCH-027`, `VOL-ARCH-028`, `VOL-ARCH-030`, `VOL-ARCH-031`, `VOL-ARCH-032`, `VOL-ARCH-040`, `VOL-ARCH-041`, `VOL-ARCH-042`, `VOL-ARCH-043`, `VOL-ARCH-046`, `VOL-ARCH-052`, `VOL-ARCH-053`, `VOL-ARCH-054`, `VOL-IN-009`, `VOL-TEST-001`, `VOL-TEST-002`, `VOL-TEST-003`, `VOL-TEST-007`

**Exit criteria.**

- Reducer, metadata parsing and coalescing have the unit/property tests of VOL-TEST-001/002/003/007 passing.
- `VOL_TEST_PIPEWIRE=1 cargo test --test live -- --ignored` passes against a private daemon: `Connected` after initial snapshot, ≥ 1 sink in the store, `Disconnected{NoDaemon}` when the socket is absent, `Connected` again after the daemon is started (≤ 12 s).
- `VOL_LOG=debug vol` on the author machine logs a store summary (counts of nodes/devices/ports/links, default sink/source names) that matches `pw-dump`.
- VF-01 and VF-02 resolved and recorded in `SPEC.md` §7 and the plan; Q3 decided.
- Quit within 1 s leaves no `vol` thread/proxy behind (`Command::Shutdown` join budget).

**Manual acceptance (SPEC 09 §4).** (partial) MA-01 no-panic without daemon

### M2 — Playback tab (read-only)

**Status:** Not started · **Size:** L · **Depends on:** M1 · **Plan:** [`docs/plans/02-playback-readonly.md`](docs/plans/02-playback-readonly.md)

**Goal.** First real screen: live Playback rows with title/target/state, volume bar, percent, dB, mute indicator, channel summary, filter, selection, scrolling, status line, disconnected/empty states. No writes.

**SPEC requirement IDs (25).** `VOL-ARCH-024`, `VOL-ARCH-044`, `VOL-VM-001`, `VOL-VM-002`, `VOL-VM-003`, `VOL-VM-005`, `VOL-VM-006`, `VOL-ROW-002`, `VOL-ROW-003`, `VOL-ROW-004`, `VOL-ROW-006`, `VOL-ROW-007`, `VOL-FLT-001`, `VOL-PB-001`, `VOL-PB-002`, `VOL-PB-003`, `VOL-PB-004`, `VOL-PB-008`, `VOL-MTR-006`, `VOL-KEY-004`, `VOL-KEY-011`, `VOL-VIS-005`, `VOL-VIS-008`, `VOL-PLAT-004`, `VOL-TEST-032`

**Exit criteria.**

- Playback tab renders live streams from a running session; adding/removing a stream (e.g. `pw-cat --playback`) updates the list without moving the selection off its `RowKey`.
- Snapshots at 80×24, 60×10, 40×16 (locked rows) match the 02-playback mockups for the same fixture.
- MA-01, MA-02 (read side) and MA-11 pass.
- Volume math tests (VOL-VM-005/006) and bar geometry (VOL-TEST-032) pass; Q11 decided.

**Manual acceptance (SPEC 09 §4).** MA-01, MA-02, MA-11

### M3 — Volume and mute writes

**Status:** Not started · **Size:** M · **Depends on:** M2 · **Plan:** [`docs/plans/03-volume-mute.md`](docs/plans/03-volume-mute.md)

**Goal.** All volume/mute/lock/channel interactions on the selected row, with cubic-root conversion, optimistic echo, key-repeat coalescing and error reporting.

**SPEC requirement IDs (20).** `VOL-ARCH-035`, `VOL-ARCH-036`, `VOL-ARCH-037`, `VOL-ARCH-051`, `VOL-VM-004`, `VOL-VM-007`, `VOL-VM-008`, `VOL-VM-009`, `VOL-VM-010`, `VOL-VM-011`, `VOL-VM-012`, `VOL-ROW-001`, `VOL-ROW-005`, `VOL-KEY-005`, `VOL-KEY-014`, `VOL-PB-005`, `VOL-TEST-004`, `VOL-TEST-005`, `VOL-TEST-040`, `VOL-TEST-044`

**Exit criteria.**

- `l`/`h`/`H`/`L`/`PgUp`/`PgDn`/digits/`Home`/`End`/`m`/`Space`/`c`/`,`/`.` work on Playback rows against a live session.
- MA-03, MA-04, MA-05 pass; written `channelVolumes` equal `(p/100)^3` ± 1e-6 (VOL-TEST-005).
- Scenario tests VOL-TEST-040/044 pass; VF-03, VF-04, VF-05 resolved; Q8 decided.

**Manual acceptance (SPEC 09 §4).** MA-03, MA-04, MA-05

### M4 — Recording tab and stream move

**Status:** Not started · **Size:** M · **Depends on:** M3 · **Plan:** [`docs/plans/04-recording-move.md`](docs/plans/04-recording-move.md)

**Goal.** Recording tab on the shared row model, `ListPopup`, move-stream via metadata for both Playback and Recording.

**SPEC requirement IDs (12).** `VOL-REC-001`, `VOL-REC-002`, `VOL-REC-003`, `VOL-REC-005`, `VOL-REC-006`, `VOL-REC-007`, `VOL-REC-008`, `VOL-PB-006`, `VOL-PB-007`, `VOL-FLT-002`, `VOL-FLT-003`, `VOL-TEST-042`

**Exit criteria.**

- Recording tab shows capture streams with source labels incl. `Monitor of <sink>`.
- Move popup moves a live stream (MA-07) and "Default device" restores policy routing.
- Popup closes with the right status when its stream disappears (VOL-TEST-042); VF-09 (and VF-10 outcome documented); Q4 decided.

**Manual acceptance (SPEC 09 §4).** MA-07

### M5 — Output and Input devices, defaults, ports

**Status:** Not started · **Size:** L · **Depends on:** M4 · **Plan:** [`docs/plans/05-output-input.md`](docs/plans/05-output-input.md)

**Goal.** Output Devices and Input Devices tabs: default markers and `d`, route (port) resolution and popup, hardware/virtual filters, hot-plug and disappearance behavior.

**SPEC requirement IDs (19).** `VOL-OUT-001`, `VOL-OUT-002`, `VOL-OUT-003`, `VOL-OUT-004`, `VOL-OUT-005`, `VOL-OUT-006`, `VOL-OUT-008`, `VOL-OUT-009`, `VOL-OUT-010`, `VOL-OUT-011`, `VOL-OUT-013`, `VOL-IN-001`, `VOL-IN-002`, `VOL-IN-003`, `VOL-IN-004`, `VOL-IN-005`, `VOL-IN-007`, `VOL-ARCH-047`, `VOL-TEST-041`

**Exit criteria.**

- `d` moves the default (MA-06) and `★` follows `default.audio.sink/source`.
- Port popup switches a route on a multi-port card (MA-08).
- Scenario test VOL-TEST-041 passes; VF-11, VF-12 resolved; Q7, Q13, Q14 decided.

**Manual acceptance (SPEC 09 §4).** MA-06, MA-08

### M6 — Configuration tab (card profiles)

**Status:** Not started · **Size:** S · **Depends on:** M5 · **Plan:** [`docs/plans/06-configuration.md`](docs/plans/06-configuration.md)

**Goal.** Card list with profile popup and live updates.

**SPEC requirement IDs (9).** `VOL-CFG-001`, `VOL-CFG-002`, `VOL-CFG-003`, `VOL-CFG-004`, `VOL-CFG-005`, `VOL-CFG-006`, `VOL-CFG-007`, `VOL-CFG-008`, `VOL-TEST-006`

**Exit criteria.**

- Profile switch works and Output/Input tabs follow (MA-09).
- VF-14 resolved; view-derivation tests of VOL-TEST-006 complete for all tabs.

**Manual acceptance (SPEC 09 §4).** MA-09

### M7 — Peak meters

**Status:** Not started · **Size:** M · **Depends on:** M5 · **Plan:** [`docs/plans/07-meters.md`](docs/plans/07-meters.md)

**Goal.** Capture-stream peak meters for Output/Input/Playback/Recording rows with desired-set diffing, windowing, decay and rendering; `--no-meters`.

**SPEC requirement IDs (18).** `VOL-MTR-001`, `VOL-MTR-002`, `VOL-MTR-003`, `VOL-MTR-004`, `VOL-MTR-005`, `VOL-MTR-007`, `VOL-MTR-008`, `VOL-ARCH-034`, `VOL-ARCH-045`, `VOL-VIS-006`, `VOL-PB-009`, `VOL-OUT-007`, `VOL-IN-006`, `VOL-REC-004`, `VOL-TEST-033`, `VOL-TEST-045`, `VOL-TEST-050`, `VOL-TEST-043`

**Exit criteria.**

- MA-10 and MA-13 pass.
- VF-06, VF-07 resolved; Q5, Q6 decided.
- Meter tests VOL-TEST-033/045 pass; live meter test (VOL-TEST-050 d) passes.

**Manual acceptance (SPEC 09 §4).** MA-10, MA-13

### M8 — Polish: help, mouse, NO_COLOR, snapshots

**Status:** Not started · **Size:** M · **Depends on:** M7 · **Plan:** [`docs/plans/08-polish.md`](docs/plans/08-polish.md)

**Goal.** Help overlay, optional mouse, full theme/`NO_COLOR` coverage, complete keymap and snapshot suites, terminal matrix.

**SPEC requirement IDs (10).** `VOL-ARCH-061`, `VOL-KEY-006`, `VOL-KEY-008`, `VOL-KEY-012`, `VOL-KEY-013`, `VOL-VIS-007`, `VOL-TEST-020`, `VOL-TEST-030`, `VOL-TEST-031`, `VOL-PLAT-005`

**Exit criteria.**

- Every row of 07 §2–§4 is in the keymap table test; snapshot suite of VOL-TEST-030 complete; mono-mode test passes.
- MA-12 passes; Q9 decided.

**Manual acceptance (SPEC 09 §4).** MA-12

### M9 — Packaging and release v0.1.0

**Status:** Not started · **Size:** S · **Depends on:** M8 · **Plan:** [`docs/plans/09-release.md`](docs/plans/09-release.md)

**Goal.** README, Cargo metadata, version bump, VERIFY closure, local release gate and the `v0.1.0` tag that triggers `release.yml`.

**SPEC requirement IDs (5).** `VOL-PKG-001`, `VOL-PKG-002`, `VOL-PKG-003`, `VOL-QA-008`, `VOL-PLAT-003`

**Exit criteria.**

- `just release-check` passes; all `VERIFY` items in SPEC §7 closed or explicitly waived; MA-01…MA-14 recorded.
- Tag `v0.1.0` pushed; `release.yml` publishes tarball + `SHA256SUMS`.

**Manual acceptance (SPEC 09 §4).** MA-14 (all MA-01…MA-13 re-run)

### M10 — Backlog (deferred, post-0.1)

**Status:** Not started · **Size:** — · **Depends on:** M9 · **Plan:** [`docs/plans/10-deferred.md`](docs/plans/10-deferred.md)

**Goal.** Specified-as-deferred features; not scheduled. Each becomes its own milestone when promoted.

**SPEC requirement IDs (4).** `VOL-PB-010`, `VOL-OUT-012`, `VOL-IN-008`, `VOL-CFG-009`

**Exit criteria.**

- n/a (promotion creates a new milestone and plan).

## Release policy

* Versions follow SemVer; `0.x` until all `MUST` requirements are `Implemented` and MA-01…MA-14 pass on two distributions (Fedora-class PipeWire 1.x and Debian 12-class PipeWire 0.3.65 — VOL-PLAT-003), then `1.0.0`.
* **v0.1.0** = end of M9. Steps (all local until the last): `just release-check` → bump `version` in `Cargo.toml`, run `cargo build` once so `Cargo.lock` picks it up (then `cargo build --locked` must pass) → rename `## [Unreleased]` to `## [0.1.0] - <date>` in `CHANGELOG.md` (and add a fresh empty `[Unreleased]`) → commit `chore(release): v0.1.0` → `git tag v0.1.0` → `git push origin main v0.1.0` (the exact procedure is also in `CONTRIBUTING.md` §Release process). The tag push is the **only** CI action; `release.yml` verifies tag == `Cargo.toml` version, builds `x86_64-linux`, uploads `vol-vX.Y.Z-x86_64-linux.tar.gz` + `SHA256SUMS`.
* GitHub release notes are generated from the commit history by `release.yml` (`generate_release_notes`), so commit subjects matter (PLAN.md §4); `CHANGELOG.md` (Keep a Changelog) is the curated record and is updated per user-visible change (PLAN.md §7).
* aarch64 and distro packages are post-0.1 (see Backlog); `release.yml` currently builds x86_64 only.

## Deferred / non-goals

Non-goals (SPEC §3.1, will not be built): PulseAudio client/fallback; sound-server configuration (pipewire/WirePlumber config editing, clock/quantum); module loading (tunnel/RTP/combine/loopback creation), EQ/effects; Bluetooth pairing and codec selection (v1); recording/playback features (vol only captures for meters); video/MIDI/JACK-specific UI and patchbay; its own persistence or config file; i18n; ASCII-only mode (v1); macOS/Windows; `Ctrl-Z` suspend; text input (rename).

Deferred (specified, assigned to M10, not in v0.1): terminate stream (VOL-PB-010), latency offset (VOL-OUT-012), "Monitor of …" entries in Input Devices and moving capture streams to a monitor (VOL-IN-008, 03 §5), Configuration "Show: All Devices" and Bluetooth codec (VOL-CFG-009).

Dropped from pavucontrol (SPEC §3.3): System Sounds/event-role row, icons, stream-restore UI, digital pass-through, base-volume markers, port availability groups, dialogs/menus.

## Backlog / post-1.0 ideas

Unscheduled; each needs a spec section before promotion. Candidates: terminate stream; latency offset; sink monitors as input entries and monitor-targeting moves; Bluetooth codec/profile helpers; per-channel balance control; type-to-filter/search; config/state file (persist filters, channel lock, key remap); theming and ASCII-safe glyph set; shell completions and man page; aarch64 release artifact and distro packages (AUR, Nix package, deb/rpm); `cargo install` story; saved "scenes" (device+volume presets); `--json` one-shot status (non-TUI) for scripting; optional PulseAudio-protocol read-only fallback (currently a non-goal, revisit only on demand).

## Decision log (Open Questions from SPEC §8)

Each question has a **Proposed default** so work proceeds; the project owner may overrule any of them at any time. "Decide by" is the milestone whose first task needs the answer; "If overruled" lists the specs/plans to edit.

| Q | Proposed default | Decide by | If overruled |
|---|---|---|---|
| Q1 `Esc` quits? | **Decided (2026-10-24, M0-T08):** `Esc` never quits; `q`/`Ctrl-C` quit (VOL-KEY-007). | M0-T08 | Edit VOL-KEY-001/007, `keymap` tests; ARCH-066 note. |
| Q2 Digits = absolute volume vs tab select | **Decided (2026-10-24, M0-T08):** digits set volume; tabs via `Tab`/`[`/`]`/`F1`–`F5`, optional `Alt-n` (VOL-KEY-005/013). | M0-T08 | Edit 07 §2.1/§2.2, help text, keymap table; `Alt-n` becomes `n`. |
| Q3 Stay on `pipewire 0.8.0`? | **Decided: upgrade to `pipewire 0.10.1` (0.8.0 / `libspa 0.8.0` no longer compile against PipeWire 1.6.9), 2026-10-24.** All dependencies bumped to latest together (ratatui 0.30 + crossterm 0.29); MSRV raised to 1.88. M1-T01 verifies the 0.10.1 API surface. | M1-T01 | Re-pin in `Cargo.toml`, adapt VOL-ARCH-015/016 names; MSRV re-check (VOL-QA-003). |
| Q4 Metadata keys for moving streams | **Proposed default:** write `target.node` and `target.object` as `Spa:Id`; drop the redundant one if VF-09 shows it unnecessary. | M4-T01 | Edit 02 §3.4, backend `pw/metadata.rs`, MA-07. |
| Q5 Meter a `Stream/Output/Audio` node? | **Proposed default:** try; if refused, Playback rows show empty meters. | M7-T01 | Edit 02 §2.4 table, VOL-PB-009. |
| Q6 Passive vs active source meters | **Proposed default:** passive (`node.passive=true`), `--no-meters` available. | M7-T01 | Edit 02 §2.4 properties, 05 §4 privacy text; adds a flag if active is chosen. |
| Q7 `Audio/Duplex` / monitor entries as inputs | **Proposed default:** not stored / deferred (VOL-IN-008/009). | M5-T01 | Edit 01 §5 node-kind table, 05; moves IN-008 into a milestone. |
| Q8 Locked-channel scaling | **Proposed default:** balance-preserving (VOL-VM-008). | M3-T01 | Edit VM-008, `volume.rs` tests (VOL-TEST-004). |
| Q9 ASCII fallback | **Proposed default:** none in v1. | M8-T01 | New requirement + glyph table in 08 §10; theme gains a glyph set. |
| Q10 Dependencies/logging | **Decided (dev-deps, 2026-10-24):** `proptest`, `insta` dev-only (M0). **Proposed default:** `serde_json` yes (metadata JSON, M1); hand-rolled file logger, off unless `VOL_LOG` (M1). | M0-T02 / M1-T06 | Swap in `tracing` + `tracing-appender` (ARCH-054 text). |
| Q11 Master display for unequal channels | **Proposed default:** max (VOL-VM-002). | M2-T01 | Edit VM-002/008 and tests. |
| Q12 Persist UI state | **Proposed default:** no persistence in v1. | post-0.1 | New config-file spec; Backlog item. |
| Q13 Hardware vs Virtual heuristic | **Proposed default:** `device.id` present and `node.virtual != true`. | M5-T01 | Edit 04 §2, `view::is_hardware` + tests. |
| Q14 Sessions without WirePlumber | **Proposed default:** supported for volume/mute/ports/profiles; defaults/moves report unavailability (VOL-ARCH-047). | M5-T01 | Edit ARCH-047, OUT-004. |

## Risks (cross-cutting)

| Risk | Impact | Mitigation |
|---|---|---|
| `pipewire-rs 0.10.1` lacks an API (Metadata setter, pod helpers) | M1/M4 blocked | M1-T01 spike first; fall back to `pipewire::sys` with `// SAFETY:` blocks isolated in `backend/pw/` (VOL-QA-002 exception) or upgrade (Q3). |
| WirePlumber behavior for `target.*`, defaults and route saving differs between versions | M4/M5 behavior | Manual verification on the author's 1.6.9 system; record observed behavior in SPEC; oldest-version check deferred to M9 (VF-16). |
| Real-session tests change the user's audio | Annoyance/data loss | Live tests are `#[ignore]` + `VOL_TEST_PIPEWIRE=1` and use a private daemon/null sinks (VOL-QA-007). |
| Local-only verification can be bypassed (`--no-verify`) | Broken main | Hook + PR template checklist; PLAN.md forbids `--no-verify` for milestone merges (VOL-QA-009). |
| Dependabot bumps (ratatui, its crossterm, pipewire) | Compile errors / type mismatch | Do not merge bumps independently; use `ratatui::crossterm` re-export; bump together deliberately and run `just check`. |
| Scope creep from pavucontrol parity | Schedule | Deferred/non-goal lists are normative; promotion needs a spec section. |
