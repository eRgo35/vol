# 08 — Visual design

Area prefix `VOL-VIS`. Row geometry lives in [02 §2.3](02-playback.md); this file defines widgets, glyphs, colors, minimum size and `NO_COLOR`.

## 1. Screen structure

| Rows (0-based) | Content | Height |
|---|---|---|
| 0 | Tab bar | 1 |
| 1 | Filter bar (`Show: … (f)` left, count/position right) | 1 |
| 2 … H−2 | Body (rows / empty state / banner) | H − 3 |
| H−1 | Status line (07 §6) | 1 |

At 80×24: body = rows 2–22 (21 lines = 7 locked rows). Overlays (popup, help) are drawn last over the body with `Clear` on their own rectangle only.

## 2. Tab bar

`ratatui::widgets::Tabs` (already used). Titles from `SelectedTab::to_string()` (unchanged). Divider `│` (U+2502) with one-space padding; the selected title uses the `tab_selected` style (current: `Modifier::BOLD` + `Color::Yellow`). If the full bar exceeds `W`: start from the first tab index *s* for which the selected title fits entirely; draw `◂ ` when *s* > 0; draw tabs from *s* until the width is exhausted, truncating the last title with `…`; draw ` ▸` when titles remain hidden. Example (W = 40, Playback selected): `␠Playback │ Recording │ Output… ▸`.

## 3. Widgets

| Widget | Implementation | Notes |
|---|---|---|
| Tab bar | `ratatui::widgets::Tabs` | existing |
| Filter bar | `Paragraph` of two spans, right part right-aligned | |
| `VolumeBar` | custom `impl Widget` writing cells into `Buffer` | `LineGauge`/`Gauge` are **not** used: they map `ratio ∈ [0,1]` linearly with no 100 % tick and a single fill style. The prototype (`legacy.rs`) used `LineGauge`; superseded. |
| `PeakMeter` | custom `impl Widget` (8 cells) | |
| `VolumeRow` | composes the above per 02 §2.3 | takes `&RowModel`, `&Theme`, `RowUi` |
| `ListPopup`, `HelpOverlay` | `Clear` + `Block::bordered()` (`BorderType::Plain`) + `List`/`Paragraph` | border style `popup_border` |
| `StatusLine` | `Paragraph` | |
| Empty-state / banner | centered `Paragraph`, vertically at `body.y + body.height/3` | |

## 4. Volume bar

Given bar width `G`, percent `p` (the displayed channel or master, ≥ 0; `p_draw = min(p, 150)`):

```
fill = ⌊G·p_draw/150 + 0.5⌋           // number of filled cells, 0..=G
t    = ⌊2G/3⌋                          // 100 % tick column (0-based); G ≥ 3 else no tick
cell i:  i == t            → '┃' (U+2503)            style tick
         i <  fill, i < t  → '━' (U+2501)            style bar_fill
         i <  fill, i > t  → '═' (U+2550)            style bar_over
         otherwise         → '─' (U+2500)            style bar_empty
```

Rules: 100 % at tick: `fill == t` (cells `0..t−1` filled, tick drawn). `p > 100` ⇒ cells right of the tick use `bar_over` — over-amplification is visible by glyph (`═`) **and** color. When `NodeAudio` is `None` the bar area is `G` cells of `─` with `--` centered, all `dim`. Muted rows draw `bar_fill`/`bar_over` as `bar_muted` (color) but keep glyphs, and the whole row gets `Modifier::DIM` except `[M]`.

Text columns: `pct` cell = `" "` + `format!("{:>3}%", p)` (`" 72%"`, `"150%"`); values above 999 print `999%`. `dB` cell = `" "` + `fmt_db(g)` right-aligned in 8 cells (`-18.1 dB`, `+10.6 dB`, `-inf dB`); values that would not fit are clamped to `+99.9 dB`.

## 5. Peak meter

8 cells; glyph selection per VOL-MTR-003; empty cell `·` (U+00B7). Cell colors (color mode): cells 0–5 `meter_low`, cell 6 `meter_mid`, cell 7 `meter_high` (thresholds at −15/−7.5/−0 dBFS of the 60 dB scale ⇒ cell *c* covers `D ∈ [c/8, (c+1)/8)`; green ≤ 0.75 ≈ −15 dBFS, yellow < 0.875 ≈ −7.5 dBFS, red above). Decay/ballistics: 02 VOL-MTR-002. In mono mode only glyph height carries information.

## 6. Minimum size

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-VIS-001 | MUST | Minimum terminal size is **40 columns × 12 rows**. Below that (either dimension) the app draws only a centered two-line message, no tabs/rows: `Terminal too small` / `need 40x12, have <W>x<H>`; input is restricted per VOL-KEY-010; backend keeps running; meters are stopped (`SetMeters{targets: []}`). | Missing |
| VOL-VIS-002 | MUST | If the terminal is narrower than the message, the message is truncated to `W` columns, never wrapped, never panics. Zero-size areas never panic (all widgets early-return on empty `Rect`). | Missing |

```
                                  
                                  
                                  
                                  
        Terminal too small        
      need 40x12, have 34x9       
                                  
                                  
                                  
```

At 12 rows the body is 9 lines (3 locked rows).

## 7. Empty and disconnected states

| State | Body content |
|---|---|
| Disconnected | one centered line `○ PipeWire not available — retrying in <n> s (attempt <k>)` (`NoDaemon`), `○ Connection to PipeWire lost — retrying in <n> s` (`ConnectionLost`), `○ Permission denied connecting to PipeWire` (`PermissionDenied`); the status line shows the socket path; tab bar and filter bar stay; countdown updates each second |
| Connecting (before first result) | `Connecting to PipeWire…` |
| Connected, no rows | tab-specific empty state text (VOL-PB-004, VOL-REC-005, VOL-OUT-008, VOL-IN-005, VOL-CFG-005) |

```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: Applications (f)                                                         
                                                                                
                                                                                
                                                                                
             ○ PipeWire not available — retrying in 4 s (attempt 3)             
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ○ disconnected │ /run/user/1000/pipewire-0                                     
```

Empty state, connected:
```
 Playback │ Recording │ Output Devices │ Input Devices │ Configuration          
 Show: Applications (f)                                              0 streams  
                                                                                
                                                                                
                                                                                
                                                                                
                              No playback streams                               
                   Applications that play audio appear here.                    
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
 ● PipeWire 1.6.9 │                      out: Starship/Matisse… │ ? help        
```

## 8. Style roles

`Theme::new(color: bool)` returns `Style`s by role. Named ANSI colors only (inherit the user's terminal palette; no RGB/indexed).

| Role | Color mode | Mono mode (`NO_COLOR`) |
|---|---|---|
| `tab_selected` | BOLD, fg Yellow (current) | BOLD + REVERSED |
| `tab_normal` | default | default |
| `sel_bar` (`▌`) | fg Cyan, BOLD | BOLD |
| `title_selected` | BOLD | BOLD |
| `title_idle` (corked/suspended stream) | DIM | DIM |
| `bar_fill` | fg Green | default |
| `bar_over` (> 100 %) | fg Red | default (glyph `═` distinguishes) |
| `bar_empty` | fg DarkGray | DIM |
| `bar_muted` | fg DarkGray | DIM |
| `tick` | BOLD | BOLD |
| `mute_on` (`[M]`) | fg Red, BOLD | BOLD |
| `mute_off` (`[ ]`) | DIM | DIM |
| `pct_over` (text of > 100 %) | fg Red | BOLD |
| `default_star` (`★`) | fg Yellow | default |
| `state_running` (`●`) / `state_idle` (`○`) | fg Green / DIM | default / DIM |
| `meter_low/mid/high/empty` | Green / Yellow / Red / DarkGray | default / default / default / DIM |
| `popup_border` | fg Cyan | default |
| `popup_sel` | REVERSED | REVERSED |
| `popup_check` (`✓`) | fg Green | BOLD |
| `status_ok` (`●`) / `status_bad` (`○`) | fg Green / fg Red | default |
| `msg_error` | fg Red | BOLD, text prefixed `! ` |
| `hint` (status hint, empty-state hint) | DIM | DIM |

Muted row: all text roles except `mute_on` gain DIM (in addition to the table).

## 9. `NO_COLOR`

Per <https://no-color.org>: if environment variable `NO_COLOR` is **set and non-empty**, or `--no-color` is passed, `Theme::new(false)` is used: **no** `Style::fg/bg/underline_color` is applied anywhere (only `Modifier`s). Information never relies on color alone: mute (`[M]`), default (`★`), running (`●`/`○`), over-amplification (`═`), selection (`▌` + BOLD), connection (`●`/`○`), errors (`! ` prefix) all have glyph/modifier redundancy. `TERM=dumb` is not special-cased.

## 10. Glyph inventory (all must be single-cell in `unicode-width`)

`▌ ● ○ ★ ▾ ▸ ◂ ━ ═ ─ ┃ │ · █ ▉ ▊ ▋ ▌ ▍ ▎ ▏ → ← … ✓ ▶ ┌ ┐ └ ┘`. No ASCII fallback in v1 (Open question Q9). Box/block glyphs require a UTF-8 locale and a font with box-drawing and block-elements ranges.

## 11. Requirements

| ID | Level | Requirement | Status |
|---|---|---|---|
| VOL-VIS-003 | MUST | Screen structure per §1. | Stub (tab bar row + one paragraph) |
| VOL-VIS-004 | MUST | Tab bar per §2; selected tab bold yellow in color mode. | Stub (bold yellow implemented; `│` divider and overflow handling missing) |
| VOL-VIS-005 | MUST | `VolumeBar` per §4 including 100 % tick, over-amp glyph/color, muted and unavailable variants. | Missing (prototype used `LineGauge`, uncompiled) |
| VOL-VIS-006 | MUST | `PeakMeter` per §5 and VOL-MTR-003. | Missing |
| VOL-VIS-007 | MUST | Theme roles per §8; `NO_COLOR`/`--no-color` per §9, covered by a test that renders every tab in mono mode and asserts that no cell has `fg`, `bg` ≠ `Color::Reset`. | Missing |
| VOL-VIS-008 | MUST | Disconnected/empty states per §7. | Missing |
| VOL-VIS-009 | MUST | Rendering is a pure function of `(&App, area, &mut Buffer)`; no widget reads the clock (time is injected via `UiState.now`) so snapshots are deterministic. | Missing |
| VOL-VIS-010 | SHOULD | No cell outside the area handed to a widget is written (debug-asserted in tests via a guard buffer). | Missing |
