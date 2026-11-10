# 2026-11-10: fix-racy-worktree-snapshot

**Went well:**
- **A flaky test was taken seriously.** One red MSRV run on PR #6 passed on rerun. Instead of moving on, it was traced to a real bug: `--worktree` and the hook could miss an edit that kept a file's size. A hand reproduction showed `No regressions found.` for a +7 increase.
- **Deterministic tests instead of racing the clock.** The tests set the file's time, the index entry's time and the index's time themselves, with `core.trustctime=false`. They fail on the old code every time, unlike the test that found the bug.
- **Checking whether a loop proves anything.** The flaky test passed 300 times out of 300 with the fix, and also 300 out of 300 with the old code. The record says so, rather than calling the loop evidence.

**Had to step in:** These were the human's calls:
- starting the fix before `add-mcp-server`;
- Gate 1 and Gate 2;
- handing the agent the choice between an error and a fallback once the approved fallback turned out not to work;
- the merge.

**Surprise:**
- **The approved fallback was never tried before approval.** `git add -A --renormalize` had been checked for `skip-worktree` only. In apply it failed on files deleted from disk, and it skipped untracked files. The design changed to an error during apply.
- **The first reproduction attempts proved nothing.** They ran within the same timestamp tick as the setup, or after a `git status` that rewrote the index. The bug only showed once time had passed.
- **The review found two bugs that the fix itself had brought in:**
  - an older qgate's clean-up would have deleted the new, old-dated copies, so the copies got a new name;
  - an exact time comparison rejected a coarser, safe time.

  The verification round then found the one unsafe "earlier" time: 0 whole seconds, which git takes as no time.

**Effort:** one session, 1 PR, 1 review round and a verification round.

**Blog idea:** "A flaky test is a bug report." The test was right. It raced the clock in a way that a fast CI runner sometimes won, and that race was real for users too.
