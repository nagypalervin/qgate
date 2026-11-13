# report-formats Specification

## Purpose

Defines the formats `qgate check` can write its findings in for programs
to read, such as AI coding agents, hooks, the MCP server and CI scripts,
and the `--format` option that selects one.

## Requirements

### Requirement: Format option

`qgate check` SHALL take `--format <format>` in every form (two files,
revisions, working tree). `text`, the default, SHALL keep the output the
`check-command` capability specifies. `json` SHALL write the JSON report
instead, `markdown` the Markdown report, `azure` the Azure Pipelines
annotations and `github` the GitHub Actions annotations.

#### Scenario: Default is text

- **GIVEN** `f` in `src/a.ts` went from 2 to 4 between `main` and `HEAD`
- **WHEN** the user runs `qgate check --base main --format text`
- **THEN** standard output is the same as for `qgate check --base main`

#### Scenario: JSON in every form

- **WHEN** the user runs `qgate check old.ts new.ts --format json`, `qgate check --base main --format json` or `qgate check --base main --worktree --format json`
- **THEN** standard output is a JSON report

#### Scenario: Markdown and Azure in every form

- **WHEN** the user runs any of the three forms with `--format markdown` or with `--format azure`
- **THEN** standard output is the Markdown report or the Azure Pipelines annotations for the same findings

#### Scenario: GitHub in every form

- **WHEN** the user runs any of the three forms with `--format github`
- **THEN** standard output is the GitHub Actions annotations for the same findings

### Requirement: JSON report document

With `--format json`, standard output SHALL be exactly one JSON object in
UTF-8 on a single line, followed by a newline. It SHALL have the keys
`schema_version` (the number `1`), `findings` (an array) and `summary`
(an object), in this order. The same inputs SHALL give byte-identical
output.

#### Scenario: One finding

- **GIVEN** `f` in `src/a.ts` went from 2 to 4 between `main` and `HEAD`, and nothing else changed
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** standard output is `{"schema_version":1,"findings":[{"rule":"complexity/cyclomatic-increase","severity":"warning","message":"f: cyclomatic complexity 2 -> 4 (+2)","path":"src/a.ts","line":1,"metric":{"before":2,"after":4}}],"summary":{"count":1,"failing":0,"fail_on":"error"}}` followed by a newline
- **AND** the exit code is 0

#### Scenario: No findings

- **GIVEN** the change makes nothing worse
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** standard output is `{"schema_version":1,"findings":[],"summary":{"count":0,"failing":0,"fail_on":"error"}}` followed by a newline

### Requirement: Finding fields

Each finding SHALL have `rule`, `severity` (`error`, `warning` or
`info`), `message` and `path`, as the text format shows them, and when it
has them: `line`; `end_line`, the last line of a range (may equal `line`);
`metric`, with `before` (absent for a new item) and `after` (absent for a
removed one); `related`, a non-empty array of objects with `path`, `line`
and, for a range, `end_line`. A key without a value SHALL be left out,
never `null`.

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

Metric values and line numbers SHALL be JSON numbers. A whole number below
2^53 in magnitude SHALL be written without a fraction or exponent (`4`,
not `4.0`). A `metric` with a value that is not finite, or with neither
`before` nor `after`, SHALL be left out as a whole, so an absent `before`
or `after` always means a new or removed item.

#### Scenario: Complexity values

- **WHEN** a finding's complexity goes from 2 to 4
- **THEN** its metric is written as `{"before":2,"after":4}`

#### Scenario: Values that cannot be written

- **GIVEN** a finding whose metric has an `after` value that is not finite and no `before` value
- **WHEN** the report is written as JSON
- **THEN** the finding has no `metric` key, and no `null` appears

### Requirement: Order of findings in the JSON report

The `findings` array SHALL hold the findings in the order the text format
prints them (see "Deterministic output order" in `check-command`).

#### Scenario: Several files

- **GIVEN** findings in `src/b.ts` line 5 and `src/a.ts` line 40
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** the finding for `src/a.ts` comes first in `findings`

### Requirement: JSON summary

`summary` SHALL hold `count` (the number of findings), `failing` (how
many of them are at or above the `--fail-on` level) and `fail_on` (that
level as written on the command line, `error` by default).

#### Scenario: Failing run

- **GIVEN** inputs that produce two Error findings and one Warning finding
- **WHEN** the user runs `qgate check --base main --format json --fail-on warning`
- **THEN** `summary` is `{"count":3,"failing":3,"fail_on":"warning"}`
- **AND** the exit code is 1

#### Scenario: Never failing

- **GIVEN** inputs that produce one Error finding
- **WHEN** the user runs `qgate check --base main --format json --fail-on never`
- **THEN** `summary` is `{"count":1,"failing":0,"fail_on":"never"}`
- **AND** the exit code is 0

### Requirement: Exit codes and errors in every format

The exit codes and the summary line on standard error SHALL be the same
in every format. When the check cannot run, standard output SHALL stay
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

#### Scenario: The same exit code in Markdown and Azure

- **GIVEN** inputs that produce one Error finding
- **WHEN** the user runs `qgate check --base main` with `--format markdown`, and again with `--format azure`
- **THEN** both exit with code 1 and end standard error with the same summary line as the text format

#### Scenario: Markdown never failing

- **GIVEN** inputs that produce one Error finding
- **WHEN** the user runs `qgate check --base main --format markdown --fail-on never`
- **THEN** standard output is the Markdown report with that finding
- **AND** the exit code is 0

### Requirement: Versioned JSON format

Within `schema_version` 1, a later qgate MAY add keys and new `rule` ids,
but SHALL NOT remove or rename a key, change its type or meaning, or add a
`severity` value; that needs a new `schema_version`. The repository SHALL
hold a JSON Schema for version 1 that every report validates against.

#### Scenario: A rule id the schema does not list

- **GIVEN** the JSON Schema for version 1
- **WHEN** a report holds a finding whose `rule` is an id that no analyzer reports today, written as `<analyzer>/<rule>` in lowercase
- **THEN** the report is valid against it

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

### Requirement: Markdown report document

With `--format markdown`, standard output SHALL be one UTF-8 Markdown
document ending in a newline:
- the marker line;
- a blank line;
- the heading `### qgate`;
- a summary sentence;
- if there are findings, a table with the columns Severity, Location,
  Finding and Rule, one row per finding, in the text format's order.

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

### Requirement: Markdown summary sentence

The summary sentence SHALL say how many findings there are and how many
of them are at or above the `--fail-on` level, naming that level as
written on the command line (`error` by default). With no findings it
SHALL say that no regressions were found.

#### Scenario: Findings below the level

- **GIVEN** inputs that produce two Warning findings and no Error
- **WHEN** the user runs `qgate check --base main --format markdown`
- **THEN** the summary sentence says there are 2 findings and none at or above `error`

#### Scenario: A finding at the level

- **GIVEN** inputs that produce one Error and one Warning
- **WHEN** the user runs `qgate check --base main --format markdown --fail-on warning`
- **THEN** the summary sentence says there are 2 findings and 2 at or above `warning`

### Requirement: Markdown marker line

The first line SHALL be an HTML comment that renderers hide and scripts
can read: `<!-- qgate count=<n> failing=<m> fail_on=<level> -->`. Here
`<n>` is the number of findings, `<m>` the number at or above the level
and `<level>` the `--fail-on` level. Within this capability, a later
qgate SHALL NOT change this line's form.

#### Scenario: Reading the counts

- **GIVEN** inputs that produce one Error and one Warning
- **WHEN** the user runs `qgate check --base main --format markdown`
- **THEN** the first line is `<!-- qgate count=2 failing=1 fail_on=error -->`

### Requirement: Markdown table cells

Each table cell SHALL render as the plain text it holds, whatever the
finding's path, message or rule contains:
- a character that is Markdown syntax, a table delimiter or HTML SHALL
  be escaped, so it neither starts markup nor ends the cell;
- a line break SHALL become a space.

The Location cell SHALL be `<path>:<line>`, or `<path>` for a finding
without a line.

#### Scenario: A message with table and HTML characters

- **GIVEN** a finding whose message is `a | b <script> *c*`
- **WHEN** the Markdown report is rendered
- **THEN** its row still has four cells, and the Finding cell renders as the text `a | b <script> *c*`

### Requirement: Markdown related locations

The Finding cell SHALL list a finding's related locations after the
message, as `<path>:<line>`, or `<path>` for one without a line. It
SHALL leave out a location whose `<path>:<line>` the message already
names: at the start of the message or after whitespace, and not
followed by an ASCII digit.

#### Scenario: A duplicate with its copies

- **GIVEN** a finding with two related locations, neither of which its message names
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell holds the message followed by both related locations as `<path>:<line>`

#### Scenario: A copy the message names

- **GIVEN** a duplicate-block finding whose message is `duplicates src/grade.ts:1-19 (lines 1-19, 85 tokens)` and whose only related location is `src/grade.ts` line 1
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell holds the message alone, without "also at"

#### Scenario: A similar location the message does not name

- **GIVEN** a finding whose message names `src/a.ts:10-19` and whose related locations are `src/a.ts` line 1 and `src/a.ts` line 10
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell lists `src/a.ts:1` after the message, and not `src/a.ts:10`

#### Scenario: A path the message names only as part of a longer path

- **GIVEN** a finding whose message names `lib/src/a.ts:1-6` and whose related locations are `lib/src/a.ts` line 1 and `src/a.ts` line 1
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell lists `src/a.ts:1` after the message, and not `lib/src/a.ts:1`

### Requirement: Markdown row limit

The table SHALL hold at most 50 rows. With more findings it SHALL hold
the first 50 in the text format's order. A line after the table SHALL
then say how many findings are not shown. The marker line and the
summary sentence SHALL still count every finding.

#### Scenario: Sixty findings

- **GIVEN** inputs that produce 60 findings
- **WHEN** the Markdown report is rendered
- **THEN** the table has 50 rows and a line after it says that 10 more findings are not shown
- **AND** the marker line has `count=60`

### Requirement: Azure Pipelines annotations

With `--format azure`, standard output SHALL hold one line per Error or
Warning finding, in the text format's order, of the form
`##vso[task.logissue type=<error|warning>;sourcepath=<path>;linenumber=<line>;code=<rule>]<message>`.
`linenumber` SHALL be left out for a finding without a line. Info
findings SHALL be left out. With no Error or Warning finding, standard
output SHALL be `No regressions found.` and a newline.

#### Scenario: One error

- **GIVEN** inputs that produce one Error finding in `src/a.ts` on line 1, with the rule `complexity/cyclomatic-increase` and the message `f: cyclomatic complexity 2 -> 8 (+6)`
- **WHEN** the user runs `qgate check --base main --format azure`
- **THEN** standard output is `##vso[task.logissue type=error;sourcepath=src/a.ts;linenumber=1;code=complexity/cyclomatic-increase]f: cyclomatic complexity 2 -> 8 (+6)` and a newline

#### Scenario: No findings

- **GIVEN** inputs that produce no findings
- **WHEN** the user runs `qgate check --base main --format azure`
- **THEN** standard output is `No regressions found.` and a newline

### Requirement: Escaping in Azure Pipelines annotations

Every value SHALL be escaped so that Azure Pipelines reads back exactly
the finding's text:
- in the property values (`sourcepath`, `code`): `%` as `%AZP25`, `;` as
  `%3B`, `]` as `%5D`, carriage return as `%0D` and line feed as `%0A`;
- in the message: `%`, carriage return and line feed in the same way.

#### Scenario: A path with a semicolon

- **GIVEN** a finding in the file `src/a;b.ts`
- **WHEN** the annotations are rendered
- **THEN** its line has `sourcepath=src/a%3Bb.ts`

#### Scenario: A message with a percent sign and a line break

- **GIVEN** a finding whose message is `100% done` followed by a line feed and `next`
- **WHEN** the annotations are rendered
- **THEN** the message part of its line is `100%AZP25 done%0Anext`

### Requirement: GitHub Actions annotations

With `--format github`, standard output SHALL hold one line per Error or
Warning finding, in the text format's order, of the form
`::<error|warning> file=<path>,line=<line>,title=<rule>::<message>`.
`line` SHALL be left out for a finding without a line. Info findings
SHALL be left out. With no Error or Warning finding, standard output
SHALL be `No regressions found.` and a newline.

#### Scenario: One error

- **GIVEN** inputs that produce one Error finding in `src/a.ts` on line 1, with the rule `complexity/cyclomatic-increase` and the message `f: cyclomatic complexity 2 -> 8 (+6)`
- **WHEN** the user runs `qgate check --base main --format github`
- **THEN** standard output is `::error file=src/a.ts,line=1,title=complexity/cyclomatic-increase::f: cyclomatic complexity 2 -> 8 (+6)` and a newline

#### Scenario: A warning without a line

- **GIVEN** inputs that produce one Warning finding in `src/b.ts` without a line, with the rule `architecture/forbidden-import`
- **WHEN** the user runs `qgate check --base main --format github`
- **THEN** its line starts with `::warning file=src/b.ts,title=architecture/forbidden-import::`

#### Scenario: Info findings left out

- **GIVEN** inputs that produce one Info finding and nothing else
- **WHEN** the user runs `qgate check --base main --format github`
- **THEN** standard output is `No regressions found.` and a newline

### Requirement: Escaping in GitHub Actions annotations

Every value SHALL be escaped so that GitHub Actions reads back exactly
the finding's text:
- in the message: `%` as `%25`, carriage return as `%0D` and line feed
  as `%0A`;
- in the property values (`file`, `title`): the same, and `:` as `%3A`
  and `,` as `%2C`.

#### Scenario: A path with a comma

- **GIVEN** a finding in the file `src/a,b.ts`
- **WHEN** the annotations are rendered
- **THEN** its line has `file=src/a%2Cb.ts`

#### Scenario: A message with a percent sign and a line break

- **GIVEN** a finding whose message is `100% done` followed by a line feed and `next`
- **WHEN** the annotations are rendered
- **THEN** the message part of its line is `100%25 done%0Anext`
