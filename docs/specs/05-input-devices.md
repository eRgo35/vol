# 05 — Input Devices tab

Area prefix `VOL-IN`. Shared rules: [02 §2](02-playback.md) and [04 §2–§3](04-output-devices.md). Only differences are listed.

## 1. pavucontrol → vol mapping

| pavucontrol | vol |
|---|---|
| Input Devices tab: one row per source (microphones, line-ins, virtual sources); monitor sources hidden by default | one row per `Audio/Source*` node. PipeWire monitors are ports of sinks, **not nodes**, so there is nothing to hide |
| "Show: All / Hardware / Virtual Input Devices" (the "Monitors" variant is a Pulse concept) | `DeviceFilter { All, Hardware, Virtual }` (display names below) |
| "Set as fallback" | `d` → `default.configured.audio.source` |
| Port combo (e.g. Front/Rear Microphone, Line In) | Route with `direction == Input` |
| Meter shows mic level | `Source(serial)` meter |

## 2. Row population

`kind == Source` ∧ `props["vol.meter"] != "true"` ∧ filter. `is_hardware` identical to 04 §2. `title`, `is_default` (`Defaults.source == Some(node.name)`), volume model, mute, channel names: as 04 §3. Port resolution 04 §3.1 with `dir = In`. Sorted by `Serial`.

Filter display names: `All Input Devices`, `Hardware Input Devices`, `Virtual Input Devices`.

Source label rules (line 1 right label): port label only (never a stream-style `←`).

Observed example: `EnumRoute` `analog-input-front-mic` / `analog-input-rear-mic` with `available:"no"` (nothing plugged) ⇒ `Port: Front Microphone ▾ (unplugged)`.

```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: All Input Devices (f)                                      2 devices     
▌★ USB PnP Audio Device Mono                                  Port: Microphone ▾
▌ [ ] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━──┃────────────────  94%  -1.6 dB ████▍···
▌                                                                        running
   Starship/Matisse HD Audio Controller Analog S… Port: Rear Microphone ▾ (unpl…
  [ ] ━━━━━━━━━━━━━━━━━━━━━━━━──────────┃────────────────  70%  -9.3 dB ········
     FL 70%  FR 70%  locked                                            suspended
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │                      in: USB PnP Audio Device │ ? help      
```

## 3. Actions

| Key | Effect |
|---|---|
| volume/mute/lock/channel | 02 §2.1 on the source node `Props` |
| `d` | `Command::SetDefault{kind: Source, node_name}`; metadata key `default.configured.audio.source`; echo via `default.audio.source`. Status `"Default input: <title>"` |
| `p`/`Enter` | Port popup, 04 §3.2 with `Input` routes |

## 4. Meters and privacy

Capturing a microphone for a level meter makes desktop environments show a "microphone in use" indicator. vol opens a meter stream **only for source rows drawn on the active Input Devices tab** (and the Recording tab's linked-source meters). Meter streams are `node.passive` (02 §2.4): an idle/suspended source is not woken, so its meter stays empty until another client records from it. `--no-meters` disables meters entirely (VOL-MTR-008).

## 5. Requirements

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-IN-001 | MUST | Tab renders one row per `Source` node per §2 using 02 §2.3. | Stub |
| VOL-IN-002 | MUST | Filter bar names/cycle per §2; independent of Output Devices' filter. | Missing |
| VOL-IN-003 | MUST | Default marker from `default.audio.source`; `d` per §3. | Missing |
| VOL-IN-004 | MUST | Port popup lists `Input` routes only; unplugged suffix per 04 §3.1. | Missing |
| VOL-IN-005 | MUST | Empty state: `No input devices` / `Open the Configuration tab: a card may be set to Off.` | Missing |
| VOL-IN-006 | MUST | Meter per §4; no meter stream is created for source rows not on screen. Leaving the tab destroys them within 200 ms (VOL-MTR-005). | Missing |
| VOL-IN-007 | MUST | Disappearing source mid-interaction: as VOL-OUT-009/010 with "input". | Missing |
| VOL-IN-008 | MAY | "Monitor of `<sink>`" entries in this tab (pavucontrol's All-Input-Devices variants include monitors): **deferred**; would be synthetic rows (no node) with `SinkMonitor` meter and no volume control. | Missing |
| VOL-IN-009 | MAY | `Audio/Duplex` nodes: not stored (VOL-ARCH §5); revisit if a real device exposes them (Open question Q7). | Missing |
