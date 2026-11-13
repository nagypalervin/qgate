# configuration Specification

## Purpose

Configuration lets a repository tell qgate what to enforce: it defines
where `qgate.toml` is read from and in which revision, and what happens
when the file is missing or invalid.

## Requirements

### Requirement: Configuration from the head revision

In revision mode the system SHALL read the configuration from the file
`qgate.toml` at the root of the repository as it is in the head revision.
Without `--worktree`, uncommitted changes to the file SHALL NOT affect the
result. With `--worktree`, the working tree is the head revision (see
"Working tree as head" in `revision-input`), so the file SHALL be read as
it is on disk, and an untracked `qgate.toml` that is not ignored counts.

#### Scenario: Rules come from the head revision

- **GIVEN** branch `feature` adds a forbid rule to `qgate.toml` and an import that the rule forbids
- **WHEN** the user runs `qgate check --base main --head feature` with `main` checked out
- **THEN** the forbidden import is reported

#### Scenario: Uncommitted changes are ignored

- **GIVEN** the head revision has no `qgate.toml`, and the working tree has one with a forbid rule that the changed files break
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported

#### Scenario: Uncommitted changes count with the working tree

- **GIVEN** the head revision has no `qgate.toml`, and the working tree has an untracked one with a forbid rule that an uncommitted import breaks
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** the forbidden import is reported

#### Scenario: Invalid configuration on disk

- **GIVEN** `qgate.toml` is valid in `HEAD` but has a syntax error on disk
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard error reports the invalid `qgate.toml`
- **AND** the exit code is 2

### Requirement: Missing configuration

When the head revision has no `qgate.toml`, the system SHALL apply no
architecture rules, exclude no file and otherwise behave as without a
configuration file.

#### Scenario: Repository without qgate.toml

- **GIVEN** a repository with no `qgate.toml` in which `feature` makes a function more complex
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** the output is the same as before this change

### Requirement: Invalid configuration

When `qgate.toml` exists in the head revision but is not valid, the system
SHALL print an error naming `qgate.toml` and the problem to standard error,
print nothing to standard output and exit with code 2. Not valid means: not
TOML, an unknown table or key, a value of the wrong type, a rule with an
empty or duplicate `name`, an empty `from` or `to` list, an invalid
pattern, or a `qgate.toml` that is not a regular file.

#### Scenario: Syntax error

- **GIVEN** line 3 of `qgate.toml` is `name = "domain-is-pure`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml` and line 3
- **AND** the exit code is 2

#### Scenario: Misspelled table

- **GIVEN** `qgate.toml` contains `[[architecture.forbd]]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml` and `forbd`
- **AND** the exit code is 2

#### Scenario: Duplicate rule name

- **GIVEN** two forbid rules in `qgate.toml` are both named `domain-is-pure`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `domain-is-pure` as a duplicate
- **AND** the exit code is 2

#### Scenario: Empty pattern list

- **GIVEN** a forbid rule in `qgate.toml` has `to = []`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names the rule and `to`
- **AND** the exit code is 2

#### Scenario: Invalid pattern

- **GIVEN** a forbid rule in `qgate.toml` has `from = ["src/[domain"]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names the rule and the pattern `src/[domain`
- **AND** the exit code is 2

#### Scenario: Not a regular file

- **GIVEN** `qgate.toml` is a directory in the head revision
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml`
- **AND** the exit code is 2

### Requirement: The two-file form ignores the configuration

`qgate check <before> <after>` SHALL NOT read `qgate.toml` and SHALL NOT
apply architecture rules.

#### Scenario: Invalid qgate.toml next to two files

- **GIVEN** the current directory contains a `qgate.toml` that is not TOML
- **WHEN** the user runs `qgate check before.ts after.ts` on files with no findings
- **THEN** standard output is `No regressions found.`
- **AND** the exit code is 0

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
