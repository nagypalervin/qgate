# ci-integration Specification

## Purpose

Defines the CI setups that qgate documents and rehearses. For each, it
states what the setup guarantees on a pull request: the release it
installs, the changes it checks, and when it fails the build. Teams can
then copy the setup and rely on it as a merge gate.

## Requirements

### Requirement: Documented Azure Pipelines job

The README SHALL give a `qgate` job for Azure Pipelines in two variants:
- one for a Microsoft-hosted Linux agent (`ubuntu-latest`) in bash;
- one for a Microsoft-hosted Windows agent (`windows-latest`) in PowerShell 7.

Each variant SHALL work when copied unchanged into the `jobs` of a
pipeline for an Azure Repos Git repository. It SHALL need no token,
service connection or permission beyond those of a default pipeline.

#### Scenario: Copying the Linux job

- **GIVEN** an Azure Repos Git repository with TypeScript sources
- **WHEN** a user copies the README's Linux job into the repository's pipeline and makes no other change to it
- **THEN** the pipeline runs without asking for a service connection or a token

#### Scenario: Copying the Windows job

- **GIVEN** the same repository
- **WHEN** a user copies the README's Windows job instead
- **THEN** the pipeline runs without asking for a service connection or a token

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
SHALL fail when the check exits non-zero, which it does for a finding at
or above the `--fail-on` level, and the findings SHALL be in the job log.

#### Scenario: A pull request that adds an error

- **GIVEN** a pull request into `main` that raises a function's cyclomatic complexity enough for an error finding
- **WHEN** its pull request build runs the documented job, on either agent
- **THEN** the job fails
- **AND** its log shows the finding's line in the text format of the `check-command` capability: severity `error`, the path and line, the message and `[complexity/cyclomatic-increase]`

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
