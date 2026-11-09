# Tasks

One pull request, branch `change/add-fail-on`. `design.md` is skipped (one
crate, no dependency, no migration); decisions made during apply go into
the proposal's "Implementation Notes". Every task ends with
`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` passing, chained with `&&` only.

## 1. Fail on findings

- [x] 1.1 Write failing end-to-end tests in `crates/qgate-cli/tests/check.rs` for every scenario of "Failing on findings" that the two-file fixtures can express (Error finding exits 1, warnings pass, `--fail-on warning`, `--fail-on never` with identical stdout, a missing file with `--fail-on warning` exits 2, the summary line for two Errors and one Warning, no summary on a passing run) and for "Unknown fail-on level"; replace `error_finding_exits_zero`
- [x] 1.2 Write failing end-to-end tests in `crates/qgate-cli/tests/revisions.rs` for "Unparsable file in revision mode" (exit 0 by default, 1 with `--fail-on warning`) and "Errors that stop the check win" (unknown base with `--fail-on warning` exits 2)
- [x] 1.3 Implement `--fail-on error|warning|never` in `qgate-cli`: a clap value parser restricted to the three values, the level compared with `Severity`'s order, exit 1 after the report is written, and the summary line on standard error; a broken output pipe does not change the exit code. Verify the 1.1 and 1.2 tests pass and record decisions under "Implementation Notes" in the proposal
- [x] 1.4 Add `--fail-on never` to the "Smoke test qgate check" step in `.github/workflows/ci.yml`, and verify the step passes on the pull request
- [x] 1.5 Update `README.md`: exit codes (0, 1 with `--fail-on`, 2), the default and how to opt out, and the "In CI" section; verify every command shown behaves as described

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- Write the `docs/journal/` entry, then archive the change on this branch (`/opsx:archive`).
- Gate 3: the human merges.
