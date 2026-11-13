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

## Risks / Trade-offs

- **[A broad pattern hides real code]** `exclude = ["src"]` turns
  qgate off for most of a repository. → It is the user's setting, and
  the README shows narrow patterns.
- **[Excluding a file also stops architecture checks of its imports]**
  A migration could import something a rule forbids. → Intended:
  excluded means "not analyzed". A rule's `to` still matches excluded
  targets.
- **[Self-exclusion in a pull request]** See D4.
