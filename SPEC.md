# vol — Specification

`vol` is a terminal UI for controlling audio volume, devices and streams on **PipeWire**. It is a **clean-room** reimplementation of the *observable behavior* of pavucontrol (PulseAudio Volume Control): same five-tab information architecture and per-row controls, rebuilt for a keyboard-driven TUI and PipeWire's object model.

This document is the index, the conventions, the glossary and the open questions. Normative detail is in `docs/specs/`. Status values describe the repository at commit `ce3e5a3` (5 stub tabs, no PipeWire code).

## 1. Document map

| Doc | Area | ID prefixes | Implemented | Stub | Missing | n/a | Total |
|---|---|---|---|---|---|---|---|
| [`01-architecture.md`](docs/specs/01-architecture.md) | Architecture | VOL-ARCH | 2 | 4 | 43 | 0 | 49 |
| [`02-playback.md`](docs/specs/02-playback.md) | Shared model + Playback | VOL-VM, VOL-ROW, VOL-MTR, VOL-FLT, VOL-PB | 2 | 1 | 39 | 0 | 42 |
| [`03-recording.md`](docs/specs/03-recording.md) | Recording | VOL-REC | 0 | 1 | 7 | 0 | 8 |
| [`04-output-devices.md`](docs/specs/04-output-devices.md) | Output Devices | VOL-OUT | 0 | 1 | 11 | 1 | 13 |
| [`05-input-devices.md`](docs/specs/05-input-devices.md) | Input Devices | VOL-IN | 0 | 1 | 8 | 0 | 9 |
| [`06-configuration.md`](docs/specs/06-configuration.md) | Configuration | VOL-CFG | 0 | 1 | 8 | 0 | 9 |
| [`07-input-and-keybindings.md`](docs/specs/07-input-and-keybindings.md) | Input, keys, status line | VOL-KEY | 2 | 1 | 11 | 0 | 14 |
| [`08-visual-design.md`](docs/specs/08-visual-design.md) | Visual design | VOL-VIS | 0 | 2 | 8 | 0 | 10 |
| [`09-testing-and-quality.md`](docs/specs/09-testing-and-quality.md) | Testing, QA, platforms, packaging | VOL-TEST, VOL-QA, VOL-PLAT, VOL-PKG | 2 | 5 | 30 | 0 | 37 |
| | **Total** | | 8 | 17 | 165 | 1 | 191 |

Counts exclude the 8 `VOL-GEN-*` rows of §2 (all `n/a`).

Planning: [`ROADMAP.md`](ROADMAP.md) (milestones, decision log), [`PLAN.md`](PLAN.md) (execution conventions), `docs/plans/` (per-milestone tasks, [coverage map](docs/plans/coverage.md)).

Reading order: this file → [01 architecture](docs/specs/01-architecture.md) → [02 shared row model + Playback](docs/specs/02-playback.md) → 03–06 (tabs) → [07 keys](docs/specs/07-input-and-keybindings.md) → [08 visuals](docs/specs/08-visual-design.md) → [09 testing/packaging](docs/specs/09-testing-and-quality.md).

## 2. Conventions

| ID | Level | Convention | Status |
|---|---|---|---|
| VOL-GEN-001 | MUST | The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, RECOMMENDED, MAY, OPTIONAL are interpreted as in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) / [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174) when in upper case. The repository-local shorthands `NEVER` = MUST NOT and `AVOID` = SHOULD NOT are not used in these specs. | n/a |
| VOL-GEN-002 | MUST | Every requirement has a stable ID `VOL-<AREA>-<NNN>` (three digits), unique across the repository, never renumbered or reused; a removed requirement keeps its row with status `n/a` and the text `Withdrawn: <reason>`. | n/a |
| VOL-GEN-003 | MUST | Requirements live in tables with columns `ID | Level | Requirement | Status`. The status is evaluated against the repository at the commit named above and is one of: **Implemented** (behavior present and matches), **Stub** (code exists but is placeholder/partial; parenthesis says what is missing), **Missing** (nothing), **n/a** (informational/withdrawn/dropped). Implementers update the status in the same change that implements. | n/a |
| VOL-GEN-004 | MUST | Anything not directly observed or documented is marked `VERIFY:` (with the procedure to verify) or `[INFERENCE]`. `VERIFY` items are indexed in §7. Resolving one means editing the spec text and recording the result in the PR. | n/a |
| VOL-GEN-005 | MUST | Clean-room rule: contributors MUST NOT read, fetch, grep or copy pavucontrol source code (any repository, distro source package, or decompilation). Permitted inputs: pavucontrol's observable UI behavior, PipeWire/WirePlumber/SPA documentation and headers, `libpipewire` and `pipewire-rs` APIs, and the CLI behavior of `pw-cli`, `pw-dump`, `pw-metadata`, `pw-link`, `wpctl`, `pactl` treated as black boxes. Sources consulted are listed in §6. | n/a |
| VOL-GEN-006 | MUST | Signatures in specs are Rust *shapes*, not implementations; field/variant names are normative, exact derives/visibility are not. Exact strings in quotes or code spans (labels, status messages, property keys) are normative. | n/a |
| VOL-GEN-007 | SHOULD | Mockups are generated, 80-column-exact, and match snapshot tests (VOL-TEST-030). Where a mockup and text disagree, the text wins and the mockup is a bug. | n/a |
| VOL-GEN-008 | MUST | **Verification is local.** Code is verified by `just check` (fmt, clippy `-D warnings`, test, doc, deny, msrv) and the `scripts/hooks/pre-push` hook. GitHub Actions is used only for tag-triggered releases (`.github/workflows/release.yml`). No spec, plan or milestone may require CI-based verification. | n/a |

## 3. Scope (v1)

In scope: the five tabs (Playback, Recording, Output Devices, Input Devices, Configuration) with live registry updates; per-node volume (0–150 %), per-channel volume with lock, mute; default sink/source; moving streams; port (route) and card profile selection; peak meters; filters; help overlay; reconnect to a restarting daemon; keyboard control (mouse optional); `NO_COLOR`.

### 3.1 Non-goals

* No PulseAudio client/protocol, no `libpulse`; no support when only a PulseAudio server (no PipeWire native socket) is present.
* No sound server configuration (editing `pipewire.conf`, WirePlumber scripts/rules, clock rate/quantum), no module loading (`module-tunnel`, RTP, combine/loopback sink creation), no EQ/effects.
* No Bluetooth pairing/management; no Bluetooth codec selection in v1.
* No audio recording/playback functionality; vol only *captures* for meters.
* No video/MIDI nodes; no JACK-specific UI; no patchbay/graph editor (that is a different tool).
* No persistence layer of its own (per-application volume/device memory is WirePlumber's job); no config file in v1.
* No i18n (English UI strings only); no ASCII-only fallback in v1; no macOS/Windows.
* No `Ctrl-Z` suspend; no text-input widgets (no renaming).

### 3.2 Deferred (specified-as-intended, not in v1)

| Feature | Where |
|---|---|
| Latency offset ("Advanced") | VOL-OUT-012 |
| Terminate stream | VOL-PB-010 |
| Move capture stream to a sink monitor; "Monitor of …" input entries | 03 §5, VOL-IN-008 |
| Configuration "Show: All Devices" filter; Bluetooth codec | VOL-CFG-009 |
| Mouse | VOL-KEY-012 |
| Active (device-waking) meters, pavucontrol-style | Q6 |

### 3.3 Dropped from pavucontrol

System-sounds/event-role row; icons; stream-restore UI; digital pass-through/encodings; base-volume markers; port availability groups; the dialogs/menus (replaced by popups).

## 4. Terminology mapping (pavucontrol/PulseAudio → vol/PipeWire)

| PulseAudio / pavucontrol | PipeWire / vol | Notes |
|---|---|---|
| sink | node with `media.class` `Audio/Sink` (prefix match, so `Audio/Sink/Virtual` too) | Output Devices tab |
| source | node with `media.class` `Audio/Source` (prefix match) | Input Devices tab; **no** implicit monitor nodes |
| monitor source ("Monitor of X") | output ports with `port.monitor = true` on the sink node | not a node; used for meters and recording-stream labels |
| sink-input (playback stream) | node with `media.class = Stream/Output/Audio` | Playback tab |
| source-output (recording stream) | node with `media.class = Stream/Input/Audio` | Recording tab |
| card | Device object, `media.class = Audio/Device` | Configuration tab |
| card profile | Device `Profile` / `EnumProfile` params | `Profile.index`, `save` |
| port | Device route: `Route` / `EnumRoute` params (`index`, `device`, `direction`, `available`) | linked to a node by `card.profile.device ∈ route.devices` |
| port availability yes/no/unknown | `SPA_PARAM_AVAILABILITY_{yes,no,unknown}` | "no" → "(unplugged)" |
| default sink / source ("Set as fallback") | metadata `default`: `default.audio.sink`/`default.audio.source` (effective), `default.configured.audio.*` (user choice) | subject 0, JSON `{"name": node.name}` |
| move sink-input / source-output | metadata `default`, subject = stream node id, keys `target.object` (+ `target.node`) | WirePlumber re-links; VERIFY Q4 |
| volume (cubic-root scale, 0–65536, 100 % = 65536) | `Props.channelVolumes[i]·Props.volume` = `ℓ³`, UI percent = `100·∛g` | 02 §2.1 |
| mute | `Props.mute` | |
| channel map / "lock channels" | `Props.channelMap` / UI-only lock | |
| peak-detect record stream | `pw_stream` capture, `node.passive`, `stream.monitor`, `stream.capture.sink` | 02 §2.4 |
| port latency offset | `Route.props.latencyOffsetNsec` / node `ProcessLatency` | deferred |
| `application.name`, `media.name`, `application.icon_name`, `media.role` | same keys in node props (`application.icon-name` in native PipeWire; pipewire-pulse streams may use either spelling) | icon not rendered |
| device description (`device.description`), `node.description`, `node.nick` | same keys | title chains per tab |
| virtual stream/device | `media.role ∈ {abstract, filter}` / `node.virtual = true` / no `device.id` / `node.link-group` | heuristic; 02 §3.1, 04 §2 |
| stream-restore / device-manager modules | WirePlumber state (`default-routes`, `default-profile`, `route-settings` metadata objects observed) | not touched by vol |

## 5. Glossary

| Term | Meaning in this project |
|---|---|
| **Object / global** | Anything in the PipeWire registry: Core, Client, Device, Node, Port, Link, Factory, Module, Metadata. Has a numeric **id** (reused) and a **serial** (`object.serial`, never reused). |
| **Node** | A media processing element (sink, source, stream, filter, driver). Has properties (`info.props`), a state, ports, and params (`Props`, `ProcessLatency`, …). |
| **Port** | An endpoint of a node (`port.direction` in/out, `audio.channel`, `port.monitor`). |
| **Link** | Connection from an output port to an input port. |
| **Device** | Hardware/software device object (e.g. an ALSA card) that creates nodes; has `EnumProfile`/`Profile`/`EnumRoute`/`Route` params. = pavucontrol "card". |
| **Stream** | A node created by an application via `pw_stream`/pipewire-pulse (`Stream/Output/Audio`, `Stream/Input/Audio`). |
| **Sink / Source** | Node that consumes (`Audio/Sink`) / produces (`Audio/Source`) audio on behalf of a device or virtual endpoint. |
| **Monitor** | The set of output ports of a sink that carry what the sink plays. |
| **Route** | A device-level destination (a "port" to the user): e.g. `analog-output-headphones`. Described by `EnumRoute`; the active one(s) by `Route`. |
| **Profile** | A device configuration (set of nodes it exposes), e.g. `output:analog-stereo+input:analog-stereo`; `off` disables. |
| **Param / Pod** | Typed parameter objects (`Props`, `Route`, …) serialized as SPA PODs, read by `enum_params`/`subscribe_params`, written by `set_param`. |
| **Props** | Node parameter object: `volume`, `mute`, `channelVolumes`, `channelMap`, `softVolumes`, … |
| **Session manager (WirePlumber)** | Policy daemon: links streams to devices, tracks defaults, restores routes/profiles, handles `target.*` metadata. |
| **Metadata** | A key/value store object (name `default` is the relevant one): `(subject id, key, type, value)`. |
| **Level / percent / gain** | `g` linear amplitude; `ℓ = ∛g`; percent = `100·ℓ`. See 02 §2.1. |
| **Row** | One list entry of a tab (stream, device or card) rendered per 02 §2.3. |
| **RowKey** | Stable identity of a row across updates (`Serial` or name). |
| **Reducer** | `Store::apply(StoreEvent)`; the only mutation path of PipeWire-derived state. |
| **Coalescing** | Merging redundant backend events within a 16 ms window before delivery. |

## 6. Sources

No pavucontrol source code was read, fetched, grepped or cloned (VOL-GEN-005). pavucontrol UI behavior (tab names, per-row controls, "Show:" filter names, 0–150 % slider with 100 % marker, "Set as fallback", port/profile combos, per-row level meters, lock-channels toggle, "Advanced" latency offset) is taken from the author's recollection of the application as a user; items where that recollection is uncertain are flagged in Open questions.

Consulted:

1. Local PipeWire/SPA headers, PipeWire 1.6.9 (read-only): `/usr/include/spa-0.2/spa/param/{props,route,profile,param,latency,audio/raw}.h` — property/param ids, `SPA_PARAM_AVAILABILITY_*`, channel ids.
2. Black-box observation, read-only, on the author's live PipeWire 1.6.9 + WirePlumber session: `pw-dump` (node/device/metadata property names and values, `Props`, `Route`, `EnumRoute`, `EnumProfile`, `ProcessLatency` shapes; metadata `default`, `route-settings`, `default-profile`), `wpctl status`, `wpctl get-volume`, `pactl get-sink-volume`, `pactl get-source-volume` (volume cross-check in 02 §2.1). No state was modified.
3. `pipewire-rs` published documentation, <https://pipewire.pages.freedesktop.org/pipewire-rs/pipewire/> (crate index, `channel`, `metadata`, `device` pages) — API surface and threading guidance. The docs.rs build for `pipewire 0.8.0` failed, so 0.8.0 specifics are VERIFY items (VOL-ARCH-016).
4. `Cargo.lock` of this repository (pinned versions); repository files listed in 01 §1.
5. Conventions: RFC 2119/8174, <https://no-color.org> (recalled, not re-fetched).
6. Not consulted although referenced by the crate docs: <https://docs.pipewire.org> (property/param semantics were taken from the headers and live observation instead; Open question Q4 lists what that leaves unverified).

## 7. VERIFY index

| # | Item (where) | Procedure summary |
|---|---|---|
| VF-01 | `pipewire 0.10.1` API surface & loop type names (VOL-ARCH-015/016) | `cargo doc` locally; compile a 30-line spike using every API in VOL-ARCH-016 |
| VF-02 | Core `error` event semantic on daemon death (VOL-ARCH-042) | kill `pipewire`; log `(id, seq, res, message)` |
| VF-03 | `Props.volume ≠ 1.0` multiplicative (VOL-VM-001) | `pw-cli set-param … Props '{ volume: 0.5 }'` on a test node |
| VF-04 | Pod encoding of `channelVolumes`/`mute` writes (VOL-VM-011) | serialize, compare with `pw-cli` equivalent on a null sink |
| VF-05 | ACP hardware volume echo / no double attenuation (VOL-VM-012) | set via vol, read device `Route` param |
| VF-06 | Meter capture of a `Stream/Output/Audio` node (02 §2.4, Q5) | start vol meter; `pw-link -l` |
| VF-07 | `node.passive` meters don't wake suspended nodes (02 §2.4, Q6) | observe `wpctl status`/`pw-cli ls Node` state |
| VF-08 | Virtual-stream heuristic (02 §3.1) | `pw-loopback`, filter-chain |
| VF-09 | Stream move metadata keys/types (02 §3.4, 03, Q4) | `pw-metadata -n default <id> target.object <serial> Spa:Id` + `pw-link -l` |
| VF-10 | Capture-stream → sink monitor by metadata (03 §5) | as VF-09 with a sink serial |
| VF-11 | Route switch by `Route` param with `save` (04 §3.2) | `pw-cli set-param <dev> Route '{ index: i, device: N, save: true }'` |
| VF-12 | `wpctl set-default` metadata equivalence (04 §3.3) | `pw-metadata -n default` before/after |
| VF-13 | Latency offset mechanism (VOL-OUT-012) | `Route.props.latencyOffsetNsec` vs node `ProcessLatency` |
| VF-14 | Profile switch by `Profile` param (06 §4) | `pw-cli set-param <dev> Profile '{ index: i, save: true }'` |
| VF-15 | MSRV 1.88 builds (VOL-QA-003) | `cargo +1.88 check --locked --all-targets` |
| VF-16 | Minimum PipeWire/WirePlumber versions (VOL-PLAT-003) | MA-02…MA-09 on Debian 12 |

## 8. Open questions

Every question carries a **Proposed default** so work can proceed; the defaults are provisional and stay open for the project owner to overrule. Overruling edits this table and the referenced requirement(s) in the same change. Planning impact per question: see `ROADMAP.md` §Decision log.

| # | Question | Proposed default |
|---|---|---|
| Q1 | Should `Esc` quit (current template and prototype behavior) or only close overlays? | Esc never quits (VOL-KEY-007); `q`/`Ctrl-C` quit. |
| Q2 | Digits `1`–`9`,`0` as absolute volume (this spec) vs. direct tab selection (`1`–`5`)? | Volume; tabs via `Tab`/`[`/`]`/`F1`–`F5` (+ optional `Alt-n`). |
| Q3 | Stay on `pipewire 0.8.0` (locked; Feb 2024) or upgrade to 0.9.x/0.10.x (loop types renamed to `MainLoopRc/Box`, `ContextRc/Box`; newer SPA bindings)? Any API gaps (VOL-ARCH-016) to be filled through `sys` crates? | Decided: upgrade to `pipewire 0.10.1` (0.8.0 / `libspa 0.8.0` no longer compile against PipeWire 1.6.9), 2026-10-24. VF-01 checks the 0.10.1 API surface; spec names are API-neutral. |
| Q4 | Which metadata keys/types does the installed WirePlumber actually honor for moving a stream (`target.object` only, `target.node` only, both; `Spa:Id` vs `Spa:String`)? Undocumented in the sources consulted. | Write both as `Spa:Id`; the second write removed if VF-09 shows it unnecessary. |
| Q5 | Can a capture `pw_stream` target a `Stream/Output/Audio` node to meter a single playback stream? If not, Playback rows have no meter. | Try; degrade to empty meter. |
| Q6 | Meters on sources: passive (privacy, no wake, empty when idle) vs. active (pavucontrol-like, wakes devices, mic-in-use indicator)? | Passive; `--no-meters` to disable. |
| Q7 | `Audio/Duplex` nodes and "Monitor of …" entries in Input Devices: show or not? | Not stored / deferred. |
| Q8 | Locked-channel scaling: balance-preserving (this spec) vs. set-all-equal? pavucontrol's exact behavior when channels differ is from memory only. | Balance-preserving (VOL-VM-008). |
| Q9 | ASCII fallback glyph set for non-UTF-8 terminals/fonts? | None in v1. |
| Q10 | Dependencies: `serde_json` (metadata JSON), `proptest`, `insta`, logging (`tracing` + `tracing-appender` vs. a hand-rolled file logger)? | Decided (dev-deps part): `proptest 1.12`, `insta 1.49` dev-only, 2026-10-24. Proposed: `serde_json` yes; hand-rolled logger, off by default. |
| Q11 | Master volume display for unequal channels: max (this spec) vs. mean? | Max (VOL-VM-002). |
| Q12 | Persist per-row UI state (lock) or filters across runs (needs a config/state file)? | No persistence. |
| Q13 | Which property best decides "Hardware" vs "Virtual" (`device.id`, `node.virtual`, `device.bus`, `factory.name`)? | `device.id` present and `node.virtual != true` (VOL-OUT §2). |
| Q14 | Support PipeWire builds without WirePlumber (e.g. `pipewire-media-session`)? | Works for volume/mute/ports/profiles; defaults/moves unavailable (VOL-ARCH-047). |
