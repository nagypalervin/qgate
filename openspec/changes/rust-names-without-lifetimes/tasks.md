# Tasks

One pull request, `change/rust-names-without-lifetimes`. Every commit
passes the pre-commit checks, and commit messages and the PR body are
written to files first.

## 1. Baseline

- [x] 1.1 Build the release binary from `main` and run the replay over all merges of `main` (the script of `share-adapter-walker`, in the scratchpad). Keep the reports, without run times, as the baseline in the scratchpad. Verified when the number of reports equals the number of merges.

## 2. Names without lifetimes

- [x] 2.1 Failing tests first, in the `rust.rs` unit tests:
  - the scenarios "Lifetimes are not part of the name" and "A comment inside a type";
  - the expectation for `fixtures/rust/units.rs` changed from `Parser<'a>.parse` to `Parser.parse`;
  - a const-generic char argument (`impl Code<'x'>`) keeping its `'x'`.

  Verified when `cargo test -p qgate-lang` fails on exactly these.
- [x] 2.2 Render the `impl` segment from the syntax tree (D2). Verified when `cargo test --workspace` passes, along with `cargo fmt` and `clippy -D warnings`.
- [ ] 2.3 A CLI test for "A lifetime renamed" in `crates/qgate-cli/tests/rust.rs`. A method of complexity 11 moves from `impl<'s> Walker<'s>` to `impl Walker<'_>`. Verified when `qgate check --base` reports no finding; the test fails if the change from 2.2 is reverted.
- [ ] 2.4 The README's Rust section says that lifetimes are not part of a unit's name, with an example (`Walker.record_token`). Verified by reading it against the spec delta.

## 3. Proof

- [ ] 3.1 Replay with the branch's release binary, compared with the baseline from 1.1 (design, Risks). Verified when every difference is in a unit name that contained `'` on `main`, or is a finding that disappeared because of such a name. The differences are counted in "Verification" of `design.md`.
- [ ] 3.2 Analyze qgate's own source with the branch's binary. Verified when no unit name contains `'`. The result is recorded in "Verification" of `design.md`.
- [ ] 3.3 The journal entry for the change.

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- Then `/opsx:archive` on the same branch; Gate 3: Ervin merges.
