# 2026-11-09: add-worktree-input

**Went well:** The snapshot idea held up:
- **Design:** the working tree becomes a commit that no ref points to, built through a private index. Every analyzer then reads it unchanged.
- **PR 2:** the CLI part was small, because the hard part was already in `qgate-git`.
- **Mutation checks:** they became routine. Every protection was broken on purpose at least once, and two options turned out to be dead code (`--no-gpg-sign`, `add.ignoreErrors=false`), so they were removed instead of kept "just in case".

**Had to step in:** These were the human's calls:
- making the repository public, and how to rewrite and move its history first;
- the `--worktree` flag instead of a new default;
- Gate 1 and Gate 2;
- the merges.

The review rounds needed no decision from the human.

**Surprise:**
- **The dangerous bugs were in the clean-up, not the snapshot.**
  - A split index leaked a `sharedindex` file on every run.
  - A killed run left a full index copy that nothing removed.
  - The age-based clean-up that fixed it could delete a running check's index on macOS and Windows, because `std::fs::copy` keeps the source's timestamp there. Git reads a missing index as an empty one, so the check would have given a wrong result without any error.
  - Linux CI cannot see this last bug, so a macOS job is still worth adding.
- **The spec relied on behaviour clap does not have.** clap ignores `requires("base")` when `--base` itself conflicts with an argument that was given. So `--worktree` next to two files would have been accepted, and `qgate check a.ts b.ts --head x` had been silently ignoring `--head` since M1.
- **Tests that matched the scenario text guarded nothing.**
  - The hook test listed `pre-commit` and `post-commit`, which `git add` never runs.
  - The duplication test paired two changed files, so it never read the head tree.

  The reviewer found both by breaking the code and seeing every test stay green.

**Effort:** one long session, 2 PRs:
- PR 1: 3 review rounds and a verification round;
- PR 2: 1 review round and a verification round.

**Blog idea:** "Your test matches the scenario. Does it fail when the code breaks?" A scenario can be satisfied by a test that would also pass with the feature removed. Only a mutation shows the difference.
