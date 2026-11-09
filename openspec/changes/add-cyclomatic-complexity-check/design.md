# Design: add-cyclomatic-complexity-check

## Context

See `proposal.md` for the why and the decisions taken during exploration,
and the two spec deltas for the observable behavior.

Current state that shapes the approach:

- `qgate-lang` has only a `Language` enum (`TypeScript`, mapped from
  `ts`/`tsx`/`mts`/`cts`). No parser, no model.
- `qgate-analyzers` has a placeholder `Analyzer` trait whose `analyze(&self)`
  takes no input.
- `qgate-core` has `Finding`, `Location`, `Metric` and `Severity`.
  `Metric::delta()` treats a new item as growing from zero.
- `qgate-report::render_text` already prints the line format the spec needs.
- `qgate-cli` parses arguments by hand and has no dependencies besides the
  workspace crates.
- Constraints from CLAUDE.md: dependencies flow `cli -> report, analyzers ->
  lang -> core`; libraries return errors, only the CLI prints and picks exit
  codes; no `unsafe` in our crates; MSRV 1.85; every public item documented.

## Goals / Non-Goals

**Goals:**

- Give `qgate-lang` and the `Analyzer` trait a first shape that the next
  analyzers (duplication, architecture) and the git input can build on
  without a rewrite.
- Keep each layer testable without the layers above it: the adapter from a
  source string, the analyzer from two models, the CLI end to end.

**Non-Goals:**

- A general, multi-file input model (added/deleted files, renames). That
  arrives with git input.
- Performance work. Two files are parsed once each; nothing here is hot.

## Decisions

### D1: Data flow and where each step lives

```
cli:        read before/after files (I/O), pick Language from extension
              |
lang:       parse(language, &str) -> Result<FileModel, ParseError>
              |   FileModel { functions: Vec<Function> }
              |   Function  { name, line, cyclomatic }
              v
analyzers:  ComplexityAnalyzer.analyze(&FileChange) -> Vec<Finding>
              |   FileChange { path, before: FileModel, after: FileModel }
              v
cli:        sort findings, report::render_text, print, exit code
```

- `qgate-lang` takes source text, not a path, so it does no I/O and the CLI
  owns all error printing.
- `FileModel` is language-neutral: a list of function units with a unique
  qualified name, the 1-based line of the name, and the cyclomatic
  complexity. Analyzers never see tree-sitter types.
- `ParseError` has two cases: syntax error (with line) and, defensively,
  grammar load failure. Unsupported extensions are decided earlier, by
  `Language::from_extension` in the CLI.

*Alternative considered:* the adapter returns decision-point nodes and the
analyzer counts (option B in exploration). Rejected: what is a decision
point is language-specific, so it belongs in the adapter (ADR 0001).

### D2: `Analyzer` takes a `FileChange`

`fn analyze(&self, change: &FileChange) -> Vec<Finding>`, with
`FileChange { path: PathBuf, before: FileModel, after: FileModel }` defined
in `qgate-analyzers`. The analyzer needs the path to fill
`Finding.location`; bundling it with both models keeps the signature stable
when more fields arrive.

This is provisional. Git input will need added and deleted files (an
optional side) and many files per run; that change will revisit the type and
record the result in an ADR, because it affects every analyzer.

*Alternative considered:* `analyze(&self, before: &FileModel, after:
&FileModel)`. Rejected: the path would have to be patched into findings
afterwards by the caller.

### D3: Counting and naming in one tree walk

The adapter walks the tree once, keeping a stack of name segments and a
stack of open function units:

- A node that defines a function unit (spec: Function units) is **named**
  if its name comes from its own declaration, the `variable_declarator`,
  `public_field_definition` or object `pair` it is the value of, or an
  unnamed `export default`. Otherwise it is anonymous and does not open a
  unit, so its decision points go to the unit on top of the stack (spec:
  Anonymous functions). With an empty unit stack they are dropped.
- Named non-function containers (class declarations, classes or object
  literals assigned to a variable, field or property) push a name segment
  but no unit.
- For `const f = function g() {}` the unit is named `f`, after the binding
  callers use.
- Getters and setters are named `get <name>` / `set <name>`; computed member
  names use their source text in brackets, e.g. `Cart.[Symbol.iterator]`.
- Each decision-point node kind (spec: Decision points) adds one to the top
  unit. `&&`, `||`, `??` are `binary_expression`s filtered by operator;
  logical assignments are `augmented_assignment_expression`s filtered by
  operator; default values are `assignment_pattern` and
  `required_parameter`/`optional_parameter` with a `value`. Type-level
  subtrees (type annotations, type aliases, interfaces) are skipped entirely,
  which keeps conditional types out.
- Duplicate qualified names get `#2`, `#3` suffixes in source order, after
  the walk.

The exact node kinds are taken from the `tree-sitter-typescript` grammar
version we pin and are covered by the fixtures, so a grammar upgrade that
renames a node fails a test instead of silently changing counts.

### D4: Syntax errors

After parsing, if `root_node().has_error()`, find the first `ERROR` or
missing node in document order and return `ParseError::Syntax { line }`.
tree-sitter would let us count a partial tree, but a misplaced `ERROR` node
can swallow or duplicate branches, so the result would not be trustworthy.

### D5: TSX as its own `Language` variant

Add `Language::Tsx`, mapped from `tsx`; `ts`, `mts` and `cts` stay
`Language::TypeScript`. The two need different grammars
(`LANGUAGE_TYPESCRIPT` and `LANGUAGE_TSX`): TSX treats `<T>x` as JSX, plain
TypeScript treats it as a type assertion. The existing
`recognizes_typescript` test changes accordingly.

*Alternative considered:* keep one variant and pass the extension to the
parser. Rejected: the grammar choice would leak a file-name detail into the
adapter API.

### D6: Dependencies

| Crate | Version | Used by | Why |
|---|---|---|---|
| `tree-sitter` | 0.26.x | `qgate-lang` | Parser runtime (ADR 0001). 0.27 needs a newer Rust than our MSRV 1.85; the MSRV-aware resolver already selects 0.26. |
| `tree-sitter-typescript` | 0.23.x | `qgate-lang` | Official TypeScript and TSX grammars, one crate for both. |
| `clap` | 4.6.x, `default-features = false`, features `std`, `help`, `usage`, `error-context` | `qgate-cli` | Subcommands, usage errors with exit code 2, help text. Builder API, no `derive`, to avoid the proc-macro dependency tree; `color` and `suggestions` are left out for the same reason. Its MSRV is 1.85. |

tree-sitter and the grammar compile C code with `cc` and use `unsafe`
internally; `forbid(unsafe_code)` applies to our crates only, so nothing in
qgate needs an exception. Each dependency is justified again in the commit
that adds it.

*Alternative considered for the CLI:* keep hand-rolled parsing. Rejected: the
module docs already planned `clap` for this change, and usage errors, help
and future flags (`--format`, config path) would all be hand-written.

### D7: Exit codes and error output

- `0`: the analysis ran, with or without findings.
- `2`: invalid invocation (clap's own exit code for usage errors), or an
  input that is unsupported, unreadable or has a syntax error.
- `1` is reserved for the later "check failed" change (findings above a
  severity), so scripts can tell a failed gate from a broken call.

Errors go to standard error as `error: <path>: <reason>` (with `:<line>` for
syntax errors); standard output stays empty, so piping findings is safe.

### D8: Thresholds and sorting

Thresholds are `const`s in the complexity module (`INCREASE_ERROR_DELTA =
5`, `NEW_WARNING_ABOVE = 10`, `NEW_ERROR_ABOVE = 20`), so M2 can replace them
with configuration in one place. Findings set `metric` with before/after
values; for a new unit `before` is `None`, which matches `Metric::delta()`.

The CLI sorts findings by line, then rule id, before rendering. Sorting is
orchestration across analyzers, so it does not belong in a single analyzer;
`qgate-report` only renders what it is given.

### D9: Tests and fixtures

- `qgate-lang`: one TypeScript fixture per topic under
  `crates/qgate-lang/fixtures/typescript/` (decision points, type-level
  syntax, naming, nesting and callbacks, overloads, duplicates, syntax
  error) plus one TSX fixture. Tests in `#[cfg(test)] mod tests` load them
  with `include_str!` and compare `(name, line, cyclomatic)` lists.
- `qgate-analyzers`: before/after pairs under
  `crates/qgate-analyzers/fixtures/complexity/<case>/{before,after}.ts`, one
  case per rule outcome (increase warning/error, legacy growth, new
  below/at/above thresholds, removed and improved, rename).
- `qgate-cli`: end-to-end tests in `crates/qgate-cli/tests/check.rs` that run
  the built binary (`CARGO_BIN_EXE_qgate`) with `std::process::Command`. This
  is the one place where tests live outside the source file, because a
  binary's exit code and streams can only be checked from outside. No
  test-only dependencies.

### D10: ESLint as the reference

Reference: ESLint 9's `complexity` rule, which counts optional chaining and
default values. Once, during implementation, run ESLint with
`complexity: ["error", 0]` over the `qgate-lang` fixtures (via `npx`, never
in CI) and record the comparison table here. Expected, deliberate
differences:

1. Anonymous functions: ESLint reports them separately; qgate adds them to
   the named ancestor. Compare by summing ESLint's anonymous results into
   their ancestor.
2. Code with no named ancestor: ESLint still reports anonymous functions at
   module level; qgate does not measure them.

Any other difference is a bug in qgate or a misread of ESLint. If it is
ESLint's behavior that differs from the spec, stop and ask before changing
either.

## Risks / Trade-offs

- [tree-sitter needs a C compiler] → present on GitHub runners and typical
  dev machines; mentioned in the README build section.
- [Grammar upgrades can rename node kinds and shift counts] → versions are
  pinned in `Cargo.lock`; every counted node kind has a fixture.
- [Counting `?.` and default values makes numbers higher than people
  expect from "number of branches"] → it matches ESLint 9, which most
  TypeScript users already see; documented in the spec.
- [The name-based match reports renames as new] → accepted for this slice
  (proposal, Out of scope); rename detection is a later change.
- [The `FileChange` shape may not survive git input] → explicitly
  provisional (D2); only one analyzer depends on it today.
- [A file with a single syntax error cannot be checked at all] → this
  favors correctness over coverage; revisit if it turns out annoying on
  work-in-progress code.

## Migration Plan

Nothing to migrate: no released version, no users. The placeholder
`Analyzer` trait has no implementations.

## Implementation Notes

<!-- Decisions made during apply are recorded here, per CLAUDE.md. -->

- **1.2:** `tree-sitter` keeps its default features. `regex` is a
  non-optional dependency of tree-sitter (for its query API, which qgate
  does not use); `default-features = false` would only drop regex's `std`
  and `perf` features, not the crate.
- **1.3:** the non-syntax error case of `ParseError` is named `Parser {
  message }` rather than "grammar load failure": it covers both a grammar
  that fails to load and tree-sitter returning no tree. Neither is caused by
  the input.

### ESLint comparison

<!-- Filled in during apply (D10). -->
