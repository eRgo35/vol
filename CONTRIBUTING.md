# Contributing to vol

Read [SPEC.md](SPEC.md) and the relevant files in [docs/specs/](docs/specs/) before changing behavior. The clean-room rule below applies to every change.

## Development setup

Install the build dependencies listed in [README.md](README.md#requirements) (PipeWire and libclang development files, pkg-config, clang). Then:

```sh
rustup toolchain install stable
rustup toolchain install 1.88        # needed for `just msrv`
cargo install --locked just cargo-deny
scripts/install-hooks.sh             # once per clone
cargo run
```

`just` is the task runner. `just` with no argument lists the recipes.

### Nix

`flake.nix` provides a dev shell for `x86_64-linux` and `aarch64-linux`; `.envrc` contains `use flake` for direnv. Enter it with `nix develop` (or `direnv allow`). It is meant to build without host packages: it supplies the stable Rust toolchain (from `rust-overlay`, or `rust-toolchain.toml` if one is added), `just`, `cargo-deny`, `pkg-config`, `pipewire` (its `dev` output provides the `libpipewire-0.3` and `libspa-0.2` headers and pkg-config files), `clang` and `llvmPackages.libclang`, and sets `LIBCLANG_PATH` for bindgen. `cargo-edit`, `cargo-watch` and `rust-analyzer` are included for convenience. The shell has not yet been verified on a Nix machine; reports welcome.

The shell has no `rustup`, so `just msrv` (`cargo +1.88 …`) does not work inside it; run that recipe with a rustup-managed 1.88 toolchain outside the shell.

## Local gate

Run before every push:

```sh
just check
```

`just check` runs, in order:

| Recipe | Command |
|---|---|
| `fmt` | `cargo fmt --all -- --check` |
| `clippy` | `cargo clippy --all-targets --all-features --locked -- -D warnings` |
| `test` | `cargo test --locked --all-features --all-targets` |
| `doc` | `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --locked` |
| `deny` | `cargo deny check` (licenses are permissive-only; see `deny.toml`) |
| `msrv` | `cargo +1.88 check --locked --all-features --all-targets` |

`just release-check` runs the gate and then `cargo build --release --locked`.

Tests need no running PipeWire daemon. Live tests (planned, [09 §3](docs/specs/09-testing-and-quality.md#3-required-tests)) are `#[ignore]`d and run only with `VOL_TEST_PIPEWIRE=1`.

## Git hooks

`scripts/install-hooks.sh` sets `core.hooksPath=scripts/hooks`. The `pre-push` hook runs `just check` on every push and fails if `just` is not installed. To skip it for one push: `git push --no-verify`.

GitHub Actions does not run the gate. [.github/workflows/release.yml](.github/workflows/release.yml) only runs on version tags.

## Clean-room rule

vol reimplements the observable behavior of pavucontrol. To keep it clean:

- Do **not** read, fetch, clone, grep, copy or decompile pavucontrol source code. This includes other repositories, distribution source packages and binaries.
- Allowed inputs: pavucontrol's observable UI behavior; PipeWire, SPA and WirePlumber documentation and headers; the `libpipewire` and `pipewire-rs` APIs; and the output of `pw-cli`, `pw-dump`, `pw-metadata`, `pw-link`, `wpctl` and `pactl`, used as black boxes.
- Pull requests that reference or contain pavucontrol source code are rejected.

The full rule is VOL-GEN-005 in [SPEC.md §2](SPEC.md#2-conventions).

## Spec-driven workflow

1. Find the requirement IDs (`VOL-<AREA>-<NNN>`) that your change implements in [docs/specs/](docs/specs/). Area prefixes are listed in [SPEC.md §1](SPEC.md#1-document-map).
2. Implement the change and its tests against those requirements.
3. Update the **Status** column of each requirement you touched in the same change: `Implemented`, `Stub (what is missing)`, `Missing` or `n/a`.
4. Anything not directly observed or documented is marked `VERIFY:` (with a procedure) or `[INFERENCE]` (VOL-GEN-004). Resolve a `VERIFY` by editing the spec text and recording the result in the PR. The index is [SPEC.md §7](SPEC.md#7-verify-index).
5. Requirement IDs are never renumbered or reused. A removed requirement keeps its row with status `n/a` and the text `Withdrawn: <reason>`.
6. Public behavior changes update [README.md](README.md) (usage, keys, flags, requirements) and [CHANGELOG.md](CHANGELOG.md) in the same change (VOL-QA-008).

The PR template ([.github/PULL_REQUEST_TEMPLATE.md](.github/PULL_REQUEST_TEMPLATE.md)) asks for the spec IDs and confirms `just check` was run.

## Commit messages

Follow the existing history: a Conventional Commits-style prefix and a short imperative subject, such as `feat: ...`, `fix: ...`, `docs: ...`, `chore(deps): ...`. Put spec IDs in the subject or body. Mark breaking changes with a `BREAKING CHANGE:` footer.

## Release process

Releases are tags. Pushing a `vX.Y.Z` tag triggers [.github/workflows/release.yml](.github/workflows/release.yml), which builds the x86_64 Linux binary and publishes a GitHub release.

1. Set `version = "X.Y.Z"` in `Cargo.toml` and let Cargo update `Cargo.lock` (`cargo check`). Commit both.
2. In `CHANGELOG.md`, rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD` and add a new empty `## [Unreleased]` above it.
3. Run `just release-check`.
4. Create and push the tag, matching the version exactly:

   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

5. The workflow fails if the tag does not equal `v` plus the `Cargo.toml` version. On success it publishes `vol-vX.Y.Z-x86_64-linux.tar.gz` (binary, `LICENSE`, `README.md`) and `SHA256SUMS`, with generated release notes.
