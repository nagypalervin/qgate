# 2026-11-13: add-csharp-analysis

**Went well:**
- **A spike before the proposal.** Twenty minutes with the grammar
  showed that C# 12, a byte order mark and CRLF parse, and that an
  `#if` whose branches open blocks does not. The proposal could then
  ask the human about real limits instead of guesses.
- **The analyzers did not change.** ADR 0001 said a new language is "an
  adapter and fixtures"; that held. The C# adapter fills the same
  model, and the complexity and duplication analyzers passed their new
  C# fixtures untouched.
- **Evaluation on other people's code before the release.** 30 commits
  each of two public repositories: no crash, 0.1 s a commit, ten
  findings checked by hand against the diffs. A second look, asked for
  in review, found the one real problem (EF Core migrations), and the
  human could decide about it before the tag.
- **Mutations as a review tool.** The reviewer removed single checks
  from the new CLI code and ran the tests; four removals survived. Four
  small tests later, each of them fails.

**Had to step in:** These were the human's calls:
- dropping SARIF from M4 and choosing C# as the next work;
- Gate 1 with six questions, and later accepting that more `#if` forms
  than planned are unparsable;
- releasing 0.4.0 with migration duplicates and an exclude list next;
- letting the agent push the tag, and publishing.

**Surprise:**
- **The grammar's quirks shaped the spec.**
  - Code inside `#if` branches is nested in the directive's node, so
    muting directives hid the code (a review blocker).
  - A missing identifier reports an error that neither `is_missing` nor
    `is_error` shows, so `x = ;` passed as valid.
  - `scoped` belongs to the type, and `params` sits outside its
    parameter node.
- **Each review found what the author's tests had agreed with.** The
  author's tests checked the author's reading of the spec; the reviewer
  read the grammar and the spec separately and found the gaps.
- **A crash during `git checkout -b`** left an empty branch ref and
  NUL bytes in the reflog. `git fsck` showed the objects were intact,
  and writing the ref by hand repaired it; the scratchpad, in `/tmp`,
  was lost and its helpers were rewritten.

**Effort:** one day, 3 PRs plus a roadmap PR, 3 review rounds, one
spike, one evaluation, one release, one Azure run.

**Blog idea:** "Run it on someone else's code before you tag." The
tests passed, the reviewers agreed, and two public repositories still
found the noise a partner would have seen first.
