# Spec Delta

## ADDED Requirements

### Requirement: The action installs the tagged version

A tag `v<version>` SHALL fail the release workflow before anything is
built when the qgate version that the repository's GitHub Action
installs differs from `<version>`. The message SHALL name both
versions, and no release SHALL be created.

#### Scenario: Action left at the old version

- **GIVEN** the workspace version is `0.3.0` and the action installs `0.2.0`
- **WHEN** a maintainer pushes the tag `v0.3.0`
- **THEN** the release workflow fails with a message that names `0.2.0` and `0.3.0`
- **AND** no release exists for `v0.3.0`

#### Scenario: Matching versions

- **GIVEN** the workspace version is `0.3.0` and the action installs `0.3.0`
- **WHEN** a maintainer pushes the tag `v0.3.0`
- **THEN** a draft release `qgate 0.3.0` for `v0.3.0` exists
