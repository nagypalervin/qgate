# Tasks

This change spans three pull requests (design D6).
- **Decisions made during apply** go into `design.md`.
- **Every commit passes these checks**, which the pre-commit hook
  enforces:
  - `cargo fmt`;
  - `cargo clippy -D warnings`;
  - `cargo test --workspace`.
- **Commit messages and PR bodies** are written to files first.

## 1. Platform CI (PR 1, `change/add-release-binaries`)

- [x] 1.1 Make `blob_reader_killed_process` in `crates/qgate-git/src/lib.rs` hold on Windows (design D2):
  - assert `GitError::Command` on every platform;
  - assert `status: None` only under `cfg(unix)`.

  Verify that it still passes locally.
- [x] 1.2 Make the `mcp` unit tests' paths per-platform (design D2):
  - the start directory and the "other" absolute directory become constants with a `C:\` form on Windows;
  - expected joined paths are built with `Path::join`.

  Verify that they still pass locally.
- [ ] 1.3 Add the `platforms` job to `.github/workflows/ci.yml` (design D1). It runs `cargo test --workspace --locked` on `ubuntu-24.04-arm`, `macos-latest`, `macos-15-intel`, `windows-latest` and `windows-11-arm`, with `fail-fast: false` and `Swatinem/rust-cache`. Open PR 1 and verify that all its checks pass. Record the job durations in `design.md`.

## 2. Release workflow (PR 2, `change/add-release-binaries-2`)

- [ ] 2.1 Write `.github/scripts/check-version.sh <tag>` (design D3). Run it locally against the current version in four ways:
  - a matching tag must pass;
  - `v0.0.2` must fail with a message that names both versions;
  - a tag without `v` must fail;
  - a missing argument must fail.
- [ ] 2.2 Set the version to `0.1.0` (workspace package and the six internal workspace dependencies) and add `[profile.release]` with `strip = true` (design D5). Verify that:
  - `Cargo.lock` is updated;
  - `qgate --version` prints `qgate 0.1.0`;
  - every check passes.
- [ ] 2.3 Write `.github/workflows/release.yml` (design D3, D4):
  - the `version`, `build` and `publish` jobs;
  - triggers on tags, on pull requests that change release files, and `workflow_dispatch`;
  - permissions: `contents: write` only for `publish`;
  - concurrency per ref;
  - third-party actions pinned to commit SHAs.
- [ ] 2.4 Update `README.md`:
  - "Try it" installs from a release on Linux, macOS and Windows, with the checksum check;
  - building from source stays as the second way;
  - a note on unsigned binaries: `xattr` on macOS, `Unblock-File` on Windows;
  - the roadmap's M4 row becomes "in progress".
- [ ] 2.5 Open PR 2 and rehearse the release (design D6). Download the six archives from the workflow run and verify:
  - the names and directory layout of the archives;
  - a `SHA256SUMS` built as `publish` would, which passes `sha256sum --check`;
  - that the `aarch64-unknown-linux-musl` binary runs here, prints `qgate 0.1.0`, and `file` reports it as statically linked;
  - from the job logs, that every target's `--version` check passed and both Linux binaries are static;
  - that the `version` job tested the script, with a matching and a wrong tag.

  Record the results in `design.md`.

## 3. First release and archive (after PR 2 merges)

- [ ] 3.1 Ask Ervin to push the tag `v0.1.0`. When the release workflow finishes, check the draft release `qgate 0.1.0`:
  - six archives and `SHA256SUMS`;
  - `sha256sum --check` on downloaded assets;
  - the Linux arm64 binary prints `qgate 0.1.0`;
  - the README's Linux install commands work against the draft's assets.

  Then ask Ervin to publish the draft. Record the result in `design.md` on `change/add-release-binaries-3` (PR 3).

## Workflow follow-up

- Run the `reviewer` subagent on PR 1 and on PR 2 before asking Ervin to merge each, and fix its findings.
- On PR 3: write the `docs/journal/` entry, then archive the change (`/opsx:archive`).
- Gate 3: the human merges each pull request.
- Update the M4 memory after each merge.
