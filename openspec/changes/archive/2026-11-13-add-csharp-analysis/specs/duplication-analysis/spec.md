# Spec Delta

## MODIFIED Requirements

### Requirement: Code without tokens

In TypeScript and JavaScript, import declarations, `export ... from`
declarations and type-level code SHALL produce no tokens. Type-level code
includes type annotations, type and interface declarations, overload and
abstract signatures, abstract and `declare` fields, index signatures,
`declare` declarations, type-only exports, `implements` clauses, and the
`as T`, `satisfies T`, non-null and definite-assignment `!`, optional `?`
and `this` parameter syntax.

#### Scenario: Imports and types are not compared

- **GIVEN** two files that share only a long list of `import` declarations and an identical `interface`
- **THEN** no tokens are compared for them and no duplicate is found

#### Scenario: Declaration files

- **GIVEN** two declaration files, each with dozens of `export interface` and `export declare function` declarations
- **THEN** they produce no tokens and no duplicate is found

### Requirement: Compared files

In revision mode the system SHALL compare the head versions of the changed
files with every regular file of the head revision that has a supported
extension and the same language family: C# with C#, TypeScript with
TypeScript. A file that cannot be parsed SHALL take no
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

#### Scenario: The same tokens in two languages

- **GIVEN** the change adds a C# file and a TypeScript file whose code happens to give the same 60 normalized tokens
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Two-file form

- **WHEN** the user runs `qgate check before.ts after.ts` where `after.ts` copies a 60-token block from `other.ts` in the same directory
- **THEN** no `duplication/duplicate-block` finding is reported
