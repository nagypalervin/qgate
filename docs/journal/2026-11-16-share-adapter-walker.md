# 2026-11-16: share-adapter-walker

**Went well:**
- **A refactor with a mechanical proof.** The replay of
  `add-rust-analysis` became the regression test: 54 merges of `main`,
  checked with the binary before and after, gave byte-identical reports
  (229 findings over TypeScript, C# and Rust code). Together with the
  unchanged unit tests, no reviewer had to trust the refactor.
- **qgate measured its own clean-up.** Counting the copies among the
  adapters (an empty commit, then the sources, `qgate check --base
  HEAD~1`) gave 30 before: 18 in source code, 12 between test modules.
  After: 4 in source code, the 12 unchanged.
- **Small steps held.** Rust first, then C#, then TypeScript, each
  commit with the suites unchanged; the TypeScript walker, the oldest and
  most special, fit the shared unit bookkeeping without a change of
  behavior.

**Had to step in:** These were the human's calls:
- choosing to tidy up next, at the agent's discretion, and Gate 1 with
  the three recommendations;
- whether the four remaining copies are acceptable (put to him in the
  pull request), and merging.

**Surprise:**
- **The refactor made a new copy.** After moving all three walkers, their
  `parse` functions had become the same; the count showed it, and a
  shared `run` replaced them.
- **qgate flagged the refactor's own renames.** Splitting `impl<'s>
  Walker<'s>` into two blocks, one with `'_`, made `record_token` a "new
  function" in all three adapters: a Rust unit's name holds the impl's
  type as written, lifetime included. One inherent impl again kept the
  names; the naming rule itself needs a look.
- **Chasing the detector to zero would have hurt.** Three remaining
  copies are short idioms around each adapter's own frame type; removing
  them would need an abstraction whose only purpose is to move tokens
  below the threshold.

**Effort:** a few hours; 1 PR, 9 commits, the replay run three times.

**Follow-ups for Ervin to order:**
- Rust names without lifetime arguments (`Walker.record_token`), so a
  lifetime rename is no new function: a change to `rust-analysis`;
- the `Repo` test helper in one place (still open from
  `add-rust-analysis`).

**Blog idea:** "Use the tool's history as the refactor's test." When a
tool can replay its own merged pull requests, a refactor of the tool
gets a free, exhaustive regression test: same input, same bytes out.
