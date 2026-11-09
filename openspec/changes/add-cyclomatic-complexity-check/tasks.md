# Tasks

Three pull requests, one per group 1-3, each based on the merged previous
one. PR 1 also carries this change's planning artifacts. Branches:
`change/add-cyclomatic-complexity-check` (PR 1), then
`change/add-cyclomatic-complexity-check-2` and `-3`. Every task ends with
`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` passing.

## 1. TypeScript adapter (`qgate-lang`), PR 1

- [x] 1.1 Add `Language::Tsx` (design D5): `tsx` maps to it, `ts`/`mts`/`cts` stay `TypeScript`; update `recognizes_typescript` and verify the lang tests pass
- [x] 1.2 Add `tree-sitter` 0.26 and `tree-sitter-typescript` 0.23 to `qgate-lang` (design D6, reason in the commit message); verify `cargo build -p qgate-lang` and `cargo tree -p qgate-lang` show only the expected crates
- [x] 1.3 Define the public model and errors: `FileModel`, `Function { name, line, cyclomatic }`, `ParseError`, and `parse(Language, &str)`, all documented; verify clippy with `missing_docs` passes
- [x] 1.4 Write the decision-point fixtures (loops, switch, catch/finally, conditional expression, `&&`/`||`/`??`, logical assignment, `?.`, default values, type-level syntax) with failing tests that assert the expected complexity per spec "Decision points" and "Type-level syntax is not counted"
- [x] 1.5 Implement the tree walk and counting (design D3) and verify the 1.4 tests pass
- [x] 1.6 Write naming and unit-boundary fixtures (class members incl. getter/setter, class field arrow, object methods, named nested function, anonymous callback, module-level code and `describe`/`it`, unnamed default export, overloads, duplicate names) with failing tests that assert `(name, line, cyclomatic)` per spec; then implement naming and duplicate suffixes until they pass
- [x] 1.7 Write a fixture with a syntax error and a TSX fixture; verify `ParseError::Syntax` carries the line of the first error (design D4) and that TSX parses with the TSX grammar
- [x] 1.8 Run the one-off ESLint comparison (design D10) over the lang fixtures, record the table and any deviation under "Implementation Notes" in `design.md`; verify every difference is one of the two expected kinds, or stop and ask
- [x] 1.9 Update the `qgate-lang` module docs (no longer a placeholder) and verify `cargo doc -p qgate-lang --no-deps` builds without warnings

## 2. Complexity analyzer (`qgate-analyzers`), PR 2

- [x] 2.1 Change the `Analyzer` trait to `analyze(&self, change: &FileChange) -> Vec<Finding>` and add `FileChange` (design D2), documented; verify the workspace builds
- [x] 2.2 Write before/after fixture pairs under `fixtures/complexity/` for: small increase, large increase, legacy 40 -> 41, new at 10, 11 and 21, removed plus improved, rename; add failing tests that assert rule id, severity, line, message and metric per spec "Complexity increase rule" to "Finding content"
- [x] 2.3 Implement `ComplexityAnalyzer` with the threshold constants (design D8) and verify the 2.2 tests pass
- [x] 2.4 Update the `qgate-analyzers` module docs (complexity no longer planned) and verify `cargo doc -p qgate-analyzers --no-deps` builds without warnings

## 3. `qgate check` command (`qgate-cli`), PR 3

- [ ] 3.1 Add `clap` 4.6 with the features from design D6 (reason in the commit message) and move `--help`/`--version` to it; verify `qgate --version` and `qgate --help` behave per spec "Help and version"
- [ ] 3.2 Write failing end-to-end tests in `crates/qgate-cli/tests/check.rs` (design D9) for every scenario in the `check-command` spec, using small fixture files under `crates/qgate-cli/tests/fixtures/`
- [ ] 3.3 Implement `qgate check <before> <after>`: read files, choose the language by extension, parse, run `ComplexityAnalyzer`, sort findings by line then rule id, render, and map errors to standard error and exit code 2 (design D7); verify the 3.2 tests pass
- [ ] 3.4 Update `README.md`: status line, a `qgate check` usage example with sample output, and the C compiler note in "Building"; verify the example output matches what the binary prints for the cited fixture
- [ ] 3.5 Add `cargo run -q -p qgate-cli -- check` on a CLI fixture pair as a CI smoke step next to `--version`; verify the workflow step passes on the PR

## Workflow follow-up

- Run the `reviewer` subagent on each PR and fix its findings.
- In PR 3: write the `docs/journal/` entry, then archive the change on that branch (`/opsx:archive`) so the specs land on `main` with the code.
- Gate 3: the human merges each PR.
