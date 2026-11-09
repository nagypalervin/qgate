# Spec Delta

## ADDED Requirements

### Requirement: Compare two git revisions

The system SHALL provide `qgate check --base <rev> [--head <rev>]`, with
`--head` defaulting to `HEAD`, that analyzes the files changed between the
two revisions in the git repository containing the current directory (see
the `revision-input` capability) and prints every finding to standard output,
one per line.

#### Scenario: Pull request branch

- **GIVEN** a repository where branch `feature` made `f` in `src/a.ts` more complex than on `main`
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** standard output contains one `complexity/cyclomatic-increase` line for `src/a.ts`
- **AND** the exit code is 0

#### Scenario: Head defaults to HEAD

- **GIVEN** the checked-out branch made `f` in `src/a.ts` more complex than on `main`
- **WHEN** the user runs `qgate check --base main`
- **THEN** the output is the same as for `qgate check --base main --head HEAD`

## MODIFIED Requirements

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

### Requirement: Deterministic output order

The system SHALL print findings sorted by path, then by line number
ascending (a finding without a line first), then by rule id, so that the
same inputs always produce byte-identical output.

#### Scenario: Several findings

- **GIVEN** the after file has a new complex function on line 30 and a function that got more complex on line 10
- **WHEN** the user runs `qgate check`
- **THEN** the finding for line 10 is printed before the finding for line 30

#### Scenario: Several files

- **GIVEN** findings in `src/b.ts` line 5 and `src/a.ts` line 40
- **WHEN** the user runs `qgate check --base main`
- **THEN** the finding for `src/a.ts` is printed before the finding for `src/b.ts`

### Requirement: Supported input files

In the two-file mode the system SHALL accept files with the extensions
`.ts`, `.mts`, `.cts` and `.tsx`, and SHALL reject any other file with an
error naming the file and exit code 2. The two files SHALL be accepted even
if their extensions differ, as long as both are in this set.

#### Scenario: TSX input

- **WHEN** the user runs `qgate check before.tsx after.tsx` on valid TSX files
- **THEN** the analysis runs and the exit code is 0

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
`--base`, that has `--head` without `--base`, or that has an unknown
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
