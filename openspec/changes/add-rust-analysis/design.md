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
`.month`; the first segment of a path is a name like any other. Types
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

## Self-check results

Filled in during apply (D8).
