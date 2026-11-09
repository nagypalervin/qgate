# Proposal: add-git-revision-input

## Why

qgate's promise is "compare two git revisions and report what got worse",
but `qgate check` can only compare two files that the user prepares by hand.
To work as a gate in CI or before opening a pull request, it has to read the
changed files straight from git. This is the missing half of milestone M1.

## What Changes

- **New mode `qgate check --base <rev> [--head <rev>]`** (`--head` defaults
  to `HEAD`). It compares the merge base of the two revisions with the head,
  which is what a pull request shows, and analyzes every changed TypeScript
  file.
- **The two-file form stays unchanged.** Giving both files and `--base` is a
  usage error.
- **File selection:** only files changed between the two revisions with a
  supported extension (`.ts`, `.mts`, `.cts`, `.tsx`):
  - modified files are compared;
  - added files are compared against an empty file, so only new functions
    above the threshold are reported;
  - renamed files are compared with their old version;
  - deleted files, symlinks, submodules and type changes are skipped.
- **New rule `input/unparsable`** (Warning): in revision mode, a file that
  cannot be parsed (syntax error, syntax the grammar does not support yet,
  or not valid UTF-8) is reported as a finding and skipped. The rest of the
  report is still produced. The two-file mode keeps refusing such a file
  with exit code 2.
- **Output** keeps the line format, with paths relative to the repository
  root. Findings are sorted by path, then line, then rule id.
- **Errors** (not a git repository, git not installed, unknown revision, no
  merge base) exit with code 2. An unknown base revision gets a hint about
  shallow clones (`fetch-depth: 0` in GitHub Actions).
- **New crate `qgate-git`**: runs the `git` command and returns the changed
  files with their old and new contents. It knows nothing about languages or
  findings. Recorded in a new **ADR 0002**, together with the decision to call
  `git` instead of using the `gix` crate.

### Decisions taken during exploration

- **`git` subprocess, not `gix`.** `gix` 0.87 adds 131 to 202 crates,
  depending on features; the workspace has about 25 external crates today.
  Calling `git` adds none, and git's own semantics (merge base, rename
  detection) are what users expect. Recorded in ADR 0002, with the point at
  which to revisit it.
- **Merge-base semantics.** A direct diff of `main` against `HEAD` would blame
  a pull request for changes that landed on `main` after it branched.
- **Flags, not `main..HEAD`.** In git, `..` and `...` mean different things
  for `log` and `diff`, and `..` also appears in file paths.
- **`FileChange` stays as it is.** An added file gets an empty `FileModel` as
  its before side. The CLI builds one `FileChange` per changed file.
- **A file that cannot be parsed is reported, not fatal.** A quality gate
  that silently skips a file is worse than a noisy one, and one file with new
  syntax must not block every pull request until configuration exists (M2).
- **`qgate-git` is a leaf crate.** Dependency direction `cli -> git`. The CLI
  stays thin, and the git logic is testable on its own.

### Out of scope

- Uncommitted changes (working tree, staged files).
- `base..head` / `base...head` range syntax.
- Excluding paths (generated or vendored code): with `qgate.toml` in M2.
- A summary line or JSON output (M3).
- Failing the run on findings (`--fail-on`): a separate change.
- Distribution as a GitHub Action or release binary (M3).
- Known limitation: git detects a rename only above 50% similarity. A file
  that is renamed and heavily rewritten shows up as deleted plus added, so its
  complex functions are reported as new.

## Capabilities

### New Capabilities

- `revision-input`: how qgate gets the files to analyze from git, covering
  the revisions it compares, which files it selects and how it treats their
  git status, how it reports a file it cannot parse, and the git-level errors.

### Modified Capabilities

- `check-command`:
  - adds the `--base`/`--head` invocation;
  - scopes the file-only requirements (supported files, unreadable files,
    syntax errors) to the two-file mode;
  - extends the finding path and the sort order to many files;
  - extends invalid invocations to mixing the two modes.

## Impact

- **New crate `crates/qgate-git`:** no workspace or external dependencies.
  It runs `git` as a subprocess.
- **`qgate-cli`:** depends on `qgate-git` and gets a second mode for `check`.
- **`qgate-lang`, `qgate-analyzers`, `qgate-core`, `qgate-report`:** no API
  change expected.
- **Docs:**
  - `docs/adr/0002-git-via-subprocess.md` (new);
  - `CLAUDE.md` "Layout", README repository layout and usage.
- **Runtime requirement:** `git` on the `PATH` in revision mode. Tests of
  `qgate-git` and the CLI create temporary git repositories, so `git` must be
  available where tests run (it is on GitHub runners).

## Open Questions

Resolved at Gate 1 (2026-11-09): the human approved the proposal and the
design as written, which keeps the proposed default for each question below.
The specs already reflect them.

1. **Rule id `input/unparsable`.** It breaks the `analyzer/rule` pattern
   slightly, because "input" is not an analyzer. *Proposed:* keep it: it is
   clear and collides with nothing. Rule ids are a public API, so this is the
   moment to rename it.
2. **Base side cannot be parsed.** *Proposed:* report `input/unparsable`
   without a line number, with a message that names the base version and its
   error line. Otherwise the finding would point at a line of a file version
   the user is not looking at.
3. **Paths when run from a subdirectory.** *Proposed:* always relative to
   the repository root, and all changed files in the repository are analyzed,
   not only those below the current directory.
