# Design: fix-racy-worktree-snapshot

## Context

- **The snapshot** (`crates/qgate-git/src/worktree.rs`) works as follows:
  - `PrivateIndex::create` copies the user's index into a new file `<git dir>/qgate-index-<pid>-<subsec nanos>-<n>` and sets that file's time to now;
  - `add -A` then runs against the copy, followed by `write-tree` and `commit-tree`;
  - without an index, `read-tree <head>` writes a new one instead.
- **Why the copy is dated now.** `remove_stale` removes copies whose modification time is more than a day old. These are copies that interrupted checks left behind (spec "Interrupted checks"). Dating the copy now keeps a running check's copy safe (commits b1bbd17 and 04789fc).
- **How git decides whether a file changed.** It compares the file's stat data (modification time, size, inode, and ctime unless `core.trustctime=false`) with what the index entry recorded. If the entry's modification time is not older than the index file's own modification time, the entry is "racily clean": git cannot trust the stat data, and it compares the content.
  - Dating the copy later than the original index turns such entries into trusted ones.
  - That is the bug; see proposal.md, "Why".

## Goals / Non-Goals

**Goals:**
- A snapshot stages exactly what `git add -A` would stage from the user's index at that moment.
- A running check's copy is never removed by another check, as now.

**Non-Goals:**
- Being more careful than git. What `git status` would miss (see the proposal's out-of-scope list) the snapshot may miss too.
- Changing the snapshot's other guarantees: no hooks, no split index, no change to the user's index, refs or files.

## Decisions

### D1. The copy keeps the original index's modification time

- **What happens:**
  - after copying, the copy's modification time is set to the original index's, and read back;
  - git then sees the same racily clean entries in the copy as in the original, and compares their content.
- **Alternatives:**
  - **`git update-index --refresh` on the copy.** It decides with the same stat comparison, so it misses the same edits.
  - **A fresh index from `read-tree HEAD`.** Every file would be hashed again. The marks the spec relies on would also be lost: `skip-worktree`, `assume-unchanged` and intent-to-add.
  - **`add -A --renormalize` every time.** It is correct, and it was checked by hand to keep `skip-worktree`. But it reads every tracked file and runs every clean filter (Git LFS) on each check, and the hook checks every turn. It is kept as the fallback (D3).

### D2. A copy's age comes from its name

- **The new name:** `qgate-index-<pid>-<seconds since 1970>-<subsec nanos>-<n>`. The seconds are the time the check started.
- **`remove_stale`:**
  - **Names of this form:** the copy is removed when the seconds in its name are more than a day ago.
  - **Names of the old form** (`<pid>-<nanos>-<n>`, one number fewer): the copy is judged by its modification time, as before. Earlier versions dated their copies when they made them, so that time is still right.
  - **Anything else, and times in the future:** the copy is kept.
- **Why the name:** with D1, the modification time is the user's index time. That time can be days old for a copy that a running check uses right now.
- **Side effect:** the age is now measured with the local clock at both ends. On a network file system, the server's clock no longer matters.
- **Alternative:** a marker file next to each copy, holding its start time. That means two files per check, and more for an interrupted check to leave behind. A name cannot get out of step with its file.

### D3. If the time cannot be set, rehash

- **When it applies:** setting the time fails, or the time read back differs from the original's, for example on a file system that stores coarser times. Then the snapshot runs `add -A --renormalize` instead of `add -A`.
- **What that does:** git rereads every tracked file and stages its content, whatever its stat data says. A manual check confirmed that it still leaves `skip-worktree` files alone. Task 1.3 verifies `assume-unchanged` the same way.
- **Why not an error:** the check stays correct, only slower, on a rare file system. An error would show up in every agent turn.
- **Testability:** an internal parameter forces the fallback in tests, because a real refusal cannot be provoked on the CI file system.

### D4. Deterministic tests

The failing test raced the clock. The new tests set the times themselves:
- `core.trustctime=false` in the test repository, so that the ctime, which cannot be set, does not count;
- a file whose modification time is set to a fixed time `T` before `git add`, so that its index entry records `T`;
- an edit that keeps the size, written in place so that the inode stays the same, and then dated `T` again;
- the index file dated `T` as well, which makes the entry racily clean.

`git diff-files` lists the file. Before the fix, the snapshot does not. The same setup serves the unit test in `qgate-git` and the end-to-end test for the new spec scenario in `qgate-cli`.

## Risks / Trade-offs

- **[Split index]** With `core.splitIndex`, entries can live in a `sharedindex.*` file, which has its own time.
  - **Mitigation:** the snapshot already writes one plain file. Task 1.2 runs the new racy test with a split index as well, so the result does not depend on it.
- **[Copies left by earlier versions]** These keep the old name form.
  - **Mitigation:** D2 judges them by their modification time, as before. Task 2.1 tests both name forms.
- **[A slow fallback]** `--renormalize` costs time in large repositories.
  - **Mitigation:** it only applies where file times cannot be set, and a check measured on such a file system can be revisited then.

## Migration Plan

None. Leftover copies in the old name form are still cleaned up (D2).
