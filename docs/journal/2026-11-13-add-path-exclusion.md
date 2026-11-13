# 2026-11-13: add-path-exclusion

**Went well:**
- **Moving the pattern code without changing it.** The architecture
  analyzer's glob code became a shared `PathPatterns` module. Its
  existing tests were the check, and the reviewer compared the
  architecture error messages of `main` and the branch byte for byte.
- **One gate instead of many branches.** "Excluded" became "not
  supported" in `FileSelection`, the one place that already decided
  what takes part. Renames, deleted files, the duplication corpus and
  the JavaScript base followed without new code paths.
- **The problem came from the evaluation.** The migrations that the C#
  evaluation flagged were the fixture of the first test, and the
  release was tried on the same case before Ervin published it.

**Had to step in:** These were the human's calls:
- Gate 1, with five questions, all answered as recommended;
- merging, and publishing 0.5.0 after the agent pushed the tag.

**Surprise:**
- **Mutations found the untested callers.** The reviewer removed each
  `FileSelection` call in turn. The two for deleted files survived: no
  test deleted an excluded file.
- **The task asked for a comment the spec says never comes.** A pull
  request that adds a migration and its exclusion together has no
  findings, so the action writes no comment, as the comment spec says.
  Pushing the setting onto the other rehearsal pull request instead
  showed something better: an existing comment turning into "No
  regressions found."

**Effort:** one day, 2 PRs, 2 review rounds, one release, one GitHub
rehearsal.

**Blog idea:** "Write the rehearsal against the spec, not against your
picture of the product." The task's author had imagined two comments.
The product, doing what its spec said, produced one.
