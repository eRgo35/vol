# M0 — Foundation and refactor

**Status:** Not started · **Size:** M · **Depends on:** — · **Branch:** `m0/foundation` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m0--foundation-and-refactor)

**Goal.** Turn the template skeleton into the target architecture without any PipeWire code: library + binary split, template leftovers and `legacy.rs` removed, pure key map, dirty-flag draw loop, theme, CLI flags, size guard, tab bar, message/model types, `AudioBackend` trait with a scriptable mock, lints/MSRV/dev-dependencies, dev environment.

## Requirement IDs assigned to this milestone (48)

`VOL-GEN-001`, `VOL-GEN-002`, `VOL-GEN-003`, `VOL-GEN-004`, `VOL-GEN-005`, `VOL-GEN-006`, `VOL-GEN-007`, `VOL-GEN-008`, `VOL-ARCH-001`, `VOL-ARCH-002`, `VOL-ARCH-003`, `VOL-ARCH-004`, `VOL-ARCH-005`, `VOL-ARCH-012`, `VOL-ARCH-033`, `VOL-ARCH-050`, `VOL-ARCH-060`, `VOL-ARCH-062`, `VOL-ARCH-063`, `VOL-ARCH-064`, `VOL-ARCH-065`, `VOL-ARCH-066`, `VOL-VIS-001`, `VOL-VIS-002`, `VOL-VIS-003`, `VOL-VIS-004`, `VOL-VIS-009`, `VOL-VIS-010`, `VOL-KEY-001`, `VOL-KEY-002`, `VOL-KEY-003`, `VOL-KEY-007`, `VOL-KEY-009`, `VOL-KEY-010`, `VOL-PB-011`, `VOL-PB-012`, `VOL-QA-001`, `VOL-QA-002`, `VOL-QA-003`, `VOL-QA-004`, `VOL-QA-005`, `VOL-QA-006`, `VOL-QA-007`, `VOL-QA-009`, `VOL-QA-010`, `VOL-PLAT-001`, `VOL-PLAT-002`, `VOL-PKG-002`

Parts of some requirements are delivered earlier or later than their owner milestone; each task below lists the IDs it touches. Verification: local only (`just check`).

## Starting point (commit `ce3e5a3`)

`src/{main,app,event,ui}.rs`, `src/tabs/*.rs` (five `fn …_tab(&App) -> String` stubs), uncompiled `src/legacy.rs`; binary-only crate; no tests; `Cargo.toml` without `rust-version`/lints/dev-deps. Tooling already in the working tree: `justfile`, `deny.toml`, `scripts/{install-hooks.sh,hooks/pre-push}`, `.github/workflows/release.yml`.

## Pre-flight (once)

* `scripts/install-hooks.sh`; `rustup toolchain install 1.88`; `pkg-config --modversion libpipewire-0.3` prints a version (author machine: 1.6.9); `just --version`, `cargo deny --version`.
* `git switch -c m0/foundation`.

## Tasks

### M0-T01 Baseline gate
* **Do:** run `just check` on the unmodified tree. Fix only what blocks it (fmt, unused imports, e.g. the `use crate::App` in `tabs/output_devices.rs`, clippy lints). If `msrv` fails because 1.88 is missing, install it. Note any `cargo deny` warnings (duplicate crate versions: warn only).
* **Files:** whatever the failures require (no behavior change).
* **Tests first:** none (existing: zero tests; `cargo test` must still pass).
* **Done when:** `just check` exits 0; `bash scripts/hooks/pre-push` exits 0.
* **Reqs:** VOL-QA-001, VOL-QA-005, VOL-QA-006, VOL-QA-009, VOL-QA-010 (verify only: `release.yml` untouched).

### M0-T02 Cargo hygiene, lints, dev-dependencies
* **Do:** in `Cargo.toml`: `rust-version = "1.88"`; `[features] mock = []`; `[dev-dependencies] proptest`, `insta` (latest versions that build on 1.88 — check with `just msrv`; pin lower if needed); `[lints.rust] unsafe_code = "deny"`; `[lints.clippy] unwrap_used = "warn"`, `expect_used = "warn"`, `dbg_macro = "deny"`, `todo = "deny"`, `print_stdout = "deny"`, `print_stderr = "deny"`. Allow the unwrap/expect lints in tests via `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` in `lib.rs` and `#![allow(clippy::unwrap_used, clippy::expect_used)]` at the top of each `tests/*.rs`. Do **not** add production dependencies yet (`serde_json` arrives in M1).
* **Files:** `Cargo.toml`, `Cargo.lock`.
* **Tests first:** none.
* **Done when:** `just check` green incl. `msrv` and `deny` (licenses of `insta`/`proptest` are permissive and allowed in `deny.toml`; if not, add a justified exception).
* **Reqs:** VOL-QA-002, VOL-QA-003, VOL-QA-004. Decides Q10 (dev-deps part).
* **Note:** keep `crossterm` at the version `ratatui 0.30` re-exports (0.29.x); import crossterm types via `ratatui::crossterm` to avoid dual-version type errors.

### M0-T03 Library + binary split
* **Do:** create `src/lib.rs` with `pub mod app; pub mod event; pub mod tabs; pub mod ui;` (more later); `main.rs` keeps only `fn main` using `vol::…`. Add crate docs line. Module visibility: `pub` (lib) so unused-code lints don't fire on not-yet-used pub items.
* **Files:** `src/lib.rs`, `src/main.rs`.
* **Tests first:** none (structure). `cargo doc` must not warn about lib/bin name collision (`just doc`); if it does, set `[[bin]] doc = false`.
* **Done when:** `just check` green; `cargo run` behaves as before.
* **Reqs:** VOL-ARCH-001 (partial).

### M0-T04 Delete the prototype
* **Do:** `git rm src/legacy.rs`; ensure nothing references it (`grep -rn legacy src/`). Its useful ideas (bars, 5 % step) are already in the spec; note in commit body that `j`/`k` were inverted there.
* **Done when:** `just check` green.
* **Reqs:** VOL-ARCH-002.

### M0-T05 Dev environment and build docs
* **Do:** `flake.nix`: add `just`, `pipewire` (the `dev` output supplies headers/pkg-config files), `clang`, `llvmPackages.libclang`, keep `pkg-config`; `env.LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib"`; drop `openssl` and the darwin systems (`x86_64-darwin`, `aarch64-darwin`). `README.md` already contains requirements, the build-package table and the key/flag tables (written by the tooling task): verify they match SPEC 09 §7 and keep the *planned/Implemented* column honest. Update the Nix paragraph in `CONTRIBUTING.md` (§Nix) once the flake is fixed.
* **Files:** `flake.nix`, `README.md` (verify only), `CONTRIBUTING.md` (Nix paragraph).
* **Tests first:** none (docs/env). If Nix is available: `nix develop -c just check`; otherwise record "not run (no nix)" in the commit body.
* **Done when:** README states Linux-only and lists the packages (already true); flake edits reviewed; `CONTRIBUTING.md` no longer says the shell is incomplete.
* **Reqs:** VOL-PLAT-001, VOL-PLAT-002, VOL-PKG-002.

### M0-T06 Model and message types (data only)
* **Do:** create the data types of 01 §4–§5 exactly (names/fields normative): `src/model/{mod,ids,props,node,device,port,link,metadata}.rs` (`ObjectId`, `Serial`, `PropMap`, `Node`, `NodeKind`, `NodeState`, `NodeAudio`, `Channel`, `Device`, `Profile`, `RouteInfo`, `ActiveRoute`, `Availability`, `PortDir`, `Port`, `Link`, `Defaults`, `StreamTarget`) and `src/backend/mod.rs` (`BackendEvent`, `DisconnectReason`, `MeterSample`, `StoreEvent`, `Command`, `DefaultKind`, `MeterTarget`, `MeterKind`, `CommandKind`, `BackendError`, `BackendGone`, trait `AudioBackend`, struct `BackendHandle`). Derive `Debug, Clone, PartialEq`; `Eq/Hash/Ord` for id types. `Store` itself is M1. Small pure constructors only: `Availability::from_spa(u32)` (0 unknown, 1 no, 2 yes; other → Unknown), `PortDir::from_prop(&str)` (`"in"`/`"out"`).
* **Files:** as listed + `src/lib.rs` mods.
* **Tests first:** `model::device::tests::availability_from_spa_ids`, `model::device::tests::availability_unknown_for_out_of_range`, `model::port::tests::port_dir_from_prop`, `model::ids::tests::ids_order_and_hash`, **`tests/layering.rs::model_and_coalesce_have_no_forbidden_imports`** (greps `src/model/**` and `src/backend/coalesce.rs` for `pipewire`, `libspa`, `ratatui`, `crossterm`, `tokio`), **`tests/layering.rs::pipewire_only_in_backend_pw`** (no `pipewire::`/`libspa::` outside `src/backend/pw/`).
* **Done when:** types compile, tests pass, `just check` green.
* **Reqs:** VOL-ARCH-001, VOL-ARCH-050 (typed errors), VOL-QA-007 (no PipeWire code path exists outside `backend/pw`).

### M0-T07 Mock backend and inert backend
* **Do:** `src/backend/mock.rs` (`#[cfg(any(test, feature = "mock"))]`): `MockScript { initial: Vec<BackendEvent> }`, `MockControl { push, commands, fail_next }`, `spawn_mock(script) -> (BackendHandle, MockControl)`; `BackendHandle::inert()` (real, always compiled: receiver that never yields, `send` records nothing and returns `Ok`) used by `main` until M1. Mock `send`: records the command; if `fail_next` is armed, emits `BackendEvent::CommandFailed{cmd: CommandKind, error}` instead of silently succeeding (command is still recorded). Fixtures (`default_system`) arrive in M2 once `Store` exists; here only `empty_connected()` and `disconnected(reason)`.
* **Files:** `src/backend/mock.rs`, `src/backend/mod.rs`.
* **Tests first:** `backend::mock::tests::initial_events_are_delivered_in_order`, `…::push_after_spawn_is_delivered`, `…::commands_are_recorded_in_order`, `…::fail_next_yields_command_failed_once`, `…::send_after_receiver_dropped_returns_backend_gone`, `backend::tests::inert_backend_never_emits_and_accepts_commands`.
* **Done when:** tests pass under `just test` (`--all-features` enables `mock`).
* **Reqs:** VOL-ARCH-012 (delivery path), VOL-QA-004.

### M0-T08 Actions, `UiState`, pure key map
* **Do:** `src/action.rs` with the full `Action` enum of 07 §1 (variants not yet wired are fine: `lib` pub). `src/ui_state.rs`: `UiState { tab: SelectedTab, overlay: Overlay, terminal_too_small: bool, status: Option<StatusMsg>, now: Instant }` (`StatusMsg { text, kind, expires_at }`), `Overlay { None, Help{scroll}, Popup(PopupState) }` (`PopupState` placeholder struct with only what M4 needs: `title: String, items: Vec<String>, selected: usize` — acceptable now because it is data), `Mode` derivation. `src/keymap.rs`: `map_key(&UiState, KeyEvent) -> Option<Action>` implementing **tabs, quit, Esc, F-keys** only (rows/volume keys return `None` until their milestones; the table test grows with them). Resolves **Q1 and Q2** in code.
* **Files:** `src/action.rs`, `src/ui_state.rs`, `src/keymap.rs`, `src/lib.rs`.
* **Tests first (all in `keymap::tests`):** `q_quits_in_normal_mode`, `q_closes_overlay_instead_of_quitting`, `ctrl_c_quits_in_every_mode`, `esc_does_not_quit`, `esc_closes_overlay`, `release_events_ignored`, `repeat_events_handled`, `tab_cycles_forward`, `backtab_cycles_backward`, `brackets_cycle_tabs`, `f1_to_f5_go_to_tabs`, `ctrl_q_is_not_q`, `ctrl_z_is_ignored`, `shift_modifier_ignored_for_chars`, `too_small_mode_allows_only_quit`.
* **Done when:** tests pass; no wiring yet.
* **Reqs:** VOL-KEY-001, VOL-KEY-002, VOL-KEY-003, VOL-KEY-007, VOL-KEY-009, VOL-KEY-010, VOL-ARCH-063, VOL-ARCH-065.

### M0-T09 Theme and color policy
* **Do:** `src/theme.rs`: `Theme::new(color: bool)` returning `Style` per role of 08 §8 (all roles, even ones used later); `pub fn color_enabled(no_color_flag: bool, env_no_color: Option<&str>) -> bool` (disabled iff flag, or env set and non-empty).
* **Files:** `src/theme.rs`.
* **Tests first (`theme::tests`):** `mono_theme_sets_no_fg_or_bg_on_any_role`, `color_theme_tab_selected_is_bold_yellow`, `muted_roles_use_dim_in_mono`, `no_color_unset_keeps_color`, `no_color_empty_string_keeps_color`, `no_color_nonempty_disables`, `flag_disables_even_if_env_unset`.
* **Done when:** pass; roles table in 08 §8 matches code (reviewed).
* **Reqs:** VOL-VIS-007 (partial; completed in M8).

### M0-T10 CLI
* **Do:** `src/cli.rs`: `Cli { mouse, no_color, no_meters, help, version }`, `parse(impl IntoIterator<Item=String>) -> Result<Cli, CliError>`, `USAGE: &str`. `VOL_NO_METERS=1` handled by a separate pure helper `no_meters(cli, env)`. Exit codes: help/version → 0 on stdout; error → usage on stderr, exit 2, **before** the terminal is touched. Output via `std::io::Write` (the `print_*` lints stay denied).
* **Files:** `src/cli.rs`, `src/main.rs`.
* **Tests first (`cli::tests`):** `no_args_gives_defaults`, `help_short_and_long`, `version_short_and_long`, `mouse_flag`, `no_color_flag`, `no_meters_flag_and_env`, `unknown_flag_is_error`, `flags_take_no_values`, `usage_mentions_every_flag`.
* **Done when:** pass; `cargo run -- --help` prints usage; `cargo run -- --bogus; echo $?` prints 2.
* **Reqs:** VOL-ARCH-005.

### M0-T11 App refactor: pure state, dirty-flag loop, backend events
* **Do:**
  1. `app.rs`: `App { running, ui: UiState, backend: BackendHandle-side handles, dirty: bool, options }`; remove `counter`, `events` field and `AppEvent`. `App::new(Options, ...)` has no side effects (no tokio, no terminal).
  2. `event.rs`: `Event { Tick, Crossterm, Backend(BackendEvent) }`; `EventHandler::new(backend_rx: UnboundedReceiver<BackendEvent>)` spawns the crossterm/tick task and a forwarder (`tokio::select!` over backend receiver); keeps `TICK_FPS = 30.0`. Signals: tokio `signal::unix` SIGTERM/SIGHUP/SIGINT produce `Event::Quit`.
  3. `App::update(&mut self, Action)` and `App::on_event(&mut self, Event)`; `App::needs_draw()`; draw loop: drain events, draw **only on Tick** and only if `dirty` (first frame forced). `Event::Crossterm(Resize)` marks dirty and recomputes `terminal_too_small` (< 40×12). Backend events mark dirty (store comes in M1).
  4. `run(&mut self, terminal, events)` takes the `EventHandler` as argument.
* **Files:** `src/app.rs`, `src/event.rs`, `src/main.rs`.
* **Tests first:** `app::tests::q_stops_running`, `…::esc_keeps_running`, `…::tab_switch_marks_dirty_and_changes_tab`, `…::tick_without_change_does_not_need_draw`, `…::first_frame_needs_draw`, `…::resize_marks_dirty`, `…::resize_below_40x12_sets_too_small`, `…::resize_to_40x12_clears_too_small`, `…::backend_event_marks_dirty`, `…::default_tab_is_playback`, `…::f_keys_switch_tabs`; `#[tokio::test] event::tests::backend_events_are_forwarded_as_event_backend` (feature mock), `…::dropping_receiver_stops_tasks`.
* **Done when:** pass; manual: idle `vol` redraws nothing (observe with `strace -c -e write -p` or add a debug counter in a scratch run — record result).
* **Reqs:** VOL-ARCH-033, VOL-ARCH-064, VOL-ARCH-066, VOL-ARCH-012, VOL-ARCH-004, VOL-PB-011, VOL-PB-012, VOL-KEY-002.

### M0-T12 Rendering: structure, tab bar, too-small screen, stub tabs
* **Do:** `src/ui.rs`: `render(&App, Rect, &mut Buffer)` per 08 §1: row 0 tab bar, row 1 filter bar (empty placeholder text `Show: —`), body, last row status line (placeholder `○ connecting…` left, `? help` right). Pure helper `tab_bar_layout(titles: &[String], selected: usize, width: u16) -> TabBarLayout` implementing 08 §2 (divider `│`, `◂ `/` ▸`, `…` truncation). Too-small screen per VIS-001/002. Tabs: `src/tabs/*.rs` become `pub fn render(app:&App, area: Rect, buf:&mut Buffer)` drawing the tab name centered and `(not implemented yet)` dim; delete the `String` stubs. `src/theme.rs` used for all styles. No widget reads the clock.
* **Files:** `src/ui.rs`, `src/tabs/*.rs`, `tests/render.rs`, `tests/snapshots/`.
* **Tests first:** `ui::tests::tab_bar_fits_at_80`, `…::tab_bar_overflow_at_40_selected_first` (expects `" Playback │ Recording │ Output… ▸"`), `…::tab_bar_scrolls_so_selected_fits`, `…::tab_bar_never_exceeds_width` **(prop)** (width 1..=120 × selected 0..5), `…::selected_tab_titles_are_stable` (strum strings, VOL-ARCH-004); `tests/render.rs::render_80x24_snapshot` **(snap)**, `::render_60x24_snapshot` **(snap)**, `::render_40x12_is_not_too_small`, `::render_39x12_shows_too_small_message` **(snap)**, `::render_40x11_shows_too_small_message`, `::render_zero_and_one_cell_areas_do_not_panic`, `::no_cell_written_outside_area` (render into a sub-rect of a larger buffer; border untouched), `::selected_tab_is_bold_yellow` (style of the selected title cell), `::mono_theme_has_no_colors_anywhere`.
* **Done when:** pass; `cargo run` shows five working tabs.
* **Reqs:** VOL-VIS-001, VOL-VIS-002, VOL-VIS-003, VOL-VIS-004, VOL-VIS-009, VOL-VIS-010, VOL-ARCH-003.

### M0-T13 Terminal lifecycle in `main`
* **Do:** order per VOL-ARCH-060: parse CLI → `color_eyre::install()` → backend handle (`BackendHandle::inert()`) → `ratatui::init()` → build `App` and `EventHandler` → `run` → `ratatui::restore()` on every exit path (including `Err`). `--mouse` is accepted and stored (capture itself arrives in M8). `--no-color`/`NO_COLOR` select the theme.
* **Files:** `src/main.rs`.
* **Tests first:** none automatable beyond T10/T11; manual: (a) `vol`, press `q`: shell prompt/cursor normal (`stty -a` before/after equal); (b) `vol & sleep 1; kill -TERM %1`: terminal restored; (c) force a panic in a scratch build (`panic!` on key `p`), terminal restored, backtrace printed by color-eyre; record results in the commit body.
* **Done when:** manual checks recorded.
* **Reqs:** VOL-ARCH-060, VOL-ARCH-062.

### M0-T14 Spec/plan housekeeping and DoD
* **Do:** re-check that `docs/specs/01-architecture.md` §2 (already lists `lib.rs`, `cli.rs`, `log.rs`, `ui_state.rs`, `row_list.rs`, and `mock` as Cargo feature `mock`) still matches what was built and fix it if not; set statuses for every requirement completed above (GEN rows stay `n/a`); recount `SPEC.md` §1 table; mark M0 `Done` in `ROADMAP.md` and tick this file; run the orphan/duplicate check from `docs/plans/coverage.md`.
* **Done when:** all M0 IDs are `Implemented` (or `n/a` for GEN); `just check` green; branch merged ff-only to `main`.
* **Reqs:** VOL-GEN-001…008 (adoption: conventions are applied in every commit from here).

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-GEN-001 | M0-T14 |
| VOL-GEN-002 | M0-T14 |
| VOL-GEN-003 | M0-T14 |
| VOL-GEN-004 | M0-T14 |
| VOL-GEN-005 | M0-T14 |
| VOL-GEN-006 | M0-T14 |
| VOL-GEN-007 | M0-T14 |
| VOL-GEN-008 | M0-T14 |
| VOL-ARCH-001 | M0-T03, M0-T06 |
| VOL-ARCH-002 | M0-T04 |
| VOL-ARCH-003 | M0-T12 |
| VOL-ARCH-004 | M0-T11 |
| VOL-ARCH-005 | M0-T10 |
| VOL-ARCH-012 | M0-T07, M0-T11 |
| VOL-ARCH-033 | M0-T11 |
| VOL-ARCH-050 | M0-T06 |
| VOL-ARCH-060 | M0-T13 |
| VOL-ARCH-062 | M0-T13 |
| VOL-ARCH-063 | M0-T08 |
| VOL-ARCH-064 | M0-T11 |
| VOL-ARCH-065 | M0-T08 |
| VOL-ARCH-066 | M0-T11 |
| VOL-VIS-001 | M0-T12 |
| VOL-VIS-002 | M0-T12 |
| VOL-VIS-003 | M0-T12 |
| VOL-VIS-004 | M0-T12 |
| VOL-VIS-009 | M0-T12 |
| VOL-VIS-010 | M0-T12 |
| VOL-KEY-001 | M0-T08 |
| VOL-KEY-002 | M0-T08, M0-T11 |
| VOL-KEY-003 | M0-T08 |
| VOL-KEY-007 | M0-T08 |
| VOL-KEY-009 | M0-T08 |
| VOL-KEY-010 | M0-T08 |
| VOL-PB-011 | M0-T11 |
| VOL-PB-012 | M0-T11 |
| VOL-QA-001 | M0-T01 |
| VOL-QA-002 | M0-T02 |
| VOL-QA-003 | M0-T02 |
| VOL-QA-004 | M0-T02, M0-T07 |
| VOL-QA-005 | M0-T01 |
| VOL-QA-006 | M0-T01 |
| VOL-QA-007 | M0-T06 |
| VOL-QA-009 | M0-T01 |
| VOL-QA-010 | M0-T01 |
| VOL-PLAT-001 | M0-T05 |
| VOL-PLAT-002 | M0-T05 |
| VOL-PKG-002 | M0-T05 |

Touched here but owned by another milestone (partial work or verification only):

| ID | Owner | Task(s) |
|---|---|---|
| VOL-VIS-007 | M8 | M0-T09 |

## Definition of done (M0)

- [ ] `just check` green on `main`; pre-push hook installed and exercised once.
- [ ] Every ID in the M0 list has status `Implemented` (VOL-GEN-* `n/a`).
- [ ] `src/legacy.rs`, `counter`, `AppEvent::{Increment,Decrement}`, `String`-returning tab stubs are gone.
- [ ] `vol --help`, `--version`, unknown-flag exit code 2 verified by hand; `q`, `Ctrl-C`, `kill -TERM` restore the terminal.
- [ ] Snapshots for 80×24, 40×12 (not too small), 39×12 and 40×11 (too small) committed.
- [ ] `docs/specs/01-architecture.md` §2 updated; ROADMAP status updated.

## Risks

| Risk | Mitigation |
|---|---|
| `print_stdout` deny conflicts with `--help` | write via `std::io::Write` handles (no `println!`); no `#[allow]`. |
| `insta`/`proptest` newest versions exceed MSRV 1.88 | pin an older compatible version; `just msrv` catches it in T02. |
| lib+bin same name warns in `cargo doc` | `[[bin]] doc = false` (T03). |
| `Event: Clone` requires `BackendEvent: Clone` | all message types derive `Clone`; `BackendError::Pipewire(i32, String)` is clonable. |
| Dependabot crossterm 0.29 branch conflicts with ratatui 0.29 | ignore until ratatui is bumped together; use `ratatui::crossterm` paths. |
| Inert backend in `main` hides "connecting" forever | intentional for M0; replaced in M1-T15. |
