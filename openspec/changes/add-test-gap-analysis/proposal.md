# Proposal: add-test-gap-analysis

## Why

The partner pilot (an Angular frontend and a .NET backend on Azure
DevOps) should show all of M5. The first half is "code changed without
its tests". It is the cheapest risk signal a reviewer can get: a pull
request that changes a service but not its spec. Today qgate does not
look at tests at all. The README and `qgate-analyzers` already announce
a `tests` analyzer for M5. The order agreed on 2026-11-13 puts it next,
as 0.7.0, before hotspots (M5b, 0.8.0).

## What Changes

- **A new analyzer, `tests`.** It runs in revision mode, on the whole
  change, as duplication does. For each changed source file it looks
  for the file's test by naming convention, and reports when code
  changed but the test did not.
- **What counts as changed code.** A *changed unit* is a function unit
  of the head version that is new, or whose code differs from the unit
  of the same name in the base version. Whitespace, comments,
  semicolons and TypeScript types are ignored (types: decided during
  PR 1, design D1). Removed units, and edits outside function units (imports,
  types, fields), change nothing. So a file that only reorders imports
  or edits a comment needs no test change.
- **Where a source file's test is (built-in conventions):**
  - **TypeScript, Angular style:** `cart.service.ts` is tested by
    `cart.service.spec.ts` (or `.test.ts`) in the same directory. TSX
    likewise. A `*.spec.ts` or `*.test.ts` file is a test, never a
    source.
  - **C#, test projects:**
    - a `.cs` file belongs to the project of the nearest `.csproj`
      above it;
    - a project is a test project when the last segment of its name
      ends in `Test` or `Tests` (`Core.Test`, `Shop.Domain.UnitTests`);
    - the test projects of `Shop.Domain` are those named
      `Shop.Domain.<X>`, where `X` ends in `Test` or `Tests`;
    - `Order.cs` is tested by any `OrderTests.cs` or `OrderTest.cs` in
      those test projects, in any folder.

    Every `.cs` file of a test project is a test.
- **Two findings, one per source file, both Warning** (they never fail
  a check with the default `--fail-on error`):
  - `tests/unchanged-test`: the file has a test, units changed, and no
    test of the file was added or changed.

    Message: `Cart.total and 2 more changed, but not its test
    src/app/cart.service.spec.ts`.
  - `tests/missing-test`: the file has no test, its units are all new
    (its base version had no function units), and its project tests
    other code.

    Message: `Cart.total and 2 more have no test: expected
    src/app/cart.service.spec.ts` (`has no test` for a single unit).
    For C#: `... expected OrderTests.cs in Shop.Domain.UnitTests`.

  The finding sits at the first changed unit.
- **"Its project tests other code"** keeps repositories without tests
  quiet:
  - **C#:** the project has a test project;
  - **TypeScript:** the nearest directory with a `package.json` (or the
    repository root) contains a test file somewhere below it.
- **`[tests] exclude` in `qgate.toml`** lists source paths that need no
  test, such as `src/environments/**` or `**/Program.cs`. Same pattern
  syntax as `[files] exclude`.
- **What `[files] exclude` does here.** Files it excludes, generated C#
  files and unparsable files never need a test. Test files, however,
  are judged by path, regardless of `[files] exclude`. A team that
  excludes `**/*.spec.ts` from duplication still gets credit for
  changing a spec.
- **Every revision-mode entry point** gets the findings: `--worktree`,
  the Claude Code hook and the MCP server. The two-file form does not:
  it has no change set.
- **README:** a "Tests changed with the code" section. The M5 roadmap
  row records the first half.
- **Version 0.7.0,** including `action.yml`.

### Out of scope

- **Coverage.** No coverage files are read; this is about naming
  conventions only (a coverage-drop rule may come later).
- **Existing source files without a test** that the change edits. They
  are not reported; see Gate 1, question 3.
- **Deleted tests.** A change that deletes `OrderTests.cs` is not
  reported.
- **Other test layouts:**
  - `__tests__` directories;
  - a separate `test/` tree for TypeScript;
  - Angular templates (`*.component.html`) as changed code;
  - pairing by class name instead of file name in C#;
  - test projects named differently from their source project, such as
    eShop's `Ordering.UnitTests` for `Ordering.Domain`.

  Configurable pairing is question 1.
- **Telling whether the changed test actually exercises the changed
  code.**
- **JavaScript sources,** which qgate only reads for duplication.

### For Gate 1

Each question has a recommendation; design.md has the reasons.

1. **Pairing a file with its test.**
   - (a) The built-in conventions above, plus `[tests] exclude`.
   - (b) Also configurable pairing in `qgate.toml` now, for example
     `[[tests.pair]] source = "src/Ordering.*/**" tests =
     "tests/Ordering.UnitTests/**"`.

   *Recommended: (a)*, and the evaluation (question 7) shows whether
   (b) is needed before the pilot. **Question back:** do you know how
   the partner names its test projects and test classes? If they
   differ from `<Project>.<X>Tests` / `<Class>Tests.cs`, (b) moves into
   this change.
2. **What counts as a change.**
   - (a) New or changed function units, ignoring whitespace and
     comments.
   - (b) Any change to the file.

   *Recommended: (a).* (b) flags import reorders, comment edits and
   type-only edits, which need no test. (a) needs `qgate-lang` to tell
   whether a unit's code changed: each unit gets a fingerprint of its
   code (design D1).
3. **An existing source file without a test that the change edits.**
   - (a) Not reported.
   - (b) Info.
   - (c) Warning under `tests/missing-test`.

   *Recommended: (a) in 0.7.0.* The file was untested before; reporting
   it is close to an absolute score ("this file has no test"), and in a
   legacy codebase it would fire on almost every pull request. The
   evaluation counts these cases, so a later change can decide with
   data.
4. **Rule ids, severity and granularity.** `tests/unchanged-test` and
   `tests/missing-test`, both Warning, one finding per source file.
   *Recommended: yes.* Two ids mirror complexity's `increase`/`new`
   pair and can be filtered apart. The finding is never Error: a
   refactor can rightly leave its tests alone, so it is a hint for the
   reviewer, not a gate. Per file rather than per unit keeps a large
   edit to one service at one line in the PR comment.
5. **On by default.**
   - (a) On in every revision check, kept quiet by the "project tests
     other code" scope.
   - (b) Only when `qgate.toml` has a `[tests]` table.

   *Recommended: (a).* It matches complexity and duplication, which
   need no setup. Warnings do not fail by default, and `[tests] exclude
   = ["**"]` turns it off. A team that runs with `--fail-on warning`
   will see new failures after upgrading; the release notes say so.
6. **Test files and `[files] exclude`.** Test files count by path even
   when `[files] exclude` names them. *Recommended: yes* (see "What
   Changes").
7. **Evaluation before the release.** Run qgate over the last 30
   commits that touch code of each of:
   - `bitwarden/clients`: Angular, colocated `*.spec.ts`;
   - `bitwarden/server`: .NET, `test/Core.Test` for `src/Core`;
   - `jasontaylordev/CleanArchitecture`: .NET with `*.UnitTests` and
     `*.FunctionalTests`, and an Angular client.

   Sort every finding by hand into three groups:
   - a test should have changed;
   - a refactor that needed no test change;
   - a miss: the commit did change this code's test, under another
     name.

   Also record the time per commit, and the edited untested files of
   question 3. **Stop rule:** if misses are more than a third of a
   repository's findings, the release waits for your decision on
   pairing. `dotnet/eShop` runs once as a known mismatch, to measure
   what convention (a) misses. *Recommended: yes.*
8. **The rehearsal: Azure Pipelines.** Use the README's Linux job at
   `0.7.0` in the rehearsal project, on a pull request that changes a
   C# method and a TypeScript function without their tests. Expected:
   two Warning rows in the comment, two warning build issues, and the
   check step passes. *Recommended: yes.*
9. **Three pull requests, with 0.7.0 between the second and third.**
   - PR 1: unit fingerprints in `qgate-lang`. Small; nothing visible to
     users changes.
   - PR 2: the analyzer, `[tests] exclude`, CLI wiring, the README
     section, the version and the evaluation record.
   - PR 3: the README versions, the roadmap, the rehearsal and the
     archive.

   *Recommended: yes.* The previous changes used two pull requests, but
   PR 2 alone is already as large as the previous change's first.

**Decided at Gate 1 (2026-11-14):** Ervin approved the proposal with
every recommendation. The partner's test naming was not given, so
pairing stays with the built-in conventions (question 1 (a)). The
evaluation's stop rule decides whether configurable pairing or the
`<ProjectReference>` fallback (design D3) is needed.

## Capabilities

### New Capabilities

- `test-analysis`: which files are tests, how a source file is paired
  with its tests, what counts as changed code, and the two findings.

### Modified Capabilities

- `configuration`: the `[tests]` table with `exclude`. An invalid
  `tests.exclude` pattern is a configuration error. Without
  `qgate.toml`, no source is exempt.

## Impact

- **Code:**
  - `crates/qgate-lang`: `Function` gets a fingerprint of its code,
    for TypeScript and C#;
  - `crates/qgate-analyzers`: a new `tests` module. Like duplication,
    it works on the whole change rather than as an `Analyzer`. It gets
    paths and models only, no I/O;
  - `crates/qgate-config`: the `[tests]` table;
  - `crates/qgate-cli`: wiring, using the head file list that
    duplication already reads and git's raw changed paths, plus
    end-to-end tests.
- **No new dependency, no new ADR:** dependency directions are
  unchanged.
- **Output:** two new public rule ids. The JSON shape, report formats
  and CI scripts are unchanged.
- **Docs:** the README's new section, its roadmap and its layout line
  for `qgate-analyzers`.
