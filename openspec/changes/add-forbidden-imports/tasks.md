# Tasks

Three pull requests:

| Branch | Content |
|---|---|
| `change/add-forbidden-imports` | imports |
| `change/add-forbidden-imports-2` | configuration |
| `change/add-forbidden-imports-3` | analyzer and CLI |

Decisions made during apply go into `design.md`. Every task ends with
`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo test --workspace` passing; the pre-commit hook enforces it.

## 1. Imports in the language model (PR 1)

- [x] 1.1 Add fixtures under `crates/qgate-lang/fixtures/typescript/` and failing tests in `typescript.rs` for design D4: every form of "Imports that count" (including `import type`, side-effect imports, `export * from`, `import x = require()`, `import()` and `require()` inside a function), a multi-line import whose line is the specifier's, and the ignored forms (template literal with substitutions, identifier argument, `require` with two arguments); verify they fail
- [x] 1.2 Add `Import` and `FileModel::imports` and collect them in the walk (design D4); verify the 1.1 tests pass and every existing `qgate-lang` test passes unchanged
- [x] 1.3 Update the crate docs and the `FileModel` doc; verify `RUSTDOCFLAGS="-D warnings" cargo doc -p qgate-lang --no-deps` builds

## 2. Configuration (PR 2)

- [x] 2.1 Write ADR 0003 (design D1: the leaf crate `qgate-config` and the edge `cli -> config`) and update the layout and dependency lines in `CLAUDE.md` and `.claude/agents/reviewer.md`; verify the ADR is listed in `docs/adr/` and both files name `cli -> config`
- [x] 2.2 Create `crates/qgate-config` with `toml` and `serde` (design D2; the commit message says why) and unit tests for every non-pattern case of "Invalid configuration" (syntax error with its line, unknown table or key, wrong type, empty or duplicate name, empty `from` or `to`) and for a valid file; verify the tests pass
- [x] 2.3 Add `Repository::file_at` to `qgate-git` (design D5) with tests for a regular file, an executable file, a missing file, a directory, a symlink, an option-like revision and a path with glob characters; verify they pass
- [x] 2.4 Read `qgate.toml` in revision mode after the diff, before any file is analyzed (design D8, steps 1 and 2 without the analyzer; reordered at the PR 2 review) with end-to-end tests in `crates/qgate-cli/tests/revisions.rs` for "Missing configuration", every non-pattern scenario of "Invalid configuration", "Uncommitted changes are ignored" (with an invalid working-tree file, so ignoring it is observable) and, in `check.rs`, "Invalid qgate.toml next to two files"; verify they pass

## 3. The analyzer and the CLI (PR 3)

- [x] 3.1 Add `FileChange::old_path` (design D6) and set it in both CLI modes; verify all existing tests pass
- [x] 3.2 Add `qgate-analyzers::architecture` with `globset` (design D3, D7) and fixture-based tests (a before/after pair per case) for "Forbid rules", "Import targets", "Patterns", "Only new violations are reported" and "Forbidden import finding", plus `PatternError` for an invalid pattern; verify they pass
- [ ] 3.3 Run the analyzer in revision mode (design D8, step 3) with end-to-end tests in `revisions.rs` for "Domain importing the UI" (exit code 1), "Rules come from the head revision", "Moved into the domain", "Rule added in the same change", "Invalid pattern" and "Syntax error in a file with a forbidden import"; verify they pass
- [ ] 3.4 Check that the tests bite: break, one at a time, the base subtraction, the old path for renamed files, the `/**` directory match, the glob-less prefix match, the relative resolution and the type-only import form, and confirm a test fails for each
- [ ] 3.5 Update `README.md`:
  - the `architecture/forbidden-import` row in the rules table;
  - a `qgate.toml` section with the example rule, the pattern rules, aliases, directory patterns, and the head-revision rule;
  - the `qgate-analyzers` crate docs ("Available").

  Verify the README example produces the documented finding in a temporary repository.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- After PR 3: write the `docs/journal/` entry, then archive the change on PR 3's branch (`/opsx:archive`).
- Gate 3: the human merges each pull request.
