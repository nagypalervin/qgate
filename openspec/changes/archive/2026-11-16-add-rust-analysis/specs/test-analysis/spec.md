# Spec Delta

## MODIFIED Requirements

### Requirement: Checked changes

In revision mode the system SHALL check every changed source file whose
head version is supported, not excluded by `[files] exclude`, not
generated C#, and parses. Deleted files SHALL NOT be checked. The
two-file form SHALL NOT check tests. Rust files SHALL be neither
sources nor tests: they SHALL NOT be checked, and no Rust file SHALL
count as the test of another file.

#### Scenario: Two-file form

- **WHEN** the user runs `qgate check before.ts after.ts` where `after.ts` adds a function and no `after.spec.ts` exists
- **THEN** no `tests/` finding is reported

#### Scenario: Unparsable source file

- **GIVEN** the change edits `src/app/cart.service.ts`, which has `cart.service.spec.ts` beside it, and leaves a syntax error in it
- **WHEN** the user runs `qgate check --base main`
- **THEN** an `input/unparsable` finding is reported for it, and no `tests/` finding

#### Scenario: Working tree

- **GIVEN** an uncommitted edit changes the body of `Cart.total` in `src/app/cart.service.ts`, and `src/app/cart.service.spec.ts` is unchanged
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** a `tests/unchanged-test` finding is reported for `src/app/cart.service.ts`

#### Scenario: Rust files

- **GIVEN** a repository with Rust files in which other crates have `tests/` directories, and the change adds `crates/shop/src/cart.rs` with two functions and no test
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported
