## MODIFIED Requirements

### Requirement: Configuration from the head revision

In revision mode the system SHALL read the configuration from the file
`qgate.toml` at the root of the repository as it is in the head revision.
Without `--worktree`, uncommitted changes to the file SHALL NOT affect the
result. With `--worktree`, the working tree is the head revision (see
"Working tree as head" in `revision-input`), so the file SHALL be read as
it is on disk, and an untracked `qgate.toml` that is not ignored counts.

#### Scenario: Rules come from the head revision

- **GIVEN** branch `feature` adds a forbid rule to `qgate.toml` and an import that the rule forbids
- **WHEN** the user runs `qgate check --base main --head feature` with `main` checked out
- **THEN** the forbidden import is reported

#### Scenario: Uncommitted changes are ignored

- **GIVEN** the head revision has no `qgate.toml`, and the working tree has one with a forbid rule that the changed files break
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported

#### Scenario: Uncommitted changes count with the working tree

- **GIVEN** the head revision has no `qgate.toml`, and the working tree has an untracked one with a forbid rule that an uncommitted import breaks
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** the forbidden import is reported

#### Scenario: Invalid configuration on disk

- **GIVEN** `qgate.toml` is valid in `HEAD` but has a syntax error on disk
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard error reports the invalid `qgate.toml`
- **AND** the exit code is 2
