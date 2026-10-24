# 04 — Output Devices tab

Area prefix `VOL-OUT`. Shared definitions: [02-playback.md §2](02-playback.md). Input Devices ([05](05-input-devices.md)) is the mirror image; shared device-row rules live **here** (§2–§4) and 05 only lists differences.

## 1. pavucontrol → vol mapping

| pavucontrol | vol |
|---|---|
| Output Devices tab: one row per sink (card/hardware + virtual) | one row per `Audio/Sink*` node |
| Row: name, port combo, lock, mute, slider, meter, "Set as fallback" button, "Advanced" (latency offset) | Row: `★`, title, `Port: <route> ▾` label, `[M]`, bar, %, dB, meter, node state; `d` = set default; **Advanced deferred** |
| "Show: All / Hardware / Virtual Output Devices" | `DeviceFilter { All, Hardware, Virtual }` |
| Default sink ("fallback") | `★` on the sink whose `node.name` equals `default.audio.sink` (metadata `default`) |
| Port = PulseAudio sink port | Route (`EnumRoute`/`Route` params of the owning Device) |
| Monitor of sink (as input source) | not a node in PipeWire; its level feeds the meter (`SinkMonitor`), see VOL-IN-008 for the deferred "monitor as input" entry |

## 2. Row population

```rust
pub enum DeviceFilter { All, Hardware, Virtual }        // cycle order; default All
pub struct DeviceRow {
    pub key: String,                  // node.name
    pub node: ObjectId, pub serial: Serial,
    pub title: String,
    pub is_default: bool,
    pub port: Option<PortLabel>,      // None => no port selection available
    pub state: NodeState,
    pub audio: Option<NodeAudio>,
    pub meter: MeterTarget,
}
pub struct PortLabel { pub text: String, pub unplugged: bool, pub choices: usize /*candidate count*/ }
```

A node is an Output Devices row iff `kind == Sink` ∧ `props["vol.meter"] != "true"` ∧ filter matches.
`is_hardware(node) := props.contains("device.id") ∧ props["node.virtual"] != "true"` (observed: ALSA sinks carry `device.id` = owning `Audio/Device` id and no `node.virtual`; null sinks, loopbacks, filter-chain/echo-cancel sinks carry `node.virtual=true` and/or no `device.id`). `Hardware` = `is_hardware`; `Virtual` = `!is_hardware`; `All` = both.

## 3. Fields

| Field | Source | Fallback / rule |
|---|---|---|
| `title` | `node.description` | `node.nick` → `node.name` |
| `is_default` | `Store.defaults.sink == Some(node.name)` | `★` marker (line 1, col 2); else `␠` |
| `state` | `Node.state` | printed on line 3, right-aligned (`running`, `idle`, `suspended`, `creating`, `error`) |
| volume/mute/channels | `NodeAudio` | 02 §2.1–2.3 |
| meter | `SinkMonitor(serial)` | 02 §2.4 |
| port label | §3.1 | right label of line 1 (replaces the stream "→ device" label) |

### 3.1 Port (route) resolution

Let `D = Store.devices[props["device.id"]]` (None → no port label), `N = props["card.profile.device"]` parsed as `i32` (None → no port label), `dir = Out` for sinks (`In` for sources).

* **Active route**: `r ∈ D.active_routes` with `r.card_profile_device == N ∧ r.dir == dir`. Its description is `D.routes[r.index].description`.
* **Candidates**: `c ∈ D.routes` with `c.dir == dir ∧ c.devices.contains(N) ∧ (D.active_profile.is_none() ∨ c.profiles.contains(D.active_profile))`, sorted by `priority` descending then `index` ascending.
* **Label** = `Port: <active description> ▾` and, if `available == No`, suffix ` (unplugged)`. If there is no active route but candidates exist: `Port: — ▾`. One candidate → `▾` omitted. Zero candidates → no label (virtual nodes, many Bluetooth nodes).
* **Availability text**: `Yes`/`Unknown` → none; `No` → ` (unplugged)`.

Example observed (PipeWire 1.6.9, HDA card, `pw-dump`): device 47 `Route` = `{index:3, direction:Output, name:"analog-output-lineout", description:"Line Out", available:yes, device:4, profile:2}`; sink node `card.profile.device = 4`; `EnumRoute[0]` = `analog-input-front-mic` (`direction:Input`, `available:no`, `devices:[0]`).

### 3.2 Setting a port

`p`/`Enter` opens popup "Port — `<title>`" listing candidates (`<description>` + ` (unplugged)` when `No`; `✓` on active). Apply → `Command::SetRoute{device: D.id, route_index, card_profile_device: N}` → backend:

```
Device::set_param(ParamType::Route, 0,
  Object(SPA_TYPE_OBJECT_ParamRoute, SPA_PARAM_Route,
    Property(SPA_PARAM_ROUTE_index,  Int(route_index)),
    Property(SPA_PARAM_ROUTE_device, Int(N)),
    Property(SPA_PARAM_ROUTE_save,   Bool(true))))
```
VERIFY: `pw-cli set-param <device-id> Route '{ index: <i>, device: <N>, save: true }'` on a card with ≥ 2 output routes switches the port and `Route` echoes the new `index`; confirm `save: true` persists across a WirePlumber restart.

Selecting an unplugged port is allowed (it is a user decision); a status message `"<port> is unplugged"` is shown.

### 3.3 Setting the default

`d` → `Command::SetDefault{kind: Sink, node_name}` → metadata `default`, subject `0`, key `default.configured.audio.sink`, type `Spa:String:JSON`, value `{"name":"<node.name>"}` (serialize with `serde_json`, which also escapes quotes). The UI does **not** optimistically move `★`; it moves when `default.audio.sink` changes (observed: WirePlumber mirrors `default.configured.audio.*` into `default.audio.*`; both keys present, same JSON shape). Status `"Default output: <title>"` on echo, or after 1 s without echo `"Default not applied (WirePlumber running?)"`. VERIFY `wpctl set-default <id>` produces the same metadata write (observe with `pw-metadata -n default`).

### 3.4 Layout

80×24, first row selected and default; second row's port is unplugged; third is muted:
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: All Output Devices (f)                                     3 devices     
▌★ Starship/Matisse HD Audio Controller Analog Stereo           Port: Line Out ▾
▌ [ ] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━────┃────────────────  87%  -3.6 dB ███▎····
▌    FL 87%  FR 87%  locked                                              running
   CA0132 Sound Core3D Analog Stereo              Port: Headphones ▾ (unplugged)
  [ ] ━━━━━━━━━━━───────────────────────┃────────────────  31% -30.5 dB ········
     FL 31%  FR 31%  locked                                            suspended
   GA106 HDMI / DisplayPort 1                                     Port: HDMI 1 ▾
  [M] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┃──────────────── 100%   0.0 dB ········
     FL 100%  FR 100%  locked                                          suspended
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │ Default output set   out: Starship/Matisse… │ ? help        
```
Channels unlocked (`c`), 5.1 sink, channel 0 focused:
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: All Output Devices (f)                                     1 device      
▌★ Starship/Matisse HD Audio Controller Analog Surround 5.1     Port: Line Out ▾
▌ [ ] FL  ━━━━━━━━━━━━━━━━━━━━━━━━━━━────┃───────────────  87%  -3.6 dB ███▎····
▌     FR  ━━━━━━━━━━━━━━━━━━━━━━━━━━━────┃───────────────  87%  -3.6 dB ········
▌     FC  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┃─────────────── 100%   0.0 dB ········
▌     LFE ━━━━━━━━━━━━━━━━━━━────────────┃───────────────  60% -13.3 dB ········
▌     RL  ━━━━━━━━━━━━━━━━━━━━━━━━━━━────┃───────────────  87%  -3.6 dB ········
▌     RR  ━━━━━━━━━━━━━━━━━━━━━━─────────┃───────────────  70%  -9.3 dB ········
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │ Channels unlocked    out: Starship/Matisse… │ ? help        
```
Narrow (< 56 columns) and mid tiers: 02 §2.3.

## 4. Requirements

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-OUT-001 | MUST | Tab renders one `DeviceRow` per sink node per §2–§3 with layout of 02 §2.3; line 1 right label = port label. | Stub |
| VOL-OUT-002 | MUST | Filter bar `Show: All Output Devices` / `Hardware Output Devices` / `Virtual Output Devices`; cycle All → Hardware → Virtual. | Missing |
| VOL-OUT-003 | MUST | `★` identifies the default sink by `node.name` equality with metadata `default.audio.sink` (JSON `name`). Nodes with identical `node.name` are impossible (daemon-unique); if two share, the lowest `Serial` wins. | Missing |
| VOL-OUT-004 | MUST | `d` writes `default.configured.audio.sink` per §3.3; on a store without the `default` metadata: status `"No default-device metadata (is WirePlumber running?)"` and no write. | Missing |
| VOL-OUT-005 | MUST | Port label and popup per §3.1–§3.2; popup absent (status `"No selectable ports"`) when candidates == 0, and a single-candidate popup still opens (shows the one entry). | Missing |
| VOL-OUT-006 | MUST | Volume, mute, lock, channel focus per 02 §2.1; sinks with `audio == None` render `--`. | Missing |
| VOL-OUT-007 | MUST | Meter = sink monitor level (02 §2.4). Hidden (cells `·`) for `NodeState::Suspended` sinks (passive meter cannot wake them). | Missing |
| VOL-OUT-008 | MUST | Empty state: `No output devices` / `Open the Configuration tab: a card may be set to Off.`; with a filter hiding ≥ 1: `No devices match this filter (<n> hidden)`. | Missing |
| VOL-OUT-009 | MUST | A sink that disappears (unplug, profile change, module unload) is removed live; selection per VOL-ROW-004; an open popup for it closes with `"Device removed"`; a command in flight → `CommandFailed{NodeGone}` → `"Device removed"`. | Missing |
| VOL-OUT-010 | MUST | A *default* sink that disappears: `★` vanishes (no sink starred) until metadata reports the new default; vol never picks a new default itself. | Missing |
| VOL-OUT-011 | SHOULD | Hot-plug: a new sink appears in `Serial` order (end of list) without stealing selection. | Missing |
| VOL-OUT-012 | MAY | Latency offset (pavucontrol "Advanced" tab): **deferred**. Intended mechanism: for ACP hardware sinks `Route.props.latencyOffsetNsec` (Long; present with value `0` in the observed active route, `Props.latencyOffsetNsec` exists in `spa/param/props.h`), for other nodes the node `ProcessLatency` param (`ns` Long; observed on node 51/57 with `ns: 0`). Unit shown in ms. VERIFY which one WirePlumber honors before specifying keys. | Missing |
| VOL-OUT-013 | — | Dropped: digital pass-through/encoding toggles, per-port "base volume" ticks (`volumeBase` ignored; the 100 % tick is fixed at gain 1.0), port availability *groups*, device icons. | n/a |
