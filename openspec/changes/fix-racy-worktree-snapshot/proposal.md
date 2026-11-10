# Proposal: fix-racy-worktree-snapshot

## Why

**`--worktree` can miss an edit.** This happens when the edit keeps the file's size and lands in the same file-system timestamp that git last recorded for the file. `qgate hook claude-code` runs this snapshot at the end of every agent turn, so a missed edit means a regression that neither the agent nor the user hears about.

**How it was found.** The `snapshot_in_a_linked_worktree` test failed once in CI (the MSRV job of PR #6) and passed when rerun. The test edits a file right after `git worktree add`, from `a = 1` to `a = 2`, which keeps the size.

**The cause.** The snapshot works on a private copy of the index, and the copy is dated at the moment it is made. Git trusts an index entry's recorded timestamp and size unless the entry is at least as new as the index file itself. That check is git's "racy git" protection. With the copy dated later than the original index, an entry that git would re-read is taken as unchanged instead.

**Reproduced by hand,** with `core.trustctime=false` so that only the modification time and the size count:
- `git diff-files` lists the edited file;
- `qgate check --base HEAD --worktree` prints `No regressions found.` for a +7 complexity increase;
- the hook writes nothing.

A copy that keeps the original index's timestamp sees the edit.

## What Changes

- **The private index copy keeps the original index's modification time,** so git applies the same racy-git check it applies to the user's own index. A snapshot then stages exactly what `git add -A` would stage from the user's index.
- **The clean-up of copies that interrupted checks left behind** no longer judges a copy's age by its modification time, which now says nothing about when the check ran.
  - It reads the age from the copy's name, which gets the time the check started.
  - Copies named by earlier versions are still judged by their modification time; those versions dated them when they were made.
- **If the time cannot be set** (a file system that refuses it), the snapshot re-reads every tracked file instead of trusting the recorded timestamps (`git add -A --renormalize`). That is correct, just slower, so the check is not stopped.
- **The behaviour users see does not change,** apart from the fix: no option, output or exit code changes.

### Out of scope

- **Edits that git itself would miss.** The snapshot trusts what the user's own index trusts, no more:
  - `assume-unchanged`;
  - a file-system monitor (`core.fsmonitor`);
  - timestamps a file system cannot tell apart, which `git status` would miss too.
- **Rehashing every file on every check.** It would be robust, but it costs time in large repositories, and it runs clean filters such as Git LFS for every file. It is only the fallback.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `revision-input`: the "Working tree as head" requirement gets a scenario for an edit that keeps the size and lands in the same timestamp. The requirement text already demands the right result ("every file as git would stage it from disk").

## Impact

- **Code:** `crates/qgate-git/src/worktree.rs` only:
  - how a private index is created and named;
  - `remove_stale`;
  - their tests.
- **Tests:** a new unit test in `qgate-git` and an end-to-end test in `qgate-cli` for the new scenario, both deterministic: they set the timestamps instead of racing the clock. The existing stale-copy test changes to name copies by their age.
- **No new dependency.** Setting a file's time is in the standard library (Rust 1.75), within the MSRV.
- **README:** no change. It already says that copies more than a day old are removed.

## Open questions

None for the human. See `design.md` for the technical choices.
