# Tasks

One pull request on `change/report-errors-first`, then a release (0.11.0)
if Ervin agrees.

## 1. Order and rule summary

- [x] 1.1 Add the failing tests first, in `crates/qgate-report/src/markdown.rs`: the scenarios of the spec delta: "Errors before Warnings", "Text order within a severity", "Errors after many Warnings in path order", "Hidden findings of two rules", "Every finding fits" (no rule summary at exactly 50), plus a singular count (`1 error`) and a rule with Info findings. Verify that the order and summary tests fail, and that the existing tests still pass.
- [x] 1.2 Sort a copy of the findings stably by severity, descending, before the table and the cut (design D1). Update the doc comments of `render_markdown` and `MAX_ROWS`. Verify that the order tests of 1.1 pass.
- [x] 1.3 Write the rule summary when `count > MAX_ROWS` (design D2): per rule, counts per severity, most severe first, singular for one, lines ordered by Errors, Warnings, Infos (descending), then rule id. Verify that every test of 1.1 passes.
- [x] 1.4 End-to-end: a test in `crates/qgate-cli/tests/formats.rs` with an Error in a later path than a Warning, checking that `--format markdown` lists the Error first while `--format text` and `--format json` keep path order (spec scenario "Other formats keep their order"). Verify that it passes.
- [x] 1.5 Run the mutation check by hand, each reverted: no sort; an unstable sort or reversed order within a severity; summary always shown; summary counting only hidden findings. Verify that each fails a test, and record the result in design.md "Verification".
- [x] 1.6 README "What the pull request shows" (GitHub) and the Azure section: the table lists Errors first, and a cut table adds the per-rule counts. Verify that both sections say so and no other README text names the order.
- [x] 1.7 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the Pester tests of `ci/`, and verify that all pass.

## 2. Replay

- [x] 2.1 Render the Markdown report of the 150 public evaluation pull requests (`openspec/changes/archive/2026-11-18-evaluate-angular-dotnet/evaluation/`) with this branch's binary. Verify that no Error is hidden where a Warning is shown, count the pull requests where the old order hid an Error, and record that count in design.md "Verification". Partner measurements stay outside this repository.

## 3. Review and journal

- [x] 3.1 Run the `reviewer` subagent on the branch and fix its findings. Verify that the checks of 1.7 pass again.
- [x] 3.2 Add `docs/journal/2026-11-18-report-errors-first.md` (dated on the day of the work), following `docs/journal/README.md`. Verify that it names the lesson in generic terms only.

## Workflow follow-up

- Before every commit: the staged diff passes the partner deny-list check (outside this repository).
- Archive the change on the same branch, before the pull request merges (`/opsx:archive`), so code and living spec land on `main` together.
- Open the pull request; the human merges it (Gate 3). Then the version bump and tag, as for 0.10.0; Ervin publishes.
