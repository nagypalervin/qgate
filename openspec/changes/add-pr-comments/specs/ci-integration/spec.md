# Spec Delta

## MODIFIED Requirements

### Requirement: Documented Azure Pipelines job

The README SHALL give a `qgate` job for Azure Pipelines in two variants:
- one for a Microsoft-hosted Linux agent (`ubuntu-latest`) in bash, with
  a PowerShell 7 step for the comment;
- one for a Microsoft-hosted Windows agent (`windows-latest`) in PowerShell 7.

Each variant SHALL work when copied unchanged into the `jobs` of a
pipeline for an Azure Repos Git repository. It SHALL need no service
connection. Its only token SHALL be the pipeline's own
`System.AccessToken`, and its only permission beyond a default
pipeline's SHALL be the one the comment needs.

#### Scenario: Copying the Linux job

- **GIVEN** an Azure Repos Git repository with TypeScript sources, whose project's Build Service may contribute to pull requests
- **WHEN** a user copies the README's Linux job into the repository's pipeline and makes no other change to it
- **THEN** the pipeline runs without asking for a service connection or a token of its own

#### Scenario: Copying the Windows job

- **GIVEN** the same repository
- **WHEN** a user copies the README's Windows job instead
- **THEN** the pipeline runs without asking for a service connection or a token of its own

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

## ADDED Requirements

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
