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
- [Duplicate suffixes follow source order, so adding or removing one
  duplicate shifts the others and can report an untouched function as new,
  or hide a real change behind a wrong pair] → accepted by the human at the
  PR 2 review as a known limitation (proposal, Out of scope); duplicate
  names in one scope are rare.
- [The `FileChange` shape may not survive git input] → explicitly
  provisional (D2); only one analyzer depends on it today.
- [A file with a single syntax error cannot be checked at all] → this
  favors correctness over coverage; revisit if it turns out annoying on
  work-in-progress code.
- [The pinned grammar lags behind TypeScript: valid code with very new
  syntax (e.g. variance annotations `interface I<in out T>` with grammar
  0.23) is refused as a syntax error] → the error message says "or syntax
  the parser does not support yet" instead of claiming the code is wrong;
  grammar upgrades are the fix, guarded by the fixtures.

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
- **1.5:** optional calls `f?.()` have no `optional_chain` node in the
  grammar, only a bare `?.` token under `call_expression`; that token is
  counted. Member and subscript chains (`a?.b`, `a?.[0]`) are counted by
  their `optional_chain` node.
- **1.5:** a class body is a measurement barrier: decision points in class
  field initializers and static blocks that are not inside a method or a
  named field function count towards nothing, even when the class is
  declared inside a function. This is what the spec's "class field
  initializers that are not functions" requires, and it matches ESLint,
  which treats initializers and static blocks as their own scopes.
- **1.5:** the walk is iterative (tree cursor plus a frame stack), not
  recursive, so deeply nested or generated code cannot overflow the stack.
- **1.5:** the walk already implements the full naming rules of D3, because
  naming and unit boundaries are one mechanism. The 1.6 naming tests were
  therefore written after the code, not before it; they are still derived
  from the spec scenarios, not from the output.
- **1.4/1.5:** the decision-point test was written first and observed to
  fail (empty model) before the walk existed; test and code were committed
  together so that every commit on the branch builds and passes.
- **1.2 (build-time dependencies):** besides `cc`, tree-sitter's build
  script pulls in `serde_json` and its dependencies (`indexmap`,
  `hashbrown`, `foldhash`, `itoa`, `serde_core`, `zmij`). They run only at
  build time and do not end up in the binary.

#### Changes after the PR 1 review (2026-11-09)

The reviewer subagent found one real naming bug, untested behaviour and one
spec ambiguity. Decisions:

- **Type wrappers (asked, human chose "look through"):** parentheses,
  `as`, `satisfies` and `!` between a definition and its binding no longer
  hide the name. Before, `const h = (async () => {...}) satisfies H` was not
  measured at all and `const config = {...} satisfies Config` produced
  `load` instead of `config.load`. New spec requirement "Type wrappers do not
  hide the binding".
- **Functions wrapped in a call at module level (asked, human chose "out of
  scope"):** `export const GET = withAuth(async () => {...})` stays
  unmeasured in this change; listed by name in the proposal's Out of scope
  and pinned by a spec scenario and fixture. A follow-up change handles it.
- **Third ESLint difference (asked, human approved):** see "ESLint
  comparison" below. Task 1.8 said to stop and ask; this was done only at
  the review, after the fact.
- **Duplicate suffixes are now assigned during the walk**, not after it:
  a suffix skips names that are already taken (a literal key `"x#2"` no
  longer collides with a generated `x#2`), and nested units inherit it
  (`A.f#2.g`, not `A.f.g#2`). Spec "Duplicate names" now says both.
- **Property names** drop exactly one pair of outer quotes; escapes stay as
  written (`'it\'s'` gives `it\'s`). Before, all leading and trailing quotes
  were trimmed. New spec requirement "Property names".
- **Naming rules made explicit in the spec:** a named function expression
  passed as an argument is named by its own name; a binding wins over the own
  name; unnamed default-exported objects and classes give `default.<member>`.
- **Grammar lag:** see the new entry under Risks; the `ParseError::Syntax`
  message changed accordingly.
- **Fixtures (D9 deviation):** D9 planned one fixture per topic. The first
  three fixtures each cover several topics (`naming.ts` holds most naming
  rules); the review added single-topic fixtures for generators, wrappers,
  module-level code, static blocks, default exports, duplicates and property
  names. Every node kind in `FUNCTION_KINDS` and every decision-point kind
  now has a fixture.

#### PR 2: complexity analyzer

- **2.2/2.3:** the analyzer fixtures use one function shape, `N` single-line
  `if` statements plus a return, so each complexity is `N + 1` at a glance;
  the counting itself is covered by the `qgate-lang` fixtures. The tests ran
  against an empty `analyze` first (five of seven failed, the two
  "no findings" cases passed trivially); tests and implementation were
  committed together so every commit stays green.
- **2.3:** the rule ids are public constants (`INCREASE_RULE`, `NEW_RULE`) so
  that the CLI tests and later configuration refer to one definition of this
  public API. The thresholds stay private constants (D8).
- **2.3:** the analyzer returns findings in the after file's source order;
  the CLI sorts across analyzers (D8). Matching uses a name map of the
  before file. That map cannot collide because `qgate-lang` guarantees
  unique names; a `debug_assert` makes the dependency visible. Unique is not
  the same as stable, though: see the duplicate-suffix entry under Risks.
- **PR 2 review (2026-11-09):** the reviewer subagent found that the upper
  Warning boundary of `complexity/cyclomatic-new` (20) was untested; the
  `new_functions` fixture now has `atTwenty`, and changing `>` to `>=` at
  that boundary fails a test. The duplicate-suffix shift was put to the
  human, who accepted it as a known limitation.

### ESLint comparison

Run on 2026-11-09 with ESLint 9.39.5, `typescript-eslint` 8.72.1 parser,
`complexity: ["error", { max: 0 }]` (classic variant), over copies of the
three valid `qgate-lang` fixtures. Not part of CI.

**Summing rule:** an anonymous function's own base of 1 is not a decision
point, so it adds `eslint - 1` to its named ancestor.

| Fixture | Units | Equal | Equal after summing | Expected differences |
|---|---|---|---|---|
| `decision_points.ts` | 16 | 16 | - | none |
| `component.tsx` | 2 | 1 (`Greeting` 2) | 1 (`List`: 1 + (2 - 1) = 2) | none |
| `naming.ts` | 23 | 20 | 3 (`Cart.total`, `process`: 1 + (2 - 1) = 2; `bound`: ESLint names it `ownName`, same value 2) | ESLint also reports 3 class field initializers (lines 12, 38, 82) and 2 module-level `describe`/`it` arrows (lines 109, 110); qgate measures neither |

No unexpected difference. The decision-point counts, including optional
chaining and default values, match ESLint exactly.

D10 listed two expected kinds of difference; the run showed a third one,
which the spec already requires and the 1.5 note above describes:

3. Class field initializers and static blocks: ESLint measures each as its
   own scope ("Class field initializer has a complexity of ..."); qgate does
   not measure them (spec: "Code outside named functions is not measured").
   A field whose value is a function (`handle = () => {}`) is a unit in both.

The human approved this third kind on 2026-11-09, at the PR 1 review.

**Second run** (same versions, after the review) over the fixtures the
review added: `generators.ts`, `wrappers.ts`, `default_object.ts`,
`default_class.ts`, `duplicates.ts` and `property_names.ts` match ESLint for
every unit (ESLint names differ only where qgate adds a prefix or a
duplicate suffix). `module_level.ts` and `static_block.ts` show only the
expected kinds 2 and 3.
