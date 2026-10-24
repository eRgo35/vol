# M6 — Configuration tab (card profiles)

**Status:** Not started · **Size:** S · **Depends on:** M5 · **Branch:** `m6/configuration` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m6--configuration-tab-card-profiles)

**Goal.** Card list with profile popup and live updates.

## Requirement IDs assigned to this milestone (9)

`VOL-CFG-001`, `VOL-CFG-002`, `VOL-CFG-003`, `VOL-CFG-004`, `VOL-CFG-005`, `VOL-CFG-006`, `VOL-CFG-007`, `VOL-CFG-008`, `VOL-TEST-006`

Smallest tab: cards and profiles. Also closes the view-derivation test suite (VOL-TEST-006) for all tabs.

## Pre-flight

`git switch -c m6/configuration`. **Careful:** changing a real card's profile interrupts audio; do manual checks on a spare card (USB) or switch back immediately.

## Tasks

### M6-T01 Card row view
* **Do:** `view::card_rows(&Store) -> Vec<CardRow>` (06 §2): key `device.name`, title chain (`device.description` → `device.nick` → `device.name`), active profile text rules (`Profile: …`, ` (unavailable)`, `Profile: …` before param, `Profile: #<n>`), choices sorted priority desc/index asc.
* **Tests first (`model::view::tests`):** `card_title_chain`, `card_active_profile_description`, `card_profile_unavailable_suffix`, `card_profile_missing_param_shows_ellipsis`, `card_profile_index_not_in_enum_shows_hash_index`, `card_choices_sorted_off_last`, `cards_sorted_by_serial`, `observed_hda_card_fixture_profiles`; **TEST-006 completion checklist:** `view_derivation_checklist_covers_all_tabs` (a single test enumerating that each title-chain/virtual/hardware/port/target-label case exists — implemented as a table over the other tests' fixtures).
* **Reqs:** VOL-CFG-001 (data half), VOL-TEST-006.

### M6-T02 Configuration tab rendering
* **Do:** `tabs/configuration.rs` 3-line rows (title, `Profile: … ▾`, blank), selection bar on the first two lines, right-side count `<n> cards` (or position), no filter name, empty state `No audio cards`/`PipeWire reports no audio devices.`.
* **Tests first:** `tests/render.rs::configuration_80x24` **(snap)** (06 §3 mockup), `::configuration_empty_state`, `::configuration_40x12_truncates_title`.
* **Reqs:** VOL-CFG-001, VOL-CFG-002, VOL-CFG-005.

### M6-T03 Profile popup and `SetProfile`
* **Do:** popup (`Profile — <title>`, choices, `✓` current, unavailable dim + still selectable with message `<profile> is unavailable`); `params::encode_profile(index, save)`; `Device::set_param(Profile)` handler; row text updates from the echoed `Profile` param.
* **Tests first:** `backend::pw::params::tests::encode_profile_roundtrip`; `app::tests::p_on_card_opens_profile_popup`, `…::unavailable_profile_selectable_with_message`, `…::apply_sends_set_profile_with_index`, `…::no_profiles_shows_message`, `tests/render.rs::configuration_popup_80x24` **(snap)** (06 §3 second mockup).
* **VERIFY:** VF-14 (`pw-cli set-param <dev> Profile '{ index: <i>, save: true }'`).
* **Reqs:** VOL-CFG-003, VOL-CFG-004.

### M6-T04 No-ops, removal, stable keys
* **Do:** volume/mute/lock/default keys on this tab → `Not applicable here`; `f`/`F` → `No filters on this tab`; card removal closes popup (`Card removed`); `CommandFailed{DeviceGone}` → same; `RowKey = device.name` persists across profile changes (nodes churn but device persists).
* **Tests first (scenarios):** `keys_not_applicable_on_configuration`, `filter_keys_show_message_on_configuration`, `card_removed_while_popup_open_closes_with_status`, `selection_survives_profile_change_with_node_churn`.
* **Reqs:** VOL-CFG-006, VOL-CFG-007, VOL-CFG-008.

### M6-T05 Manual acceptance and close-out
* **Do:** MA-09 (switch profile of a spare card; Output/Input tabs follow live); close VF-14; statuses/recount/ROADMAP; merge.

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-CFG-001 | M6-T01, M6-T02 |
| VOL-CFG-002 | M6-T02 |
| VOL-CFG-003 | M6-T03 |
| VOL-CFG-004 | M6-T03 |
| VOL-CFG-005 | M6-T02 |
| VOL-CFG-006 | M6-T04 |
| VOL-CFG-007 | M6-T04 |
| VOL-CFG-008 | M6-T04 |
| VOL-TEST-006 | M6-T01 |

## Definition of done (M6)

- [ ] `just check` green.
- [ ] All 9 M6 IDs `Implemented`.
- [ ] MA-09 recorded; VF-14 closed.

## Risks

| Risk | Mitigation |
|---|---|
| Profile switch drops audio streams | document; manual test on a spare card. |
| Cards with huge profile lists | popup scrolls (M4); sorted by priority. |
| `save: true` semantics persisted by WirePlumber differ | observe, document in 06 §4. |
