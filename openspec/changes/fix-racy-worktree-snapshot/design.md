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
  - **`add -A --renormalize` every time.** It is correct, and it was checked by hand to keep `skip-worktree`. But it reads every tracked file and runs every clean filter (Git LFS) on each check, and the hook checks every turn. An earlier draft kept it as a fallback for D3, but it does not work as one (see D3).

### D2. A copy's age comes from its name

- **The new name:** `qgate-snapshot-index-<pid>-<seconds since 1970>-<subsec nanos>-<n>`. The seconds are the time the check started.
- **`remove_stale`:**
  - **Names of this form:** the copy is removed when the seconds in its name are more than a day ago.
  - **Names of earlier versions** (`qgate-index-<pid>-<nanos>-<n>`): the copy is judged by its modification time, as before. Earlier versions dated their copies when they made them, so that time is still right.
  - **Anything else, and times in the future:** the copy is kept.
- **Why the name:** with D1, the modification time is the user's index time. That time can be days old for a copy that a running check uses right now.
- **Why a new stem** (changed after review): earlier versions remove every `qgate-index-*` file whose modification time is more than a day old.
  - Under the old stem, an older qgate running at the same time could delete a new check's copy, for example a pinned version in a pre-commit hook next to the agent hook.
  - Without its copy, `add -A` starts from an empty index, and `write-tree` can even write the empty tree. That is a silent miss again.
  - Earlier versions do not match `qgate-snapshot-index-`, so they leave such copies alone.
- **Side effect:** the age is now measured with the local clock at both ends. On a network file system, the server's clock no longer matters.
- **Alternative:** a marker file next to each copy, holding its start time. That means two files per check, and more for an interrupted check to leave behind. A name cannot get out of step with its file.

### D3. If the time cannot be kept, the snapshot fails

- **When it applies:** the snapshot stops with a work-tree error in three cases:
  - reading the index's time fails: `cannot read the modification time of the index <path>: <reason>`;
  - setting the time on the copy fails;
  - the copy reads back a **later** time than the index's.

  The last two give `cannot give the private index <path> the index's modification time: <reason>`.
- **An earlier time is accepted** (changed after review). A copy dated earlier than the index makes git compare the content of at least the files it would compare with the index itself: more work, never a missed edit. Only a later time lets git trust a file it must read.
  - This matters where the copy's file system stores coarser times than the index's.
  - That can happen, because `--git-path index` follows `GIT_INDEX_FILE` while the copy stays in the git directory, so the two may be on different file systems.
  - The spec already covers this case ("Working tree errors": no snapshot of the working tree can be taken).
  - The hook shows it to the user, never to the model.
- **Why an error is acceptable:** the case should not happen.
  - A time can only be stored coarser, that is, truncated and so earlier, which is accepted.
  - Every common file system lets the owner set the time of a file it just created.
  - If the case does happen, an error is visible, while a check that goes on can miss an edit silently.
- **Why not a fallback:** the approved draft rehashed with `git add -A --renormalize`. In apply (task 1.3) it turned out not to work:
  - it stops with `fatal: unable to stat` when a tracked file was deleted from disk;
  - it implies `-u`, so untracked files are not added.

  A fallback that dates the copy in 1970, so that every entry is racily clean, would be more code for a case that should not happen. If time cannot be set at all, it would not help either. The human left this choice to the agent, and the error was chosen.
- **Testing:** a real refusal cannot be provoked on the CI file system. The comparison of the two times is a small function with its own unit test: equal and earlier times pass, and a later one fails.

### D4. Deterministic tests

The failing test raced the clock. The new tests set the times themselves:
- `core.trustctime=false` in the test repository, so that the ctime, which cannot be set, does not count;
- a file whose modification time is set to a fixed time `T` before `git add`, so that its index entry records `T`;
- an edit that keeps the size, written in place so that the inode stays the same, and then dated `T` again;
- the index file dated `T` as well, which makes the entry racily clean.

`git diff-files` lists the file. Before the fix, the snapshot does not. The same setup serves the unit test in `qgate-git` and the end-to-end test for the new spec scenario in `qgate-cli`.

## Implementation Notes

- **The order of the tasks.**
  - Task 1.2 alone would let `remove_stale` remove a running check's copy of an index that is more than a day old. So tasks 1.1 to 2.2 land in one commit, and no commit has that regression.
  - The pre-commit hook runs the tests, so the failing tests of 1.1 cannot be committed on their own.
  - Before that commit, both 1.1 tests were seen to fail on the old code, and to pass with only the copy's time changed.
- **The old test `private_index_is_dated_now_even_when_the_index_is_old`** asserted what D1 reverses. It became `private_index_keeps_the_index_time_and_survives_clean_up`. Its purpose is kept: a running check's copy of a three-day-old index survives another run's clean-up.
- **Names that do not parse:** names with other than three or four numbers, or with anything that is not a number, are kept. Before, every name with the prefix was judged by its time.
- **Mutations (task 2.2).** Each of these fails tests:
  - **dating the copy now:** 4 tests fail: both racy unit tests, the clean-up test of a live copy, and the end-to-end scenario;
  - **judging new-form names by their modification time:** 2 tests fail;
  - **a comparison that accepts a different time:** 1 test fails.

  The comparison was later changed to accept an earlier time; see "After review".

  Deleting the call to the comparison is not caught. A real refusal cannot be provoked on the CI file system, so no test reaches that path.
- **Split index.** The racy edit is also caught with `core.splitIndex=true`, and the test fails when the copy is dated now. Git compares the entries with the time of the main index file, which the copy keeps.

- **Verification (task 3.1).**
  - **The hand reproduction from the proposal,** with the new release build: a +7 increase in a file that kept its size, dated exactly as its index entry and the index.
    - `qgate check --base HEAD --worktree` reports it, and the hook blocks. That holds 7 seconds after the setup, when the old build reported `No regressions found.` in the same setup.
    - No private index is left behind.
  - **The loop proves nothing on this machine.** `snapshot_in_a_linked_worktree` passed 300 times out of 300 with the fix. It also passed 300 out of 300 with the old code from `main`, built in a temporary work tree.
    - The CI runner's timing does not occur on this development machine (Linux 7.0, ARM64 VM).
    - The evidence is the deterministic tests instead: both fail on the old code and pass on the new.
    - `snapshot_sees_a_racily_clean_edit` also passed 300 times out of 300.

- **After review.** The `reviewer` subagent found two gaps, both fixed:
  - **Earlier versions' clean-up.** An older qgate's clean-up could delete a new check's copy. New copies now use the stem `qgate-snapshot-index` (D2), and the README names both stems.
  - **Coarser time on the copy.** An exact read-back rejected a coarser, earlier time on the copy, although that is safe. Only a later time is now an error (D3). The test asserts both.

  Further changes from the review:
  - the error for a failed read of the index's time now names the index, not the copy;
  - new tests: a racily clean edit in a linked work tree, a recent lock file with an old modification time, a name with a future time, and the stem the copy must not start with.

  Each new behaviour bites:
  - **dating the copy now:** fails 4 unit tests, including the linked-work-tree one;
  - **exact equality in the comparison:** fails 1 test;
  - **using the old stem:** fails 2 tests.

## Risks / Trade-offs

- **[Split index]** With `core.splitIndex`, entries can live in a `sharedindex.*` file, which has its own time.
  - **Mitigation:** the snapshot already writes one plain file. Task 1.2 runs the new racy test with a split index as well, so the result does not depend on it.
- **[Copies left by earlier versions]** These keep the old name form.
  - **Mitigation:** D2 judges them by their modification time, as before. Task 2.1 tests both name forms.
- **[Earlier versions running at the same time]** Their clean-up judges copies by modification time.
  - **Mitigation:** new copies use a stem that earlier versions do not match (D2).
- **[A file system that cannot keep the time]** The check fails on every run there.
  - **Mitigation:** the error names the private index and the reason, so the case can be found and handled if it ever occurs. The reasoning in D3 says it should not occur.

## Migration Plan

None. Leftover copies in the old name form are still cleaned up (D2).
