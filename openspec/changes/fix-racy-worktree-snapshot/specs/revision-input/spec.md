# Spec Delta

## MODIFIED Requirements

### Requirement: Working tree as head

With `--worktree`, the system SHALL use the working tree as the head
revision: every file as git would stage it from disk (after filters such as
line-ending conversion), staged or not, plus the untracked files that git's
ignore rules do not exclude. A tracked file missing from disk SHALL count as
deleted; files the index marks `skip-worktree` or `assume-unchanged` SHALL
count as unchanged. The before side SHALL be the merge base of `--base` and
`HEAD`.

#### Scenario: Uncommitted edit

- **GIVEN** `HEAD` equals `main`, and `f` in `src/a.ts` was made more complex on disk without a commit
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** a `complexity/cyclomatic-increase` finding is reported for `f` in `src/a.ts`

#### Scenario: Without the flag uncommitted edits are not seen

- **GIVEN** the same uncommitted edit
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output is `No regressions found.`

#### Scenario: Committed and uncommitted changes together

- **GIVEN** the checked-out branch committed a new import in `src/a.ts`, and `f` in `src/b.ts` was made more complex on disk without a commit
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** findings are reported for both files, as if both changes had been committed

#### Scenario: Untracked new file

- **GIVEN** an untracked `src/new.ts` with one function of complexity 12
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** one `complexity/cyclomatic-new` Warning is reported for `src/new.ts`

#### Scenario: Ignored file

- **GIVEN** `.gitignore` lists `dist/`, and `dist/out.ts` contains a function of complexity 30
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** no finding is reported for `dist/out.ts`

#### Scenario: The disk version wins over the staged one

- **GIVEN** `src/a.ts` was staged with `f` at complexity 3 and then edited on disk to complexity 9, while `f` has complexity 2 on `main`
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** the `complexity/cyclomatic-increase` finding for `f` reports `2 -> 9`

#### Scenario: File moved without git

- **GIVEN** `src/old.ts` was moved to `src/new.ts` with a plain file move (not `git mv`), and `f` in it was made more complex
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** a `complexity/cyclomatic-increase` finding is reported for `f` in `src/new.ts`
- **AND** no `complexity/cyclomatic-new` finding is reported for the file's other functions

#### Scenario: Deleted from disk

- **GIVEN** the checked-out branch added `src/legacy.ts` with a function of complexity 30, and it was then deleted from disk without `git rm`
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** no finding is reported for `src/legacy.ts`

#### Scenario: Files git does not look at

- **GIVEN** the index marks `src/a.ts` `assume-unchanged` and `src/b.ts` `skip-worktree`, and a function in each was made more complex on disk
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** no finding is reported for `src/a.ts` or `src/b.ts`

#### Scenario: An edit in the same instant as the index write

- **GIVEN** `f` in `src/a.ts` was made more complex on disk without changing the file's size, and the file's modification time equals both the time recorded for it in the index and the index file's own time, as when the edit lands in the same file-system timestamp as the last index write
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** a `complexity/cyclomatic-increase` finding is reported for `f` in `src/a.ts`
