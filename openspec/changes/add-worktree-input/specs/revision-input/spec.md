## ADDED Requirements

### Requirement: Working tree as head

With `--worktree`, the system SHALL use the working tree as the head
revision: every file with the contents it has on disk, staged or not, plus
the untracked files that git's ignore rules do not exclude. A tracked file
missing from disk SHALL count as deleted. Files the index marks
`skip-worktree` or `assume-unchanged` SHALL count as unchanged, as git
status shows them. The before side SHALL be the merge base of `--base` and
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

- **GIVEN** the tracked `src/legacy.ts` was deleted from disk without `git rm`
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** no finding is reported for `src/legacy.ts`

### Requirement: The working tree replaces the head everywhere

With `--worktree`, everything this and the other capabilities say about the
head revision SHALL apply to the working tree: which files changed, rename
detection, the configuration and the files that duplication compares
against.

#### Scenario: Copy of an uncommitted file

- **GIVEN** an untracked `src/util.ts` contains a 60-token block, and the uncommitted `src/a.ts` copies it
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** a `duplication/duplicate-block` finding is reported that names both files

#### Scenario: Only the uncommitted changes

- **GIVEN** the checked-out branch committed a complex function, and the working tree adds another one in a different file
- **WHEN** the user runs `qgate check --base HEAD --worktree`
- **THEN** only the uncommitted function is reported

#### Scenario: Clean working tree

- **GIVEN** the working tree has no uncommitted changes and no untracked files
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** the output is the same as for `qgate check --base main`

### Requirement: Checking the working tree changes nothing

A check with `--worktree` SHALL NOT change the index (what is staged), `HEAD`,
branches or other refs, the stash, the reflog or any file in the working
tree, and SHALL NOT leave a file behind in the repository, apart from
objects in git's object store that no ref reaches. It SHALL NOT run git hooks.

#### Scenario: Staged changes stay staged

- **GIVEN** `src/a.ts` has a staged change, `src/b.ts` an unstaged one and `src/c.ts` is untracked
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** `git status --porcelain` prints the same as before the check
- **AND** `git diff --cached` prints the same as before the check

#### Scenario: No commit appears

- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** `HEAD`, every ref and the reflog of `HEAD` are the same as before the check

#### Scenario: Hooks do not run

- **GIVEN** the repository has a `pre-commit` and a `post-commit` hook that write a file when they run
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** neither file exists after the check

### Requirement: Working tree errors

With `--worktree`, besides the errors of "Git errors", the system SHALL
report on standard error, print nothing to standard output and exit with
code 2 when `HEAD` names no commit, or when git cannot read the working
tree. Error messages SHALL name the head as `HEAD` or the working tree,
never by an internal id.

#### Scenario: Repository without commits

- **GIVEN** a repository that has no commit yet and an untracked `src/a.ts`
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard error names `HEAD` as an unknown revision
- **AND** it does not suggest fetching the base
- **AND** standard output is empty
- **AND** the exit code is 2

#### Scenario: Unreadable file in the working tree

- **GIVEN** an untracked, not ignored `src/secret.ts` that the user cannot read
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard error says that the working tree could not be read, with git's message naming `src/secret.ts`
- **AND** standard output is empty
- **AND** the exit code is 2

#### Scenario: Unrelated histories with the working tree

- **GIVEN** `other` has no common ancestor with `HEAD`
- **WHEN** the user runs `qgate check --base other --worktree`
- **THEN** standard error says that `other` and `HEAD` have no merge base
- **AND** the exit code is 2
