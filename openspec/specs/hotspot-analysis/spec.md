# hotspot-analysis Specification

## Purpose

Hotspot analysis gives findings context from git history: a finding in
a file that is both complex and changed often says so, so a reviewer
sees where something got worse in the code that is changed most. It
never reports a finding of its own and never changes a severity.

## Requirements

### Requirement: Hotspot context in revision mode

In revision mode, and so with `--worktree`, in the Claude Code hook and
in the MCP server, the system SHALL add hotspot context to every finding
located in a hotspot file, except `input/unparsable`. It SHALL NOT
report a finding of its own, nor change a finding's severity, the
`--fail-on` result or the exit code. The two-file form SHALL add no
hotspot context.

#### Scenario: A complexity increase in a hotspot

- **GIVEN** `src/Cart.cs` is a hotspot, and the change raises the complexity of `Cart.Total()` on line 12 from 4 to 6
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output contains `warning src/Cart.cs:12  Cart.Total(): cyclomatic complexity 4 -> 6 (+2); hotspot: 31 commits in 6 months, complexity 87  [complexity/cyclomatic-increase]`
- **AND** the exit code is 0

#### Scenario: A hotspot nothing made worse

- **GIVEN** `src/Cart.cs` is a hotspot, and the change edits a comment in it
- **WHEN** the user runs `qgate check --base main`
- **THEN** no finding is reported

#### Scenario: An unparsable hotspot

- **GIVEN** `src/Cart.cs` is a hotspot in the base revision, and the change leaves a syntax error in it
- **WHEN** the user runs `qgate check --base main`
- **THEN** its `input/unparsable` finding has no hotspot context

#### Scenario: Two-file form

- **WHEN** the user runs `qgate check before.ts after.ts` in a repository where `after.ts`'s path is a hotspot
- **THEN** no finding has hotspot context

### Requirement: The window

The window SHALL start 182 days before the committer date of the head
commit; with `--worktree`, of `HEAD`, from which the history is then
read. A commit SHALL be in the window when its committer date is after
the start. History behind a commit dated before the start is not read,
so with skewed clocks a commit in the window can be missed. The result
SHALL NOT depend on the time the check runs.

#### Scenario: The same check later

- **GIVEN** a repository whose history does not change
- **WHEN** the user runs `qgate check --base main` today and again a month later
- **THEN** both runs report the same hotspot context

#### Scenario: An old commit

- **GIVEN** a commit that changed `src/Cart.cs` 200 days before the head commit
- **THEN** it does not count for `src/Cart.cs`

#### Scenario: The edge of the window

- **GIVEN** one commit dated exactly 182 days before the head commit and one a second later, each changing `src/Cart.cs`
- **THEN** only the second counts

### Requirement: Churn

A file's churn SHALL be the number of non-merge commits in the window,
reachable from the head commit, that add, modify, rename or copy it,
counted under its path in the head revision through the renames in the
commit's own history. A rename made on another branch than a commit's
SHALL NOT be followed for it.

#### Scenario: Merge commits

- **GIVEN** three commits that change `src/Cart.cs`, merged into the head branch by a merge commit
- **THEN** they count three, and the merge commit does not count

#### Scenario: Renamed on another branch

- **GIVEN** `main` renames `src/Basket.cs` to `src/Cart.cs`, while a branch merged later edits `src/Basket.cs` twice
- **THEN** those two commits do not count for `src/Cart.cs`

#### Scenario: Renamed in the window

- **GIVEN** `src/Basket.cs` changed in four commits, then was renamed to `src/Cart.cs`, which changed in two more
- **THEN** the churn of `src/Cart.cs` is 7

### Requirement: Complexity of a file

A file's complexity SHALL be the sum of the cyclomatic complexity of its
function units in the head revision. A file that cannot be parsed, and
a changed file reported as `input/unparsable`, SHALL NOT be ranked.

#### Scenario: Two methods

- **GIVEN** `src/Cart.cs` has two units of complexity 3 and 5 in the head revision
- **THEN** its complexity is 8

### Requirement: Ranked files

The ranked files SHALL be the files of the head revision whose language
is supported, that `[files] exclude` does not name, that are not
generated C# and that are not tests by the conventions of
`test-analysis`, each language family ranked on its own: TypeScript
with TypeScript, C# with C#, Rust with Rust. Rust has no test
conventions, so every Rust file is ranked. A test's findings get no
hotspot context.

#### Scenario: Excluded migrations

- **GIVEN** `[files]` has `exclude = ["**/Migrations/**"]`, and migrations changed in every commit
- **THEN** no migration is ranked, and they push no other file out of the top

#### Scenario: Tests are not ranked

- **GIVEN** `src/cart.spec.ts` changed in more commits than `src/cart.ts` and is more complex, and the change makes both more complex
- **WHEN** the user runs `qgate check --base main`
- **THEN** the finding for `src/cart.ts` has hotspot context and the one for `src/cart.spec.ts` has none

#### Scenario: Families apart

- **GIVEN** a repository with 40 C# files and 400 TypeScript files
- **THEN** a C# file's rank is among the 40 C# files only

#### Scenario: Rust files

- **GIVEN** a repository with 30 Rust files, among them `crates/cli/tests/check.rs`, an integration test
- **THEN** each of the 30 is ranked among the Rust files, `crates/cli/tests/check.rs` too

### Requirement: Hotspot files

A ranked file SHALL be a hotspot when all of these hold:
- its churn is at least 5;
- fewer than 10% of its family's ranked files have a higher churn;
- its complexity is at least 1;
- fewer than 25% of its family's ranked files have a higher
  complexity.

#### Scenario: Changed often and complex

- **GIVEN** 40 ranked C# files, of which `src/Cart.cs` has the second highest churn, 31, and the fifth highest complexity, 87
- **THEN** `src/Cart.cs` is a hotspot

#### Scenario: Changed often but simple

- **GIVEN** `src/Dto.cs` has the highest churn, but more than a quarter of the ranked files are more complex
- **THEN** `src/Dto.cs` is not a hotspot

#### Scenario: Complex but rarely changed

- **GIVEN** `src/Legacy.cs` has the highest complexity and changed in 4 commits in the window
- **THEN** `src/Legacy.cs` is not a hotspot

#### Scenario: Ties

- **GIVEN** 20 ranked files, of which `src/A.ts` and `src/B.ts` share the highest churn, 9, and are both complex enough
- **THEN** both are hotspots

### Requirement: The hotspot note

The note SHALL be appended to the finding's message as `; hotspot: <n>
commits in 6 months, complexity <c>`, with the file's churn and
complexity, and SHALL appear wherever the message does: text,
Markdown, Azure Pipelines and GitHub Actions annotations, and JSON.

#### Scenario: Markdown

- **GIVEN** the change of "A complexity increase in a hotspot"
- **WHEN** the user runs `qgate check --base main --format markdown`
- **THEN** the finding's cell reads `Cart.Total(): cyclomatic complexity 4 -\> 6 (+2); hotspot: 31 commits in 6 months, complexity 87`, with `>` escaped as for every cell

#### Scenario: Azure Pipelines

- **GIVEN** the same change
- **WHEN** the user runs `qgate check --base main --format azure`
- **THEN** the warning's message ends with `; hotspot: 31 commits in 6 months, complexity 87`

### Requirement: Shallow history

When the repository is shallow and the check has a changed file to
analyze, the system SHALL add no hotspot context, and `qgate check`
SHALL print `qgate: the history is shallow; hotspots not computed` to
standard error once; the hook and the MCP server print nothing more. Standard output, the findings and the exit
code SHALL be as without hotspot analysis.

#### Scenario: A shallow clone in CI

- **GIVEN** a clone made with `git clone --depth 50` in which the base revision is present
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error contains `qgate: the history is shallow; hotspots not computed`
- **AND** no finding has hotspot context, and the exit code is that of the findings
