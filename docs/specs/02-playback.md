# 02 — Shared row model and the Playback tab

Area prefixes: `VOL-VM` (volume model), `VOL-ROW` (row layout/selection), `VOL-MTR` (meters), `VOL-FLT` (filters/popups), `VOL-PB` (Playback tab).
Sections 2.x are **normative for all five tabs**; 03–06 refer to them by number.

## 1. pavucontrol → vol mapping for this tab

| pavucontrol (observable behavior) | vol |
|---|---|
| "Playback" tab lists one row per sink-input (application stream) | one row per `Stream/Output/Audio` node |
| Row: icon, "Application: media name", device combo box, lock-channels toggle, mute toggle, volume slider (0–150 %), level meter | Row: state marker, title, target label, `[M]` mute, lock indicator, `VolumeBar`, percent, dB, `PeakMeter` |
| "Show: All Streams / Applications / Virtual Streams" combo | `StreamFilter { All, Applications, Virtual }` cycled with `f` |
| Move a stream to another sink (combo / context menu) | `p`/`Enter` popup → `Command::MoveStream` (metadata `target.object`/`target.node`) |
| Per-stream "Terminate playback" | **Deferred** (needs `Registry::destroy_global`; permission semantics unverified) |
| "System Sounds" row for event-role streams (stream-restore of role `event`) | **Dropped.** Event-role streams appear as ordinary rows |
| Icons (`application.icon-name`) | **Dropped** (stored in `props`, never rendered) |
| Stream-restore of device/volume per application | **Not implemented by vol**; PipeWire/WirePlumber own persistence |

## 2. Shared definitions

### 2.1 Volume model

PipeWire stores node volumes as **linear amplitude gains** in `Props`: `volume` (Float master multiplier, normally `1.0`) and `channelVolumes` (Array of Float, one per channel, same order as `channelMap`). The user-visible percentage uses the **cubic-root ("PulseAudio-compatible") scale**, identical to `pactl`/pavucontrol/`wpctl`:

```
g   : linear gain (dimensionless, amplitude)      0.0 ≤ g ≤ 3.375 (=1.5³) in vol's UI
ℓ   : level   = ∛g                                0.0 ≤ ℓ ≤ 1.5        (1.0 == "100 %" == 0 dB)
p   : percent = round_half_up(100·ℓ)              0   ≤ p ≤ 150
g   = ℓ³ = (p/100)³
dB  = 20·log10(g) = 60·log10(ℓ)                   100 % → 0.0 dB; 150 % → +10.63 dB; 50 % → −18.06 dB
```

Evidence (live, PipeWire 1.6.9 + WirePlumber, read-only): `wpctl get-volume @DEFAULT_AUDIO_SINK@` → `0.87`; node `Props.channelVolumes` = `[0.658492, 0.658492]` (= 0.869995³; `pactl` raw value `57016/65536 = 0.869995`, shown by `wpctl` rounded to 0.87); `pactl get-sink-volume` → `87% / -3.63 dB` (60·log10 0.87 = −3.63). So `wpctl` value = ℓ, `Props.channelVolumes[i]` = ℓ³, and percent = 100·ℓ. Same for sources (`0.94` ↔ `61680/65536`, `-1.58 dB`).

```rust
pub const LEVEL_MAX: f32 = 1.5;          // 150 %
pub const PERCENT_MAX: i32 = 150;
pub fn gain_to_level(g: f32) -> f32;     // g.max(0.0).cbrt()
pub fn level_to_gain(l: f32) -> f32;     // l.clamp(0.0, LEVEL_MAX).powi(3)
pub fn level_to_percent(l: f32) -> i32;  // (l*100.0 + 0.5).floor() as i32, clamped at 0 (no upper clamp, VOL-VM-003)
pub fn percent_to_level(p: i32) -> f32;  // p.clamp(0,150) as f32 / 100.0
pub fn gain_to_db(g: f32) -> Option<f32>;// None if g <= 1e-5 (≤ −100 dB) else Some(20.0*g.log10())
pub fn fmt_db(g: f32) -> String;         // None -> "-inf dB"; |dB|<0.05 -> " 0.0 dB"; else format!("{:+.1} dB")
```

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-VM-001 | MUST | Effective gain of channel *i* is `g_i = Props.volume × Props.channelVolumes[i]`. `Props.softVolumes`, `monitorVolumes`, `softMute`, `monitorMute` are ignored. (VERIFY on a stream whose `volume ≠ 1.0`: set `pw-cli set-param <id> Props '{ volume: 0.5 }'` on a test stream node, confirm audible/graph effect is multiplicative.) | Implemented |
| VOL-VM-002 | MUST | Node master level `ℓ_m = ∛(max_i g_i)`; shown percent `p_m = level_to_percent(ℓ_m)`. Empty `channelVolumes` → no volume (`--`). | Implemented |
| VOL-VM-003 | MUST | Display range 0–150 %. Values above 150 % (set externally; PipeWire allows up to `channelmix.max-volume`, default 10.0 observed) are displayed clamped to 150 % in the bar, and the text shows the true percent (e.g. `210%`) in over-amplified style. vol never **writes** above 150 %. | Stub (model reports the true percent and the conversions cap writes at 150 %; bar clamp in M2-T04, over-amplified text style in M2-T10, writes in M3) |
| VOL-VM-004 | MUST | Writes: for target levels `ℓ'_i`, `channelVolumes[i] = level_to_gain(ℓ'_i) / volume` when `volume ≥ 1e-6`, else `volume := 1.0` and `channelVolumes[i] = level_to_gain(ℓ'_i)` (carried by `Command::SetVolumes.master = Some(1.0)`). Values are `f32`. | Missing |
| VOL-VM-005 | MUST | Property (proptest): ∀ p ∈ 0..=150: `level_to_percent(gain_to_level(level_to_gain(percent_to_level(p)))) == p`, with the gain passed through an `f32` round trip. | Implemented |
| VOL-VM-006 | MUST | Property: `fmt_db(level_to_gain(1.0)) == " 0.0 dB"`; `fmt_db(level_to_gain(1.5)) == "+10.6 dB"`; `fmt_db(0.0) == "-inf dB"`; monotonic: `p1<p2 ⇒ db(p1)<db(p2)` where `db = gain_to_db(level_to_gain(percent_to_level(p)))`, `None` (−inf) ordered below every finite value; only 0–2 % are `None` (≤ −100 dB). | Implemented |
| VOL-VM-007 | MUST | Steps (07 §3) operate on **integer percent of the master**: `p' = clamp(p_m + Δ, 0, 150)`. Setting an absolute value (digits, Home, End) sets `p' = value`. A step never changes the value by less than the requested Δ because of rounding (stepping from the displayed integer, not from the float). | Missing |
| VOL-VM-008 | MUST | **Locked** (default): all channels move together preserving balance: if `ℓ_m > 0`: `ℓ'_i = ℓ_i · (p'/100)/ℓ_m` (ratio in the level domain; the max channel lands exactly on `p'/100`); if `ℓ_m == 0`: `ℓ'_i = p'/100` for all *i*. Each `ℓ'_i` is then clamped to `[0, 1.5]`. | Missing |
| VOL-VM-009 | MUST | **Unlocked**: only the focused channel *k* changes: `p'_k = clamp(p_k + Δ, 0, 150)`; other channels untouched. | Missing |
| VOL-VM-010 | MUST | Mute is `Props.mute` (Bool), toggled by writing the inverse of the **currently displayed** (pending-overlaid) value. Mute does not alter volumes. | Missing |
| VOL-VM-011 | MUST | Writes are `Node::set_param(ParamType::Props, 0, pod)` with a `Props` object containing only the changed properties (`channelVolumes` [+ `volume`], or `mute`). Pod layout: `Object(SPA_TYPE_OBJECT_Props, SPA_PARAM_Props, Property(SPA_PROP_channelVolumes, Array<Float>), …)`. VERIFY: serialize with `libspa::pod::serialize::PodSerializer` and compare bytes to `pw-cli set-param <id> Props '{ channelVolumes: [0.5, 0.5] }'` on a throwaway null sink (`pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=vol-test media.class=Audio/Sink audio.position=[FL FR] }'`). | Missing |
| VOL-VM-012 | SHOULD | For ACP-backed hardware sinks the node `Props` and the device's active `Route.props` carry the same `channelVolumes` (observed `[0.658492, 0.658492]` in both on this system, `route.hw-volume=true`); vol writes **node Props only** (same as `wpctl`). VERIFY after write that the Device `Route` param echoes the change (no double attenuation: `softVolumes` stays `1.0`). | Missing |

### 2.2 Channel names

`channelMap` elements are `spa_audio_channel` ids. Display name table (3 columns, left-justified):

| id | SPA | name | id | SPA | name |
|---|---|---|---|---|---|
| 2 | `MONO` | `MON` | 9 | `FLC` | `FLC` |
| 3 | `FL` | `FL` | 10 | `FRC` | `FRC` |
| 4 | `FR` | `FR` | 11 | `RC` | `RC` |
| 5 | `FC` | `FC` | 12 | `RL` | `RL` |
| 6 | `LFE` | `LFE` | 13 | `RR` | `RR` |
| 7 | `SL` | `SL` | 14 | `TC` | `TC` |
| 8 | `SR` | `SR` | other (0 `UNKNOWN`, 1 `NA`, ≥15) | — | `C<n>` where *n* = 1-based channel position |

(Ids 0–14 verified against `/usr/include/spa-0.2/spa/param/audio/raw.h` declaration order on PipeWire 1.6.9. If `channelMap` is missing or its length ≠ `channelVolumes.len()`, names are `C1…Cn`.)

### 2.3 Row layout (all tabs)

A row is a vertical block. `W` = body width (terminal width). Tier: `Full` W ≥ 72; `Mid` 56 ≤ W ≤ 71 (no dB column); `Narrow` 40 ≤ W ≤ 55 (no dB, no meter).

**Line 1 (title line)**: `sel(1) marker(1) ␠ title(flex) [␠ label(≤ L)]`
- `sel` = `▌` when the row is selected else `␠`.
- `marker`: streams → `●` if `NodeState::Running` else `○`; devices → `★` if default else `␠`.
- `label` max width `L = min(30, ⌊3W/8⌋)` (80 → 30, 60 → 22, 40 → 15); truncated with `…`; title gets the rest (≥ 8 or label is dropped).

**Line 2 (locked)**: `sel(1) ␠ mute(3) ␠ bar(G) pct(5) [dB(9)] [meter(9)]` where `pct` = `␠` + right-aligned 4 cols (`" 72%"`, `"150%"`), `dB` = `␠` + right-aligned 8 cols, `meter` = `␠` + 8 cells.
`G = W − 6 − 5 − (9 if dB) − (9 if meter)`: Full → `W − 29`; Mid → `W − 20`; Narrow → `W − 11`. (80 → 51, 60 → 40, 40 → 29.)
`mute(3)` = `[M]` when muted else `[ ]`.

**Line 3 (locked)**: `sel(1) ␠␠␠␠ summary(flex) [state(right-aligned)]`; `summary` = `NAME p%` pairs joined by two spaces then two spaces + `locked` (only if channel count ≥ 2; mono → blank). `state` (device tabs only) = lowercase `NodeState` word, right-aligned; dropped if it would overlap.

**Unlocked** (lock off, channel count ≥ 2): line 1 as above; then one line per channel: `sel ␠ mute(3, first channel line only; else 3 spaces) ␠ name(3) ␠ bar(G−4) pct [dB] [meter, first line only]`. No line 3. The focused channel's name is rendered `REVERSED` (08 §3). Single-channel nodes are always "locked".

**Heights**: locked = 3 lines; unlocked = `1 + C` lines. Rows are stacked with no separator.

Bar and meter glyph rules: 08 §4–§5. Bar fill cells: `fill = ⌊G·p/150 + 0.5⌋` (p of the displayed channel/master, clamped 150); 100 % tick column `t = ⌊2G/3⌋` (0-based).

Locked 80×24 (Playback):
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: Applications (f)                                              3 streams  
▌● Firefox: Playing music – Lo-fi beats           → Starship/Matisse HD Audio C…
▌ [ ] ━━━━━━━━━━━━━━━━━━━━━━━━──────────┃────────────────  72%  -8.6 dB █████···
▌    FL 72%  FR 72%  locked                                                     
 ● Spotify: Song Title                            → Starship/Matisse HD Audio C…
  [ ] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┃══════────────── 120%  +4.8 dB ███████▎
     FL 120%  FR 120%  locked                                                   
 ○ mpv: video.mkv                                 → CA0132 Sound Core3D Analog …
  [M] ━━━━━━━━━━━━━━━━━━━───────────────┃────────────────  55% -15.6 dB ········
     FL 55%  FR 55%  locked                                                     
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │ Volume 72%           out: Starship/Matisse… │ ? help        
```
Unlocked first row, 40×16 (Narrow tier; first channel focused; tab bar overflow per 08 §2):
```
 Playback │ Recording │ Output… ▸       
 Show: Applications (f)      2 streams  
▌● Firefox: Playing mus… → Analog Stereo
▌ [ ] FL  ━━━━━━━━━━━━────┃────────  72%
▌     FR  ━━━━━━━━━━━─────┃────────  64%
 ● Spotify: Song Title   → Analog Stereo
  [ ] ━━━━━━━━━━━━━━━━━━━┃═══────── 120%
     FL 120%  FR 120%  locked           
                                        
                                        
                                        
                                        
                                        
                                        
                                        
 ● connected │ Vol 72%        ? help    
```
60×10 (Mid tier):
```
 Playback │ Recording │ Output Devices │ Input… ▸           
 Show: Applications (f)                           1 stream  
▌● Firefox: Playing music                    → Analog Stereo
▌ [ ] ━━━━━━━━━━━━━━━━━━━───────┃─────────────  72% ████····
▌    FL 72%  FR 72%  locked                                 
                                                            
                                                            
                                                            
                                                            
 ● connected │ Volume 72%                      ? help       
```

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-ROW-001 | MUST | Row geometry exactly as above for all three tiers; unit-tested via `TestBackend` at 80×24, 60×10, 40×16. | Stub (tiers, column widths and the locked row in `widgets::volume_row`; meter column blank until M7; unlocked layout in M3) |
| VOL-ROW-002 | MUST | Only rows that fit **entirely** in the body are drawn; the viewport scrolls minimally so the selected row is fully visible (if the selected row alone is taller than the body, it is clipped at the bottom). | Implemented |
| VOL-ROW-003 | MUST | The filter bar's right side shows `<sel>/<total>` (e.g. `2/9`) when not all rows fit, else `<total> <noun>` (`3 streams`, `1 stream`, `3 devices`, `3 cards`). | Implemented |
| VOL-ROW-004 | MUST | **Row identity** `RowKey`: streams → `Serial`; sinks/sources → `node.name` (String); cards → `device.name`. UiState stores `selected: Option<RowKey>` per tab and the last numeric index. If the selected key disappears the selection moves to `min(last_index, len−1)`; if the list is empty it is `None`. | Stub (stream keys only; sink/source and card keys arrive with their rows in M5/M6) |
| VOL-ROW-005 | MUST | Per-row UI state (lock on/off, focused channel) is keyed by `RowKey`, defaults to *locked*, channel 0, and is discarded when the key disappears. Not persisted across runs. | Missing |
| VOL-ROW-006 | MUST | Sort order: streams by `Serial` ascending; devices by `Serial` ascending; cards by `Serial` ascending. Stable across updates (no reordering on volume changes). | Stub (streams only; device and card rows arrive in M5/M6) |
| VOL-ROW-007 | MUST | Title truncation uses grapheme-aware width (`unicode-width`, already a transitive dependency of ratatui) and never splits a multi-cell glyph. | Implemented |

### 2.4 Peak meters

pavucontrol shows a per-row level bar fed by a peak-detect record stream on the row's monitor. vol equivalent: one **capture `pw_stream` per distinct target**, owned by `backend/pw/meter.rs`.

Meter target per row:

| Row kind | `MeterKind` | Captured object |
|---|---|---|
| Sink (Output Devices) | `SinkMonitor` | the sink node (`target.object` = sink `object.serial`, `stream.capture.sink = true`) |
| Source (Input Devices) | `Source` | the source node |
| Playback stream | `StreamOutput` | the stream node itself (its output ports) — VERIFY that WirePlumber links a capture stream to a `Stream/Output/Audio` node given `target.object = <stream serial>`; if it refuses, per-stream meters degrade to "no meter" (cells all `·`) and Open question Q5 is answered |
| Recording stream | `Source` or `SinkMonitor` | the node the stream is currently linked **from** (`Link.in_node == stream.id` → `out_node`); if the link's output `Port.monitor == true` the kind is `SinkMonitor` of that port's node. Unlinked → no meter |

Stream properties (`pw_properties`) for each meter stream:

| Key | Value |
|---|---|
| `media.type` / `media.category` | `Audio` / `Capture` |
| `media.name` | `vol peak meter` |
| `node.name` | `vol.meter.<target serial>` |
| `application.name` | `vol` |
| `vol.meter` | `true` (custom; used to hide own streams in lists) |
| `target.object` | target `object.serial` as decimal string |
| `stream.capture.sink` | `true` iff `SinkMonitor` |
| `stream.monitor` | `true` |
| `node.passive` | `true` (must not keep suspended devices awake; VERIFY the suspended-sink case: meter stays `0` and `wpctl status` shows the sink still `suspended` after 5 s) |
| `node.dont-reconnect` | `true` |
| `node.latency` | `2048/48000` (hint only) |

Format: request `F32` raw audio, rate/channels unconstrained; on `param_changed(Format)` read the channel count and plane layout; in `process` compute `max |x|` over **all channels and samples** of the dequeued buffer.

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-MTR-001 | MUST | `peak` windowing: `vol-pw` keeps `window_max` per meter; every 40 ms (loop timer) it emits `MeterSample{target, peak: window_max}` and resets `window_max = 0`. Emission rate ≤ 25 Hz per target, one `BackendEvent::Meters` per timer tick carrying all targets. Targets with `peak == 0` for two consecutive windows stop being emitted until non-zero (the app treats silence as decaying to 0). | Missing |
| VOL-MTR-002 | MUST | The app computes the *displayed level* from `peak`: `L = clamp((20·log10(peak) + 60) / 60, 0, 1)` (−60 dBFS → 0, 0 dBFS → 1; `peak == 0 → 0`). Display value `D ← max(L, D − dt/3 s)` (20 dB/s fall-off, i.e. 1/3 full-scale per second), `dt` = seconds since the last tick. | Missing |
| VOL-MTR-003 | MUST | Meter rendering: 8 cells, 8 sub-steps each (64 steps): `steps = ⌊D·64 + 0.5⌋`; cell *c* (0-based) shows `' ▏▎▍▌▋▊▉█'[clamp(steps − 8c, 0, 8)]`, with fully-empty cells drawn as `·`. Colors in 08 §5. | Missing |
| VOL-MTR-004 | MUST | `SetMeters` is sent only when the **desired set** changes: the desired set = meter targets of rows currently drawn on the active tab (max 16; extra rows get no meter). Tab switch, scroll, filter change, resize, node add/remove recompute it. Different rows with the same target share one stream. | Missing |
| VOL-MTR-005 | MUST | Backend diff: start streams for new targets, destroy streams for removed targets (stream `disconnect` + drop) within 200 ms. A target whose node disappears is destroyed silently. | Missing |
| VOL-MTR-006 | MUST | The app hides vol's own meter streams: nodes with `props["vol.meter"] == "true"` or a `node.name` starting with `vol.meter.` are excluded in `model::view` for every list. | Implemented |
| VOL-MTR-007 | SHOULD | A meter that has produced no sample for 2 s while its target node is `Running` shows all-empty cells (not a stale level). | Missing |
| VOL-MTR-008 | MAY | `--no-meters` flag / `VOL_NO_METERS=1` disables all meter streams (battery/CPU). Default: meters on. | Missing |

### 2.5 Filters and popups

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-FLT-001 | MUST | Each tab has a `Filter` enum (values in each tab's section). Row 2 of the screen is the **filter bar**: `␠Show: <Filter name> (f)` left, count right. `f` cycles forward, `F` backward, wrapping. The filter is per tab, kept for the session, not persisted. | Stub (filter bar and `f`/`F` done; only Playback has a filter so far, Recording/Output/Input filters arrive in M4/M5) |
| VOL-FLT-002 | MUST | `ListPopup`: modal, centered, width `min(60, W−4)`, height `min(items+2, H−4)`, title in the top border `┌─ <title> ─…┐`, selected line prefixed `▶ `, current value suffixed `  ✓`, scrolling when items > height−2. `Enter` applies and closes, `Esc`/`q`/`h`/`←` cancels. A popup is bound to a `RowKey` and is closed (status `"<noun> removed"`) if that key disappears. | Missing |
| VOL-FLT-003 | MUST | While a popup or the help overlay is open, all non-overlay keys are ignored (except `Ctrl-C`, which quits). | Missing |

Popup example (Output Devices, port chooser):
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: All Output Devices (f)                                     3 devices     
 ★ Starship/Matisse HD Audio Controller Analog Stereo           Port: Line Out ▾
  [ ] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━────┃────────────────  87%  -3.6 dB ███▎····
     FL 87%  FR 87%  locked                                              running
▌  CA0132 Sound Core3D Analog Stereo              Port: Headphones ▾ (unplugged)
▌ [ ] ━━━━━━━━━━━───────────────────────┃────────────────  31% -30.5 dB ········
▌    FL 31%  FR 31%  locked                                            suspended
   GA106 HDMI / DisplayPort 1                                     Port: HDMI 1 ▾
  [M] ━━━━┌─ Port — CA0132 Sound Core3D Analog Stereo ───────────────┐B ········
     FL 10│  Line Out                                                │ suspended
          │▶ Headphones (unplugged)  ✓                               │          
          │  Speakers                                                │          
          └──────────────────────────────────────────────────────────┘          
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │ Default output set   out: Starship/Matisse… │ ? help        
```

## 3. Playback tab

Tab title `Playback`; `SelectedTab::PlaybackTab`; F-key `F1`. Rows come from `model::view::playback_rows(&Store, StreamFilter, &UiPending) -> Vec<StreamRow>` (`UiPending`: optimistic values overlaid on `audio`, filled in M3).

### 3.1 Row population

```rust
pub enum StreamFilter { Applications, All, Virtual }   // cycle order; default Applications
pub enum RowKey { Stream(Serial), /* device/card keys: 04–06 */ }
pub enum TargetLabel { Device(String), NoDevice }   // see 3.3; pending move added in M4
pub struct StreamRow {
    pub key: RowKey, pub node: ObjectId,
    pub title: String, pub media: Option<String>,
    pub running: bool,
    pub target: TargetLabel,
    pub audio: Option<NodeAudio>,
    pub channel_names: Vec<String>,          // §2.2, one per channelVolumes entry
    pub meter: Option<MeterTarget>,          // added in M7 (Q5)
}
```

A node is a Playback row iff `kind == PlaybackStream` ∧ it is not one of vol's meter streams (VOL-MTR-006) ∧ filter matches.
`is_virtual_stream(node) := props["media.role"] ∈ {"abstract","filter"} ∨ props["node.virtual"]=="true" ∨ props.contains("node.link-group")` (heuristic; VERIFY with a `pw-loopback` and a filter-chain instance that both land in "Virtual").
`Applications` = not virtual; `Virtual` = virtual; `All` = both.

### 3.2 Fields

| Field | Source | Fallback chain / rule |
|---|---|---|
| `title` | `application.name` | `node.description` → `application.process.binary` → `node.name` |
| `media` | `media.name` | omitted if empty or equal to `title` |
| Line-1 text | `"{title}: {media}"` if `media` else `"{title}"` | truncated on the right; the `title` part keeps ≥ 8 cells before `media` is cut |
| `running` | `Node.state == Running` | else `○` |
| target label | §3.3 | |
| volume / mute / channels | `NodeAudio` (§2.1–2.3) | `audio == None` → bar area shows `--` centered in dim text, `[ ]`, no meter |
| meter | `StreamOutput(node serial)` | |
| (stored, not rendered) | `application.icon-name`, `media.role`, `application.process.id` | — |

### 3.3 Target label (the stream's current device)

Resolution order, first match wins; label text is prefixed `→ `:
1. `UiState.pending_move[stream]` (set when a move was issued): `→ <desc> …` (trailing `…` = in progress).
2. The `Link` with the lowest id such that `out_node == stream.id` and `store.nodes[in_node].kind == Sink`: `→ <sink desc>`; `<sink desc>` = `node.description` → `node.nick` → `node.name`.
3. Else `(no device)` in dim style.

A pending move is cleared when (a) a link to the requested sink is observed, or (b) 2 s elapse → status `"Move not applied by the session manager"`.

### 3.4 Actions

| Action | Effect |
|---|---|
| Volume keys, mute, lock, channel focus | §2.1; `Command::SetVolumes`/`SetMute` |
| `p` / `Enter` | Popup "Move — `<title>`": entry 0 `Default device (follow system default)` then every non-meter `Sink` node by serial ascending, labelled by `<sink desc>`; `✓` on the one currently linked (entry 0 `✓` if no `target.object` metadata set for this stream). Apply → `Command::MoveStream{stream, target}` |
| `d` | Not applicable on this tab: status `"Streams have no default"` |

Backend for `MoveStream` (VERIFY every step on a live system; this is the least-documented path):
1. `target = Some(sink)` → on metadata `default`: `set_property(subject = stream.id, "target.node", "Spa:Id", "<sink.id>")` and `set_property(subject = stream.id, "target.object", "Spa:Id", "<sink.serial>")`.
2. `target = None` → `set_property(stream.id, "target.node", None, None)` and same for `target.object` (removal).
3. Verification procedure (manual): `pw-metadata -n default <stream-id> target.object <serial> Spa:Id`, then `pw-link -l` (or `wpctl status`) must show the stream re-linked to that sink within 1 s. If `target.node` alone or `target.object` alone suffices on the installed WirePlumber, record it in Open questions Q4 and drop the redundant write.
4. Streams created with `node.dont-reconnect`/fixed `node.target` by the application may refuse to move; the 2 s timeout in §3.3 reports it.

### 3.5 Requirements

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-PB-001 | MUST | Tab renders `StreamRow`s per §2.3 with fields per §3.2 and target label per §3.3. | Stub (row model `model::view::playback_rows` done; tab still prints "Playback Tab"/counter until M2-T10) |
| VOL-PB-002 | MUST | Filter bar: `Show: Applications` / `All Streams` / `Virtual Streams` (display names), cycle order `Applications → All Streams → Virtual Streams`. | Implemented |
| VOL-PB-003 | MUST | Streams appear/disappear live (registry add/remove) without moving the selection off its `RowKey` (VOL-ROW-004). | Stub (selection follows `RowKey`; live add/remove scenario in M2-T10) |
| VOL-PB-004 | MUST | Empty state (no rows, filter `All`): centered dim text `No playback streams` / `Applications that play audio appear here.` With a filter hiding ≥ 1 stream: `No streams match this filter (<n> hidden)` / `Press f to change the filter.` | Missing |
| VOL-PB-005 | MUST | Volume/mute/lock/channel operations per §2.1 on the selected row; the change is visible within one frame (optimistic, VOL-ARCH-036) and confirmed by the store echo. | Missing |
| VOL-PB-006 | MUST | Move popup per §3.4; applying while the stream has vanished → `CommandFailed{NodeGone}` → status `"Stream ended"`. | Missing |
| VOL-PB-007 | MUST | A stream that disappears while its popup is open closes the popup (VOL-FLT-002); one that disappears between key press and command write is a no-op with status `"Stream ended"`. | Missing |
| VOL-PB-008 | MUST | Corked/idle streams (`NodeState::Idle` or `Suspended`) are shown (marker `○`, title dim) — they are not hidden. | Stub (rows kept with `running = false`; rendering in M2-T10) |
| VOL-PB-009 | SHOULD | Meters only for rows drawn (VOL-MTR-004). | Missing |
| VOL-PB-010 | MAY | Terminate stream (`x`): **deferred** (not in v1). | Missing |
| VOL-PB-011 | MUST | Playback tab is the initial tab at startup. | Implemented (`SelectedTab::default()`) |
| VOL-PB-012 | MUST | Tab selection via `F1`–`F5` and the tab bar highlight (bold yellow) remain. | Implemented |
