# Tasks

Two pull requests, one per group. PR 1 also carries this change's planning
artifacts. Branches: `change/add-git-revision-input` (PR 1), then
`change/add-git-revision-input-2`. Every task ends with `cargo fmt --all`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` passing (chained with `&&`).

## 1. `qgate-git` crate and ADR 0002, PR 1

- [x] 1.1 Write `docs/adr/0002-git-via-subprocess.md` (git subprocess instead of `gix`, with the measured crate counts; the `qgate-git` leaf crate; when to revisit) and update the "Layout" and dependency-direction lines in `CLAUDE.md` and the repository layout in `README.md`; verify the ADR follows the format of ADR 0001
- [x] 1.2 Add the `crates/qgate-git` workspace member with no dependencies, the documented public types of design D2 (`Repository`, `ChangedFile`, `BlobId`, `GitError` with `Display` and `Error`) and stub methods; verify `cargo build --workspace` and clippy with `missing_docs` pass and `cargo tree -p qgate-git` shows no dependencies
- [x] 1.3 Write the temp-repository test helper (design D5) and failing tests for `discover` (inside a repository, in a subdirectory, not a repository) and for revision resolution and merge base (unknown revision, option-like revision `--output=...` leaves no file, no merge base, merge base on a moved base); then implement `discover` and the resolution and merge-base steps of design D3 until they pass
- [x] 1.4 Write failing tests for `changed_files`: modified, added, deleted (absent), renamed (old path set), renamed from `.js`, symlink, type change and submodule entry (absent), executable mode, a non-UTF-8 path, and no changes; then implement the `--raw -z -M` parsing until they pass
- [x] 1.5 Write a failing test that `read_blob` returns the exact bytes of an old and a new version (including invalid UTF-8 content); implement it with `git cat-file blob`; verify the tests pass
- [x] 1.6 Write the `qgate-git` crate docs with a doctest-free usage example (doctests cannot create repositories) and verify `cargo doc -p qgate-git --no-deps` builds without warnings

## 2. `qgate check --base` in the CLI, PR 2

- [ ] 2.1 Extend the `check` arguments (design D4: optional positionals, `--base`, `--head` defaulting to `HEAD`, conflicts) and add end-to-end tests for the `Invalid invocation` scenarios (files plus `--base`, `--head` without `--base`, one argument); verify the existing two-file tests still pass unchanged
- [ ] 2.2 Write failing end-to-end tests in `crates/qgate-cli/tests/revisions.rs` with temporary repositories for every scenario of the `revision-input` delta and the revision-mode scenarios of the `check-command` delta (pull request branch, head default, path from a subdirectory, several files order)
- [ ] 2.3 Implement revision mode in `qgate-cli` (design D4): file selection, before-side choice, parsing with `input/unparsable` findings, analysis, `(path, line, rule)` sort and `GitError` messages with the shallow-clone hint; verify the 2.1 and 2.2 tests pass
- [ ] 2.4 Update `README.md`: revision-mode usage, the CI snippet with `actions/checkout` and `fetch-depth: 0`, `git` as a requirement for revision mode and tests, and the `input/unparsable` rule; verify every command shown runs as written against a temporary repository

## Workflow follow-up

- Run the `reviewer` subagent on each PR and fix its findings.
- In PR 2: write the `docs/journal/` entry, then archive the change on that branch (`/opsx:archive`) so the specs land on `main` with the code.
- Gate 3: the human merges each PR.
