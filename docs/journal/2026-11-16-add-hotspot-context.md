# 2026-11-16: add-hotspot-context

**Went well:**
- **The explore session found the shape before any code.** A hotspot
  is a state, and qgate reports deltas. Four short questions settled
  it: context on findings that already got worse, no finding or list of
  its own, annotation only, Tornhill's "complex and changed often". The
  CLAUDE.md rule against absolute scores ruled out a `qgate hotspots`
  command before it was proposed.
- **The reviewer mutated the code.** In PR 2 it changed the ranking and
  the window and reran the suite. One test turned out unable to fail (C#
  was never ranked), and two edges had no test at all (the "fewer than
  k%" rule and the window's 182 days). The fixes were checked the same
  way.
- **The data backed the decisions.** On full clones of four
  repositories, severity raising would have turned 49 Warnings into
  Errors in 60 commits; 39 of 56 notes sat on central code; the history
  read cost about 3% of a large check.

**Had to step in:** These were the human's calls:
- the explore session's four choices, Gate 1 with all ten
  recommendations, and Gate 2;
- leaving tests out of the ranking, after the evaluation;
- moving the coverage check to the roadmap's "Later" row once M5 was
  done;
- merging the pull requests, letting the agent push the tag, and
  publishing. He also asked why he should paste the release notes
  himself; the agent now puts them into the draft.

**Surprise:**
- **Git history is a graph, not a list.** The first parser mapped
  renames with one "newest name" table and lost commits from parallel
  branches, which a no-fast-forward merge workflow, the Azure default,
  makes common. The fix follows the graph. The one case git cannot show
  cheaply, a rename on another branch than the edits, is a documented
  and tested limit.
- **A design claim was false and nobody had run it.** "`--since` plus a
  Rust filter keeps the window exact under clock skew" was wrong:
  `--since` with `--topo-order` hides what lies behind an older commit.
  The reviewer reproduced it in a few commits.
- **A failed `cd` let git commands run in the wrong repository.** A
  scratch clone had vanished, and a `;`-chained `git switch main; git
  reset --hard origin/main` ran in qgate itself. The checkout was
  switched back and nothing was lost, only because `main` already
  equalled `origin/main`. Commands against other repositories now take
  `git -C`.

**Effort:** three calendar days, 3 PRs, 3 review rounds (one with mutation
testing), an evaluation on four full clones run twice, one release, one
rehearsal run.

**Blog idea:** "Make the reviewer try to break the tests." A review
that reads code finds design gaps; one that changes `<` to `<=` and
watches the suite stay green finds the tests that were only decoration.
