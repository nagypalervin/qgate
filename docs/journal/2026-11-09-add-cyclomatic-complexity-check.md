# 2026-11-09: add-cyclomatic-complexity-check

**Went well:** The full OpenSpec loop on a real feature: explore, propose,
three small PRs, archive. Explore settled the hard questions (metric,
matching, what counts as worse) before any code, so apply was mostly
mechanical. Using ESLint's `complexity` rule as the reference made the
counts checkable: every decision-point count matched on the first run.
Mutating the code (removing the sort, flipping `>` to `>=`) was a cheap
way to prove that tests written after the code still bite.

**Had to step in:** Four times, all spec questions the AI surfaced but
could not decide: `satisfies`/`as` hiding a function's name (look through
them), module-level wrapper calls like `withAuth(async () => ...)` (out of
scope, follow-up change), class field initializers vs ESLint (accepted),
duplicate-name suffixes shifting (known limitation). The AI accepted the
third one on its own first, although its task said to stop and ask; the
reviewer caught it.

**Surprise:** The reviewer subagent earned its keep. Each PR's first
review said "changes needed", and each time for real: a naming collision,
an untested severity boundary, a crash on `| head`, and a task ticked
before its check could have run. Also: a commit went in with failing
clippy because the check command was not chained with `&&`; it was
amended before pushing, and the checks are chained since.

**Effort:** one long day; 3 PRs, 2 review rounds each.

**Blog idea:** "The reviewer that never saw the conversation": why a
clean-context review agent finds what the implementing agent rationalized.
