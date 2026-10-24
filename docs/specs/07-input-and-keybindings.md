# 07 — Input, key bindings, status line

Area prefix `VOL-KEY`. Current implementation (`app.rs::handle_key_events`): `Esc`/`q`/`Ctrl-C` quit; `←`/`→` change a demo counter; `F1`–`F5` switch tabs. Everything else below is new.

## 1. Model

`keymap::map_key(&UiState, KeyEvent) -> Option<Action>` is **pure** (no store access, no I/O) and is unit-tested with table-driven tests (VOL-TEST-020). The mode is derived from `UiState.overlay`:

```rust
pub enum Overlay { None, Help { scroll: u16 }, Popup(PopupState) }
pub enum Mode { Normal, Popup, Help, TooSmall }   // TooSmall: terminal < 40x12 (08 §6)

pub enum Action {
    Quit,
    NextTab, PrevTab, GoTab(SelectedTab),
    RowNext, RowPrev, RowFirst, RowLast,
    VolumeDelta(i32),          // integer percent, see §2.2
    VolumeSet(i32),            // absolute percent 0..=150
    ToggleMute, ToggleLock, ChannelPrev, ChannelNext,
    SetDefault, OpenPicker, FilterNext, FilterPrev,
    ToggleHelp, CloseOverlay,
    PopupNext, PopupPrev, PopupFirst, PopupLast, PopupApply, PopupJump(usize /*0-based*/),
    Mouse(MouseAction),        // §5, only with --mouse
}
```

Dispatch rules: only `KeyEventKind::Press | Repeat`. For `KeyCode::Char(c)`: the `SHIFT` modifier is ignored (terminals differ in reporting capitals); `CONTROL`/`ALT`/`SUPER` modified characters match **only** the explicit entries below (so `Ctrl-m` ≠ `m`). `Ctrl-C` quits in every mode.

## 2. Normal mode

### 2.1 Tabs and rows

| Key(s) | Action | Notes |
|---|---|---|
| `q`, `Ctrl-C` | `Quit` | `q` does **not** quit while an overlay is open |
| `Tab`, `]` | `NextTab` | wraps Configuration → Playback |
| `BackTab` (Shift-Tab), `[` | `PrevTab` | wraps |
| `F1`…`F5` | `GoTab(Playback…Configuration)` | existing behavior, kept |
| `Alt-1`…`Alt-5` | `GoTab(…)` | MAY; digits alone are volume (§3) |
| `j`, `Down` | `RowNext` | clamps at last row (no wrap) |
| `k`, `Up` | `RowPrev` | clamps at first row |
| `g` / `G` | `RowFirst` / `RowLast` | |
| `?` | `ToggleHelp` | |
| `Esc` | `CloseOverlay` if an overlay is open, else clears the transient status message | **no longer quits** (template behavior removed, VOL-ARCH-066) |

### 2.2 Volume, mute, channels, device actions

| Key(s) | Action | Step / semantics |
|---|---|---|
| `l`, `Right` | `VolumeDelta(+5)` | |
| `h`, `Left` | `VolumeDelta(−5)` | |
| `L`, `Shift-Right`, `+`, `=` | `VolumeDelta(+1)` | fine step |
| `H`, `Shift-Left`, `-`, `_` | `VolumeDelta(−1)` | fine step |
| `PageUp` / `PageDown` | `VolumeDelta(+10)` / `(−10)` | coarse step |
| `1`…`9` | `VolumeSet(10·n)` | 10 %…90 % |
| `0` | `VolumeSet(100)` | "0 dB" |
| `Home` / `End` | `VolumeSet(0)` / `VolumeSet(150)` | |
| `m`, `Space` | `ToggleMute` | |
| `c` | `ToggleLock` | per-row; no-op on mono (status `"Needs 2 or more channels"`) |
| `,` / `.` | `ChannelPrev` / `ChannelNext` | only when row is unlocked; else status `"Unlock channels first (c)"` |
| `d` | `SetDefault` | Output/Input tabs only |
| `p`, `Enter` | `OpenPicker` | Playback/Recording: move popup; Output/Input: port popup; Configuration: profile popup |
| `f` / `F` | `FilterNext` / `FilterPrev` | |

Volume semantics (clamp, lock, balance): 02 §2.1 (VOL-VM-007..009). Volume keys on Configuration → `"Not applicable here"`. Keys that need a selection when the list is empty are no-ops (no status).

### 2.3 Why these choices (rationale, informative)

* `h/l` mirror a horizontal slider (←/→) so vim and arrow users get the same result; `H/L` are the "shifted" fine step because Shift-arrows are not reported by every terminal.
* Digits set absolute volume (like media players), hence tab selection avoids digits (`Tab`, `[`/`]`, `F1`–`F5`, optional `Alt-n`).
* `j/k` are **down/up** (the prototype `legacy.rs` had them inverted; that is a bug).

## 3. Popup mode (`ListPopup`)

| Key(s) | Action |
|---|---|
| `j`, `Down` / `k`, `Up` | `PopupNext` / `PopupPrev` (clamp) |
| `g` / `G`, `Home` / `End` | `PopupFirst` / `PopupLast` |
| `1`…`9` | `PopupJump(n−1)` (select only; if the index exists) |
| `Enter`, `l`, `Right`, `Space` | `PopupApply` (closes) |
| `Esc`, `q`, `h`, `Left` | `CloseOverlay` |
| `Ctrl-C` | `Quit` |
| everything else | ignored |

Initial selection = the current value (`✓`) or index 0.

## 4. Help overlay

Opened by `?`; closed by `?`, `Esc`, `q`, `Enter`. `j/k` scroll when taller than the body. Box: width `min(56, W−4)`, height `min(lines+2, H−4)`, title `Help`. Content (exact text, 11 lines):
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: Applications (f)                                              3 streams  
▌● Firefox: Playing music – Lo-fi beats           → Starship/Matisse HD Audio C…
▌ [ ] ━━━━━━━━━━━━━━━━━━━━━━━━──────────┃────────────────  72%  -8.6 dB █████···
▌    FL 72%  FR 72%  locked                                                     
 ● Spotify: ┌─ Help ───────────────────────────────────────────────┐ HD Audio C…
  [ ] ━━━━━━│  Tabs      Tab / S-Tab   [ ]   F1-F5                 │ dB ███████▎
     FL 120%│  Rows      j k  ↑ ↓   g G                            │            
 ○ mpv: vide│  Volume    h l ← →  ±5%     H L +/- ±1%              │e3D Analog …
  [M] ━━━━━━│            PgUp PgDn ±10%   1-9 =10-90%   0 =100%    │ dB ········
     FL 55% │            Home =0%   End =150%                      │            
            │  Mute      m  Space                                  │            
            │  Channels  c lock/unlock   , . pick channel          │            
            │  Default   d                                         │            
            │  Pick      p Enter   (port / profile / move)         │            
            │  Filter    f / F                                     │            
            │  Quit      q  Ctrl-C         Close   Esc ? q         │            
            └──────────────────────────────────────────────────────┘            
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │ Volume 72%           out: Starship/Matisse… │ ? help        
```
(Background shows the screen below; the overlay clears only its own rectangle.)

## 5. Mouse (optional, `--mouse`)

Mouse capture is **off** unless `--mouse` is given (capture breaks terminal text selection). When on (`MAY`; requirements marked MAY):

| Event | Target (hit-test map rebuilt every draw, `UiState.hit: Vec<(Rect, HitTarget)>`) | Action |
|---|---|---|
| Left click | tab title | `GoTab` |
| Left click | row (any line of it) | select row |
| Left click | volume bar of a row | select row and `VolumeSet(round(150·(x − bar_x0)/(bar_w − 1)))` clamped 0..=150 (locked rows: master; unlocked: that channel's line) |
| Left click | `[M]` cell | select row + `ToggleMute` |
| Left click | line-1 right label | select row + `OpenPicker` |
| Left click | filter bar | `FilterNext` |
| Left click | popup item | select + `PopupApply`; outside popup → `CloseOverlay` |
| Wheel up/down | bar of a row | select row + `VolumeDelta(±5)` |
| Wheel up/down | elsewhere | `RowPrev`/`RowNext` |

## 6. Status line

One row at the bottom. Layout (left to right) with `W` columns:

```
 <conn> │ <message-or-hint>            <default> │ ? help
```

| Part | Content |
|---|---|
| `<conn>` | `●` + ` PipeWire <server_version>` when connected; `○ disconnected` otherwise (`○ connecting…` before first result) |
| `<message>` | latest transient message (TTL 4 s; errors styled error, others normal). If none: the **hint** for the active tab: Playback/Recording `h/l vol  m mute  c lock  p move`; Output/Input `h/l vol  m mute  d default  p port`; Configuration `p profile` |
| `<default>` | Playback/Output: `out: <default sink title>`; Recording/Input: `in: <default source title>`; Configuration: empty. Omitted when no default. Truncated to 24 columns with `…` |
| ` │ ? help` | always shown (last 9 columns) |

Width handling: compute right part = `<default>` + `│ ? help`; left part gets the remaining columns, truncated with `…`; if W < 60 `<default>` is dropped first; if still not fitting, `<message>` is truncated; `? help` is dropped only below 24 columns (can't occur above the 40-column minimum).

Transient message catalogue (exact strings; formatted values in `<>`):

| Trigger | Message |
|---|---|
| default set (echo) | `Default output: <title>` / `Default input: <title>` |
| default not applied after 1 s | `Default not applied (WirePlumber running?)` |
| no metadata | `No default-device metadata (is WirePlumber running?)` |
| move issued / not applied | `Moving "<title>" → <label>` / `Move not applied by the session manager` |
| node gone | `Stream ended` (streams) / `Device removed` (devices) / `Card removed` |
| `d` on streams | `Streams have no default` |
| volume not ready | `Volume not available yet` |
| lock/channel misuse | `Needs 2 or more channels` / `Unlock channels first (c)` |
| lock toggle | `Channels locked` / `Channels unlocked` |
| unplugged/unavailable choice | `<port> is unplugged` / `<profile> is unavailable` |
| no ports/profiles | `No selectable ports` / `No profiles` |
| no filter | `No filters on this tab` |
| wrong tab | `Not applicable here` |
| backend errors | `CommandFailed` mapped: `NotConnected` → `Not connected to PipeWire`; `PermissionDenied` → `Permission denied by PipeWire`; `Pipewire(errno, msg)` → `PipeWire error: <msg>` |

## 7. Requirements

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-KEY-001 | MUST | `q` and `Ctrl-C` quit when no overlay is open (and `Ctrl-C` always). | Implemented (`q`, `Ctrl-C`; `Esc` also quits today — to be removed) |
| VOL-KEY-002 | MUST | `F1`–`F5` switch tabs. | Implemented |
| VOL-KEY-003 | MUST | `Tab`/`Shift-Tab`/`[`/`]` cycle tabs with wrap. | Missing |
| VOL-KEY-004 | MUST | Row navigation per §2.1 (`j k ↓ ↑ g G`), clamped. | Missing |
| VOL-KEY-005 | MUST | Volume bindings per §2.2 with steps +5/−5, ±1, ±10, absolute 10–90 %, 100 %, 0 %, 150 %. | Missing |
| VOL-KEY-006 | MUST | `m`/`Space` mute; `c` lock; `,` `.` channel focus; `d` default; `p`/`Enter` picker; `f`/`F` filter; `?` help. | Missing |
| VOL-KEY-007 | MUST | `Esc` closes overlays; with none open it only clears the status message and never quits. | Missing (today `Esc` quits) |
| VOL-KEY-008 | MUST | Popup keys per §3; help overlay per §4. | Missing |
| VOL-KEY-009 | MUST | Only `Press`/`Repeat` are handled; `Release` ignored. | Missing |
| VOL-KEY-010 | MUST | In `Mode::TooSmall` only `q`/`Ctrl-C` and resize act; all other keys are ignored (no blind writes). | Missing |
| VOL-KEY-011 | MUST | Status line per §6 incl. message catalogue and width handling. | Missing |
| VOL-KEY-012 | MAY | Mouse per §5, behind `--mouse`. | Missing |
| VOL-KEY-013 | MAY | `Alt-1`…`Alt-5` tab shortcuts. | Missing |
| VOL-KEY-014 | MUST | The `←`/`→` demo-counter handlers are replaced by volume handlers (VOL-ARCH-066). | Stub |
