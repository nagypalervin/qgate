# 2026-11-16: add-rust-analysis

**Went well:**
- **The adapter pattern carried a third language in one day.** Units,
  decision points, tokens and fingerprints followed the C# adapter; the
  analyzers needed no change, the CLI a few lines (`Family::ALL` in the
  duplication loop, the two-file form's messages). A spike before the
  proposal parsed all 39 `.rs` files of the repository, so the grammar
  risk was measured, not guessed.
- **The replay turned "does it work on real code?" into numbers.** qgate
  over all 52 merges of its own history took 3.85 s and gave 218
  findings: 62 in fixtures (all noise), 73 in test code, 83 in source.
  Five of six source Errors were functions a reviewer would want to
  see, and all six came with hotspot context: each sat in a file that
  was both complex and changed often.
- **The reviewer reran the replay** and got the same numbers, then found
  the two results the design had promised and the write-up had skipped
  (macros, integration tests among hotspots). Both were measured before
  the merge: 1.3% of decision tokens hide in macro arguments; an
  integration test sat in the churn top in 36 merges and pushed no
  source out.

**Had to step in:** These were the human's calls:
- inserting Rust and a self-check before the partner evaluation, and the
  scope (complexity and duplication; replay, CI gate and hook);
- Gate 1 with all six recommendations;
- two spec ambiguities the review of PR 1 found: a `const` inside a
  function is its code, and a tuple field `.0` keeps its text;
- after the replay: D3 and D6 stay, and the repository excludes its
  fixtures;
- merging the pull requests, letting the agent push the tag, and
  publishing.

**Surprise:**
- **qgate caught the agent copying itself.** The C# adapter had copied
  the TypeScript adapter's walker skeleton in 9 places; PR 1 of this
  change did it again, 9 more copies. Neither the reviewer subagent nor
  the human raised it; the replay did. `github.rs` turned out to be a
  611-token copy of `azure.rs` from an earlier change.
- **A new enum variant is a "complexity increase".** 12 of 57 source
  Warnings came from `match`es that list every variant and got one more
  arm, which rustc demands. True by the definition, useless in review.
  The rule stays, consistent with `case` in TypeScript and C#, and the
  README says so.
- **The new machine's compiler found what qgate does not look for.**
  Rust 1.100 warns that `qgate-lang` does not use its `qgate-core`
  dependency, a decay of the kind qgate is meant to catch, invisible to
  qgate because it lives in a manifest, not in code.

**Effort:** one day, on a freshly set-up machine; 3 PRs, 3 review rounds,
a replay of 52 merges run twice (once by the reviewer), one release.

**Follow-ups for Ervin to order:**
- a shared walker for the three adapters (19 duplicates in the replay);
- the `Repo` test helper in one place (it lives in `qgate-git` and in
  the CLI's tests);
- the unused `qgate-core` dependency of `qgate-lang`;
- whether the `qgate` CI job becomes a required check;
- test code: 73 of the 218 findings, mostly duplication in integration
  tests; skipping `#[cfg(test)]` modules would remove only 17, so D6
  stays, but excluding `crates/*/tests/**` is the next lever if the CI
  gate gets noisy;
- Rust test analysis and `use` rules: the replay gives no strong case
  for either yet;
- then, as planned, the Angular + .NET evaluation for the partner pilot.

**Blog idea:** "Point the tool at its own history first." Replaying the
merges of the tool's own repository is the cheapest honest evaluation:
you know which findings are right, the noise has names (fixtures, enum
variants), and the tool catches its author, here an AI agent, repeating
itself.
