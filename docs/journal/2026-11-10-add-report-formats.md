# 2026-11-10: add-report-formats

**Went well:**
- **The spec fixed exact bytes.** Each kind of finding has a scenario with the full JSON line, so the renderer's unit tests and the end-to-end tests were written straight from the spec, and implementing the renderer took one pass.
- **No new crate.** `serde_json` and all of its dependencies were already locked as tree-sitter's build dependencies, so a public JSON format came without growing the build.
- **Splitting the change in two.** PR 1 put the structure into the core types (`end_line`, `related`) with the text output unchanged, so PR 2 was only the format.

**Had to step in:** These were the human's calls:
- Gate 1 and Gate 2, including one document with an envelope instead of JSON Lines, and related locations from version 1;
- the merges.

The review rounds needed no decision from the human.

**Surprise:**
- **Most review findings were about what is frozen, not about bugs.** The code was right in every round. What changed was the contract around it:
  - `summary.findings` became `summary.count`;
  - new rule ids are allowed within version 1, new severities are not;
  - a metric with a value that is not finite is left out as a whole, so an absent `before` keeps its one meaning.
- **"Exactly what the spec says" was harder for the schema than for the code.** `minProperties: 1` looked like "has `before` or `after`" until the reviewer pointed out that an unknown key alone satisfies it. The exact rule needed `anyOf`.
- **A guard against unsupported keywords was not enough.** The hand-written validator's guard checked keyword names, but a `type` given as an array would have been skipped silently. It now checks value shapes too.
- **A scenario that git satisfies on its own.** "Several files" passed with the sort removed, because git already lists `src/a.ts` before `src/b.ts`. Only a finding that is collected later (a duplicate block) makes the test need the sort.

**Effort:** one long session, 2 PRs:
- PR 1: 1 review round and a verification round;
- PR 2: 1 review round and a verification round.

**Blog idea:** "Version 1 is the expensive part." Writing the JSON took an hour. Deciding what it promises (absent versus `null`, which lists may grow, what a missing value means) took the reviews. Every promise had to be pinned down twice, in the spec and in a schema a machine can check.
