# Tasks

Two pull requests with the 0.5.0 release between them (design D6).
Tests come first in every task; every commit passes the pre-commit
checks; commit messages and PR bodies are written to files first.

## 1. The setting (PR 1, `change/add-path-exclusion`)

- [x] 1.1 `qgate-config`: failing tests for `[files]` with `exclude` (a list, an empty list, a missing table, an unknown key in `[files]`, a wrong type), then the `Files` table (D1).
- [x] 1.2 `qgate-analyzers`: move the pattern code into `patterns.rs` as `PathPatterns` (D2) with unit tests for matching and spelling; the architecture analyzer uses it, and its existing tests pass (their helper now calls `matches` instead of `is_match`).
- [x] 1.3 `qgate-cli`: failing CLI tests for each scenario of "Excluded paths" and for an excluded `.js` file renamed to `.ts`, then `FileSelection` replacing the free `language_of` in every caller (D3) and the error of D5.
- [x] 1.4 README: the `[files] exclude` setting in the `qgate.toml` section, and the C# section's note on migrations pointing to it; check each sentence against a test.
- [x] 1.5 Version 0.5.0 in `Cargo.toml` (with `Cargo.lock`) and every `QGATE_VERSION` of `action.yml`; both release checks pass locally for `v0.5.0`.

## 2. Release 0.5.0 (between the pull requests)

- [ ] 2.1 After PR 1 merges and Ervin agrees, push `v0.5.0` and check the draft: six archives, `SHA256SUMS`, `sha256sum --check`, the Linux arm64 binary printing `qgate 0.5.0`. Ervin publishes. Record it in `design.md` on `change/add-path-exclusion-2`.

## 3. README and rehearsal (PR 2, `change/add-path-exclusion-2`)

- [ ] 3.1 Every README install block, `uses:` and the pinned commit at 0.5.0; `grep` finds no `0.4.0`.
- [ ] 3.2 In the GitHub rehearsal repository, run the README workflow (`@v0.5.0`) on a pull request that adds a migration repeating an older one, with and without `exclude = ["**/Migrations/**"]`; read both comments through the API and record them in `design.md`.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- On PR 2: the `docs/journal/` entry, then `/opsx:archive`.
- Gate 3: Ervin merges each pull request.
