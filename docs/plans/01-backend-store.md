# M1 — PipeWire backend and registry mirror

**Status:** Done (2026-10-24) · **Size:** L · **Depends on:** M0 · **Branch:** `m1/backend-store` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m1--pipewire-backend-and-registry-mirror)

**Goal.** A `vol-pw` thread that mirrors nodes, devices, ports, links and the `default` metadata into a pure `Store` via a reducer, with coalescing, reconnect/backoff, shutdown and file logging. Headless: verified by unit tests and opt-in live tests, no new UI.

## Requirement IDs assigned to this milestone (30)

`VOL-ARCH-010`, `VOL-ARCH-011`, `VOL-ARCH-013`, `VOL-ARCH-014`, `VOL-ARCH-015`, `VOL-ARCH-016`, `VOL-ARCH-020`, `VOL-ARCH-021`, `VOL-ARCH-022`, `VOL-ARCH-023`, `VOL-ARCH-025`, `VOL-ARCH-026`, `VOL-ARCH-027`, `VOL-ARCH-028`, `VOL-ARCH-030`, `VOL-ARCH-031`, `VOL-ARCH-032`, `VOL-ARCH-040`, `VOL-ARCH-041`, `VOL-ARCH-042`, `VOL-ARCH-043`, `VOL-ARCH-046`, `VOL-ARCH-052`, `VOL-ARCH-053`, `VOL-ARCH-054`, `VOL-IN-009`, `VOL-TEST-001`, `VOL-TEST-002`, `VOL-TEST-003`, `VOL-TEST-007`

Headless milestone: no new screens. Everything pure is tested without a daemon; everything that talks to PipeWire is covered by opt-in live tests plus manual checks on the author's session. Pure code first (T02–T06), then the thread and registry (T07–T13), then glue (T14–T16).

## Starting point

M0 delivered: message/model data types, `AudioBackend`/`BackendHandle`, mock, `Event::Backend`, dirty-flag loop. There is no `Store`, no PipeWire code, and `pipewire = "0.10.1"` is declared but unused.

## Pre-flight

* `git switch -c m1/backend-store`. Create the scratch directory `~/tmp/pw-spike` (outside the repo).
* Private daemon recipe for live tests (VERIFY in T14): `export XDG_RUNTIME_DIR=$(mktemp -d); export PIPEWIRE_RUNTIME_DIR=$XDG_RUNTIME_DIR; pipewire & wireplumber &` (WirePlumber is only needed for metadata/defaults; reducer-level live checks work without it).

## Tasks

### M1-T01 Spike: pipewire-rs 0.10.1 API surface (VF-01)
* **Do:** in the scratch crate (`pipewire = "=0.10.1"`), write ≤ 100 lines proving each API in VOL-ARCH-016: `pipewire::init`, `MainLoop`/`Context`/`connect`, `channel::{channel, Receiver::attach}`, registry `global`/`global_remove`, binding `Node`/`Device`/`Metadata` (`registry.bind`), `subscribe_params`/`enum_params`/`param` listener, `Device::set_param`, `Node::set_param`, `Metadata::set_property` + `property` listener, `core.sync` + `done`, core `error` listener, loop timer, `Stream` + `param_changed`/`process`, building and parsing a `Props` pod with `libspa::pod` (`Value`, `Object`, `serialize::PodSerializer`, `deserialize::PodDeserializer`). (Q3 already decided in M0: 0.8.0 no longer builds against PipeWire 1.6.9.)
* **Output (committed):** a "Spike results" table appended to **this plan** (API → exists in 0.10.1? → exact path/signature → workaround), `SPEC.md` §7 VF-01 marked closed, and edits to VOL-ARCH-015/016 with the real names. If a needed API is missing, record the `pipewire::sys` fallback and which module will hold the `unsafe` (only `backend/pw/**`, `// SAFETY:` per block).
* **Tests first:** n/a (spike).
* **Done when:** table complete; decision recorded; scratch crate deleted.
* **Reqs:** VOL-ARCH-015, VOL-ARCH-016.

### M1-T02 Node/device classification and channel names
* **Do:** in `model/node.rs`: `NodeKind::from_media_class(&str) -> Option<NodeKind>` (01 §5: prefix `Audio/Sink`, `Audio/Source`; exact `Stream/Output/Audio`, `Stream/Input/Audio`; everything else `None`, incl. `Audio/Duplex`), `NodeState::from_info(&str)`, `Channel::from_spa_id(u32)` + `short_name(position)` (02 §2.2 table). In `model/props.rs`: typed accessors on `PropMap` (`get`, `get_u32`, `get_i32`, `flag("node.virtual")`, `node_title()` fallback chain is **M2**, not here). Device: `is_audio_device(&PropMap)` (`media.class == "Audio/Device"`).
* **Files:** `src/model/{node,props,device}.rs`.
* **Tests first:** `model::node::tests::kind_from_media_class_table` (sink, `Audio/Sink/Virtual`, source, `Audio/Source/Virtual`, both stream classes → kinds; `Stream/Output/Video`, `Stream/Input/Video`, `Midi/Bridge`, `Audio/Duplex`, `Audio/Device`, empty → `None`), `…::node_state_from_info_strings`, `…::channel_names_for_known_ids` (2→MON, 3→FL … 14→TC), `…::unknown_channel_is_c_plus_position` (id 0, 1, 15, position 4 → `C5`), `model::props::tests::flag_parses_true_false_and_missing`, `…::get_u32_rejects_garbage`, `model::device::tests::only_audio_device_class_is_audio_device`.
* **Done when:** pass.
* **Reqs:** VOL-IN-009 (Duplex not stored — enforced in T03), VOL-ARCH-028.

### M1-T03 `Store` and reducer
* **Do:** `model/store.rs`: `Store` (01 §5), `ConnState`, `Store::apply(StoreEvent) -> bool`, `Store::clear()`, `revision`. Rules: VOL-ARCH-021…024, 026, 027 exactly (cascade on `NodeRemoved`; drop events for unknown ids; `NodeAdded` with `kind == None` is dropped; non-`Audio/Device` `DeviceAdded` dropped; idempotent re-apply returns `false`; equality checks decide the return value).
* **Files:** `src/model/store.rs`.
* **Tests first (`model::store::tests`):** `node_added_stores_sink`, `node_added_ignores_video_stream`, `node_added_ignores_midi_and_duplex`, `duplicate_node_added_with_same_props_returns_false`, `node_info_replaces_props_and_reports_change`, `node_info_for_unknown_id_is_noop`, `node_audio_before_node_added_is_dropped`, `node_audio_sets_and_replaces`, `node_removed_cascades_ports_links_and_stream_target`, `node_removed_unknown_is_noop`, `device_added_only_for_audio_device`, `device_profiles_enum_and_active_index`, `device_routes_enum_and_active`, `port_added_requires_known_node`, `port_removed_unknown_is_noop`, `link_added_requires_known_nodes`, `link_removed_unknown_is_noop`, `disconnected_clears_maps_keeps_conn`, `connected_sets_conn`, `revision_increments_iff_apply_returns_true`. **(prop)** `model::store::tests::random_event_sequences_never_panic` (arbitrary `StoreEvent`s from a generator over small id space).
* **Done when:** pass.
* **Reqs:** VOL-ARCH-020, 021, 022, 023, 026, 027; VOL-TEST-001, VOL-TEST-002.

### M1-T04 Defaults and stream targets from metadata
* **Do:** `model/metadata.rs`: parse `StoreEvent::Metadata{subject:0, key, type_, value}` for `default.audio.sink|source` and `default.configured.audio.sink|source` (JSON `{"name": String}`; add `serde_json` as a dependency — Q10), and `target.object`/`target.node` with `subject != 0` into `stream_targets`; `value: None` removes; `MetadataCleared` empties both. Unparsable → `None` and returns whether state changed. Add `serde_json` to `Cargo.toml` (check `deny` + MSRV).
* **Files:** `src/model/metadata.rs`, `src/model/store.rs`, `Cargo.toml`.
* **Tests first:** `model::metadata::tests::default_sink_parsed`, `…::default_source_parsed`, `…::configured_keys_parsed_separately`, `…::malformed_json_gives_none`, `…::wrong_type_value_gives_none`, `…::unrelated_keys_ignored`, `…::removal_clears_default`, `…::target_keys_stored_per_subject`, `…::target_key_removed_when_value_none`, `…::metadata_cleared_empties_everything`, `…::name_with_escaped_quotes_roundtrips` (`{"name":"a\"b"}`).
* **Done when:** pass.
* **Reqs:** VOL-ARCH-025, VOL-ARCH-026, VOL-TEST-007.

### M1-T05 Coalescing `Batch`
* **Do:** `backend/coalesce.rs` (pure; no tokio/pipewire): `Batch::new()`, `push(StoreEvent)`, `len()`, `take() -> Vec<StoreEvent>`, implementing the six rules of VOL-ARCH-031 (keyed last-wins for `NodeAudio`/`NodeInfo`/`NodeLatency`/`Device*`/`Metadata(subject,key)`; `Added` + later `Removed` in one batch cancel — including events for that id in between).
* **Files:** `src/backend/coalesce.rs`.
* **Tests first (`backend::coalesce::tests`):** `consecutive_node_audio_keeps_last`, `node_info_keeps_last`, `metadata_same_subject_key_keeps_last`, `metadata_different_keys_both_kept`, `added_then_removed_cancels_including_param_events`, `removed_then_added_both_kept`, `order_of_survivors_preserved`, `flush_threshold_256_reports_full`; **(prop)** `coalesced_application_equals_sequential_application` (random vectors from the same generator as T03; apply raw vs coalesced to two `Store`s; equal except `revision`).
* **Done when:** pass (the property runs ≥ 256 cases).
* **Reqs:** VOL-ARCH-031, VOL-TEST-003.

### M1-T06 File logger
* **Do:** `src/log.rs`: tiny logger (`error|warn|info|debug` macros or functions), enabled only when `VOL_LOG` is set to a level; writes `$XDG_STATE_HOME/vol/vol.log` (fallback `~/.local/state/vol/vol.log`), creating the directory; never writes to stdout/stderr; thread-safe (`Mutex<File>`, `OnceLock`). Initialize in `main` before the terminal. Q10 default (hand-rolled).
* **Files:** `src/log.rs`, `src/main.rs`.
* **Tests first:** `log::tests::disabled_without_env`, `…::level_filtering`, `…::path_prefers_xdg_state_home`, `…::path_falls_back_to_home_local_state`, `…::creates_directory_and_appends` (tempdir via `std::env::temp_dir()` + unique name; no external crate).
* **Done when:** pass.
* **Reqs:** VOL-ARCH-054.

### M1-T07 `vol-pw` thread skeleton, channels, shutdown
* **Do:** `backend/pw/mod.rs`: `spawn_pipewire() -> BackendHandle`: thread `vol-pw` (`std::thread::Builder::name`), `pipewire::init()`, `MainLoop`, attach `pipewire::channel::Receiver<Command>`; `AudioBackend` impl wraps the `Sender` (error → `BackendGone`); outbound `tokio::sync::mpsc::UnboundedSender<BackendEvent>`; `Command::Shutdown` quits the loop; `BackendHandle::shutdown(self, timeout: Duration)` joins (1 s budget, then logs and detaches). All `pipewire` imports confined to `backend/pw/**` (layering test). No registry yet.
* **Files:** `src/backend/pw/mod.rs`, `src/backend/mod.rs`.
* **Tests first:** `tests/live.rs` (**live, ignored**; helper `tests/common/mod.rs::require_live()` returns unless `VOL_TEST_PIPEWIRE=1`): `live::spawn_and_shutdown_within_one_second`, `live::sending_after_shutdown_returns_backend_gone`. Pure: `backend::pw::tests::thread_name_is_vol_pw`.
* **Done when:** live tests pass locally; `just check` green (live ignored).
* **Reqs:** VOL-ARCH-010, 011, 013, 014, 015.

### M1-T08 Connection, core listeners, `Connected`/`Disconnected`
* **Do:** `Context::connect(None)`; core `info` → `server_version`; core `error` → loss flag; emit `Disconnected{NoDaemon, …}` when `connect` fails with ENOENT/ECONNREFUSED, `PermissionDenied` for EACCES/EPERM. Socket description string = `$PIPEWIRE_REMOTE` or `$XDG_RUNTIME_DIR/pipewire-0`. (Reconnect loop is T13; here a single attempt.)
* **Files:** `src/backend/pw/mod.rs`, new `src/backend/pw/conn.rs`.
* **Tests first:** pure: `backend::pw::conn::tests::errno_maps_to_disconnect_reason` (ENOENT, ECONNREFUSED → NoDaemon; EACCES → PermissionDenied; EPIPE → ConnectionLost), `…::socket_description_uses_remote_then_runtime_dir`. Live: `live::no_daemon_reports_disconnected_no_daemon` (`PIPEWIRE_REMOTE=/nonexistent`).
* **Done when:** pass; VF-02 partially observed (log the `error` callback arguments).
* **Reqs:** VOL-ARCH-040 (first attempt), 042.

### M1-T09 Dict → `PropMap` and registry binding
* **Do:** `backend/pw/registry.rs`: registry `global` listener for types Node, Device, Port, Link, Metadata(name == `default` only); bind proxies and keep them (and their listeners) in `Rc<RefCell<…>>` maps keyed by `ObjectId`; `global_remove` drops proxies and emits `*Removed`; node/device `info` listeners emit `NodeInfo`/`DeviceInfo` (only when `props` changed bit set) with full `PropMap` (`object.serial` → `Serial`); ports/links converted from props (`node.id`, `port.direction`, `audio.channel`, `port.monitor`; `link.output.node`, `link.output.port`, `link.input.node`, `link.input.port`). Pure converters: `fn props_to_map<'a>(iter: impl Iterator<Item=(&'a str,&'a str)>) -> PropMap`, `fn port_from_props(...)`, `fn link_from_props(...)`.
* **Files:** `src/backend/pw/registry.rs`.
* **Tests first:** `backend::pw::registry::tests::props_to_map_keeps_all_pairs`, `…::serial_parsed_from_object_serial`, `…::missing_serial_skips_object`, `…::port_from_props_input_and_monitor`, `…::link_from_props_all_fields`, `…::link_with_missing_field_skipped`. Live: `live::registry_snapshot_contains_a_sink` (private daemon with a null sink created by the test via `pw-cli create-node` or `pipewire -c` config; polls the event stream ≤ 3 s).
* **Done when:** pass; `VOL_LOG=debug` of a real run prints counts.
* **Reqs:** VOL-ARCH-010.

### M1-T10 Param decoding (pure, pod-based)
* **Do:** `backend/pw/params.rs`: `decode_props(&Pod) -> Result<NodeAudio, DecodeError>` (`volume` Float default 1.0, `channelVolumes` Array<Float>, `mute` Bool default false, `channelMap` Array<Id>), `decode_enum_profile`, `decode_profile_index`, `decode_enum_route`, `decode_route`, `decode_process_latency` (`ns` Long). Malformed → `Err` and the caller skips the param (VOL-ARCH-053). Encoding helpers arrive in M3/M4/M5/M6.
* **Files:** `src/backend/pw/params.rs`.
* **Tests first:** build pods with `PodSerializer` from known values (per spike names): `backend::pw::params::tests::props_roundtrip_stereo` (volume 1.0, `[0.658492; 2]`, mute false, map `[FL, FR]`), `…::props_missing_volume_defaults_to_one`, `…::props_wrong_type_is_error`, `…::props_empty_channel_volumes_ok_len_zero`, `…::enum_profile_fields` (index, name, description, priority, available yes/no/unknown), `…::profile_index`, `…::enum_route_input_and_output_directions_with_devices_and_profiles`, `…::route_active_index_device_direction`, `…::process_latency_ns`, `…::truncated_pod_is_error_not_panic`. Fixture values copied from the observed `pw-dump` in SPEC 09 §2.
* **Done when:** pass.
* **Reqs:** VOL-ARCH-053.

### M1-T11 Param subscription and initial snapshot
* **Do:** on node bind: `subscribe_params([Props, ProcessLatency])` + `enum_params(Props)`; on device bind: `subscribe_params([EnumProfile, Profile, EnumRoute, Route])` + `enum_params` for each. `param` listeners decode (T10) and emit `NodeAudio`/`NodeLatency`/`DeviceProfiles`/`DeviceProfile`/`DeviceRoutes`/`DeviceActive`; EnumProfile/EnumRoute replies are accumulated per `seq` into one event (all objects of one enumeration). After the registry's first `global` burst, `core.sync(0)`; on the matching `done`, flush the batch and emit `Connected` (VOL-ARCH-032).
* **Files:** `src/backend/pw/registry.rs`, `src/backend/pw/mod.rs`.
* **Tests first:** pure: `backend::pw::registry::tests::enum_replies_accumulate_until_next_seq` (accumulator type), `…::initial_sync_state_machine_emits_connected_once`. Live: `live::connected_arrives_after_snapshot_with_params` (store applied from events has ≥ 1 sink with `audio.is_some()`).
* **Done when:** pass; manual `VOL_LOG=debug`: device 47-like object shows profiles/routes counts.
* **Reqs:** VOL-ARCH-032.

### M1-T12 `default` metadata binding
* **Do:** `backend/pw/metadata.rs`: bind the global whose `metadata.name == "default"`; `property` listener → `StoreEvent::Metadata{subject,key,type_,value}`; on removal of that global → `MetadataCleared`; on reappearance bind again (VOL-ARCH-046). Keep the bound `Metadata` proxy for M4/M5 setters (setter functions are added then).
* **Files:** `src/backend/pw/metadata.rs`, `src/backend/pw/registry.rs`.
* **Tests first:** pure: `backend::pw::metadata::tests::only_default_name_is_bound`, `…::property_none_value_maps_to_removal`. Live: `live::default_metadata_exposes_default_audio_sink` (skipped with a message if no WirePlumber in the private session; run once manually on the real session).
* **Done when:** pass; manual: `systemctl --user restart wireplumber` → `MetadataCleared` then repopulated (log).
* **Reqs:** VOL-ARCH-046.

### M1-T13 Flush timer, reconnect loop, panic guard
* **Do:** 16 ms one-shot timer arming on first event in an empty `Batch` (or immediate flush at ≥ 256); reconnect state machine: attempts with backoff (0.5 s × 2^(n−1), cap 10 s) driven by a loop timer, service the command channel while disconnected (answer `CommandFailed{NotConnected}` except `Shutdown`/`SetMeters`), teardown after core error done **outside** the callback; `catch_unwind` around the thread body → `Disconnected{ConnectionLost}` and restart (max 5 restarts per 60 s, then stay disconnected with a `Pipewire(…, "backend crashed")` event).
* **Files:** `src/backend/pw/mod.rs`, new `src/backend/pw/reconnect.rs`.
* **Tests first:** pure: `backend::pw::reconnect::tests::backoff_schedule_is_0_5_1_2_4_8_10_10`, `…::attempt_resets_after_connected`, `…::restart_budget_allows_five_per_minute_then_denies`, `…::command_while_disconnected_maps_to_not_connected_except_shutdown`. Live: `live::killing_daemon_reports_connection_lost_within_2s`, `live::restarting_daemon_reconnects_within_12s`, `live::shutdown_during_backoff_returns_within_100ms`.
* **Done when:** pass (live tests run by hand on the private daemon).
* **Reqs:** VOL-ARCH-030, 040, 041, 042, 043, 052.

### M1-T14 Live test harness and `just` recipe
* **Do:** finish `tests/common/mod.rs` (private daemon helper: temp runtime dir, spawn `pipewire` (+ optional `wireplumber`), wait for socket, kill on drop, null sink creation via `pw-cli`) — all gated on `VOL_TEST_PIPEWIRE=1` (VOL-QA-007). Add the recipe `test-live` to `justfile` (`VOL_TEST_PIPEWIRE=1 cargo test --test live -- --ignored --test-threads=1`). Document the procedure at the top of `tests/live.rs`. VERIFY the private-daemon env vars on this system and write the result into the doc comment.
* **Files:** `tests/common/mod.rs`, `tests/live.rs`, `justfile` (this single recipe, tooling-owner change).
* **Done when:** `just test-live` passes locally; `just check` (which never runs ignored tests) green.
* **Reqs:** VOL-QA-007 (already M0; harness only), supports TEST-050 (M7).

### M1-T15 App glue: apply backend events to the store
* **Do:** `App` owns `Store`; `Event::Backend(Batch)` → `store.apply` for each (dirty if any `true`); `Connected`/`Disconnected` → `Store::conn`; `BackendEvent::CommandFailed` → stored for M3 status handling (no-op now); `main` uses `spawn_pipewire()` instead of `inert()`; quit sends `Shutdown` and joins (1 s). `VOL_LOG=debug` logs a one-line store summary at most once per second.
* **Files:** `src/app.rs`, `src/main.rs`.
* **Tests first:** `app::tests::batch_event_applies_to_store_and_marks_dirty`, `…::identical_batch_does_not_mark_dirty`, `…::disconnected_clears_store`, `…::connected_sets_conn` (use the mock). Scenario (mock): `tests/scenarios.rs::startup_with_disconnected_then_connected_populates_store` (feature mock).
* **Done when:** pass; manual `VOL_LOG=debug vol` shows summary equal to `pw-dump` counts.
* **Reqs:** VOL-ARCH-012 (end-to-end), VOL-ARCH-020.

### M1-T16 Close-out
* **Do:** record VF-02 result, update statuses of all M1 IDs, recount SPEC table, tick plan, update ROADMAP; run MA-01 first half (no panic without daemon); run the coverage check; merge ff-only.
* **Done when:** Definition of Done below.

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-ARCH-010 | M1-T07, M1-T09 |
| VOL-ARCH-011 | M1-T07 |
| VOL-ARCH-013 | M1-T07 |
| VOL-ARCH-014 | M1-T07 |
| VOL-ARCH-015 | M1-T01, M1-T07 |
| VOL-ARCH-016 | M1-T01 |
| VOL-ARCH-020 | M1-T03, M1-T15 |
| VOL-ARCH-021 | M1-T03 |
| VOL-ARCH-022 | M1-T03 |
| VOL-ARCH-023 | M1-T03 |
| VOL-ARCH-025 | M1-T04 |
| VOL-ARCH-026 | M1-T03, M1-T04 |
| VOL-ARCH-027 | M1-T03 |
| VOL-ARCH-028 | M1-T02 |
| VOL-ARCH-030 | M1-T13 |
| VOL-ARCH-031 | M1-T05 |
| VOL-ARCH-032 | M1-T11 |
| VOL-ARCH-040 | M1-T08, M1-T13 |
| VOL-ARCH-041 | M1-T13 |
| VOL-ARCH-042 | M1-T08, M1-T13 |
| VOL-ARCH-043 | M1-T13 |
| VOL-ARCH-046 | M1-T12 |
| VOL-ARCH-052 | M1-T13 |
| VOL-ARCH-053 | M1-T10 |
| VOL-ARCH-054 | M1-T06 |
| VOL-IN-009 | M1-T02 |
| VOL-TEST-001 | M1-T03 |
| VOL-TEST-002 | M1-T03 |
| VOL-TEST-003 | M1-T05 |
| VOL-TEST-007 | M1-T04 |

Touched here but owned by another milestone (partial work or verification only):

| ID | Owner | Task(s) |
|---|---|---|
| VOL-QA-007 | M0 | M1-T14 |
| VOL-ARCH-012 | M0 | M1-T15 |

## Definition of done (M1)

- [x] `just check` green; `just test-live` green on the author's machine.
- [x] All 30 M1 IDs `Implemented`.
- [x] `grep -rn "pipewire::\|libspa::" src | grep -v "src/backend/pw/"` is empty (layering test).
- [x] VF-01, VF-02 closed; Q3 decided; Spike results table committed.
- [x] Quitting during backoff or while connected exits within 1 s; no `vol-pw` thread remains.

## Risks

| Risk | Mitigation |
|---|---|
| 0.10.1 lacks `Metadata::set_property`/pod helpers | T01 decides: sys fallback. Metadata *setters* are only needed in M4, readers now. |
| Callbacks hold `Rc<RefCell<…>>` cycles → leaks / double borrow | single `State` struct behind `Rc<RefCell>`; callbacks borrow briefly and never call back into PipeWire while borrowed; teardown order documented in `pw/mod.rs`. |
| Dropping `Core` inside its own `error` callback | set flag, `loop.quit()`, tear down after `run()` returns (T13). |
| Param `seq` bookkeeping for EnumRoute/EnumProfile | accumulator keyed by `seq` (T11 pure test). |
| Private daemon needs a seat/dbus (WirePlumber, `ReserveDevice1`) | live tests use `support.null-audio-sink` only and skip metadata tests when WirePlumber is unavailable. |
| Event flood at startup (hundreds of objects) | 16 ms/256-event batching (T05/T13). |

## Spike results (M1-T01, VF-01)

Scratch crate with `pipewire = "=0.10.1"` (92 lines, `#![deny(unsafe_code)]`), compiled and run on 2026-10-24 against a **private** PipeWire 1.6.9 daemon (own `XDG_RUNTIME_DIR`/`PIPEWIRE_RUNTIME_DIR`, no WirePlumber, one `support.null-audio-sink` created with `pw-cli create-node`). "Ran" = the call executed and its callback fired on that daemon; "compiled" = type-checked only.

| API | In 0.10.1? | Exact path / signature | Verified | Workaround / note |
|---|---|---|---|---|
| init | yes | `pipewire::init()` (idempotent, `OnceLock`-guarded) | ran | — |
| Main loop | yes | `pipewire::main_loop::MainLoopRc::new(Option<&spa::utils::dict::DictRef>) -> Result<MainLoopRc, Error>`; `.loop_() -> &pipewire::loop_::Loop`, `.run()`, `.quit()`; `MainLoopBox` is the non-refcounted variant | ran | use `MainLoopRc` (clonable into callbacks) |
| Context / connect | yes | `pipewire::context::ContextRc::new(&MainLoopRc, Option<PropertiesBox>) -> Result<ContextRc, Error>`; `ContextRc::connect_rc(Option<PropertiesBox>) -> Result<pipewire::core::CoreRc, Error>` (`ContextBox`/`Context::connect -> CoreBox<'_>` are the borrowed variants) | ran | — |
| Command channel | yes | `pipewire::channel::channel::<T>() -> (Sender<T>, Receiver<T>)`; `Receiver::attach(self, &Loop, impl Fn(T)) -> AttachedReceiver<'_, T>`; `Sender::send(&self, T) -> Result<(), T>` (`Sender<T>: Clone`, `Send` when `T: Send`) | ran | — |
| Registry | yes | `CoreRc::get_registry_rc() -> Result<RegistryRc, Error>`; `Registry::add_listener_local().global(Fn(&GlobalObject<&DictRef>)).global_remove(Fn(u32)).register() -> registry::Listener`; `GlobalObject { id, type_: pipewire::types::ObjectType, props: Option<&DictRef>, .. }` | `global` ran; `global_remove` compiled | — |
| Bind | yes | `Registry::bind::<T: ProxyT, _>(&GlobalObject<_>) -> Result<T, Error>` with `T` = `pipewire::node::Node`, `pipewire::device::Device`, `pipewire::metadata::Metadata` (also `port::Port`, `link::Link`) | Node, Metadata ran; Device compiled (no devices without a session manager) | proxy + listener must be kept alive together |
| Node/Device params | yes | `subscribe_params(&[ParamType])`; `enum_params(seq: i32, Option<ParamType>, start: u32, num: u32)` (returns `()`); `add_listener_local().info(Fn(&NodeInfoRef / &DeviceInfoRef)).param(Fn(seq: i32, ParamType, index: u32, next: u32, Option<&Pod>)).register() -> NodeListener / DeviceListener` | Node ran; Device compiled | the `seq` passed to `enum_params` is **not** echoed (passed 7, reply carried 1073741836) and the generated seq is not returned, so T11 groups replies by the seq carried in the `param` events, not by the one it sent |
| `set_param` | yes | `Node::set_param(ParamType, flags: u32, &Pod)`, `Device::set_param(ParamType, flags: u32, &Pod)` | Node ran (`channelVolumes [0.5, 0.5]` echoed in the next `Props`); Device compiled | — |
| Metadata | yes | `Metadata::set_property(subject: u32, key: &str, type_: Option<&str>, value: Option<&str>)` (`value: None` removes); `add_listener_local().property(Fn(u32, Option<&str>, Option<&str>, Option<&str>) -> i32).register() -> MetadataListener` | ran on the daemon's `settings` metadata (set + remove of a scratch key both delivered; removal arrives as `type None, value None`) | `default` metadata needs WirePlumber (not started); observed on the real session in T12 |
| Core sync / done | yes | `Core::sync(seq: i32) -> Result<AsyncSeq, Error>`; core listener `.done(Fn(id: u32, AsyncSeq))`; compare `AsyncSeq::seq()` | ran (`sync(0)` returned seq 3, `done(0, 3)`) | match on the returned `AsyncSeq`, not on the argument |
| Core info / error | yes | `.info(Fn(&pipewire::core::Info))` (`Info::version()` = `"1.6.9"`); `.error(Fn(id: u32, seq: i32, res: i32, message: &str))` | ran (`Registry::destroy_global(9999)` → `error(2, 3, -2, "no global 9999")`) | VF-02 (daemon death) stays open for T08/T13 |
| Loop timer | yes | `Loop::add_timer(Fn(expirations: u64)) -> TimerSource<'_>`; `TimerSource::update_timer(value: Option<Duration>, interval: Option<Duration>) -> SpaResult`; drop disarms | ran | — |
| Stream | yes | `pipewire::stream::StreamRc::new(CoreRc, &str, PropertiesBox) -> Result<StreamRc, Error>`; `add_local_listener::<D: Default>()` / `add_local_listener_with_user_data(D)` `.param_changed(FnMut(&Stream, &mut D, id: u32, Option<&Pod>)).process(FnMut(&Stream, &mut D)).register() -> Result<StreamListener<D>, Error>`; `Stream::connect(spa::utils::Direction, Option<u32>, StreamFlags, &mut [&Pod])`; `dequeue_buffer() -> Option<Buffer<'_>>` | created + connected (node and port appeared); `process`/`param_changed` not fired: nothing links the stream without a session manager | exercised for real in M7 (VF-06) |
| Pod build/parse | yes | `pipewire::spa::pod::{Pod, Value, ValueArray, Object, Property}`; `serialize::PodSerializer::serialize(Cursor<Vec<u8>>, &Value) -> Result<(Cursor<Vec<u8>>, u64), GenError>`; `Pod::from_bytes(&[u8]) -> Option<&Pod>`, `Pod::as_bytes()`; `deserialize::PodDeserializer::deserialize_from::<Value>(&[u8]) -> Result<(&[u8], Value), DeserializeError<&[u8]>>` | ran: the null sink's whole `Props` param (incl. the `params` Struct) deserializes into `Value::Object` | — |
| Param keys | partly | safe types: `pipewire::spa::param::ParamType`, `pipewire::spa::utils::{SpaTypes, Id}`; object keys only as plain constants: `pipewire::spa::sys::{SPA_PROP_*, SPA_PARAM_PROFILE_*, SPA_PARAM_ROUTE_*, SPA_PARAM_PROCESS_LATENCY_*, SPA_DIRECTION_*}` | compiled | constants need no `unsafe` |
| `libspa` access | yes | re-exported as `pipewire::spa` (and `pipewire::sys` for `pipewire-sys`) | compiled | no direct `libspa` dependency needed |
| `keys::TARGET_OBJECT` | gated | behind the crate feature `v0_3_44` (not enabled) | — | use the literal `"target.object"` |

**Decision:** every API in VOL-ARCH-016 exists in `pipewire 0.10.1` with a safe wrapper; no `pipewire::sys`/`libspa::sys` function fallback and no `unsafe` block is needed for M1 (`unsafe_code = "deny"` stays). Only plain `pipewire::spa::sys` constants are used.
