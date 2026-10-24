# 03 — Recording tab

Area prefix `VOL-REC`. Shared definitions (volume model, row layout, meters, filters, popups, selection): [02-playback.md §2](02-playback.md). This file lists only differences from the Playback tab.

## 1. pavucontrol → vol mapping

| pavucontrol | vol |
|---|---|
| "Recording" tab: one row per source-output (capture stream) | one row per `Stream/Input/Audio` node |
| Device combo (move recording to another source, incl. "Monitor of …") | `p`/`Enter` popup → `Command::MoveStream` to a `Source` node (monitors: **deferred**, §5) |
| "Show: All Streams / Applications / Virtual Streams" | `StreamFilter` (same enum as Playback) |
| Level meter shows the **source's** level (what the app is hearing) | Meter of the node the stream is linked from (02 §2.4) |
| Terminate recording | **Deferred** (as VOL-PB-010) |

## 2. Row population

A node is a Recording row iff `kind == CaptureStream` ∧ `props["vol.meter"] != "true"` ∧ filter matches (`is_virtual_stream` from 02 §3.1). Sorted by `Serial` ascending. `view::recording_rows(&Store, StreamFilter) -> Vec<StreamRow>` (same `StreamRow` type; `target` is a *source label*).

Title/media/state/volume/mute/channel fields: identical to 02 §3.2 (a capture stream's `Props.channelVolumes`/`mute` are the stream's own input gain, set by e.g. `pw-cli`/pavucontrol-equivalents; they are honored by the graph).

### 2.1 Source label (the stream's current input)

Prefixed `← `. First match wins:
1. `UiState.pending_move[stream]`: `← <desc> …`.
2. The `Link` with the lowest id such that `in_node == stream.id`; let `o = store.nodes[out_node]`, `p = store.ports[out_port]`:
   - if `p.monitor == true` (the stream taps a sink's monitor port; `p.node` is the sink): `← Monitor of <sink desc>`;
   - else if `o.kind == Source`: `← <source desc>` (`node.description` → `node.nick` → `node.name`);
   - else (`o` is another stream/filter node): `← <o title>` per 02 §3.2.
3. Else `(no device)` dim.

Meter target: step 2 gives `Source(o.serial)` / `SinkMonitor(sink.serial)`; no link → no meter.

## 3. Actions

| Action | Effect |
|---|---|
| Volume/mute/lock/channel | 02 §2.1 (writes the capture stream's own `Props`) |
| `p` / `Enter` | Popup "Move — `<title>`": entry 0 `Default device (follow system default)`, then every `Source` node (non-`vol.meter`) by serial ascending; `✓` on the currently linked source. `Command::MoveStream{stream, target}` with the same metadata procedure as 02 §3.4 (VERIFY with a capture stream, e.g. `pw-record --target <serial>` as the test client). |
| `d` | Status `"Streams have no default"` |

Screen (80×24, first row selected; second row taps a sink monitor):
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: Applications (f)                                              2 streams  
▌● Telegram Desktop: Voice message                   ← USB PnP Audio Device Mono
▌ [ ] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┃──────────────── 100%   0.0 dB ██▍·····
▌                                                                               
 ○ OBS Studio: Monitor of Speakers                ← Monitor of Starship/Matisse…
  [ ] ━━━━━━━━━━━━━━━───────────────────┃────────────────  45% -20.8 dB ········
     FL 45%  FR 45%  locked                                                     
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │ Volume 100%          in: USB PnP Audio Device │ ? help      
```

## 4. Requirements

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-REC-001 | MUST | Tab renders `StreamRow`s for `CaptureStream` nodes per §2 and 02 §2.3. | Stub |
| VOL-REC-002 | MUST | Filter bar: `Show: Applications` / `All Streams` / `Virtual Streams`; same cycle order as Playback; filter state independent of Playback's. | Missing |
| VOL-REC-003 | MUST | Source label per §2.1, including `Monitor of <sink>` when the link originates from a `port.monitor == true` port. | Missing |
| VOL-REC-004 | MUST | Meter per §2.1 and 02 §2.4; deduplicated with Input Devices meters when the same source is shown on both (only the active tab's rows count). | Missing |
| VOL-REC-005 | MUST | Empty state: `No recording streams` / `Applications that record audio appear here.`; with filter hiding ≥ 1: as VOL-PB-004 with "recording streams". | Missing |
| VOL-REC-006 | MUST | Move popup lists only `Source` nodes (never monitors in v1; never vol's own meters). | Missing |
| VOL-REC-007 | MUST | Disappearing stream/source mid-interaction handled per VOL-PB-007 (status `"Stream ended"` / `"Device removed"`). | Missing |
| VOL-REC-008 | SHOULD | vol's own meter streams (`vol.meter=true`) never appear (VOL-MTR-006); this MUST hold even for the brief interval before their props arrive: nodes whose `node.name` starts with `vol.meter.` are hidden by name as well. | Missing |

## 5. Deferred / dropped

| Feature | Decision | Reason |
|---|---|---|
| Move a capture stream to "Monitor of `<sink>`" | **Deferred** | In PipeWire a monitor is a set of ports on the sink node, not a node. Capturing it requires the stream property `stream.capture.sink=true` at connect time; whether WirePlumber honors `target.object=<sink serial>` from metadata for an already-running stream without that property is unverified (VERIFY: `pw-metadata -n default <capture-stream-id> target.object <sink serial> Spa:Id`; then `pw-link -l`). |
| Per-stream "Terminate recording" | **Deferred** | See VOL-PB-010. |
| Event-role/system-sounds handling | N/A | Playback only. |
