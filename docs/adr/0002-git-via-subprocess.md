# ADR 0002: Read git through the `git` command, in a `qgate-git` crate

- Status: accepted
- Date: 2026-11-09

## Context

qgate compares two git revisions. It needs the merge base of two
revisions, the files changed between two commits (with renames), and the
contents of old and new file versions. Something has to talk to git, and
nothing in the workspace does yet: `qgate-core` must stay free of I/O and
git, `qgate-lang` only parses text, and `qgate-cli` is meant to stay thin.

CLAUDE.md asks for well-maintained dependencies with few transitive
dependencies. The workspace had about 25 external crates when this was
decided.

## Decision

- Run the **`git` command as a subprocess**; do not link a git library.
- Put all git access in a new **`qgate-git`** crate. It depends on nothing
  in the workspace, returns plain data (paths, blob ids, bytes) and errors,
  and never prints. Dependency direction: `cli -> git`.
- Pass user-supplied revisions only to `git rev-parse --verify
  --end-of-options`; every later command receives full commit ids, so a
  revision can never act as an option.

## Reasons

- `gix`, the maintained pure-Rust option, adds 131 crates with only the
  `revision` feature, 155 with `revision` and `blob-diff`, and 202 with its
  defaults (measured with `gix` 0.87, the newest that supports MSRV 1.85).
  Calling `git` adds none.
- git's own merge base and rename detection are exactly what users compare
  qgate's output against; a library would have to match them.
- `git` is present wherever a repository is checked out, including CI
  runners.
- A separate leaf crate keeps the CLI thin and lets git handling be tested
  on its own against real temporary repositories.

## Consequences

- Revision mode and its tests need `git` on the `PATH` (2.24 or newer for
  `--end-of-options`).
- One process per git call. Fine for pull-request-sized changes.
- Revisit when per-call process cost matters, for example an MCP server
  (M5) answering many small queries: first move blob reads to one long-lived
  `git cat-file --batch`, and only then reconsider a library.
- Error handling relies on exit codes, not on git's localized messages;
  other failures pass git's stderr through.
