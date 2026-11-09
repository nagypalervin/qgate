# ADR 0003: Read `qgate.toml` in a `qgate-config` leaf crate

- Status: accepted
- Date: 2026-11-09

## Context

The first architecture rule (`add-forbidden-imports`) brings a
configuration file, `qgate.toml`. Revision mode reads it from the head
revision. Someone has to turn its bytes into rules and report a broken
file well: syntax errors with their line, unknown keys, wrong types, and
rule-level problems such as duplicate names.

The workspace rules leave no obvious home:

- `qgate-core` holds shared types only and must not parse.
- `qgate-cli` is meant to stay thin.
- Analyzers should not depend on a file format: a format change should not
  touch analysis code.

## Decision

- Put the file format in a new **`qgate-config`** crate. It turns the bytes
  of `qgate.toml` into plain data (`Config`, `ForbidRule` with strings) or
  an error. It depends on nothing in the workspace, does no I/O and never
  prints.
- Dependency direction: **`cli -> config`**, a leaf like `cli -> git`.
- Analyzers define their own rule types. The CLI copies the plain config
  values into them; pattern syntax is validated where patterns are
  interpreted, in the analyzer.
- Parse with `toml` and `serde` derive. `deny_unknown_fields` rejects every
  unknown table and key.

## Reasons

- **A leaf crate** keeps the file format testable on its own, like
  `qgate-git`, and keeps the CLI's code wiring only.
- **Analyzers stay format-free.** Rejected alternatives:
  - `analyzers -> config` would couple every analyzer to the file format;
  - rule types in `qgate-core` would put pattern logic in a types-only
    crate.
- **`toml` 0.9**, with `default-features = false` and the features `parse`,
  `serde` and `std`, is the maintained reference implementation, and its
  errors carry line and column. Measured in a throwaway crate, it adds
  about 11 crates together with `serde` derive, most of them build-time
  proc-macro crates (`syn`, `quote`, `serde_derive`, `proc-macro2`).
  `basic-toml` adds about 7 but is in maintenance mode; a hand-written
  parser is not worth owning.

## Consequences

- The dependency rules in `CLAUDE.md` and the reviewer agent's prompt name
  `cli -> config`.
- **A new setting** (thresholds, the `--fail-on` default) is a new table in
  `qgate-config` plus wiring in the CLI. Analyzers keep their own types.
- **The vocabulary of `qgate.toml` is closed.** Unknown keys are errors, so
  a newer `qgate.toml` can fail on an older qgate. This is accepted: a
  misspelled key must not silently switch a rule off.
