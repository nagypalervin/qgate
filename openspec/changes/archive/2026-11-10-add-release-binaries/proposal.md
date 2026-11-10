# Proposal: add-release-binaries

## Why

Today you install qgate with `cargo install` from a clone. That needs a
Rust toolchain on every developer machine and every CI agent. It is the
largest barrier for the partner teams. The hook and the MCP server
(M3) both need `qgate` on the `PATH` of the developer's machine. The CI
integration (the rest of M4) needs it on GitHub Actions runners and
Azure DevOps agents.

qgate has also only ever run on Linux x64. Its tests have never run on
macOS or Windows.

## What Changes

- **CI runs the test suite on every platform qgate ships for**, for
  every pull request:
  - Linux on x64 and arm64;
  - macOS on Apple Silicon (arm64) and Intel (x64);
  - Windows on x64 and arm64.

  Two tests that assume Unix are fixed so that they hold on Windows.
- **A release workflow.** Pushing a tag `v<version>` that matches the
  workspace version:
  - builds qgate on each of the six platforms, after running the tests
    there;
  - packs each binary with the license files;
  - writes one `SHA256SUMS` file;
  - creates a **draft** GitHub Release with all of them.

  Ervin reviews the draft and publishes it. A tag that does not match
  the version fails the workflow and creates nothing.
- **Linux binaries are static (musl).** They run on any distribution,
  including Alpine-based CI images.
- **The version becomes `0.1.0`**, the first release.
- **README:**
  - **"Try it":** installing from a release for each OS, with the
    checksum check. Building from source stays as the second way.
  - **Unsigned binaries:** what macOS Gatekeeper and Windows SmartScreen
    do with an unsigned download, and how to get past them.
  - **Roadmap:** M4 becomes "in progress".

### Decisions taken during exploration

- **All six platforms** (Ervin): Linux, macOS and Windows, each on x64
  and arm64.
- **No code signing in the first release** (Ervin). macOS and Windows
  users get a documented one-time step. Signing needs paid
  certificates, and is decided later from partner feedback.
- **Spike, 2026-11-10:** the full test suite ran on all six GitHub
  Actions runners, from a throwaway branch that has since been deleted.
  - **Linux and macOS:** all green. A static musl build passed the
    tests on Linux x64 and arm64, and the binary is about 7 MB.
  - **Windows, x64 and arm64:** two tests failed, both on assumptions
    in the tests themselves:
    - `blob_reader_killed_process` expects a killed process to have no
      exit code, which only holds on Unix;
    - an `mcp` unit test treats `/work/app` as an absolute path, which
      Windows does not.

    qgate's behaviour needs no change. Reported paths use `/` on
    Windows too, because they come from git.
- **A draft release, published by hand.** The tag is the human's
  trigger. The draft lets Ervin look at the assets before anyone can
  download them, which matters most for the first release.
- **Three pull requests:**
  1. the platform CI with the test fixes;
  2. the release workflow, the version and the README;
  3. after Ervin has tagged `v0.1.0` and the draft has been checked: the
     record of that check, the journal entry and the archive.

  The release can only be checked once PR 2 has merged, and the archive
  belongs on the last pull request (design D6).

### Out of scope

- Code signing and notarization (macOS, Windows).
- Package managers: Homebrew, Scoop, winget, `cargo binstall`, crates.io.
- An install script (`curl | sh`).
- Release notes beyond GitHub's generated list.
- The GitHub Action and the Azure DevOps task that use these binaries.
  Those are the next M4 changes.

## Capabilities

### New Capabilities

- `releases`: how qgate is released:
  - the platforms it ships for;
  - what a version tag produces (the draft release, the archives, the
    checksums);
  - which guarantees each binary carries: tested on its platform,
    reporting its version, static on Linux.

### Modified Capabilities

None. No command changes behaviour.

## Impact

- **`.github/workflows/`:**
  - `ci.yml` gets a platform test matrix;
  - a new `release.yml` runs on version tags. It builds and packs on
    pull requests that change it too, but publishes nothing then.
- **Rust code:** only two tests change (`qgate-git`, `qgate-cli`).
- **`Cargo.toml`:**
  - version `0.1.0`;
  - a release profile that strips symbols;
  - `Cargo.lock` follows.
- **Docs:** README.
- **GitHub:** the release workflow gets `contents: write` for its
  publishing job only.
