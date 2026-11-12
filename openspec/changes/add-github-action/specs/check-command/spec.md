# Spec Delta

## MODIFIED Requirements

### Requirement: Invalid invocation

The system SHALL treat as a usage error a `check` command that has neither
exactly two file arguments nor `--base`, that has both file arguments and
`--base`, that has `--head` without `--base`, that has `--worktree` without
`--base`, that has both `--worktree` and `--head`, that has a `--fail-on`
value other than `error`, `warning` or `never`, that has a `--format`
value other than `text`, `json`, `markdown`, `azure` or `github`, or
that has an unknown argument: print a usage message to standard error
and exit with code 2.

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

#### Scenario: Working tree without base

- **WHEN** the user runs `qgate check --worktree`
- **THEN** standard error shows a usage error
- **AND** the exit code is 2

#### Scenario: Working tree and head together

- **WHEN** the user runs `qgate check --base main --head feature --worktree`
- **THEN** standard error shows a usage error
- **AND** the exit code is 2

#### Scenario: Working tree with two files

- **WHEN** the user runs `qgate check before.ts after.ts --worktree`
- **THEN** standard error shows a usage error
- **AND** the exit code is 2

#### Scenario: Head with two files

- **WHEN** the user runs `qgate check before.ts after.ts --head feature`
- **THEN** standard error shows a usage error
- **AND** the exit code is 2

#### Scenario: Unknown fail-on level

- **WHEN** the user runs `qgate check a.ts b.ts --fail-on info`
- **THEN** standard error names the allowed values `error`, `warning` and `never`
- **AND** the exit code is 2

#### Scenario: Unknown format

- **WHEN** the user runs `qgate check a.ts b.ts --format xml`
- **THEN** standard error names the allowed values `text`, `json`, `markdown`, `azure` and `github`
- **AND** standard output is empty
- **AND** the exit code is 2
