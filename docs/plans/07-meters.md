# M7 — Peak meters

**Status:** Not started · **Size:** M · **Depends on:** M5 · **Branch:** `m7/meters` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m7--peak-meters)

**Goal.** Capture-stream peak meters for Output/Input/Playback/Recording rows with desired-set diffing, windowing, decay and rendering; `--no-meters`.

## Requirement IDs assigned to this milestone (18)

`VOL-MTR-001`, `VOL-MTR-002`, `VOL-MTR-003`, `VOL-MTR-004`, `VOL-MTR-005`, `VOL-MTR-007`, `VOL-MTR-008`, `VOL-ARCH-034`, `VOL-ARCH-045`, `VOL-VIS-006`, `VOL-PB-009`, `VOL-OUT-007`, `VOL-IN-006`, `VOL-REC-004`, `VOL-TEST-033`, `VOL-TEST-045`, `VOL-TEST-050`, `VOL-TEST-043`

Capture-stream peak meters on every row tab. Pure math and widgets first, then the diffing logic, then the backend streams, then integration and live tests. Q5 and Q6 are decided by an early spike.

## Pre-flight

`git switch -c m7/meters`. Needs a running audio source for checks: `pw-cat --playback` of a −6 dBFS sine (or any player) and a microphone/null source.

## Tasks

### M7-T01 Spike: meter capture semantics (VF-06, VF-07, Q5, Q6)
* **Do:** scratch crate or `pw-cat --record` equivalents (`pw-record --target <serial> -P '{ node.passive=true stream.capture.sink=true }' /dev/null`): (a) capture a sink monitor, (b) capture a source, (c) capture a `Stream/Output/Audio` node via `target.object=<stream serial>`, (d) with `node.passive=true` against a **suspended** sink/source: does it stay suspended (`wpctl status` / `pw-cli ls Node`), (e) latency hint effect. Record in this plan, SPEC §7 (VF-06/07 closed) and §8 (Q5/Q6 decided), and edit 02 §2.4 table/properties accordingly.
* **Reqs:** supports VOL-MTR-004/005, VOL-PB-009.

### M7-T02 Meter math and `PeakMeter` widget (pure)
* **Do:** `model/meter.rs`: `peak_to_level(peak: f32) -> f32` (VOL-MTR-002 mapping), `MeterState{display: f32, last_sample: Option<Instant>}` with `update(peak, now)`, `decay(dt)` (1/3 full-scale per second), `stale(now)` (2 s, MTR-007); `widgets/meter.rs::PeakMeter` (8 cells, 64 steps, glyph table, `·` empty, colors per 08 §5).
* **Tests first:** `model::meter::tests::peak_one_is_full_scale`, `…::peak_0_001_is_zero`, `…::peak_zero_is_zero`, `…::decay_20_db_per_second_over_simulated_ticks`, `…::new_peak_overrides_decay_when_higher`, `…::stale_after_2s`, **(prop)** `…::display_always_in_0_1`; `widgets::meter::tests::glyph_for_each_step_0_to_64` (64 cases), `…::cells_0_to_5_green_6_yellow_7_red`, `…::mono_theme_uses_no_color`, `…::empty_cells_are_middle_dot`.
* **Reqs:** VOL-MTR-002, VOL-MTR-003, VOL-MTR-007, VOL-VIS-006, VOL-TEST-033.

### M7-T03 Desired meter set and `SetMeters` diffing (app side)
* **Do:** `view::meter_targets(&App) -> Vec<MeterTarget>`: from the rows **drawn** on the active tab: Output → `SinkMonitor`, Input → `Source`, Playback → `StreamOutput` (or none if Q5 says so), Recording → linked source/`SinkMonitor` via links/ports (03 §2.1); dedupe by serial; cap 16; `UiState.meters_sent: Vec<MeterTarget>`; send `Command::SetMeters{targets}` only when the set changes (tab switch, scroll, filter, resize, node add/remove); stop all when too-small (VIS-001) or `--no-meters`; **re-send after `Connected`** (ARCH-045). `MeterSample`s update `MeterState` per `Serial`; ticks apply decay and mark dirty while any display level > 0.
* **Tests first (`app::tests` / `view::tests`):** `meter_targets_for_output_rows_are_sink_monitors`, `…_input_rows_are_sources`, `…_recording_row_uses_linked_source`, `…_recording_row_on_monitor_port_uses_sink_monitor`, `…_unlinked_recording_row_has_no_target`, `…_dedupe_same_target`, `…_capped_at_16`, `…_only_drawn_rows_count_after_scroll`, `set_meters_sent_only_when_set_changes`, `set_meters_empty_on_tab_without_meters_and_when_too_small`, `set_meters_resent_after_connected`, `meter_samples_update_display_and_mark_dirty`, `idle_meters_at_zero_do_not_mark_dirty`.
* **Reqs:** VOL-MTR-004, VOL-ARCH-045 (app half), VOL-ARCH-034 (app half).

### M7-T04 Backend meter streams
* **Do:** `backend/pw/meter.rs`: for each target create a `Stream` with the properties of 02 §2.4 (names `vol.meter.<serial>`, `vol.meter=true`, passive/dont-reconnect/monitor/capture.sink per T01 results), format negotiation `F32` → read channels/layout in `param_changed`; `process` computes `max |x|` across channels into a per-meter window max; a 40 ms loop timer emits **one** `BackendEvent::Meters(Vec<MeterSample>)` (never through `Batch`), skipping all-zero repeats (MTR-001); `SetMeters` diffs: start new, destroy removed ≤ 200 ms (MTR-005); target node removal destroys silently; destroy all on disconnect and shutdown (ARCH-045/013).
* **Tests first:** pure: `backend::pw::meter::tests::peak_over_planar_and_interleaved_buffers`, `…::window_resets_after_emit`, `…::zero_windows_stop_emitting_after_two`, `…::diff_start_stop_sets`, `…::stream_props_for_sink_monitor_have_capture_sink_true`, `…::stream_props_for_source_do_not`; live: `live::meter_reports_peak_of_sine_within_5_percent` (null sink + `pw-cat --playback` −6 dBFS sine ⇒ peak ≈ 0.5 ± 0.05), `live::removing_target_destroys_meter`, `live::shutdown_destroys_all_meter_nodes` (`pw-cli ls Node` has no `vol.meter.*`).
* **Reqs:** VOL-MTR-001, VOL-MTR-005, VOL-ARCH-034, VOL-ARCH-045.

### M7-T05 Integrate the meter column
* **Do:** meter column in all row tabs (Full/Mid tiers per 02 §2.3): Output/Input/Playback/Recording rows read `MeterState` by target; hidden (empty cells) for suspended sinks (VOL-OUT-007) and when no data; `--no-meters`/`VOL_NO_METERS=1` wired to suppress `SetMeters` (MTR-008).
* **Tests first:** `tests/render.rs::output_meter_cells_follow_state` (inject levels via `UiState`; assert glyph cells), `::playback_meter_column_absent_in_narrow_tier`, `::suspended_sink_meter_is_empty`, `tests/scenarios.rs::no_meters_flag_sends_no_set_meters`.
* **Reqs:** VOL-PB-009, VOL-OUT-007, VOL-IN-006, VOL-REC-004, VOL-MTR-008.

### M7-T06 Scenarios and live checklist
* **Do:** `tests/scenarios.rs::disconnected_then_connected_populates_rows_and_resends_set_meters` (VOL-TEST-043), `::tab_switch_changes_desired_meter_set` and `::leaving_tab_sends_set_meters_without_its_targets` (VOL-TEST-045); complete the live suite (VOL-TEST-050 a–d: connect, set-volume echo, kill/restart daemon, meter peak) — a–c already exist from M1/M3, here verify they are in `live.rs` and d is added.
* **Reqs:** VOL-TEST-043, VOL-TEST-045, VOL-TEST-050.

### M7-T07 Manual acceptance and close-out
* **Do:** MA-10 (≈ 25 Hz response, fall-off ~20 dB/s), MA-13 (no `vol.meter.*` left after quit and after `kill -TERM`); check the "microphone in use" indicator appears only on the Input/Recording tabs with visible source rows, and disappears on tab switch; update statuses/recount/ROADMAP; merge.

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-MTR-001 | M7-T04 |
| VOL-MTR-002 | M7-T02 |
| VOL-MTR-003 | M7-T02 |
| VOL-MTR-004 | M7-T01, M7-T03 |
| VOL-MTR-005 | M7-T01, M7-T04 |
| VOL-MTR-007 | M7-T02 |
| VOL-MTR-008 | M7-T05 |
| VOL-ARCH-034 | M7-T03, M7-T04 |
| VOL-ARCH-045 | M7-T03, M7-T04 |
| VOL-VIS-006 | M7-T02 |
| VOL-PB-009 | M7-T01, M7-T05 |
| VOL-OUT-007 | M7-T05 |
| VOL-IN-006 | M7-T05 |
| VOL-REC-004 | M7-T05 |
| VOL-TEST-033 | M7-T02 |
| VOL-TEST-045 | M7-T06 |
| VOL-TEST-050 | M7-T06 |
| VOL-TEST-043 | M7-T06 |

## Definition of done (M7)

- [ ] `just check` and `just test-live` green.
- [ ] All 18 M7 IDs `Implemented`; Q5/Q6 decided; VF-06/07 closed.
- [ ] MA-10, MA-13 recorded; CPU use of an idle app with meters on < 3 % of one core (note value in the PR body).

## Risks

| Risk | Mitigation |
|---|---|
| Meter streams appear in other mixers and confuse users | stable names/`vol.meter` property; documented in README (M9). |
| Capture of a stream node refused by WirePlumber | Q5 default: empty meters for Playback rows. |
| Passive meters stay empty on idle sources | accepted (Q6); `README` explains; active mode would be a new flag (backlog). |
| Process callback cost / allocations | no allocation in `process`; atomics or `Cell` for window max; emission on loop timer only. |
