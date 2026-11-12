# CLAUDE.md

Instructions for AI coding agents (Claude Code and others) working in this
repository. Humans are welcome to read it too. `AGENTS.md`-style tools can
symlink to this file.

## What this project is

qgate is a Rust CLI that compares two git revisions and reports what got
**worse**: architecture violations, complexity increase, duplication, tests
falling behind. See `README.md` for the why and the roadmap.

This project is itself an experiment in AI-assisted development, so how you
work here matters as much as what you produce.

## Layout

- `crates/qgate-core`: shared types only. No I/O, no parsing, no git.
- `crates/qgate-lang`: language adapters, tree-sitter based.
- `crates/qgate-analyzers`: one module per analyzer, implementing `Analyzer`.
- `crates/qgate-report`: renders findings. No analysis logic here.
- `crates/qgate-git`: reads revisions, changed files and blobs by running
  `git` (ADR 0002). Knows nothing about languages or findings.
- `crates/qgate-config`: reads `qgate.toml` into plain data (ADR 0003).
  No I/O; knows nothing about analysis.
- `crates/qgate-cli`: the `qgate` binary. Thin; wires the others together.
- `ci/`: scripts of the CI integrations, in PowerShell, tested with Pester.
  Not a crate. `ci/azure/qgate-pr-comment.ps1` ships in the release
  archives; `ci/github/qgate-action.ps1` runs the GitHub Action.
- `action.yml`: the GitHub Action (`uses: nagypalervin/qgate@v<version>`).
  It only wires its inputs to `ci/github/`; its `QGATE_VERSION` lines must
  match the release (the release workflow checks them).
- `docs/adr/`: architecture decision records. Read them before changing structure.

Dependencies flow one way: `cli -> report, analyzers -> lang -> core`,
`cli -> git` and `cli -> config`. `core`, `git` and `config` depend on
nothing in the workspace. Do not
add reverse or sideways dependencies; if you think you need one, stop and
propose an ADR instead.

## Before you finish any change

Run all of these and make sure they pass:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the same checks with warnings as errors. The pre-commit hook in
`.githooks/` runs them too once enabled (`git config core.hooksPath
.githooks`, see README "Building"); do not bypass it with `--no-verify`.

## Conventions

- Edition 2024, MSRV 1.85. Clippy `pedantic` is on; fix warnings rather than
  silencing them. An `#[allow]` needs a comment saying why.
- No `unsafe`. It is forbidden at the workspace level.
- Every public item has a doc comment (`missing_docs` is on).
- Library crates return errors; only `qgate-cli` prints and picks exit codes.
- Rule ids are `analyzer/rule`, lowercase, e.g. `complexity/increase`. Treat
  them as a public API: never rename one silently.
- Tests live next to the code (`#[cfg(test)] mod tests`). Every analyzer gets
  fixture-based tests with a before/after pair of source files.

## Things not to do

- Do not add a dependency without saying why in the commit message. Prefer
  well-maintained crates with few transitive dependencies.
- Do not make the core analysis depend on an LLM or the network. AI features
  are optional layers on top (see README, design principle 1).
- Do not report absolute scores. qgate reports deltas.
- Do not write large, sweeping changes. Keep pull requests small and focused;
  qgate would flag a huge one, and so will the reviewer.

## Workflow: OpenSpec

Every non-trivial change goes through OpenSpec (`openspec/`). `openspec init`
installed these slash commands for Claude Code (`.claude/commands/opsx/`):
`/opsx:explore` (think, never write code), `/opsx:propose`, `/opsx:update`
(revise planning artifacts), `/opsx:apply`, `/opsx:sync` (merge spec deltas
into `openspec/specs/` without archiving) and `/opsx:archive`.

OpenSpec telemetry is off for this repository: `.claude/settings.json` sets
`OPENSPEC_TELEMETRY=0`. Keep it that way; when running `openspec` outside
Claude Code, set the variable yourself.

1. **Propose** (`/opsx:propose`). Create a change under `openspec/changes/<name>/`: proposal
   (why, in scope, out of scope), spec deltas with testable scenarios, tasks.
   Ask about anything ambiguous; record open questions in the proposal.
   **Gate 1:** the human approves the proposal before any code is written.
2. **Design** (only if needed). For non-obvious technical choices, write
   `design.md` in the change folder. **Gate 2:** the human approves it.
3. **Apply** (`/opsx:apply`). Work through `tasks.md` in small steps, one pull request at a
   time: fixtures and failing tests first, then code, then the checks above.
   In the same commit as the code, tick the task and record any decision you
   made along the way (in `design.md`, or the proposal if there is none).
   If the spec turns out to be ambiguous, stop and ask; do not guess.
4. **Review.** Run the `reviewer` subagent (`.claude/agents/reviewer.md`) on
   the change. It reviews with a clean context and never edits code; fix its
   findings yourself.
5. **Archive** (`/opsx:archive`). Archive the change *on the branch of its last pull request*,
   so code and living spec land on `main` in the same merge.
   **Gate 3:** the human merges.

After a change, add a short entry to `docs/journal/` (see its README).

## Working style

- If a requirement is ambiguous, ask instead of guessing.
- Commit messages: imperative mood, short subject line, body explains why.
- Branch names: `change/<openspec-change-name>`.
