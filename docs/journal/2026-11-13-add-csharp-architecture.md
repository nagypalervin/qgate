# 2026-11-13: add-csharp-architecture

**Went well:**
- **Namespaces as paths.** Namespace patterns check their spelling,
  then turn `.` into `/` and reuse `PathPatterns`. So `*`, `**`,
  braces and "a name also matches everything below it" work the same
  for both languages, without a second glob engine.
- **One rule id, one key space.** A `using` is C#'s import. Keeping
  `architecture/forbidden-import` with a `uses ...` message left the
  JSON schema, filters and severities unchanged, and C# reuses the
  analyzer's "only new violations" logic as it is.
- **The evaluation found a real one.** On the whole CleanArchitecture
  tree, a layer rule reported exactly one finding,
  `Application/GlobalUsings.cs` using EF Core. `grep` agreed, and the
  last 30 commits of both repositories gave no new noise.

**Had to step in:** These were the human's calls:
- the order of work for the partner pilot: C# rules first, then M5;
- Gate 1, with six questions, all answered as recommended;
- merging both pull requests, and publishing 0.6.0 after the agent
  pushed the tag.

**Surprise:**
- **The grammar names the alias `name`.** In tree-sitter-c-sharp's
  `using_directive`, the `name` field is the alias in `using X = A.B;`,
  not the namespace. A small spike crate showed this before any code
  relied on it.
- **The first timings were 20 to 40 times too slow.** About 3 s per
  commit came from a partial clone fetching blobs on demand. Re-timed
  once the blobs were local, it was 0.08 s and 0.16 s.
- **The binary did not grow.** The release binary is 11,211,840 bytes,
  the same as 0.5.0's; the archives grew by 6 to 9 KB.

**Effort:** one day, 2 PRs, 2 review rounds (one "changes needed" without
a blocker, one "ready"), one release, one Azure rehearsal.

**Blog idea:** "The evaluation is part of the feature." The rules were
done when the tests passed. They were trusted when the same rule run on
someone else's real layered code found the one using a human would also
call wrong, and nothing else.
