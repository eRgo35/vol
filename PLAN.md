# vol — Execution plan

Index and working conventions for implementing [`SPEC.md`](SPEC.md). Scheduling and scope live in [`ROADMAP.md`](ROADMAP.md); this file says **how** to work; the per-milestone task lists are in `docs/plans/`.

## 1. Index

| Milestone | Plan | Goal (short) |
|---|---|---|
| M0 | [`docs/plans/00-foundation.md`](docs/plans/00-foundation.md) | Refactor to target architecture, mock backend, lints/MSRV, dev env |
| M1 | [`docs/plans/01-backend-store.md`](docs/plans/01-backend-store.md) | PipeWire thread, registry mirror, reducer, coalescing, reconnect |
| M2 | [`docs/plans/02-playback-readonly.md`](docs/plans/02-playback-readonly.md) | Playback tab, read-only |
| M3 | [`docs/plans/03-volume-mute.md`](docs/plans/03-volume-mute.md) | Volume/mute/lock writes |
| M4 | [`docs/plans/04-recording-move.md`](docs/plans/04-recording-move.md) | Recording tab, popups, move stream |
| M5 | [`docs/plans/05-output-input.md`](docs/plans/05-output-input.md) | Output/Input devices, defaults, ports |
| M6 | [`docs/plans/06-configuration.md`](docs/plans/06-configuration.md) | Configuration/profiles |
| M7 | [`docs/plans/07-meters.md`](docs/plans/07-meters.md) | Peak meters |
| M8 | [`docs/plans/08-polish.md`](docs/plans/08-polish.md) | Help, mouse, NO_COLOR, snapshots |
| M9 | [`docs/plans/09-release.md`](docs/plans/09-release.md) | Packaging, `v0.1.0` |
| M10 | [`docs/plans/10-deferred.md`](docs/plans/10-deferred.md) | Backlog |
| — | [`docs/plans/coverage.md`](docs/plans/coverage.md) | Requirement ID → milestone map (orphan/duplicate check) |

## 2. Verification is local

* The gate is **`just check`** = `fmt` → `clippy` (`-D warnings`, all targets/features, `--locked`) → `test` (`--locked --all-features --all-targets`) → `doc` (`RUSTDOCFLAGS=-D warnings`) → `deny` → `msrv` (`cargo +1.85 check`). Run it before every push.
* `scripts/install-hooks.sh` sets `core.hooksPath=scripts/hooks`; the `pre-push` hook runs `just check`. `git push --no-verify` is for emergencies only and MUST NOT be used to land a milestone (VOL-QA-009).
* GitHub Actions runs **only** for tags `v*.*.*` (`release.yml`). Plans never add CI jobs, status checks or "CI will verify" steps (VOL-GEN-008, VOL-QA-010). The PR template's checklist is the review-time substitute.
* Tests needing a PipeWire daemon are `#[ignore]`d and run explicitly: `VOL_TEST_PIPEWIRE=1 cargo test --test live -- --ignored`, against a **private** daemon or `support.null-audio-sink` nodes created by the test (VOL-QA-007). They are part of milestone acceptance (run by hand) but not of `just check`.
* Toolchain prerequisites on the dev machine: `just`, `cargo-deny`, `rustup toolchain install 1.85`, `libpipewire-0.3` + `libspa-0.2` dev packages, `clang`/`libclang` (build deps table: SPEC 09 §7).
* Tooling files (`justfile`, `deny.toml`, `scripts/`, `.github/`) are owned by the tooling task. A plan that needs a new recipe (e.g. `test-live`) lists it as an explicit task; the implementer edits the file in that task's commit.

## 3. Branching

* `main` is always green (`just check` passes on every commit that is pushed to it). No force-push to `main`.
* One branch per milestone: `m<N>/<slug>` (e.g. `m1/backend-store`). Tasks are commits on that branch (or short-lived `m<N>/<task>` sub-branches for risky tasks such as spikes).
* Merge to `main` when a task group is green: `git switch main && git merge --ff-only m<N>/<slug>` (rebase the branch first if needed). A milestone may be merged in several increments; each increment must pass `just check` and leave the app runnable.
* Spikes (`M1-T01`) live in a scratch directory outside the repository (e.g. `~/tmp/pw-spike`); only the *findings* are committed (plan + SPEC text).
* Dependabot branches are not merged individually if they change `pipewire`, `crossterm`, `ratatui` (they are coupled; see ROADMAP risks). Evaluate them inside the task that owns that decision.

## 4. Commits

Conventional Commits (matches existing history: `feat:`, `fix:`; the old `BREAKING-CHANGE:` prefix is not used any more):

```
<type>(<scope>): <imperative subject, ≤ 72 chars>

<why, what changed; wrap at 80>

Refs: VOL-VM-005, VOL-TEST-004
```

* `type`: `feat`, `fix`, `refactor`, `test`, `docs`, `build`, `chore`, `perf`. Test-first sequences may be one commit (`feat(model): …` containing tests + code) or two (`test(model): …` then `feat(model): …`); the pushed state must be green either way.
* `scope`: `model`, `backend`, `pw`, `ui`, `keys`, `theme`, `cli`, `spec`, `plan`, `build`, `release`.
* Every commit that implements or changes a requirement has a `Refs:` footer with the IDs. Release notes are generated from these subjects, so write them for users.
* `CONTRIBUTING.md` allows spec IDs in the subject or body; this plan standardizes on the `Refs:` footer. Breaking changes use a `BREAKING CHANGE:` footer (as in `CONTRIBUTING.md`).
* One task = one commit (or a few); never mix a refactor and a behavior change.

## 5. Working a task

1. Read the task in the milestone plan: *Do*, *Files*, *Tests first*, *Done when*, *Reqs*.
2. Write the listed tests first; run them and see them fail for the right reason (`cargo test <name>`).
3. Implement the smallest change that passes. Keep pure modules free of forbidden imports (`tests/layering.rs`, VOL-ARCH-001).
4. `just check`. Fix everything; do not `#[allow]` a lint to get green unless the plan says so (and then with a one-line reason).
5. Update the SPEC status cells (§6), tick the task in the plan, commit.
6. At a milestone's end: Definition of Done list, manual acceptance items, update ROADMAP status.

Task IDs are `M<N>-T<NN>`; each task is meant to take ≤ ½ day. If a task is bigger, split it in the plan first (edit the plan, then proceed).

## 6. Marking SPEC status

* Status cells are `Implemented` / `Stub (what's missing)` / `Missing` / `n/a`. Change them **in the same commit** that makes them true; never in advance.
* `Implemented` requires: the behavior exists, a test named in the plan covers it (or a manual item is recorded for requirements that cannot be automated), and `just check` passes.
* When a requirement's text turns out wrong or incomplete, edit the text and note it in the commit body; IDs are never renumbered or reused (VOL-GEN-002). Withdrawn requirements keep their row with `n/a` and `Withdrawn: <reason>`.
* `VERIFY:` items: when verified, replace the marker with the observed fact (and the date/PipeWire version), update `SPEC.md` §7 (mark the VF row `closed` and cite the plan task), and keep the procedure text if it is useful as a manual acceptance step.
* The counts table in `SPEC.md` §1 is regenerated at the end of each milestone (not per commit). Procedure: recount the last column of the `| VOL-…` rows per file (the coverage check in `docs/plans/coverage.md` shows how to parse them) and rewrite the table.
* Open questions: resolving one edits the `SPEC.md` §8 row (replace "Proposed default" by "Decided: …, <date>"), the ROADMAP decision log, and the affected requirements.

## 7. Definition of done (every task)

* Tests listed in the task exist and pass; no `#[ignore]` except live tests.
* `just check` passes locally.
* No new `unwrap`/`expect`/`println!` outside tests (clippy policy, VOL-QA-002); no `TODO`/`todo!()`/dead code left behind.
* SPEC statuses/plan checkboxes updated; mockups in docs updated if rendering changed (VOL-GEN-007).
* User-visible changes add a line under `## [Unreleased]` in `CHANGELOG.md` (Keep a Changelog; renamed to the version heading at release time).
* `README.md` stays truthful: when a key, flag or feature becomes real, flip its *planned* → *Implemented* in the README tables in the same commit (VOL-QA-008 is finalized in M9).

## 8. Conventions used in plans

* "Tests first" entries are `path::test_name` (unit tests in `src/**` modules: `module::tests::name`; integration tests: `tests/<file>.rs::name`). Property tests are marked **(prop)**, snapshots **(snap)**, live tests **(live, ignored)**.
* File paths are relative to the repository root; module layout follows `docs/specs/01-architecture.md` §2 plus the M0 additions recorded there (`lib.rs`, `cli.rs`, `ui_state.rs`).
* "Reqs" lists IDs touched by the task; the milestone-level list (one milestone per ID) is authoritative for coverage.
* Clean-room rule (VOL-GEN-005) applies to every task: no pavucontrol source, ever; PipeWire/WirePlumber CLIs and headers are fair game and findings are recorded in the SPEC sources list.
* Manual acceptance items refer to `docs/specs/09-testing-and-quality.md` §4 (MA-01…MA-14).
