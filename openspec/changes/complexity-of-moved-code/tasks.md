# Tasks

Two pull requests from `change/complexity-of-moved-code` (design D5).
Every commit passes the pre-commit checks, and commit messages and the
PR bodies are written to files first.

## 1. Moved functions (PR 1)

- [x] 1.1 Failing tests first, in the `complexity.rs` unit tests, with before/after fixture pairs under `fixtures/complexity/`, for the scenarios:
  - "Method moved to a class in another file", "Moved and grown" and "Moved function message" (C#);
  - "Moved out of a deleted file", "Moved to another class in the same file", "Ambiguous own name" and "Same code settles an ambiguous own name" ("Another language family" is a CLI test, 1.4: the analyzer is given one family at a time);
  - the own-name split of D2 on `Cart.Total(System.Int32)`, `<Id as From<a::B>>.from`, `Cart.get size` and `A.create#2`.

  `renamed_function_is_removed_plus_new` stays as it is. Verified when `cargo test -p qgate-analyzers` fails on exactly the new tests.
- [x] 1.2 Add `complexity::analyze(changes, gone)` with the matching by qualified name and then by own name (D1, D2), and the "moved from" message (D4). `ComplexityAnalyzer::analyze` calls it with one change. Verified when `cargo test --workspace` passes, along with `cargo fmt` and `clippy -D warnings`.
- [x] 1.3 In the CLI, parse each gone file once into a `FileModel`, give duplication its tokens, and call complexity once per family after the per-file loop (D1). Verified when the existing CLI tests pass unchanged.
- [x] 1.4 Add a CLI test in `crates/qgate-cli/tests/` for a C# method of complexity 12 that moves from one class into a new class in a new file, while its old file is deleted. Verified when `qgate check --base` reports no finding, and the test fails if 1.3 is reverted. Also "Another language family", in `tests/rust.rs`.
- [x] 1.5 Re-measure with the branch's release binary (see 3.1 for the method). Verified when SV19 is gone, every other difference from the baseline is a moved unit, and the slowest run stays within 30 s. Record the result in "Verification" of `design.md`.

From the review of PR 1 (Ervin's decisions of 2026-11-17, D1 and D2):

- [x] 1.6 Failing tests first:
  - in `qgate-lang`, every adapter's tokens carry the unit of their own code, and `None` outside units; a nested named unit's tokens carry its own unit, not the enclosing one;
  - in `complexity.rs`, a fixture for "Same own name, different code", and a unit test of the in-order share;
  - a CLI test for "Moved out of a deleted JavaScript file".

  Verified when the new tests fail and no other test does.
- [x] 1.7 Add `Token::unit`, recorded by `Tokens::push` from the unit each adapter passes to the fingerprint (D3). Verified when the `qgate-lang` tests pass, and the duplication tests pass unchanged.
- [x] 1.8 Settle each own-name group in two passes, same code then code evidence (D2), and leave the units of deleted JavaScript files out of complexity (D1). Verified when `cargo test --workspace` passes, along with `cargo fmt` and `clippy -D warnings`.
- [x] 1.9 Re-measure as in 1.5. Verified when the result is the same as 1.5's, or every difference is explained, and the slowest run stays within 30 s.

From the second review of PR 1 (Ervin's decision of 2026-11-17, D2):

- [x] 1.10 Failing tests first: a fixture for "Short removed namesake"; the in-order share counts both units; a group whose first pass settles one pair leaves only the rest to the second (removed `A`, `B`; after `C` with the code of `A`, and `D` with evidence for both). Verified when the new tests fail and no other test does.
- [x] 1.11 Make the evidence of a move half of both units' own tokens (D2). Also from the review: the before model of a file renamed from JavaScript drops its tokens' units, since it has no functions; the TypeScript token test covers a bound call. Verified when `cargo test --workspace` passes, along with `cargo fmt` and `clippy -D warnings`.
- [x] 1.12 Re-measure as in 1.5. Verified when every difference from 1.9 is explained, and the slowest run stays within 30 s.

## 2. Inlined functions (PR 2)

- [x] 2.1 Failing tests first:
  - in `complexity.rs`, fixtures for "Inlined behind a feature flag", "Inlined and grown beyond it", "Inlined function message" and "Removed but not inlined";
  - a case where one removed unit qualifies for two grown units, which credits only the larger share (D3).

  Verified when the new tests fail and no other test does.
- [x] 2.2 Credit inlined units in `complexity::analyze` (D3) and add the "beyond the inlined" message (D4). Verified when `cargo test --workspace` passes, along with `cargo fmt` and `clippy -D warnings`.
- [x] 2.3 Describe moved and inlined functions in the README's complexity section, with the two message forms. Verified by reading it against the spec delta.

From the re-measurement of PR 2 (Ervin's decision of 2026-11-17, D3):

- [x] 2.4 Make the evidence of inlining two thirds of both the removed unit's own tokens and the code the change added to the grown unit (D3), with tests for removed code that was already there and for a removed unit among much new code. Verified when `cargo test --workspace` passes, along with `cargo fmt` and `clippy -D warnings`, and the re-measurement of 3.1 is repeated.

From the review of PR 2 (Ervin's decision of 2026-11-17, D3):

- [x] 2.5 Pair inlined units one to one, the longest common subsequence first, and give no credit above 2^26 pairs of tokens in the alignment of the added code (D3). Tests: two grown units inlining one helper, one grown unit with two alike removed units, a moved unit that also inlined code, a moved unit not taken as inlined, the ties and the limit of `added`. Verified when `cargo test --workspace` passes, along with `cargo fmt` and `clippy -D warnings`, and the re-measurement of 3.1 is repeated.

## 3. Proof

- [x] 3.1 Re-measure the 150 pull requests of `evaluate-angular-dotnet` with the branch's release binary:
  - take `evaluation/run.py`, `commits.tsv` and the frozen `config/*.toml` from that branch (`git show change/evaluate-angular-dotnet:...`) into the scratchpad;
  - run against the same repository clones;
  - compare the complexity findings with the 0.9.0 reports in the scratchpad (`ev/out/`).

  Verified when every report parses.
- [x] 3.2 Check the differences. Verified when:
  - CL11 and SV19 are gone;
  - every other appeared, disappeared or changed complexity finding is a moved or inlined unit, which the pull request's diff confirms;
  - no other complexity finding judged useful or defensible in `findings.md` disappeared or got milder;
  - the slowest run stays within 30 s, also for server 8854, 8856 and 8859.

  The counts go into "Verification" of `design.md`.
- [x] 3.3 Check D3's threshold on CL11 and on every candidate pair near it. Verified by recording both shares of each pair that reaches 0.4 in its smaller one. The first rule (half) failed 3.2, so Ervin set two thirds both ways on the added code; the shares are recorded in "Verification".
- [x] 3.4 The journal entry for the change.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- Then `/opsx:archive` on the branch of PR 2. Gate 3: Ervin merges.
- After the duplication fix, the second measurement goes into `evaluate-angular-dotnet`. A release (0.10.0) is Ervin's call.
