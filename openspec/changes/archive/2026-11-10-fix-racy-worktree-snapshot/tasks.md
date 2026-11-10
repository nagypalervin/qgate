# Tasks

One pull request, `change/fix-racy-worktree-snapshot`, with these planning
artifacts. Decisions made during apply go into `design.md`. Every commit
passes `cargo fmt`, `cargo clippy -D warnings` and `cargo test
--workspace` (the pre-commit hook enforces it), and CI also runs the
tests on Rust 1.85. Commit messages and PR bodies are written to files
first.

## 1. The copy keeps the index's time

- [x] 1.1 Write failing tests for an edit that git sees as racily clean, set up as in design D4:
  - a unit test in `crates/qgate-git/src/worktree.rs`: `git diff-files` lists the file, and the snapshot must list it too;
  - an end-to-end test in `crates/qgate-cli` for the scenario "An edit in the same instant as the index write": a `complexity/cyclomatic-increase` finding for `f`.

  Verify that both fail on the current code, and that the unit test passes when the copy keeps the original time (a temporary one-line change).
- [x] 1.2 Keep the original index's modification time on the copy, and read it back (design D1). Verify:
  - the 1.1 tests pass;
  - every existing snapshot test passes unchanged, including the split-index test;
  - the new unit test also passes with `core.splitIndex=true`.
- [x] 1.3 Make the snapshot fail with a work-tree error when the copy cannot keep the index's time: reading it fails, setting it fails, or the time read back differs (design D3, changed during apply). Verify with a unit test of the comparison: equal times pass; different times give `GitError::WorkTree` naming the private index.

## 2. Age from the name

- [x] 2.1 Put the start time's seconds into the copy's name, and make `remove_stale` judge by them, with old-form names judged by their modification time (design D2). Update the stale-copy test, and verify that:
  - a two-day-old copy by name is removed;
  - a copy named a minute ago is kept, even when its modification time is three days old;
  - an old-form copy with a two-day-old modification time is removed;
  - an old-form copy with a recent modification time is kept;
  - a name that does not parse is kept.
- [x] 2.2 Check that the tests bite. Break each of these, one at a time, and confirm that a test fails for each:
  - dating the copy now;
  - judging new-form names by their modification time;
  - accepting a time that reads back different in the comparison.

## 3. Verification

- [x] 3.1 Run the hand reproduction from the proposal against the new release build: `qgate check --base HEAD --worktree` reports the +7 increase, and the hook blocks. Also run `snapshot_in_a_linked_worktree` 300 times in a loop and confirm that it does not fail. Record both results in `design.md`.

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- Write the `docs/journal/` entry, then archive the change on the branch (`/opsx:archive`).
- Gate 3: the human merges the pull request.
