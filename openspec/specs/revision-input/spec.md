# revision-input Specification

## Purpose

Revision input lets qgate take the code to analyze straight from git: it
decides which two versions are compared, which changed files are analyzed
and how, and how git-level problems and unparsable files are reported.

## Requirements

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
revisions and whose head path has the extension `.ts`, `.mts`, `.cts`,
`.tsx` or `.cs`, except generated C# files (see `csharp-analysis`).
Other files SHALL be ignored without a message. All changed files in
the repository count, regardless of the current directory.

#### Scenario: Unchanged and unsupported files

- **GIVEN** the head changes `src/a.ts` and `README.md`, and the repository also contains an unchanged, complex `src/b.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** only `src/a.ts` is analyzed

#### Scenario: C# files

- **GIVEN** the head changes `src/Cart.cs`, `src/Cart.Designer.cs` and `src/Shop.csproj`
- **WHEN** the user runs `qgate check --base main`
- **THEN** only `src/Cart.cs` is analyzed

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
supported extension, the file SHALL be treated as added, except that a
JavaScript version (`.js`, `.jsx`, `.mjs`, `.cjs`) supplies its imports
for the architecture rules; if that version cannot be read, no import of
the file counts as new, wherever it moved.

#### Scenario: Renamed and modified

- **GIVEN** the head renames `src/old.ts` to `src/new.ts` and makes `f` in it more complex
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `complexity/cyclomatic-increase` finding is reported for `f` in `src/new.ts`

#### Scenario: Renamed from an unsupported extension

- **GIVEN** the head renames `src/util.coffee` or `src/util.js` to `src/util.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** every function in `src/util.ts` is analyzed as new

#### Scenario: Renamed from JavaScript that cannot be read

- **GIVEN** a forbid rule, and the head renames `src/domain/order.js`, which is not valid UTF-8 and imports a forbidden target, to `src/domain/order.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported for `src/domain/order.ts`
- **AND** its functions are analyzed as new

#### Scenario: Unreadable JavaScript moved into a forbidden part

- **GIVEN** a rule that forbids `src/domain/**` to import `src/ui/**`, and the head renames `src/app/order.js`, which is not valid UTF-8 and imports `"../ui/Button"`, to `src/domain/order.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported

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

### Requirement: The working tree replaces the head everywhere

With `--worktree`, everything this and the other capabilities say about the
head revision SHALL apply to the working tree: which files changed, rename
detection, the configuration and the files that duplication compares
against.

#### Scenario: Copy of an uncommitted file

- **GIVEN** an untracked `src/util.ts` contains a 60-token block, and the uncommitted `src/a.ts` copies it
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** a `duplication/duplicate-block` finding is reported that names both files

#### Scenario: Copy of a file deleted from disk

- **GIVEN** the checked-out branch committed `src/old.ts` with a 60-token block, `src/old.ts` was then deleted from disk, and an untracked `src/b.ts` holds the same block
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** no `duplication/duplicate-block` finding is reported

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
refs, the stash, the reflog or any file in the working tree. A completed
check SHALL leave nothing behind but storage for the snapshot's contents: objects no ref
reaches and, under a clean filter such as Git LFS, that filter's store. It
SHALL NOT run git hooks, apart from a configured file-system monitor that
git asks which files changed.

#### Scenario: Staged changes stay staged

- **GIVEN** `src/a.ts` has a staged change, `src/b.ts` an unstaged one and `src/c.ts` is untracked
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** `git status --porcelain` prints the same as before the check
- **AND** `git diff --cached` prints the same as before the check

#### Scenario: No commit appears

- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** `HEAD`, every ref and the reflog of `HEAD` are the same as before the check

#### Scenario: Hooks do not run

- **GIVEN** the repository has `pre-commit`, `post-commit` and `post-index-change` hooks that write a file when they run
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** none of the files exists after the check

#### Scenario: Split index

- **GIVEN** the repository uses `core.splitIndex`, and most tracked files changed on disk
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** the git directory holds the same `sharedindex.*` files as before the check

### Requirement: Interrupted checks

A check with `--worktree` that is interrupted, for example killed by a
timeout, MAY leave a private copy of the index in the git directory. A later
check with `--worktree` SHALL remove such copies once they are more than a
day old, and SHALL NOT remove younger ones, which a running check may use.

#### Scenario: Copy left by an interrupted check

- **GIVEN** an interrupted check left a private copy of the index two days ago, and a check that started a minute ago is still running
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** the two-day-old copy is gone
- **AND** the running check's copy is still there

### Requirement: Working tree errors

With `--worktree`, besides the errors of "Git errors", the system SHALL
report on standard error, print nothing to standard output and exit with
code 2 when `HEAD` names no commit, or when no snapshot of the working tree
can be taken. Messages about an unknown `HEAD`, a missing merge base or a
`qgate.toml` that is not a regular file SHALL name the head as `HEAD` or
the working tree, never by an internal id.

#### Scenario: Repository without commits

- **GIVEN** a repository that has no commit yet and an untracked `src/a.ts`
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard error names `HEAD` as an unknown revision
- **AND** it does not suggest fetching the base
- **AND** standard output is empty
- **AND** the exit code is 2

#### Scenario: Base HEAD in a repository without commits

- **GIVEN** a repository that has no commit yet and an untracked `src/a.ts`
- **WHEN** the user runs `qgate check --base HEAD --worktree`
- **THEN** standard error names `HEAD` as an unknown revision
- **AND** it does not suggest fetching the base
- **AND** the exit code is 2

#### Scenario: qgate.toml is not a regular file on disk

- **GIVEN** `qgate.toml` is a regular file in `HEAD`, and a directory on disk
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard error says that `qgate.toml` is not a regular file in the working tree
- **AND** the exit code is 2

#### Scenario: Unreadable file in the working tree

- **GIVEN** an untracked, not ignored `src/secret.ts` that the user cannot read
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** standard error says that no snapshot of the working tree could be taken, with git's message naming `src/secret.ts`
- **AND** standard output is empty
- **AND** the exit code is 2

#### Scenario: Unrelated histories with the working tree

- **GIVEN** `other` has no common ancestor with `HEAD`
- **WHEN** the user runs `qgate check --base other --worktree`
- **THEN** standard error says that `other` and `HEAD` have no merge base
- **AND** the exit code is 2
