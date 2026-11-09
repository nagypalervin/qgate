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
| `toml` 1.x, `default-features = false`, features `parse`, `serde`, `std` | 5 built crates in this workspace (`serde` derive was already locked) | a byte span, turned into line and column | the reference implementation, actively maintained, MSRV 1.85 |
| `basic-toml` 0.1 | fewer | line and column | maintenance mode, older TOML |
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
| `call_expression` whose `function` is `import`, with a `string` first argument (a second argument holds import attributes) | that argument |
| `call_expression` whose `function` is the identifier `require`, with exactly one `string` argument | that argument |

- **Specifier text:** the string between the outer quotes, as written,
  with no escape decoding, the same rule as property names.
- **Line:** the line of the string literal, so a multi-line `import { a,
  b } from "x"` points to the line that names the module.
- **Non-literal arguments:** template strings (with or without
  substitutions) and identifiers are skipped.
- **Type positions:** the walk does not enter type subtrees (decision
  points never sit there), so it scans each skipped subtree for imports
  separately; `let t: import("./x").T` counts like any other import.

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

The CLI reads the configuration **after** the diff, before analyzing any
file (corrected at the PR 2 review; see Implementation Notes). The diff
checks both revisions and their merge base, so an unknown base, the
usual shallow-clone failure, keeps its message and hint even when
`qgate.toml` is broken.

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

1. `changed_files(base, head)`, as before; then `file_at(head,
   "qgate.toml")`.
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
- **[Grammar lag]** tree-sitter-typescript 0.23 rejects some valid
  forms, and a file using one is `input/unparsable` and gets no
  architecture findings, like any unparsable file:
  - `export type * from "x"` (TypeScript 5.0);
  - `export import x = require("y")`;
  - import types with a suffix: `import("./x").Foo<string>`,
    `import("./w").Bar[]`, `keyof import("./k").T`;
  - `interface I extends import("./ie").Base {}` and
    `class Z extends Base<import("./ce").T> {}`.

  Plain import types (`import("./x").T`, `typeof import("./x")`, in a
  union or as a type argument elsewhere) parse. → Listed in the README in PR 3; a grammar update
  fixes it.
- **[Head resolved twice]** `changed_files` and `file_at` each resolve
  the head revision. If the ref moves between the two calls (a concurrent
  fetch), the configuration could come from another commit than the
  diff. → Accepted as very unlikely; resolving once would mean passing
  commit ids through the git API, whose errors name the revisions as
  the user wrote them.

## Migration Plan

None. Without a `qgate.toml`, behaviour is unchanged.

## Implementation Notes

- **PR 1, imports:**
  - Comments between call arguments are skipped when checking for "exactly
    one string literal", so `require(/* why */ "x")` counts.
  - An empty specifier (`import ""`) is recorded as written. It matches
    no pattern, so recording it costs nothing and keeps the rule simple:
    every import is recorded as written.
  - Imports are recorded in `Walker::enter` before any other handling, so
    a dynamic import inside a bound call or a type wrapper is still found.
- **PR 1, review round 1:**
  - `import("x")` in a type position was recorded or dropped depending on
    where the type stood (`as` type: recorded; `let t:` annotation:
    dropped), against "wherever they appear" and the decision that
    type-only imports count. Skipped type subtrees are now scanned for
    imports.
  - `import("x", { with: ... })` was dropped by the one-argument rule.
    For `import()` only the first argument must be a string; `require`
    keeps the one-argument rule. The spec says so now, with a scenario.
  - Tests pin comment skipping, the empty specifier, module-level calls
    (the bound-call path) and the TSX line; the quote stripping is shared
    with property names (`unquote`).
- **PR 2, `file_at`:** besides `--literal-pathspecs`, the entry's path
  must equal the requested path exactly, so a pattern can never select
  another file even if a git version treated the argument as one. A test
  shows that `*.toml`, `qgate.tom?` and `:(glob)*.toml` find nothing.
- **PR 2, review round 1:**
  - Reading `qgate.toml` before the diff resolved only the head, so an
    unknown base was reported as a broken configuration, without the
    shallow-clone hint. The diff now runs first (D5, D8); an e2e test pins
    "unknown base + invalid `qgate.toml`".
  - `toml` moved from 0.9 to 1.x (same features, MSRV 1.85, one `winnow`
    instead of two). The dependency counts were corrected against the
    workspace's lock file: 5 built crates, not about 11.
  - Error columns count characters, not bytes; tests pin a non-ASCII line
    and a CRLF file. Rule names in messages are escaped (`{:?}`).
  - A blank name (`"  "`) is rejected like an empty one: it could not tell
    findings apart.
  - `run` removes `GIT_*_PATHSPECS` from git's environment: git refuses
    `--literal-pathspecs` next to another global pathspec setting, which
    made every revision-mode run fail for users who export one.
