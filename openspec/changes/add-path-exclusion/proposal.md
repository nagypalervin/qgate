# Proposal: add-path-exclusion

## Why

The evaluation of `add-csharp-analysis` showed that EF Core migrations,
which carry no generated-code header, report duplicate Warnings: two
for a new migration in `dotnet/eShop`, fifteen for its initial import.
Every pull request that adds a migration would show them in its
comment. Ervin decided to release 0.4.0 as it was and to add a general
way to leave files out, which also covers other code a team does not
want checked (vendored libraries, generated clients without a header,
fixtures).

## What Changes

- **`[files] exclude` in `qgate.toml`:** a list of path patterns, with
  the same syntax and spelling rules as the `from` and `to` patterns of
  architecture rules. Example: `exclude = ["**/Migrations/**"]`.
- **An excluded file is left out everywhere in revision mode**, as if
  it had an unsupported extension:
  - not analyzed (no complexity, architecture or `input/unparsable`
    findings);
  - not compared for copies, neither as a changed file nor as an
    unchanged one;
  - its base version is not code that existed before.
  Each version of a changed file is judged by its own path, as for
  generated C# files: a file renamed out of an excluded directory
  counts as added.
- **The exclude list comes from the head revision's `qgate.toml`**, like
  the architecture rules, so `--worktree`, the Claude Code hook and the
  MCP server use it too. An invalid pattern fails with exit code 2,
  naming `qgate.toml`, `files.exclude` and the pattern.
- **The two-file form is unchanged:** it reads no configuration.
- **README:** the `qgate.toml` section and the C# section's note on
  migrations show the setting.
- **Version 0.5.0,** including `action.yml`.

### Out of scope

- An include list, or excluding by language.
- Excluding import targets: a forbid rule's `to` still matches a path
  in an excluded directory. Excluding decides what is analyzed, not
  what may be imported.
- Reading `.gitignore`-style files or `.editorconfig`.
- Per-rule or per-analyzer exclusion.

### For Gate 1

Each question has a recommendation; design.md has the reasons.

1. **The setting's place: a `[files]` table with `exclude`** rather than
   a top-level `exclude` key, so later file settings have a home.
   *Recommended: `[files] exclude`.*
2. **Silent exclusion,** with no line saying how many files were left
   out, as for unsupported and generated files. *Recommended: silent.*
3. **Two pull requests with 0.5.0 between them,** as before: the
   setting and the version, then the README versions, a rehearsal and
   the archive. *Recommended: yes.*
4. **The rehearsal:** the README's GitHub workflow at `@v0.5.0` on a
   rehearsal pull request that adds an EF Core-style migration
   repeating an older one, with `exclude = ["**/Migrations/**"]`: no
   duplicate. *Recommended: yes.*
5. **Which revision's list counts: the head's,** like the architecture
   rules. A pull request can then exclude its own files and hide their
   findings, but the change to `qgate.toml` is in its diff for the
   reviewer to see, as a removed forbid rule is today. Reading the
   base's list instead would stop that, but a pull request that adds a
   migration and the exclusion together would still report the
   migration. *Recommended: the head's.*

**Decided at Gate 1 (2026-11-13):** Ervin approved the proposal with
every recommendation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `configuration`: a new requirement, "Excluded paths", for the
  `[files] exclude` setting and what an excluded file takes part in;
  "Missing configuration" also excludes nothing.

## Impact

- **Code:**
  - `crates/qgate-config`: the `[files]` table;
  - the pattern code of the architecture analyzer becomes a module that
    the CLI can use too (`crates/qgate-analyzers`);
  - `crates/qgate-cli`: the file selection of revision mode.
- **No new dependency:** `globset` is already used.
- **Docs:** README `qgate.toml` and C# sections; version 0.5.0 in
  `Cargo.toml` and `action.yml`.
