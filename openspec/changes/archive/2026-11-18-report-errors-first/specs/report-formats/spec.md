## MODIFIED Requirements

### Requirement: Markdown report document

With `--format markdown`, standard output SHALL be one UTF-8 Markdown
document ending in a newline:
- the marker line;
- a blank line;
- the heading `### qgate`;
- a summary sentence;
- when not every finding fits the table, the rule summary;
- if there are findings, a table with the columns Severity, Location,
  Finding and Rule, one row per finding: Error findings first, then
  Warning, then Info, and within a severity in the text format's order.

The same inputs SHALL give byte-identical output.

#### Scenario: One error

- **GIVEN** inputs that produce one Error finding in `src/a.ts` on line 1, with the rule `complexity/cyclomatic-increase`
- **WHEN** the user runs `qgate check --base main --format markdown`
- **THEN** the table has one row with `error`, `src/a.ts:1`, the finding's message and `complexity/cyclomatic-increase`

#### Scenario: No findings

- **GIVEN** inputs that produce no findings
- **WHEN** the user runs `qgate check --base main --format markdown`
- **THEN** the summary sentence says that no regressions were found
- **AND** the document has no table

#### Scenario: Errors before Warnings

- **GIVEN** a Warning finding in `src/a.ts` on line 1 and an Error finding in `src/b.ts` on line 9
- **WHEN** the Markdown report is rendered
- **THEN** the row for `src/b.ts:9` comes before the row for `src/a.ts:1`

#### Scenario: Text order within a severity

- **GIVEN** Warning findings in `src/b.ts` on line 5 and in `src/a.ts` on line 40, and no other finding
- **WHEN** the Markdown report is rendered
- **THEN** the row for `src/a.ts:40` comes first, as in the text format

#### Scenario: Other formats keep their order

- **GIVEN** a Warning finding in `src/a.ts` on line 1 and an Error finding in `src/b.ts` on line 9
- **WHEN** the user runs `qgate check --base main` with `--format text`, `json`, `azure` or `github`
- **THEN** the finding for `src/a.ts` comes first, as before this change

### Requirement: Markdown row limit

The table SHALL hold at most 50 rows. With more findings it SHALL hold
the first 50 in the table's order, so an Error is left out only when
50 or more Errors precede it. A line after the table SHALL then say
how many findings are not shown. The marker line, the summary sentence
and the rule summary SHALL still count every finding.

#### Scenario: Sixty findings

- **GIVEN** inputs that produce 60 findings
- **WHEN** the Markdown report is rendered
- **THEN** the table has 50 rows and a line after it says that 10 more findings are not shown
- **AND** the marker line has `count=60`

#### Scenario: Errors after many Warnings in path order

- **GIVEN** 60 Warning findings in `src/a00.ts` to `src/a59.ts`, and 2 Error findings in `src/z.ts`
- **WHEN** the Markdown report is rendered
- **THEN** the first two rows are the Errors in `src/z.ts`, followed by 48 Warnings
- **AND** a line after the table says that 12 more findings are not shown

## ADDED Requirements

### Requirement: Markdown rule summary

When there are more than 50 findings, the Markdown report SHALL list,
after the summary sentence and before the table, one line per rule that
has findings, counting all its findings per severity, most severe
first. The lines SHALL be ordered by Error, then Warning, then Info
count, each descending, then by rule id. With 50 findings or fewer, the
report SHALL have no rule summary.

#### Scenario: Hidden findings of two rules

- **GIVEN** 58 Warning findings of `duplication/duplicate-block`, and 1 Error and 3 Warning findings of `complexity/cyclomatic-increase`
- **WHEN** the Markdown report is rendered
- **THEN** the rule summary is ``- `complexity/cyclomatic-increase`: 1 error, 3 warnings`` followed by ``- `duplication/duplicate-block`: 58 warnings``

#### Scenario: Every finding fits

- **GIVEN** inputs that produce 50 findings
- **WHEN** the Markdown report is rendered
- **THEN** the report has no rule summary and no line about findings not shown

#### Scenario: Singular and missing severities

- **GIVEN** more than 50 findings, of which `tests/missing-test` has exactly one, a Warning
- **WHEN** the Markdown report is rendered
- **THEN** its line is ``- `tests/missing-test`: 1 warning``, with no count for Errors or Infos
