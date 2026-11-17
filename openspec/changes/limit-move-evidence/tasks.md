# Tasks

One pull request on `change/limit-move-evidence`, no release.

## 1. The limit of the move evidence

- [x] 1.1 Add the failing tests first, in `crates/qgate-analyzers/src/complexity.rs`: a unit test of `shares_half` (half shared, but middles above 2^26 pairs → false; same size, one middle token differs → true) and the three analyzer tests of the spec's "Limit of the move evidence" (C#, `run_files`, generated sources: both ends changed → Warning `complexity/cyclomatic-new`; one token changed → no finding; unchanged → no finding). Verify that the "both ends" test and the unit test fail, and that they fail on the result, not by a timeout.
- [x] 1.2 Extract the cut of the common start and end from `added` into a helper and use it in `added`. Verify with `added_code` and `added_code_has_a_limit`, unchanged and passing.
- [x] 1.3 Cut and limit in `shares_half` (design D1, D2): after the size check, cut the common start and end, return false above `ALIGNED_PAIRS` pairs of the middles, and otherwise compare half of the longer unit with the cut length plus `common_in_order` of the middles. Update the doc comments of `shares_half`, `moves` and `ALIGNED_PAIRS`. Verify that the tests of 1.1 and every existing move test pass.
- [x] 1.4 Run the mutation check of design D3 (no limit, no cut, no cut length in the sum), each by hand and reverted. Verify that each fails a test, and record the result in design.md "Verification".
- [x] 1.5 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and verify that all three pass.

## 2. Re-measurement

- [x] 2.1 Build the release binary and replay the 150 pull requests (design D4): take `evaluation/run.py`, `commits.tsv` and `config/*.toml` from `change/evaluate-angular-dotnet` with `git show`, and compare with the 0.9.0 reports in the earlier session's scratchpad `ev/out` (regenerate them from v0.9.0 if gone) using `compare.py` from the archived `complexity-of-moved-code`. Verify that complexity loses only SV19 and CL11, that the 326 other-rule findings are byte-identical, and that the slowest run stays well within 30 s. Record the result in design.md "Verification". If the result differs, stop and ask.

## 3. Review and journal

- [ ] 3.1 Run the `reviewer` subagent on the branch and fix its findings. Verify that the checks of 1.5 pass again.
- [ ] 3.2 Add `docs/journal/2026-11-17-limit-move-evidence.md` (dated on the day of the work), following `docs/journal/README.md`. Verify that it names the decision on the cut and the replay result.

## Workflow follow-up

- Archive the change on the same branch, before the pull request merges (`/opsx:archive`), so code and living spec land on `main` together.
- Open the pull request; the human merges it (Gate 3).
