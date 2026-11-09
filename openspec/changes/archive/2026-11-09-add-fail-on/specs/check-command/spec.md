# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Findings do not change the exit code`
- TO: `### Requirement: Failing on findings`

## MODIFIED Requirements

### Requirement: Failing on findings

When the analysis ran, the system SHALL exit with code 1 if at least one
finding has a severity at or above the level given by `--fail-on`, and with
code 0 otherwise. The levels are `error` (the default), `warning` and
`never`, which never fails on findings. Exit code 2 for a check that could
not run takes precedence. Standard output SHALL be the same whatever the
level.

#### Scenario: Error finding

- **GIVEN** inputs that produce a finding with severity Error
- **WHEN** the user runs `qgate check` without `--fail-on`
- **THEN** the exit code is 1

#### Scenario: Warnings pass by default

- **GIVEN** inputs that produce only Warning findings
- **WHEN** the user runs `qgate check` without `--fail-on`
- **THEN** the exit code is 0

#### Scenario: Failing on warnings

- **GIVEN** inputs that produce only Warning findings
- **WHEN** the user runs `qgate check --fail-on warning`
- **THEN** the exit code is 1

#### Scenario: Never failing

- **GIVEN** inputs that produce a finding with severity Error
- **WHEN** the user runs `qgate check --fail-on never`
- **THEN** the exit code is 0
- **AND** standard output is the same as without `--fail-on never`

#### Scenario: Unparsable file in revision mode

- **GIVEN** revision mode reports only an `input/unparsable` finding
- **WHEN** the user runs `qgate check --base main`
- **THEN** the exit code is 0
- **AND** with `--fail-on warning` the exit code is 1

#### Scenario: Errors that stop the check win

- **WHEN** the user runs `qgate check --base no-such-branch --fail-on warning`
- **THEN** the exit code is 2

#### Scenario: Summary of a failing run

- **GIVEN** inputs that produce two findings with severity Error and one with severity Warning
- **WHEN** the user runs `qgate check`
- **THEN** standard error ends with the line `qgate: failing: 2 findings at or above "error" (see --fail-on)`
- **AND** standard output lists all three findings

#### Scenario: A passing run prints no summary

- **GIVEN** inputs that produce only Warning findings
- **WHEN** the user runs `qgate check`
- **THEN** standard error is empty

### Requirement: Compare two git revisions

The system SHALL provide `qgate check --base <rev> [--head <rev>]`, with
`--head` defaulting to `HEAD`, that analyzes the files changed between the
two revisions in the git repository containing the current directory (see
the `revision-input` capability) and prints every finding to standard output,
one per line.

#### Scenario: Pull request branch

- **GIVEN** a repository where branch `feature` made `f` in `src/a.ts` more complex than on `main`, from 2 to 4 (a Warning)
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** standard output contains one `complexity/cyclomatic-increase` line for `src/a.ts`
- **AND** the exit code is 0

#### Scenario: Head defaults to HEAD

- **GIVEN** the checked-out branch made `f` in `src/a.ts` more complex than on `main`
- **WHEN** the user runs `qgate check --base main`
- **THEN** the output is the same as for `qgate check --base main --head HEAD`

### Requirement: Supported input files

In the two-file mode the system SHALL accept files with the extensions
`.ts`, `.mts`, `.cts` and `.tsx`, and SHALL reject any other file with an
error naming the file and exit code 2. The two files SHALL be accepted even
if their extensions differ, as long as both are in this set.

#### Scenario: TSX input

- **WHEN** the user runs `qgate check before.tsx after.tsx` on valid TSX files that produce no Error finding
- **THEN** the analysis runs and the exit code is 0

#### Scenario: Unsupported extension

- **WHEN** the user runs `qgate check before.js after.js`
- **THEN** standard error contains an error that names `before.js` as an unsupported file type
- **AND** standard output is empty
- **AND** the exit code is 2

### Requirement: Invalid invocation

The system SHALL treat as a usage error a `check` command that has neither
exactly two file arguments nor `--base`, that has both file arguments and
`--base`, that has `--head` without `--base`, that has a `--fail-on` value
other than `error`, `warning` or `never`, or that has an unknown argument:
print a usage message to standard error and exit with code 2.

#### Scenario: One argument

- **WHEN** the user runs `qgate check after.ts`
- **THEN** standard error shows how to call `qgate check`
- **AND** the exit code is 2

#### Scenario: Files and revisions together

- **WHEN** the user runs `qgate check before.ts after.ts --base main`
- **THEN** standard error shows a usage error
- **AND** the exit code is 2

#### Scenario: Head without base

- **WHEN** the user runs `qgate check --head feature`
- **THEN** standard error shows a usage error
- **AND** the exit code is 2

#### Scenario: Unknown fail-on level

- **WHEN** the user runs `qgate check a.ts b.ts --fail-on info`
- **THEN** standard error names the allowed values `error`, `warning` and `never`
- **AND** the exit code is 2
