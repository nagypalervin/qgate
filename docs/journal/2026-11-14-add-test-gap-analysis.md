# 2026-11-14: add-test-gap-analysis

**Went well:**
- **The evaluation earned its place.** Sorting 67 findings from four
  real repositories by hand found what no fixture had: a project and
  its test project spelled `Sso` and `SSO.Test`, test helpers in
  `spec/` and `e2e/` taken for sources, and acceptance test projects
  named as the place for a missing class test. Three small rules came
  out of it, and a rerun showed exactly the eight intended findings
  gone and nothing new.
- **The fixture pairs pinned a fuzzy idea.** "Did this function
  change?" became a fingerprint, and a before/after pair per language,
  one kind of edit per function, says exactly which edits count.
- **The review of PR 2 read the evaluation, not just the code.** It
  added up every count of the record and asked for the per-finding list
  and the scripts to be kept, so the numbers can be checked from the
  repository. The review of PR 3 caught that the release notes, the
  promised warning for `--fail-on warning` users, never reached the
  release page.

**Had to step in:** These were the human's calls:
- Gate 1, with all nine recommendations, and Gate 2;
- whether TypeScript type annotations count as a change: the tasks and
  the design disagreed, and the agent asked before writing code;
- the three conventions after the evaluation;
- rehearsing `missing-test` instead of `unchanged-test` in Azure,
  rather than merging a setup pull request into the rehearsal `main`;
- merging the pull requests, letting the agent push the tag, publishing,
  and adding the release notes afterwards. Once "merged" arrived before
  the merge had gone through on GitHub; the agent checked the pull
  request's state before tagging.

**Surprise:**
- **The planning documents contradicted themselves before any code.**
  The first contradiction (task 1.1 against D1 on types) surfaced as
  soon as the walk was read; a second one (`.mts` in D3 against D6)
  only in review.
- **The reviewer found real misses in the fingerprint.** JSX text
  reflowed, a swapped `withAuth` wrapper and a changed `@HostListener`
  all looked like no change. The design said "the walk's tokens", and
  the walk does not see a decorator, which the grammar puts before the
  method.
- **Most findings need no test, and that is fine.** About half pointed
  at a test that should have changed; most of the rest were composition
  roots, renamed i18n keys and response models. A Warning with the
  expected test file in its message, plus `[tests] exclude`, seems the
  right weight for that.
- **The agent's own slips:** a blanket version replace that also
  rewrote the roadmap's history, code fences cut out of the README along
  with an install block, and an evaluation helper that left the clones
  on the wrong commit. Each was caught by looking at the output before
  moving on.

**Effort:** one long day, 3 PRs, 3 review rounds, an evaluation on four
repositories (twice), one release, one rehearsal run.

**Blog idea:** "Let real repositories write your edge cases." Fixtures
test what you thought of; 117 commits of other people's code showed
the three conventions nobody had thought of, and the hand sort said
which findings were noise.
