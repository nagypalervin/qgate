# Spec Delta

## MODIFIED Requirements

### Requirement: Ranked files

The ranked files SHALL be the files of the head revision whose language
is supported, that `[files] exclude` does not name, that are not
generated C# and that are not tests by the conventions of
`test-analysis`, each language family ranked on its own: TypeScript
with TypeScript, C# with C#, Rust with Rust. Rust has no test
conventions, so every Rust file is ranked. A test's findings get no
hotspot context.

#### Scenario: Excluded migrations

- **GIVEN** `[files]` has `exclude = ["**/Migrations/**"]`, and migrations changed in every commit
- **THEN** no migration is ranked, and they push no other file out of the top

#### Scenario: Tests are not ranked

- **GIVEN** `src/cart.spec.ts` changed in more commits than `src/cart.ts` and is more complex, and the change makes both more complex
- **WHEN** the user runs `qgate check --base main`
- **THEN** the finding for `src/cart.ts` has hotspot context and the one for `src/cart.spec.ts` has none

#### Scenario: Families apart

- **GIVEN** a repository with 40 C# files and 400 TypeScript files
- **THEN** a C# file's rank is among the 40 C# files only

#### Scenario: Rust files

- **GIVEN** a repository with 30 Rust files, among them `crates/cli/tests/check.rs`, an integration test
- **THEN** each of the 30 is ranked among the Rust files, `crates/cli/tests/check.rs` too
