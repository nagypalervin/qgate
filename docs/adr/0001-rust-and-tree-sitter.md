# ADR 0001: Rust and tree-sitter

- Status: accepted
- Date: 2026-11-08

## Context

qgate has to parse source code in several languages, compare two
revisions quickly enough to run on every pull request, and be trivial to
install in CI and on developer machines.

## Decision

- Write qgate in **Rust**, organised as a Cargo workspace of small crates
  (`core`, `lang`, `analyzers`, `report`, `cli`).
- Parse source code with **tree-sitter**, behind language adapters in
  `qgate-lang` that produce a language-neutral model.
- Start with **TypeScript** as the first supported language.

## Reasons

- A single static binary: no runtime to install in CI, fast startup.
- tree-sitter has mature Rust bindings and grammars for most languages, and
  is error-tolerant, which matters for half-finished code.
- Separate crates keep the dependency direction explicit and enforceable.
- TypeScript is widely used with AI coding assistants and familiar to the
  author, so results are easy to judge by hand.
- Rust is new to the author. That is intentional: the project is also an
  experiment in how well AI assistance works in a less familiar language.

## Consequences

- Contributors need a Rust toolchain.
- Each new language needs an adapter and fixtures, but no analyzer changes.
- Compile times will grow with grammars; we may feature-gate languages later.
