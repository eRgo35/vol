# M3 — Volume and mute writes

**Status:** Not started · **Size:** M · **Depends on:** M2 · **Branch:** `m3/volume-mute` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m3--volume-and-mute-writes)

**Goal.** All volume/mute/lock/channel interactions on the selected row, with cubic-root conversion, optimistic echo, key-repeat coalescing and error reporting.

## Requirement IDs assigned to this milestone (20)

`VOL-ARCH-035`, `VOL-ARCH-036`, `VOL-ARCH-037`, `VOL-ARCH-051`, `VOL-VM-004`, `VOL-VM-007`, `VOL-VM-008`, `VOL-VM-009`, `VOL-VM-010`, `VOL-VM-011`, `VOL-VM-012`, `VOL-ROW-001`, `VOL-ROW-005`, `VOL-KEY-005`, `VOL-KEY-014`, `VOL-PB-005`, `VOL-TEST-004`, `VOL-TEST-005`, `VOL-TEST-040`, `VOL-TEST-044`

All write paths of the volume model. Pure stepping/locking/write-plan code first (T01), then pod encoding and backend (T02), then optimistic UI (T03–T05).

## Pre-flight

`git switch -c m3/volume-mute`. A throwaway test node for manual/live checks: `pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=vol-test media.class=Audio/Sink audio.position=[FL FR] }'` (remove afterwards with `pw-cli destroy <id>`).

## Tasks

### M3-T01 Stepping, locking and write plan (pure)
* **Do:** extend `model/volume.rs`: `step(master_percent, delta) -> percent` clamp 0..=150 (VM-007), `set_absolute(p)`, `scale_locked(levels, new_percent) -> levels` (VM-008, balance-preserving; all-equal when master is 0; clamp each to [0, 1.5]), `set_channel(levels, k, new_percent)` (VM-009), `plan_write(&NodeAudio, levels') -> WritePlan{channel_volumes, master: Option<f32>}` (VM-004: divide by `volume`, reset zero master). `WritePlan::into_command(node)`. Q8 decided here.
* **Tests first (`model::volume::tests`):** `step_adds_delta_to_displayed_integer_percent`, `step_clamps_at_0_and_150`, `locked_scaling_preserves_ratios` **(prop)** (ratios within 1e-5; max channel lands on target), `locked_scaling_from_zero_sets_all_equal`, `locked_scaling_clamps_each_channel_at_1_5`, `unlocked_changes_only_focused_channel` **(prop)**, `write_plan_cubes_levels` (87 → `0.658503` ± 1e-6 for both channels), `write_plan_divides_by_master_volume` (volume 0.5), `write_plan_zero_master_sets_master_one`, `write_plan_never_exceeds_3_375_gain`.
* **Reqs:** VOL-VM-004, 007, 008, 009, VOL-TEST-004, VOL-TEST-005.

### M3-T02 Pod encoding and backend write handlers
* **Do:** `backend/pw/params.rs`: `encode_props(channel_volumes: Option<&[f32]>, master: Option<f32>, mute: Option<bool>) -> Vec<u8>` (an `Object` with only the given properties); `backend/pw/mod.rs`: handle `Command::SetVolumes`/`SetMute` → `Node::set_param(ParamType::Props, 0, pod)` on the bound proxy; missing proxy → `CommandFailed{error: NodeGone(id)}`; `permission denied` mapped to `PermissionDenied`.
* **Tests first:** `backend::pw::params::tests::encode_props_volumes_roundtrips_through_decode`, `…::encode_mute_only_has_one_property`, `…::encode_master_and_volumes`; live (ignored): `live::set_volume_on_null_sink_is_echoed_within_200ms`, `live::set_mute_is_echoed`, `live::command_for_removed_node_fails_with_node_gone`.
* **VERIFY:** VF-03 (master `volume` multiplicative), VF-04 (bytes equal to `pw-cli set-param <id> Props '{ channelVolumes: [0.5, 0.5] }'` effect) — record results in SPEC.
* **Reqs:** VOL-VM-011, VOL-ARCH-051 (backend half).

### M3-T03 Pending (optimistic) state and key-repeat coalescing
* **Do:** `UiState.pending: HashMap<ObjectId, Pending{levels, mute, since}>`; `view` overlays pending on `NodeAudio` (the `UiPending` placeholder from M2 is filled); resolution when the store matches within 0.5 % (percent) / mute equal, or after 500 ms (injected `now`); outgoing volume commands coalesced to ≤ 1 per node per 16 ms in `App::on_tick`, latest wins; mute is sent immediately.
* **Tests first (`app::tests`):** `pending_overlays_store_value`, `pending_cleared_when_store_matches`, `pending_cleared_after_500ms`, `ten_deltas_in_one_frame_send_one_command`, `commands_for_different_nodes_sent_independently`, `mute_sent_immediately`, `external_change_during_pending_is_overwritten_by_user_value`.
* **Reqs:** VOL-ARCH-035, 036, 037.

### M3-T04 Row UI state and unlocked layout
* **Do:** `UiState.row_ui: HashMap<RowKey, RowUi{locked: bool = true, channel: usize = 0}>` discarded when the key disappears (VOL-ROW-005); `ToggleLock`, `ChannelPrev/Next` actions; unlocked layout of 02 §2.3 (`1 + C` lines, mute on first line, channel name 3 cols, focused name REVERSED, meter column only on first line).
* **Tests first:** `widgets::volume_row::tests::unlocked_line_prefix_is_10_cols_at_80`, `…::unlocked_gauge_width_is_locked_minus_3`, `…::focused_channel_name_reversed`, `…::mono_row_never_unlocked`, `app::tests::row_ui_discarded_when_key_vanishes`, `tests/render.rs::playback_40x16_first_row_unlocked` **(snap)** (matches the 02 §2.3 mockup), `::output_5_1_unlocked_80x24` is added in M5.
* **Reqs:** VOL-ROW-001, VOL-ROW-005.

### M3-T05 Keymap: volume, mute, lock, channel; status messages
* **Do:** add to `keymap::map_key`: `h/l/←/→` ±5; `H/L/Shift-←/→/-/_/+/=` ±1; `PgUp/PgDn` ±10; `1`–`9` → 10…90; `0` → 100; `Home`/`End` → 0/150; `m`/`Space`; `c`; `,`/`.`. Remove the `←/→` counter remnants (none left after M0; assert via test). `App::update` handles them (volume keys act on the selected stream; Configuration → `Not applicable here`, devices come later). Messages: `Needs 2 or more channels`, `Unlock channels first (c)`, `Volume not available yet`, `Channels locked/unlocked`.
* **Tests first (`keymap::tests`):** `volume_keys_map_to_expected_deltas` (table of all keys), `digits_set_absolute`, `home_end_absolute`, `shift_arrows_fine_step`, `ctrl_h_is_not_volume`; `app::tests::lock_toggle_message`, `…::channel_keys_when_locked_show_message`, `…::volume_on_node_without_audio_shows_message`.
* **Reqs:** VOL-KEY-005, VOL-KEY-014, VOL-PB-005.

### M3-T06 Error reporting
* **Do:** `BackendEvent::CommandFailed` → status via the 07 §6 mapping (`NotConnected`, `PermissionDenied`, `Pipewire`, `NodeGone` → `Stream ended`/`Device removed` by row kind); TTL 4 s; pending entry for that node dropped.
* **Tests first:** `app::tests::command_failed_node_gone_shows_stream_ended`, `…::command_failed_clears_pending`, `…::status_message_expires_after_4s`, `tests/scenarios.rs::fail_next_node_gone_shows_status_then_clears` (mock `fail_next`).
* **Reqs:** VOL-ARCH-051, VOL-TEST-044.

### M3-T07 Scenarios
* **Tests (mock, `tests/scenarios.rs`):** `l_on_72_percent_stream_sends_one_set_volumes_077_cubed`, `ten_l_presses_in_one_frame_send_one_command_with_final_value`, `m_sends_set_mute_not_current`, `c_then_h_changes_only_focused_channel`, `digit_7_sets_70_percent_locked_preserving_balance`, `volume_command_not_sent_for_node_without_audio`.
* **Reqs:** VOL-TEST-040.

### M3-T08 Verification and close-out
* **Do:** VF-05 (node `Props` write on an ACP hardware sink echoes to the device `Route` props; `softVolumes` stays 1.0) — run on a Playback stream **and** record the hardware-sink result (the Output tab arrives in M5, so use `wpctl set-volume` comparison via a scratch run of the backend); MA-03, MA-04, MA-05 on a `pw-cat --playback` stream; update statuses/recount/ROADMAP; merge.
* **Reqs:** VOL-VM-010, VOL-VM-012.

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-ARCH-035 | M3-T03 |
| VOL-ARCH-036 | M3-T03 |
| VOL-ARCH-037 | M3-T03 |
| VOL-ARCH-051 | M3-T02, M3-T06 |
| VOL-VM-004 | M3-T01 |
| VOL-VM-007 | M3-T01 |
| VOL-VM-008 | M3-T01 |
| VOL-VM-009 | M3-T01 |
| VOL-VM-010 | M3-T08 |
| VOL-VM-011 | M3-T02 |
| VOL-VM-012 | M3-T08 |
| VOL-ROW-001 | M3-T04 |
| VOL-ROW-005 | M3-T04 |
| VOL-KEY-005 | M3-T05 |
| VOL-KEY-014 | M3-T05 |
| VOL-PB-005 | M3-T05 |
| VOL-TEST-004 | M3-T01 |
| VOL-TEST-005 | M3-T01 |
| VOL-TEST-040 | M3-T07 |
| VOL-TEST-044 | M3-T06 |

## Definition of done (M3)

- [ ] `just check` green; `just test-live` green (write tests).
- [ ] All 20 M3 IDs `Implemented`; Q8 decided; VF-03/04/05 closed.
- [ ] MA-03/04/05 recorded; 150 % reachable with `═` beyond the tick.

## Risks

| Risk | Mitigation |
|---|---|
| Streams apply `volume` differently (app-controlled master) | VM-004 handles `volume ≠ 1`; VF-03 verifies; fall back to writing `volume` only if observed necessary (spec edit). |
| Echo flicker while holding a key | pending overlay (T03) + 0.5 % tolerance. |
| Hardware volume quantization changes the echoed value (e.g. 87 → 86) | tolerance and 500 ms timeout; display store value afterwards. |
| `PermissionDenied` for streams of other users/sandboxed clients | mapped to a status message, no retry. |
