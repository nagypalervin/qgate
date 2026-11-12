# Design: add-github-action

## Context

- `add-pr-comments` gave qgate `--format markdown` (with the marker line
  `<!-- qgate count=<n> failing=<m> fail_on=<level> -->`) and
  `--format azure`, and shipped `ci/azure/qgate-pr-comment.ps1`, a
  PowerShell 7.4 script tested with Pester 5.9.1 in the `pester (...)`
  CI jobs on Linux and Windows. Its design (D4, D5) is the model here.
- Releases are draft GitHub Releases built from a `v<version>` tag; the
  `version` job of `release.yml` checks the tag against the workspace
  version with `.github/scripts/check-version.sh`. Archives are
  `qgate-<version>-<target>.tar.gz` (`.zip` on Windows) with one
  directory inside, plus `SHA256SUMS`.
- README "In GitHub Actions" today: an install `run` step on
  `ubuntu-latest`, then `qgate check --base "origin/$BASE_REF"` in text
  format, then an optional JSON artifact.
- From GitHub's docs (2026-11-12): `issues: write` is the scope named
  for adding a comment; on `pull_request` from a fork, write scopes
  become read-only; the job summary takes at most 1 MiB per step;
  composite actions may use `shell: pwsh`; an action's files are at
  `$GITHUB_ACTION_PATH`. The docs do not state the annotation limits,
  the workflow-command escaping, or the `pwsh` version on each image;
  the spike (D8) settles them.

## Goals / Non-Goals

**Goals:**
- Every step of the action that has logic is in one PowerShell script
  with Pester tests; `action.yml` only wires inputs to it.
- The action works unchanged on the six release targets' runners.
- No text from the pull request reaches the log where the runner would
  read it as a workflow command, except the escaped annotations.

**Non-Goals:**
- A self-test of the action in this repository's CI before a release
  (D9 explains why).
- A `token` input: the comment always comes from `github-actions[bot]`.

## Decisions

### D1. `--format github` in `qgate-report`

A new module `github.rs` beside `azure.rs`, same shape: one line per
Error or Warning finding, `No regressions found.` otherwise, Info left
out. Properties: `file`, `line` (when there is one), `title` (the rule).
Escaping follows `@actions/core`'s `escapeData` and `escapeProperty`:
`%`, CR, LF in the message; additionally `:` and `,` in properties.

- **No `endLine`.** It would mark a duplicate block's whole range, but
  the Azure format has no counterpart, and a long range hides the diff
  under the annotation. Can be added later without breaking anything.
- **Alternative: `::notice` for Info.** Rejected for symmetry with
  `--format azure`; no analyzer reports Info findings today.

`qgate-cli` gets `Format::Github` (`github`); the usage error lists five
values.

### D2. The action: `action.yml` and one script

`action.yml` at the repository root, `runs.using: composite`. Inputs:

| Input | Default | Use |
|---|---|---|
| `base` | `''` | revision to compare with; empty means `origin/<base branch>` on `pull_request` |
| `fail-on` | `error` | passed to both qgate runs |
| `comment` | `'true'` | `'false'` skips everything that touches comments |

Four steps, all `shell: pwsh`, each one line:
`& "$env:GITHUB_ACTION_PATH/ci/github/qgate-action.ps1" <Step>`, with
the inputs, `github.token` and the qgate version in `env:`.

1. `Install`: resolve the base (D5) and the target, download, verify,
   unpack into `$RUNNER_TEMP/qgate-<version>/`. (Apply changed this:
   no `GITHUB_ENV`; see "Decisions during apply".)
2. `Report`: `qgate check --base $base --format markdown --fail-on
   $failOn` into `$RUNNER_TEMP/qgate-<version>/report.md`; exit 0 and 1 are both fine,
   2 fails the step. Append the file to `$GITHUB_STEP_SUMMARY`.
3. `Comment` (`if: inputs.comment == 'true'`): D4.
4. `Gate`: `qgate check --base $base --format github --fail-on
   $failOn`; its exit code is the step's.

**Why one script with a step argument:** each step stays testable by
dot-sourcing the script and mocking two wrappers, `Invoke-Qgate` and
`Invoke-GitHub`, as the Azure tests mock `Invoke-Azure`. Inline YAML
logic would be untested. Separate scripts per step would repeat the
shared helpers (error reporting, the marker parser).

**Why qgate runs twice** (report, then gate): the same as the Azure job.
The annotations must come from the gate's own stdout, and the Markdown
must exist before the comment step, which must run before the gate can
fail the job.

**The script is not in the release archives.** The action reads it from
its own checkout at the tag, which already matches the binaries.

### D3. Install

- Target from `RUNNER_OS` × `RUNNER_ARCH`: `Linux`/`macOS`/`Windows` ×
  `X64`/`ARM64` maps to the six targets; anything else fails with a
  message naming both values.
- Version: `QGATE_VERSION` in `action.yml`'s `env:` for the `Install`
  step, the one place the version is written. The release workflow
  reads it (D7).
- Download the archive and `SHA256SUMS` from the release URL into
  `$RUNNER_TEMP/qgate-<version>/` with `Invoke-WebRequest
  -MaximumRetryCount 3`; compare `Get-FileHash -Algorithm SHA256` with
  the archive's line. A mismatch or missing line fails with the
  archive's name, before unpacking.
- Unpack with `tar` (`.tar.gz`) or `Expand-Archive` (`.zip`).
- `QGATE` is the full path of the binary; `PATH` is left alone, so the
  action does not change what later steps of the job run.

### D4. The comment

- **Pull request:** number and head repository from the event payload
  at `$GITHUB_EVENT_PATH`.
- **Forks: skip with a notice** (`::notice::`), before any API call,
  when the head repository's `full_name` differs from
  `GITHUB_REPOSITORY` or is null (a deleted fork): "the comment is
  skipped for pull requests from forks: GitHub gives them a
  GITHUB_TOKEN that cannot write".
- **Find the comment:** `GET /repos/{repo}/issues/{n}/comments
  ?per_page=100`, following `Link: rel="next"`. The qgate comment is
  the first whose author is `github-actions[bot]` and whose body starts
  with `<!-- qgate `. Checking the author keeps a pasted report from
  being taken over.
- **Decide** from the marker line of the new report (`count`):
  - `count > 0`: `PATCH /repos/{repo}/issues/comments/{id}` if found,
    otherwise `POST .../issues/{n}/comments`;
  - `count = 0`: `PATCH` if found, otherwise nothing.
- **Headers:** `Authorization: Bearer`, `Accept:
  application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`. The
  token comes from `env:` and is never printed.
- **Errors:** an HTTP 403 or 404 on any call becomes one `::error::`
  that names the permission to grant, `pull-requests: write` (the
  spike showed that `issues: write` is not enough), and the step exits
  1, so the gate does not run. Other failures print the status and
  GitHub's `message`.
- **Body size:** the report has at most 50 rows, far below GitHub's
  65,536 characters; no truncation needed.

### D5. Base and event

- `base` input set: use it, on any event.
- Empty, and `GITHUB_EVENT_NAME` is `pull_request`: `origin/$GITHUB_BASE_REF`.
- Empty otherwise: fail in `Install`, before downloading, with "no
  base revision to compare with; run the action on a pull_request
  event, or set its base input".
- The comment step also skips (silently) on events other than
  `pull_request`: there is no pull request to comment on.
- A shallow checkout is not detected: qgate's own error names the
  missing revision, and the README says `fetch-depth: 0`.

### D6. Untrusted text in the log

The runner reads workflow commands from a step's stdout and stderr.
Paths and messages come from the pull request, and qgate's errors (exit
2) can quote revision names or config values. So the script:
- prints the gate's stdout as it is (qgate escapes it, D1);
- prints qgate's stderr from both runs between
  `::stop-commands::<random token>` and `::<token>::`; the report step's
  stdout goes only to the job summary and the comment.

### D7. Version and release check

- Workspace version `0.3.0`; `QGATE_VERSION: 0.3.0` in `action.yml`.
- `release.yml`'s `version` job gains a step that reads
  `QGATE_VERSION` from `action.yml` and fails, naming both versions,
  when it differs from the tag's. Its self-test (like the existing
  `v$VERSION.1` probe) checks the mismatch path.
- A users' pin `@v0.3.0` thus gets qgate 0.3.0, and the script at the
  same commit.

### D8. Spike (task 1), in the private rehearsal repository

One hand-written workflow, no qgate code, run on `ubuntu-latest`,
`windows-latest` and `macos-latest`:
- **Comment permissions:** create and edit an issue comment on a pull
  request with `permissions:` set to `contents: read` only, plus
  `pull-requests: write`, plus `issues: write`; record the status codes
  and the error body of the failing case.
- **Annotations:** one step printing 15 errors and 15 warnings, a second
  step 15 more; count what the run shows per step and per job. Lines
  with `%`, `:`, `,`, CR and LF, escaped as in D1, read back as typed.
- **`pwsh`:** `$PSVersionTable.PSVersion` on each image (7.4 or later is
  required), and `>` of a native command's UTF-8 output with `é` and
  `ő` written unchanged.
- **Stop-commands:** a `::error::` line between `::stop-commands::` and
  its end token is shown as text.

Results go into this file before the code; if they contradict D1 to D6,
the change is updated (and its approval asked again) before PR 1.

### D9. Testing

- **Rust:** unit tests in `github.rs` as in `azure.rs`; CLI tests for
  `--format github` in every form, and the five-value usage error.
- **Pester** (`ci/github/qgate-action.Tests.ps1`): target mapping,
  checksum match and mismatch, base resolution per event, the four
  comment branches, the fork skip, pagination, 403 and 404
  messages, exit-code handling of the report and gate steps,
  stop-commands wrapping. The `pester` jobs run `ci/` (both
  directories); job names and required checks stay the same.
- **No self-test of `uses: ./` in CI:** the action downloads the release
  that matches its version, which does not exist until the tag is
  published. The README rehearsal (D10) runs the published action.

### D10. The README and its rehearsal (PR 2)

README "In GitHub Actions" becomes:
- a workflow with `on: pull_request`, `permissions:` (`contents: read`
  and the comment permission), `actions/checkout@v4` with
  `fetch-depth: 0`, and `uses: nagypalervin/qgate@v0.3.0`;
- the inputs; what the pull request shows (annotations, summary,
  comment, result); forks; pinning to a commit SHA;
- the JSON-artifact example, kept, after the action.

Rehearsal in the private repository, Linux and Windows runners, the
workflow cut out of the README and compared with `diff` as before:
1. a pull request with an error: annotation, summary, one comment,
   failed check;
2. a commit that fixes it: comment says no regressions, check passes;
3. a commit with a new error in a non-ASCII path: same comment updated;
4. a clean pull request: no comment;
5. `permissions:` without the comment permission: the step fails naming
   it, no gate;
6. `comment: false` with `contents: read` only: no comment, gate fails;
7. `on: push` without `base`: the message from D5.

Forks are covered by the Pester tests only; a fork needs a second
account.

### D11. Repository layout

`action.yml` at the root and `ci/github/` beside `ci/azure/`. README
"Repository layout" and `CLAUDE.md` "Layout" gain the action. No crate
changes structure; the dependency rules are untouched.

### D12. Two pull requests

- **PR 1,** `change/add-github-action`: spike record, D1, D2 to D7,
  tests, version 0.3.0. Then Ervin pushes `v0.3.0` and publishes.
- **PR 2,** `change/add-github-action-2`: README, rehearsal record,
  review fixes, journal entry, archive.

## Spike results (task 0.1, 2026-11-12)

Rehearsal repository `nagypalervin/qgate-rehearsal` (private, created by
the agent with `gh` at Ervin's request), pull request 1, run
43263496151, branch `spike`. One hand-written workflow, no qgate code
except the 0.2.0 release for the install prototype.

**Comment permissions** (job-level `permissions:`, pull request from the
same repository):

| Permissions | List comments | Create | Edit |
|---|---|---|---|
| `contents: read` | 403 | 403 | (none to edit) |
| `contents: read`, `issues: write` | 403 | 403 | (none to edit) |
| `contents: read`, `pull-requests: write` | 200 | 201 | 200 |

The 403 body is `{"message":"Resource not accessible by integration",
...}`. The docs name `issues: write` for comments, but on a pull
request only `pull-requests: write` works. D4's error message and the
README name `pull-requests: write`.

**Annotations:**
- Escaping as in D1 reads back exactly: `file=src/a%2Cb%3Ac.ts` →
  `src/a,b:c.ts`; `title=r%3Ax%2Cy%25z` → `r:x,y%z`; the message
  `100%25 done%0Anext: one, two` → `100% done`, line feed, `next: one,
  two` (raw `:` and `,` in the message are fine); `%0D` → CR.
- An annotation without `line` gets `start_line` 0 and keeps its file.
- **Limit: 10 errors and 10 warnings per step.** Step A printed 15 of
  each, steps B and C 15 errors each; the check run kept 10 + 10, 10
  and 10 (40 in the job, under any job limit). The log still shows all
  60 lines. A job limit was not reached and is not needed: the action
  annotates in one step.

**`::stop-commands::`:** an `::error::` line between
`::stop-commands::spike7f3a` and `::spike7f3a::` stayed text; a
`::warning::` after the end token became an annotation. D6 holds.

**`pwsh` and the install prototype** (D3), on `ubuntu-latest` (Linux
X64), `windows-latest` (Windows X64) and `macos-latest` (macOS ARM64):
- `pwsh` 7.6.9 on all three;
- `git -c core.quotepath=false ls-files > file` kept the UTF-8 bytes of
  `src/értékelés.ts` on all three;
- `tar`: GNU tar 1.35, bsdtar 3.8.9 (Windows), bsdtar 3.5.3 (macOS);
  `Expand-Archive` present everywhere;
- the prototype mapped `RUNNER_OS` × `RUNNER_ARCH` to the target,
  downloaded qgate 0.2.0 and `SHA256SUMS` with `Invoke-WebRequest
  -MaximumRetryCount 3`, matched `Get-FileHash`, unpacked (`tar`, or
  `Expand-Archive` for the `.zip`), and `qgate --version` printed
  `qgate 0.2.0` on all three.

**Other observations:**
- `actions/checkout@v4` logs a warning that its Node.js 20 runtime is
  deprecated and forced onto Node.js 24. The README (PR 2) should name
  the current major of `actions/checkout`; to be checked then.
- `ubuntu-latest` logs a notice that the label moves to Ubuntu 26. The
  musl binaries do not depend on the image.

None of this contradicts D1 to D6; the apply continues with 1.1.

## Decisions during apply

- **No `GITHUB_ENV`** (changes D2 and D3). Every step recomputes the base
  and the binary's path (`$RUNNER_TEMP/qgate-<version>/qgate-<version>-<target>/`)
  from the same inputs. The job's environment stays as it was, and a
  base input cannot write other variables through `GITHUB_ENV`.
- **`QGATE_VERSION` once per step** (changes D3's "one place"). A
  composite action has no shared `env:`, and every step needs the
  version for the paths. `.github/scripts/check-action-version.sh` checks
  every `QGATE_VERSION` line against the tag, so a missed one fails the
  release; the `release.yml` self-test also requires them to equal the
  workspace version on every pull request that touches `action.yml`.
- **Tests mock `Invoke-WebRequest`,** not a wrapper (changes D9). The
  wrapper `Invoke-GitHub` (headers, paging through `Link`, error
  mapping) is then tested too. `Invoke-Qgate` is mocked;
  `Expand-QgateArchive` is tested with a real `.tar.gz` and `.zip`.
- **The gate's stdout is copied as bytes** to the process's standard
  output (`Write-QgateOutput`), so non-ASCII paths in annotations do
  not depend on the console encoding.
- **No `actionlint`:** it checks workflows, not an action's metadata
  file. `action.yml` was checked by hand against the metadata syntax,
  and the pre-release run (below) runs it.
- **A run before the release** (new task 1.8). The action downloads the
  release that matches `QGATE_VERSION`, so the real action can only run
  after `v0.3.0`. To catch wiring mistakes earlier, a copy of
  `action.yml` and `ci/github/` with `QGATE_VERSION: 0.2.0` runs as a
  local action (`uses: ./qgate-action`) in the rehearsal repository.
  0.2.0 has no `--format github`, so the gate takes the exit-code-2
  path; install, report, job summary and comment run for real.

## Pre-release run (task 1.8, 2026-11-12)

Rehearsal repository, pull request 2 (branch `smoke`), run
43265505693. A copy of this branch's `action.yml` (every
`QGATE_VERSION` set to `0.2.0`) and `ci/github/qgate-action.ps1` as
`./qgate-action`; the workflow grants `contents: read` and
`pull-requests: write`, checks out with `fetch-depth: 0`, and runs the
action on `ubuntu-latest` (now Ubuntu 26.04), then on `windows-latest`.

| Step | Linux | Windows |
|---|---|---|
| Install | `installed qgate 0.2.0 (x86_64-unknown-linux-musl); comparing with origin/main` | the same for `x86_64-pc-windows-msvc` |
| Report | `added the report to the job summary` | the same |
| Comment | `created the qgate comment (1 finding, 0 at or above error)` | `updated the qgate comment (...)`, the same comment |
| Gate | qgate's `error: invalid value 'github' for '--format <format>'` between the stop-commands lines, then `qgate check could not run (exit code 2)` as an error; the step failed | the same |

- The pull request has one comment, by `github-actions[bot]`, with the
  marker line and the row for `src/értékelés.ts:1` (the non-ASCII path
  intact).
- The runner masks the stop-commands token in the log (`***`).
- The change to `src/grade.ts` produced no finding: the rehearsal's
  `main` had been seeded with the complex `grade.ts` of the Azure
  rehearsal, so the branch made it simpler. The PR 2 rehearsal resets
  `main` to the simple version first.

## Verification on PR 1 (tasks 1.5 and 1.6)

PR #16, all 18 checks green on the first push.
- **Pester (1.5):** both `pester (ubuntu-latest)` and `pester
  (windows-latest)` ran `ci/azure/qgate-pr-comment.Tests.ps1` and
  `ci/github/qgate-action.Tests.ps1`: 81 tests passed on each. The job
  names, and so the required checks, did not change.
- **Release rehearsal (1.6),** run 43265888318:
  - the `version` job's self-test printed "every QGATE_VERSION in
    action.yml is 0.3.0" for `v0.3.0` and refused `v0.3.0.1`;
  - six archives and `SHA256SUMS`; the Linux arm64 archive passed
    `sha256sum --check` and holds `qgate`, `qgate-pr-comment.ps1`,
    `README.md` and both licenses;
  - its binary printed `qgate 0.3.0` and, in a throwaway repository,
    `::warning file=src/a.ts,line=1,title=complexity/cyclomatic-increase::f:
    cyclomatic complexity 1 -> 4 (+3)`.

## Release 0.3.0 (task 2.1)

- PR #16 merged as e29df6e. At Ervin's request the agent pushed the
  annotated tag `v0.3.0` ("qgate 0.3.0", Ervin's noreply identity, as
  for 0.2.0) after `check-version.sh` and `check-action-version.sh`
  accepted it on `main`.
- Release run 43269011917: all jobs green; the `version` job printed
  "every QGATE_VERSION in action.yml is 0.3.0, as the tag v0.3.0 says".
- The draft held six archives and `SHA256SUMS`; `sha256sum --check`
  passed for all six; each holds the binary, `qgate-pr-comment.ps1`,
  `README.md` and both licenses; the Linux arm64 binary printed
  `qgate 0.3.0`.
- Ervin published the draft on 2026-11-12 (17:31 UTC); `releases/latest`
  is `v0.3.0` and its files download.

## Review of PR 1

The `reviewer` subagent found no blocker. Its findings and what was done:

- **Should-fix: Dependabot was skipped on a wrong premise.** Since 2021,
  workflows triggered by Dependabot get the permissions the workflow's
  `permissions:` key grants, so the README workflow gives them a token
  that can write. The skip is removed: the spec's requirement (then
  "Pull requests with a read-only token", renamed "Pull requests from
  forks" in the review of PR 2) now names forks only, and a test
  checks that a Dependabot pull request gets the comment. Changed after
  Gate 1; the proposal says so.
- **Should-fix: the spec promised an annotation per finding.** The spike
  showed GitHub keeps 10 errors and 10 warnings per step. The
  requirement "Checking a pull request with the GitHub Action" now
  promises one annotation command per finding in the log, says what
  GitHub shows, and points to the job summary.
- **Should-fix: a failed download named nothing.** `Save-QgateDownload`
  now names the URL and says that the release `v<version>` must be
  published (a draft's files cannot be downloaded); tested.
- **Nits fixed:**
  - stale design text (report path, the event author, the report
    step's output);
  - `$event` renamed (an automatic variable);
  - the comment request sends `charset=utf-8`;
  - the Install step no longer gets `QGATE_INPUT_FAIL_ON`;
  - the release self-test checks that the mismatch message names both
    versions;
  - new tests: the gate's stop-commands wrapping and order, a clean
    report in the job summary, and `Write-QgateOutput` copying bytes
    through a child `pwsh`;
  - the `pester` jobs also run on `macos-latest`. The new check is not
    in the "Protect main" ruleset; adding it is Ervin's call.
- **Kept, by design:**
  - Any failure of the comment step (a 500, no answer, a 403 for a rate
    limit) skips the gate, as the Azure job does. The job fails either
    way, and the error quotes GitHub's own message, so a rate limit or
    a locked conversation is visible even though the hint names the
    permission.
  - The scenarios "A warning without a line" and "Info findings left
    out" are covered by `github.rs` unit tests only, as their Azure
    counterparts are.
  - qgate's standard error is decoded with the console encoding by
    `2>`; non-ASCII text in an error message may look wrong in a
    Windows log. It reaches only the log.

## Rehearsal of the README workflow (task 3.2)

Rehearsal repository, 2026-11-12, with the published `@v0.3.0`. Before
it: pull requests 1 and 2 (spike, pre-release run) closed, and `main`'s
`src/grade.ts` reset to the simple version (complexity 2).

**The workflow is the README's.** A scratch script printed the first
`yaml` block of "In GitHub Actions" unchanged:

```sh
awk '/^### In GitHub Actions$/ { s = 1; next }
     s && /^### / { exit }
     s && !done && /^```yaml$/ { b = 1; next }
     b && /^```$/ { b = 0; done = 1; next }
     b { print }' README.md
```

Each branch's `.github/workflows/qgate.yml` was compared with it by
`diff`:
- Linux scenarios 1 to 4: no difference;
- Linux 5: `6d5` (no `pull-requests: write`); Linux 6: `6d5` and
  `15a15,16` (`with: comment: false`); Linux 7: `2c2` (`on: push`);
- Windows 1 to 4: `10c10` (`runs-on: windows-latest`); Windows 5 and 6:
  the Linux lines plus `10c9`; Windows 7: `2c2` and `10c10`.

| # | Scenario | Linux run (PR) | Windows run (PR) | Result on both |
|---|---|---|---|---|
| 1 | `grade` 2 -> 8 | 43270761465 (#3) | 43271598570 (#7) | step failed; error annotation `src/grade.ts:1`; comment created (Linux 6831081351, Windows 6831199408) |
| 2 | fix pushed | 43270874931 | 43271717142 | step succeeded; the same comment edited to the `count=0` report, "No regressions found." |
| 3 | `ertekeles` 1 -> 7 in `src/értékelés.ts` | 43271049647 | 43271826787 | step failed; annotation and comment row name `src/értékelés.ts:1` intact; still one comment |
| 4 | README only | 43271163964 (#4) | 43271978936 (#8) | step succeeded; no comment |
| 5 | no `pull-requests: write` | 43271266084 (#5) | 43272101206 (#9) | step failed with the 403 message naming `pull-requests: write`; no gate annotation; no comment |
| 6 | `comment: false`, `contents: read` | 43271402040 (#6) | 43272211304 (#10) | step failed with the `grade` annotation; no comment |
| 7 | `on: push`, no `base` | 43271490407 | 43272363368 | step failed: "no base revision to compare with; run the action on a pull_request event, or set its base input" |

- **The comment's bodies,** from its edit history (GraphQL
  `userContentEdits`), the same on both pull requests: scenario 1
  `count=1 failing=1` with the row `error | src/grade.ts:1 | grade:
  cyclomatic complexity 2 -\> 8 (+6)`; scenario 2 `count=0` and "No
  regressions found."; scenario 3 `count=1 failing=1` with the row for
  `src/értékelés.ts:1`. Linux 6831081351 (created 17:34:01, edited
  17:34:56 and 17:35:36), Windows 6831199408 (17:38:54, 17:39:35,
  17:40:14).
- qgate's summary line on standard error appeared between the
  stop-commands lines in every run that had one (scenarios 1, 3, 5 and
  6); the runner masked the token. Scenarios 2 and 4 print nothing on
  standard error, and 7 fails before qgate runs.
- The README's steps without the action (the install block and the
  `--format github` line, wrapped in one job unchanged) ran on Linux,
  run 43272496815 (#11): the archive passed `sha256sum -c`, and the
  step failed with the `grade` annotation.
- `actions/checkout@v7` logged no deprecation warning.

**Ervin's check in the web UI (task 3.3).** Ervin looked at pull
request 3 (Linux, after scenario 3) and pasted the rendered comment:
the heading "qgate", "1 finding, 1 at or above error.", and the table
with `error | src/értékelés.ts:1 | ertekeles: cyclomatic complexity 1
-> 7 (+6) | complexity/cyclomatic-increase`. The table renders, the
marker line and the `\>` escape do not show. The annotation was
checked through the check-run API above; the job summary was not
reported separately, and GitHub has no API for it (the check run's
`output` fields are empty). See "Review of PR 2".

After the review of PR 2, Ervin looked at Windows scenario 3 (pull
request 7, run 43271826787):
- **the job summary** on the run's Summary page: he pasted the same
  rendered report as the comment ("1 finding, 1 at or above error." and
  the `src/értékelés.ts:1` row);
- **the annotation on the diff:** on "Files changed", at line 1 of
  `src/értékelés.ts`, he saw the error annotation "ertekeles:
  cyclomatic complexity 1 -> 7 (+6)". The page's copied text does not
  include annotations, so this was asked as a question. Ervin also merged that test pull request into the
rehearsal repository's `main` (it was meant only to be looked at); no
effect on qgate, but the next rehearsal starts from a reverted `main`.

## Review of PR 2

The `reviewer` subagent found no blocker. Its findings and what was done:

- **Should-fix: the job summary does not hold every finding.** Its
  table stops at 50 rows (`MAX_ROWS`). The README now says "a table of
  up to 50 findings" and that the step's log lists every finding; the
  ci-integration delta no longer says the job summary lists the
  findings.
- **Should-fix: nobody had seen the job summary or the annotation on
  the diff.** GitHub has no API for the job summary. Ervin looked at
  both on pull request 7 (see the rehearsal section); task 3.3 was
  reopened until then.
- **Should-fix: the README promised six platforms, two had run.** A
  second rehearsal ran the README workflow (`comment: false`, so the
  jobs do not race) on the other four, pull request 12, run
  43276141365: `ubuntu-24.04-arm`, `windows-11-arm`, `macos-latest`
  (ARM64) and `macos-15-intel` each installed its own target
  (`aarch64-unknown-linux-musl`, `aarch64-pc-windows-msvc`,
  `aarch64-apple-darwin`, `x86_64-apple-darwin`) and failed with the
  `grade` annotation. The sentence stays.
- **Should-fix: `actions/upload-artifact@v4` runs on Node 20.** The
  README names `@v7` (Node 24). Its JSON steps, cut out of the README
  unchanged, ran in the same run: the check step failed with exit code
  1, the upload ran (`if: ${{ !cancelled() }}`), and the artifact
  `qgate-report` held one finding and the summary `count=1 failing=1`.
- **Scenarios the first rehearsal left to Pester,** now run (task 3.5):
  - "A warning with the default level" and "A stricter level", pull
    request 13, run 43276192820: with one Warning (`grade` 2 -> 5), the
    default job succeeded with a warning annotation, and the job with
    the README's `fail-on: warning` failed;
  - "A push with a base", branch `extra/pushbase`, run 43276190605:
    `base: origin/main` on `push` failed with the `grade` annotation,
    and the comment step said "not a pull_request event (push); no
    comment to write".
  - "A pull request from a fork" stays covered by Pester only.
- **Nits fixed:** the gate is "skipped" rather than "the check does not
  run" in the README; self-hosted runners also need `git` and network
  access, and `tar` only on Linux and macOS; pinning a commit does not
  pin the binary, which still comes from the release; forks "usually"
  get a read-only token, and the requirement is renamed "Pull requests
  from forks"; the README's status note mentions the CI integrations;
  D4 and D5 match the code; the extraction is printed above; the diff
  list is exact; the stop-commands claim names the runs it covers; the
  comment bodies come from the edit history.
- **Slip, not rewritten:** task 3.1 was ticked in the commit after the
  README change, not in the same one, as CLAUDE.md asks.

## Risks / Trade-offs

- **[The action is first run for real after the release]** A bug found
  in the rehearsal needs 0.3.1. → The Pester tests cover every branch;
  the spike checks the platform facts first; a patch release is cheap.
- **[Marketplace listing may need a repository without workflows]** →
  Out of scope; moving `action.yml` later changes the `uses:` path, so
  the README would say so then.
- **[GitHub shows only some annotations]** 10 errors and 10 warnings
  per step (spike). → The gate's log still prints every finding, and
  the job summary and the comment list up to 50; the README says so.
- **[`github-actions[bot]` as the only author]** A user who switches to
  a PAT through a fork of the action gets a second comment. → Not
  supported; no `token` input.
- **[Users pin by tag]** A moved tag would change their code. → The
  README recommends a commit SHA for strict pinning; qgate never moves
  release tags.
- **[Rate limits]** Listing comments costs a request per 100 comments.
  → Negligible against the 1,000 requests per hour of `GITHUB_TOKEN`.
