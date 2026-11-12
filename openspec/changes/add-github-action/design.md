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
   unpack; write `QGATE` (the binary's path) and `QGATE_BASE` to
   `$GITHUB_ENV`.
2. `Report`: `qgate check --base $base --format markdown --fail-on
   $failOn` into `$RUNNER_TEMP/qgate.md`; exit 0 and 1 are both fine,
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

- **Pull request:** number, head repository and author from the event
  payload at `$GITHUB_EVENT_PATH`.
- **Read-only token, skip with a notice** (`::notice::`), before any
  API call:
  - the head repository's `full_name` differs from `GITHUB_REPOSITORY`,
    or it is null (deleted fork): "the comment is skipped for pull
    requests from forks: their GITHUB_TOKEN cannot write";
  - `GITHUB_ACTOR` is `dependabot[bot]`: the same, for Dependabot.
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
- Empty otherwise: fail in `Install`, before downloading, with "qgate
  needs a pull_request event or a base input".
- The comment step also skips (silently) on events other than
  `pull_request`: there is no pull request to comment on.
- A shallow checkout is not detected: qgate's own error names the
  missing revision, and the README says `fetch-depth: 0`.

### D6. Untrusted text in the log

The runner reads workflow commands from a step's stdout and stderr.
Paths and messages come from the pull request, and qgate's errors (exit
2) can quote revision names or config values. So the script:
- prints the gate's stdout as it is (qgate escapes it, D1);
- prints qgate's stderr from both runs, and the report step's output,
  between `::stop-commands::<random token>` and `::<token>::`.

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
  comment branches, fork and Dependabot skips, pagination, 403 and 404
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
  comment, result); forks and Dependabot; pinning to a commit SHA;
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

Forks and Dependabot are covered by the Pester tests only; a fork needs
a second account.

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
