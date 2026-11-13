# Tasks

Three pull requests with the 0.4.0 release between the second and the
third (design D9). Tests come first in every task; every commit passes
the pre-commit checks; commit messages and PR bodies are written to files
first.

## 1. The C# adapter (PR 1, `change/add-csharp-analysis`)

- [x] 1.1 Check the generated-code convention against Roslyn's source (`GeneratedCodeUtilities`: file name endings and prefix, header markers) and record the result in `design.md` D5; correct the `csharp-analysis` delta first if they differ.
- [x] 1.2 Add `tree-sitter-c-sharp` (pinned `0.23`) to the workspace and `qgate-lang`; the commit message says why. Move the helpers both adapters need (line, first syntax error, `#2` names) into a shared module without changing TypeScript behavior: `cargo test -p qgate-lang` passes unchanged.
- [x] 1.3 `Language::CSharp` from `.cs`, and a family check for the CLI (D7). Unit tests: the extension, the family of every language, an empty file parses.
- [x] 1.4 Failing fixture tests in `crates/qgate-lang/fixtures/csharp/` for units and names (D1, D2): every scenario of the "C# function units", "C# lambdas count towards their named ancestor", "C# qualified names" and "C# parameter lists in names" requirements, plus explicit interface implementations, default interface methods, partial classes and a static next to an instance constructor (`#2`). Then the walker until they pass.
- [x] 1.5 Failing fixture tests for decision points (D3): each counted kind once, `case 1: case 2:`, the `_` arm, `default`, optional parameters, `is not null`, and every scenario of "C# decision points" and "Conditional compilation". Then the counting until they pass.
- [x] 1.6 Failing fixture tests for tokens (D4): the renamed-copy and different-members scenarios, usings, namespaces, preprocessor lines, comments, interpolated and raw strings, a byte order mark. Then the tokens until they pass.
- [x] 1.7 `csharp_generated_name` and `csharp_generated_header` (D5) with unit tests: each name form in mixed case, a header after a byte order mark and `#nullable`, a block comment header, a marker after the first code (not generated), a file that does not parse.
- [x] 1.8 Syntax errors: the spike's `#if`/`#else` file gives `ParseError::Syntax` with its first error line. Record the release binary's size before and after the grammar in `design.md`.

## 2. Wiring, evaluation and version (PR 2, `change/add-csharp-analysis-2`)

- [x] 2.1 Analyzer fixture pairs: `crates/qgate-analyzers/fixtures/complexity/csharp_*` (an increase, a new complex method, an overload added above another) and `fixtures/duplication/*.cs`, with failing tests first.
- [x] 2.2 CLI, revision mode: `.cs` among changed files, generated files skipped by name and header (D5), one duplication index per family (D6). Failing CLI tests first for each scenario of the modified `revision-input` and `duplication-analysis` requirements and of "Generated C# files are skipped", including the working tree.
- [x] 2.3 CLI, two-file form: `.cs` accepted, C# with TypeScript rejected (D7), the unsupported-type message names `.cs`. Failing CLI tests first for the scenarios of "Supported input files".
- [x] 2.4 Evaluation (D8) on the two repositories Ervin approved at Gate 1: script in the scratchpad, results and the hand check of ten findings recorded in `design.md`. Stop and ask Ervin if a cause affects many files.
- [ ] 2.5 README: C# in the status line, the checks it gets, its decision points and names, generated files, the two-file form; the roadmap row "Languages" in progress. Check every sentence against a test or the evaluation.
- [ ] 2.6 Version 0.4.0 in `Cargo.toml` (with `Cargo.lock`) and every `QGATE_VERSION` of `action.yml`; both release checks pass locally for `v0.4.0`.

## 3. Release 0.4.0 (between PR 2 and PR 3)

- [ ] 3.1 After PR 2 merges and Ervin agrees, push `v0.4.0` and check the draft: six archives, `SHA256SUMS`, `sha256sum --check`, the Linux arm64 binary printing `qgate 0.4.0` and analyzing a `.cs` pair. Ervin publishes. Record it, with the archive sizes, in `design.md` on `change/add-csharp-analysis-3`.

## 4. Docs, Azure run and archive (PR 3, `change/add-csharp-analysis-3`)

- [ ] 4.1 Every README install block, `uses:` and the pinned commit at 0.4.0; `grep` finds no `0.3.1`.
- [ ] 4.2 In the Azure rehearsal project, run the README's Windows job on a pull request that makes a C# method more complex and copies another; check the comment's rows through the API and record the run in `design.md`. Needs a PAT valid on the day; ask Ervin for one after 2026-11-17.
- [ ] 4.3 The roadmap row "Languages" says C# is done (architecture rules for `using` still open).

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- On PR 3: the `docs/journal/` entry, then `/opsx:archive`.
- Gate 3: Ervin merges each pull request.
