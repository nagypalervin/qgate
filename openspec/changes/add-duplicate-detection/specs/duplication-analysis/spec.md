# Spec Delta

## Purpose

Duplication analysis makes copied code visible per change: it reports the
blocks that a change newly duplicates, including copies with renamed
variables, and names an existing copy so the change can reuse it instead.

## ADDED Requirements

### Requirement: Normalized tokens

The system SHALL compare code as normalized tokens; comments, semicolons
and layout SHALL be ignored. Identifiers SHALL become one placeholder.
Strings, numbers and regexes, and each text part of a template or of JSX
text, SHALL become another, one per literal or part whatever its content.
Member, key, class and enum member names, JSX attribute and namespaced
names, lowercase JSX element names, keywords (`true`, `null` included),
operators and other punctuation SHALL be kept.

#### Scenario: Renamed copy

- **GIVEN** `function fmt(d: Date) { const y = d.getFullYear(); return y + "-"; }`
- **AND** `function formatOrderDate(when: Date) { const year = when.getFullYear(); return year + "/"; }`
- **THEN** both have the same normalized tokens

#### Scenario: Different member names

- **GIVEN** two functions that differ only in calling `.getFullYear()` and `.getUTCFullYear()`
- **THEN** their normalized tokens differ

### Requirement: Code without tokens

Import declarations, `export ... from` declarations and type-level code
SHALL produce no tokens. Type-level code includes type annotations, type
and interface declarations, overload and abstract signatures, index
signatures, `declare` declarations, type-only exports, and the `as T`,
`satisfies T`, `x!`, optional `?` and `this` parameter syntax.

#### Scenario: Imports and types are not compared

- **GIVEN** two files that share only a long list of `import` declarations and an identical `interface`
- **THEN** no tokens are compared for them and no duplicate is found

#### Scenario: Declaration files

- **GIVEN** two declaration files, each with dozens of `export interface` and `export declare function` declarations
- **THEN** they produce no tokens and no duplicate is found

### Requirement: Compared files

In revision mode the system SHALL compare the head versions of the changed
files with every regular file of the head revision that has a supported
extension. A file that cannot be parsed SHALL take no part; a changed one
is reported as `input/unparsable` as before, an unchanged one is skipped
silently. The two-file form SHALL NOT check duplication.

#### Scenario: Copy of an existing file the change does not touch

- **GIVEN** `src/utils/date.ts`, unchanged by the change, contains a block that the change copies into `src/orders/format.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `src/orders/format.ts` names `src/utils/date.ts`

#### Scenario: Two-file form

- **WHEN** the user runs `qgate check before.ts after.ts` where `after.ts` contains the same 60-token block twice
- **THEN** no `duplication/duplicate-block` finding is reported

### Requirement: Duplicate blocks

A duplicate block SHALL be a maximal run of at least 50 consecutive
normalized tokens that occurs at two or more places in the compared
files, in the same file or in different files.

#### Scenario: Below the threshold

- **GIVEN** the change copies a block of 40 normalized tokens from an existing file
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

### Requirement: Only new duplication is reported

A duplicate block SHALL be reported only if the change increased its
number of occurrences: it occurs at least twice in the head revision, and
the changed files contain more occurrences after the change than before.
Only occurrences the change added SHALL be reported. When every occurrence
is new, the first by path and position SHALL count as the original. A file
renamed from JavaScript counts its JavaScript version as before.

#### Scenario: Existing duplication

- **GIVEN** two files already share a 60-token block, and the change edits one of them elsewhere
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Moved code

- **GIVEN** the change moves a 60-token block from `src/a.ts` into `src/b.ts`, and no other copy exists
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Renamed from JavaScript

- **GIVEN** `src/util.js` shares a 60-token block with `src/a.ts`, and the change renames `src/util.js` to `src/util.ts` without other edits
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Copy within the change

- **GIVEN** the change adds the same 60-token block to new files `src/a.ts` and `src/b.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** one finding is reported, for `src/b.ts`, naming `src/a.ts`

### Requirement: Duplicate block finding

Each reported occurrence SHALL be one finding with rule
`duplication/duplicate-block` and severity Warning, at the path and first
line of the new occurrence. The message SHALL be `duplicates
<path>:<first>-<last> (lines <first>-<last>, <n> tokens)`, naming the
first other occurrence by path and line that is not itself reported; with
`k` further occurrences, `, and <k> more` SHALL end the parentheses.

#### Scenario: Message

- **GIVEN** `src/utils/date.ts` lines 3-9 hold a 54-token block, and the change adds a renamed copy at lines 12-18 of `src/orders/format.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output contains `warning src/orders/format.ts:12  duplicates src/utils/date.ts:3-9 (lines 12-18, 54 tokens)  [duplication/duplicate-block]`
- **AND** the exit code is 0

#### Scenario: Several existing copies

- **GIVEN** the block of the previous scenario also exists in `src/a.ts` and `src/b.ts`
- **WHEN** the change adds another copy to `src/orders/format.ts`
- **THEN** the message names `src/a.ts` and ends with `54 tokens, and 2 more)`

#### Scenario: Failing on duplication

- **WHEN** the user runs `qgate check --base main --fail-on warning` on a change that adds a duplicate block
- **THEN** the exit code is 1
