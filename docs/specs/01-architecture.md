# 01 — Architecture

Conventions, ID scheme, status legend, glossary: see [`SPEC.md`](../../SPEC.md). Area prefix here: `VOL-ARCH`.
Status is measured against commit `ce3e5a3` (`HEAD` when this spec was written).

## 1. Current repository state (analysis)

| Item | State | Notes |
|---|---|---|
| `Cargo.toml` | Present | edition 2024; no `rust-version`; deps (bumped to latest 2026-10-24): `crossterm 0.29` (`event-stream`), `futures 0.3`, `ratatui 0.30`, `tokio 1` (`full`), `color-eyre 0.6`, `strum 0.28` (`derive`), `pipewire 0.10.1` (lock: `libspa 0.10.1`). `pipewire` is declared but **not used by any compiled code**. |
| `src/main.rs` | Implemented (bootstrap only) | `#[tokio::main]`; `color_eyre::install()`; `ratatui::init()`; `App::new().run(terminal).await`; `ratatui::restore()` after run. Declares `mod app, event, tabs, ui`. `legacy.rs` is **not** declared → not compiled. |
| `src/app.rs` | Stub | `App { running, counter, events, selected_tab }`. Draws on every loop iteration. Keys: `Esc`/`q`/`Ctrl-C` quit; `←`/`→` inc/dec a demo `counter`; `F1`–`F5` switch tab. No `KeyEventKind` filter (Windows/kitty-protocol would double-fire). `tick()` is empty. |
| `src/event.rs` | Implemented (template) | `Event {Tick, Crossterm(CrosstermEvent), App(AppEvent)}`; `AppEvent {Increment, Decrement, SwitchTab(SelectedTab), Quit}`; `EventHandler` = unbounded mpsc + a tokio task merging `crossterm::event::EventStream` and a 30 FPS `tokio::time::interval` (`TICK_FPS = 30.0`). `Increment`/`Decrement` are template leftovers. |
| `src/ui.rs` | Stub | `SelectedTab` enum (`PlaybackTab`, `RecordingTab`, `OutputDevicesTab`, `InputDevicesTab`, `ConfigurationTab`; strum `Display`, `EnumIter`, `FromRepr`; titles "Playback", "Recording", "Output Devices", "Input Devices", "Configuration"). `impl Widget for &App`: 1-row `Tabs` (highlight bold yellow) + a centered `Paragraph` of the tab's `String`. |
| `src/tabs/*.rs` | Stub ×5 | each is `fn <name>_tab(app:&App) -> String` printing the tab name and the demo counter. |
| `src/legacy.rs` | Dead code | Earlier prototype: `LineGauge` volume bars with `symbols::line::DOUBLE`, 3 hard-coded outputs, 5 % steps, `h/l` tabs, `j/k` rows (inverted: `j`=up), `←/→` volume. Not compiled; has `use` lines that no longer compile as a module. Source of design ideas only. MUST be deleted once superseded (VOL-ARCH-002). |
| Verification tooling | Implemented (working tree) | The committed `ci.yml` (fmt/clippy/doc + tests on macOS/Windows only) is deleted in the working tree and replaced by a **local gate**: `justfile` (`just check` = fmt, clippy `-D warnings`, test, doc `-D warnings`, deny, msrv 1.88), `deny.toml`, `scripts/hooks/pre-push` + `scripts/install-hooks.sh`. `.github/workflows/release.yml` builds/publishes only on tags `v*.*.*`. GitHub Actions never verifies code. |
| `flake.nix` | Incomplete | devShell has Rust toolchain, `openssl`, `pkg-config`, `cargo-deny/edit/watch`, `rust-analyzer`. Lacks `pipewire`, `clang`/`libclang` (`LIBCLANG_PATH`), `wireplumber`. |
| `README.md` | Template | Contains only the Ratatui template text. |
| Tests | None | No `#[test]`, no `tests/`, no dev-dependencies. |

## 2. Target module layout

```
src/
  lib.rs             crate root (library `vol`; integration tests and the binary link against it)
  main.rs            thin binary: CLI flags, tokio runtime, backend spawn, terminal lifecycle
  cli.rs             Cli, parse(), USAGE (pure)
  log.rs             optional file logger (VOL_LOG)
  ui_state.rs        UiState, Overlay, Mode, StatusMsg, per-tab ListState, row UI, pending optimistic state
  app.rs             App { store, ui: UiState, backend: BackendHandle, events }; update(Action)->Effects
  action.rs          Action enum (user intent), Effect enum (side effects)
  keymap.rs          fn map_key(&UiState, KeyEvent) -> Option<Action>   (pure)
  event.rs           Event, EventHandler (terminal + tick + backend bridge)
  ui.rs              top-level layout: tab bar, filter bar, body, status line, overlays
  theme.rs           Theme { color: bool } -> Style helpers (NO_COLOR aware)
  model/
    mod.rs           re-exports
    ids.rs           ObjectId, Serial
    props.rs         PropMap + typed accessors for well-known keys
    node.rs          Node, NodeKind, NodeAudio, NodeState, Channel
    device.rs        Device, Profile, RouteInfo, ActiveRoute, Availability
    port.rs          Port
    link.rs          Link
    metadata.rs      Defaults, StreamTarget
    store.rs         Store + Store::apply(StoreEvent) -> bool   (the reducer)
    volume.rs        Level, conversions, stepping, balance-preserving scaling (pure)
    view.rs          derived row models: StreamRow, DeviceRow, CardRow + filters/sort (pure)
  backend/
    mod.rs           trait AudioBackend, BackendEvent, Command, BackendError
    coalesce.rs      Batch (pure coalescing buffer)
    mock.rs          MockBackend + fixtures (scriptable; compiled under cfg(test) or Cargo feature "mock", which `--all-features` enables)
    pw/
      mod.rs         spawn() -> BackendHandle ; thread entry; reconnect loop
      registry.rs    registry listener, proxy bookkeeping
      params.rs      Pod <-> model conversion (Props, Route, EnumRoute, Profile, EnumProfile, ProcessLatency)
      metadata.rs    "default" metadata listener + setters
      meter.rs       peak-meter capture streams
  tabs/
    mod.rs           trait-free: pub fn render(tab, &App, Rect, &mut Buffer)
    playback.rs  recording.rs  output_devices.rs  input_devices.rs  configuration.rs
  widgets/
    volume_row.rs    VolumeRow (3-line / N-line row)
    volume_bar.rs    VolumeBar (0–150 % gauge with 100 % tick)
    meter.rs         PeakMeter
    popup.rs         ListPopup
    help.rs          HelpOverlay
    statusline.rs    StatusLine
    row_list.rs      scrolling list of rows with RowKey selection
```

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ARCH-001 | MUST | Crate layout follows §2. `model/` and `backend/coalesce.rs` MUST NOT import `pipewire`, `libspa`, `ratatui`, `crossterm` or `tokio` (pure, unit-testable). Only `backend/pw/**` imports `pipewire`/`libspa`. | Missing |
| VOL-ARCH-002 | MUST | `src/legacy.rs` is deleted when the first real tab renders live data. | Missing |
| VOL-ARCH-003 | MUST | Tab functions render into `Buffer` (widgets), not `String`. The `fn xxx_tab(&App) -> String` signature is removed. | Stub |
| VOL-ARCH-004 | MUST | `SelectedTab` keeps its current five variants and display strings (stable public names). | Implemented |
| VOL-ARCH-005 | SHOULD | `main.rs` parses `--help/-h`, `--version/-V`, `--mouse`, `--no-color` by hand (no new dependency). Unknown flag → print usage to stderr, exit 2 **before** entering raw mode. | Missing |

## 3. Threading model

```
                       ┌──────────────── tokio runtime (main thread) ────────────────┐
 terminal ──crossterm──►  EventTask ──Event::Crossterm──┐                              │
                       │  tick 30 Hz ─Event::Tick───────┼──► mpsc ──► App::run loop ──►│ draw()
                       │  BackendBridge ─Event::Backend─┘            │ (Action → update)
                       └──────────────────────────────────────────────┼──────────────┘
                                         ▲ BackendEvent               │ Command
                                         │ (tokio mpsc unbounded)     ▼ (pipewire::channel::Sender)
                       ┌──────────────────────── "vol-pw" OS thread ───────────────────┐
                       │  MainLoop ─ Context ─ Core ─ Registry ─ proxies (Node/Device/ │
                       │  Metadata/Port/Link) ─ meter pw_streams ─ timer(16 ms flush)  │
                       └────────────────────────────────────────────────────────────────┘
```

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ARCH-010 | MUST | Exactly one OS thread named `vol-pw` owns **every** PipeWire object (`MainLoop`, `Context`, `Core`, `Registry`, proxies, listeners, meter `Stream`s). They are `!Send`; none crosses threads. | Missing |
| VOL-ARCH-011 | MUST | App → backend: `pipewire::channel::channel::<Command>()`; the `Receiver` is `attach`ed to the loop of `vol-pw`; the `Sender` (`Clone + Send`) lives in `BackendHandle`. Sending never blocks. | Missing |
| VOL-ARCH-012 | MUST | Backend → app: `tokio::sync::mpsc::UnboundedSender<BackendEvent>` (non-async `send` is safe from `vol-pw`). A bridge task forwards each item as `Event::Backend(BackendEvent)` into the existing `EventHandler` channel. | Missing |
| VOL-ARCH-013 | MUST | Shutdown: app sends `Command::Shutdown`; `vol-pw` destroys meter streams, drops listeners/proxies/core/context and returns from `MainLoop::run`; app `join`s with a 1 s budget (if exceeded: log, continue exit). No PipeWire object may outlive the process's terminal restore. | Missing |
| VOL-ARCH-014 | MUST | The tokio side never calls PipeWire FFI. The PipeWire side never touches ratatui/crossterm. | Missing |
| VOL-ARCH-015 | SHOULD | `pipewire::init()` is called once, on `vol-pw`, before the loop is created. (VERIFY: `pipewire 0.10.1` `pipewire::init()`; loop types are `MainLoopRc/MainLoopBox` and `ContextRc/ContextBox` since 0.9; Q3 decided: upgrade.) | Missing |
| VOL-ARCH-016 | MUST | The backend needs these `pipewire` APIs; VERIFY each exists in the pinned version (docs.rs failed to build 0.8.0 docs; the published `pipewire-rs` docs for the current release list them): `pipewire::channel::{channel, Sender, Receiver}` (`Receiver::attach`), `Registry` `global`/`global_remove` listeners, `Node`/`Device` `subscribe_params`, `enum_params(seq, Option<ParamType>, start, num)`, `set_param(ParamType, flags, &Pod)`, `param` listener, `Metadata::set_property(subject, key, type, value)` (value `None` removes) and its `property` listener, `Core::sync` + `done` listener, `Stream` with `param_changed`/`process` listeners, `main_loop` timers (`add_timer`/`update_timer`). Missing pieces are filled through `pipewire::sys`/`libspa::sys` and recorded in Open questions Q3. | Missing |

### 3.1 `AudioBackend` seam (for tests and mock)

```rust
pub trait AudioBackend: Send {
    /// Non-blocking. Err only if the backend thread is gone.
    fn send(&self, cmd: Command) -> Result<(), BackendGone>;
}

pub struct BackendHandle {
    pub commands: Box<dyn AudioBackend>,
    pub events: tokio::sync::mpsc::UnboundedReceiver<BackendEvent>,
}

pub fn spawn_pipewire() -> BackendHandle;            // backend::pw
pub fn spawn_mock(script: MockScript) -> (BackendHandle, MockControl); // backend::mock
```

`App` is generic only through `BackendHandle` (trait object); no generics leak into `ui`/`tabs`.

## 4. Message definitions

### 4.1 Identifiers

```rust
/// Registry global id. Reused by the daemon after removal. Valid only while present in Store.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub u32);

/// props["object.serial"]. Unique for the daemon's lifetime; used for target.object and row identity.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Serial(pub u64);

pub type PropMap = std::collections::BTreeMap<String, String>; // all values stringified as in info.props
```

### 4.2 `BackendEvent` (backend → app)

```rust
pub enum BackendEvent {
    /// Initial enumeration finished (core sync "done" received after all binds). Sent once per connection.
    Connected { server_version: String, socket: String },
    /// Connection lost or never established. Store MUST be cleared by the app on receipt.
    Disconnected { reason: DisconnectReason, retry_in: Option<std::time::Duration>, attempt: u32 },
    /// Coalesced registry/param changes (see §6). Order within the Vec is application order.
    Batch(Vec<StoreEvent>),
    /// Peak levels, at most one message per 40 ms (25 Hz). Linear 0.0..=1.0+ (not dB).
    Meters(Vec<MeterSample>),
    /// A previously sent command could not be applied.
    CommandFailed { cmd: CommandKind, error: BackendError },
}

pub enum DisconnectReason { NoDaemon /*ENOENT/ECONNREFUSED*/, ConnectionLost /*EPIPE or core error id 0*/, PermissionDenied }

pub struct MeterSample { pub target: Serial, pub peak: f32 }  // max |x| over the window, all channels
```

### 4.3 `StoreEvent` (reducer input; produced only by the backend, consumed only by `Store::apply`)

```rust
pub enum StoreEvent {
    NodeAdded   { id: ObjectId, serial: Serial, props: PropMap },
    NodeInfo    { id: ObjectId, props: PropMap, state: NodeState },   // full replacement of props
    NodeAudio   { id: ObjectId, audio: NodeAudio },                   // from Props param
    NodeLatency { id: ObjectId, process_latency_ns: i64 },            // from ProcessLatency param
    NodeRemoved { id: ObjectId },

    DeviceAdded    { id: ObjectId, serial: Serial, props: PropMap },
    DeviceInfo     { id: ObjectId, props: PropMap },
    DeviceProfiles { id: ObjectId, profiles: Vec<Profile> },          // EnumProfile
    DeviceProfile  { id: ObjectId, index: i32 },                      // Profile
    DeviceRoutes   { id: ObjectId, routes: Vec<RouteInfo> },          // EnumRoute
    DeviceActive   { id: ObjectId, active: Vec<ActiveRoute> },        // Route
    DeviceRemoved  { id: ObjectId },

    PortAdded   { id: ObjectId, node: ObjectId, dir: PortDir, channel: Option<String>, monitor: bool },
    PortRemoved { id: ObjectId },
    LinkAdded   { id: ObjectId, out_node: ObjectId, out_port: ObjectId, in_node: ObjectId, in_port: ObjectId },
    LinkRemoved { id: ObjectId },

    /// "default" metadata only. value None = key removed. type_ as delivered (e.g. "Spa:String:JSON", "Spa:Id").
    Metadata { subject: u32, key: String, type_: Option<String>, value: Option<String> },
    MetadataCleared,
}
```

### 4.4 `Command` (app → backend)

```rust
pub enum Command {
    /// Write Props.channelVolumes (per-channel gains, already cubed and divided by Props.volume; see 02-playback §2.1).
    /// master = Some(v) additionally writes Props.volume = v (only used to reset a zero master to 1.0).
    SetVolumes { node: ObjectId, channel_volumes: Vec<f32>, master: Option<f32> },
    SetMute    { node: ObjectId, mute: bool },
    /// Write metadata "default" key `default.configured.audio.{sink|source}` = {"name": node_name}.
    SetDefault { kind: DefaultKind, node_name: String },
    /// target = None clears target.object/target.node for the stream (follow default policy).
    MoveStream { stream: ObjectId, target: Option<ObjectId> },
    SetProfile { device: ObjectId, index: i32 },
    SetRoute   { device: ObjectId, route_index: i32, card_profile_device: i32 },
    /// Idempotent: backend diffs against running meters. Replaces the full desired set.
    SetMeters  { targets: Vec<MeterTarget> },
    Shutdown,
}
pub enum DefaultKind { Sink, Source }
pub struct MeterTarget { pub node: ObjectId, pub serial: Serial, pub kind: MeterKind }
pub enum MeterKind { SinkMonitor, Source, StreamOutput }
pub enum CommandKind { SetVolumes, SetMute, SetDefault, MoveStream, SetProfile, SetRoute }
pub enum BackendError { NotConnected, NodeGone(ObjectId), DeviceGone(ObjectId), PermissionDenied, Pipewire(i32 /*errno*/, String) }
```

## 5. State model

```rust
pub struct Store {
    pub nodes:   BTreeMap<ObjectId, Node>,
    pub devices: BTreeMap<ObjectId, Device>,
    pub ports:   BTreeMap<ObjectId, Port>,
    pub links:   BTreeMap<ObjectId, Link>,
    pub defaults: Defaults,
    pub stream_targets: BTreeMap<ObjectId /*stream node id*/, StreamTarget>,
    pub conn: ConnState,
    pub revision: u64,           // +1 per applied change; UI redraws iff changed
}
pub enum ConnState { Connecting, Connected { server_version: String }, Disconnected { reason: DisconnectReason, retry_at: Instant, attempt: u32 } }

pub struct Node {
    pub id: ObjectId, pub serial: Serial, pub kind: NodeKind, pub props: PropMap,
    pub state: NodeState, pub audio: Option<NodeAudio>, pub process_latency_ns: Option<i64>,
}
pub enum NodeKind { Sink, Source, PlaybackStream, CaptureStream }          // others are not stored
pub enum NodeState { Creating, Suspended, Idle, Running, Error }
pub struct NodeAudio {
    pub volume: f32,                  // Props.volume (master gain multiplier), default 1.0
    pub channel_volumes: Vec<f32>,    // Props.channelVolumes, linear gain per channel
    pub mute: bool,                   // Props.mute
    pub channel_map: Vec<Channel>,    // Props.channelMap
}
pub struct Device { pub id: ObjectId, pub serial: Serial, pub props: PropMap,
    pub profiles: Vec<Profile>, pub active_profile: Option<i32>,
    pub routes: Vec<RouteInfo>, pub active_routes: Vec<ActiveRoute> }
pub struct Profile { pub index: i32, pub name: String, pub description: String, pub priority: i32, pub available: Availability }
pub struct RouteInfo { pub index: i32, pub dir: PortDir, pub name: String, pub description: String,
    pub priority: i32, pub available: Availability, pub profiles: Vec<i32>, pub devices: Vec<i32> }
pub struct ActiveRoute { pub index: i32, pub card_profile_device: i32, pub dir: PortDir }
pub enum Availability { Unknown, No, Yes }      // SPA_PARAM_AVAILABILITY_{unknown,no,yes} = 0,1,2
pub enum PortDir { In, Out }                    // props["port.direction"] = "in"/"out"
pub struct Port { pub id: ObjectId, pub node: ObjectId, pub dir: PortDir, pub channel: Option<String>, pub monitor: bool }
pub struct Link { pub id: ObjectId, pub out_node: ObjectId, pub out_port: ObjectId, pub in_node: ObjectId, pub in_port: ObjectId }
pub struct Defaults { pub sink: Option<String>, pub source: Option<String>,                 // node.name of effective default
                      pub configured_sink: Option<String>, pub configured_source: Option<String> }
pub struct StreamTarget { pub object: Option<String>, pub node: Option<String> }
```

Property sources (observed on PipeWire 1.6.9 + WirePlumber, `pw-dump`):

| Model field | PipeWire source |
|---|---|
| `Node.kind` | `info.props["media.class"]`: starts-with `Audio/Sink` → `Sink`; starts-with `Audio/Source` → `Source`; `Stream/Output/Audio` → `PlaybackStream`; `Stream/Input/Audio` → `CaptureStream`. Anything else (`Video/*`, `Midi/*`, `Audio/Duplex`, `Stream/*/Video`, drivers) is **not stored** (the reducer drops `NodeAdded` for it). |
| `Node.state` | `info.state` (`creating`, `suspended`, `idle`, `running`, `error`) |
| `NodeAudio.*` | node `Props` param (`SPA_PARAM_Props`): `volume` (Float), `channelVolumes` (Array Float), `mute` (Bool), `channelMap` (Array Id) |
| `Device` | registry global type `PipeWire:Interface:Device`, `media.class == "Audio/Device"` only |
| `Profile` | Device `EnumProfile` objects (`index`, `name`, `description`, `priority`, `available`) ; active index from Device `Profile` param |
| `RouteInfo` | Device `EnumRoute` objects (`index`, `direction`, `name`, `description`, `priority`, `available`, `profiles`, `devices`) |
| `ActiveRoute` | Device `Route` param objects (`index`, `device`, `direction`) |
| `Port`/`Link` | registry `Port` (`node.id`, `port.direction`, `audio.channel`, `port.monitor`) and `Link` (`link.output.node`, `link.output.port`, `link.input.node`, `link.input.port`) |
| `Defaults`/`StreamTarget` | metadata named `default`: keys `default.audio.sink`, `default.audio.source`, `default.configured.audio.sink`, `default.configured.audio.source` (subject 0, type `Spa:String:JSON`, value `{"name":"<node.name>"}`); per-stream keys `target.object`, `target.node` with `subject = stream node id` |

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ARCH-020 | MUST | `Store` is the only holder of PipeWire-derived state; widgets receive `&Store` (or derived `view` rows), never raw backend data. | Missing |
| VOL-ARCH-021 | MUST | `Store::apply(&mut self, StoreEvent) -> bool` returns `true` iff observable state changed; `revision` increments only then. It is total (never panics) and idempotent for repeated identical events. | Missing |
| VOL-ARCH-022 | MUST | `NodeRemoved` also removes: the node's `Port`s, any `Link` whose `out_node` or `in_node` is that node, and `stream_targets[id]`. `DeviceRemoved` clears nothing else (nodes disappear via their own events). `PortRemoved`/`LinkRemoved` for unknown ids are no-ops. | Missing |
| VOL-ARCH-023 | MUST | Events for ids not in the store (`NodeAudio` before `NodeAdded`, etc.) are dropped, not buffered. The backend guarantees `*Added` precedes param events for that id. | Missing |
| VOL-ARCH-024 | MUST | A node with `audio == None` is stored but derived rows show volume as `--` and reject volume/mute actions with status `"Volume not available yet"`. | Missing |
| VOL-ARCH-025 | MUST | `Defaults.sink/source` are parsed from `default.audio.*` values as JSON `{"name": String}`; parse failure → `None` and a debug log (never a crash). Values of other metadata keys are ignored. | Missing |
| VOL-ARCH-026 | MUST | The reducer treats `Metadata{subject, key: "target.object" or "target.node", value:None}` as removal. | Missing |
| VOL-ARCH-027 | MUST | `Disconnected` → `Store::clear()` (empties all maps, keeps `conn`). `Connected` → `conn = Connected`. Row selection survives via `RowKey` (VOL-ROW-004, 02 §2.3), not via indices. | Missing |
| VOL-ARCH-028 | SHOULD | Node/device property strings are stored verbatim; derived labels (title chains in 02–06) are computed in `model::view`, not in widgets. | Missing |

## 6. Update and coalescing rules

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ARCH-030 | MUST | `vol-pw` appends `StoreEvent`s to a `Batch` and flushes it as one `BackendEvent::Batch` (a) when a 16 ms one-shot timer on the PW loop fires, or (b) immediately when the batch holds ≥ 256 events. The timer is armed by the first event appended to an empty batch. | Missing |
| VOL-ARCH-031 | MUST | Coalescing inside `Batch` (pure, in `backend/coalesce.rs`): (1) consecutive `NodeAudio` for the same id keep the last; (2) `NodeInfo` same id keep last; (3) `Metadata` same `(subject,key)` keep last; (4) `*Added` followed by `*Removed` for the same id in one batch cancel both; (5) `DeviceRoutes/Profiles/Profile/Active` same id keep last; (6) relative order of the surviving events is preserved. | Missing |
| VOL-ARCH-032 | MUST | Initial snapshot: after binding registry, `vol-pw` binds each `Node`/`Device`/`Metadata("default")`/`Port`/`Link`, calls `subscribe_params` + `enum_params` as per kind (Node: `Props`, `ProcessLatency`; Device: `EnumProfile`, `Profile`, `EnumRoute`, `Route`), then `core.sync(0)`; `Connected` is emitted on the matching `done` **after** flushing the batch. Param replies that arrive later are normal batch traffic. | Missing |
| VOL-ARCH-033 | MUST | The app loop: handle **all** pending events, then draw at most once per `Tick` and only if `dirty` (store revision changed, UI state changed, meter changed, resize). Max draw rate 30 Hz (existing `TICK_FPS`). Idle app (no input, no changes) MUST NOT redraw. | Stub (draws on every event) |
| VOL-ARCH-034 | MUST | `Meters` messages never go through `Batch`. The app stores only the latest `peak` per `Serial` plus the display value with decay (08 §5). | Missing |
| VOL-ARCH-035 | MUST | Volume writes from key-repeat are coalesced in the **app**: at most one `SetVolumes` per node per 16 ms; the latest value wins. | Missing |
| VOL-ARCH-036 | MUST | Optimistic UI: after `SetVolumes`/`SetMute` the `UiState.pending[node]` holds the requested values; rows render pending over store values until (a) the store's value equals pending within `abs(Δp) ≤ 0.5 %` (cubic percent) / equal mute, or (b) 500 ms elapse, whichever first. | Missing |
| VOL-ARCH-037 | MUST | Echo suppression is by comparison only (no sequence numbers); external changes during the pending window are overwritten by the user's value. | Missing |

## 7. Reconnect behavior

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ARCH-040 | MUST | `vol-pw` calls `Context::connect(None)` (honours `PIPEWIRE_REMOTE`, default `pipewire-0` in `$XDG_RUNTIME_DIR`). Failure → `Disconnected{reason: NoDaemon or PermissionDenied, retry_in: Some(d), attempt}`; the UI starts and shows the disconnected state (no abort). | Missing |
| VOL-ARCH-041 | MUST | Backoff: `d = min(10 s, 0.5 s × 2^(attempt-1))` → 0.5, 1, 2, 4, 8, 10, 10, … s. `attempt` resets to 0 on `Connected`. Retries never stop while the app runs. | Missing |
| VOL-ARCH-042 | MUST | Loss detection: core `error` event with `id == 0` (VERIFY: `pipewire::core::Listener::error(id, seq, res, message)`; res `-EPIPE`), or `done`-less socket hangup. Handler sets a flag and quits the loop; teardown (drop proxies, core, context) happens **outside** the callback. | Missing |
| VOL-ARCH-043 | MUST | Between attempts the thread keeps servicing the command channel: every command except `Shutdown`/`SetMeters` replies `CommandFailed{error: NotConnected}` within 100 ms; `Shutdown` exits within 100 ms (retry wait is a loop timer, not `thread::sleep`). | Missing |
| VOL-ARCH-044 | MUST | On `Disconnected` the app: clears `Store`, closes popups, keeps tab/filter selection, and renders the banner of 08 §7 on the active tab. | Missing |
| VOL-ARCH-045 | MUST | Meter streams are destroyed on disconnect and recreated from the app's next `SetMeters` after `Connected`. The app re-sends `SetMeters` on every `Connected`. | Missing |
| VOL-ARCH-046 | SHOULD | WirePlumber restart (daemon alive; metadata object `default` disappears and reappears): the registry `global_remove` for the metadata id emits `MetadataCleared`; on reappearance the backend re-binds. Defaults then re-populate. | Missing |
| VOL-ARCH-047 | MUST | No WirePlumber (daemon up, no `default` metadata ever appears): app works; "default" markers absent; `d` shows status `"No default-device metadata (is WirePlumber running?)"`; moves/ports still work for nodes/devices. | Missing |

## 8. Error handling

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ARCH-050 | MUST | `main` returns `color_eyre::Result<()>`; backend and reducer code use typed errors (`BackendError`), not `eyre`, so tests can match on them. | Stub |
| VOL-ARCH-051 | MUST | A failed command never panics and never blocks the UI; the app shows `CommandFailed` as a status message (07 §6) for 4 s and logs it. | Missing |
| VOL-ARCH-052 | MUST | Any panic in `vol-pw` is caught (`catch_unwind` at thread entry), reported as `Disconnected{ConnectionLost}` and the reconnect loop restarts the backend (max 5 restarts in 60 s, then stay disconnected with status "backend crashed"). | Missing |
| VOL-ARCH-053 | MUST | Malformed pods (unexpected types/lengths) are skipped for that param, with `log::debug`-level text to the log file (§9); the node/device stays visible. | Missing |
| VOL-ARCH-054 | SHOULD | Logging goes to a file only (never stdout/stderr while in raw mode): `$XDG_STATE_HOME/vol/vol.log` (fallback `~/.local/state/vol/vol.log`), enabled only if `VOL_LOG` is set (one of `error`, `warn`, `info`, `debug`). No new mandatory dependency (a ~40-line writer, or `tracing` + `tracing-appender` if the maintainer prefers; decide in Open questions). | Missing |

## 9. Terminal lifecycle

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ARCH-060 | MUST | Order in `main`: parse CLI → `color_eyre::install()` → build backend handle → `ratatui::init()` (raw mode + alternate screen + panic hook that restores the terminal first) → run → `ratatui::restore()` on **every** exit path (including error return of `run`). | Implemented (without backend/CLI) |
| VOL-ARCH-061 | MUST | `--mouse` enables `EnableMouseCapture` after init and `DisableMouseCapture` before restore, also on the panic path (custom hook chained before ratatui's). | Missing |
| VOL-ARCH-062 | MUST | `SIGTERM`, `SIGHUP` (tokio `signal::unix`) trigger the same graceful quit as `q`. `SIGINT` likewise (raw mode normally turns Ctrl-C into a key event; external `kill -INT` still arrives as a signal). | Missing |
| VOL-ARCH-063 | MUST | Only `KeyEventKind::Press` and `KeyEventKind::Repeat` are dispatched; `Release` ignored. | Missing |
| VOL-ARCH-064 | MUST | `Event::Resize` marks dirty; layout is recomputed from `frame.area()` each draw (no cached sizes). | Missing |
| VOL-ARCH-065 | MAY | `Ctrl-Z` suspend is **not** supported in v1 (raw mode delivers it as a key; it is ignored). | Missing |
| VOL-ARCH-066 | MUST | Template leftovers removed: `App.counter`, `AppEvent::Increment/Decrement`, `←/→` counter handlers, `Esc` = quit. (`Esc` no longer quits; see 07.) | Stub |
