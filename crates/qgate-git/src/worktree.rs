//! The working tree as a commit (design D1 and D2 of `add-worktree-input`).
//!
//! [`Repository::worktree_snapshot`] stages the whole working tree into a
//! private copy of the index, writes it as a tree and wraps the tree in a
//! commit that no ref points to. Every other method then reads that commit
//! like any revision. The user's index, refs, reflog and stash stay as they
//! were; the only trace is unreachable objects, which `git gc` prunes.

use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{GitError, Repository, command_error, git_command, spawn_error, stderr_of, stdout_of};

/// Configuration for every snapshot command. Git runs the
/// `post-index-change` hook on every index write, even of a private index,
/// so hooks are pointed at a directory that holds none.
const SNAPSHOT_CONFIG: [&str; 2] = ["-c", "core.hooksPath=/dev/null"];

/// The author and committer of every snapshot. `commit-tree` fails without
/// an identity, which an agent's sandbox often lacks, and a fixed date
/// makes the same working tree give the same commit id.
const SNAPSHOT_IDENTITY: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "qgate"),
    ("GIT_AUTHOR_EMAIL", "qgate@localhost"),
    ("GIT_AUTHOR_DATE", "@0 +0000"),
    ("GIT_COMMITTER_NAME", "qgate"),
    ("GIT_COMMITTER_EMAIL", "qgate@localhost"),
    ("GIT_COMMITTER_DATE", "@0 +0000"),
];

/// Numbers private indexes, so threads of one process never share one.
static NEXT_INDEX: AtomicUsize = AtomicUsize::new(0);

impl Repository {
    /// The id of a new commit that holds the working tree as it is on disk:
    /// tracked files with their current contents, staged or not, without
    /// the ones deleted from disk, plus the untracked files that git's
    /// ignore rules do not exclude. Files the index marks `skip-worktree`
    /// or `assume-unchanged` keep their indexed version. The commit's
    /// parent is `HEAD`, so the id works as a head revision for every other
    /// method, and the merge base with any base is the one `HEAD` has.
    ///
    /// The index, refs, reflog, stash and working tree are left as they
    /// were, and no hook runs. The same working tree gives the same id.
    ///
    /// # Errors
    ///
    /// [`GitError::UnknownRevision`] for `HEAD` if the repository has no
    /// commit yet, [`GitError::WorkTree`] if git cannot read the working
    /// tree (for example a file that is not ignored cannot be read), or
    /// another [`GitError`] if git fails.
    pub fn worktree_snapshot(&self) -> Result<String, GitError> {
        let head = self.resolve("HEAD")?;
        let index = PrivateIndex::create(self, &head)?;
        let add = self.snapshot_git(&["add", "-A"], &index)?;
        if !add.status.success() {
            return Err(GitError::WorkTree {
                detail: stderr_of(&add),
            });
        }
        let tree = self.snapshot_checked(&["write-tree"], &index)?;
        // `commit-tree`, unlike `commit`, ignores `commit.gpgSign`.
        self.snapshot_checked(
            &[
                "commit-tree",
                "-p",
                &head,
                "-m",
                "qgate working tree",
                &tree,
            ],
            &index,
        )
    }

    /// Run a snapshot command against the private index, with the snapshot
    /// configuration and identity.
    fn snapshot_git(&self, args: &[&str], index: &PrivateIndex) -> Result<Output, GitError> {
        let all: Vec<&str> = SNAPSHOT_CONFIG.iter().chain(args).copied().collect();
        let mut command = git_command(&self.dir, &all);
        command.env("GIT_INDEX_FILE", &index.path);
        command.envs(SNAPSHOT_IDENTITY);
        command
            .output()
            .map_err(|e| spawn_error(&self.dir, &all, &e))
    }

    /// [`Repository::snapshot_git`] for a command that must succeed; its
    /// trimmed standard output.
    fn snapshot_checked(&self, args: &[&str], index: &PrivateIndex) -> Result<String, GitError> {
        let out = self.snapshot_git(args, index)?;
        if out.status.success() {
            Ok(stdout_of(&out))
        } else {
            Err(command_error(args, &out))
        }
    }
}

/// A copy of the index in the git directory, removed (with its lock file)
/// when dropped. It lives next to the real index, so a split index still
/// finds its shared part (design D1).
struct PrivateIndex {
    path: PathBuf,
}

impl PrivateIndex {
    /// A private index holding what the index git would use holds, or
    /// `head`'s tree if there is no index yet.
    fn create(repo: &Repository, head: &str) -> Result<Self, GitError> {
        let name = format!(
            "qgate-index-{}-{}",
            std::process::id(),
            NEXT_INDEX.fetch_add(1, Ordering::Relaxed)
        );
        // `--git-path index` follows `GIT_INDEX_FILE` and linked work trees.
        let args = ["rev-parse", "--git-path", "index", "--git-path", &name];
        let out = repo.git(&args)?;
        if !out.status.success() {
            return Err(command_error(&args, &out));
        }
        let mut lines = out.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty());
        let (Some(real), Some(private)) = (lines.next(), lines.next()) else {
            return Err(GitError::UnexpectedOutput {
                args: args.iter().map(|&a| a.to_owned()).collect(),
                detail: String::from_utf8_lossy(&out.stdout).trim().to_owned(),
            });
        };
        let absolute = |path: &[u8]| {
            let path = repo.dir.join(path_from_bytes(path));
            std::path::absolute(&path).unwrap_or(path)
        };
        let real = absolute(real);
        // The guard exists before the file, so every way out removes it.
        let index = Self {
            path: absolute(private),
        };
        // A run that was killed may have left these under the same name.
        index.remove();
        match std::fs::copy(&real, &index.path) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {
                repo.snapshot_checked(&["read-tree", head], &index)?;
            }
            Err(e) => {
                return Err(GitError::WorkTree {
                    detail: format!("cannot copy the index {}: {e}", real.display()),
                });
            }
        }
        Ok(index)
    }

    /// Remove the private index and its lock file, if they exist.
    fn remove(&self) {
        let mut lock = self.path.clone().into_os_string();
        lock.push(".lock");
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(lock);
    }
}

impl Drop for PrivateIndex {
    fn drop(&mut self) {
        self.remove();
    }
}

/// A path git printed, byte for byte where the platform allows.
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestRepo;
    use crate::{ChangedFile, Changes};

    fn repository(repo: &TestRepo) -> Repository {
        Repository::discover(&repo.dir).expect("a repository")
    }

    fn snapshot(repo: &TestRepo) -> String {
        repository(repo)
            .worktree_snapshot()
            .expect("a snapshot of the working tree")
    }

    /// The text of a blob, for comparing contents.
    fn text(repo: &TestRepo, id: &crate::BlobId) -> String {
        String::from_utf8(repository(repo).read_blob(id).expect("a blob")).expect("UTF-8")
    }

    /// Everything a check must leave as it was, read without taking
    /// git's optional locks (a plain `git status` may rewrite the index).
    fn repository_state(repo: &TestRepo) -> Vec<String> {
        let git_dir = repo.dir.join(".git");
        let index = std::fs::read(git_dir.join("index")).expect("the index");
        vec![
            format!("{index:?}"),
            repo.git(&["--no-optional-locks", "status", "--porcelain=v1", "-uall"]),
            repo.git(&["--no-optional-locks", "diff", "--cached", "--no-ext-diff"]),
            repo.git(&["for-each-ref"]),
            repo.git(&["reflog", "show", "--no-abbrev", "HEAD"]),
            repo.git(&["stash", "list"]),
            repo.git(&["rev-parse", "HEAD"]),
        ]
    }

    /// Files a check left in the git directory under the private index's
    /// name.
    fn private_indexes(repo: &TestRepo) -> Vec<String> {
        std::fs::read_dir(repo.dir.join(".git"))
            .expect("the git directory")
            .map(|entry| entry.expect("an entry").file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| name.starts_with("qgate-index"))
            .collect()
    }

    #[test]
    fn snapshot_holds_the_working_tree() {
        let repo = TestRepo::new();
        repo.write(".gitignore", "dist/\n");
        repo.write("src/edit.ts", "export const e = 1;\n");
        repo.write("src/staged.ts", "export const s = 1;\n");
        repo.write("src/gone.ts", "export const g = 1;\n");
        let moved = "export function f() {\n  return 1;\n}\n// one\n// two\n";
        repo.write("src/old.ts", moved);
        repo.write("src/sparse.ts", "export const p = 1;\n");
        repo.commit("base");

        // Unstaged edit.
        repo.write("src/edit.ts", "export const e = 2;\n");
        // Staged, then edited again: the disk version counts.
        repo.write("src/staged.ts", "export const s = 2;\n");
        repo.git(&["add", "src/staged.ts"]);
        repo.write("src/staged.ts", "export const s = 3;\n");
        // Deleted from disk only.
        std::fs::remove_file(repo.dir.join("src/gone.ts")).expect("delete");
        // Moved without git: a rename in the snapshot.
        std::fs::rename(repo.dir.join("src/old.ts"), repo.dir.join("src/new.ts")).expect("move");
        // Untracked, and untracked but ignored.
        repo.write("src/added.ts", "export const a = 1;\n");
        repo.write("dist/out.ts", "export const o = 1;\n");
        // Changed on disk, but git does not look at it.
        repo.git(&["update-index", "--skip-worktree", "src/sparse.ts"]);
        repo.write("src/sparse.ts", "export const p = 2;\n");

        let snapshot = snapshot(&repo);
        let Changes { changed, deleted } = repository(&repo)
            .changes("HEAD", &snapshot)
            .expect("changes");

        let summary: Vec<(&str, Option<&str>, String)> = changed
            .iter()
            .map(|f: &ChangedFile| {
                (
                    f.path.as_str(),
                    f.old_path.as_deref(),
                    text(&repo, &f.after),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("src/added.ts", None, "export const a = 1;\n".to_owned()),
                ("src/edit.ts", None, "export const e = 2;\n".to_owned()),
                ("src/new.ts", Some("src/old.ts"), moved.to_owned()),
                ("src/staged.ts", None, "export const s = 3;\n".to_owned()),
            ]
        );
        let deleted: Vec<&str> = deleted.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(deleted, vec!["src/gone.ts"]);
    }

    #[test]
    fn snapshot_from_a_subdirectory_covers_the_whole_tree() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");
        repo.write("root.ts", "export const r = 1;\n");
        repo.write("src/b.ts", "export const b = 1;\n");

        let from_src = Repository::discover(&repo.dir.join("src")).expect("a repository");
        let snapshot = from_src.worktree_snapshot().expect("a snapshot");
        let paths: Vec<String> = from_src
            .changed_files("HEAD", &snapshot)
            .expect("changes")
            .into_iter()
            .map(|f| f.path)
            .collect();

        assert_eq!(paths, vec!["root.ts", "src/b.ts"]);
    }

    #[test]
    fn clean_working_tree_is_the_head_tree() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");

        let snapshot = snapshot(&repo);

        assert_eq!(
            repo.git(&["rev-parse", &format!("{snapshot}^{{tree}}")]),
            repo.git(&["rev-parse", "HEAD^{tree}"])
        );
        assert_eq!(
            repository(&repo)
                .changed_files("HEAD", &snapshot)
                .expect("changes"),
            vec![]
        );
    }

    #[test]
    fn snapshot_of_the_same_tree_is_the_same_commit() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");
        repo.write("src/a.ts", "export const a = 2;\n");
        repo.write("src/b.ts", "export const b = 1;\n");

        assert_eq!(snapshot(&repo), snapshot(&repo));
    }

    #[test]
    fn snapshot_changes_nothing() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.write("src/b.ts", "export const b = 1;\n");
        repo.write("src/s.ts", "export const s = 1;\n");
        repo.commit("base");
        // A stash entry, which the check must keep.
        repo.write("src/s.ts", "export const s = 2;\n");
        repo.git(&["stash", "-q"]);
        repo.write("src/a.ts", "export const a = 2;\n");
        repo.git(&["add", "src/a.ts"]);
        repo.write("src/b.ts", "export const b = 2;\n");
        repo.write("src/c.ts", "export const c = 1;\n");
        let before = repository_state(&repo);

        snapshot(&repo);

        assert_eq!(repository_state(&repo), before);
        assert_eq!(private_indexes(&repo), Vec::<String>::new());
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_runs_no_hooks() {
        use std::os::unix::fs::PermissionsExt;

        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");
        let hooks = repo.dir.join(".git/hooks");
        std::fs::create_dir_all(&hooks).expect("hooks directory");
        for hook in ["pre-commit", "post-commit", "post-index-change"] {
            let path = hooks.join(hook);
            let marker = repo.dir.join(format!(".git/ran-{hook}"));
            std::fs::write(&path, format!("#!/bin/sh\ntouch '{}'\n", marker.display()))
                .expect("write hook");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("make hook executable");
        }
        repo.write("src/a.ts", "export const a = 2;\n");
        repo.write("src/b.ts", "export const b = 1;\n");

        snapshot(&repo);

        let ran = |hook: &str| repo.dir.join(format!(".git/ran-{hook}")).exists();
        for hook in ["pre-commit", "post-commit", "post-index-change"] {
            assert!(!ran(hook), "the {hook} hook ran");
        }
        // The hooks do work: a plain `git add` runs post-index-change.
        repo.git(&["add", "-A"]);
        assert!(ran("post-index-change"));
    }

    #[test]
    fn snapshot_without_identity_and_with_signing_configured() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");
        // No identity may be guessed, and signing would fail.
        repo.git(&["config", "user.useConfigOnly", "true"]);
        repo.git(&["config", "commit.gpgSign", "true"]);
        repo.git(&["config", "gpg.program", "/nonexistent/qgate-test-gpg"]);
        repo.write("src/a.ts", "export const a = 2;\n");

        let snapshot = snapshot(&repo);

        assert_eq!(
            repo.git(&["rev-parse", &format!("{snapshot}^")]),
            repo.git(&["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn repository_without_commits_has_no_head() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");

        assert_eq!(
            repository(&repo).worktree_snapshot(),
            Err(GitError::UnknownRevision {
                revision: "HEAD".to_owned()
            })
        );
        assert_eq!(private_indexes(&repo), Vec::<String>::new());
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_is_a_work_tree_error() {
        use std::os::unix::fs::PermissionsExt;

        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");
        // Even with this, `git add` still fails on the file.
        repo.git(&["config", "add.ignoreErrors", "true"]);
        repo.write("src/secret.ts", "export const s = 1;\n");
        let secret = repo.dir.join("src/secret.ts");
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000))
            .expect("make the file unreadable");
        if std::fs::read(&secret).is_ok() {
            // Running as root: nothing is unreadable.
            return;
        }

        let result = repository(&repo).worktree_snapshot();

        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644))
            .expect("make the file readable again");
        match result {
            Err(GitError::WorkTree { detail }) => {
                assert!(detail.contains("src/secret.ts"), "{detail}");
            }
            other => panic!("expected a work tree error, got {other:?}"),
        }
        assert_eq!(private_indexes(&repo), Vec::<String>::new());
    }
}
