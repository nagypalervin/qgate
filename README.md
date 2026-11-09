# qgate

**Detects where your codebase starts to break under fast, AI-assisted development.**

> Status: early. `qgate check` compares TypeScript code between two git
> revisions (or two files) for cyclomatic complexity; more analyzers and
> CI integration follow the [roadmap](#roadmap) below.

## Try it

In a git repository with TypeScript code, check what your branch changed
compared with `main`:

```sh
qgate check --base main
```

qgate compares the merge base of `main` and `HEAD` with `HEAD` (what a
pull request shows) and analyzes every changed `.ts`, `.mts`, `.cts` and
`.tsx` file. `--head <rev>` checks another revision instead of `HEAD`.
Paths in the output are relative to the repository root.

Or compare two files directly, here two fixtures from this repository:

```sh
cargo run -q -p qgate-cli -- check \
  crates/qgate-cli/tests/fixtures/old/orders.ts \
  crates/qgate-cli/tests/fixtures/src/orders.ts
```

```
error   crates/qgate-cli/tests/fixtures/src/orders.ts:42  OrderService.process: cyclomatic complexity 3 -> 9 (+6)  [complexity/cyclomatic-increase]
```

qgate reports, per function:

| Rule | Reported when | Warning | Error |
|---|---|---|---|
| `complexity/cyclomatic-increase` | an existing function got more complex | +1 to +4 | +5 or more |
| `complexity/cyclomatic-new` | a new function is already complex (new files count as new) | 11 to 20 | above 20 |
| `input/unparsable` | `--base` mode only: a changed file has a syntax error, syntax the parser does not support yet, or is not UTF-8; it is skipped | always | - |

It exits with `0` when the analysis ran (findings do not fail it yet) and
`2` for invalid input: an unsupported or unparsable file in the two-file
form, an unknown revision, or a directory that is not a git repository.

### In CI

`--base` needs the base branch in the clone. `actions/checkout` fetches a
single commit by default, so fetch the full history:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0
- run: qgate check --base origin/${{ github.base_ref }}
```

qgate has no release binaries yet (planned for M3); until then, build it
from this repository.

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

| Milestone | Scope |
|---|---|
| **M1** | `qgate check`, TypeScript parsing, complexity delta, terminal output |
| **M2** | Architecture rules (`qgate.toml`), duplication detection |
| **M3** | GitHub Action, SARIF output, pull request comments |
| **M4** | Baseline and trends over time |
| **M5** | MCP server, optional AI summary |

## Repository layout

```
crates/
  qgate-core/       shared types: Finding, Metric, Severity
  qgate-lang/       language adapters (tree-sitter)
  qgate-analyzers/  architecture, complexity, duplication, tests
  qgate-report/     terminal, JSON, SARIF, Markdown
  qgate-git/        changed files and blobs, via the `git` command
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
