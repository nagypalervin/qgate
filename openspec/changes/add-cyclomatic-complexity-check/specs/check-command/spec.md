# Spec Delta

## Purpose

The `qgate check` command is how a user asks qgate what got worse between
two versions of their code: it takes the inputs, runs the analysis and
reports findings or errors in a stable, scriptable way.

## ADDED Requirements

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

### Requirement: Finding line format

Each finding line SHALL have the form `<severity> <path>:<line>  <message>  [<rule>]`,
where `<severity>` is `warning` or `error` left-aligned in a 7-character
column, `<path>` is the `<after>` argument exactly as given, and `<line>` is
the finding's line in the after file.

#### Scenario: Rendered finding

- **GIVEN** a finding with severity Error for function `OrderService.process` on line 42, complexity 3 before and 9 after
- **WHEN** `qgate check old/orders.ts src/orders.ts` reports it
- **THEN** the line reads `error   src/orders.ts:42  OrderService.process: cyclomatic complexity 3 -> 9 (+6)  [complexity/cyclomatic-increase]`

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

The system SHALL print findings sorted by line number ascending, then by rule
id, so that the same inputs always produce byte-identical output.

#### Scenario: Several findings

- **GIVEN** the after file has a new complex function on line 30 and a function that got more complex on line 10
- **WHEN** the user runs `qgate check`
- **THEN** the finding for line 10 is printed before the finding for line 30

### Requirement: Findings do not change the exit code

In this version the system SHALL exit with code 0 whenever the analysis ran,
regardless of the number or severity of findings.

#### Scenario: Error finding

- **GIVEN** inputs that produce a finding with severity Error
- **WHEN** the user runs `qgate check`
- **THEN** the exit code is 0

### Requirement: Supported input files

The system SHALL accept files with the extensions `.ts`, `.mts`, `.cts` and
`.tsx`, and SHALL reject any other file with an error naming the file and
exit code 2. The two files SHALL be accepted even if their extensions
differ, as long as both are in this set.

#### Scenario: TSX input

- **WHEN** the user runs `qgate check before.tsx after.tsx` on valid TSX files
- **THEN** the analysis runs and the exit code is 0

#### Scenario: Unsupported extension

- **WHEN** the user runs `qgate check before.js after.js`
- **THEN** standard error contains an error that names `before.js` as an unsupported file type
- **AND** standard output is empty
- **AND** the exit code is 2

### Requirement: Unreadable input files

The system SHALL report a missing or unreadable input file as an error that
names the file, print nothing to standard output and exit with code 2.

#### Scenario: Missing file

- **WHEN** the user runs `qgate check before.ts does-not-exist.ts`
- **THEN** standard error contains an error that names `does-not-exist.ts`
- **AND** the exit code is 2

### Requirement: Files with syntax errors

The system SHALL refuse to analyze a file that contains a syntax error,
report the file and the line of the first syntax error on standard error,
print nothing to standard output and exit with code 2.

#### Scenario: Broken after file

- **GIVEN** `after.ts` has an unclosed `{` that makes line 7 the first syntax error
- **WHEN** the user runs `qgate check before.ts after.ts`
- **THEN** standard error contains `after.ts:7` and says it has a syntax error
- **AND** the exit code is 2

### Requirement: Invalid invocation

The system SHALL treat a `check` command with fewer or more than two file
arguments, or an unknown argument, as a usage error: print a usage message
to standard error and exit with code 2.

#### Scenario: One argument

- **WHEN** the user runs `qgate check after.ts`
- **THEN** standard error shows how to call `qgate check`
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
