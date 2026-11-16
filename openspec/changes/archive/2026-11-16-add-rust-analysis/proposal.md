# Proposal: add-rust-analysis

## Why

qgate is written in Rust, yet it cannot check itself: for a `.rs` file it
reports nothing. Checking our own changes is the cheapest honest test of
the tool before the partner evaluation: we know this code, its history
and its weak spots, so we can tell a useful finding from noise. What we
learn (noise, misses, run time, how it feels to work under the gate)
feeds the partner pilot and an article on AI-assisted development.

## What Changes

- **A Rust language adapter** in `qgate-lang` (`tree-sitter-rust`): the
  same `FileModel` as for TypeScript and C# (function units with their
  cyclomatic complexity and fingerprint, and normalized tokens), so the
  complexity and duplication analyzers work on Rust unchanged.
- **Rust files take part** in both modes: `.rs` in the two-file form and
  among the changed files of revision mode, including the working tree,
  the Claude Code hook and the MCP server. Rust is a language family of
  its own: copies are found and hotspots ranked among Rust files only.
- **Units and names:** every `fn` with a body is a unit: free functions,
  methods, associated functions, default methods of traits and nested
  `fn` items. Closures count towards their named ancestor. A unit is
  named by its inline modules, its impl and its own name: `Cart.total`,
  `<Cart as Display>.fmt`, `tests.adds_two`.
- **Decision points** for Rust: `if` (with `if let`), `while` (with
  `while let`), `for`, `let ... else`, each `match` arm whose pattern is
  not the wildcard `_`, match guards, `|` between pattern alternatives,
  `&&` and `||`.
- **Tokens for copies:** identifiers and literals become placeholders;
  path segments after `::` and member names after `.` keep their text;
  `use` declarations, `extern crate`, `mod name;`, attributes and
  comments (doc comments too) produce none. Types are code, as in C#.
- **Version 0.9.0,** and the README says what qgate measures in Rust.
- **Self-check (dogfooding)**, three ways:
  1. **Replay:** the new qgate runs over every merged pull request of
     this repository (each merge against its first parent); findings,
     unparsable files, noise and run time are recorded in `design.md`
     and the journal, as raw material for the article.
  2. **CI gate:** a job in `.github/workflows/ci.yml` runs the qgate
     GitHub Action, pinned to the released 0.9.0, on qgate's own pull
     requests, and comments.
  3. **While coding:** the Claude Code Stop hook and the MCP server are
     configured for this repository, so later work is checked by qgate
     before it reaches a pull request.

### Out of scope

- **Architecture rules for Rust** (`use crate::...` between modules).
  Cargo already enforces the dependency direction between crates; rules
  inside a crate become a change of their own if the self-check shows a
  need.
- **Test analysis for Rust.** Unit tests live in the same file
  (`#[cfg(test)] mod tests`), so "code changed without its tests" needs
  a design of its own. Rust files are neither sources nor tests for
  `test-analysis`; the replay records what that misses.
- **Generated Rust files** (`// @generated`, build script output).
  `[files] exclude` covers them; qgate has none.
- **Macro bodies:** tree-sitter does not parse the arguments of a macro
  call or the body of `macro_rules!` as code, so decision points inside
  them (`assert!(a && b)`) do not count. Their tokens are still compared.
- **Making the CI job a required check.** Ervin decides that in the
  "Protect main" ruleset after the replay shows the noise level.
- **The article itself.** This change records the material; writing it
  is separate.
- **Tidying the workspace** (the unused `qgate-core` dependency of
  `qgate-lang`, the stale branch). That follows the self-check.

### For Gate 1

Each question has a recommendation; `design.md` has the reasons.

1. **`?` is not a decision point.** It returns early on an error, like a
   call that may throw in TypeScript or C#, which qgate does not count.
   Counting it would make every function that does I/O look complex.
   *Recommended: do not count.*
2. **Match arms:** each arm counts, except a wildcard `_` arm, as with C#
   switch expressions, and so does each `|` in a pattern (`'a' | 'e'`),
   as C#'s pattern `or` does. `loop` does not count (its exits are the
   `if`s and `break`s inside). *Recommended: as listed.*
3. **Names:** inherent methods `Type.method`; trait implementations
   `<Type as Trait>.method`, so `fmt` of `Display` and of `Debug` keep
   apart; types and traits as written, with their type arguments and
   without whitespace; inline modules (`mod tests { ... }`) are part of
   the name, file modules are not. *Recommended: as listed.*
4. **Test code is measured like other code:** `#[cfg(test)]` modules and
   `#[test]` functions get complexity and duplication findings, as
   TypeScript `*.spec.ts` files do. The replay shows whether that is
   noise. *Recommended: yes, and decide again with the replay's numbers.*
5. **The CI gate uses the released Action** (`nagypalervin/qgate@v0.9.0`
   with `fail-on: error`), not `uses: ./`: a release pull request bumps
   `QGATE_VERSION` before the binaries exist, so the local Action would
   fail exactly there. *Recommended: the released Action.*
6. **The hook and MCP configuration are committed** (`.claude/settings.json`
   and `.mcp.json`), with a README note that they need `qgate` on the
   `PATH`. Without it, Claude Code reports a hook error and goes on.
   The hook runs with `--base main`: the default `--base HEAD` sees
   nothing here, because the apply workflow commits within the turn
   (design D10). *Recommended: commit them, with `--base main`.*

## Capabilities

### New Capabilities

- `rust-analysis`: how qgate reads Rust source: which files, which
  units and how they are named, what counts as a decision point, which
  tokens are compared.

### Modified Capabilities

- `check-command`: the two-file form accepts `.rs`; Rust compared with
  another family is rejected.
- `revision-input`: `.rs` files are analyzed.
- `complexity-analysis`: units, names and decision points of Rust come
  from `rust-analysis`.
- `duplication-analysis`: Rust is a family of its own.
- `hotspot-analysis`: Rust files are ranked among Rust files.
- `test-analysis`: Rust files are not checked.

## Impact

- **Code:** `qgate-lang` (new `rust.rs`, `Language::Rust`,
  `Family::Rust`), `qgate-cli` (file selection, the family loop of
  duplication, the two-file form's message), `qgate-analyzers/tests.rs`
  only if a Rust file would otherwise get a role.
- **Dependency:** `tree-sitter-rust` 0.24 (MIT, the tree-sitter
  organization's grammar, built like the two grammars we already use).
- **CI:** a new job in `ci.yml`; the release workflow is unchanged.
- **Repository configuration:** `.claude/settings.json` (Stop hook),
  `.mcp.json`.
- **Docs:** README (Rust section, roadmap "Languages" row, 0.9.0),
  journal entry, release record.
