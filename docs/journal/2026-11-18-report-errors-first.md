# 2026-11-18: report-errors-first

**Went well:**
- **A lesson from a real stream of pull requests, kept generic.** The
  pilot measurement found the cause in one sentence: the comment cut its
  table in path order, so on large pull requests the Errors fell below
  the cut. The fix and its tests needed nothing from that project; the
  scenarios use made-up paths and rules.
- **Small scope by design.** Only the Markdown renderer changed. Text,
  JSON and the CI annotations keep their order, and an end-to-end test
  pins that, so no consumer had to change.
- **Mutants by hand were quick.** Six mutants of the order and the rule
  summary, each killed by a test of `markdown.rs`.

**Had to step in:** Ervin's calls:
- the fix order of the pilot lessons, this one first;
- Gate 1, with all four recommendations: severity order, the rule
  summary only when rows are hidden, a bullet list, one pull request
  and a release;
- merging and publishing.

**Surprise:** The public replay could not show the problem: only one of
the 150 evaluation pull requests had more than 50 findings. Open-source
projects merge small pull requests; a team's integration branches do
not. A replay set chosen for its language mix said little about size.

**Effort:** one session. Tests first, six mutants, one review round.

**Blog idea:** "The first 50 rows are the report." Nobody scrolls past a
cut, so the order of a truncated list is a decision about what the
reader sees, not a detail of formatting.
