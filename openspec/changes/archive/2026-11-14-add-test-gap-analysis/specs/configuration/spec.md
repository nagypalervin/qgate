# Spec Delta

## MODIFIED Requirements

### Requirement: Missing configuration

When the head revision has no `qgate.toml`, the system SHALL apply no
architecture rules, exclude no file, exempt no source file from test
analysis and otherwise behave as without a configuration file.

#### Scenario: Repository without qgate.toml

- **GIVEN** a repository with no `qgate.toml` in which `feature` makes a function more complex
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** the output is the same as before this change

## ADDED Requirements

### Requirement: The tests table

`qgate.toml` MAY have a `[tests]` table whose only key is `exclude`, a
list of path patterns (see "Sources that need no test" in
`test-analysis`). An empty list or a missing table SHALL exempt no
file. An unknown key or an invalid pattern SHALL be an invalid
configuration, and the error SHALL name `tests.exclude` and the
pattern.

#### Scenario: Invalid tests exclude pattern

- **GIVEN** `qgate.toml` has `[tests]` with `exclude = ["/src/**"]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml`, `tests.exclude` and the pattern `/src/**`
- **AND** the exit code is 2

#### Scenario: Misspelled key

- **GIVEN** `qgate.toml` has `[tests]` with `exlude = ["src/environments/**"]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml` and `exlude`
- **AND** the exit code is 2

#### Scenario: Excluding everything

- **GIVEN** `qgate.toml` has `[tests]` with `exclude = ["**"]`, and the change edits a function whose spec is unchanged
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported
