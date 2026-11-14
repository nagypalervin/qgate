# Proposal: add-hotspot-context

## Why

The second half of M5, after 0.7.0's tests analyzer: change risk from
git history, after Adam Tornhill's *Your Code as a Crime Scene*. A
hotspot is code that is both complex and changed often; a pull request
that makes such code worse deserves the reviewer's attention first. The
partner pilot (Angular and .NET) should show this half too. It is
release 0.8.0 in the agreed order.

A hotspot is a state of the code base, not a change, and qgate reports
deltas (design principle 2, CLAUDE.md: "Do not report absolute
scores"). The explore session on 2026-11-14 therefore settled the
shape: history gives **context to findings qgate already reports**,
and is never a finding of its own.

## What Changes

- **Hotspot context on findings.** In `--base` and `--worktree` mode,
  so also in the Claude Code hook and the MCP server, every finding in
  a hotspot file gets a note at the end of its message:

  ```
  warning src/Cart.cs:12  Cart.Total: cyclomatic complexity 4 -> 6 (+2); hotspot: 31 commits in 6 months, complexity 87  [complexity/cyclomatic-increase]
  ```

  The exception is `input/unparsable`. The JSON report gets a
  `hotspot` object on such a finding, with the commit count and the
  file's complexity.
- **Annotation only.** Severity, `--fail-on` and exit codes do not
  change. A hotspot that nothing made worse is not reported.
- **What a hotspot is.** The definition is per file and per language
  family, measured at the head revision. A file is a hotspot when both
  of these hold:
  - **changed often:** it is among the 10% of the family's files
    changed by the most commits in the 6 months before the head commit,
    with at least 5 such commits;
  - **complex:** it is among the 25% of the family's files with the
    highest complexity, the sum of its units' cyclomatic complexity.
- **Shallow history.** A shallow repository gets no hotspot context and
  one line on standard error, so missing history never passes for calm
  code. The check runs as before.
- **README:** a "Hotspots" note in the analyzer sections and the
  `fetch-depth`/`fetchDepth` requirement it relies on. The M5 roadmap
  row is done when this ships.
- **Version 0.8.0,** including `action.yml`.

### Decided in the explore session (2026-11-14)

- Hotspots add context to findings that are already reported (option
  B). They are not a finding of their own (A), and there is no
  separate command (C).
- Annotation only in 0.8.0. Raising severity in hotspots is left for
  later; the evaluation measures what it would do.
- A hotspot is both complex and changed often (Tornhill), with relative
  thresholds, at file level.
- No `qgate hotspots` command: a ranked list for the whole repository
  is an absolute report. If the pilot needs one, an ADR decides first.

### Out of scope

- A list or map of hotspots, and any other report without a change.
- Function-level hotspots (Tornhill's "X-ray").
- Raising severity in hotspots.
- Coupling between files that change together.
- Authors and ownership (knowledge maps).
- `[hotspots]` settings in `qgate.toml` (see Gate 1, question 6).

### For Gate 1

Each question has a recommendation; design.md has the reasons.

1. **The window.** Options:
   - (a) the 6 months (182 days) before the head commit's committer
     date; with `--worktree`, before `HEAD`'s;
   - (b) a number of commits;
   - (c) the 6 months before the wall clock.

   *Recommended: (a).* (c) would give the same pull request another
   result tomorrow, against design principle 1. (b) makes busy and
   quiet repositories incomparable.
2. **Counting churn.**
   - Non-merge commits reachable from the head that change the file,
     following renames, read with one `git log` process.
   - A file renamed in the window keeps its count under its head path.

   *Recommended: yes.* Merge commits would count every change twice on
   merge-heavy branches.
3. **Who is ranked.**
   - The files are those of the head revision that duplication reads:
     supported, not excluded by `[files] exclude`, and not generated.
   - Each language family (TypeScript, C#) is ranked on its own.

   *Recommended: yes.* Excluded migrations and generated files would
   otherwise crowd the top, and a C# solution's numbers say nothing
   about its Angular client.
4. **Which findings get context:** every finding located in a hotspot
   file, except `input/unparsable`. That covers complexity,
   duplication, architecture and tests. *Recommended: yes.* Whatever
   got worse in a hotspot is where the reviewer should look first.
5. **Shallow history:** no context, and one line on standard error
   from `qgate check` (`qgate: the history is shallow; hotspots not
   computed`). The hook and the MCP server add nothing, since they call
   the check in-process.
   *Recommended: yes.* The alternatives are worse:
   - partial history undercounts silently;
   - failing the check would break every pipeline without
     `fetchDepth: 0`.
6. **No settings in 0.8.0.** The window and the thresholds are fixed,
   and the evaluation tunes them before the release. *Recommended:
   yes.* ADR 0003 closes `qgate.toml`'s vocabulary, so a key is cheap to
   add later and costly to take back.
7. **Output.**
   - Every format shows the note at the end of the message (the
     schema says not to parse `message`).
   - JSON also gets a structured `hotspot` key, which is allowed within
     `schema_version` 1.
   - `qgate-core`'s `Finding` gets the field. That is a mechanical edit
     of about 30 places that build findings.

   *Recommended: yes.*
8. **Evaluation before the release.**
   - **Repositories:** the four of 0.7.0, cloned with at least 12 months
     of history.
   - **Record:**
     - how many files are hotspots per family;
     - for every annotated finding, whether it sits in code a reviewer
       would call fragile;
     - how many findings severity raising (option 2) would have turned
       into Errors;
     - the extra time per commit.
   - **Thresholds:** the evaluation may adjust them before the release,
     and you decide.

   *Recommended: yes.*
9. **The rehearsal: Azure Pipelines,** with the README's Linux job at
   0.8.0. The pull request makes a function more complex in a file that
   is a hotspot on a branch with enough commits, so it never touches the
   rehearsal `main`, as in 0.7.0. Expected: the Warning row and the
   build issue carry the hotspot note. *Recommended: yes.*
10. **Three pull requests, with 0.8.0 between the second and third.**
    - PR 1: history in `qgate-git`, and the `hotspot` field in
      `qgate-core`. Nothing visible changes.
    - PR 2: the ranking, the CLI, the formats, the README, the version
      and the evaluation.
    - PR 3: the README versions, the roadmap, the rehearsal, the
      journal and the archive.

    *Recommended: yes.*

**Decided at Gate 1 (2026-11-14):** Ervin approved the proposal with
every recommendation, and asked for the apply to start, which also
approves the design (Gate 2).

## Capabilities

### New Capabilities

- `hotspot-analysis`: the window, churn, complexity, the ranking, which
  findings get the note, its text, shallow history.

### Modified Capabilities

- `report-formats`: a finding may carry a `hotspot` key in the JSON
  report ("Finding fields").

## Impact

- **Code:**
  - `crates/qgate-git`: reading the window's history (commit count per
    head path, renames followed) and whether the repository is shallow;
  - `crates/qgate-core`: an optional `hotspot` on `Finding`;
  - `crates/qgate-analyzers`: the ranking, a pure function of churn and
    complexity per path;
  - `crates/qgate-report`: the JSON key;
  - `crates/qgate-cli`: wiring, using the head models duplication
    already parses, and end-to-end tests with dated commits.
- **No new dependency, no new ADR:** git stays a subprocess (ADR 0002).
- **Docs:** the README and `docs/schema/report-v1.json`.
