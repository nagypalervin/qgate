# Design: add-forbidden-imports

## Context

See proposal.md for why and what.

- **`FileModel`:** it holds only function units today. The TypeScript
  adapter makes one non-recursive walk over the tree-sitter tree
  (`crates/qgate-lang/src/typescript.rs`).
- **`Analyzer::analyze(&FileChange)`:** it is per file. `FileChange` has
  the head path and both models, but not the old path of a renamed file.
  `ChangedFile.old_path` from `qgate-git` already has that path.
- **Dependency direction** (CLAUDE.md, ADR 0002): `cli -> report,
  analyzers -> lang -> core`, and `cli -> git`. `core` holds shared types
  only, without parsing.
- **Dependencies:** none for configuration, globs or serde yet.
  `regex-automata`, `regex-syntax`, `aho-corasick`, `memchr` and
  `serde_core` are already in the tree through tree-sitter.

## Goals / Non-Goals

**Goals:**

- Implement the `configuration` and `architecture-analysis` specs in three
  small pull requests.
- Keep analyzers free of the file format: an analyzer takes plain rule
  values, not TOML.

**Non-Goals:**

- A general settings system for every analyzer. Only `[architecture]`
  exists, but the crate is shaped so that `[complexity]` can be added
  later.
- Resolving modules the way TypeScript does (see the proposal, Out of
  scope).

## Decisions

### D1: A leaf crate `qgate-config`, recorded in ADR 0003

`qgate-config` turns the bytes of `qgate.toml` into plain data:

```
Config { architecture: Architecture { forbid: Vec<ForbidRule> } }
ForbidRule { name: String, from: Vec<String>, to: Vec<String> }
```

It checks everything that does not need pattern semantics: TOML syntax,
unknown keys (`#[serde(deny_unknown_fields)]`), types, empty or duplicate
names, and empty lists. It depends on nothing in the workspace, like
`qgate-git`. The CLI copies a `ForbidRule` into the analyzer's own rule
type: three fields, a few lines of wiring.

```
cli -> report
cli -> analyzers -> lang -> core
cli -> git
cli -> config            (new)
```

**Alternatives considered:**

| Option | Problem |
|---|---|
| Parse in `qgate-cli` | The CLI stops being thin; config errors and validation are not wiring. |
| Rule types in `qgate-core` | Core holds only types. Pattern matching is logic, and parsing is not allowed there. |
| `analyzers -> config` | Every analyzer would depend on the file format, and a format change would touch analysis code. |

A new edge needs an ADR (CLAUDE.md). ADR 0003 records it. The "Layout"
and dependency lines in `CLAUDE.md` and in `.claude/agents/reviewer.md`
are updated in the same pull request. **Gate 2 approval covers this edit
of the agent prompt.**

### D2: `toml` + `serde` derive for parsing

| Option | Crates added | Messages | Status |
|---|---|---|---|
| `toml` 0.9, `default-features = false`, features `parse`, `serde`, `std` | about 11, mostly build-time proc-macro (`syn`, `quote`, `serde_derive`, `proc-macro2`) | line and column, with the key path | the reference implementation, actively maintained |
| `basic-toml` 0.1 | about 7 | line and column | maintenance mode, older TOML |
| hand-written parser | 0 | ours | a TOML parser is not our problem |

Choose `toml`. The commit message states why, as CLAUDE.md requires.
`deny_unknown_fields` turns a misspelled table into an error with its
name.

### D3: `globset` for patterns, in `qgate-analyzers`

The exploration leaned towards our own glob matcher. Measuring showed
that `globset` adds only `globset` and `bstr`; its regex crates are
already in the tree. It is ripgrep's matcher, and globs have edge cases
we do not want to own, so use `globset`.

- **Options:** `literal_separator(true)`, so `*` and `?` stop at `/`;
  `backslash_escape(true)`.
- **Spec extras, on top of globset:**
  - a pattern without glob characters (`*?[{`) matches `p` and anything
    starting with `p/`; it is compiled as two globs, `p` and `p/**`;
  - a pattern ending in `/**` also matches the directory itself; it is
    compiled with an extra glob for the prefix.
- **Where:** the `architecture` analyzer compiles its patterns at
  construction: `ArchitectureAnalyzer::new(rules) -> Result<Self,
  PatternError>`. The error names the rule and the pattern. The CLI turns
  it into the "Invalid pattern" error with exit 2, so pattern validation
  lives next to pattern semantics.

### D4: Imports in `FileModel`

```
FileModel { functions, imports: Vec<Import> }
Import { specifier: String, line: u32 }   // in source order
```

The walk records imports in `enter`, at any depth, so a dynamic import
inside a function counts.

| Node | Specifier |
|---|---|
| `import_statement` | the `source` field |
| `export_statement` with a `source` field | the `source` field |
| `import_require_clause` (`import x = require("m")`) | its string |
| `call_expression` whose `function` is `import` or the identifier `require`, with exactly one `string` argument | that argument |

- **Specifier text:** the string between the outer quotes, as written,
  with no escape decoding, the same rule as property names.
- **Line:** the line of the string literal, so a multi-line `import { a,
  b } from "x"` points to the line that names the module.
- **Non-literal arguments:** template strings and identifiers are
  skipped.

### D5: Reading `<head>:qgate.toml` in `qgate-git`

A new method, `Repository::file_at(&self, revision: &str, path: &str) ->
Result<Option<Vec<u8>>, GitError>`.

1. **Resolve.** It resolves `revision` to a commit id with the existing
   `resolve`, which rejects option-like values.
2. **Look up.** It runs `git ls-tree -z <commit> -- <path>` with
   `--literal-pathspecs` (`GIT_LITERAL_PATHSPECS=1`), so the path is
   never a pattern.
3. **Decide:**
   - no entry gives `None`;
   - a regular blob (mode `100644` or `100755`) is read with
     `read_blob`;
   - anything else (tree, symlink, submodule) is a new
     `GitError::NotARegularFile`.

The CLI reads the configuration **before** the diff. An invalid
`qgate.toml` then fails fast. An unknown revision still fails first,
because `file_at` resolves the revision.

### D6: `FileChange` gains the old path

`FileChange { path, old_path: Option<PathBuf>, before, after }`.

- `old_path` is `Some` only for a renamed file.
- The two-file form sets `None`.
- The complexity analyzer ignores it.

`FileChange` was documented as provisional (design D2 of the first
change), so this is the expected revision.

### D7: The analyzer

For each rule whose `from` matches a path:

```
head_violations = { (rule, target) : target of an import in `after`,
                                     resolved against `path`, matches `to` }
base_violations = same for `before`, resolved against `old_path` or `path`,
                  counted only if `from` matched that base path
report head_violations - base_violations, each at the first import line
of its target in `after`
```

- **Resolution:** a pure function over `/`-separated strings. It starts
  from the directory of the importing path and applies `.` and `..`. Going
  above the root returns `None`.
- **No file-system access.** Paths are git paths, `/`-separated on every
  platform.
- **Order:** findings come out in rule order, then target order. The CLI
  sorts them anyway (path, line, rule).

### D8: CLI wiring

In revision mode:

1. `file_at(head, "qgate.toml")`.
2. Parse it (`qgate-config`) and build `ArchitectureAnalyzer::new(...)`.
   Every error at this step exits with 2 and the message
   `qgate.toml: <problem>`.
3. Run `[ComplexityAnalyzer, ArchitectureAnalyzer?]` on every analyzable
   changed file.

Unparsable files are skipped as today, so there are no architecture
findings for them. The two-file form never calls step 1.

## Risks / Trade-offs

- **[Aliases]** `@/ui/Button` is matched only as written. A user with
  tsconfig aliases must list both forms (`"src/ui/**", "@/ui/**"`).
  → Documented in the README with an example; tsconfig `paths` support is
  the obvious follow-up.
- **[No extension probing]** A file-level pattern such as
  `src/ui/Button.tsx` never matches the import `../ui/Button`.
  → The README recommends directory patterns. A file-level pattern without
  the extension (`src/ui/Button`) matches.
- **[Head configuration]** A pull request can remove a rule and pass.
  → Accepted in exploration; the `qgate.toml` diff is visible in review,
  as is any edit of the CI workflow.
- **[Interim state after PR 2]** A valid `qgate.toml` is read and
  validated but not applied until PR 3. → All three pull requests land
  before the change is archived; there is no release in between.
- **[Build time]** `syn` and `serde_derive` add compile time to a clean
  build. → They are compiled once and cached in CI. The alternative is a
  hand-written deserializer, which is not worth it.

## Migration Plan

None. Without a `qgate.toml`, behaviour is unchanged.
