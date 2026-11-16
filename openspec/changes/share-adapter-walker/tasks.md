# Tasks

One pull request (design D6), `change/share-adapter-walker`. No test or
fixture line changes; every commit passes the pre-commit checks; commit
messages and the PR body are written to files first.

## 1. Baseline

- [x] 1.1 Build the release binary from `main` and run the replay (design D5, check 2) over all merges of `main`; keep the reports without run times as the baseline in the scratchpad. Run check 3 on `main` and record the number of duplicates among the adapters in `design.md`.

## 2. The shared skeleton

- [x] 2.1 `walk` and the `Visit` trait in `common.rs` (D1); the Rust adapter moves to them first, as the smallest. Verified: `cargo test -p qgate-lang` passes unchanged.
- [x] 2.2 `Units<S>` (D2) and `Tokens` (D4) in `common.rs`; the Rust adapter uses them. Verified: `cargo test --workspace` passes unchanged.
- [x] 2.3 The C# adapter on `walk`, `Units<Option<usize>>` and `Tokens`. Verified: `cargo test --workspace` passes unchanged.
- [x] 2.4 The TypeScript adapter on `walk` and `Tokens`, and on `Units<Scope>` if it fits without a change of behavior (D3); the outcome and its reason recorded in `design.md`. Verified: `cargo test --workspace` passes unchanged.
- [x] 2.5 Module docs of `common.rs` and the three adapters say what is shared and what is each language's own; no doc mentions a removed item (`cargo doc -p qgate-lang` without warnings).

## 3. Proof and review

- [x] 3.1 Replay with the branch's release binary; the reports equal the baseline byte for byte (D5, check 2). Check 3 on the branch: none of the walker copies, no new copy. Both recorded in "Verification" of `design.md`. (Amended: the replay is byte-identical; 4 source copies remain, 1 a language rule and 3 short idioms, see "Verification" in `design.md`. Ervin accepted them on 2026-11-16.)
- [x] 3.2 The journal entry for the change, with the numbers of 1.1 and 3.1.

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- Then `/opsx:archive` on the same branch; Gate 3: Ervin merges.
