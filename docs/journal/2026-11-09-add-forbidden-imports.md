# 2026-11-09: add-forbidden-imports

**Went well:** Splitting the change into three pull requests that each
stood on its own: imports, then configuration, then the analyzer. Each
one had a clear reviewer brief and a small diff. Measuring dependencies
before deciding paid off again, twice:
- `globset` turned out to add two crates, not a regex engine, which
  overturned the plan to write our own glob matcher;
- the first `toml` count, about 11 crates, had been taken in a standalone
  crate. Against the real lock file it was 5.

**Had to step in:**
- Five exploration choices: architecture before configuration, deny
  lists, how imports resolve, the head revision's config, and what
  "new" means.
- Gate 1 and 2, including the edit of the reviewer agent's prompt.
- Two review questions on PR 3: how to treat pattern spellings that
  match nothing, and what to do with a JavaScript-to-TypeScript rename.

**Surprise:**
- PR 3 took five review rounds. Two of them existed only because the AI's
  own fixes were wrong:
  - Round 2 turned an unreadable old JavaScript file into
    `input/unparsable`. That quietly changed complexity results and exit
    codes for repositories without `qgate.toml`.
  - Round 3's fix for that resolved stand-in imports against the wrong
    path.

  Each fix was locally reasonable and tested, but the tests only checked
  the case the fix was about. Comparing the branch binary with a `main`
  binary on the same repositories is what caught it.
- "Silently matches nothing" was the theme of the change. A misspelled
  key, a trailing slash, a leading `./` or `.//` can each switch a rule
  off without a sound. Every one of them is now either normalized or an
  error.
- The reviewer read "rejecting a leading `/`" literally and found that
  `.//src` slipped through, because the check ran before `./` was
  stripped.
- `globset` 0.4.20 raised its MSRV to 1.88. The MSRV-aware resolver
  quietly locked 0.4.19, and nothing in CI would have noticed a broken
  MSRV. Building once with Rust 1.85 settled it; a CI job for it is still
  missing.

**Effort:** one session; 3 PRs, with 2, 2 and 5 review rounds.

**Blog idea:** "Diff the binaries, not just the tests": when a fix is
supposed to change nothing outside its case, run the old and new binary
on the same inputs and compare the bytes.
