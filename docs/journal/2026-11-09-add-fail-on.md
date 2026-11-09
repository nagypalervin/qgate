# 2026-11-09: add-fail-on

**Went well:** A small change that stayed small. Four exploration
decisions (fail on Error by default, one `--fail-on` level, unparsable
files only at `warning`, a summary line on standard error) made the
proposal short and the code one crate. Mutation checks on the threshold,
the comparison, the plural, the broken pipe and `never` were each caught
by a test.

**Had to step in:** Only the four exploration choices and Gate 1. The
spec delta grew after review without a new gate: another capability
contradicted the approved behaviour, and the fix only aligned it.

**Surprise:**
- The first review found the contradiction in a spec the change did not
  list. `revision-input` still said an unparsable file exits with 0, and
  archiving would have put two opposite SHALL statements on `main`.
  Changing a default touches every spec that pinned the old one, not only
  the requirement that owns it.
- The first commit changed the default before the CI smoke step was
  updated, so that single commit fails CI. The repository merges with
  merge commits, so it stays in history; ordering the CI fix first would
  have avoided it.

**Effort:** one session; 1 PR, 2 review rounds.

**Blog idea:** "Breaking your own default before anyone depends on it":
why a quality gate should fail by default, and what a delta-only tool
gains from it.
