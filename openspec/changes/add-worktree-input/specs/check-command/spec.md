## MODIFIED Requirements

### Requirement: Compare two git revisions

The system SHALL provide `qgate check --base <rev> [--head <rev>]`, with
`--head` defaulting to `HEAD`, that analyzes the files changed between the
two revisions in the git repository containing the current directory (see
the `revision-input` capability) and prints every finding to standard output,
one per line. It SHALL also provide `qgate check --base <rev> --worktree`,
which checks the working tree instead of a head revision (see "Working tree
as head" in `revision-input`) and prints its findings in the same way.

#### Scenario: Pull request branch

- **GIVEN** a repository where branch `feature` made `f` in `src/a.ts` more complex than on `main`, from 2 to 4 (a Warning)
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** standard output contains one `complexity/cyclomatic-increase` line for `src/a.ts`
- **AND** the exit code is 0

#### Scenario: Head defaults to HEAD

- **GIVEN** the checked-out branch made `f` in `src/a.ts` more complex than on `main`
- **WHEN** the user runs `qgate check --base main`
- **THEN** the output is the same as for `qgate check --base main --head HEAD`

#### Scenario: Working tree

- **GIVEN** `f` in `src/a.ts` was made more complex on disk, from 2 to 4 (a Warning), without a commit
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard output contains one `complexity/cyclomatic-increase` line for `src/a.ts`
- **AND** the exit code is 0

### Requirement: Invalid invocation

The system SHALL treat as a usage error a `check` command that has neither
exactly two file arguments nor `--base`, that has both file arguments and
`--base`, that has `--head` without `--base`, that has `--worktree` without
`--base`, that has both `--worktree` and `--head`, that has a `--fail-on`
value other than `error`, `warning` or `never`, or that has an unknown
argument: print a usage message to standard error and exit with code 2.

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

#### Scenario: Unknown fail-on level

- **WHEN** the user runs `qgate check a.ts b.ts --fail-on info`
- **THEN** standard error names the allowed values `error`, `warning` and `never`
- **AND** the exit code is 2
