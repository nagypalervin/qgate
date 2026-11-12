# Tasks

This change spans two pull requests, with a release between them (design
D12).
- **Decisions made during apply** go into `design.md`.
- **Every commit passes the pre-commit checks:** `cargo fmt`, `cargo
  clippy -D warnings` and `cargo test --workspace`.
- **Commit messages and PR bodies** are written to files first.
- **The rehearsal repository** is private; test pull requests go there,
  never to `nagypalervin/qgate`.

## 0. Spike (before any code, design D8)

- [x] 0.1 Once the private rehearsal repository exists (Ervin creates it, or tells the agent to), push a TypeScript fixture and the spike workflow, and open a pull request in it. Run the workflow on `ubuntu-latest`, `windows-latest` and `macos-latest` and record in `design.md`:
  - status codes for creating and editing a comment with `contents: read`, with `pull-requests: write` and with `issues: write`, and the error body of the failing case;
  - annotations shown per step and per job;
  - the read-back of escaped `%`, `:`, `,`, CR and LF;
  - the `pwsh` version per image, and whether `>` keeps `é` and `ő`;
  - whether `::stop-commands::` hides a `::error::` line.

  If a result contradicts design D1 to D6, update the artifacts and ask Ervin before 1.1.

## 1. Format, action, release check (PR 1, `change/add-github-action`)

- [ ] 1.1 Write failing unit tests for `render_github` in `crates/qgate-report` (design D1):
  - one error, one warning;
  - a finding without a line;
  - an Info finding left out;
  - no findings, and only Info findings (`No regressions found.`);
  - each escape in `file`, `title` and the message.

  Then implement it in `github.rs`.
- [ ] 1.2 Add `github` to `--format` in `qgate-cli`. Add end-to-end tests in `crates/qgate-cli/tests/`:
  - `--format github` in each `check` form;
  - exit code 1 and the stderr summary line with an Error;
  - the usage error for `--format xml` naming all five values.

  Update `qgate-report`'s module doc and the `--format` help text.
- [ ] 1.3 Write `ci/github/qgate-action.Tests.ps1` (Pester 5.9.1, mocked `Invoke-Qgate` and `Invoke-GitHub`) for every case in design D9, then `ci/github/qgate-action.ps1` until the tests pass locally.
- [ ] 1.4 Write `action.yml` (design D2): the three inputs, the four steps, `QGATE_VERSION`, the `comment` condition. Check it with `actionlint` if it can be installed with a verified checksum; otherwise review it by hand against GitHub's metadata syntax.
- [ ] 1.5 Point the `pester (<os>)` jobs at `ci/` so they run both directories; verify on the PR that both jobs run the new tests and keep their names.
- [ ] 1.6 Release check and version (design D7):
  - a step in `release.yml`'s `version` job compares `action.yml`'s `QGATE_VERSION` with the tag, with a self-test of the mismatch;
  - `release.yml` triggers on changes to `action.yml`;
  - the version becomes `0.3.0`, with `Cargo.lock` following.

  Verify from the PR's release rehearsal that the Linux arm64 binary prints `qgate 0.3.0` and renders `--format github`.
- [ ] 1.7 Add `action.yml` and `ci/github/` to `CLAUDE.md`'s "Layout" and to the README's "Repository layout" (design D11).

## 2. Release 0.3.0 (between the pull requests)

- [ ] 2.1 After PR 1 merges, ask Ervin to push `v0.3.0` (or to let the agent push it). Check the draft release:
  - six archives and `SHA256SUMS`;
  - `sha256sum --check`;
  - the Linux arm64 binary prints `qgate 0.3.0`.

  Then ask Ervin to publish. Record the result in `design.md` on `change/add-github-action-2`.

## 3. README and rehearsal (PR 2, `change/add-github-action-2`)

- [ ] 3.1 Rewrite README "In GitHub Actions" (design D10), and add `github` to the formats paragraph. Move every install block to `0.3.0` and verify with `grep` that no `0.2.0` is left in them.
- [ ] 3.2 Rehearse the README workflow, extracted unchanged, in the rehearsal repository on `ubuntu-latest` and `windows-latest` with scenarios 1 to 7 of design D10. Check comments, annotations and the job summary through the REST API.
- [ ] 3.3 Ask Ervin to look at one scenario-1 pull request in the web UI: the annotation on the diff, the job summary, the comment.
- [ ] 3.4 Record the rehearsal in `design.md`: run ids, comment ids and bodies per scenario, the extraction `diff`, Ervin's check.

## Workflow follow-up

- Run the `reviewer` subagent on PR 1 and on PR 2 before asking Ervin to merge each, and fix its findings.
- On PR 2: write the `docs/journal/` entry, then archive the change (`/opsx:archive`).
- Gate 3: Ervin merges each pull request.
- Update the M4 memory after each merge.
