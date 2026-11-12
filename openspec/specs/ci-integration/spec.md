# ci-integration Specification

## Purpose

Defines the CI setups that qgate documents and rehearses. For each, it
states what the setup guarantees on a pull request: the release it
installs, the changes it checks, and when it fails the build. Teams can
then copy the setup and rely on it as a merge gate.

## Requirements

### Requirement: Documented Azure Pipelines job

The README SHALL give a `qgate` job for Microsoft-hosted Azure
Pipelines agents in two variants:
- Linux (`ubuntu-latest`), in bash with a `pwsh` step for the comment;
- Windows (`windows-latest`), in PowerShell 7.

Each variant SHALL work when copied unchanged into the `jobs` of a
pipeline for an Azure Repos Git repository. It SHALL need no service
connection. Its only token SHALL be `System.AccessToken`, and its
only permission beyond a default pipeline's SHALL be the one the
comment needs.

#### Scenario: Copying the Linux job

- **GIVEN** an Azure Repos Git repository with TypeScript sources, whose project's Build Service may contribute to pull requests
- **WHEN** a user copies the README's Linux job into the repository's pipeline and makes no other change to it
- **THEN** the pipeline runs without asking for a service connection or a token of its own

#### Scenario: Copying the Windows job

- **GIVEN** the same repository
- **WHEN** a user copies the README's Windows job instead
- **THEN** the pipeline runs without asking for a service connection or a token of its own

### Requirement: Verified install in Azure Pipelines

The documented job SHALL install a pinned qgate release outside the
checkout and check the archive against the release's `SHA256SUMS`. If
the download fails or the checksum does not match, the job SHALL stop
before unpacking, so it neither installs nor runs qgate.

#### Scenario: A corrupted archive on Linux

- **GIVEN** the downloaded archive does not match its line in `SHA256SUMS`
- **WHEN** the Linux job's install step runs
- **THEN** the step fails before unpacking and `qgate check` does not run

#### Scenario: A corrupted archive on Windows

- **GIVEN** the downloaded archive does not match its line in `SHA256SUMS`
- **WHEN** the Windows job's install step runs
- **THEN** the step fails before unpacking and `qgate check` does not run

#### Scenario: A release that cannot be downloaded

- **GIVEN** the pinned version has no release, so the download gets HTTP 404
- **WHEN** the install step of either job runs
- **THEN** the step fails before unpacking and `qgate check` does not run

### Requirement: Checking a pull request in Azure Pipelines

In a pull request build, the documented job SHALL run `qgate check` on
the changes the pull request makes against its target branch. The job
SHALL fail when the check that gates it exits non-zero, which it does
for a finding at or above the `--fail-on` level. The findings SHALL be
shown as build issues and in the pull request's qgate comment, as the
requirements below describe.

#### Scenario: A pull request that adds an error

- **GIVEN** a pull request into `main` that raises a function's cyclomatic complexity enough for an error finding
- **WHEN** its pull request build runs the documented job, on either agent
- **THEN** the job fails
- **AND** its log shows the finding as an error annotation with the path and line, the message and `complexity/cyclomatic-increase`

#### Scenario: A pull request without findings

- **GIVEN** a pull request into `main` that changes only `README.md`
- **WHEN** its pull request build runs the documented job, on either agent
- **THEN** the job succeeds and its log shows `No regressions found.`

### Requirement: Merge gate with a branch policy

The README SHALL describe how to require the pipeline in a build
validation branch policy on the target branch. With that policy, a pull
request whose documented job fails SHALL be blocked from completing.

#### Scenario: The policy blocks a failing pull request

- **GIVEN** a required build validation policy on `main` for the pipeline with the documented job
- **WHEN** a pull request's job fails because of an error finding
- **THEN** the policy's status for the pull request is rejected

### Requirement: Only pull request builds in Azure Pipelines

The documented job SHALL run only when the build reason is a pull
request. For any other build, such as a CI build after a push or a
manual run, the job SHALL be skipped and SHALL not fail the build.

#### Scenario: A manual run

- **WHEN** the pipeline with the documented job is run by hand on a branch
- **THEN** the `qgate` job is skipped and the build does not fail because of it

#### Scenario: A CI build after a push

- **GIVEN** the pipeline has a CI trigger for the pushed branch
- **WHEN** a push to that branch starts a build
- **THEN** the `qgate` job is skipped and the build does not fail because of it

### Requirement: Findings in the Azure Pipelines build summary

In a pull request build, the documented job SHALL report each Error and
Warning finding as an issue of the build, with its file, line and rule,
so that the build summary lists it. Reporting the issues SHALL NOT by
itself change the job's result.

#### Scenario: An error in the build summary

- **GIVEN** a pull request that raises a function's cyclomatic complexity enough for an error finding
- **WHEN** its pull request build runs the documented job, on either agent
- **THEN** the build has an error issue whose source is the finding's file and line and whose code is `complexity/cyclomatic-increase`
- **AND** the job fails

### Requirement: One qgate comment per pull request

The documented job SHALL keep at most one qgate comment thread on a pull
request, written by the project's Build Service, holding the latest
Markdown report:
- a build with findings SHALL create the thread, or update it and set it
  active if it exists;
- a build without findings SHALL update an existing thread to say no
  regressions were found and close it;
- if no thread exists, it SHALL create none.

#### Scenario: First build with a finding

- **GIVEN** a pull request without a qgate thread, whose build finds one error
- **WHEN** the documented job runs
- **THEN** the pull request has one active qgate thread whose comment lists that finding

#### Scenario: A later build with findings

- **GIVEN** a pull request with a qgate thread from an earlier build
- **WHEN** a new commit is pushed and the job finds findings again
- **THEN** the pull request still has one qgate thread, active, whose comment is the new report

#### Scenario: Findings fixed

- **GIVEN** a pull request with an active qgate thread
- **WHEN** a new commit removes every finding and the job runs
- **THEN** the thread says that no regressions were found and is closed

#### Scenario: A clean pull request

- **GIVEN** a pull request without a qgate thread, whose build finds nothing
- **WHEN** the documented job runs
- **THEN** the pull request has no qgate thread

### Requirement: Missing comment permission

When the build identity may not contribute to pull requests, the comment
step SHALL fail with a message that names the missing permission
("Contribute to pull requests") and the identity that needs it (the
project's Build Service). It SHALL NOT leave a partial comment.

#### Scenario: Permission not granted

- **GIVEN** the project's Build Service may not contribute to pull requests on the repository
- **WHEN** the documented job runs for a pull request with a finding
- **THEN** the comment step fails, and its log names "Contribute to pull requests" and the Build Service
