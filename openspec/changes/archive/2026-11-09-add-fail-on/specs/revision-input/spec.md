# Spec Delta

## MODIFIED Requirements

### Requirement: Unparsable files are reported, not fatal

In revision mode, when either version of a selected file cannot be parsed or
is not valid UTF-8, the system SHALL skip that file, report one Warning
finding with rule `input/unparsable` for its head path and continue with the
other files. The finding affects the exit code like any other Warning (see
"Failing on findings" in the `check-command` capability): it fails the run
only with `--fail-on warning`.

#### Scenario: Syntax error in the head version

- **GIVEN** the head version of `src/a.ts` has its first syntax error on line 12, and `src/b.ts` got more complex by less than 5 (a Warning)
- **WHEN** the user runs `qgate check --base main`
- **THEN** the output contains `warning src/a.ts:12  file not analyzed: syntax error on line 12 (or syntax the parser does not support yet)  [input/unparsable]`
- **AND** it also contains the finding for `src/b.ts`
- **AND** the exit code is 0

#### Scenario: Syntax error only in the base version

- **GIVEN** the merge-base version of `src/a.ts` has its first syntax error on line 3 and the head version is valid
- **WHEN** the user runs `qgate check --base main`
- **THEN** the output contains `warning src/a.ts  file not analyzed: syntax error on line 3 of the base version (or syntax the parser does not support yet)  [input/unparsable]`

#### Scenario: Not valid UTF-8

- **GIVEN** the head version of `src/a.ts` is not valid UTF-8
- **WHEN** the user runs `qgate check --base main`
- **THEN** the output contains `warning src/a.ts  file not analyzed: not valid UTF-8  [input/unparsable]`

#### Scenario: Base version not valid UTF-8

- **GIVEN** the merge-base version of `src/a.ts` is not valid UTF-8 and the head version is valid
- **WHEN** the user runs `qgate check --base main`
- **THEN** the output contains `warning src/a.ts  file not analyzed: the base version is not valid UTF-8  [input/unparsable]`

#### Scenario: Both versions broken

- **GIVEN** both versions of `src/a.ts` have syntax errors, the head version first on line 7
- **WHEN** the user runs `qgate check --base main`
- **THEN** the only finding for `src/a.ts` is the `input/unparsable` finding for line 7 of the head version
