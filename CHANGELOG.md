# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Specification: `SPEC.md` and `docs/specs/` (requirement IDs `VOL-<AREA>-<NNN>`, status per requirement, VERIFY index).
- Local quality gate: `justfile` with `just check` (fmt, clippy, test, doc, cargo-deny, MSRV 1.88) and `just release-check`.
- `scripts/install-hooks.sh` and a `pre-push` hook that runs `just check`.
- `deny.toml` for cargo-deny (permissive licenses only).
- Release workflow `.github/workflows/release.yml`: builds an x86_64 Linux tarball and `SHA256SUMS` on `vX.Y.Z` tags.
- Issue and pull request templates in `.github/`.
- `CONTRIBUTING.md` with setup, checks, clean-room rule, spec-driven workflow and release process.
- Command-line flags: `-h`/`--help`, `-V`/`--version`, `--no-color`, and (accepted, not yet active) `--mouse`, `--no-meters`; an unknown argument exits with status 2 before the terminal is touched.
- Monochrome theme with `--no-color` or `NO_COLOR`.
- Tab switching with `Tab`/`Shift-Tab`/`[`/`]` (wrapping) and `F1`–`F5`; the tab bar scrolls and shortens titles on narrow terminals.
- "Terminal too small" screen below 40×12.
- `SIGTERM`, `SIGHUP` and `SIGINT` quit cleanly and restore the terminal.
- Nix dev shell with the PipeWire and libclang build dependencies.
- PipeWire backend: vol connects at startup and mirrors audio nodes, devices, ports, links and the default sink/source (not shown on screen yet); it keeps running without a daemon and reconnects with backoff (0.5 s up to 10 s).
- Optional file log: `VOL_LOG=error|warn|info|debug` writes `$XDG_STATE_HOME/vol/vol.log`.
- `just test-live`: opt-in tests against a private, throwaway PipeWire daemon.

### Changed

- `README.md` replaces the template text with project documentation, planned features, requirements, keys and the clean-room statement.
- `.github/dependabot.yml` updated.
- `Esc` no longer quits; it closes overlays or clears the status message.
- The screen is redrawn only when something changed.
- Dependencies bumped to latest (ratatui 0.30, crossterm 0.29, pipewire 0.10.1, tokio 1.54, strum 0.28); MSRV raised to 1.88.

### Removed

- `.github/workflows/ci.yml`. Verification runs locally through `just check`; GitHub Actions only builds tagged releases.
- The template's counter demo (`←`/`→` keys).

[Unreleased]: https://github.com/eRgo35/vol/commits/main
