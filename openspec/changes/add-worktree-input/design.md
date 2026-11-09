# Design: add-worktree-input

## Context

See proposal.md for why and what, and the `revision-input` delta for the
required behaviour.

- **Everything in revision mode takes commits.** `qgate-git` resolves every
  revision with `rev-parse --verify <rev>^{commit}`. The following all
  build on such commit ids:
  - `changes` (`diff --raw -M` from the merge base);
  - `file_at` (`qgate.toml`);
  - `files_at` (the head tree for duplication);
  - the batch blob reader.

  The CLI threads two strings, `base` and `head`, through
  `check_revisions`, `load_config` and `duplication`.
- **Rename detection lives in the diff.** `git diff --raw -M` pairs a
  deleted path with an added one only when both sides are trees or
  commits. A diff against the working tree (`git diff <rev>`) does not see
  untracked files at all, so it cannot pair a file moved with `mv`.
- **ADR 0002:** git through the `git` command only, no library, and
  revisions never reach git as options.
- **Tests** create temporary repositories isolated from the developer's
  git configuration (`test_support::isolate`). They need git 2.32 or newer.

## Goals / Non-Goals

**Goals:**

- One way into the analysis: the working tree becomes a commit id, and
  nothing after that point knows the difference.
- Leave the user's repository as it was (the spec's "changes nothing"
  requirement). That includes the index, which a concurrent `git` command
  or an editor may be using.
- Keep the cost proportional to what changed, not to the repository size.

**Non-Goals:**

- A staged-only mode (the index as head).
- Watching the working tree or caching snapshots between runs.
- Cleaning up the objects a snapshot writes; `git gc` does that.

## Decisions

### D1. Snapshot the working tree into a commit through a temporary index

`Repository::worktree_snapshot()` returns the id of a new commit object
that holds the working tree. The steps:

1. **Find the head commit.** Resolve `HEAD` as a commit. If that fails,
   the result is `UnknownRevision("HEAD")`.
2. **Make a private index.** Get a unique path in the git directory from
   `git rev-parse --git-path qgate-index-<pid>-<n>`, so linked work trees
   and a relocated `GIT_DIR` are respected. Copy the index git would use
   there, from `git rev-parse --git-path index`, which honours an exported
   `GIT_INDEX_FILE`. If there is no index, run `git read-tree <head>` into
   the private one instead.
3. **Stage everything into the private index.** Run
   `GIT_INDEX_FILE=<private> git add -A` from the top level, with hooks
   disabled (D2). This stages
   modified, deleted and untracked-but-not-ignored files exactly as
   `git add -A` would.
4. **Write the tree:** `GIT_INDEX_FILE=<private> git write-tree`.
5. **Write the commit:** `git commit-tree <tree> -p <head> -m
   "qgate working tree"`, with a fixed author and committer (name `qgate`,
   e-mail `qgate@localhost`, date `@0 +0000`) set through `GIT_AUTHOR_*` and
   `GIT_COMMITTER_*`.
6. **Delete the private index** and any `.lock` next to it, on every path
   out (a guard with `Drop`).

The CLI then passes the commit id as the head to the existing calls.

**Why a commit and not just the tree:**
- `merge-base` and `rev-parse ^{commit}` need commits. With `HEAD` as
  parent, `merge-base(base, snapshot)` equals `merge-base(base, HEAD)`.
- `--base HEAD --worktree` then compares exactly the uncommitted changes.

**Why the copied index:**
- It keeps git's stat cache, so `git add -A` hashes only files whose stat
  data changed. Starting from `read-tree` would hash every file.
- It also keeps the `skip-worktree` and `assume-unchanged` bits, which the
  spec relies on.

**Why the fixed identity and date:**
- `commit-tree` fails without a configured identity, and an agent's
  sandbox often has none.
- A fixed date makes the same working tree give the same commit id.
- `commit-tree` is plumbing and ignores `commit.gpgSign` (since git 2.4),
  so a user who signs every commit gets no prompt and no failure. The
  porcelain `git commit` would try to sign.

**Alternatives considered:**
- **A second code path that reads files from disk:** `git diff <merge-base>`
  for tracked files, plus `git ls-files --others --exclude-standard` for
  untracked ones, plus reading from the file system. Rejected:
  - it duplicates `changes`, `file_at` and `files_at`;
  - it loses renames to untracked paths;
  - every analyzer would need two sources.
- **`git stash create`:** it ignores untracked files. It also records the
  index as a second parent and runs in the user's index context. Rejected.
- **A temporary index outside the git directory** (system temp dir). It
  works for a plain index. With `core.splitIndex`, git keeps the shared
  index part (`sharedindex.<id>`) next to the index file, so a copy
  elsewhere would lose it. Keeping the copy in the git directory avoids the
  question.

### D2. Nothing reaches refs, the index or hooks

- **No refs:** `add`, `write-tree` and `commit-tree` never update a ref or
  the reflog.
- **No hooks:** `commit-tree` runs no hooks. `git add`, however, writes an
  index, and git runs the `post-index-change` hook on every index write,
  even a private one. Every snapshot command therefore runs with
  `-c core.hooksPath=/dev/null`, a directory that holds no hooks, on Linux
  and macOS alike.
- **`core.fsmonitor` still runs.** A configured file-system monitor is a
  query that git asks which files changed, not a hook. Disabling it would
  only make `git add -A` slower.
- **The real index is only read**, by a plain file copy. Git replaces an
  index by renaming `index.lock` over it, so a copy sees either the old or
  the new file, never half of each.
- **What is written:**
  - the private index, which is removed;
  - loose objects (blobs, trees, one commit) that no ref reaches. Git
    prunes those after `gc.pruneExpire`, two weeks by default.
- **Filters still run:** `git add -A` runs configured clean filters
  (e.g. Git LFS), so the snapshot holds what a commit would hold.

### D3. Error messages name `HEAD`, not the snapshot id

- **CLI side:** `check_revisions` gets a head *label* next to the head
  revision. For `--worktree` the label is `HEAD` and the revision is the
  snapshot id. `git_message` uses the label, so "no merge base between
  `other` and `HEAD`" reads as the spec requires.
- **Git side:** errors raised inside the snapshot are mapped as follows:
  - an unknown `HEAD` becomes `UnknownRevision("HEAD")`;
  - a failing `git add -A` becomes a new `GitError::WorkTree { stderr }`.
    The CLI prints it as "cannot read the working tree: <git's message>".

### D4. CLI shape

- **Flag:** `--worktree` is a flag that `requires("base")` and
  `conflicts_with("head")`.
- **Usage lines:** a third line, `qgate check --base <rev> --worktree
  [--fail-on <level>]`.
- **Help text:** "Check the working tree (uncommitted and untracked files)
  instead of --head".
- **Two-file form:** the existing `conflicts_with_all(["before", "after"])`
  on `--base` already rejects `--worktree` together with two files, because
  `--worktree` requires `--base`.

### D5. Where the code goes

- **`qgate-git`:**
  - `worktree_snapshot`;
  - its private index guard;
  - `GitError::WorkTree`.

  No new dependency. The unique name uses the process id plus an atomic
  counter, so tests that run in parallel threads do not collide.
- **`qgate-cli`:**
  - the flag;
  - the label/revision split;
  - the new error text.
- **Unchanged:** `qgate-analyzers`, `qgate-config` and `qgate-report`.

## Risks / Trade-offs

- **[Risk] A large untracked file that is not ignored is hashed into the
  object store on every run** (build output, a dump). → `git add -A` would
  do the same. This is documented in the README section, with the advice
  to ignore such files. qgate itself only reads supported files from the
  snapshot.
- **[Risk] Loose objects pile up when an agent checks after every edit.**
  → Identical contents give identical objects. Others are pruned by
  `git gc` after two weeks. They take a few kilobytes per changed file.
- **[Risk] An agent relaxes `qgate.toml` in the working tree and passes its
  own check.** → This is the same as a pull request that edits
  `qgate.toml`: the change is in the diff, and CI reads the committed file.
  A later change can report a `qgate.toml` that differs from `HEAD`
  (proposal, Out of scope).
- **[Risk] Run from inside a git hook, `GIT_INDEX_FILE` points to a
  temporary index.** → `--git-path index` follows it, so the snapshot
  starts from the index the hook sees. The working tree wins anyway (D1).
  A staged-only mode for hooks is out of scope.
- **[Risk] An unreadable file fails the whole check (exit 2)** rather than
  being skipped. → This matches `git add -A`, and the message names the
  file. `--ignore-errors` would hide a file from the check without a
  trace.
- **[Trade-off] Sparse checkouts:** files outside the sparse cone are
  `skip-worktree` and count as unchanged. That is what git status shows,
  and what the spec says.
- **[Risk] A crash or kill between steps 2 and 6 leaves a
  `qgate-index-*` file in the git directory.** → It is invisible to git
  and to the user, holds no data that is not in the work tree, and the
  next run uses a new name. This is too minor for the README.

## Migration Plan

None. The option is new and the default is unchanged.

## Open Questions

- Should a later version warn when the working tree's `qgate.toml` differs
  from `HEAD`'s? It can be added without changing this design (Risks).

## Implementation Notes

- **The snapshot is its own module**, `crates/qgate-git/src/worktree.rs`,
  as `lib.rs` already has 1,600 lines. It adds an `impl Repository` block
  and reuses the crate's private helpers (`resolve`, `git_command`).
- **A user's `add.ignoreErrors=true` changes nothing.** With it, `git add`
  goes on past an unreadable file but still exits non-zero, so the
  snapshot still fails with `GitError::WorkTree`. An explicit
  `add.ignoreErrors=false` was tried and removed: no test could tell it
  apart (task 1.3). The unreadable-file test sets the option, to keep it
  that way.
- **The fixed date is `@0 +0000`**, git's raw format for the epoch, so no
  date parsing depends on the locale.
- **A stale private index is removed before use.** A killed run can leave
  `qgate-index-<pid>-<n>` and its `.lock`, and a later process can get the
  same pid. A leftover `.lock` would otherwise make `git add` fail.
- **No `--no-gpg-sign`.** The design first passed it to `commit-tree`.
  Removing it broke no test (task 1.3), because `commit-tree` never signs
  on configuration; only `git commit` does. It was dead code, so it is
  gone. The test with `commit.gpgSign=true` stays: it fails if the
  snapshot ever moves to `git commit`.
- **The hook test writes its markers to an absolute path.** Git does not
  export `GIT_DIR` to hooks of a plain `.git` repository. The test also
  shows that a plain `git add` does run `post-index-change` there, so the
  test would see the hook if the snapshot ran it.
