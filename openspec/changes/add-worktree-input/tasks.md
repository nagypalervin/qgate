# Tasks

Two pull requests:
- `change/add-worktree-input`: the snapshot in `qgate-git`, together with
  these planning artifacts;
- `change/add-worktree-input-2`: the CLI, the specs' end-to-end scenarios
  and the README.

Decisions made during apply go into `design.md`. Every commit passes
`cargo fmt`, `cargo clippy -D warnings` and `cargo test --workspace` (the
pre-commit hook enforces it), and CI also runs the tests on Rust 1.85.
Commit messages and PR bodies are written to files first.

## 1. Working tree snapshot in qgate-git (PR 1)

- [x] 1.1 Write failing tests in `crates/qgate-git/src/worktree.rs` (the snapshot's own module, see design Implementation Notes), using `TestRepo`, for design D1 and D2:
  - **Content:** `worktree_snapshot` followed by `changes(base, snapshot)` reports:
    - an unstaged edit;
    - a staged-then-edited file, with its disk contents;
    - an untracked file;
    - a tracked file deleted from disk;
    - a plain `mv` of a tracked file, as a rename.

    It leaves out an ignored file, and files marked `skip-worktree` or `assume-unchanged`.
  - **Clean tree:** gives no changes against `HEAD`.
  - **Nothing changes:** the index file's bytes, `git status --porcelain`, every ref (`git for-each-ref`), `HEAD`'s reflog and `git stash list` are byte-identical before and after.
  - **Hooks do not run:** `pre-commit`, `post-commit` and `post-index-change` hooks that write a marker file leave no marker.
  - **No leftovers:** no `qgate-index-*` file is left in the git directory.
  - **Determinism:** the same working tree gives the same snapshot id twice.
  - **Errors:**
    - a repository without commits gives `UnknownRevision("HEAD")`;
    - an unreadable untracked file gives `GitError::WorkTree` naming the file (skipped when running as root);
    - with `commit.gpgSign=true` and no identity configured, the snapshot still succeeds.

  Verify the tests fail.
- [x] 1.2 Implement `Repository::worktree_snapshot`, the private index guard and `GitError::WorkTree` (D1, D2, D5), without new dependencies; verify the 1.1 tests pass and every existing `qgate-git` test passes unchanged
- [x] 1.3 Check that the tests bite. Break each of these, one at a time, and confirm a test fails for each:
  - the index copy (start from `read-tree` only);
  - `-A` (use plain `add -u`);
  - the guard's removal;
  - `--no-gpg-sign` (no test failed: `commit-tree` ignores `commit.gpgSign`, so the flag was removed; see design, Implementation Notes);
  - the fixed identity;
  - `core.hooksPath=/dev/null`.

  Added after review rounds 1 and 2, each also caught by a test:
  - the fixed date;
  - `core.splitIndex=false`;
  - the `read-tree` fallback;
  - the main work tree's index used in a linked work tree;
  - no stale private index removal;
  - removal regardless of age;
  - a name prefix without the dash.
- [x] 1.4 Update the `qgate-git` crate docs (the method list and the "Commands run at the top level" paragraph); verify `RUSTDOCFLAGS="-D warnings" cargo doc -p qgate-git --no-deps` builds

## 2. The CLI, end-to-end scenarios and the README (PR 2)

- [x] 2.1 Write failing tests in `crates/qgate-cli/tests/revisions.rs`, building temporary repositories as the existing tests do:
  - **"Working tree as head", every scenario:**
    - Uncommitted edit;
    - Without the flag;
    - Committed and uncommitted changes together;
    - Untracked new file;
    - Ignored file;
    - The disk version wins;
    - File moved without git;
    - Deleted from disk;
    - Files git does not look at.
  - **"The working tree replaces the head everywhere":**
    - Copy of an uncommitted file;
    - Only the uncommitted changes;
    - Clean working tree.
  - **"Checking the working tree changes nothing":**
    - Staged changes stay staged;
    - No commit appears;
    - Hooks do not run.

    "Split index" and "Interrupted checks" are covered in `qgate-git`
    (review rounds 1 and 2), where the configuration and the leftover
    files are set up directly.
  - **"Working tree errors":**
    - Repository without commits;
    - Base HEAD in a repository without commits;
    - qgate.toml is not a regular file on disk;
    - Unrelated histories, whose message names `HEAD`;
    - Unreadable file (skipped as root).
  - **`configuration`:**
    - Uncommitted changes count with the working tree;
    - Invalid configuration on disk.
  - **`check-command` "Working tree".**

  Verify they fail.
- [x] 2.2 Write failing tests in `crates/qgate-cli/tests/check.rs` for the new "Invalid invocation" scenarios: working tree without base, with `--head`, with two files. Verify they fail.
- [x] 2.3 Implement `--worktree` in `qgate-cli` (D3, D4):
  - the flag with `requires("base")` and `conflicts_with("head")`;
  - the third usage line and the help text;
  - the snapshot taken before the diff;
  - the head label in `git_message`;
  - `GitError::WorkTree` printed through its `Display` ("cannot take a snapshot of the working tree: ...").

  Verify the 2.1 and 2.2 tests pass, and that every existing test passes unchanged.
- [ ] 2.4 Update `README.md`:
  - a "Checking uncommitted work" section: what counts, that nothing in the repository changes, `--base HEAD --worktree` for only the uncommitted changes, and the advice to ignore large generated files;
  - the option in "Try it".

  Run every README command in a temporary repository and verify it behaves as documented.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- Write the `docs/journal/` entry, then archive the change on the branch of PR 2 (`/opsx:archive`).
- Gate 3: the human merges each pull request.
