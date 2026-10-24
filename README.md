# vol

A keyboard-driven terminal volume mixer for PipeWire: per-stream, device and route control in a [Ratatui] TUI, with the information layout of pavucontrol.

> **Status: early development.** The repository currently contains only a five-tab skeleton (Playback, Recording, Output Devices, Input Devices, Configuration). There is no PipeWire integration yet, so no volume, mute, default-device, move, port or profile control works. See [ROADMAP.md](ROADMAP.md) for the current state. The feature list below is the plan, not a description of what runs today.

## Planned features

Scope is defined in [SPEC.md §3](SPEC.md#3-scope-v1).

- Five tabs: Playback (application streams), Recording (capture streams), Output Devices (sinks), Input Devices (sources), Configuration (cards and profiles).
- Per-node volume from 0 to 150 %, per-channel volume with a channel lock, and mute.
- Set the default sink and source.
- Move playing and recording streams to another sink or source.
- Select device ports (routes) and card profiles.
- Peak meters per row (passive; no device wake-ups).
- Filters per tab, a help overlay, and a status line.
- Reconnect when the PipeWire daemon restarts.
- Keyboard control (mouse optional); respects `NO_COLOR`.

Not planned for v1: PulseAudio protocol or PulseAudio-only servers, `pipewire.conf` or WirePlumber configuration, module loading, EQ or effects, Bluetooth pairing, audio playback or recording, persistence of its own, i18n, macOS and Windows. The full non-goal list is in [SPEC.md §3.1](SPEC.md#31-non-goals).

## Requirements

- Linux (x86_64 or aarch64). macOS and Windows are not supported.
- Runtime: a running PipeWire daemon with its native socket, and WirePlumber for default devices, stream moves and profile persistence.
- Build: Rust 1.88 or newer (edition 2024), plus PipeWire and libclang development files:

| Distribution | Build packages | Runtime packages |
|---|---|---|
| Debian / Ubuntu | `build-essential pkg-config libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev` | `libpipewire-0.3-0 pipewire wireplumber` |
| Fedora | `pipewire-devel clang-devel pkgconf-pkg-config` | `pipewire wireplumber` |
| Arch | `pipewire clang pkgconf` (+ `base-devel`) | `pipewire wireplumber` |

For Nix, see [CONTRIBUTING.md](CONTRIBUTING.md#nix).

## Build and run

```sh
cargo run
```

Build a release binary with `cargo build --release`. The binary is `target/release/vol`.

## Keys

Keys are from [docs/specs/07-input-and-keybindings.md](docs/specs/07-input-and-keybindings.md). Entries marked *planned* are not implemented yet.

| Key | Action | Status |
|---|---|---|
| `q`, `Ctrl-C` | Quit | Implemented |
| `F1`–`F5` | Select tab | Implemented |
| `Tab`, `]` / `Shift-Tab`, `[` | Next / previous tab (wraps) | planned |
| `Alt-1`–`Alt-5` | Select tab (optional) | planned |
| `j` / `k`, `↓` / `↑` | Next / previous row (clamped) | planned |
| `g` / `G` | First / last row | planned |
| `h` / `l`, `←` / `→` | Volume −5 / +5 % | planned |
| `H` / `L`, `-` / `+` | Volume −1 / +1 % (fine) | planned |
| `PgDn` / `PgUp` | Volume −10 / +10 % | planned |
| `1`–`9` | Volume 10–90 % | planned |
| `0` | Volume 100 % | planned |
| `Home` / `End` | Volume 0 % / 150 % | planned |
| `m`, `Space` | Toggle mute | planned |
| `c` | Lock / unlock channels | planned |
| `,` / `.` | Previous / next channel (when unlocked) | planned |
| `d` | Set default (Output and Input tabs) | planned |
| `p`, `Enter` | Open picker (move, port or profile) | planned |
| `f` / `F` | Next / previous filter | planned |
| `?` | Help overlay | planned |
| `Esc` | Close overlay; never quits | planned (currently quits) |

In a popup: `j`/`k` or arrows move, `g`/`G` or `Home`/`End` jump, `1`–`9` select, `Enter`, `l`, `→` or `Space` apply, and `Esc`, `q`, `h` or `←` close.

Planned command-line flags: `--mouse` (opt-in mouse support), `--no-color`, `--no-meters`. Planned environment: `NO_COLOR`, `VOL_LOG`, `PIPEWIRE_REMOTE`.

## Documentation

| Document | Contents |
|---|---|
| [SPEC.md](SPEC.md) | Scope, terminology, conventions, requirement index, open questions |
| [docs/specs/](docs/specs/) | Normative requirements per area (`VOL-<AREA>-<NNN>`) |
| [ROADMAP.md](ROADMAP.md) | What is implemented and what is next |
| [PLAN.md](PLAN.md) | Implementation plan |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Development setup, checks, clean-room rule, release process |
| [CHANGELOG.md](CHANGELOG.md) | Notable changes per version |

## Development

Verification runs locally. GitHub Actions only builds tagged releases.

- `just check` runs the full gate: formatting, clippy (warnings are errors), tests, docs, `cargo deny`, and MSRV 1.88 type-check.
- `scripts/install-hooks.sh` points git at `scripts/hooks`, so `git push` runs `just check` first. Skip once with `git push --no-verify`.

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup details.

## Clean-room statement

vol is a clean-room reimplementation of the observable behavior of pavucontrol. Behavior comes from its user interface, PipeWire, SPA and WirePlumber documentation and headers, and the output of PipeWire command-line tools used as black boxes. No pavucontrol source code was read, fetched, copied or decompiled. The rule and its sources are in [SPEC.md §2](SPEC.md#2-conventions) (VOL-GEN-005) and [§6](SPEC.md#6-sources).

## License

Copyright (c) Michał Czyż <mike@c2yz.com>

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[Ratatui]: https://ratatui.rs
[LICENSE]: ./LICENSE
