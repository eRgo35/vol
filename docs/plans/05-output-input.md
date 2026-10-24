# M5 — Output and Input devices, defaults, ports

**Status:** Not started · **Size:** L · **Depends on:** M4 · **Branch:** `m5/output-input` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m5--output-and-input-devices-defaults-ports)

**Goal.** Output Devices and Input Devices tabs: default markers and `d`, route (port) resolution and popup, hardware/virtual filters, hot-plug and disappearance behavior.

## Requirement IDs assigned to this milestone (19)

`VOL-OUT-001`, `VOL-OUT-002`, `VOL-OUT-003`, `VOL-OUT-004`, `VOL-OUT-005`, `VOL-OUT-006`, `VOL-OUT-008`, `VOL-OUT-009`, `VOL-OUT-010`, `VOL-OUT-011`, `VOL-OUT-013`, `VOL-IN-001`, `VOL-IN-002`, `VOL-IN-003`, `VOL-IN-004`, `VOL-IN-005`, `VOL-IN-007`, `VOL-ARCH-047`, `VOL-TEST-041`

Two device tabs sharing one view/row implementation (04 §2–§4, 05 differences), plus default setting and route selection.

## Pre-flight

`git switch -c m5/output-input`. Ensure the author's session has ≥ 2 sinks (one may be a null sink) and a card with ≥ 2 output routes (HDA analog/HDMI) for MA-08.

## Tasks

### M5-T01 Decisions: hardware heuristic, Duplex/monitors, no-WirePlumber (Q13, Q7, Q14)
* **Do:** inspect `pw-dump` on the author's session for ALSA, Bluetooth (if present), null-sink, loopback, echo-cancel and filter-chain nodes; confirm `is_hardware(node) := props.contains("device.id") ∧ props["node.virtual"] != "true"` classifies them as intended; record counter-examples (e.g. Bluetooth nodes without `card.profile.device`). Confirm Duplex/monitor handling stays deferred; confirm the no-WirePlumber behavior. Write the decisions into SPEC §8 (Q7/Q13/Q14 *Decided*), 04 §2.
* **Reqs:** supports VOL-OUT-002.

### M5-T02 Device row views (pure)
* **Do:** `view::output_rows(&Store, DeviceFilter)`, `view::input_rows(...)` → `DeviceRow` (04 §2): `is_hardware`, title chain (`node.description` → `node.nick` → `node.name`), `is_default` from `Defaults` (name equality; lowest serial wins on duplicates), node state, `PortLabel` via 04 §3.1 (`resolve_ports(&Store, &Node) -> PortInfo{active, candidates}`), filter, sort.
* **Tests first (`model::view::tests`):** `output_rows_only_sinks`, `input_rows_only_sources`, `hardware_filter_requires_device_id_and_not_virtual`, `virtual_filter_complement`, `default_marker_by_node_name`, `default_missing_when_metadata_absent`, `title_chain_description_nick_name`, `port_active_route_matches_card_profile_device_and_direction`, `port_candidates_filtered_by_devices_and_active_profile`, `port_candidates_sorted_priority_desc_index_asc`, `port_unplugged_suffix_when_available_no`, `port_none_for_virtual_node`, `port_single_candidate_has_no_arrow`, `port_active_missing_shows_dash_when_candidates_exist`, `observed_hda_fixture_resolves_line_out_with_available_yes` (shapes from SPEC 04 §3.1).
* **Reqs:** VOL-OUT-002, 003, 005 (resolution half), VOL-IN-002, 003 (marker half), 004 (resolution half).

### M5-T03 Output Devices tab
* **Do:** `tabs/output_devices.rs`: `★` marker column, right label `Port: … ▾`, line 3 node state right-aligned, filter bar names/cycle, empty states, unlocked layout reuse; volume/mute/lock via M3 handlers on device nodes (`OUT-006`).
* **Tests first:** `tests/render.rs::output_80x24_default_unplugged_muted` **(snap)** (the 04 §3.4 mockup), `::output_unlocked_5_1_80x24` **(snap)**, `::output_empty_state`, `::output_filter_hardware_hides_virtual`, `tests/scenarios.rs::volume_keys_on_output_row_write_sink_props`.
* **Reqs:** VOL-OUT-001, 006, 008, VOL-OUT-002.

### M5-T04 Input Devices tab
* **Do:** `tabs/input_devices.rs` mirroring T03 (`Input` routes, `default.audio.source`, filter names, empty text `No input devices`).
* **Tests first:** `tests/render.rs::input_80x24` **(snap)** (05 §2 mockup), `::input_empty_state`, `view::tests::input_port_uses_input_routes_only`.
* **Reqs:** VOL-IN-001, 002, 005.

### M5-T05 Set default (`d`), metadata setter, messages
* **Do:** `backend/pw/metadata.rs::set_default(kind, node_name)` writing `default.configured.audio.{sink|source}` as `Spa:String:JSON` `{"name": …}` (via `serde_json`); `Command::SetDefault` handler; no `default` metadata bound → `CommandFailed` mapped to `No default-device metadata (is WirePlumber running?)` **before** writing; UI waits for the echo of `default.audio.*` (no optimistic star), echo → `Default output: <title>` / `Default input: <title>`, 1 s without echo → `Default not applied (WirePlumber running?)`. `d` on streams → `Streams have no default`; on Configuration → `Not applicable here`.
* **Tests first:** pure: `backend::pw::metadata::tests::default_value_json_escapes_name`, `…::default_key_names_per_kind`; `app::tests::d_on_output_row_sends_set_default_with_node_name`, `…::default_echo_shows_message`, `…::default_not_applied_after_1s`, `…::d_without_metadata_shows_message_and_sends_nothing`, `…::d_on_playback_shows_no_default_message`; live: `live::set_default_sink_updates_default_audio_sink` (needs WirePlumber; skip otherwise).
* **VERIFY:** VF-12 — `wpctl set-default <id>` performs the same write (observe `pw-metadata -n default` before/after).
* **Reqs:** VOL-OUT-004, VOL-IN-003 (write half), VOL-ARCH-047.

### M5-T06 Ports: route popup and `SetRoute`
* **Do:** `params::encode_route(index, device, save)`; `Device::set_param(Route)` handler (`DeviceGone` if missing); port popup via `ListPopup` (entries `<description>` + ` (unplugged)`, `✓` active); selection of an unplugged route allowed with status `<port> is unplugged`; zero candidates → `No selectable ports` (no popup); the row label updates from the echoed `Route` param, not optimistically.
* **Tests first:** `backend::pw::params::tests::encode_route_roundtrips_index_device_save`; `app::tests::p_on_output_row_opens_port_popup_with_candidates`, `…::p_without_candidates_shows_message`, `…::popup_apply_sends_set_route_with_card_profile_device`, `…::unplugged_route_selection_shows_message`; live: `live::set_route_on_alsa_card_is_echoed` (**manual-only** on the author's hardware; not automatable with null sinks — document as MA-08).
* **VERIFY:** VF-11.
* **Reqs:** VOL-OUT-005, VOL-IN-004.

### M5-T07 Disappearance and hot-plug
* **Do:** popup bound to node/device; default sink vanishes → `★` disappears, no re-pick (VOL-OUT-010); new node appended in serial order without stealing selection (OUT-011); command failures map to `Device removed`.
* **Tests first (scenarios):** `output_popup_closes_when_sink_removed_with_device_removed_status`, `default_sink_removed_clears_star_until_metadata_changes`, `hotplug_sink_appended_without_changing_selection`, `input_source_removed_mid_command_reports_device_removed`, `d_sends_set_default_then_sink_removed_reports_device_removed`, **VOL-TEST-041:** `m_sends_set_mute_d_sends_set_default_on_output_tab_and_status_on_playback` (combined scenario per spec text).
* **Reqs:** VOL-OUT-009, 010, 011, VOL-IN-007, VOL-TEST-041.

### M5-T08 Docs, decisions, manual acceptance
* **Do:** MA-06 (default moves; `wpctl status` agrees), MA-08 (port switch); `d` with WirePlumber stopped shows the message (ARCH-047); close VF-11/12; confirm VOL-OUT-013 (dropped features) stays `n/a`; update statuses/recount/ROADMAP; regenerate mockups if snapshots changed; merge.
* **Reqs:** VOL-OUT-013 (informational; status stays `n/a`).

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-OUT-001 | M5-T03 |
| VOL-OUT-002 | M5-T01, M5-T02, M5-T03 |
| VOL-OUT-003 | M5-T02 |
| VOL-OUT-004 | M5-T05 |
| VOL-OUT-005 | M5-T02, M5-T06 |
| VOL-OUT-006 | M5-T03 |
| VOL-OUT-008 | M5-T03 |
| VOL-OUT-009 | M5-T07 |
| VOL-OUT-010 | M5-T07 |
| VOL-OUT-011 | M5-T07 |
| VOL-OUT-013 | M5-T08 |
| VOL-IN-001 | M5-T04 |
| VOL-IN-002 | M5-T02, M5-T04 |
| VOL-IN-003 | M5-T02, M5-T05 |
| VOL-IN-004 | M5-T02, M5-T06 |
| VOL-IN-005 | M5-T04 |
| VOL-IN-007 | M5-T07 |
| VOL-ARCH-047 | M5-T05 |
| VOL-TEST-041 | M5-T07 |

## Definition of done (M5)

- [ ] `just check` and `just test-live` green.
- [ ] All 19 M5 IDs `Implemented` (VOL-OUT-013 stays `n/a`).
- [ ] MA-06, MA-08 recorded; Q7/Q13/Q14 decided; VF-11/12 closed.

## Risks

| Risk | Mitigation |
|---|---|
| Bluetooth/other nodes lack `card.profile.device` or routes | no port label, `No selectable ports`; tested (`port_none_for_virtual_node`). |
| Default echo slower than 1 s on loaded systems | timer is a status nicety only; state still follows metadata. |
| Selecting an unplugged route breaks audio unexpectedly | allowed by spec with an explicit message; revisit if users complain. |
| Same description for two sinks (e.g. identical USB DACs) | title chain unchanged; `RowKey` is `node.name` so selection stays correct; consider appending `node.nick` later (backlog). |
