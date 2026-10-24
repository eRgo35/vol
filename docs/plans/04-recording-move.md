# M4 — Recording tab and stream move

**Status:** Not started · **Size:** M · **Depends on:** M3 · **Branch:** `m4/recording-move` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m4--recording-tab-and-stream-move)

**Goal.** Recording tab on the shared row model, `ListPopup`, move-stream via metadata for both Playback and Recording.

## Requirement IDs assigned to this milestone (12)

`VOL-REC-001`, `VOL-REC-002`, `VOL-REC-003`, `VOL-REC-005`, `VOL-REC-006`, `VOL-REC-007`, `VOL-REC-008`, `VOL-PB-006`, `VOL-PB-007`, `VOL-FLT-002`, `VOL-FLT-003`, `VOL-TEST-042`

Recording tab on the shared row model, the modal `ListPopup`, and moving streams (both tabs). Q4 is decided first because it determines the backend setter.

## Pre-flight

`git switch -c m4/recording-move`. Two test clients: `pw-cat --playback /dev/zero --raw …` (or any looping player) and `pw-record --target <serial>`.

## Tasks

### M4-T01 Verify stream move metadata (VF-09, VF-10, Q4)
* **Do:** manual, scratch only: with a playing stream and two sinks (one `support.null-audio-sink`), run `pw-metadata -n default <stream-id> target.object <sink-serial> Spa:Id` and variants (`target.node <sink-id> Spa:Id`, both, string types); observe `pw-link -l`/`wpctl status` for relinking, and removal (`pw-metadata -n default -d <stream-id> target.object`). Repeat for a capture stream → source and → sink serial (monitor attempt). Record the matrix of what worked (WirePlumber version) in this plan, `SPEC.md` §7 (VF-09/10 closed) and Q4 (decided), and fix 02 §3.4 / 03 §5 text.
* **Done when:** the exact `Command::MoveStream` backend recipe is written in 02 §3.4 (keys, types, order, clear procedure).
* **Reqs:** supports VOL-PB-006, VOL-REC-006.

### M4-T02 `ListPopup` widget and popup mode keys
* **Do:** `widgets/popup.rs` (`PopupState`, `ListPopup`: geometry `min(60, W−4)` × `min(items+2, H−4)`, title in border, `▶ ` selection, `✓` current, scrolling); keymap popup mode per 07 §3; `UiState.overlay = Popup(PopupState{ owner: RowKey, kind, … })`; `Action::Popup*`; while open all non-overlay keys ignored except `Ctrl-C` (FLT-003).
* **Tests first:** `widgets::popup::tests::geometry_80x24`, `…::geometry_40x12_clamps`, `…::scrolls_when_items_exceed_height`, `…::selected_line_prefix_and_check_suffix`, `…::initial_selection_is_current_value`, `keymap::tests::popup_mode_table` (every row of 07 §3), `…::popup_blocks_normal_keys`, `tests/render.rs::popup_over_playback_80x24` **(snap)** (matches `OUT_POP`-style mockup layout).
* **Reqs:** VOL-FLT-002, VOL-FLT-003.

### M4-T03 Recording row view
* **Do:** `view::recording_rows(&Store, StreamFilter)` (02 §2.1 of 03): same `StreamRow`, source label (monitor port → `Monitor of <sink>`; source; other node; none), meter target resolution fn (used in M7). Titles reuse M2 helpers.
* **Tests first:** `model::view::tests::recording_label_for_source_link`, `…::recording_label_monitor_port_is_monitor_of_sink`, `…::recording_label_from_non_source_node_uses_title`, `…::recording_label_none_when_unlinked`, `…::recording_filter_applications_virtual_all`, `…::recording_hides_vol_meter_streams_by_name_prefix`.
* **Reqs:** VOL-REC-003, VOL-REC-008.

### M4-T04 Recording tab
* **Do:** `tabs/recording.rs` using `row_list`; filter bar names; empty states (`No recording streams` …, hidden-count variant); keys shared with Playback (volume/mute/lock are live on capture streams' own `Props`).
* **Tests first:** `tests/render.rs::recording_80x24` **(snap)** (matches 03 §3 mockup incl. monitor row), `::recording_empty_state`, `tests/scenarios.rs::recording_tab_volume_key_writes_capture_stream_props`.
* **Reqs:** VOL-REC-001, VOL-REC-002, VOL-REC-005.

### M4-T05 Move command (backend) and pending move
* **Do:** `backend/pw/metadata.rs`: `set_stream_target(stream: ObjectId, target: Option<(ObjectId, Serial)>)` per T01's recipe via `Metadata::set_property` (or the sys fallback decided in M1); `Command::MoveStream` handler; `UiState.pending_move: HashMap<ObjectId, PendingMove{label, since}>`: label `→ <desc> …` until a link to the target appears or 2 s → status `Move not applied by the session manager`; status `Moving "<title>" → <label>` on issue.
* **Tests first:** pure: `backend::pw::metadata::tests::move_to_sink_writes_expected_properties` (property list builder), `…::move_to_default_removes_both_keys`; `app::tests::pending_move_cleared_when_link_to_target_appears`, `…::pending_move_times_out_after_2s_with_status`; live: `live::move_stream_between_two_null_sinks` (needs WirePlumber; skip with message otherwise).
* **Reqs:** VOL-PB-006 (backend half).

### M4-T06 Move popup (Playback and Recording)
* **Do:** `p`/`Enter` on a stream row opens the popup (entry 0 `Default device (follow system default)`, then Sinks/Sources by serial, `vol.meter` excluded, `✓` on current link target or entry 0 when no `target.object` metadata); apply → `MoveStream`.
* **Tests first:** `app::tests::p_on_playback_row_opens_sink_popup`, `…::p_on_recording_row_opens_source_popup_without_monitors`, `…::popup_apply_default_sends_move_none`, `…::popup_check_on_current_sink`, `tests/scenarios.rs::move_popup_apply_sends_move_stream_with_target_id`.
* **Reqs:** VOL-PB-006, VOL-REC-006.

### M4-T07 Disappearance during interaction
* **Do:** popup bound to `RowKey`: store change removing the key closes the popup with `Stream ended`; `CommandFailed{NodeGone}` after apply → same message; selection per VOL-ROW-004.
* **Tests first:** `tests/scenarios.rs::stream_removed_while_move_popup_open_closes_popup_with_status`, `::target_sink_removed_while_popup_open_removes_entry`, `::move_apply_for_vanished_stream_reports_stream_ended`.
* **Reqs:** VOL-PB-007, VOL-REC-007, VOL-TEST-042.

### M4-T08 Manual acceptance and close-out
* **Do:** MA-07 on a real session; update statuses, recount, ROADMAP; merge.

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-REC-001 | M4-T04 |
| VOL-REC-002 | M4-T04 |
| VOL-REC-003 | M4-T03 |
| VOL-REC-005 | M4-T04 |
| VOL-REC-006 | M4-T01, M4-T06 |
| VOL-REC-007 | M4-T07 |
| VOL-REC-008 | M4-T03 |
| VOL-PB-006 | M4-T01, M4-T05, M4-T06 |
| VOL-PB-007 | M4-T07 |
| VOL-FLT-002 | M4-T02 |
| VOL-FLT-003 | M4-T02 |
| VOL-TEST-042 | M4-T07 |

## Definition of done (M4)

- [ ] `just check` and `just test-live` green.
- [ ] All 12 M4 IDs `Implemented`; Q4 decided; VF-09/VF-10 closed (with the "monitor move" outcome documented even if negative).
- [ ] MA-07 recorded.

## Risks

| Risk | Mitigation |
|---|---|
| WirePlumber ignores metadata for some streams (`node.dont-reconnect`, fixed `target.object`) | 2 s timeout message; documented limitation. |
| Metadata write permission/semantics differ per WirePlumber version | T01 matrix; oldest-version check in M9. |
| `Monitor of …` label depends on port `port.monitor` being present | fall back to the generic node title if the port is unknown. |
