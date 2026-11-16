# Design: add-rust-analysis

## Context

See proposal.md for why. The language layer is built for this: an
adapter turns source into a `FileModel` (units with complexity and
fingerprint, imports, usings, normalized tokens), and analyzers never see
syntax trees. TypeScript and C# each have an adapter on the shared
helpers of `common.rs` (tree building, refusing syntax errors, unique
names). Language-specific code outside `qgate-lang` is small:
`qgate-cli` picks the language by extension, loops over the families
for duplication, ranks hotspots per family and names the extensions in
the two-file form's error; `tests.rs` recognizes TypeScript and C# by
extension, so any other file already has no role there.

A spike on 2026-11-16 parsed all 39 `.rs` files of this repository with
`tree-sitter-rust` 0.24.2 and `tree-sitter` 0.26: none had an error.
The grammar knows edition 2024 (let chains), `or_pattern`, let-else
(`let_declaration` with an `alternative`), `try_expression` for `?`,
and a separate `function_signature_item` for a `fn` without a body.

## Goals / Non-Goals

**Goals:**

- Rust analysis at the level of the C# adapter of 0.4.0: complexity and
  duplication, nothing more.
- qgate checking its own pull requests, in CI and while coding.
- Numbers from the replay that can carry an article: what qgate found in
  51 merged pull requests, what of it was useful, what was noise.

**Non-Goals:**

- Matching clippy's lints or rustc's notion of anything. qgate measures
  deltas of its own model.
- Macro expansion. qgate reads source as written, as it does not
  evaluate C#'s `#if`.

## Decisions

### D1. One adapter, one family

`qgate-lang` gets `rust.rs` beside `csharp.rs`, `Language::Rust` (`.rs`)
and `Family::Rust`. The CLI's list of families for duplication becomes
`Family::ALL`, so a fourth language cannot be forgotten there.
*Alternative:* treat Rust like TypeScript's JavaScript, a member of an
existing family. No other language's code is comparable with Rust's.

### D2. Names: `Type.method` and `<Type as Trait>.method`

A Rust file is a module, so file modules add nothing a path does not
already say; inline modules are part of the name because two `fn new`
in `mod a` and `mod b` are different units. An inherent `impl` is named
by its type, a trait `impl` by `<Type as Trait>`, the syntax Rust itself
uses for that method (`<Cart as Display>::fmt`). Without the trait,
`fmt` of `Display` and of `Debug` would be `Cart.fmt` and `Cart.fmt#2`,
named by source order, and moving one would compare the wrong bodies:
the problem C# solved with parameter types. Rust has no overloading, so
parameters are not needed. Types and traits keep their type arguments
(`From<u8>` and `From<u16>` are two impls), written without whitespace,
like C#'s generic types. `.` stays the separator, as in the other
languages and the reports, not `::`.
*Alternatives:* `Cart::fmt` with `#2` suffixes (order-dependent, see
above); the trait without the type (`Display.fmt`), which collides for
two types in one file.

### D3. Decision points: what branches, not what can fail

Counted: `if` (also `if let`), `while` (also `while let`), `for`,
let-else, every non-`_` match arm, match guards, `|` in patterns, `&&`
and `||`, including those inside let chains. The list follows the C#
adapter: match arms are switch expression arms, the pattern `|` is C#'s
`or`.

`?` is not counted. It is an early return on an error, like a call that
may throw in TypeScript or C#, which qgate does not count either; the
alternative (`match` on the `Result`) would count, but that is the
point of `?`: it removes the branch from the code the reader follows.
Counting it would put most I/O functions of this repository above 10.
`loop`, `break`, `continue` and `return` are not counted: the conditions
that lead to them are.

A `const` or `static` inside a function is the function's code: its
decision points count and its text is part of the fingerprint, as for a
C# local constant. Only those outside functions are not measured (Ervin,
2026-11-16, after the review of PR 1 found the first version left local
ones out of both).

### D4. Macros are token trees

tree-sitter does not parse a macro call's arguments or a `macro_rules!`
body; they are `token_tree`s of raw tokens. qgate does not expand
macros, so decision points in them are invisible (`assert!(a && b)`
stays 1). Their tokens are normalized by the same rules as other code,
so a copied `format!` or `vec!` is still found. Closures written inside
a macro call are invisible too. The replay counts how often this
matters in this repository (Risks).

### D5. Tokens

As in C#: identifiers and lifetimes become `$id`, literals `$lit`,
keywords, primitive types and punctuation keep their text. A name after
`.` (`field_identifier`) and a path segment after `::` keep their text:
`Vec::new()` and `Vec::default()` must differ, as must `.year` and
`.month`; the first segment of a path is a name like any other. A tuple
field such as `.0` is a field name and keeps its text too, so
`self.0.len()` and `self.1.len()` differ (Ervin, 2026-11-16, after the
review of PR 1: newtypes and tuple structs are common in Rust). A loop
label is a name like a lifetime: one `$id`. Types
are code, as in C#: `Result<u32, Error>` is part of what a copy copies.
No tokens for `use`, `extern crate`, `mod x;`, the head of an inline
`mod x {`, attributes, comments and semicolons: they repeat across files
without being copied logic (every test module starts with
`#[cfg(test)] mod tests { use super::*;`).

### D6. Test code is code

Code in `#[cfg(test)]` modules and `#[test]` functions is measured
(Gate 1, question 4). TypeScript spec files are measured too, and a
`cfg` decides compilation, not whether code is read by people. If the
replay shows test code produces most of the noise, a follow-up can skip
units under `#[cfg(test)]`, as a decision with numbers behind it.

### D7. Tests and hotspots need no Rust code

`tests.rs` already gives no role to a file it does not recognize by
extension, so a `.rs` file is neither source nor test; a test pins this
down. Hotspots rank per family through `Language::family`, so Rust
files rank among themselves. Integration tests under `tests/` are
ranked, because `test-analysis` has no Rust conventions; the replay
shows whether that pushes sources out of the top.

### D8. The replay

For each merge on `main`'s first-parent history (51 on 2026-11-16: the
pull requests of the history before the move to the public repository,
whose numbers restarted there, and #1 to #33 since), run the qgate of
this change, built from the change branch, as:

```sh
qgate check --base "$M^1" --head "$M^2" --format json --fail-on never
```

qgate compares from the merge base, so each run sees exactly that pull
request's changes. Recorded per pull request: findings by rule and
severity, `input/unparsable` files, hotspot context, wall-clock time.
Then each Error and a sample of Warnings is classified by hand:
*useful* (a reviewer would want to see it), *debatable*, *noise*. The
loop runs from a script in the scratchpad, not in the repository: it is
a one-off evaluation, and this section keeps the command. The numbers go
into "Self-check results" below and the journal entry. The replay runs
before the release, so what it finds can still change D3 to D6.

### D9. The CI gate uses the released Action

A `qgate` job in `ci.yml`, on `pull_request` only, with
`contents: read` and `pull-requests: write`, `fetch-depth: 0`, and
`uses: nagypalervin/qgate@v0.9.0` with `fail-on: error`. Not
`uses: ./`: the local `action.yml` downloads the binaries of its own
`QGATE_VERSION`, which a release pull request bumps before the tag
exists, so the job would fail exactly on release pull requests. The pin
is bumped by hand after each release (a line in the release record's
checklist). The job is not a required check; Ervin decides that after
the replay.

### D10. The hook checks against `main`

`.claude/settings.json` gets the Stop hook with
`qgate hook claude-code --base main`, and `.mcp.json` the MCP server.
The default `--base HEAD` would see nothing here: the apply workflow
edits, runs the checks and commits within one turn. With `--base main`
every turn on a change branch checks the whole branch, and an Error the
branch accepts on purpose sends the agent back once per turn; the hook's
one-round limit bounds that, and the experience is article material.
On `main` itself the two bases are the same. Both files need `qgate` on
the `PATH`; Ervin's machine gets the released 0.9.0 binary in
`~/.local/bin`, so the hook checks with what users get, not with the
working copy.

## Risks / Trade-offs

- [Grammar lags behind rustc] → A file with newer syntax is
  `input/unparsable`, a Warning; the replay counts them. The spike found
  none in this repository.
- [Logic inside macros is invisible] → Accepted (D4); the replay reports
  functions whose body is mostly a macro call.
- [`?` hides real branching] → A function that grows ten `?` does not
  get more complex for qgate. Accepted (D3): the reader does not follow
  those branches either.
- [Test code noise in duplication] → Test functions repeat setup by
  design. D6 measures it first; the follow-up is cheap.
- [Integration tests among hotspots] → They churn with every feature;
  the replay shows the effect. A Rust test convention belongs to a
  future test-analysis change.
- [The CI pin goes stale] → The job keeps working on an older qgate; the
  release checklist line catches it.
- [Hook noise on accepted Errors] → One extra round per turn at most;
  if it is too much, the hook drops back to `--base HEAD` and the CI gate
  carries the branch.
- [Fork pull requests] → The Action skips the comment for forks with a
  notice; the gate still fails on Errors.

## Migration Plan

Additive: `.rs` files were ignored before, so a repository with Rust
code gets new findings after upgrading, only for changed Rust files. No
configuration changes. Rollback: `[files] exclude = ["**/*.rs"]`.

## Measurements

- **Binary size** (task 1.5, `cargo build --release -p qgate-cli` on
  aarch64-unknown-linux-gnu, 2026-11-16): 11,605,688 bytes before the
  grammar, 12,719,800 after: +1,088 KiB (+9.6%).
- **Grammar detail found in task 1.4:** `macro_rules!` is one token,
  not `macro_rules` and `!`; it changes nothing for copies, since every
  definition starts the same way.
- **Grammar lag found in the review of PR 1:** `tree-sitter-rust` 0.24
  does not know the edition-2024 `safe` qualifier in an `unsafe extern`
  block (`unsafe extern "C" { safe fn sqrt(x: f64) -> f64; }`); such a
  file is `input/unparsable`. The README names it (task 2.5).
- **Accepted in the review of PR 1:** the patterns `(_)` and `_ | 1`
  count as an arm (and a `|`) though they match like `_`; rustc warns
  about both, so idiomatic code does not have them.

## Release 0.9.0

- **Tagged** `v0.9.0` on `172c131` (the merge of PR 2) by Claude at
  Ervin's go, annotated like `v0.8.0`, 2026-11-16 14:16 UTC; the
  release workflow passed (9 jobs).
- **Draft checked:** six archives and `SHA256SUMS`, `sha256sum --check`
  OK for all six; the Linux arm64 binary is static, prints
  `qgate 0.9.0` and reports `total: cyclomatic complexity 2 -> 3 (+1)`
  for the `rs` pair of the CLI tests. Archive sizes: 2.0 MB (Windows
  arm64) to 2.3 MB (Linux x64).
- **Notes** written into the draft by Claude; **published** by Ervin at
  14:30 UTC.
- **The CI pin** (D9): there is no release checklist file, so the
  reminder to bump `nagypalervin/qgate@v0.9.0` after a release is a
  comment at the pin in `ci.yml`.
- **The exclusion works:** with `qgate.toml` in the working tree, two
  copies of a file under `src/` are reported, two under `fixtures/`
  are not.

## Self-check results

Replay of task 2.4 (D8), 2026-11-16, with the build of
`change/add-rust-analysis-2` at `aeb53f6` (version still 0.8.0), with
the default configuration (this repository has no `qgate.toml`).

**Scope.** 52 merges, not 51: PR #34 (PR 1 of this change) merged
meanwhile. 36 of them change `.rs` files.

**Run time.** 3.85 s for all 52, at most 0.23 s per run (the merge of
add-hotspot-context, 14 Rust files, history ranked). No run failed, no
Rust file was unparsable.

**Findings.** 218 in all, in 32 merges. All 60 TypeScript and C#
findings are in test fixtures.

| Where | Findings | Errors | Of which duplication |
|---|---|---|---|
| Test fixtures (`*/fixtures/*`) | 62 | 3 | 38 |
| Integration tests (`*/tests/*`) | 56 | 2 | 53 |
| `#[cfg(test)]` modules in `src/` | 17 | 0 | 14 |
| Source code | 83 | 6 | 20 |

A gate with `fail-on: error` would have failed 7 of the 52 pull
requests: one for fixtures alone, one for test support code, five for
source code. 35 findings carried hotspot context, 34 of them in source
code.

**Hand check.** All 6 source Errors, all 20 source duplicates and all 57
source complexity Warnings, classified as *useful* (a reviewer would want
to see it), *debatable* or *noise*:

- **Errors (6):**
  - **Useful (5):** `main` 6 -> 11; `check_revisions` 12 -> 20 and
    `duplication` 15 -> 21, both in the CLI's busiest file (hotspot, 38
    commits); `parse_log` new at 21, a state machine. `Walker<'s>.enter`
    8 -> 16 in the TypeScript adapter is useful too: the dispatcher
    gained a nested special case.
  - **Debatable (1):** `git_message` 4 -> 10: two new match arms, each
    with a guard holding `&&`. Flat and easy to read, yet +6.
- **Duplicates (20):**
  - **Useful (19):** `github.rs` is a 611-token copy of `azure.rs`. The C#
    adapter (add-csharp-analysis) copied the TypeScript adapter's walker
    skeleton in 9 places (walk loop, unit opening, frames, tokens);
    PR 1 of this change did the same with 9 more copies from the C# and
    TypeScript adapters. Neither the reviewer subagent nor Ervin raised
    it; qgate would have. A shared walker in `common.rs` is the obvious
    follow-up.
  - **Debatable (1):** a block that runs from the end of a function into
    the test helpers.
- **Complexity Warnings (57):**
  - **Noise (12): a new enum variant.** Every exhaustive `match` over
    the enum gets a new arm, which rustc demands:
    `Language.from_extension`, `Language.family` and `parse` for a new
    language, `Format.name` and `Format.render` for a new format,
    `<GitError as fmt::Display>.fmt` for a new error. In one of them
    (`typescript.rs` `parse`) the new variant joined an existing arm,
    `Language::CSharp | Language::Rust`; the `|` counts as much as a
    separate arm would, so the rule is consistent.
  - **New functions of 11 to 20 (20):** *useful (11)* where the
    function does several things a reviewer could split
    (`parse_raw_diff`, `BlobReader.exchange`, `DuplicationIndex.blocks`,
    `duplication`, `mcp_server`, `Server<C>.handle`, `parse_arguments`,
    `cell`, `Conventions<'a>.new`, `revision_check`, `target`);
    *debatable (9)* where it is a dispatcher over syntax kinds or
    messages, complex by nature (`Walker<'s>.enter` and
    `Walker<'s>.record_token` of each adapter, `produces_no_tokens`,
    `import_source`, `Server<C>.dispatch`, `decision_points`).
  - **Useful (25): growth of existing functions,** most of them in the
    CLI's `main.rs`, which grew from complexity 49 to 179 over the
    history, every step of it a hotspot.

**Test code.** 73 findings (33%) are in test code, 67 of them
duplication. The integration-test copies are mostly the `Repo` helper,
which exists both in `qgate-git`'s `test_support.rs` and in the CLI's
`tests/support`, and repeated test set-ups. Only 17 findings are in
`#[cfg(test)]` modules, so skipping those (D6) would change little;
`[files] exclude` for fixtures and integration tests would.

**Macros (D4).** In the Rust code of this repository at the time of the
replay (fixtures left out), 13 decision tokens (`if`, `match`, `&&`,
...) sit inside macro arguments, against 983 decision points qgate
counts: 1.3% are invisible. 107 of 1,217 functions consist of macro
calls only, nearly all of them tests made of `assert!`s.

**Integration tests among hotspots (D7).**
`crates/qgate-cli/tests/revisions.rs` is among the most changed Rust
files (churn top 10%) in 36 of the 52 merges, yet it pushed no source
file out of the top in any of them: without the tests, the 10% shrinks
too. None of the 56 integration-test findings had hotspot context,
because no test file was also in the complexity top 25%.

**Per pull request.** The first 19 rows are the history before the move
to the public repository, whose numbers restarted there. "Of them
outside fixtures" is what a gate with the fixtures excluded would show.

| Merge | Pull request | `.rs` files | Findings | Errors | Of them outside fixtures | Seconds |
|---|---|---|---|---|---|---|
| `bf402a5` | #2 `add-cyclomatic-complexity-check` | 3 | 2 | 0 | 1 | 0.04 |
| `388d54b` | #3 `add-cyclomatic-complexity-check-2` | 2 | 19 | 3 | 0 | 0.04 |
| `b20e54a` | #4 `add-cyclomatic-complexity-check-3` | 2 | 8 | 0 | 0 | 0.04 |
| `0b103b0` | #5 `add-git-revision-input` | 2 | 1 | 0 | 1 | 0.03 |
| `d26fca1` | #6 `add-git-revision-input-2` | 4 | 6 | 1 | 6 | 0.04 |
| `7b2ff9f` | #7 `add-fail-on` | 3 | 6 | 0 | 3 | 0.06 |
| `aa174d8` | #8 `analyze-wrapped-functions` | 4 | 6 | 1 | 2 | 0.06 |
| `0a8c895` | #9 `pre-commit-hook` | 0 | 0 | 0 | 0 | 0.01 |
| `0bfd2a3` | #10 `add-forbidden-imports` | 3 | 2 | 0 | 1 | 0.06 |
| `699380c` | #11 `add-forbidden-imports-2` | 5 | 2 | 0 | 2 | 0.06 |
| `3fa180a` | #12 `add-forbidden-imports-3` | 5 | 5 | 0 | 5 | 0.08 |
| `388966f` | #13 `msrv-ci-2` | 0 | 0 | 0 | 0 | 0.01 |
| `840ac6d` | #14 `add-duplicate-detection` | 4 | 4 | 0 | 3 | 0.07 |
| `f71b062` | #15 `add-duplicate-detection-2` | 1 | 1 | 0 | 1 | 0.07 |
| `88dee45` | #16 `add-duplicate-detection-3` | 6 | 8 | 0 | 4 | 0.10 |
| `967fc26` | #17 `roadmap` | 0 | 0 | 0 | 0 | 0.01 |
| `a39c4c7` | #18 `m2-done` | 0 | 0 | 0 | 0 | 0.01 |
| `9792c83` | #19 `roadmap-agent-first` | 2 | 0 | 0 | 0 | 0.06 |
| `c2cf98e` | #1 `license-holder` | 0 | 0 | 0 | 0 | 0.01 |
| `bb27486` | #2 `add-worktree-input` | 2 | 1 | 0 | 1 | 0.08 |
| `4e8cdc5` | #3 `add-worktree-input-2` | 3 | 4 | 1 | 4 | 0.09 |
| `d7e543c` | #4 `add-report-formats` | 6 | 1 | 0 | 1 | 0.09 |
| `fc1016a` | #5 `add-report-formats-2` | 8 | 10 | 2 | 10 | 0.11 |
| `85334c1` | #6 `add-agent-hooks` | 3 | 3 | 0 | 3 | 0.09 |
| `4f53dea` | #7 `fix-racy-worktree-snapshot` | 2 | 4 | 0 | 4 | 0.10 |
| `e657ad0` | #8 `add-mcp-server` | 3 | 11 | 0 | 11 | 0.11 |
| `96b9ca0` | #9 `close-m3` | 0 | 0 | 0 | 0 | 0.01 |
| `68849fd` | #10 `add-release-binaries` | 2 | 0 | 0 | 0 | 0.13 |
| `fdca116` | #11 `add-release-binaries-2` | 0 | 0 | 0 | 0 | 0.01 |
| `2dd4e7a` | #12 `add-release-binaries-3` | 0 | 0 | 0 | 0 | 0.01 |
| `8fa081f` | #13 `add-azure-pipelines` | 0 | 0 | 0 | 0 | 0.01 |
| `c24029f` | #14 `add-pr-comments` | 7 | 7 | 0 | 7 | 0.12 |
| `5400961` | #15 `add-pr-comments-2` | 0 | 0 | 0 | 0 | 0.01 |
| `e29df6e` | #16 `add-github-action` | 5 | 3 | 0 | 3 | 0.13 |
| `85feeec` | #17 `add-github-action-2` | 0 | 0 | 0 | 0 | 0.01 |
| `f03ae2c` | #18 `fix-pr-comment-details` | 2 | 2 | 0 | 2 | 0.11 |
| `ada1eb8` | #19 `fix-pr-comment-details-2` | 1 | 0 | 0 | 0 | 0.11 |
| `af528f9` | #20 `close-m4` | 0 | 0 | 0 | 0 | 0.01 |
| `ea2fd6e` | #21 `add-csharp-analysis` | 9 | 17 | 0 | 16 | 0.16 |
| `cb474ba` | #22 `add-csharp-analysis-2` | 6 | 17 | 2 | 15 | 0.15 |
| `5731da7` | #23 `add-csharp-analysis-3` | 0 | 0 | 0 | 0 | 0.01 |
| `d408ea1` | #24 `add-path-exclusion` | 6 | 8 | 0 | 8 | 0.13 |
| `98961ec` | #25 `add-path-exclusion-2` | 0 | 0 | 0 | 0 | 0.01 |
| `fdcbadd` | #26 `add-csharp-architecture` | 10 | 5 | 0 | 5 | 0.17 |
| `a858dea` | #27 `add-csharp-architecture-2` | 0 | 0 | 0 | 0 | 0.01 |
| `1133b17` | #28 `add-test-gap-analysis` | 4 | 10 | 0 | 5 | 0.16 |
| `19e4f32` | #29 `add-test-gap-analysis-2` | 6 | 14 | 0 | 3 | 0.19 |
| `a1f22a9` | #30 `add-test-gap-analysis-3` | 0 | 0 | 0 | 0 | 0.01 |
| `5c5d56a` | #31 `add-hotspot-context` | 14 | 1 | 1 | 1 | 0.23 |
| `1ab25be` | #32 `add-hotspot-context-2` | 9 | 5 | 0 | 5 | 0.17 |
| `e8c8128` | #33 `add-hotspot-context-3` | 1 | 0 | 0 | 0 | 0.14 |
| `6575cc5` | #34 `add-rust-analysis` | 10 | 25 | 0 | 23 | 0.17 |

## Decisions after the replay

Put to Ervin before the release (task 2.4); both on 2026-11-16:

- **D3 stays.** A new enum variant adds one to every exhaustive `match`
  over it: a real new path, counted as TypeScript's `case` and C#'s
  `case` are. The README says so. If it disturbs the CI gate, a
  language-neutral change can treat it later (a threshold for `+1`, say).
- **D6 stays,** and this repository's `qgate.toml` excludes the
  fixtures, `[files] exclude = ["**/fixtures/**"]` (task 4.2). Skipping
  `#[cfg(test)]` modules would remove 17 findings; the 56 of the
  integration tests stay visible, because some of them are real (the
  `Repo` helper lives in two crates).
- **Follow-ups for Ervin to order** (journal entry, task 4.4): a shared
  walker for the three adapters (19 duplicates); the `Repo` test helper
  in one place; the unused `qgate-core` dependency of `qgate-lang`.
