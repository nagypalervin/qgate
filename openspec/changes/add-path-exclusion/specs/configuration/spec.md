# Spec Delta

## ADDED Requirements

### Requirement: Excluded paths

`qgate.toml` MAY list path patterns under `exclude` in a `[files]`
table, with the syntax and spelling of architecture rule patterns (see
"Patterns" and "Pattern spelling" in `architecture-analysis`). In
revision mode a version of a file whose path matches one SHALL be left
out as if its extension were not supported: not analyzed, not compared
for copies, and no code that existed before, also when it is a deleted
or renamed JavaScript file.

#### Scenario: Migrations left out

- **GIVEN** `qgate.toml` has `[files]` with `exclude = ["**/Migrations/**"]`, and the change adds `src/Data/Migrations/20261120_AddLines.cs`, which repeats a block of the unchanged `src/Data/Migrations/20261101_Init.cs` and has a method of complexity 25
- **WHEN** the user runs `qgate check --base main`
- **THEN** no finding is reported for either file

#### Scenario: An excluded file is no original

- **GIVEN** `exclude = ["vendor"]`, and the change adds `src/orders/format.ts`, which copies a block of the unchanged `vendor/date.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: A deleted excluded file is no earlier code

- **GIVEN** `exclude = ["vendor"]`, `vendor/date.js` and `src/utils/date.ts` share a block, and the change deletes `vendor/date.js` and adds `src/orders/format.ts` with the block and other code
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `src/orders/format.ts` names `src/utils/date.ts`

#### Scenario: Renamed out of an excluded directory

- **GIVEN** `exclude = ["legacy/**"]`, and the change renames `legacy/Report.cs`, which has a method of complexity 15, to `src/Report.cs` without other edits
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `complexity/cyclomatic-new` Warning is reported for `src/Report.cs`

#### Scenario: Excluded in the working tree

- **GIVEN** the working tree's `qgate.toml` excludes `generated/**`, and an uncommitted `generated/Client.ts` has a function of complexity 25
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** no finding is reported

#### Scenario: Invalid exclude pattern

- **GIVEN** `qgate.toml` has `[files]` with `exclude = ["/src/**"]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml`, `files.exclude` and the pattern `/src/**`
- **AND** the exit code is 2

## MODIFIED Requirements

### Requirement: Missing configuration

When the head revision has no `qgate.toml`, the system SHALL apply no
architecture rules, exclude no file and otherwise behave as without a
configuration file.

#### Scenario: Repository without qgate.toml

- **GIVEN** a repository with no `qgate.toml` in which `feature` makes a function more complex
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** the output is the same as before this change
