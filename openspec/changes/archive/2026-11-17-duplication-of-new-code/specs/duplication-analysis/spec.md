# Spec Delta

## MODIFIED Requirements

### Requirement: Only new duplication is reported

A duplicate block SHALL be reported only if the change increased the
number of files that hold it, only in files that did not hold it before,
at most as many as it added, and only if at least 50 of its tokens are
new: on lines the change wrote, not covered by duplicated code the file
already had, nor by a window whose block an earlier finding in the same
file reported. A file that holds it with none of its tokens on lines
the change wrote SHALL NOT count as a new copy: the block moved there.
When every file that holds it is new and none of them is such a file,
the first by path SHALL count as the original.

#### Scenario: Existing duplication

- **GIVEN** two files already share a 60-token block, and the change edits one of them elsewhere
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Moved code

- **GIVEN** the change moves a 60-token block from `src/a.ts` into `src/b.ts`, and no other copy exists
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Code written next to shared code

- **GIVEN** `src/a.ts` and `src/u.ts` share a 60-token block, and the change adds a function to `src/a.ts` right above it
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: Copy within the change

- **GIVEN** the change adds the same 60-token block to new files `src/a.ts` and `src/b.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** one finding is reported, for `src/b.ts`, naming `src/a.ts`

## ADDED Requirements

### Requirement: Lines the change wrote

A line the change wrote SHALL be a line of a changed file's head version
that the change added and that did not move. Added and removed lines
SHALL come from a line diff of the file's two versions, the base being
what counts as before. Every line of a file without one SHALL be added,
every line of a deleted file removed, and a renamed file SHALL be
diffed like an edited one. Lines SHALL be compared by their text
without leading and trailing white space.

#### Scenario: The same small edit in existing copies

- **GIVEN** `src/a.spec.ts` and `src/b.spec.ts` already share a 60-token block, and the change adds one argument to a constructor call on one line of the block in both files
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: A copy completed by a few written lines

- **GIVEN** `src/u.ts` holds a 60-token block, `src/a.ts` holds its first 30 tokens, and the change writes the other 30 into `src/a.ts` right after them
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported, because only 30 of the block's tokens are on lines the change wrote

#### Scenario: A new copy in an edited file

- **GIVEN** `src/u.ts` holds a 60-token block, and the change writes a copy of it into the existing `src/a.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `src/a.ts` names `src/u.ts`

### Requirement: Moved lines

Three consecutive added lines of a file SHALL count as moved when they
match three consecutive removed lines, of the same file or another, and
at least one of the three is not empty. Each run of removed lines SHALL
match one run of added lines only: first one in the file it was removed
from, then one in other files, in path and line order.

#### Scenario: A split file

- **GIVEN** `src/big.ts` holds the same 60-token block twice, and the change deletes it and adds `src/one.ts` and `src/two.ts`, each with one of the two copies indented one level deeper, among other code moved from `src/big.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `duplication/duplicate-block` finding is reported

#### Scenario: A helper copied into two files

- **GIVEN** `src/old.ts` holds a 60-token helper once, and the change deletes it and adds `src/a.ts` and `src/b.ts`, both with the helper
- **WHEN** the user runs `qgate check --base main`
- **THEN** one finding is reported, for `src/b.ts`, naming `src/a.ts`

#### Scenario: Moved within a file and copied

- **GIVEN** the change moves a 60-token block from the top to the bottom of `src/a.ts`, and adds a copy of it to the new `src/0.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `src/0.ts` names `src/a.ts`

#### Scenario: Moved and copied next to an existing copy

- **GIVEN** `src/a.ts` and `src/c.ts` share a 60-token block, and the change deletes `src/a.ts` and adds `src/b.ts` and `src/d.ts`, both with the block
- **WHEN** the user runs `qgate check --base main`
- **THEN** one finding is reported, for `src/d.ts`, naming `src/b.ts`

#### Scenario: Moved and renamed

- **GIVEN** `src/big.ts` holds the same 60-token block twice, and the change deletes it and adds `src/one.ts` and `src/two.ts`, each with one of the copies, and renames a variable on every line of the copy in `src/two.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `duplication/duplicate-block` finding for `src/two.ts` names `src/one.ts`
