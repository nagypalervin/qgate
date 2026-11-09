# Tasks

Three pull requests: `change/add-duplicate-detection` (tokens),
`-2` (reading the head tree) and `-3` (the analysis, the CLI and the
README). Decisions made during apply go into `design.md`. Every commit
passes `cargo fmt`, `cargo clippy -D warnings` and `cargo test
--workspace` (the pre-commit hook enforces it), and CI runs the tests on
Rust 1.85 too. Commit messages and PR bodies are written to files first.

## 1. Normalized tokens (PR 1)

- [x] 1.1 Add fixtures under `crates/qgate-lang/fixtures/typescript/` and failing tests in `typescript.rs` for design D1 and "Normalized tokens": a renamed copy gives the same token texts, different member names give different ones, literals and template-string fragments become `$lit`, comments are ignored, imports, `export ... from` and type-level code produce no tokens, and each token has its line; verify they fail
- [x] 1.2 Add `Token` and `FileModel::tokens` and emit them in the walk (D1); verify the 1.1 tests pass and every existing `qgate-lang` test passes unchanged
- [x] 1.3 Update the crate and `FileModel` docs; verify `RUSTDOCFLAGS="-D warnings" cargo doc -p qgate-lang --no-deps` builds

## 2. Reading the head tree (PR 2)

- [ ] 2.1 Add `Repository::files_at` (D2) with tests: regular and executable files in nested directories, sorted by path; symlinks and submodules left out; an option-like revision rejected without writing anything; verify they pass
- [ ] 2.2 Add `Repository::blob_reader` and `BlobReader::read` (D2) with tests: many blobs read through one reader with exact bytes (including non-UTF-8 and empty blobs), a missing id is an error, and the process ends when the reader is dropped; verify they pass
- [ ] 2.3 Update the `qgate-git` crate docs; verify `cargo doc -p qgate-git --no-deps` builds without warnings

## 3. The analysis, the CLI and the README (PR 3)

- [ ] 3.1 Add `qgate-analyzers::duplication` (D3, D4) with fixture-based tests (before/after pairs plus unchanged head files) for every scenario of "Duplicate blocks", "Only new duplication is reported" and "Duplicate block finding": the 49/50-token threshold, existing duplication, moved code, a copy within the change, a duplicate within one file, merged blocks and their line ranges, several existing copies, and a forced hash collision that is not reported; verify they pass
- [ ] 3.2 Wire the index into revision mode (D5) with end-to-end tests in `crates/qgate-cli/tests/revisions.rs` for "Copy of an existing file the change does not touch", "Message" (exit 0), "Failing on duplication" (exit 1 with `--fail-on warning`) and an unparsable unchanged head file, and in `check.rs` for "Two-file form"; verify they pass
- [ ] 3.3 Check that the tests bite: break, one at a time, the `delta > 0` condition, the old/new split per file, the original rule, block merging, member-name preservation and the import exclusion, and confirm a test fails for each
- [ ] 3.4 Measure revision mode on a generated repository of 2,000 TypeScript files (about 200,000 lines) with a 20-file change; record the time and peak memory in `design.md` (Risks); verify the numbers are recorded
- [ ] 3.5 Update `README.md`: the `duplication/duplicate-block` row in the rules table and a short duplication section (what counts, what is new, the threshold, the two-file form); and the `qgate-analyzers` crate docs; verify the README example produces the documented finding in a temporary repository

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- After PR 3: write the `docs/journal/` entry, then archive the change on PR 3's branch (`/opsx:archive`).
- Gate 3: the human merges each pull request.
