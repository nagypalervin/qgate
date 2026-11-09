# qgate

**Detects where your codebase starts to break under fast, AI-assisted development.**

> Status: early. `qgate check` compares TypeScript code between two git
> revisions (or two files) for cyclomatic complexity, newly duplicated
> code and, with a [`qgate.toml`](#architecture-rules-qgatetoml),
> forbidden imports; more analyzers and CI integration follow the
> [roadmap](#roadmap) below.

## Try it

Install it from a clone of this repository (release binaries come with
M3):

```sh
cargo install --locked --path crates/qgate-cli
```

Then, in a git repository with TypeScript code, check what your branch
changed compared with `main`:

```sh
qgate check --base main
```

qgate compares the merge base of `main` and `HEAD` with `HEAD` (what a
pull request shows) and analyzes every changed `.ts`, `.mts`, `.cts` and
`.tsx` file. `--head <rev>` checks another revision instead of `HEAD`.
Paths in the output are relative to the repository root.

Or compare two files directly, here two fixtures from this repository
(without installing):

```sh
cargo run -q -p qgate-cli -- check \
  crates/qgate-cli/tests/fixtures/old/orders.ts \
  crates/qgate-cli/tests/fixtures/src/orders.ts
```

```
error   crates/qgate-cli/tests/fixtures/src/orders.ts:42  OrderService.process: cyclomatic complexity 3 -> 9 (+6)  [complexity/cyclomatic-increase]
```

qgate reports:

| Rule | Reported when | Warning | Error |
|---|---|---|---|
| `complexity/cyclomatic-increase` | an existing function got more complex | +1 to +4 | +5 or more |
| `complexity/cyclomatic-new` | a new function is already complex (new files count as new) | 11 to 20 | above 20 |
| `architecture/forbidden-import` | `--base` mode only: a change adds an import that a rule in `qgate.toml` forbids (see below) | - | always |
| `duplication/duplicate-block` | `--base` mode only: a change adds a copy of a block of at least 50 tokens that exists elsewhere in the repository (see below) | always | - |
| `input/unparsable` | `--base` mode only: a changed file has a syntax error, syntax the parser does not support yet, or is not UTF-8; it is skipped | always | - |

Exit codes:

- `0`: the analysis ran and no finding reached the `--fail-on` level;
- `1`: at least one finding did. The report is printed as usual, and one
  line on standard error says why the run failed, for example
  `qgate: failing: 1 finding at or above "error" (see --fail-on)`;
- `2`: the check could not run, for example: an invalid invocation, an
  unsupported or unparsable file in the two-file form, an unknown revision,
  revisions without a merge base, a directory outside a git repository, or
  no `git` on the `PATH`. This wins over `1`.

`--fail-on` takes `error` (the default: only Errors fail the run),
`warning` (Warnings fail it too, including `input/unparsable`) or `never`
(findings never fail it; only a check that could not run does). The
example above has an Error finding, so it exits with `1`; add
`--fail-on never` to only see the report.

### Duplicated code

In `--base` mode qgate compares the code a change adds with every
TypeScript file of the checked revision and reports a new copy:

```
warning src/orders/format.ts:4  duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)  [duplication/duplicate-block]
```

- **Renamed copies count:**
  - variable names and literal values are ignored, so a helper copied
    under other names is still a copy;
  - member names (`.getFullYear`), keywords and operators are compared;
  - comments, semicolons, imports and type annotations are ignored.
- **Copies across files only.** A block counts when it occurs in two or
  more files; repetition within one file (a data table, similar test
  cases one after another) does not.
- **Only new copies are reported.** Duplication the code base already has
  is not, nor code written next to it; nor is code that only moved, also
  out of a deleted file; nor a file renamed or moved from `.js` to `.ts`.
  When a change adds the same block to two new files, the first by path
  counts as the original.
- **A block must span at least 50 tokens**, about 5 to 8 lines.
- It is a Warning, so it fails a run only with `--fail-on warning`. The
  two-file form does not check duplication.

### Architecture rules (`qgate.toml`)

Put a `qgate.toml` at the repository root to forbid imports between parts
of your code:

```toml
[[architecture.forbid]]
name = "domain-is-pure"
from = ["src/domain/**"]
to   = ["src/ui/**", "src/infra/**", "react"]
```

A change that adds `import { Button } from "../ui/Button";` to
`src/domain/order.ts` then fails with:

```
error   src/domain/order.ts:3  imports src/ui/Button ("../ui/Button"), forbidden by "domain-is-pure"  [architecture/forbidden-import]
```

- **Only new imports are reported.** An import the file already had in the
  base revision does not fail the check, so you can add a rule to a
  codebase that still breaks it. A move that git detects as a rename is
  checked against the file's old path: moving a file within `from`
  reports nothing, moving it into `from` reports its forbidden imports. A
  file renamed from `.js` to `.ts` keeps the imports it already had (if the
  old JavaScript cannot be read, none of its imports counts as new,
  wherever the file moved).
- **Patterns** are globs relative to the repository root: `*` and `?` stay
  within one directory, `**` spans directories. A pattern without glob
  characters also matches everything below it (`react` matches
  `react/jsx-runtime`, not `react-dom`), and `dir/**` also matches `dir`.
  A leading `./`, `//` inside and a trailing `/` are ignored
  (`./src//ui/` is `src/ui`); a pattern that starts with `/` (after any
  `./`) or has a `..` segment is an error. A trailing `/` does not make a
  glob match below it: write `**/ui/**`, not `**/ui/`. Braces (`{a,b}`)
  and `\` escapes are glob syntax; a pattern using them is not rewritten
  beyond a leading `./`.
- **`from`** matches the importing file's path. **`to`** matches the import
  target: a relative import (`./`, `../`) resolved to a path from the root,
  without extension (`src/ui/Button`); anything else as written
  (`react`, `@/ui/Button`). So prefer directory patterns, and list path
  aliases too: `to = ["src/ui/**", "@/ui/**"]`. Suffixes and prefixes
  stay part of the target: `"../ui/B?raw"` is `src/ui/B?raw` (matched by
  `src/ui/**`, not by `src/ui/B`), and `node:fs` is not `fs`.
- **Every import counts**, `import type` and `import("...")` in a type
  included; dynamic imports and `require` count with a string literal.
- **The file is read from the revision being checked** (`--head`, by default
  `HEAD`), not from the working tree. A missing `qgate.toml` means no
  rules; an invalid one (unknown keys included) fails with exit code `2`.
  The two-file form ignores it.
- **Not yet supported:** tsconfig `paths`, root-absolute imports
  (`import "/src/ui/Button"`, which no pattern can name), and files the
  TypeScript grammar
  cannot parse yet (`export type * from`, `import("./x").Foo<T>`), which
  are reported as `input/unparsable` instead.

### In CI

For a workflow triggered `on: pull_request` (`github.base_ref` is empty
for other events). `--base` needs the base branch in the clone, and
`actions/checkout` fetches a single commit by default, so fetch the full
history:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0
- run: qgate check --base "origin/$BASE_REF"
  env:
    BASE_REF: ${{ github.base_ref }}
```

The job fails when the pull request adds an Error finding. Use
`--fail-on warning` for a stricter gate, or `--fail-on never` to report
without failing the job.

qgate has no release binaries yet (planned for M3); until then, install
it with `cargo install` as above.

## Why

AI coding assistants make it cheap to write a lot of code quickly. Speed
moves the bottleneck: architecture boundaries erode, functions quietly grow
more complex, near-duplicate blocks multiply, and tests fall behind the code
they are supposed to cover. Each pull request looks fine on its own; the
damage shows up months later.

qgate makes that drift visible **per change**, while it is still cheap
to fix, and records it over time so you can answer a simple question with
data instead of gut feeling:

*When we develop faster, where does the system break first?*

## How it works

qgate compares two revisions (`base..head`) and reports what got
**worse**. It measures deltas, not absolute values, so you can adopt it on
an existing codebase without first fixing every legacy problem.

```
git diff base..head ─► parser (tree-sitter) ─► analyzers ─► findings ─► reporters
                                              ├ architecture    ├ terminal
                                              ├ complexity      ├ JSON / SARIF
                                              ├ duplication     ├ PR comment
                                              └ tests           └ MCP server
```

Design principles:

1. **Deterministic core.** The same input always gives the same result. An
   LLM may summarise findings for a PR comment, but it never decides what a
   finding is, and it is always optional.
2. **Deltas, not scores.** "This function went from 8 to 23" is actionable;
   "your codebase scores 61/100" is not.
3. **Close the loop with agents.** An MCP server lets AI coding agents ask
   qgate what they just broke *before* they open a pull request.

## Roadmap

| Milestone | Scope | Status |
|---|---|---|
| **M1** | `qgate check`, TypeScript parsing, complexity delta, git revisions, `--fail-on` | done |
| **M2** | Architecture rules (`qgate.toml`), duplication detection | in progress |
| **M3** | Release binaries, GitHub Action, SARIF output, pull request comments | planned |
| **M4** | MCP server: AI coding agents check their own changes before opening a pull request | planned |
| **M5** | Change risk from git history: hotspots (code that is both complex and changed often), code changed without its tests | planned |
| **M6** | Baseline and trends over time, optional AI summary | planned |

The hotspot analysis in M5 builds on published work on behavioral code
analysis, notably Adam Tornhill's *Your Code as a Crime Scene*.

## Repository layout

```
crates/
  qgate-core/       shared types: Finding, Metric, Severity
  qgate-lang/       language adapters (tree-sitter)
  qgate-analyzers/  architecture, complexity, duplication, tests
  qgate-report/     terminal, JSON, SARIF, Markdown
  qgate-git/        changed files and blobs, via the `git` command
  qgate-config/     reads qgate.toml
  qgate-cli/        the `qgate` binary
docs/adr/               architecture decision records
docs/journal/           notes on how AI-assisted development went
openspec/               specs and changes (OpenSpec workflow, see CLAUDE.md)
.claude/agents/         independent reviewer agent
```

## Building

Requires Rust 1.85 or newer and a C compiler (tree-sitter compiles its
grammars from C at build time). `qgate check --base` needs `git` on the
`PATH`; the tests need git 2.32 or newer, because they create temporary
repositories isolated from your git configuration.

```sh
cargo build
cargo test --workspace
cargo run -p qgate-cli -- --version
```

To run the CI checks (format, clippy, tests) before every commit, enable
the repository's hook once per clone:

```sh
git config core.hooksPath .githooks
```

`git commit --no-verify` skips it for a work-in-progress commit.

## About

A personal open source project by [Ervin Nagypál](https://nagypalervin.hu/),
built in the open as an experiment in AI-assisted development. Lessons
learned along the way are written up on the blog.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
