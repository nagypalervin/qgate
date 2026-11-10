# Proposal: add-worktree-input

## Why

M3 is about checking code while it is being written. An AI coding agent edits
files in the working tree and commits late, or never. `qgate check --base`
only compares commits, so it cannot see the agent's work in progress. Every
M3 integration needs this first: agent hooks, the MCP server, and a developer
running qgate before a commit.

## What Changes

- **New option `--worktree`** for `qgate check --base <rev>`. It checks the
  working tree instead of a commit.
  - The before side stays the merge base of `--base` and `HEAD`.
  - The after side is the working tree as it is on disk:
    - tracked files with their current contents, staged or not;
    - untracked files that `.gitignore` does not exclude;
    - files deleted from disk count as deleted.
- **The working tree is the head everywhere the head is used.** This covers:
  - the changed files;
  - their rename detection;
  - `qgate.toml`;
  - the files that duplication compares against.

  Every analyzer therefore works as it does for a commit. For example, a
  file the agent moved with `mv` is still a rename, not a new file.
- **Nothing in the repository changes.** The check leaves alone:
  - the index, `HEAD`, branches and other refs;
  - the stash, the reflog and the files on disk.

  Its only trace is unreachable git objects, which `git gc` prunes.
- **Invalid combinations are usage errors (exit 2):** `--worktree` together
  with `--head`, and `--worktree` without `--base`.
- **Default unchanged:** without `--worktree`, `--head` still defaults to
  `HEAD`, so CI checks exactly what was committed.
- **New git error:** `--worktree` in a repository without a commit is
  reported as an unknown revision `HEAD` (exit 2).
- **README:** a "Checking uncommitted work" section, and the option in the
  usage lines and the `--help` output.

### Decisions taken during exploration

- **A separate flag, not a new default.** `git diff main` compares with the
  working tree, so a working-tree default would be familiar locally. But
  `--head` defaulting to `HEAD` is published behaviour. In CI, a
  working-tree default would also pick up build output that is not ignored.
  Hooks and the MCP server pass the flag themselves.
- **Disk contents, not the index.** Agents do not stage, so "what is on
  disk" is what they just wrote. A staged-only mode for git pre-commit hooks
  can come later.
- **Untracked files count**, filtered by `.gitignore`: an agent's new files
  are untracked until someone commits them.
- **Snapshot, not a second code path.** The working tree becomes a git
  commit that no ref points to, and the existing revision pipeline reads it
  unchanged. How, and why not reading files from disk, is in `design.md`
  (D1).

### Out of scope

- JSON output (`add-report-formats`), agent and git hooks
  (`add-agent-hooks`), the MCP server (`add-mcp-server`).
- A staged-only mode (the index as head) for git pre-commit hooks.
- Windows (qgate is not tested there yet).
- Telling the user that `qgate.toml` differs between the working tree and
  `HEAD` (see design, Risks).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `revision-input`:
  - **New requirement "Working tree as head":** what the after side
    contains.
  - **New requirement "The working tree replaces the head everywhere":**
    changed files, renames, configuration and duplication.
  - **New requirement "Checking the working tree changes nothing".**
  - **New requirement "Working tree errors":** no commit yet, and a working
    tree git cannot read.
- `check-command`:
  - **"Compare two git revisions":** the `--worktree` form.
  - **"Invalid invocation":** `--worktree` with `--head` or without
    `--base`.
- `configuration`:
  - **"Configuration from the head revision":** uncommitted changes to
    `qgate.toml` count with `--worktree`. Today the requirement says they
    never count.

`duplication-analysis` ("Compared files") and `architecture-analysis` refer to
"the head revision". The new `revision-input` requirement defines the working
tree as that head, so their text does not change.

## Impact

- **`qgate-git`:**
  - one new public method that snapshots the working tree into a commit id;
  - a new public error variant, `GitError::WorkTree`, for a snapshot that
    cannot be taken;
  - the existing methods take the id like any revision.

  No new dependency, still only the `git` command (ADR 0002).
- **`qgate-cli`:**
  - the flag and the two usage errors;
  - the snapshot taken before the diff;
  - error messages that name `HEAD` or the working tree, not the snapshot's
    commit id.
- **Specs:** `revision-input`, `check-command`, `configuration` (deltas).
- **Docs:** README.
- **Users:** the option is new and the default is unchanged. One
  invocation changes: `qgate check a.ts b.ts --head x` used to ignore
  `--head` and compare the two files; it is now the usage error that
  "Invalid invocation" already required (`--head` without `--base`), with
  exit code 2.
