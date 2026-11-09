# Proposal: add-cyclomatic-complexity-check

## Why

qgate's promise is "per change, what got worse", but today it can only print
its version. The first useful slice is the thinnest one that runs end to end
(input, parser, analyzer, finding, report) and catches the most common
regression in fast, AI-assisted code: functions that quietly gain branches.
Building it now also gives the empty `qgate-lang` and `qgate-analyzers`
crates their first real shape before more analyzers depend on it.

## What Changes

- New subcommand **`qgate check <before> <after>`**: compares two TypeScript
  files and prints, per function, where cyclomatic complexity got worse.
- **TypeScript adapter** in `qgate-lang` (tree-sitter): extracts the
  functions of one file with a qualified name, start line and cyclomatic
  complexity.
- **Complexity analyzer** in `qgate-analyzers` with two rules:
  - `complexity/cyclomatic-increase`: an existing function got more complex.
    Warning for +1 to +4, Error for +5 or more.
  - `complexity/cyclomatic-new`: a new function is already complex.
    Warning for 11 to 20, Error above 20.
- The `Analyzer` trait gets its first real input (a base and a head file
  model). **BREAKING** for the placeholder trait only; it has no
  implementations yet.
- `qgate-cli` moves from hand-rolled argument parsing to `clap`, as its
  module docs already planned.
- New dependencies: `tree-sitter`, `tree-sitter-typescript`, `clap`
  (justified in `design.md`).

### Decisions taken during exploration

- **Metric:** cyclomatic complexity (McCabe), 1 + decision points, per
  function. Cognitive complexity may follow later as a separate rule.
- **Reference definition:** ESLint's `complexity` rule. Deliberate
  deviations are listed in `design.md`.
- **Where it is computed:** in the language adapter, because what counts as a
  decision point is language-specific (ADR 0001: a new language needs an
  adapter, not analyzer changes).
- **Function units:** named nested functions are their own unit; anonymous
  functions (callbacks) count towards their nearest named ancestor.
- **Matching across versions:** by qualified name within the file
  (`Class.method`, `outer.inner`). Overload signatures are ignored; only the
  implementation counts.
- **What counts as worse:** a matched function whose complexity increased,
  or a new function above the threshold. Removed, unchanged or improved
  functions never produce a finding.
- **Severity thresholds** are fixed constants in this change. They become
  configurable with `qgate.toml` in M2.

### Out of scope

- Git revisions as input (`qgate check base..head`): next change, likely
  with an ADR on how to read git objects.
- Configuration (`qgate.toml`), thresholds, enabling or disabling rules.
- Exit codes based on severity (failing CI on Error): a separate change.
- JSON, SARIF or Markdown output.
- Rename detection: a renamed function is reported as removed + new. Known
  limitation: a renamed function that is already complex shows up as
  `complexity/cyclomatic-new`.
- Module-level code (statements outside any function), including anonymous
  callbacks with no named ancestor, such as `describe`/`it` blocks in tests.
- Cognitive complexity.
- Languages other than TypeScript.

## Capabilities

### New Capabilities

- `check-command`: the `qgate check` command line interface: arguments,
  accepted inputs, output, error handling and exit codes.
- `complexity-analysis`: how cyclomatic complexity is measured for a
  function, how functions are identified and matched between two versions,
  and when the complexity rules report a finding with which severity.

### Modified Capabilities

None. There are no existing specs.

## Impact

- `qgate-lang`: first real adapter, new public model types and a parse error
  type.
- `qgate-analyzers`: `Analyzer` trait signature changes; new `complexity`
  module.
- `qgate-cli`: new `check` subcommand, `clap`, exit codes for invalid input.
- `qgate-core` and `qgate-report`: no API change expected; `render_text` is
  reused as is.
- Build: tree-sitter compiles C code, so building needs a C compiler (present
  on the GitHub `ubuntu-latest` runners and most dev machines).
- `README.md`: status line and a usage example.

## Open Questions

All four were resolved at Gate 1 (2026-11-09): the human approved the
proposal and the design and kept every proposed default below. The specs
already reflect them.

1. **Syntax errors.** tree-sitter is error-tolerant and still returns a tree.
   *Proposed:* refuse files that contain syntax errors (error message with the
   first error line, exit code 2), because partial trees can silently distort
   the counts.
2. **Unsupported input.** *Proposed:* a file whose extension is not a
   TypeScript one is an error (exit code 2), as are a missing or unreadable
   file.
3. **TSX.** *Proposed:* include `.tsx` now. `tree-sitter-typescript` ships the
   TSX grammar in the same crate and `Language::from_extension` already maps
   it, so the cost is one grammar switch plus one fixture.
4. **Rule ids.** *Proposed:* `complexity/cyclomatic-increase` and
   `complexity/cyclomatic-new`. Rule ids are a public API, so this is the last
   cheap moment to rename them.
