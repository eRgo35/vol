# M2 — Playback tab (read-only)

**Status:** In progress · **Size:** L · **Depends on:** M1 · **Branch:** `m2/playback-readonly` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m2--playback-tab-read-only)

**Goal.** First real screen: live Playback rows with title/target/state, volume bar, percent, dB, mute indicator, channel summary, filter, selection, scrolling, status line, disconnected/empty states. No writes.

## Requirement IDs assigned to this milestone (25)

`VOL-ARCH-024`, `VOL-ARCH-044`, `VOL-VM-001`, `VOL-VM-002`, `VOL-VM-003`, `VOL-VM-005`, `VOL-VM-006`, `VOL-ROW-002`, `VOL-ROW-003`, `VOL-ROW-004`, `VOL-ROW-006`, `VOL-ROW-007`, `VOL-FLT-001`, `VOL-PB-001`, `VOL-PB-002`, `VOL-PB-003`, `VOL-PB-004`, `VOL-PB-008`, `VOL-MTR-006`, `VOL-KEY-004`, `VOL-KEY-011`, `VOL-VIS-005`, `VOL-VIS-008`, `VOL-PLAT-004`, `VOL-TEST-032`

First user-visible milestone. Pure layers first (volume math → views → layout), then widgets, then the tab. The data comes from the M1 store; nothing is written to PipeWire.

## Pre-flight

`git switch -c m2/playback-readonly`. Add `unicode-width` (already a transitive dependency of ratatui; declare it explicitly with a compatible version) for VOL-ROW-007.

## Tasks

### M2-T01 Volume model (pure)
* **Do:** `model/volume.rs` per 02 §2.1: `LEVEL_MAX`, `PERCENT_MAX`, `gain_to_level`, `level_to_gain`, `level_to_percent`, `percent_to_level`, `gain_to_db`, `fmt_db`, plus `NodeAudio::effective_gains()`, `master_level()` (max, Q11), `master_percent()`, `channel_percents()`. Stepping/locking/writes are **M3**.
* **Files:** `src/model/volume.rs`, `src/model/node.rs`.
* **Tests first (`model::volume::tests`):** `anchors_100_percent_is_gain_one`, `anchors_150_percent_is_3_375`, `fmt_db_anchors` (`" 0.0 dB"`, `"+10.6 dB"`, `"-inf dB"`, `"-18.1 dB"` for 50 %, `"-3.6 dB"` for 87 %), `observed_pw_dump_value_shows_87_percent` (`0.658492` → 87, SPEC 02 §2.1 evidence), `effective_gain_multiplies_master_volume`, `empty_channel_volumes_has_no_master`, `over_150_reports_true_percent`, **(prop)** `roundtrip_percent_0_to_150_is_identity` (via `f32`), **(prop)** `db_is_monotonic_in_percent`.
* **Done when:** pass; Q11 decided (max).
* **Reqs:** VOL-VM-001, 002, 003, 005, 006.

### M2-T02 Row view models (pure)
* **Do:** `model/view.rs`: `RowKey`, `StreamFilter`, `StreamRow` (02 §3.1), `is_virtual_stream`, `playback_rows(&Store, StreamFilter, &UiPending) -> Vec<StreamRow>`: filter, exclude `vol.meter=true` or `node.name` starting `vol.meter.`, sort by serial, `title`/`media` chains (02 §3.2), `running`, target label from links (02 §3.3 steps 2–3; step 1 pending-move arrives in M4), channel names. `UiPending` is a placeholder empty struct now (M3 fills it).
* **Files:** `src/model/view.rs`.
* **Tests first (`model::view::tests`):** `title_prefers_application_name`, `title_falls_back_to_description_binary_name`, `media_omitted_when_equal_to_title`, `virtual_by_media_role_filter`, `virtual_by_node_virtual_true`, `virtual_by_link_group`, `application_filter_excludes_virtual`, `virtual_filter_only_virtual`, `all_filter_includes_both`, `own_meter_streams_hidden_by_prop`, `own_meter_streams_hidden_by_name_prefix`, `rows_sorted_by_serial`, `target_label_from_first_link_to_sink`, `target_label_none_when_unlinked`, `running_marker_from_node_state`, `audio_none_gives_row_without_volume`.
* **Done when:** pass.
* **Reqs:** VOL-PB-001, 002, 003, 008, VOL-MTR-006, VOL-ROW-006.

### M2-T03 Mock fixtures
* **Do:** `backend/mock.rs` (or `backend/fixtures.rs` under the same cfg): `fixtures::default_system()` per SPEC 09 §2: 2 sinks (one default, with `device.id`/`card.profile.device`), 2 sources, 1 card with profiles/routes, 4 playback streams (Firefox, Spotify and mpv — the three rows of the 02 §2.3 mockup — plus a virtual loopback), 1 capture stream, links stream→sink, default metadata; values copied from the observed `pw-dump` shapes.
* **Tests first:** `backend::fixtures::tests::default_system_applies_cleanly` (every event changes the store; counts: 4 device nodes + 5 streams …), `…::default_system_has_exactly_one_default_sink`.
* **Reqs:** supports VOL-TEST-006/030.

### M2-T04 `VolumeBar` widget
* **Do:** `widgets/volume_bar.rs` per 08 §4: `VolumeBar { percent: Option<u32>, muted: bool, theme: &Theme }` implementing `Widget`; geometry fns `bar_fill(g,p)`, `bar_tick(g)` public for tests. `None` renders the `--` variant.
* **Tests first:** `widgets::volume_bar::tests::fill_and_tick_table` for G ∈ {3,29,40,47,51} × p ∈ {0,1,50,99,100,101,150} (expected values computed from the formulas in the test), `…::tick_column_is_two_thirds`, `…::over_100_uses_double_line_glyph_right_of_tick`, `…::muted_uses_muted_style`, `…::unavailable_shows_dashes`, `…::width_below_3_has_no_tick_and_no_panic`, `…::zero_width_noop`.
* **Reqs:** VOL-VIS-005, VOL-TEST-032.

### M2-T05 Row layout engine
* **Do:** `widgets/volume_row.rs`: pure `RowGeometry::new(width) -> {tier, bar_w, label_max}`; `render_locked_row(model, geometry, selected, theme, area, buf)` for 3-line locked rows (02 §2.3). Title truncation with `unicode-width` and `…` (VOL-ROW-007). Right label rule `min(30, 3W/8)`. Unlocked layout is **M3**.
* **Tests first:** `widgets::volume_row::tests::geometry_80_60_40` (bar_w 51/40/29, tiers), `…::tier_boundaries_71_72_55_56`, `…::title_truncates_with_ellipsis_and_keeps_8_cells`, `…::wide_glyph_title_not_split` (CJK), `…::label_dropped_when_title_would_drop_below_8`, `…::line2_columns_match_spec_at_80` (assert exact strings of line 2 for the 72 % and 120 % fixtures from the mockup), `…::muted_row_shows_m_marker`, `…::channel_summary_line_for_stereo_and_blank_for_mono`.
* **Reqs:** VOL-ROW-001 (locked part; completed M3), VOL-ROW-007.

### M2-T06 Selection, scrolling, count (list widget)
* **Do:** `widgets/row_list.rs`: renders the visible window; `UiState.list: HashMap<SelectedTab, ListState{selected: Option<RowKey>, last_index, offset}>`; `ListState::reselect(&[RowKey])` implements VOL-ROW-004; scrolling keeps the selected row fully visible (VOL-ROW-002); returns `ListInfo{shown, total, selected_pos}` for the filter bar count (VOL-ROW-003). Actions `RowNext/Prev/First/Last` wired in `keymap` (VOL-KEY-004).
* **Tests first:** `widgets::row_list::tests::selection_survives_reorder`, `…::selection_moves_to_clamped_index_when_key_vanishes`, `…::empty_list_has_no_selection`, `…::scroll_keeps_selected_fully_visible_for_3_line_rows`, `…::partial_rows_not_drawn`, `…::count_text_all_fit_vs_position`; `keymap::tests::j_k_g_G_arrows_row_navigation`.
* **Reqs:** VOL-ROW-002, 003, 004, VOL-KEY-004.

### M2-T07 Filter bar and `f`/`F`
* **Do:** `Filter` plumbing in `UiState` (per tab, session-only); filter bar rendering `Show: <name> (f)` + count; keymap `f`/`F` → `FilterNext/Prev`; Playback names/cycle order (`Applications → All Streams → Virtual Streams`).
* **Tests first:** `ui::tests::filter_bar_text_playback_default`, `…::filter_bar_right_count_pluralization` (`1 stream`, `3 streams`), `keymap::tests::f_and_shift_f_cycle_filter`, `app::tests::filter_cycle_wraps_and_marks_dirty`.
* **Reqs:** VOL-FLT-001, VOL-PB-002.

### M2-T08 Status line
* **Do:** `widgets/statusline.rs` per 07 §6: connection part, transient message (TTL logic with injected `now`), hint, default label, `? help`, width handling; full message catalogue as `const` strings/`fn`s (`Msg::…`) even though most triggers arrive later; `UiState::set_status`.
* **Tests first:** `widgets::statusline::tests::connected_text_with_version`, `…::disconnected_text`, `…::message_overrides_hint`, `…::message_expires_after_4s`, `…::default_label_dropped_below_60_cols`, `…::help_hint_always_visible_at_40`, `…::catalogue_strings_match_spec` (table of the exact strings in 07 §6).
* **Reqs:** VOL-KEY-011.

### M2-T09 Banners and empty states
* **Do:** `ui.rs`: disconnected banner variants with countdown (from `ConnState::Disconnected{retry_at,…}` and injected `now`), connecting text, empty-state texts for Playback (VOL-PB-004 both variants); rows hidden when disconnected; popups closed on disconnect (no popups yet—hook in place); `--` bars for nodes with `audio == None` (VOL-ARCH-024).
* **Tests first:** `tests/render.rs::disconnected_no_daemon_banner_80x24` **(snap)**, `::disconnected_connection_lost_text`, `::connecting_text`, `::empty_playback_hint_text`, `::empty_with_hidden_streams_text` (`No streams match this filter (2 hidden)`), `app::tests::disconnected_event_clears_rows_and_keeps_tab_and_filter`.
* **Reqs:** VOL-VIS-008, VOL-ARCH-044, VOL-ARCH-024, VOL-PLAT-004.

### M2-T10 Playback tab composition
* **Do:** `tabs/playback.rs`: rows via `playback_rows` → `row_list`; apply theme; initial tab remains Playback; live add/remove scenario.
* **Tests first:** `tests/render.rs::playback_80x24_locked_rows` **(snap)** (fixture `default_system`, first row selected; compare to the mockup in 02 §2.3 — update the mockup if the snapshot intentionally differs), `::playback_60x10_mid_tier` **(snap)**, `::playback_40x16_narrow_tier` **(snap, locked rows only; the unlocked first row of the mockup arrives in M3)**, `::over_100_percent_row_style` (cells right of tick are red), `::muted_row_is_dim`, `tests/scenarios.rs::stream_added_and_removed_keeps_selection_by_serial`, `::virtual_stream_hidden_in_applications_filter`.
* **Reqs:** VOL-PB-001, 003, 004, 008.

### M2-T11 Manual acceptance and close-out
* **Do:** MA-01 (banner without daemon, rows after daemon starts), MA-02 read side (percent == `round(100 × wpctl get-volume)`, dB equals `pactl get-sink-volume` ± 0.05 on the default sink — Playback shows streams, so use a `pw-cat --playback` stream whose volume is read via `wpctl`/`pw-dump`), MA-11 (kill `pipewire`: banner ≤ 2 s, `q` exits cleanly). Update statuses, recount, ROADMAP, coverage check, merge ff-only.
* **Done when:** DoD below.

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-ARCH-024 | M2-T09 |
| VOL-ARCH-044 | M2-T09 |
| VOL-VM-001 | M2-T01 |
| VOL-VM-002 | M2-T01 |
| VOL-VM-003 | M2-T01 |
| VOL-VM-005 | M2-T01 |
| VOL-VM-006 | M2-T01 |
| VOL-ROW-002 | M2-T06 |
| VOL-ROW-003 | M2-T06 |
| VOL-ROW-004 | M2-T06 |
| VOL-ROW-006 | M2-T02 |
| VOL-ROW-007 | M2-T05 |
| VOL-FLT-001 | M2-T07 |
| VOL-PB-001 | M2-T02, M2-T10 |
| VOL-PB-002 | M2-T02, M2-T07 |
| VOL-PB-003 | M2-T02, M2-T10 |
| VOL-PB-004 | M2-T10 |
| VOL-PB-008 | M2-T02, M2-T10 |
| VOL-MTR-006 | M2-T02 |
| VOL-KEY-004 | M2-T06 |
| VOL-KEY-011 | M2-T08 |
| VOL-VIS-005 | M2-T04 |
| VOL-VIS-008 | M2-T09 |
| VOL-PLAT-004 | M2-T09 |
| VOL-TEST-032 | M2-T04 |

Touched here but owned by another milestone (partial work or verification only):

| ID | Owner | Task(s) |
|---|---|---|
| VOL-TEST-006 | M6 | M2-T03 |
| VOL-TEST-030 | M8 | M2-T03 |
| VOL-ROW-001 | M3 | M2-T05 |

## Definition of done (M2)

- [ ] `just check` green; M2 snapshots committed.
- [ ] All 25 M2 IDs `Implemented`.
- [ ] MA-01, MA-02 (read), MA-11 recorded in the commit/PR body.
- [ ] Q11 decided (max); mockups in `docs/specs/02-playback.md` regenerated if snapshots changed.

## Risks

| Risk | Mitigation |
|---|---|
| Streams with `audio == None` for a while after creation | `--` rendering + test; never hide the row. |
| Width of `●`/`○`/`▌` in some terminals/fonts | all single-cell per `unicode-width`; documented (08 §10); revisit in M8 (Q9). |
| Snapshot churn when tuning layout | change geometry only through `RowGeometry`; update mockups in the same commit (VOL-GEN-007). |
| Many streams (50+) make `playback_rows` recompute per frame | compute once per dirty draw; micro-optimise only if a profile shows > 1 ms. |
