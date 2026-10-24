# M8 — Polish: help, mouse, NO_COLOR, snapshots

**Status:** Not started · **Size:** M · **Depends on:** M7 · **Branch:** `m8/polish` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m8--polish-help-mouse-nocolor-snapshots)

**Goal.** Help overlay, optional mouse, full theme/`NO_COLOR` coverage, complete keymap and snapshot suites, terminal matrix.

## Requirement IDs assigned to this milestone (10)

`VOL-ARCH-061`, `VOL-KEY-006`, `VOL-KEY-008`, `VOL-KEY-012`, `VOL-KEY-013`, `VOL-VIS-007`, `VOL-TEST-020`, `VOL-TEST-030`, `VOL-TEST-031`, `VOL-PLAT-005`

Everything that makes it feel finished: help overlay, mouse, full theme and `NO_COLOR` coverage, complete keymap and snapshot suites, terminal matrix.

## Pre-flight

`git switch -c m8/polish`. Test terminals available locally: gnome-terminal/konsole, kitty or alacritty, tmux, Linux VT (8 colors), `TERM=xterm` and `TERM=xterm-256color`.

## Tasks

### M8-T01 Glyph audit and Q9
* **Do:** run every tab in the available terminals/fonts; list glyphs that misrender (08 §10); decide Q9 (proposed: no ASCII fallback) and record the decision (SPEC §8, 08 §10). If a fallback is chosen it becomes a new requirement + task here.
* **Reqs:** supports VOL-PLAT-005.

### M8-T02 Help overlay
* **Do:** `widgets/help.rs` (content exactly as 07 §4, 11 lines; width `min(56, W−4)`, height `min(lines+2, H−4)`, scroll with `j/k`); `?`/`Esc`/`q`/`Enter` close; keymap help mode.
* **Tests first:** `widgets::help::tests::content_matches_spec_lines`, `…::geometry_80x24_and_40x12`, `…::scrolls_when_clipped`, `keymap::tests::help_mode_table`, `tests/render.rs::help_overlay_80x24` **(snap)** (07 §4 mockup).
* **Reqs:** VOL-KEY-008 (help half; popup half done in M4), VOL-KEY-006 (`?` clause).

### M8-T03 Keymap completeness
* **Do:** `keymap::tests::full_table` iterating **every** row of 07 §2–§4 (mode × key → `Action`) — written as data in the test file so spec edits show as diffs; fix gaps.
* **Reqs:** VOL-TEST-020, VOL-KEY-006.

### M8-T04 Mouse (optional, `--mouse`)
* **Do:** `EnableMouseCapture`/`DisableMouseCapture` paired with init/restore **including the panic path** (chain a hook before ratatui's); hit map `UiState.hit` rebuilt each draw by widgets; actions per 07 §5; bar click → percent formula `round(150·(x − bar_x0)/(bar_w − 1))` clamped.
* **Tests first:** `ui::tests::click_on_bar_maps_to_percent` (table: left edge 0, tick column ≈ 100, right edge 150), `…::click_on_mute_cell_toggles_mute`, `…::click_on_tab_title_switches_tab`, `…::click_outside_popup_closes`, `…::wheel_on_bar_is_plus_minus_5`, `…::mouse_events_ignored_without_flag`.
* **Reqs:** VOL-KEY-012, VOL-ARCH-061.

### M8-T05 `Alt-1`…`Alt-5`
* **Tests first:** `keymap::tests::alt_digits_go_to_tabs`, `…::plain_digits_remain_volume`.
* **Reqs:** VOL-KEY-013.

### M8-T06 Theme completion and `NO_COLOR`
* **Do:** audit that every widget takes styles only from `Theme`; mono test renders all tabs, popup, help, banner and asserts `fg/bg == Reset` for every cell and `! ` prefix on errors; style assertions for color mode.
* **Tests first:** `tests/render.rs::mono_mode_all_tabs_have_no_colors`, `::error_message_has_bang_prefix_in_mono`, `::selected_row_has_cyan_bar` , `::over_100_cells_are_red`, `::muted_row_has_dim_modifier`, `::meter_colors_by_cell`.
* **Reqs:** VOL-VIS-007, VOL-TEST-031.

### M8-T07 Snapshot suite and docs sync
* **Do:** complete VOL-TEST-030 matrix (sizes 80×24, 60×10, 40×16, 40×12, 39×12, 40×11 × all tabs/states listed in the spec); regenerate or hand-fix the mockups in `docs/specs/*.md` so they match snapshots character for character (VOL-GEN-007); terminal matrix run (VOL-PLAT-005): gnome-terminal, kitty/alacritty, tmux, Linux VT.
* **Reqs:** VOL-TEST-030, VOL-PLAT-005.

### M8-T08 Manual acceptance and close-out
* **Do:** MA-12 (`NO_COLOR=1`, 40×12, 39×12); update statuses/recount/ROADMAP; merge.

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-ARCH-061 | M8-T04 |
| VOL-KEY-006 | M8-T02, M8-T03 |
| VOL-KEY-008 | M8-T02 |
| VOL-KEY-012 | M8-T04 |
| VOL-KEY-013 | M8-T05 |
| VOL-VIS-007 | M8-T06 |
| VOL-TEST-020 | M8-T03 |
| VOL-TEST-030 | M8-T07 |
| VOL-TEST-031 | M8-T06 |
| VOL-PLAT-005 | M8-T01, M8-T07 |

## Definition of done (M8)

- [ ] `just check` green.
- [ ] All 10 M8 IDs `Implemented`.
- [ ] MA-12 recorded; Q9 decided.
- [ ] Mockups in `docs/specs/` equal the snapshots.

## Risks

| Risk | Mitigation |
|---|---|
| Mouse capture leaves terminal in capture mode after panic | dedicated panic-hook test (manual) + paired restore; flag is opt-in. |
| Snapshot maintenance burden | one fixture (`default_system`) and `insta` review workflow (`cargo insta review`); mockups regenerated from it. |
| Terminals reporting modifiers differently (Shift-arrows) | every Shift variant has a letter/char alternative (07 §2.3). |
