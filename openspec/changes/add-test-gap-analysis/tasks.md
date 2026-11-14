# Tasks

Three pull requests, with the 0.7.0 release between the second and the
third (design D11). Tests come first in every task; every commit
passes the pre-commit checks; commit messages and PR bodies are
written to files first.

## 1. Unit fingerprints (PR 1, `change/add-test-gap-analysis`)

- [x] 1.1 `qgate-lang`, TypeScript and TSX: failing adapter tests for `Function::fingerprint` (D1), then the field and its computation in the existing walk. Same fingerprint for other layout and for added comments; the same for a changed type annotation (TypeScript types are not code, decided during PR 1, design D1); a different one for a changed literal and a changed name; a nested named unit's change leaves its parent alone; a nested arrow function's change changes its parent. Verified by `cargo test -p qgate-lang`.
- [x] 1.2 `qgate-lang`, C#: the same cases for C#, plus a local function (its own unit), a lambda in a LINQ query (part of its method) and code in an `#if` branch. Then the computation. Verified by `cargo test -p qgate-lang`. No output of `qgate check` changes: the existing end-to-end tests pass unchanged.

## 2. Test analysis (PR 2, `change/add-test-gap-analysis-2`)

- [ ] 2.1 `qgate-config`: failing tests for `[tests]` (`exclude` given, empty, missing table, unknown key `exlude`), then `Tests` with `deny_unknown_fields` (D7). Verified by `cargo test -p qgate-config`.
- [ ] 2.2 `qgate-analyzers::tests`, the conventions (D3, D5): failing unit tests over path lists for every scenario of "TypeScript tests", "C# projects", "C# tests" and "Projects that test other code". That includes `Shop.Testing`, `Shop.Api.Tests` not testing `Shop`, two project files in one directory, `.mts`, a monorepo, and a file outside any project. Then the project index and the pairing. Verified by `cargo test -p qgate-analyzers`.
- [ ] 2.3 `qgate-analyzers::tests`, the findings (D2, D4, D6): fixture-based tests with before/after pairs for "Changed units" (body, layout and comments only, only a TypeScript type annotation, imports and types only, a removed function, a rename, a C# overload) and for both findings. Cover every message form (one unit, several, several unchanged tests, C# expected test project), the line of the first changed unit, a test changed by rename, an excluded spec counted as changed, and `[tests] exclude`. Then the analysis. Verified by `cargo test -p qgate-analyzers`.
- [ ] 2.4 `qgate-cli`: failing end-to-end tests in temporary repositories for the scenarios of `test-analysis` that run `qgate check`, including `--worktree`, the two-file form, the unparsable file and "Other analyzers still run"; one MCP `check` call that returns a `tests/unchanged-test` finding; the three configuration scenarios. Then the wiring: one head file list shared with duplication, raw changed paths, `tests.exclude` copied into `PathPatterns` (D7, D8). Verified by `cargo test --workspace`.
- [ ] 2.5 README: a "Tests changed with the code" section covering both rules, the conventions, the scope, `[tests] exclude`, how `[files] exclude` treats tests, and the limits named in the proposal's "Out of scope". Update the `qgate-analyzers` layout line and the crate docs' "Planned" list. Check each sentence against a test. Version 0.7.0 in `Cargo.toml` (with `Cargo.lock`) and every `QGATE_VERSION` of `action.yml`; both release checks pass locally for `v0.7.0`. Draft the release notes with the `--fail-on warning` note (D9).
- [ ] 2.6 Evaluation (D10) on `bitwarden/clients`, `bitwarden/server`, `jasontaylordev/CleanArchitecture` and, once, `dotnet/eShop`. Sort every finding by hand, record the time per commit against 0.6.0 and count the edited existing files without a test; record it all in `design.md`. Misses above a third of a repository's findings stop the release until Ervin decides.

## 3. Release 0.7.0 (between PR 2 and PR 3)

- [ ] 3.1 After PR 2 merges and Ervin agrees, push `v0.7.0` and check the draft: six archives, `SHA256SUMS`, `sha256sum --check`, the Linux arm64 binary printing `qgate 0.7.0` and reporting a `tests/unchanged-test` finding. Ervin publishes. Record it in `design.md` on `change/add-test-gap-analysis-3`.

## 4. README, roadmap and rehearsal (PR 3, `change/add-test-gap-analysis-3`)

- [ ] 4.1 Every README install block, `uses:` and the pinned commit at 0.7.0; `grep` finds no `0.6.0`. The M5 roadmap row records "code changed without its tests (0.7.0)" and keeps hotspots planned.
- [ ] 4.2 Azure rehearsal (D10): the README's Linux job at 0.7.0 in `teamcollabeu/qgate-rehearsal`, on a pull request that edits a tested C# method and a tested TypeScript function without their tests. Read the comment thread and the build issues through the REST API and record them in `design.md`: two Warning rows, two warning issues, a passing step. Abandon the pull request afterwards.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- On PR 3: the `docs/journal/` entry, then `/opsx:archive`.
- Gate 3: Ervin merges each pull request.
