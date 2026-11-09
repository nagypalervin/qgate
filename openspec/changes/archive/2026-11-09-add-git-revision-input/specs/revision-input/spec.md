# Spec Delta

## Purpose

Revision input lets qgate take the code to analyze straight from git: it
decides which two versions are compared, which changed files are analyzed
and how, and how git-level problems and unparsable files are reported.

## ADDED Requirements

### Requirement: Merge-base comparison

In revision mode the system SHALL compare the merge base of `--base` and
`--head` (the before side) with `--head` (the after side), so that changes
made on the base branch after the head branched off are not attributed to
the head.

#### Scenario: Base branch moved on

- **GIVEN** `main` simplified `f` in `src/a.ts` after `feature` branched off, and `feature` did not touch `src/a.ts`
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** no finding is reported for `src/a.ts`

#### Scenario: Head is a descendant of base

- **GIVEN** `feature` is `main` plus one commit that makes `f` more complex
- **WHEN** the user runs `qgate check --base main --head feature`
- **THEN** a `complexity/cyclomatic-increase` finding is reported for `f`

### Requirement: Only changed, supported files are analyzed

The system SHALL analyze only files that differ between the two compared
revisions and whose head path has the extension `.ts`, `.mts`, `.cts` or
`.tsx`. Other files SHALL be ignored without a message. All changed files in
the repository count, regardless of the current directory.

#### Scenario: Unchanged and unsupported files

- **GIVEN** the head changes `src/a.ts` and `README.md`, and the repository also contains an unchanged, complex `src/b.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** only `src/a.ts` is analyzed

#### Scenario: No changes

- **WHEN** the user runs `qgate check --base HEAD --head HEAD`
- **THEN** standard output is `No regressions found.`
- **AND** the exit code is 0

### Requirement: Added files

An added file, and a regular file that replaced a symbolic link or a
submodule at the same path, SHALL be compared against an empty before
version, so that every function in it counts as new.

#### Scenario: New file with a complex function

- **GIVEN** the head adds `src/new.ts` with one function of complexity 12 and one of complexity 3
- **WHEN** the user runs `qgate check --base main`
- **THEN** one `complexity/cyclomatic-new` Warning is reported, for the function of complexity 12

#### Scenario: Symbolic link replaced by a file

- **GIVEN** `src/link.ts` was a symbolic link on `main`, and the head replaces it with a regular file containing a function of complexity 12
- **WHEN** the user runs `qgate check --base main`
- **THEN** one `complexity/cyclomatic-new` Warning is reported for `src/link.ts`

### Requirement: Renamed files

A file that git detects as renamed SHALL be compared with its version under
the old path, and findings SHALL use the new path. If the old path has no
supported extension, the file SHALL be treated as added.

#### Scenario: Renamed and modified

- **GIVEN** the head renames `src/old.ts` to `src/new.ts` and makes `f` in it more complex
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `complexity/cyclomatic-increase` finding is reported for `f` in `src/new.ts`

#### Scenario: Renamed from an unsupported extension

- **GIVEN** the head renames `src/util.js` to `src/util.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** `src/util.ts` is analyzed as an added file

### Requirement: Skipped changes

The system SHALL skip deleted files, symbolic links, submodules and
regular files that became a symbolic link or a submodule, without a finding
or a message.

#### Scenario: Deleted file

- **GIVEN** the head deletes `src/legacy.ts`, which had complex functions
- **WHEN** the user runs `qgate check --base main`
- **THEN** no finding is reported for `src/legacy.ts`

### Requirement: Unparsable files are reported, not fatal

In revision mode, when either version of a selected file cannot be parsed or
is not valid UTF-8, the system SHALL skip that file, report one Warning
finding with rule `input/unparsable` for its head path, continue with the
other files and exit with code 0.

#### Scenario: Syntax error in the head version

- **GIVEN** the head version of `src/a.ts` has its first syntax error on line 12, and `src/b.ts` got more complex
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

### Requirement: Git errors

The system SHALL report the following as an error on standard error, print
nothing to standard output and exit with code 2: the current directory is
not inside a git repository, `git` cannot be run, a revision does not name a
commit, or the two revisions have no merge base.

#### Scenario: Unknown base revision

- **WHEN** the user runs `qgate check --base no-such-branch`
- **THEN** standard error names `no-such-branch` as an unknown revision
- **AND** it suggests fetching the base, for example with `fetch-depth: 0` in GitHub Actions
- **AND** the exit code is 2

#### Scenario: Unknown head revision

- **WHEN** the user runs `qgate check --base main --head no-such-branch`
- **THEN** standard error names `no-such-branch` as an unknown revision
- **AND** it does not suggest fetching the base
- **AND** the exit code is 2

#### Scenario: Not a repository

- **WHEN** the user runs `qgate check --base main` in a directory that is not inside a git repository
- **THEN** standard error says that it is not a git repository
- **AND** the exit code is 2

#### Scenario: Unrelated histories

- **GIVEN** `other` has no common ancestor with `HEAD`
- **WHEN** the user runs `qgate check --base other`
- **THEN** standard error says that the revisions have no merge base
- **AND** the exit code is 2

### Requirement: Revisions are never options

The system SHALL treat the values of `--base` and `--head` only as
revisions, never as options to `git`, even when they start with `-`.

#### Scenario: Revision that looks like an option

- **WHEN** the user runs `qgate check --base=--output=/tmp/x`
- **THEN** standard error names `--output=/tmp/x` as an unknown revision
- **AND** no file is written
- **AND** the exit code is 2
