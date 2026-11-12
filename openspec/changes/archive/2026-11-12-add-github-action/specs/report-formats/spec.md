# Spec Delta

## MODIFIED Requirements

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

## ADDED Requirements

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
