# Spec Delta

## ADDED Requirements

### Requirement: GitHub Action for pull requests

The repository SHALL provide a GitHub Action, used as
`nagypalervin/qgate@v<version>`. In a workflow triggered
`on: pull_request` that checks out the full history, the action SHALL
check the pull request's changes against its base branch with no inputs
set. It SHALL need no secret; its only token SHALL be the job's
`GITHUB_TOKEN`.

#### Scenario: Using the action with its defaults

- **GIVEN** a GitHub repository with TypeScript sources, and a workflow `on: pull_request` that checks out with `fetch-depth: 0` and grants the permissions the README names
- **WHEN** a step `uses: nagypalervin/qgate@v0.3.0` without inputs runs for a pull request into `main`
- **THEN** the step checks the pull request's changes against `origin/main`

### Requirement: Verified install in the GitHub Action

The action SHALL install the qgate release whose version equals its own
tag, for the runner's operating system and architecture, outside the
checkout. It SHALL check the archive against the release's `SHA256SUMS`
and SHALL fail before running qgate if the check fails or the runner's
platform has no release archive.

#### Scenario: Installing on Linux and Windows

- **GIVEN** a step `uses: nagypalervin/qgate@v0.3.0`
- **WHEN** it runs on `ubuntu-latest`, and again on `windows-latest`
- **THEN** each run uses qgate 0.3.0 from the archive for that runner's target

#### Scenario: Checksum mismatch

- **GIVEN** a downloaded archive whose SHA-256 differs from its line in `SHA256SUMS`
- **WHEN** the action installs qgate
- **THEN** the step fails with a message that names the archive
- **AND** qgate does not run

### Requirement: Checking a pull request with the GitHub Action

The action's result SHALL follow qgate's exit code for its `fail-on`
input (default `error`): it SHALL fail when the pull request adds a
finding at or above that level. The step's log SHALL hold one
annotation command per Error and Warning finding; GitHub shows at most
10 errors and 10 warnings of a step as annotations, and the job summary
lists the findings.

#### Scenario: An error fails the step

- **GIVEN** a pull request that raises a function's cyclomatic complexity from 2 to 8
- **WHEN** the action runs with its defaults
- **THEN** the step fails
- **AND** the run shows an error annotation on that function's file and line

#### Scenario: A warning with the default level

- **GIVEN** a pull request whose only finding is a Warning
- **WHEN** the action runs with its defaults
- **THEN** the step succeeds and the run shows a warning annotation

#### Scenario: A stricter level

- **GIVEN** the same pull request
- **WHEN** the action runs with `fail-on: warning`
- **THEN** the step fails

### Requirement: Findings in the GitHub Actions job summary

On every run, the action SHALL add the Markdown report to the job
summary: with or without findings, and also when it skips the comment.
Writing the summary SHALL NOT by itself change the step's result.

#### Scenario: Summary of a clean pull request

- **GIVEN** a pull request without findings
- **WHEN** the action runs
- **THEN** the job summary says that no regressions were found
- **AND** the step succeeds

### Requirement: One qgate comment per pull request on GitHub

With its `comment` input `true` (the default), the action SHALL keep at
most one qgate comment on a pull request, holding the latest Markdown
report:
- a run with findings SHALL create the comment, or update it if it exists;
- a run without findings SHALL update an existing comment to say no
  regressions were found;
- if no comment exists, it SHALL create none.

#### Scenario: First run with a finding

- **GIVEN** a pull request without a qgate comment, whose run finds one error
- **WHEN** the action runs
- **THEN** the pull request has one qgate comment that lists that finding

#### Scenario: A later run with findings

- **GIVEN** a pull request with a qgate comment from an earlier run
- **WHEN** a new commit is pushed and the action finds findings again
- **THEN** the pull request still has one qgate comment, holding the new report

#### Scenario: Findings fixed

- **GIVEN** a pull request with a qgate comment that lists findings
- **WHEN** a new commit removes every finding and the action runs
- **THEN** the comment says that no regressions were found

#### Scenario: A clean pull request

- **GIVEN** a pull request without a qgate comment, whose run finds nothing
- **WHEN** the action runs
- **THEN** the pull request has no qgate comment

### Requirement: Comments turned off

With its `comment` input `false`, the action SHALL neither read, create
nor update pull request comments, and SHALL need no permission beyond
reading the repository.

#### Scenario: A finding without a comment

- **GIVEN** a workflow that grants only `contents: read`
- **WHEN** the action runs with `comment: false` for a pull request with an error
- **THEN** the step fails because of the error, with its annotation and job summary
- **AND** the pull request has no qgate comment

### Requirement: Pull requests with a read-only token

For a pull request from a fork, GitHub gives the job a `GITHUB_TOKEN`
that cannot write, unless the repository sends write tokens to fork
pull requests. The action SHALL skip the comment for every fork pull
request with a notice that says why, and SHALL still write the job
summary and the annotations and decide the step's result as for any
pull request.

#### Scenario: A pull request from a fork

- **GIVEN** a pull request from a fork that adds an error
- **WHEN** the action runs
- **THEN** the run shows a notice that the comment was skipped for a fork
- **AND** the step fails because of the error, with its annotation and job summary

### Requirement: Missing comment permission on GitHub

For a pull request from the same repository, when the token may not
write the comment, the action SHALL fail with a message that names the
`permissions:` entry the workflow must grant. It SHALL NOT run the gate
after that failure.

#### Scenario: Permission not granted

- **GIVEN** a workflow that grants only `contents: read`
- **WHEN** the action runs with its defaults for a same-repository pull request with a finding
- **THEN** the step fails, and its log names the permission to grant

### Requirement: Events other than pull requests

Outside a `pull_request` event, the action SHALL use its `base` input.
Without one, it SHALL fail before installing anything, with a message
that asks for a `pull_request` trigger or a `base` input. With one, it
SHALL check against that base and skip the comment.

#### Scenario: A push without a base

- **GIVEN** a workflow `on: push`
- **WHEN** the action runs without a `base` input
- **THEN** the step fails with a message that names `pull_request` and `base`

#### Scenario: A push with a base

- **GIVEN** a workflow `on: push` on a branch that adds an error
- **WHEN** the action runs with `base: origin/main`
- **THEN** the step fails because of the error, with its annotation and job summary
- **AND** no comment is written
