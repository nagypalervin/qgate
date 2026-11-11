# Design: add-pr-comments

## Context

See `proposal.md` for the why and the spike results.

- **`qgate-report`** has two renderers:
  - `render_text(findings)`;
  - `render_json(findings, ReportSummary { failing, fail_on })`.

  It depends on `qgate-core`, `serde` and `serde_json`.
- **`qgate-cli`** maps `--format` to an internal `Format` enum (`Text`,
  `Json`) with `name()` and `render(findings, fail_on)`. The exit code
  and the stderr summary line do not depend on the format.
- **No analyzer emits `Info` today.** The JSON schema allows it.
- **The text format does not print related locations; JSON does.**
- **README "In Azure Pipelines"** (`add-azure-pipelines`) installs
  0.1.0. Its job has three steps:
  - checkout with `fetchDepth: 0`;
  - install;
  - `qgate check --base origin/<target>`.

  The branch policy is required.
- **The rehearsal project `qgate-rehearsal`:**
  - its Build Service now has "Contribute to pull requests" on the
    repository;
  - the spike's pull request 1349 is still open.
- **This machine has no `pwsh` yet.** Ervin agreed to a local install (D5).

## Goals / Non-Goals

**Goals:**
- **One look at the pull request tells why the gate closed.** The
  comment holds the findings, and the build summary links each one to
  its file and line.
- **qgate stays offline and deterministic.** The only code that holds
  a token is the comment script, inside the pipeline.
- **The script is tested twice before it is released.** Unit tests run
  in CI on both operating systems, and the script also runs against a
  real pull request.

**Non-Goals:**
- Comments on diff lines, GitHub, SARIF (proposal, out of scope).
- Showing every finding when there are hundreds. The comment caps at
  50 rows and the build log has the rest.

## Decisions

### D1. Two renderers in `qgate-report`

- `render_markdown(findings, summary: ReportSummary) -> String`
- `render_azure(findings) -> String`

They sit next to the existing renderers, with no new dependencies.
`Format` in `qgate-cli` gains `Markdown` ("markdown") and `Azure`
("azure"):
- `Markdown` gets the same `ReportSummary` as `Json`;
- `Azure` needs no summary. Its exit code is the CLI's, as for every
  format.

The format is called `azure`, not `azure-pipelines`. It is short to
type and cannot be confused with anything else in qgate. Format names
are public, so the name is fixed now.

### D2. The Markdown document

```markdown
<!-- qgate count=2 failing=1 fail_on=error -->

### qgate

2 findings, 1 at or above `error`.

| Severity | Location | Finding | Rule |
|---|---|---|---|
| error | src/grade.ts:1 | grade: cyclomatic complexity 2 -\> 8 (+6) | complexity/cyclomatic-increase |
| warning | src/b.ts:4 | 12 duplicated lines; also at src/a.ts:10 | duplication/duplicate-block |
```

**The marker line** comes first, in the fixed form the spec gives.
Renderers hide HTML comments. The Azure script reads the counts from
it. Other scripts can do the same later; on GitHub it doubles as the
comment's marker.

**The summary sentence:**
- with no findings: `No regressions found.`, and no table;
- otherwise: `<n> finding(s), <m> at or above` followed by the level in
  backticks, with `none` for `m = 0`.

The level is always one of `error`, `warning` or `never`, so the
backticks need no escaping.

**Cells.** Every cell is plain text, escaped:
- **Backslash escapes** for ``\ ` * _ [ ] < > | ~ &``. These are the
  ASCII punctuation characters that can start inline markup, an
  entity, HTML or a cell boundary in GFM tables.
- **CR and LF** become a space.
- **No code spans.** A path or message with a backtick would break one,
  and escaping is the same everywhere.

The raw text is less tidy, for example `2 -\> 8`. What matters is
the rendered text.

**Related locations** follow the message, as `; also at <path>:<line>,
<path>:<line>`. JSON already carries them, and the text format does not
show them.

**The row limit is 50.** A longer report adds a line after the table:
`<k> more finding(s) not shown.` 50 rows of typical findings stay far
below GitHub's 65,536-character comment limit and below Azure DevOps'
limit.

### D3. The Azure annotations

There is one line per Error or Warning finding, in text-format order:

```
##vso[task.logissue type=error;sourcepath=src/grade.ts;linenumber=1;code=complexity/cyclomatic-increase]grade: cyclomatic complexity 2 -> 8 (+6)
```

- **Escaping follows the spec, as the spike measured:**
  - in properties: `%`→`%AZP25`, `;`→`%3B`, `]`→`%5D`, CR→`%0D`,
    LF→`%0A`;
  - in the message: `%`, CR and LF.
- **Info findings are left out.** `logissue` knows only `error` and
  `warning`, and Info never fails a check.
- **With nothing to annotate, the output is `No regressions found.`**
  The log then says what happened instead of being empty.
- **Related locations are not annotated.** An issue has one source; the
  comment lists the copies.

### D4. The comment script

The script is `ci/azure/qgate-pr-comment.ps1`, for PowerShell 7.4 or
later. It is called as:

```
pwsh -File qgate-pr-comment.ps1 -Report comment.md
```

**Inputs** are predefined pipeline variables, read from the
environment:
- `SYSTEM_COLLECTIONURI`, `SYSTEM_TEAMPROJECT`, `BUILD_REPOSITORY_ID`
  and `SYSTEM_PULLREQUEST_PULLREQUESTID`;
- `SYSTEM_ACCESSTOKEN`, which the step must map explicitly.

A missing variable stops the script with a message that names it. For
the token, the message gives the `env:` line to add.

**Flow:**

```
read report -> parse marker line (else: "not a qgate Markdown report")
            -> GET threads -> mine = threads with property qgate.report
   count > 0 : mine ? PATCH comment + PATCH status active : POST thread (active, property)
   count = 0 : mine ? PATCH comment ("No regressions found.") + PATCH status closed : nothing
```

- **Body.** The comment body is the report without its marker line.
  Azure DevOps does not need the marker, because the thread property
  identifies the thread. Without the marker, the body does not depend on
  how Azure renders HTML comments.
- **Property.** The thread property is `qgate.report`, and its value
  is the string `1`, a version for the thread layout.
- **More than one thread.** If two builds raced and two qgate threads
  exist, the script updates the one with the lowest id and logs a
  warning naming the others. It does not close or delete them.
- **Comment id.** The comment to update is the thread's first comment,
  `comments[0].id`. The script does not assume the id is 1.
- **Errors.**
  - **403 with `TF401027`:** "The project's Build Service needs the
    'Contribute to pull requests' permission on this repository
    (Project settings → Repositories → <repo> → Security)." The script
    then exits 1.
  - **401:** a hint that the token was not passed or has expired.
  - **Any other error:** the method, URL, status and response body.

  The script only creates a thread after it has listed the existing
  ones, so a failure leaves at most the old state, never half a comment.
- **Settings:** `Set-StrictMode -Version Latest`,
  `$ErrorActionPreference = "Stop"` and
  `$PSNativeCommandUseErrorActionPreference = $true`. All URLs are built
  with `${…}`, for the spike's `?` trap.
- **Structure for tests.** The logic lives in functions. The main block
  runs only when the script is invoked, not dot-sourced, so Pester can
  load the functions and mock `Invoke-RestMethod`.

### D5. Testing the script

- **Pester tests** live in `ci/azure/qgate-pr-comment.Tests.ps1`.
  - They require Pester 5.5 or later (`#Requires`). The GitHub-hosted
    runners have it preinstalled.
  - `Invoke-RestMethod` is mocked.
  - They cover:
    - each branch of the lifecycle;
    - more than one thread;
    - each error mapping;
    - a missing environment variable;
    - a report without the marker line;
    - the URL form (no `?` trap).
- **A CI job `pester (<os>)`** runs the tests on `ubuntu-latest` and
  `windows-latest`. Both checks become required in the "Protect main"
  ruleset. That is a repository setting, so it needs Ervin's approval,
  as in `add-release-binaries`.
- **A real-API check before the release** (task 1.8). The script from
  the pull request branch is copied into a rehearsal branch, together
  with hand-written reports, and runs in the rehearsal pipeline against
  a real pull request:
  - create, update, close;
  - no thread on a clean report;
  - the error message after Ervin removes the permission for one run.
- **Locally (Ervin agreed).** PowerShell 7 is installed from its
  official GitHub release archive into
  `~/.local/opt/powershell`, checked against the release's published
  SHA-256 and linked from `~/.local/bin/pwsh`. No `sudo` and no install
  script are needed.

### D6. Release packaging and version

- **`release.yml`'s Pack step** copies `ci/azure/qgate-pr-comment.ps1`
  into every archive, next to the binary.
- **`release.yml` also triggers on `ci/azure/**`,** so a script change
  rehearses the packaging too.
- **The version becomes 0.2.0:** the workspace version and the internal
  dependencies, as in D5 of `add-release-binaries`. The version bump
  also triggers the release rehearsal on PR 1; its archives must contain
  the script.
- **After PR 1 merges,** Ervin pushes `v0.2.0` and publishes the draft
  after it has been checked. That check covers:
  - the script is in the archives;
  - `qgate --format markdown` and `--format azure` work in the Linux
    binary.

### D7. The README job with 0.2.0 (PR 2)

```
install (sets QGATE_DIR) -> markdown report -> comment -> gate with annotations
```

1. **Install.** As today, plus
   `##vso[task.setvariable variable=QGATE_DIR]` with the unpacked
   directory.
2. **Markdown report:**
   `qgate check --base … --format markdown --fail-on never > "$(Agent.TempDirectory)/qgate.md"`.
   Its exit code is 0 unless the check cannot run.
3. **Comment:** `pwsh -File "$(QGATE_DIR)/qgate-pr-comment.ps1" -Report …`,
   with `SYSTEM_ACCESSTOKEN: $(System.AccessToken)`. On Linux this is a
   `pwsh:` step too.
4. **The gate:** `qgate check --base … --format azure`. It annotates
   the build summary and fails the job at the `--fail-on` level.

**Why the gate comes last:** every step can keep the default condition
(previous steps succeeded). An earlier gate would need
`succeededOrFailed()` on the comment steps, which then also run after a
failed install.

**The cost** is that a missing permission fails the job at step 3,
before the gate runs. The log then names the permission (D4), which is
the right first thing to fix.

**qgate runs twice.** The analysis is deterministic and fast, and one
run with two outputs would need a new concept on the command line.

**The README section also gains:**
- **The permission.** "Contribute to pull requests" for "<project>
  Build Service (<organization>)", as UI steps.
- **One sentence on organizations that limit job authorization scope
  differently.** There the identity is "Project Collection Build
  Service", and the error message names whichever identity the token
  belongs to.
- **What the developer sees:**
  - the build summary issues;
  - one comment that updates itself and closes when the findings are
    gone.

The other install blocks move to 0.2.0.

### D8. The PR 2 rehearsal

The rehearsal runs, as in `add-azure-pipelines`, with jobs extracted
from the README unchanged and compared with `diff`. Per operating
system:

| # | Input | Expected |
|---|---|---|
| 1 | PR with a complexity error, no thread yet | job fails at the gate; build issue with file, line, rule; one active qgate thread listing the finding |
| 2 | push a commit that removes the error | job succeeds; the same thread says "No regressions found." and is closed |
| 3 | push a commit that brings the error back | the same thread is active again with the new report; still one qgate thread |
| 4 | a README-only PR | job succeeds; no qgate thread |

Every push in scenarios 2 and 3 changes the merge result, because the
spike showed that an empty commit queues no build.

**Ervin looks at one comment of scenario 1 in the web UI** and confirms
that the table renders and the marker line is gone.

### D9. Repository layout

`ci/` is a new top-level directory for files the CI integrations ship
(`ci/azure/` now, `ci/github/` later). `CLAUDE.md`'s "Layout" and the
README's "Repository layout" each gain one line for it. This is not a
crate, so the dependency rules and ADRs are unaffected.

### D10. Two pull requests

- **PR 1,** `change/add-pr-comments`:
  - the renderers and the `--format` values;
  - the script and its tests, plus the CI job;
  - the release packaging and 0.2.0;
  - the layout lines;
  - the real-API check of the script.
- **Between the PRs:** Ervin pushes `v0.2.0` and publishes the draft.
- **PR 2,** `change/add-pr-comments-2`:
  - the README;
  - the rehearsal record;
  - the review fixes;
  - the journal entry;
  - the archive.

## Risks / Trade-offs

- **[Azure DevOps' Markdown renderer differs from GitHub's]** Backslash
  escapes and GFM tables are assumed to render. → Ervin checks a real
  comment in PR 2. If an escape shows literally, the escaping set
  changes before the archive.
- **[Two builds race on a new PR]** Both may create a thread. → Rare,
  since policy builds for one PR are queued one after the other. The
  script then keeps one up to date and warns about the rest (D4).
- **[The identity differs per organization]** "<project> Build Service"
  vs "Project Collection Build Service". → The error message names the
  identity the token belongs to; the README mentions both.
- **[A second, non-Rust language in the repository]** It needs its own
  tests, lint and toolchain. → Pester in CI, both operating systems.
  The script is short and has no module dependencies.
- **[Pester on hosted runners changes]** → `#Requires -Modules` with a
  minimum version fails loudly instead of running different semantics.
- **[The gate runs after the comment]** A permission problem hides the
  gate's annotations for that build. → The error message says exactly
  what to fix, and the job still fails, so nothing merges unchecked.
