# Design: report-errors-first

## Context

See proposal.md, "Why". `qgate-cli` sorts findings once, by path, line
and rule (`check-command`, "Deterministic output order"), and passes the
same slice to every renderer. `render_markdown` in
`crates/qgate-report/src/markdown.rs` writes the marker line, the summary
sentence and the first `MAX_ROWS` (50) findings of that slice, then a
line counting the rest. `Severity` derives `Ord` with
`Info < Warning < Error`.

Three things read the Markdown report:
- the Azure comment script;
- the GitHub Action's comment;
- the GitHub job summary.

The scripts parse only the marker line.

## Goals / Non-Goals

**Goals:**

- No Error is hidden while a Warning or Info is shown.
- A reader of a cut table knows what the hidden rows hold.
- Every other format, and the marker line, stay byte-identical.

**Non-Goals:**

- Changing what qgate finds, or how many findings it reports.
- A configurable order or limit.

## Decisions

### D1. Order the table by severity, then the text order

The renderer copies the slice into a vector of references and sorts it
stably by severity, descending. Because the input is already in the text
order, a stable sort keeps that order within each severity.

**Alternatives:**

- **Sort in `qgate-cli` for every format.** The JSON and text orders are
  specified, and the JSON order is a public API (`report-formats`,
  "Order of findings in the JSON report"). Annotations do not need it:
  the CI shows each one on its own line in the file.
- **Show only the failing findings (`failing`, from `--fail-on`).** In
  comment-only mode, `--fail-on never` makes `failing` 0, yet Errors are
  still the findings that matter. Severity does not depend on the gate.
- **Hotspot findings first within a severity.** It is useful, but it is a
  second ordering rule, and the cut problem does not need it (proposal,
  "Out of scope").

### D2. A rule summary only when rows are hidden

When `count > MAX_ROWS`, a bullet list follows the summary sentence:
``- `<rule>`: 2 errors, 15 warnings``. The list is ordered by
(Errors, Warnings, Infos) descending, then by rule id, so the rules that
matter most come first and the output is deterministic. A blank line
separates the list from the sentence and from the table.

A count of one is singular, any other count takes an `s`: `1 error`,
`3 warnings`, `2 infos`. "infos" is not elegant, but it keeps the three
words one pattern, and Info findings are rare in a cut report.

The rule id stays in backticks. Rule ids are lowercase with `/` and `-`,
so they need no escaping inside code. The rule summary is not escaped
like table cells.

**Alternatives:**

- **Always show it.** For a short report the table already shows every
  rule. A list would only repeat it and make every existing comment
  longer.
- **A table** (rule × severity columns). It needs a column per severity,
  mostly empty, and it reads less well in a narrow comment pane.
- **Count only the hidden findings.** The reader then has to add up the
  table and the list to get each rule's total. Counting every finding
  matches the summary sentence and the marker line.

## Risks / Trade-offs

- **[A reader who knew the old order]** → the table order changes for
  every report with both Errors and Warnings. The row content does not,
  and the comment is read, not parsed. The README states the order.
- **[More than 50 Errors]** → Warnings disappear from the table entirely.
  The rule summary still counts them, and those are the pull requests
  where Errors matter most.

## Verification

- **Mutation check (task 1.5),** each mutant by hand and reverted; every
  one fails at least one test of `markdown.rs`:
  - no severity sort;
  - the order reversed within a severity;
  - the rule summary shown for every report with findings;
  - the rule summary counting only the hidden findings;
  - the rule summary ordered by rule id only;
  - the rule summary ordered by ascending counts.
- **Public replay (task 2.1).** The change does not touch analysis, so
  the two orders were applied to the evaluation's 0.10.0 JSON reports of
  the 150 public pull requests. Only one has more than 50 findings, and
  no Error was hidden in either order. Those repositories merge small
  pull requests, so the problem does not show there.
- **Larger pull requests,** measured outside this repository: the old
  order hid Errors behind Warnings in several, and the new order hides
  none. This branch's binary rendered the largest of them: every Error
  in the table, and the Errors' rules first in the rule summary.

## Migration Plan

None. The next release carries it. The CI scripts need no change,
because the marker line stays.
