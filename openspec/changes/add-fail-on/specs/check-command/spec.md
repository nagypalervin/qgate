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
