# Tasks

Two pull requests on `change/duplication-of-new-code` (design D7), no
release. The baseline of every replay is `main` after PR 43, the
reports in the scratchpad's `ev/out-lme` (regenerate them from `a9a1c74`
if gone). The replay tools come from `change/evaluate-angular-dotnet` with `git
show`, and `compare.py` from the archived `complexity-of-moved-code`.

## 1. Line hashes (PR 1)

- [x] 1.1 Add the failing tests first in `crates/qgate-lang`: `FileModel::lines` through `parse` for TypeScript, C# and Rust sources with CRLF line ends, leading and trailing white space, an empty line, a comment-only line and no trailing newline (design D2). Verify that they fail to compile or fail on the result.
- [x] 1.2 Add `FileModel::lines` (doc comment: one FNV-1a hash per line of the trimmed text, index = line − 1) and set it in `qgate_lang::parse` after the adapter. Verify that the tests of 1.1 and every existing `qgate-lang` test pass.

## 2. The line diff (PR 1)

- [x] 2.1 Add the failing tests first in a new `crates/qgate-analyzers/src/diff.rs`: `kept` against a brute-force LCS on generated small inputs with a fixed seed (equal kept counts, kept elements equal and in order), the common start and end cut, and one difference past `MAX_EDITS` giving the fallback (design D5, D6). Verify that they fail.
- [x] 2.2 Move `middles` from `complexity.rs` into `diff.rs`, generic over `T: PartialEq`, and use it from `complexity.rs`. Verify that every complexity test passes unchanged.
- [x] 2.3 Implement `kept` with Myers' greedy algorithm on the middles and the `MAX_EDITS` limit of 2^11 differences, with doc comments for both. Verify that the tests of 2.1 pass, and that mutants without the cut and without the limit each fail a test.

## 3. Plumbing and the first pull request (PR 1)

- [x] 3.1 Add `before_lines` and `after_lines` to `ChangedSource` (design D4) and fill them in `crates/qgate-cli/src/main.rs` and in the duplication tests' helpers. The analyzer does not read them yet. Verify that `cargo test --workspace` passes unchanged.
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. Verify that all three pass.
- [x] 3.3 Build the release binary and replay the 150 pull requests. Verify that every finding is identical to the baseline and that the slowest run does not grow noticeably. Record the result in design.md under a new "Verification" section.
- [x] 3.4 Run the `reviewer` subagent on PR 1's commits and fix its findings. Verify that the checks of 3.2 pass again. Then open PR 1. The human merges it.

## 4. Written lines in the duplication analyzer (PR 2)

- [x] 4.1 Add the failing fixture tests first in `crates/qgate-analyzers/src/duplication.rs`, one per new spec scenario: "The same small edit in existing copies", "A copy completed by a few written lines", "A new copy in an edited file", "A split file", "A helper copied into two files", "Moved within a file and copied", "Moved and renamed". Verify that the ones the rule changes fail on the result, and that the others pass already.
- [x] 4.2 Pair moved lines in `DuplicationIndex::new` (design D3): diff each changed file, build removed and added runs of three with at least one non-empty line, pair them one to one with the own-file pass first and then the other files in path and line order, and keep `written` per changed file and line. Verify with a unit test of the pairing, covering the own-file pass, consumption and the empty-run rule.
- [x] 4.3 Count only tokens on written lines in `blocks()` (design D1) and update the module and function doc comments. Verify that the tests of 4.1 and every existing duplication test pass.
- [x] 4.4 Add an end-to-end test in `crates/qgate-cli/tests/` (a git repository built like the existing ones): a file split into two new files and deleted, and a JavaScript file renamed to TypeScript with a moved block, as review finding 2 of PR 1 asked. Verify that no duplication finding is reported, and that swapping `before_lines` and `after_lines` in the CLI, or leaving them empty, fails the test.
- [x] 4.5 Run the mutation check of design D6 (no own-file pass, no consumption, runs of one line, empty runs allowed, untrimmed hashes, no written-line condition), each by hand and reverted. Verify that each fails a test, and record the result in design.md "Verification".
- [x] 4.6 Update the README's "Duplicated code" section: only code the change wrote counts, and moved lines are matched by their trimmed text in runs of three, also when repetition within one file becomes repetition across files. Verify that the text matches the spec delta.
- [x] 4.7 Run the checks of 3.2. Verify that all three pass.

## 5. Re-measurement and review (PR 2)

- [x] 5.1 Build the release binary and replay the 150 pull requests. Verify:
  - the judged duplication findings meet the proposal's bar: 6 of 6 useful and 12 of 13 defensible stay, with only SV30 gone; 20 of 22 noise go, with SV8 and SV27 staying (SV23 instead of SV8, explained in design.md and accepted by Ervin);
  - duplication falls from 279 to about 136;
  - every finding of another rule is identical;
  - the slowest run does not grow noticeably.

  Record the result in design.md "Verification". If a judged finding differs, stop and ask.
- [x] 5.2 Run `qgate check` on the branch itself (the MCP `check` tool). Verify that its findings are fixed or explained in the pull request.
- [x] 5.3 Run the `reviewer` subagent on the branch and fix its findings. Verify that the checks of 3.2 pass again.
- [x] 5.4 Add `docs/journal/<date>-duplication-of-new-code.md`, dated on the day of the work, following `docs/journal/README.md`. Verify that it names Ervin's decisions (proposal items 1 to 9) and the replay result.

## Workflow follow-up

- Archive the change on the branch of PR 2, before it merges (`/opsx:archive`), so code and living spec land on `main` together.
- Open PR 2; the human merges it (Gate 3). Release 0.10.0 at Ervin's call.
