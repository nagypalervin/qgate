# Spec Delta

## Purpose

Defines the formats `qgate check` can write its findings in for programs
to read, such as AI coding agents, hooks, the MCP server and CI scripts,
and the `--format` option that selects one.

## ADDED Requirements

### Requirement: Format option

`qgate check` SHALL take `--format <format>` in every form (two files,
revisions, working tree). `text`, the default, SHALL keep the output the
`check-command` capability specifies. `json` SHALL write the JSON report
instead.

#### Scenario: Default is text

- **GIVEN** `f` in `src/a.ts` went from 2 to 4 between `main` and `HEAD`
- **WHEN** the user runs `qgate check --base main --format text`
- **THEN** standard output is the same as for `qgate check --base main`

#### Scenario: JSON in every form

- **WHEN** the user runs `qgate check old.ts new.ts --format json`, `qgate check --base main --format json` or `qgate check --base main --worktree --format json`
- **THEN** standard output is a JSON report

### Requirement: JSON report document

With `--format json`, standard output SHALL be exactly one JSON object in
UTF-8 on a single line, followed by a newline. It SHALL have the keys
`schema_version` (the number `1`), `findings` (an array) and `summary`
(an object), in this order. The same inputs SHALL give byte-identical
output.

#### Scenario: One finding

- **GIVEN** `f` in `src/a.ts` went from 2 to 4 between `main` and `HEAD`, and nothing else changed
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** standard output is `{"schema_version":1,"findings":[{"rule":"complexity/cyclomatic-increase","severity":"warning","message":"f: cyclomatic complexity 2 -> 4 (+2)","path":"src/a.ts","line":1,"metric":{"before":2,"after":4}}],"summary":{"findings":1,"failing":0,"fail_on":"error"}}` followed by a newline
- **AND** the exit code is 0

#### Scenario: No findings

- **GIVEN** the change makes nothing worse
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** standard output is `{"schema_version":1,"findings":[],"summary":{"findings":0,"failing":0,"fail_on":"error"}}` followed by a newline

### Requirement: Finding fields

Each finding SHALL be an object with `rule`, `severity` (`error`,
`warning` or `info`), `message` and `path`, as the text format shows them,
and, when the finding has them: `line`, `end_line`, `metric` (`before`
and `after`, each only when known) and `related` (a non-empty array of
objects with `path`, `line` and `end_line`). A field without a value
SHALL be left out, never `null`.

#### Scenario: New function

- **GIVEN** a new function `big` with complexity 12 on line 1 of the added `src/new.ts`
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding is `{"rule":"complexity/cyclomatic-new","severity":"warning","message":"big: new function, cyclomatic complexity 12","path":"src/new.ts","line":1,"metric":{"after":12}}`

#### Scenario: Finding without a line

- **GIVEN** `src/a.ts` is not valid UTF-8 in the head revision
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding has `"path":"src/a.ts"` and no `line`, `end_line`, `metric` or `related` key

#### Scenario: Duplicate block

- **GIVEN** `src/utils/date.ts` lines 1-6 hold a 68-token block, and the change adds a renamed copy at lines 4-9 of `src/orders/format.ts`
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding is `{"rule":"duplication/duplicate-block","severity":"warning","message":"duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)","path":"src/orders/format.ts","line":4,"end_line":9,"related":[{"path":"src/utils/date.ts","line":1,"end_line":6}]}`

#### Scenario: Characters JSON must escape

- **GIVEN** a forbidden import whose message contains `"` characters
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** the message is written with each `"` escaped as `\"`, and the output parses as JSON

### Requirement: Numbers in the JSON report

Metric values and line numbers SHALL be JSON numbers. A whole number SHALL
be written without a fraction or exponent (`4`, not `4.0`).

#### Scenario: Complexity values

- **WHEN** a finding's complexity goes from 2 to 4
- **THEN** its metric is written as `{"before":2,"after":4}`

### Requirement: Order of findings in the JSON report

The `findings` array SHALL hold the findings in the order the text format
prints them (see "Deterministic output order" in `check-command`).

#### Scenario: Several files

- **GIVEN** findings in `src/b.ts` line 5 and `src/a.ts` line 40
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** the finding for `src/a.ts` comes first in `findings`

### Requirement: JSON summary

`summary` SHALL hold `findings` (the number of findings), `failing` (how
many of them are at or above the `--fail-on` level) and `fail_on` (that
level as written on the command line, `error` by default).

#### Scenario: Failing run

- **GIVEN** inputs that produce two Error findings and one Warning finding
- **WHEN** the user runs `qgate check --base main --format json --fail-on warning`
- **THEN** `summary` is `{"findings":3,"failing":3,"fail_on":"warning"}`
- **AND** the exit code is 1

#### Scenario: Never failing

- **GIVEN** inputs that produce one Error finding
- **WHEN** the user runs `qgate check --base main --format json --fail-on never`
- **THEN** `summary` is `{"findings":1,"failing":0,"fail_on":"never"}`
- **AND** the exit code is 0

### Requirement: Exit codes and errors with JSON

The exit codes and the summary line on standard error SHALL be the same
in both formats. When the check cannot run, standard output SHALL stay
empty and the error SHALL be written to standard error as text, as in the
text format.

#### Scenario: Unknown base revision

- **WHEN** the user runs `qgate check --base no-such-branch --format json`
- **THEN** standard output is empty
- **AND** standard error names `no-such-branch` as an unknown revision
- **AND** the exit code is 2

#### Scenario: Summary line on standard error

- **GIVEN** inputs that produce one Error finding
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** standard error ends with `qgate: failing: 1 finding at or above "error" (see --fail-on)`
- **AND** the exit code is 1

### Requirement: Versioned JSON format

Within `schema_version` 1, a later qgate MAY add keys to the envelope, the
summary, findings and related locations, but SHALL NOT remove a key, rename
it or change its type or meaning; that needs a new `schema_version`. The
repository SHALL hold a JSON Schema for version 1 that every report
validates against.

#### Scenario: Reports match the published schema

- **GIVEN** reports with complexity, architecture, duplication and `input/unparsable` findings, and an empty report
- **WHEN** each is checked against the JSON Schema for version 1
- **THEN** each one is valid

### Requirement: Paths in the JSON report

Paths SHALL be written as the text format shows them. A path that is not
valid UTF-8 SHALL have each invalid byte sequence replaced by U+FFFD, so
the report stays valid UTF-8.

#### Scenario: Path from the two-file form

- **WHEN** the user runs `qgate check old/orders.ts src/orders.ts --format json`
- **THEN** each finding's `path` is `src/orders.ts`
