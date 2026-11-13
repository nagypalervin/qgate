# Design: add-path-exclusion

## Context

- **`qgate-config`** (ADR 0003) turns `qgate.toml` into plain data with
  `deny_unknown_fields`. Its only table is `[architecture]`. Pattern
  syntax is checked where patterns are interpreted.
- **The architecture analyzer** owns the pattern code: `normalize` (the
  spelling rules), `compile` into a `globset::GlobSet` (a plain pattern
  also matches below it; `dir/**` also matches `dir`), and a
  `PatternError` that names the rule.
- **The CLI** loads the configuration from the head revision before it
  analyzes any file (`load_config`). It decides what takes part through
  `language_of(path)`, which `add-csharp-analysis` already extended:
  - generated C# names are "not supported";
  - every caller relies on it: changed files, renames (`left`), the
    duplication corpus, deleted files, and the base path in
    `file_change`.

## Goals / Non-Goals

**Goals:** one rule, "excluded is unsupported", applied at the one place
that already decides what is supported.

**Non-Goals:** excluding import targets, include lists, per-analyzer
settings.

## Decisions

### D1. `[files] exclude` in `qgate-config`

- **Shape:** `Config` gets a `files: Files` table with
  `exclude: Vec<String>`, defaulting to empty; unknown keys are still
  rejected.
- **Empty list:** allowed and excludes nothing. An empty pattern string
  is rejected by the pattern rules, like an empty `from` pattern.
- **Why a table (Gate 1, question 1):** a later file setting (an
  include list, a size limit) has a home, and `[files]` reads as what
  it governs.

### D2. Path patterns become a shared module in `qgate-analyzers`

- **The move:** `normalize`, the compile step and `glob_of` move from
  `architecture.rs` to `patterns.rs` as `PathPatterns`:
  - `PathPatterns::new(&[String]) -> Result<PathPatterns,
    InvalidPattern>`;
  - `matches(&self, path) -> bool`;
  - `InvalidPattern` holds the pattern and the detail.
- **The architecture analyzer** wraps `InvalidPattern` into its
  `PatternError` with the rule's name, so its messages do not change.
- **Why `qgate-analyzers`:** the CLI already depends on it.
  - `qgate-core` holds types only.
  - `qgate-config` stays a format crate (ADR 0003: syntax is checked
    where patterns are interpreted).
  - A crate of its own for one module would be more structure than it
    is worth.
- **Behavior unchanged:** the existing architecture tests are the check.

### D3. "Excluded" is "not supported", in one place

- **`FileSelection`:** the CLI builds one from the configuration: the
  compiled exclude patterns. It offers `language_of(path)`, which
  returns `None` for an excluded path and otherwise behaves as today.
  Every current caller of the free function switches to it, so each
  path (changed, renamed-from, deleted, corpus) is judged by its own
  path, as generated names are.
- **What follows from the existing code without new branches:**
  - **A changed file:** excluded at its head path, it is skipped; if it
    was renamed from an included path, its old code counts as code that
    left.
  - **The base side:** excluded at its old path, the file counts as
    added.
  - **The duplication corpus and deleted files:** excluded paths take
    no part.
  - **JavaScript renamed to TypeScript:** an excluded `.js` base is no
    base either. `is_javascript` is consulted only after the selection.
- **Silent (Gate 1, question 2):** nothing is printed for an excluded
  file, as for unsupported and generated ones. A count would be the
  first output qgate prints about files it does not analyze.

### D4. The configuration comes from the head revision (Gate 1, question 5)

- **The rule:** exclusion uses the same `qgate.toml` as the architecture
  rules, read from the head revision (configuration spec), so
  `--worktree`, the hook and the MCP server need nothing new.
- **The risk:** a pull request can exclude its own files. The change to
  `qgate.toml` is then in its diff, as a removed forbid rule is today.
- **Rejected:** reading the base's list. It would split one file into
  two revisions, and a pull request that adds a migration and its
  exclusion together would still report it.

### D5. Errors

- **The message:** an invalid exclude pattern fails before any file is
  analyzed, with exit code 2:
  `qgate.toml: files.exclude: invalid pattern "/src/**": patterns are
  relative to the repository root; remove the leading `/``.
- **Wording:** the same as a rule's, with `files.exclude` where a rule
  has `rule "<name>"`.

### D6. Two pull requests and 0.5.0

1. **PR 1, `change/add-path-exclusion`:**
   - D1 to D5, with fixture and CLI tests;
   - the README's `qgate.toml` section and its C# note on migrations;
   - version 0.5.0 in `Cargo.toml` and `action.yml`.
2. **Release:** Ervin agrees, the agent pushes `v0.5.0`, Ervin
   publishes.
3. **PR 2, `change/add-path-exclusion-2`:**
   - the README versions at 0.5.0;
   - the release record;
   - the GitHub rehearsal (Gate 1, question 4);
   - the journal entry and the archive.

## Review of PR 1

The `reviewer` subagent compared the architecture error messages of
`main` and the branch byte for byte, and mutated each `FileSelection`
call. The ones it could observe died, except the two for deleted files:
no test deleted an excluded file. Fixed with a test for a deleted
excluded `.ts` and `.js` file, which kills both, and one for a file
renamed into an excluded directory. The spec now says that a deleted or
renamed JavaScript file is no earlier code when excluded, and adds a
scenario for it.

Nits fixed:
- the scenario "An excluded file is no original" is written in the
  test's language;
- task 1.2's text names the one change to the architecture tests'
  helper;
- the crate doc lists `patterns`.

## Release 0.5.0 (task 2.1)

- PR #24 merged as d408ea1. As Ervin agreed, the agent pushed the
  annotated tag `v0.5.0` ("qgate 0.5.0") after `check-version.sh` and
  `check-action-version.sh` accepted it on `main`.
- Release run 43424632880: all nine jobs green.
- The draft held six archives and `SHA256SUMS`; `sha256sum --check`
  passed for all six. The Linux arm64 archive holds the binary,
  `qgate-pr-comment.ps1` (byte for byte the one on `main`), `README.md`
  and both licenses. Its binary (11,211,840 bytes) printed
  `qgate 0.5.0`. In a throwaway repository, on a new migration
  repeating an older one, it reported a duplicate Warning and a new
  method of complexity 26 without the setting, and "No regressions
  found." (exit 0) with `exclude = ["**/Migrations/**"]`.
- Archive sizes in bytes against 0.4.0; the change is the CLI and
  pattern code:

  | Archive | 0.4.0 | 0.5.0 |
  |---|---|---|
  | `x86_64-unknown-linux-musl.tar.gz` | 2,134,656 | 2,140,993 |
  | `aarch64-unknown-linux-musl.tar.gz` | 1,941,545 | 1,947,700 |
  | `aarch64-apple-darwin.tar.gz` | 1,939,195 | 1,945,945 |
  | `x86_64-apple-darwin.tar.gz` | 1,969,611 | 1,974,853 |
  | `x86_64-pc-windows-msvc.zip` | 1,925,716 | 1,933,168 |
  | `aarch64-pc-windows-msvc.zip` | 1,821,786 | 1,828,044 |

- Ervin published the draft on 2026-11-13 (14:43 UTC); `releases/latest`
  is `v0.5.0`.

## README at 0.5.0 (task 3.1)

Every install block, both `uses:` lines and the pinned commit
(`d408ea1f7e17f7213df5b7bd70a18051e149a096`, `v0.5.0^{commit}`) name
0.5.0. One `0.4.0` stays on purpose: the roadmap's "Languages" row says
which release brought C#. The Linux install block, run as written
against the published release (with the arm64 target and a scratch
directory instead of `~/.local/bin`), passed its checksum and installed
`qgate 0.5.0`.

## GitHub rehearsal (task 3.2, 2026-11-13)

- **Setup:** in `nagypalervin/qgate-rehearsal`, whose `main` takes
  changes only through pull requests, a base branch
  `base/migrations-0.5.0` holds:
  - the README's workflow at `@v0.5.0`, cut out unchanged;
  - an EF Core-style migration, `src/Data/Migrations/20261101120000_Init.cs`
    (three `CreateTable` calls).

  Both pull requests add `20261113120000_AddLines.cs`, the same model
  under another class name. Neither was merged; both were closed.
- **Pull request 15, without the setting** (run 43445787804): the
  action installed `qgate 0.5.0 (x86_64-unknown-linux-musl)`, compared
  with `origin/base/migrations-0.5.0` and created the comment, read
  through the API: "1 finding, none at or above `error`.", with the one
  row `warning | src/Data/Migrations/20261113120000\_AddLines.cs:6 |
  duplicates src/Data/Migrations/20261101120000\_Init.cs:6-34 (lines
  6-34, 173 tokens) | duplication/duplicate-block`.
- **The same pull request, with `exclude = ["**/Migrations/**"]` pushed
  on top** (run 43445969436): "updated the qgate comment: no regressions
  found". The comment (same id) now reads "No regressions found.", with
  the marker `count=0 failing=0`.
- **Pull request 16, migration and setting together** (run
  43445792648), the case of D4: qgate printed "No regressions found."
  and the comment step wrote nothing ("no findings and no qgate
  comment"), as the pull request comment spec says. There is no comment
  to read, which is why the setting was also pushed onto pull request
  15.

## Risks / Trade-offs

- **[A broad pattern hides real code]** `exclude = ["src"]` turns
  qgate off for most of a repository. → It is the user's setting, and
  the README shows narrow patterns.
- **[Excluding a file also stops architecture checks of its imports]**
  A migration could import something a rule forbids. → Intended:
  excluded means "not analyzed". A rule's `to` still matches excluded
  targets.
- **[Self-exclusion in a pull request]** See D4.
