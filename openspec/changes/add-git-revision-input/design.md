# Design: add-git-revision-input

## Context

See `proposal.md` for the why and the decisions taken during exploration,
and the spec deltas (`check-command` modified, `revision-input` new) for the
observable behavior.

Current state that shapes the approach:

- `qgate check <before> <after>` reads two files in `qgate-cli`, parses them
  with `qgate_lang::parse`, runs `ComplexityAnalyzer` on one `FileChange`,
  sorts by `(line, rule)` and renders with `qgate_report::render_text`, which
  already prints a finding without a line as the bare path.
- `FileChange { path, before: FileModel, after: FileModel }` was marked
  provisional "until git input"; exploration showed it is sufficient (an
  added file has an empty before model).
- Constraints from CLAUDE.md: `core` has no I/O and no git; libraries return
  errors and never print; dependency direction `cli -> report, analyzers ->
  lang -> core`; a structural change needs an ADR; few transitive
  dependencies.
- `git` 2.43 locally, recent git on GitHub runners. No recent git feature
  is required at run time (see the PR 1 review notes).

## Goals / Non-Goals

**Goals:**

- A `qgate-git` crate that turns "base, head" into a list of changed regular
  files with blob ids, and reads blobs. No knowledge of languages, parsing or
  findings.
- The CLI keeps the role it has: choose files, parse, analyze, sort, render,
  map errors to exit codes.
- Every git interaction is tested against real, temporary repositories.

**Non-Goals:**

- A general git library. Only what revision mode needs.
- Performance tuning beyond "fine for a pull request" (one process per blob).

## Decisions

### D1: Call `git`, do not link `gix` (ADR 0002)

Measured with a scratch crate against MSRV 1.85 (`gix` 0.87; 0.90 needs a
newer Rust): 202 crates with default features, 155 with `revision` and
`blob-diff`, 131 with `revision` alone. The workspace has about 25 external
crates. Calling `git` adds none and gives git's own merge base and rename
detection, which is what users compare qgate's output against.

Revisit when per-call process cost matters, for example an MCP server (M5)
answering many small queries. `git cat-file --batch` is the first step
before a library.

### D2: `qgate-git` is a leaf crate

```
cli --> git                 (new; git depends on nothing in the workspace)
cli --> report, analyzers --> lang --> core
```

Public API (all documented, errors returned, nothing printed):

```rust
pub struct Repository { /* root: PathBuf */ }

impl Repository {
    /// The repository containing `dir` (`git rev-parse --show-toplevel`).
    pub fn discover(dir: &Path) -> Result<Self, GitError>;
    /// Files changed from merge-base(base, head) to head.
    pub fn changed_files(&self, base: &str, head: &str)
        -> Result<Vec<ChangedFile>, GitError>;
    /// Raw contents of a blob.
    pub fn read_blob(&self, id: &BlobId) -> Result<Vec<u8>, GitError>;
}

pub struct ChangedFile {
    pub path: String,             // head path, relative to the repository root
    pub old_path: Option<String>, // set for renames
    pub before: Option<BlobId>,   // None for added files
    pub after: BlobId,
}

pub enum GitError {
    GitNotFound,
    NotARepository { detail: String },
    UnknownRevision { revision: String },
    NoMergeBase { base: String, head: String },
    Command { args: Vec<String>, status: Option<i32>, stderr: String },
}
```

`changed_files` returns only regular files (modes `100644`, `100755`) that
are modified, added or renamed. Deleted files, symlinks (`120000`),
submodules (`160000`) and type changes (`T`) are dropped here, because they
are facts about git, not about languages. The extension filter belongs to the
CLI, which knows the languages.

*Alternative considered:* return contents directly. Rejected: the CLI would
then read images and lock files only to throw them away.

### D3: Git invocations

| Step | Command | Why |
|---|---|---|
| repository | `git rev-parse --show-cdup --show-toplevel` (in `<dir>`) | failure = not a repository; later commands run in `<dir>/<cdup>` |
| resolve | reject `<rev>` starting with `-`, then `git rev-parse --verify --quiet <rev>^{commit}` | revisions can never become options; result is a full commit id |
| merge base | `git merge-base <base-id> <head-id>` | exit 1 with no output = no merge base |
| diff | `git diff --raw -z -M --no-ext-diff --no-textconv <mb-id> <head-id>` | NUL-separated, modes and blob ids included, renames detected |
| read | `git cat-file blob <blob-id>` | raw bytes, no textconv, no path round-trip |

- After resolution only full commit ids reach `merge-base` and `diff`, so
  user input never appears there.
- Spawning fails with `NotFound` → `GitError::GitNotFound`.
- `--raw -z` records are `:<old-mode> <new-mode> <old-id> <new-id> <status>`
  `\0<path>\0`, plus a second path for `R<score>`. Paths are taken as bytes
  and converted lossily to UTF-8 for display and for the extension check;
  contents are read by blob id, so a non-UTF-8 path never has to round-trip.
- Every command runs with the repository root as working directory and with
  `GIT_TERMINAL_PROMPT=0`, so nothing can wait for input.

### D4: CLI revision mode

- `clap`: `before` and `after` positionals become optional;
  - `--base <rev>` conflicts with them;
  - `--head <rev>` requires `--base` and defaults to `HEAD`;
  - "neither mode" is checked after parsing and turned into a usage error
    (exit 2).
- Per changed file, in path order:
  1. Skip it unless its head path has a supported extension.
  2. Decide the before side:
     - empty `FileModel` if the file is added, or renamed from a path
       without a supported extension;
     - otherwise the parsed old blob.
  3. Parse both sides. On failure, emit `input/unparsable` and continue:
     - invalid UTF-8: message `not valid UTF-8`, no line;
     - head syntax error: message
       `syntax error on line N (or syntax the parser does not support yet)`,
       with line `N`;
     - base syntax error only: message
       `syntax error on line N of the base version (...)`, with no line.

     The message is prefixed with `file not analyzed: `.
  4. Run `ComplexityAnalyzer` on the `FileChange` (path = head path).
- Sort all findings by `(path, line, rule)`. `None` lines sort first, which
  the spec states.
- The rule id `input/unparsable` is a public constant in the CLI, which is
  the only place that creates it.
- `GitError` maps to `error: <message>` and exit code 2. `UnknownRevision`
  adds: `if this is a shallow clone, fetch the base first (in GitHub
  Actions: actions/checkout with fetch-depth: 0)`.

### D5: Tests

- **`qgate-git`:** `#[cfg(test)]` tests build throwaway repositories under
  `std::env::temp_dir()` (no `tempfile` dependency) with a small helper that
  runs `git init -b main` and commits. The helper sets `GIT_CONFIG_GLOBAL` to
  an empty file, `GIT_CONFIG_NOSYSTEM=1` and fixed author and committer
  identities and dates, so the developer's own git config cannot change a
  result. The cases:
  - modified, added, deleted, renamed, renamed from `.js`;
  - symlink, type change;
  - a submodule entry, created with `git update-index --cacheinfo 160000,...`,
    so no second repository and no file-protocol clone are needed;
  - merge-base versus moved base, no merge base;
  - unknown revision and option-like revision, not a repository;
  - a non-UTF-8 path.
- **`qgate-cli`:** end-to-end tests in `tests/revisions.rs` build a
  repository the same way and run the binary in it, one test per scenario of
  the two spec deltas that the CLI owns. Fixture contents are inline strings,
  because they have to be committed in a sequence anyway.
- Tests need `git` on the `PATH`. That is documented in the README build
  section; CI runners have it.

## Risks / Trade-offs

- [git missing] → `GitNotFound` has its own error message. No feature
  newer than long-standing `rev-parse`, `merge-base`, `diff --raw` and
  `cat-file` is used, so no version probe is needed.
- [Shallow CI clones make the base unknown] → the error message says how to
  fix it; the README shows `fetch-depth: 0`.
- [One process per blob is slow on very large changes] → fine for pull
  requests; `git cat-file --batch` is the documented next step (D1).
- [Renames below 50% similarity look like delete plus add] → known
  limitation (proposal); `-M` keeps git's default threshold, so qgate agrees
  with what `git diff` shows the user.
- [User git config could change diff output] → `--raw -z` output is not
  affected by color, pager, external diff, textconv or `diff.noprefix`;
  `-M` overrides `diff.renames`; running at the top level neutralizes
  `diff.relative` (which the first version missed, see the PR 1 review
  notes). In tests the code under test is isolated as well (D5).
- [Error detection by exit code, not by parsing localized messages] →
  `rev-parse --verify --quiet` and `merge-base` signal by exit code; other
  failures surface git's stderr verbatim in `GitError::Command`.

## Migration Plan

Additive. The two-file mode and its outputs are unchanged, apart from the
sort key gaining the path, which is constant in that mode.

## Implementation Notes

<!-- Decisions made during apply are recorded here, per CLAUDE.md. -->

- **1.2:** `Repository` also exposes `root()`, the work-tree top level.
  The CLI needs it to report where it looked; it adds no behaviour.
- **1.3:** git commands run in the directory the repository was discovered
  from, not in `root`: git finds the repository itself, and a top-level
  path that is not valid UTF-8 never has to round-trip through a `String`.
  *Superseded at the PR 1 review:* this let `diff.relative` narrow the diff.
- **1.3:** the option-like-revision test also passes without
  `--end-of-options` (checked by removing it): the appended `^{commit}` and
  `--verify` already keep `rev-parse` from reading the value as an option,
  and ref names cannot start with `-`. `--end-of-options` stays as defense
  in depth; the test pins the observable outcome (unknown revision, no file
  written).
  *Superseded at the PR 1 review:* `--end-of-options` was replaced by
  rejecting revisions that start with `-`.
- **1.3:** the test helper lives in `src/test_support.rs` (`#[cfg(test)]`),
  so the later `changed_files` and `read_blob` tests share it.
- **1.4:** the diff also passes `--no-abbrev`, missing from the D3 table:
  without it `--raw` prints abbreviated blob ids, which `cat-file` would
  have to expand again and which could become ambiguous.
- **1.4:** a record is kept only if its new mode is a regular file mode
  (`100644`, `100755`) and, for modifications and renames, its old mode is
  too; everything else (deleted, `T`, symlinks, gitlinks, unmerged) is
  skipped in one place. `C` (copy) is handled like `R` for robustness,
  although `-M` alone never reports copies.
- **1.4:** results are sorted by head path in code instead of relying on
  git's output order, which for renames follows the old path.
- **1.6:** the crate example is a `no_run` doctest rather than a plain
  code block: it cannot create a repository, but it is still compiled, so
  the documented API cannot drift.

#### Changes after the PR 1 review (2026-11-09)

The reviewer subagent found one blocker and several real gaps. Decisions:

- **`diff.relative` (blocker):** with `diff.relative=true` (git 2.28+) and a
  run from a subdirectory, `git diff` listed only that subdirectory, with
  paths relative to it. Commands now run at the top level, reached as the
  discovery directory plus `rev-parse --show-cdup` (only `../` parts, so no
  lossy path round-trip). A test sets `diff.relative` in the repository's
  own config and runs from a subdirectory.
- **Git version and revisions as options:** `rev-parse` accepts
  `--end-of-options` only from git 2.30 (not 2.24 as written), and on older
  git every revision would have been "unknown". Revisions starting with `-`
  (or empty) are now rejected by qgate itself; no ref name or revision
  syntax starts with `-`. `--end-of-options` is dropped; ADR 0002 updated.
  No option injection was ever shown: probing `--default=HEAD`, `--glob=*`,
  `--all`, `--since=...` and others with the `^{commit}` suffix all failed
  to resolve. The test now checks four option-like values for base and head
  and that the work tree gains no entries; like before, it cannot fail on
  current git even without the guard, which stays as explicit defense.
- **Test isolation:** the fixtures did not clear `GIT_DIR`,
  `GIT_INDEX_FILE` and similar variables (set by git for hooks), and the code
  under test used the developer's own config. `test_support::isolate` now
  clears repository variables, uses an empty global config, no system config
  and `GIT_CEILING_DIRECTORIES` = temp dir, for the fixtures and, through a
  `#[cfg(test)]` hook in `run`, for the code under test. Verified by running
  the tests with `GIT_INDEX_FILE` and `GIT_DIR` set: nothing is written
  outside the temporary repositories.
- **Missing directory:** spawning in a directory that does not exist also
  fails with `NotFound`; it is now `NotARepository`, not "git is not
  installed".
- **Parser errors** have their own `GitError::UnexpectedOutput` instead of
  a `Command` error that read "failed with 0".
- **Type change to a regular file (asked, human chose "treat as added"):**
  a regular file that replaced a symlink or submodule is analyzed as added;
  spec "Added files" and "Skipped changes" updated, with a scenario.
- **Reviewer prompt (asked, human approved):** `.claude/agents/reviewer.md`
  now states the `cli -> git` dependency edge.
- Not changed: "`git` cannot be run" has no crate-level test, because the
  `PATH` of the code under test cannot be changed without `unsafe`
  `set_var`; the CLI tests in PR 2 can run the binary with an empty `PATH`.

#### PR 2: `qgate check --base`

- **2.1:** `check` keeps one subcommand with an overridden usage that shows
  both forms; clap enforces `--base` vs the files and `--head` needing
  `--base`, and "neither form" is a clap `MissingRequiredArgument` error
  built from the same subcommand, so it prints the same usage. `--head` has
  no clap default (a default would trip `requires`); the code falls back to
  `HEAD`.
- **2.2:** the CLI tests repeat the small temporary-repository helper of
  `qgate-git` (an integration test cannot use another crate's
  `#[cfg(test)]` code), including the isolation from repository variables
  and config; the qgate binary inherits it, so the git it starts is
  isolated too. "git cannot be run" is tested by running the binary with an
  empty `PATH`.
- **2.3:** the before side is parsed with the language of the old path,
  so a `.ts` -> `.tsx` rename parses each side with its own grammar; a
  base version that is not valid UTF-8 gets its own message
  (`the base version is not valid UTF-8`). A `ParseError::Parser` (not
  caused by the input) stays an error for the whole run, as in the two-file
  mode. The head is checked before the base, so a file broken on both sides
  is reported with the head's line.
- **2.3:** the shared sort `(path, line, rule)` replaces the two-file
  `(line, rule)` sort; the path is constant there, so its output is
  unchanged. Two mutations were checked: parsing a `.js` rename as
  TypeScript and dropping the path from the sort each fail a test.
- **2.3:** `qgate-cli` depends on `qgate-core` again (for the
  `input/unparsable` finding), after PR 3 of the previous change had dropped
  it as unused.
