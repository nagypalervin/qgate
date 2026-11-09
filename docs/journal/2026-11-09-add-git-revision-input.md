# 2026-11-09: add-git-revision-input

**Went well:** Measuring before deciding. A throwaway crate showed that
`gix` would add 131 to 202 crates, which made "call `git`" an easy ADR
instead of a debate. Temporary git repositories made every git status,
rename and merge-base case testable end to end, and mutation checks again
showed which tests actually bite.

**Had to step in:** Five spec questions the AI surfaced but could not
decide: the CLI shape and merge-base semantics, unparsable files as
findings, a symlink replaced by a file, path sort order, and whether the
reviewer agent's own prompt may be edited. All five were quick choices
between two prepared options.

**Surprise:**
- The first review found a real blocker in code that "obviously" worked.
  With `diff.relative=true` in a user's git config, running from a
  subdirectory silently dropped every file outside it. The tests had not
  caught it because the code under test was not isolated from git config
  the way the fixtures were.
- The AI claimed an option-injection bug, then disproved its own claim by
  probing, and corrected the record.
- Twice a compound shell command committed after a failed step, because a
  `;` broke the `&&` chain. Both commits were local and amended before
  pushing, but it is the same mistake as in the previous change, in a new
  form.

**Effort:** one session; 2 PRs, 2 review rounds each.

**Blog idea:** "Your git config is part of your test environment": how
`diff.relative`, `GIT_INDEX_FILE` in hooks and template directories leak
into tools that shell out to git.
