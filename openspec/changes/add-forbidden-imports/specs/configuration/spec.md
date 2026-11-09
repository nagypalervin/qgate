# Spec Delta

## ADDED Requirements

### Requirement: Configuration from the head revision

In revision mode the system SHALL read the configuration from the file
`qgate.toml` at the root of the repository as it is in the head revision.
Uncommitted changes to the file SHALL NOT affect the result.

#### Scenario: Rules come from the head revision

- **GIVEN** branch `feature` adds a forbid rule to `qgate.toml` and an import that the rule forbids
- **WHEN** the user runs `qgate check --base main --head feature` with `main` checked out
- **THEN** the forbidden import is reported

#### Scenario: Uncommitted changes are ignored

- **GIVEN** the head revision has no `qgate.toml`, and the working tree has one with a forbid rule that the changed files break
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported

### Requirement: Missing configuration

When the head revision has no `qgate.toml`, the system SHALL apply no
architecture rules and otherwise behave as without a configuration file.

#### Scenario: Repository without qgate.toml

- **GIVEN** a repository with no `qgate.toml` in which `feature` makes a function more complex
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** the output is the same as before this change

### Requirement: Invalid configuration

When `qgate.toml` exists in the head revision but is not valid, the system
SHALL print an error naming `qgate.toml` and the problem to standard error,
print nothing to standard output and exit with code 2. Not valid means: not
TOML, an unknown table or key, a value of the wrong type, a rule with an
empty or duplicate `name`, an empty `from` or `to` list, an invalid
pattern, or a `qgate.toml` that is not a regular file.

#### Scenario: Syntax error

- **GIVEN** line 3 of `qgate.toml` is `name = "domain-is-pure`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml` and line 3
- **AND** the exit code is 2

#### Scenario: Misspelled table

- **GIVEN** `qgate.toml` contains `[[architecture.forbd]]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml` and `forbd`
- **AND** the exit code is 2

#### Scenario: Duplicate rule name

- **GIVEN** two forbid rules in `qgate.toml` are both named `domain-is-pure`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `domain-is-pure` as a duplicate
- **AND** the exit code is 2

#### Scenario: Empty pattern list

- **GIVEN** a forbid rule in `qgate.toml` has `to = []`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names the rule and `to`
- **AND** the exit code is 2

#### Scenario: Invalid pattern

- **GIVEN** a forbid rule in `qgate.toml` has `from = ["src/[domain"]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names the rule and the pattern `src/[domain`
- **AND** the exit code is 2

#### Scenario: Not a regular file

- **GIVEN** `qgate.toml` is a directory in the head revision
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names `qgate.toml`
- **AND** the exit code is 2

### Requirement: The two-file form ignores the configuration

`qgate check <before> <after>` SHALL NOT read `qgate.toml` and SHALL NOT
apply architecture rules.

#### Scenario: Invalid qgate.toml next to two files

- **GIVEN** the current directory contains a `qgate.toml` that is not TOML
- **WHEN** the user runs `qgate check before.ts after.ts` on files with no findings
- **THEN** standard output is `No regressions found.`
- **AND** the exit code is 0
