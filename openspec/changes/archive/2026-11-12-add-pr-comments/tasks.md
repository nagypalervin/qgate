# Tasks

This change spans two pull requests, with a release between them (design
D10).
- **Decisions made during apply** go into `design.md`.
- **Every commit passes the pre-commit checks:** `cargo fmt`, `cargo
  clippy -D warnings` and `cargo test --workspace`.
- **Commit messages and PR bodies** are written to files first.
- **Azure DevOps** is reached only through the token file
  `~/.config/qgate-rehearsal/pat`, and only in the rehearsal project.
  The token is never printed.

## 1. Formats, script, packaging (PR 1, `change/add-pr-comments`)

- [x] 1.1 Write failing unit tests for `render_markdown` in `crates/qgate-report` (design D2):
  - one error;
  - no findings;
  - the three summary-sentence forms (none, some, all at or above the level) and singular and plural;
  - the marker line;
  - escaping of each special character and of CR and LF;
  - a finding without a line;
  - related locations;
  - 60 findings (50 rows and the "10 more" line);
  - byte-identical output for the same input.

  Then implement `render_markdown` until they pass.
- [x] 1.2 Write failing unit tests for `render_azure` (design D3):
  - one error, one warning;
  - a finding without a line;
  - an Info finding left out;
  - no findings, and only Info findings (`No regressions found.`);
  - each escape in `sourcepath`, `code` and the message.

  Then implement it.
- [x] 1.3 Add `markdown` and `azure` to `--format` in `qgate-cli` (design D1). Add end-to-end tests in `crates/qgate-cli/tests/`:
  - each format in each `check` form;
  - exit code 1 and the stderr summary line with an Error;
  - exit code 0 with `--fail-on never`;
  - exit code 2 and empty stdout for an unknown base;
  - the usage error for `--format xml` naming all four values.

  Update `qgate-report`'s module doc.
- [x] 1.4 Install PowerShell 7 locally; Ervin agreed on 2026-11-11 (design D5, proposal):
  - install it from the official release archive after checking its SHA-256;
  - verify that `pwsh --version` reports 7.4 or later.
- [x] 1.5 Write `ci/azure/qgate-pr-comment.Tests.ps1` (Pester 5.5+, mocked `Invoke-RestMethod`) for every case in design D4 and D5, then `ci/azure/qgate-pr-comment.ps1` until the tests pass:
  - locally (1.4);
  - in CI (1.6).
- [x] 1.6 Add the `pester (<os>)` job to `.github/workflows/ci.yml` for `ubuntu-latest` and `windows-latest`. Open PR 1 and verify that both pass. With Ervin's approval, make both checks required in the "Protect main" ruleset and verify the ruleset with `gh api`.
- [x] 1.7 Release packaging and version (design D6):
  - `release.yml` packs the script and triggers on `ci/azure/**`;
  - the version becomes `0.2.0`, with `Cargo.lock` following.

  Verify from the PR's release rehearsal that all six archives contain `qgate-pr-comment.ps1`, and that the rehearsal's Linux arm64 binary prints `qgate 0.2.0` and renders `--format markdown`.
- [x] 1.8 Run the script against the real API (design D5). Push the PR's script and hand-written reports to a rehearsal branch, then run it in the rehearsal pipeline on both agents:
  - create (findings, no thread);
  - update and reactivate;
  - close (no findings);
  - nothing on a clean report with no thread;
  - one run with the permission removed by Ervin, checking that the message names the permission and the Build Service.

  Record the results in `design.md`.
- [x] 1.9 Add the `ci/` line to `CLAUDE.md`'s "Layout" and to the README's "Repository layout" (design D9).

## 2. Release 0.2.0 (between the pull requests)

- [x] 2.1 After PR 1 merges, ask Ervin to push `v0.2.0`. Check the draft release:
  - six archives and `SHA256SUMS`;
  - `sha256sum --check`;
  - every archive holds `qgate-pr-comment.ps1`;
  - the Linux arm64 binary prints `qgate 0.2.0`.

  Then ask Ervin to publish. Record the result in `design.md` on `change/add-pr-comments-2`. (Ervin asked Claude to push the tag; see design.md.)

## 3. README and rehearsal (PR 2, `change/add-pr-comments-2`)

- [x] 3.1 Update the README (design D7):
  - the "In Azure Pipelines" jobs, on both agents: the `QGATE_DIR` variable, the Markdown step, the comment step with `SYSTEM_ACCESSTOKEN`, and the gate with `--format azure` last;
  - the permission steps and the identity note;
  - what the developer sees;
  - every install block on `0.2.0`;
  - a short note on the `markdown` and `azure` formats in the formats section.

  Verify with `grep` that no `0.1.0` is left in install blocks.
- [x] 3.2 Rehearse the README jobs, extracted unchanged, on both agents with scenarios 1 to 4 of design D8:
  - check the threads through the REST API;
  - check the build issues through the build timeline;
  - abandon the spike's pull request 1349 first.
- [x] 3.3 Ask Ervin to look at one scenario-1 comment in the web UI. Ervin confirms that the table renders, the escapes do not show, and there is no marker line. (Checked after scenario 3, whose comment holds scenario 1's row; see design.md.)
- [x] 3.4 Record the rehearsal in `design.md`:
  - the build ids;
  - the thread ids and statuses per scenario;
  - the extraction `diff`;
  - Ervin's check.

## Workflow follow-up

- Run the `reviewer` subagent on PR 1 and on PR 2 before asking Ervin to merge each, and fix its findings.
- On PR 2: write the `docs/journal/` entry, then archive the change (`/opsx:archive`).
- Gate 3: Ervin merges each pull request.
- Update the M4 memory after each merge. The rehearsal token expires on 2026-11-17; ask Ervin for a new one if PR 2 runs later.
