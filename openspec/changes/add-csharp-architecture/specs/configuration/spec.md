# Spec Delta

## MODIFIED Requirements

### Requirement: Invalid configuration

When `qgate.toml` exists in the head revision but is not valid, the system
SHALL print an error naming `qgate.toml` and the problem to standard error,
print nothing to standard output and exit with code 2. Not valid means: not
TOML, an unknown table or key, a value of the wrong type, a rule with an
empty or duplicate `name`, an empty `from`, `to` or `to_namespaces` list or
neither `to` nor `to_namespaces`, an invalid pattern, or a `qgate.toml`
that is not a regular file.

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

#### Scenario: Rule without targets

- **GIVEN** a forbid rule in `qgate.toml` has a `name` and `from` but neither `to` nor `to_namespaces`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names the rule, `to` and `to_namespaces`
- **AND** the exit code is 2

#### Scenario: Only namespaces

- **GIVEN** a forbid rule in `qgate.toml` has `from` and `to_namespaces` but no `to`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no configuration error is printed and the exit code is not 2

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
