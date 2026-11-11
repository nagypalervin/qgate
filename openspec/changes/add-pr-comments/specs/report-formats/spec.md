# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Exit codes and errors with JSON`
- TO: `### Requirement: Exit codes and errors in every format`

## MODIFIED Requirements

### Requirement: Format option

`qgate check` SHALL take `--format <format>` in every form (two files,
revisions, working tree). `text`, the default, SHALL keep the output the
`check-command` capability specifies. `json` SHALL write the JSON report
instead, `markdown` the Markdown report and `azure` the Azure Pipelines
annotations.

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

## ADDED Requirements

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
without a line. A finding's related locations SHALL be listed in its
Finding cell after the message.

#### Scenario: A message with table and HTML characters

- **GIVEN** a finding whose message is `a | b <script> *c*`
- **WHEN** the Markdown report is rendered
- **THEN** its row still has four cells, and the Finding cell renders as the text `a | b <script> *c*`

#### Scenario: A duplicate with its copies

- **GIVEN** a duplicate-block finding with two related locations
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell holds the message followed by both related locations as `<path>:<line>`

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
