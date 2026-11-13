# check-command Specification

## Purpose

The `qgate check` command is how a user asks qgate what got worse between
two versions of their code: it takes the inputs, runs the analysis and
reports findings or errors in a stable, scriptable way.

## Requirements

### Requirement: Compare two files

The system SHALL provide a `qgate check <before> <after>` command that
analyzes the two given files as the base and head version of the same file
and prints every finding to standard output, one per line.

#### Scenario: A function got more complex

- **GIVEN** `before.ts` contains `function f(a) { if (a) { return 1; } return 0; }`
- **AND** `after.ts` contains the same function with two more `if` statements
- **WHEN** the user runs `qgate check before.ts after.ts`
- **THEN** standard output contains one line for `f` with rule `complexity/cyclomatic-increase`
- **AND** the exit code is 0

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

### Requirement: Finding line format

Each finding line SHALL have the form `<severity> <path>:<line>  <message>  [<rule>]`,
where `<severity>` is `warning` or `error` left-aligned in a 7-character
column and `<line>` is the finding's line in the after file. In the two-file
mode `<path>` is the `<after>` argument exactly as given; in revision mode it
is the head path relative to the repository root. A finding without a line
SHALL be rendered as `<path>` alone.

#### Scenario: Rendered finding

- **GIVEN** a finding with severity Error for function `OrderService.process` on line 42, complexity 3 before and 9 after
- **WHEN** `qgate check old/orders.ts src/orders.ts` reports it
- **THEN** the line reads `error   src/orders.ts:42  OrderService.process: cyclomatic complexity 3 -> 9 (+6)  [complexity/cyclomatic-increase]`

#### Scenario: Path in revision mode from a subdirectory

- **GIVEN** a change to `src/orders.ts` between `main` and `HEAD`
- **WHEN** the user runs `qgate check --base main` from the `src` directory
- **THEN** the finding's path is `src/orders.ts`

### Requirement: No regressions

When the analysis produces no findings, the system SHALL print exactly
`No regressions found.` followed by a newline and exit with code 0.

#### Scenario: Identical files

- **WHEN** the user runs `qgate check a.ts a.ts`
- **THEN** standard output is `No regressions found.`
- **AND** the exit code is 0

#### Scenario: Only improvements

- **GIVEN** a function whose complexity went from 5 to 2, and a removed function
- **WHEN** the user runs `qgate check` on the two versions
- **THEN** standard output is `No regressions found.`

### Requirement: Deterministic output order

The system SHALL print findings sorted by path compared as a byte string
(the order git uses), then by line number ascending (a finding without a
line first), then by rule id, so that the same inputs always produce
byte-identical output.

#### Scenario: Several findings

- **GIVEN** the after file has a new complex function on line 30 and a function that got more complex on line 10
- **WHEN** the user runs `qgate check`
- **THEN** the finding for line 10 is printed before the finding for line 30

#### Scenario: Several files

- **GIVEN** findings in `src/b.ts` line 5 and `src/a.ts` line 40
- **WHEN** the user runs `qgate check --base main`
- **THEN** the finding for `src/a.ts` is printed before the finding for `src/b.ts`

#### Scenario: Byte order of paths

- **GIVEN** findings in `src/a.ts` and `src-x.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** the finding for `src-x.ts` is printed first, because `-` sorts before `/`

### Requirement: Failing on findings

When the analysis ran, the system SHALL exit with code 1 if at least one
finding has a severity at or above the level given by `--fail-on`, and with
code 0 otherwise. The levels are `error` (the default), `warning` and
`never`, which never fails on findings. Exit code 2 for a check that could
not run takes precedence. Standard output SHALL be the same whatever the
level, except for the summary of the JSON report, which names the level
and counts the findings at or above it (see "JSON summary" in
`report-formats`).

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

### Requirement: Supported input files

In the two-file mode the system SHALL accept files with the extensions
`.ts`, `.mts`, `.cts`, `.tsx` and `.cs`, and SHALL reject any other file
with an error naming the file and exit code 2. The two files SHALL be
accepted even if their extensions differ, as long as both are in this
set and both are C# or neither is; a C# file compared with a TypeScript
file SHALL be rejected with an error naming both files and exit code 2.

#### Scenario: TSX input

- **WHEN** the user runs `qgate check before.tsx after.tsx` on valid TSX files that produce no Error finding
- **THEN** the analysis runs and the exit code is 0

#### Scenario: C# input

- **WHEN** the user runs `qgate check Before.cs After.cs` on valid C# files that produce no Error finding
- **THEN** the analysis runs and the exit code is 0

#### Scenario: C# compared with TypeScript

- **WHEN** the user runs `qgate check before.ts after.cs`
- **THEN** standard error contains an error that names `before.ts` and `after.cs`
- **AND** standard output is empty
- **AND** the exit code is 2

#### Scenario: Unsupported extension

- **WHEN** the user runs `qgate check before.js after.js`
- **THEN** standard error contains an error that names `before.js` as an unsupported file type
- **AND** standard output is empty
- **AND** the exit code is 2

### Requirement: Unreadable input files

In the two-file mode the system SHALL report a missing or unreadable input
file as an error that names the file, print nothing to standard output and
exit with code 2.

#### Scenario: Missing file

- **WHEN** the user runs `qgate check before.ts does-not-exist.ts`
- **THEN** standard error contains an error that names `does-not-exist.ts`
- **AND** the exit code is 2

### Requirement: Files with syntax errors

In the two-file mode the system SHALL refuse to analyze a file that contains
a syntax error, report the file and the line of the first syntax error on
standard error, print nothing to standard output and exit with code 2. In
revision mode such a file is reported as a finding instead (see the
`revision-input` capability).

#### Scenario: Broken after file

- **GIVEN** `after.ts` has an unclosed `{` that makes line 7 the first syntax error
- **WHEN** the user runs `qgate check before.ts after.ts`
- **THEN** standard error contains `after.ts:7` and says it has a syntax error
- **AND** the exit code is 2

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

### Requirement: Help and version

The system SHALL keep `qgate --help` and `qgate --version` working, exit with
code 0 for both, and list the `check` command in the help output.

#### Scenario: Version

- **WHEN** the user runs `qgate --version`
- **THEN** standard output starts with `qgate ` followed by the package version
- **AND** the exit code is 0

#### Scenario: Help lists check

- **WHEN** the user runs `qgate --help`
- **THEN** the output mentions the `check` command
