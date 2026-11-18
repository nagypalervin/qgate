# 2026-11-18: evaluate-angular-dotnet

**Went well:**
- **Criteria fixed before the first run.** Ervin set the four go
  criteria and the configurations were committed before any finding was
  seen. So the no-go verdict needed no debate: three criteria failed,
  and each failure pointed at a mechanism, not at the idea.
- **An evaluation that turned into a work list.** The judged sample
  named the two fixes and their order (the only false Error first). It
  also gave each fix its replay bar: `complexity-of-moved-code`,
  `limit-move-evidence` and `duplication-of-new-code` were each measured
  against the same 150 pull requests and the same judgements.
- **The second measurement was cheap.** The same scripts, clones and
  configurations, with the 0.10.0 release, and a finding-by-finding
  comparison (`remeasure.py`). No finding appeared that the first run
  did not have, so the old judgements still covered everything. False
  Errors went from 1 to 0, and useful or defensible from 69% to 96%.
- **Ervin's spot-check agreed on 10 of 10,** so the agent's judgements
  stood without a second round.

**Had to step in:** These were Ervin's calls:
- Gate 1: the stand-in repositories, the criteria and their thresholds,
  the agent judging with a spot-check, and a comment-only pilot;
- after the verdict: the complexity fix first, the comment rate of
  `bitwarden/clients` judged on the partner's own pull requests, and
  accepting a known violation as a condition of blocking;
- in the fixes: the decisions recorded in their own journals;
- merging.

**Surprise:**
- **Layer rules copied from the compiler say nothing.** Every merged
  pull request had already compiled and passed lint, so the
  project-reference rules never fired. The one architecture finding
  was a violation the team had allowed with `eslint-disable`, which
  qgate cannot accept yet.
- **The comment rate is not a noise problem.** In `bitwarden/clients`
  it stayed at 44% after the fixes. Those comments are correct but small
  findings: test files not changed with the code, and +1 complexity.
- **Azure Repos writes pull request commits differently.** The scripts
  only knew GitHub's `(#N)`; the pilot needs `Merged PR N:`, squashed or
  merged.

**Effort:** two days around the fixes. 2 measurements of 150 pull
requests, 78 findings judged, 3 fix changes in between (5 PRs, a
release), one review round.

**Blog idea:** "Evaluate the tool the way the team will see it." Each
analyzer had passed its own evaluation. Only one comment per pull
request, over a stream of real pull requests, showed where the noise
came from: code that a change only moved.
