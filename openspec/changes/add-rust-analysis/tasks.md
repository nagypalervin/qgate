# Tasks

Three pull requests with the 0.9.0 release between the second and the
third (design D9: the CI gate pins the released Action). Tests come
first in every task; every commit passes the pre-commit checks; commit
messages and PR bodies are written to files first.

## 1. The Rust adapter (PR 1, `change/add-rust-analysis`)

- [x] 1.1 Add `tree-sitter-rust` (pinned `0.24`) to the workspace and `qgate-lang`; the commit message says why. `Language::Rust` from `.rs`, `Family::Rust`, `Family::ALL` (D1). Unit tests: the extension, the family of every language, an empty file parses; `cargo test -p qgate-lang` passes.
- [x] 1.2 Failing fixture tests in `crates/qgate-lang/fixtures/rust/` for units and names (D2): every scenario of "Rust files", "Rust function units", "Rust closures count towards their named ancestor" and "Rust qualified names", plus a reference type in an impl (`<&Cart as Trait>`), a lifetime (`Parser<'a>.parse`), a `fn` in an `extern` block and an `async fn`. Then the walker until they pass.
- [x] 1.3 Failing fixture tests for decision points (D3): each counted kind once, a let chain with two `&&`, a nested `|` inside a tuple pattern, the `_` arm, a binding arm (`x =>`, counted), `?`, `loop`/`break`, and every scenario of "Rust decision points". Then the counting until they pass.
- [x] 1.4 Failing fixture tests for macros and tokens (D4, D5): every scenario of "Macros in Rust", "Rust normalized tokens" and "Rust code without tokens", plus raw strings, byte strings, raw identifiers (`r#type`), lifetimes, an inner attribute `#![...]` and a `macro_rules!` definition (no unit, tokens kept). Then the tokens until they pass.
- [x] 1.5 Fingerprints: a unit's fingerprint ignores whitespace, comments and the code of nested `fn` items, and changes with its own code (unit tests, as for C#). Syntax errors give `ParseError::Syntax` with the first error line. Record the release binary's size before and after the grammar in `design.md`.

## 2. Wiring, self-check and version (PR 2, `change/add-rust-analysis-2`)

- [x] 2.1 Analyzer fixture pairs: `crates/qgate-analyzers/fixtures/complexity/rust_*` (the scenario "A Rust method that got more complex", a new complex function, a trait impl's `fmt` moved below another impl) and a Rust duplication pair, failing tests first.
- [x] 2.2 CLI, revision mode: `.rs` among changed files, one duplication index per family through `Family::ALL`, Rust hotspots ranked among Rust files, no `tests/` finding for Rust. Failing CLI tests first for each scenario of the modified `revision-input`, `duplication-analysis`, `hotspot-analysis` and `test-analysis` requirements, including the working tree.
- [x] 2.3 CLI, two-file form: `.rs` accepted, Rust with another family rejected, the unsupported-type message names `.rs`. Failing CLI tests first for the scenarios of "Supported input files".
- [ ] 2.4 Replay (D8) over the 51 merges of `main`: script in the scratchpad, per pull request the findings by rule and severity, unparsable files, hotspot context and run time; every Error and at least 20 Warnings classified as useful, debatable or noise. Results in "Self-check results" of `design.md`. Stop and ask Ervin if the numbers argue for changing D3 to D6 before the release.
- [ ] 2.5 README: Rust in the status line, the checks it gets, its decision points and names, macros and `?`, the two-file form; the roadmap row "Languages" names Rust. Check every sentence against a test or the replay.
- [ ] 2.6 Version 0.9.0 in `Cargo.toml` (with `Cargo.lock`) and every `QGATE_VERSION` of `action.yml`; both release checks pass locally for `v0.9.0`.

## 3. Release 0.9.0 (between PR 2 and PR 3)

- [ ] 3.1 After PR 2 merges and Ervin agrees, push `v0.9.0` and check the draft: six archives, `SHA256SUMS`, `sha256sum --check`, the Linux arm64 binary printing `qgate 0.9.0` and analyzing a `.rs` pair. Put the drafted release notes into the draft; Ervin publishes. Record it in `design.md` on `change/add-rust-analysis-3`.

## 4. Self-check in CI and while coding (PR 3, `change/add-rust-analysis-3`)

- [ ] 4.1 Every README install block, `uses:` and pinned commit at 0.9.0; `grep` finds no `0.8.0` outside release records.
- [ ] 4.2 The `qgate` job in `.github/workflows/ci.yml` (D9): `pull_request` only, `contents: read`, `pull-requests: write`, `fetch-depth: 0`, `uses: nagypalervin/qgate@v0.9.0`, `fail-on: error`. Verified on PR 3 itself: the job runs and its comment appears (or the "no regressions" result is shown). Add the pin bump to the release record's checklist.
- [ ] 4.3 Install the released 0.9.0 binary into `~/.local/bin` on Ervin's machine; commit the Stop hook (`qgate hook claude-code --base main`) in `.claude/settings.json`, keeping `OPENSPEC_TELEMETRY`, and `.mcp.json` with `qgate mcp` (D10). Verified: `claude mcp list` shows qgate connected, and a turn that leaves a deliberately complex function in the working tree is sent back once (then the function is removed). README "Building" says the repository uses both and needs `qgate` on the `PATH`.
- [ ] 4.4 The roadmap row "Languages" says Rust is done; the follow-ups the replay argued for (test code, Rust tests, `use` rules, the unused `qgate-core` dependency) are listed in the journal entry for Ervin to order.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- On PR 3: the `docs/journal/` entry (with the replay's numbers for the article), then `/opsx:archive`.
- Gate 3: Ervin merges each pull request.
- Ervin decides whether the `qgate` CI job becomes a required check in "Protect main".
