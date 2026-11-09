# Tasks

One pull request, branch `change/analyze-wrapped-functions`. Decisions made
during apply go into `design.md`. Every task ends with `cargo fmt --all`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` passing, chained with `&&` only.

## 1. Fixtures and failing tests

- [ ] 1.1 Move the `GET` case out of `crates/qgate-lang/fixtures/typescript/module_level.ts`, add the `app.get(...)` and `module.exports = wrap(...)` cases to it, and keep `code_without_named_ancestor_is_not_measured` expecting no unit; verify the test still passes
- [ ] 1.2 Add `wrapped_calls.ts`, `bound_call_units.ts` and `wrapped_inside_function.ts` fixtures (design D5) covering every scenario of "Calls at module level do not hide the binding", "A bound call is one unit" and "Function wrapped in a call at module level", with tests in `typescript.rs` that pin each unit's name, line and complexity; verify the new tests fail and `wrapped_inside_function` already passes

## 2. Implementation

- [ ] 2.1 Implement bound calls in the `Walker` (design D1 to D4): the bound-call stack, `pass_through_top`, the scan for a function that passes through, and entering values that pass through; verify the 1.2 tests pass, every existing `qgate-lang` test passes unchanged, and record decisions in `design.md`
- [ ] 2.2 Check that the tests bite: break the pass-through edges (arguments, the `function` field, `new`, `await`), the summing, the "no unit without a function" rule and the module-level condition one at a time, and confirm a test fails for each
- [ ] 2.3 Update the crate docs in `crates/qgate-lang/src/lib.rs` and the `Function` doc in `model.rs`, which say that code outside named functions is not measured; verify `cargo doc -p qgate-lang --no-deps` builds without warnings

## 3. End to end

- [ ] 3.1 Add a two-file end-to-end test in `crates/qgate-cli/tests/check.rs` (fixture `wrapped/before.ts` and `wrapped/after.ts`) where `export const GET = withAuth(async () => {...})` grows from 2 to 4; verify the output is `warning wrapped/after.ts:<line>  GET: cyclomatic complexity 2 -> 4 (+2)  [complexity/cyclomatic-increase]` and the exit code is 0

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- Write the `docs/journal/` entry, then archive the change on this branch (`/opsx:archive`).
- Gate 3: the human merges.
