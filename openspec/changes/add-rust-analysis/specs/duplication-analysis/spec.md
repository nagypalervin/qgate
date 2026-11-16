# Spec Delta

## MODIFIED Requirements

### Requirement: Compared files

In revision mode the system SHALL compare the head versions of the changed
files with every regular file of the head revision that has a supported
extension and the same language family: C# with C#, Rust with Rust,
TypeScript with TypeScript. A file that cannot be parsed SHALL take no
part; a changed one is reported as `input/unparsable` as before, an
unchanged one is skipped silently. The two-file form SHALL NOT check
duplication.

#### Scenario: Copy of an existing file the change does not touch

- **GIVEN** `src/utils/date.ts`, unchanged by the change, contains a block that the change copies into `src/orders/format.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `src/orders/format.ts` names `src/utils/date.ts`

#### Scenario: Copy of an existing C# file

- **GIVEN** `src/Utils/Dates.cs`, unchanged by the change, contains a method that the change copies, renamed, into `src/Orders/Format.cs`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `src/Orders/Format.cs` names `src/Utils/Dates.cs`

#### Scenario: Copy of an existing Rust file

- **GIVEN** `crates/a/src/dates.rs`, unchanged by the change, contains a function that the change copies, renamed, into `crates/b/src/format.rs`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `crates/b/src/format.rs` names `crates/a/src/dates.rs`

#### Scenario: The same tokens in two languages

- **GIVEN** the change adds a C# file and a TypeScript file whose code happens to give the same 60 normalized tokens
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Two-file form

- **WHEN** the user runs `qgate check before.ts after.ts` where `after.ts` copies a 60-token block from `other.ts` in the same directory
- **THEN** no `duplication/duplicate-block` finding is reported
