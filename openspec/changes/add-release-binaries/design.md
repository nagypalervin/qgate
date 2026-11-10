# Design: add-release-binaries

## Context

See `proposal.md` for the why.

- **CI** (`.github/workflows/ci.yml`) has two jobs, both on
  `ubuntu-latest`:
  - fmt, clippy and tests;
  - the MSRV tests on Rust 1.85.

  The actions are referenced by major tag (`@v4`, `@v2`).
- **No tag or release exists yet,** and the version is `0.0.1`:
  `workspace.package.version`, plus the `version` of each internal
  crate in `[workspace.dependencies]`.
- **tree-sitter compiles C code,** so every target needs a C toolchain.
  Native runners provide one, and the spike showed that `musl-tools`
  covers both musl targets.
- **Spike results** (2026-11-10):
  - **Linux, macOS (`macos-latest`, `macos-15-intel`) and Linux arm64
    (`ubuntu-24.04-arm`)** passed the full suite.
  - **Windows (`windows-latest`, `windows-11-arm`)** failed two tests,
    both on assumptions in the tests (see D2). Windows arm64 is the
    slowest runner, at about 4.5 minutes for build and test.
  - **musl:** a release build was static-pie on x64 and static on
    arm64, about 7 MB unstripped.

## Goals / Non-Goals

**Goals:**
- Every released binary was tested on its own platform, in the same
  workflow run, from the tagged commit.
- A release can be rehearsed on a pull request, with the same jobs, up
  to the point of publishing.
- The only job with write access to the repository is the one that
  creates the draft.

**Non-Goals:**
- Reproducible builds and provenance attestations. They could come
  later, with signing.
- Faster CI through caching beyond what `Swatinem/rust-cache` gives.

## Decisions

### D1. A `platforms` job in `ci.yml` covers the five other platforms

`ci.yml` gets a third job, `platforms`. It runs `cargo test --workspace
--locked` on a matrix with `fail-fast: false`:
- `ubuntu-24.04-arm`;
- `macos-latest`;
- `macos-15-intel`;
- `windows-latest`;
- `windows-11-arm`.

Linux x64 is already covered by the existing `check` job. Repeating it
would cost a runner and add nothing.

The job runs the native host target (glibc on Linux), as developers
and `cargo install` users do. The release workflow (D3) tests the musl
targets that it ships. That workflow runs on pull requests only when
release files change. So a change that breaks only under musl would
first show at tag time. This gap is accepted: qgate has no
libc-specific code, and the spike passed the full suite under musl.

**Required checks.** The repository's "Protect main" ruleset makes
checks block a merge. It must list the five `test (<runner>)` checks
next to `fmt, clippy, test` and `MSRV (Rust 1.85)`. Otherwise a red or
still-running platform check would not stop a merge. The check names
come from the runner labels, so a runner change (see Risks) means
changing the ruleset too. (Added in review of PR 1.)

*Alternative:* a separate workflow. One `ci.yml` keeps every
pull-request check in one place and one status list.

### D2. The two Windows failures are fixed in the tests

- **`blob_reader_killed_process`** (`qgate-git`). The test kills the
  reader's `git` process and expects `GitError::Command` with no exit
  status. On Windows, `TerminateProcess` leaves exit code 1. The test
  will:
  - assert `GitError::Command` on every platform;
  - assert `status: None` under `cfg(unix)`, and `status: Some(1)`
    under `cfg(windows)`. Rust's `Child::kill` calls
    `TerminateProcess(handle, 1)`. The Windows assertion was added in
    review of PR 1.

  The library does not change: on both systems, it reports the process
  as failed.
- **The `mcp` unit tests' start directory.** `/work/app` is not
  absolute on Windows, because it has no drive. So
  `an_unreadable_start_directory_is_a_tool_error_when_needed` joined it
  onto an unreadable working directory. The test constants become
  per-platform (`C:\work\app` on Windows), and the expected paths are
  built with `Path::join` instead of string literals. The case
  `"directory": "/other/repo"` has to move to a per-platform absolute
  path too. On Windows, `C:\work\app` joined with `/other/repo` gives
  `C:\other\repo`.

### D3. The release workflow: version check, build matrix, publish

`.github/workflows/release.yml`:

```
  on: push tags v*  |  pull_request touching release files  |  workflow_dispatch
          |
          v
  +----------------+      +--------------------------------------+      +-----------------+
  | version        | ---> | build (matrix of 6 targets)          | ---> | publish         |
  | tag: tag ==    |      |  test --target T, build --release,   |      | tags only:      |
  |   Cargo        |      |  check version and linking, pack,    |      | SHA256SUMS,     |
  | else: test the |      |  upload                              |      | draft release   |
  |   script       |      +--------------------------------------+      +-----------------+
  +----------------+
```

- **`version`:** runs `.github/scripts/check-version.sh "$GITHUB_REF_NAME"`.
  - The script reads the workspace version with
    `cargo metadata --no-deps --format-version 1` and `jq`. It fails
    unless the tag is exactly `v<version>`, and the message names both
    versions.
  - On a pull request or a manual run, the job checks the script itself
    instead: once with `v<version>`, which must pass, and once with a
    wrong tag, which must fail. A broken script then shows up before
    the first real tag.
- **`build`:** a matrix of the six targets, each on its native runner:

  | Target | Runner |
  |---|---|
  | `x86_64-unknown-linux-musl` | `ubuntu-latest` |
  | `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` |
  | `x86_64-apple-darwin` | `macos-15-intel` |
  | `aarch64-apple-darwin` | `macos-latest` |
  | `x86_64-pc-windows-msvc` | `windows-latest` |
  | `aarch64-pc-windows-msvc` | `windows-11-arm` |

  Each job runs these steps:
  1. install `musl-tools` (Linux only);
  2. `cargo test --workspace --locked --target <T>`;
  3. `cargo build --release --locked -p qgate-cli --target <T>`;
  4. check the built binary:
     - its `--version` output is `qgate <version>`;
     - on Linux, `file` reports `statically linked` or `static-pie
       linked`, and not `dynamically linked`. musl gives `static-pie`
       on x64 (spike);
     - on Windows, `llvm-readobj --coff-imports` (from rustup's
       `llvm-tools` component) lists no `VCRUNTIME` DLL;
  5. pack the archive (D4);
  6. upload it as a workflow artifact.

  `fail-fast: true` here: one failing target stops the release.

  **Windows links the C runtime statically.** By default the MSVC
  targets need `VCRUNTIME140.dll` from the Visual C++ Redistributable.
  It is not part of Windows, but it is on GitHub's runner images, so
  CI would not notice. Server Core containers, fresh arm64 machines and
  some self-hosted agents do not have it.
  - The Windows matrix entries set
    `RUSTFLAGS: -C target-feature=+crt-static` for the test and build
    steps, as ripgrep's releases do.
  - It is set in the workflow, not in `.cargo/config.toml`. A
    `RUSTFLAGS` variable makes Cargo ignore the config's rustflags, so
    a flag in both places could be silently dropped.
  - It applies only to release builds. Developers and
    `cargo install` keep the default.

  (Added in review of PR 1.)

  **`release.yml` does not set `-D warnings`,** and builds with the
  `stable` toolchain of the day. Lints were enforced when the commit
  merged, and a lint that appears between merge and tag must not block
  a release. A compiler release that breaks the build would fail the
  release before anything is published. It is then fixed like any other
  build failure.
- **`publish`:** runs only for a tag push, after every `build` job
  succeeded. It holds the only `contents: write` permission in the
  workflow. It:
  1. downloads the six archives;
  2. writes `SHA256SUMS` with `sha256sum` in the archives' directory,
     sorted by file name;
  3. if a release for the tag already exists, deletes it when it is a
     draft, which is a re-run after a failed upload, and fails when it
     is published, since a published version is never replaced;
  4. runs `gh release create "$TAG" --draft --title "qgate <version>"
     --generate-notes --repo "$GITHUB_REPOSITORY"` with the seven
     files. `GH_TOKEN` is the job's `GITHUB_TOKEN`. There is no
     checkout, so `--repo` names the repository.

  (Steps 3 and 4's `--repo` and `GH_TOKEN` were added in review of PR 1.)

  The workflow's other jobs get `contents: read`.
- **Triggers:**
  - **pull requests:** only those that change `release.yml`,
    `.github/scripts/**`, `Cargo.toml` or `Cargo.lock`. The release
    then gets rehearsed whenever its inputs change, not on every pull
    request;
  - **`workflow_dispatch`:** for a rehearsal on `main`;
  - **concurrency:** per ref, so two pushes of the same tag cannot race.
- **Pinned actions.** The release workflow pins every third-party
  action to a full commit SHA, with the version in a comment. A
  compromised tag could otherwise run in the job that can write
  releases. `ci.yml` keeps major tags, since it has only `contents:
  read`.

*Alternatives:*
- **A release action** (`softprops/action-gh-release`,
  `taiki-e/upload-rust-binary-action`). They save little over `gh`,
  which is preinstalled, and add a third party to the job that can
  write.
- **`cargo-dist`.** It generates its own workflow and installers. It is
  good, but much more machinery than six archives and a checksum file
  need. It could be revisited with package managers.
- **Cross-compiling everything from Linux with `cross` or `zig`.** It
  would be cheaper, but the binaries would not be tested on their own
  platforms, which breaks the first goal.

### D4. Archives and their layout

Each archive holds `qgate-<version>-<target>/`, which contains:
- the binary;
- `README.md`;
- `LICENSE-MIT` and `LICENSE-APACHE`.

On Linux and macOS it is packed with `tar czf`. On Windows it is packed
with `7z a -tzip`; 7-Zip is on both Windows runner images. All steps
use `shell: bash`, which the Windows images also have (Git Bash). The
packing is a few lines in the workflow, not a script, because it has
no logic worth testing apart from the archive check in D6.

**Checksum commands for the README** (added in review of PR 1), with
`<archive>` being the downloaded file:
- **Linux:** `sha256sum --check --ignore-missing SHA256SUMS`;
- **macOS:** `grep " <archive>$" SHA256SUMS | shasum -a 256 --check`.
  `shasum` ships with macOS, `sha256sum` does not;
- **Windows (PowerShell):** compare
  `(Get-FileHash <archive> -Algorithm SHA256).Hash` with the archive's
  line in `SHA256SUMS`, case-insensitively.

### D5. Release profile and version

- **`Cargo.toml`** gets `[profile.release]` with `strip = true`. Debug
  symbols are of no use to users of a release, and stripping makes the
  binary smaller. `lto` and `codegen-units = 1` are left out: they make
  six builds slower for a small gain.
- **The version becomes `0.1.0`:** `workspace.package.version` and
  the six internal `[workspace.dependencies]` entries. `Cargo.lock`
  changes with it. `qgate --version` then prints `qgate 0.1.0`. No test
  pins the version string; the release workflow's `--version` step
  checks it against the tag.

### D6. Verification and the three pull requests

1. **PR 1, `change/add-release-binaries`:**
   - the planning artifacts;
   - the D2 test fixes;
   - the `platforms` job (D1).

   Its own checks show all six platforms green.
2. **PR 2, `change/add-release-binaries-2`:**
   - `release.yml` and the version script;
   - the release profile and `0.1.0`;
   - the README.

   The workflow runs on this pull request, because it changes
   `release.yml`. The agent downloads the six archives from the run
   and checks every spec scenario that does not need a tag:
   - archive names and layout;
   - `SHA256SUMS`, computed locally the way `publish` would;
   - `--version`;
   - static Linux binaries. The agent's machine is Linux arm64, so it
     can run the `aarch64-unknown-linux-musl` binary.

   The version script is tested locally with a matching and a wrong
   tag.
3. **The first release (Ervin), after PR 2 merges:**
   1. Ervin pushes `v0.1.0`;
   2. the agent checks the draft: assets, `sha256sum --check`, and
      running the Linux arm64 binary;
   3. Ervin publishes the draft.
4. **PR 3, `change/add-release-binaries-3`:**
   - records the release check in this file;
   - archives the change;
   - adds the journal entry.

   CLAUDE.md asks for the archive on the last pull request's branch.
   The release itself can only be checked after PR 2 has merged, so
   the archive needs a third pull request.

The spec scenario "A mistyped tag" is checked through the version
script, locally and on PR 2, not by pushing a wrong tag to the public
repository. "Tests failing on one platform" holds by construction:
`publish` needs every `build` job, and each `build` job runs the tests
first. It is not tested by breaking a test on purpose.

## Implementation Notes

Decisions and results recorded during apply.

- **PR 1 (#10), first CI run, 2026-11-10.** All seven checks passed,
  with `RUSTFLAGS=-D warnings` from `ci.yml`. The spike had run
  without it, so this also showed that no code compiled only on
  Windows produces warnings.

  | Job | Duration |
  |---|---|
  | `check` (Linux x64) | 37 s |
  | MSRV | 32 s |
  | `test (ubuntu-24.04-arm)` | 35 s |
  | `test (macos-latest)` | 1 min 30 s |
  | `test (macos-15-intel)` | 1 min 41 s |
  | `test (windows-latest)` | 3 min 27 s |
  | `test (windows-11-arm)` | 5 min 38 s |

  These are cold caches. Windows arm64 sets the wall time of a pull
  request.
- **The `mcp` test constants** (D2) are `const` items chosen with
  `cfg!(windows)`, `C:\work\app` and `C:\other\repo`. The joined
  path is built with `Path::new(CWD).join("sub/dir")`, so each platform
  compares its own separator.

- **Review of PR 1.** The `reviewer` subagent found the PR's code
  correct. It also found nine problems, mostly in the approved spec and
  design, which PR 2 would have inherited. All nine were addressed in
  PR 1:
  1. **The platform checks were not required** by the "Protect main"
     ruleset → D1 and task 1.4, which needs Ervin's approval.
  2. **`file` prints `static-pie linked` for the x64 musl binary,** so a
     check for `statically linked` would have failed every release →
     the spec and D3 accept both and reject `dynamically linked`.
  3. **Windows binaries would have needed the Visual C++
     Redistributable** → `+crt-static` for the Windows release builds
     (D3), a spec requirement and scenario, and an import check. This
     changes the approved design, so it went back to Ervin.
  4. **The spec said CI tests "each of these platforms"** while D1
     tests glibc → the spec names the host target, and D1 accepts the
     musl gap.
  5. **The Windows branch of the killed-process test** asserted no exit
     code → it asserts `Some(1)` (D2).
  6. **D3's diagram said the `version` job ran on tags only** → it now
     shows the script test on other triggers.
  7. **`publish` was not safe to re-run,** and `gh` lacked `--repo` →
     D3, steps 3 and 4.
  8. **`-D warnings` and the toolchain in `release.yml`** were not
     decided → D3: no `-D warnings`, and `stable`.
  9. **`sha256sum` is not on macOS or Windows** → D4 names a command
     for each OS.

## Risks / Trade-offs

- **[GitHub retires `macos-15-intel`, the last x64 macOS image]** →
  The x64 macOS build would move to cross-compiling on `macos-latest`
  (`--target x86_64-apple-darwin`), and its tests would run under
  Rosetta 2, if the runner image still has it. Otherwise only the build
  would run there. The release workflow names its runners in one
  matrix, so the move is a one-line change plus that decision. The
  `ci.yml` check `test (macos-15-intel)` changes its name with the
  runner, so the ruleset's required checks (D1) must change with it.
- **[Unsigned binaries]** →
  - On macOS, Gatekeeper blocks a binary downloaded with a browser
    until its quarantine attribute is removed. `curl` downloads are not
    quarantined.
  - On Windows, SmartScreen can warn about a downloaded `.exe` that is
    started from Explorer. `Unblock-File` clears the mark.

  The README documents both. Signing is deferred (proposal).
- **[A wrong tag is pushed]** → The version job fails before anything
  is built, and no release is created. The tag stays, and a maintainer
  deletes it.
- **[A draft is forgotten]** → Nothing is public until it is
  published. The release check (D6, step 3) ends with publishing.
- **[Six more runners on every pull request]** → Billing is not an
  issue, since the repository is public and its minutes are free. The
  cost is wall time, about 5 minutes for Windows arm64. It runs in
  parallel with the existing jobs.
- **[`--locked` with a lockfile that does not match the new
  version]** → PR 2 updates `Cargo.lock`, and the MSRV job, which
  already uses `--locked`, fails if it does not.

## Migration Plan

`cargo install --locked --path crates/qgate-cli` keeps working, and
the README keeps it as the second way. Rollback: delete a bad release
and its tag, fix, and tag the next patch version. A published version
number is never reused.
