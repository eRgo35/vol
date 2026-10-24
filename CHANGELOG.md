# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Specification: `SPEC.md` and `docs/specs/` (requirement IDs `VOL-<AREA>-<NNN>`, status per requirement, VERIFY index).
- Local quality gate: `justfile` with `just check` (fmt, clippy, test, doc, cargo-deny, MSRV 1.85) and `just release-check`.
- `scripts/install-hooks.sh` and a `pre-push` hook that runs `just check`.
- `deny.toml` for cargo-deny (permissive licenses only).
- Release workflow `.github/workflows/release.yml`: builds an x86_64 Linux tarball and `SHA256SUMS` on `vX.Y.Z` tags.
- Issue and pull request templates in `.github/`.
- `CONTRIBUTING.md` with setup, checks, clean-room rule, spec-driven workflow and release process.

### Changed

- `README.md` replaces the template text with project documentation, planned features, requirements, keys and the clean-room statement.
- `.github/dependabot.yml` updated.

### Removed

- `.github/workflows/ci.yml`. Verification runs locally through `just check`; GitHub Actions only builds tagged releases.

[Unreleased]: https://github.com/eRgo35/vol/commits/main
