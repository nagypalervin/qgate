# Tasks

Two pull requests:
- `change/add-report-formats`: the core fields and the duplication
  analyzer filling them in, together with these planning artifacts. The
  text output does not change.
- `change/add-report-formats-2`: the JSON renderer, `--format`, the JSON
  Schema, the end-to-end scenarios and the README.

Decisions made during apply go into `design.md`. Every commit passes
`cargo fmt`, `cargo clippy -D warnings` and `cargo test --workspace` (the
pre-commit hook enforces it), and CI also runs the tests on Rust 1.85.
Commit messages and PR bodies are written to files first.

## 1. Ranges and related locations in findings (PR 1)

- [x] 1.1 Add `Location::end_line` and `Finding::related` to `qgate-core` (design D1), with doc comments, and set them to `None` and empty at every existing construction site. Verify that `cargo test --workspace` passes unchanged and that the text output tests in `crates/qgate-cli/tests/` need no edit.
- [ ] 1.2 Write failing unit tests in `crates/qgate-analyzers/src/duplication.rs` for design D2, using before/after fixtures as the existing tests do:
  - a block copied from one file: `end_line` is the new occurrence's last line, and `related` holds that file with the lines the message names;
  - copies in three files: `related` holds one location per file, in path order, the first being the file the message names;
  - two copies in one file: `related` holds the first by line;
  - a copy whose counterpart is longer than the block: the related `end_line` covers only the block, as the message does.

  Verify that they fail.
- [ ] 1.3 Fill in `end_line` and `related` in the duplication analyzer (design D2). Verify that the 1.2 tests pass and every message stays byte-identical (the existing duplication tests and `crates/qgate-cli/tests/revisions.rs`).
- [ ] 1.4 Check that the tests bite. Break each of these, one at a time, and confirm that a test fails for each:
  - the end line (use the first line);
  - one related location per file (keep every copy);
  - the related order (reverse it);
  - the related `end_line` (use the copy's own last line).

## 2. The JSON report and `--format` (PR 2)

- [ ] 2.1 Add `serde_json` to the workspace and to `qgate-report`, saying why in the commit message (design D3). Write failing unit tests in `crates/qgate-report/src/lib.rs` for `render_json`:
  - the exact bytes of the spec's "One finding", "No findings", "New function" and "Duplicate block" scenarios;
  - no `null` anywhere;
  - whole metric values written as integers, and a fractional one as a float (design D4);
  - a message with `"`, `\` and a control character, escaped;
  - a path that is not valid UTF-8, written with U+FFFD (design D7).

  Verify that they fail.
- [ ] 2.2 Implement `render_json` and `ReportSummary` with private mirror structs (design D3–D5), and update the crate docs (the "JSON follows in M3" line). Verify that the 2.1 tests pass and that `RUSTDOCFLAGS="-D warnings" cargo doc -p qgate-report --no-deps` builds.
- [ ] 2.3 Write `docs/schema/report-v1.json` (design D8). In `crates/qgate-cli/tests/`, add the schema test helper and its own tests:
  - an invalid report (a missing key, a wrong type, a `null`) is rejected;
  - a valid one is accepted;
  - the schema uses only the supported keywords.

  Add `serde_json` as a dev-dependency of `qgate-cli`. Verify that the helper tests pass.
- [ ] 2.4 Write failing end-to-end tests, one per scenario:
  - `report-formats`: Default is text, JSON in every form, One finding, No findings, New function, Finding without a line, Duplicate block, Characters JSON must escape, Complexity values, Several files, Failing run, Never failing, Unknown base revision, Summary line on standard error, Reports match the published schema, Path from the two-file form;
  - `check-command`: Unknown format;
  - `duplication-analysis`: Range and related locations.

  Each one builds a temporary repository as the existing tests do. Verify that they fail.
- [ ] 2.5 Add `--format` to `qgate-cli` (design D6): the value enum, the help text, `[--format <format>]` in every usage line, and the summary passed to `render_json`. Verify that the 2.4 tests pass and every existing test passes unchanged.
- [ ] 2.6 Check that the tests bite. Break each of these, one at a time, and confirm that a test fails for each:
  - integer output (always write floats);
  - leaving out empty fields (write `null`);
  - the `failing` count (count all findings);
  - the key order of the envelope.
- [ ] 2.7 Update `README.md`:
  - a "JSON output" section: an example report, every field, the versioning promise, that messages are for people and may be reworded, the schema file, and `jq` for pretty printing;
  - `--format` in "Try it";
  - the "In CI" and "Checking uncommitted work" examples where a script would use JSON.

  Run every README command in a temporary repository and verify that it behaves as documented.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- Write the `docs/journal/` entry, then archive the change on the branch of PR 2 (`/opsx:archive`).
- Gate 3: the human merges each pull request.
- Check whether the README roadmap's M3 line needs an update once JSON output is merged.
