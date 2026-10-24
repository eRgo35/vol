# M9 — Packaging and release v0.1.0

**Status:** Not started · **Size:** S · **Depends on:** M8 · **Branch:** `m9/release` · Index: [`PLAN.md`](../../PLAN.md) · Roadmap: [`ROADMAP.md`](../../ROADMAP.md#m9--packaging-and-release-v010)

**Goal.** README, Cargo metadata, version bump, VERIFY closure, local release gate and the `v0.1.0` tag that triggers `release.yml`.

## Requirement IDs assigned to this milestone (5)

`VOL-PKG-001`, `VOL-PKG-002`, `VOL-PKG-003`, `VOL-QA-008`, `VOL-PLAT-003`

Documentation, metadata, the last verification sweep and the tag that triggers the only GitHub Actions workflow. Everything up to the tag push is local.

## Pre-flight

`git switch -c m9/release`. Prerequisites: M0–M8 `Done`; `main` green; a second environment for the compatibility check (Debian 12 container or VM with PipeWire 0.3.65 + WirePlumber 0.4.x; local `podman`/VM — **not** CI).

## Tasks

### M9-T01 README
* **Do:** revise the existing README (written by the tooling task; status banner, planned-features section, key table with a *Status* column) into the final one — remove the "early development" banner and flip every *planned* marker that is now Implemented, and ensure it covers: what vol is (clean-room TUI for PipeWire; relation to pavucontrol stated neutrally), screenshot-free ASCII example (copy the 80×24 mockup from a snapshot), install (release tarball + `SHA256SUMS`, building from source with the dependency table), requirements (PipeWire, WirePlumber), usage (tabs, key table from 07, flags `--mouse`, `--no-color`, `--no-meters`, env `NO_COLOR`, `VOL_LOG`, `VOL_NO_METERS`, `PIPEWIRE_REMOTE`), behavior notes (meters create capture streams named `vol.meter.*`; microphone indicator; passive meters), limitations (deferred list), development (`just check`, hooks, live tests, spec/plan links), license.
* **Tests first:** `tests/readme.rs::readme_documents_every_cli_flag` (reads `cli::USAGE`, asserts each flag appears in README), `::readme_key_table_mentions_every_normal_mode_key` (derived from the keymap test table).
* **Reqs:** VOL-PKG-001, VOL-QA-008.

### M9-T02 Cargo metadata
* **Do:** `repository`, `readme`, `keywords`, `categories = ["command-line-utilities", "multimedia::audio"]`; keep `description`; `rust-version` present since M0; verify `cargo package --list` (no secrets/large files) and `cargo publish --dry-run` **only if** publishing to crates.io is wanted (not part of this plan).
* **Reqs:** VOL-PKG-003.

### M9-T03 VERIFY sweep and compatibility run
* **Do:** list remaining `VERIFY:` markers (`grep -rn "VERIFY" SPEC.md docs/specs`) — each must be closed (observed fact replaces marker; `SPEC.md` §7 row marked closed) or explicitly waived in §7 with a reason. Run the Debian 12 environment: build (`cargo build --locked`) and MA-02…MA-09 (VF-16); record the minimum PipeWire/WirePlumber versions in VOL-PLAT-003 and README.
* **Reqs:** VOL-PLAT-003.

### M9-T03b Nix dev shell verification
* **Do:** on a machine with Nix: `nix develop -c cargo build --locked` and `nix develop -c cargo test --locked --all-features`. The flake has no rustup, so `just msrv` (`cargo +1.88`) cannot run inside it: either pin a 1.88 toolchain in the flake or document `just check` minus `msrv` for Nix users. Moved here from M0-T05 (no Nix on the author machine).
* **Reqs:** VOL-PKG-002.

### M9-T04 Full acceptance run
* **Do:** run MA-01…MA-13 on the author's session (PipeWire 1.6.9) and the Debian environment where applicable; log results in the "Acceptance log" below (date, PipeWire/WirePlumber versions, pass/fail, notes). Failures become fix tasks before the tag. MA-14 = this log shows every VERIFY closed.

### M9-T05 Release gate and tag
* **Do:** `just release-check` (= `just check` + `cargo build --release --locked`); bump `version = "0.1.0"` in `Cargo.toml`, run `cargo build` to update `Cargo.lock`, `cargo build --locked` passes; in `CHANGELOG.md` rename `## [Unreleased]` to `## [0.1.0] - <date>` and add a new empty `## [Unreleased]`; commit `chore(release): v0.1.0`; merge ff-only to `main`; `git tag v0.1.0` (as in `CONTRIBUTING.md` §Release process); `git push origin main v0.1.0` (pre-push hook runs `just check` again — expected). `release.yml` (the only workflow) checks tag == `Cargo.toml` version, builds on ubuntu-latest, uploads `vol-v0.1.0-x86_64-linux.tar.gz` and `SHA256SUMS` to the GitHub Release.
* **After the push:** download the artifact, `sha256sum -c SHA256SUMS`, run `./vol --version` and one smoke session. If the workflow fails: delete the tag locally and remotely (`git push --delete origin v0.1.0`), fix, re-tag — release automation failures never justify skipping local verification.

### M9-T06 Post-release housekeeping
* **Do:** SPEC counts table; ROADMAP status `Done` for M0–M9 and a "Released" section (date, version); open `docs/plans/10-deferred.md` as the live backlog; decide whether to start `1.0` criteria tracking (ROADMAP release policy).

## Acceptance log (fill in during M9-T04)

| MA | Date | Environment (PipeWire / WirePlumber / terminal) | Result | Notes |
|---|---|---|---|---|
| MA-01 | | | | |
| MA-02 | | | | |
| MA-03 | | | | |
| MA-04 | | | | |
| MA-05 | | | | |
| MA-06 | | | | |
| MA-07 | | | | |
| MA-08 | | | | |
| MA-09 | | | | |
| MA-10 | | | | |
| MA-11 | | | | |
| MA-12 | | | | |
| MA-13 | | | | |
| MA-14 | | | | |

## Requirement → task map

Every ID of this milestone is delivered by at least one task (first listed = main owner).

| ID | Task(s) |
|---|---|
| VOL-PKG-001 | M9-T01 |
| VOL-PKG-002 | M9-T03b |
| VOL-PKG-003 | M9-T02 |
| VOL-QA-008 | M9-T01 |
| VOL-PLAT-003 | M9-T03 |

## Definition of done (M9)

- [ ] `just release-check` green.
- [ ] Tag `v0.1.0` pushed; release artifact verified (checksum + smoke run).
- [ ] All 5 M9 IDs `Implemented`; no open `VERIFY` markers without a recorded waiver.
- [ ] Acceptance log complete.

## Risks

| Risk | Mitigation |
|---|---|
| Release workflow fails on the tag (missing deps, version mismatch) | tag/`Cargo.toml` equality checked locally before tagging (`cargo metadata`); `release.yml` already installs libpipewire/libclang; delete/re-tag procedure above. |
| Behavior differs on older PipeWire (Debian 12) | T03; record minimum versions or raise the floor in README. |
| Release tarball built on ubuntu-latest links a newer libpipewire than users have | documented in README (dynamic libpipewire-0.3 ABI is stable; minimum runtime version noted). |
