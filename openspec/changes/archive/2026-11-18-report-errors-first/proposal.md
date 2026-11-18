# Proposal: report-errors-first

## Why

The Markdown report, which is the pull request comment and the job
summary, shows at most 50 findings in path order. On a large pull
request, the first 50 paths are often test files with duplication
Warnings, and most Errors fall below the cut. The reader sees Warnings
and a line saying how many more findings are not shown. Nothing tells
them that Errors are among the hidden ones, or which rules the hidden
findings come from.

The pilot's week-1 measurement found this on large pull requests (more
than a thousand findings each): most of the Errors were not in the
table. A comment-only pilot is only worth starting when its comment
shows the findings that matter first.

## What Changes

- **The Markdown table lists findings by severity:** Errors first, then
  Warnings, then Info. Within a severity, the order stays the text
  format's order (path, line, rule). The 50-row limit then cuts
  Warnings before Errors.
- **A per-rule summary when not every finding fits.** When there are
  more than 50 findings, a short list after the summary sentence counts
  the findings of each rule by severity, so the reader sees what the
  hidden rows hold.
- **Unchanged:**
  - the marker line, which scripts read;
  - the summary sentence;
  - the text, JSON, Azure and GitHub annotation formats, and their order;
  - a report with 50 findings or fewer: it differs only in row order.

### Out of scope

- **Fewer findings on large pull requests.** Why test code drives
  duplication, and how nested callbacks count toward complexity, are
  separate changes, already queued.
- **Other orderings,** such as hotspots first or grouping by file. The
  severity order is enough for the cut to stop hiding Errors.
- **A different row limit.**

### For Gate 1

Approved by Ervin on 2026-11-18 with all four recommendations.

1. **Order by severity, then the text order** (design D1).
   *Recommended: yes.*
2. **The per-rule summary only when rows are hidden** (D2). A report
   that shows every finding stays as it is, apart from row order.
   *Recommended: yes.*
3. **The summary's form** (D2): one bullet per rule, as
   ``- `<rule>`: 25 errors, 132 warnings``, with rules ordered by their
   most severe findings. *Recommended: yes.*
4. **One pull request, then a release.** The pilot's pipeline installs
   a release, so this change is worth a 0.11.0 on its own. Publishing
   stays Ervin's. *Recommended: yes.*

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `report-formats`: the Markdown report's row order and row limit, and a
  new per-rule summary when findings are hidden.

## Impact

- **Code:** `crates/qgate-report/src/markdown.rs` only. The text order
  that `qgate-cli` sorts by stays, and the Markdown renderer reorders a
  copy by severity.
- **Users:** the Azure pull request comment, and the GitHub comment and
  job summary: everything that shows the Markdown report.
  The CI scripts read only the marker line, which does not change.
- **Docs:** the README's "What the pull request shows" mentions the order.
- **No dependency.**
