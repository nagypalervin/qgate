//! The working tree as a commit (design D1 and D2 of `add-worktree-input`).
//!
//! [`Repository::worktree_snapshot`] stages the whole working tree into a
//! private copy of the index, writes it as a tree and wraps the tree in a
//! commit that no ref points to. Every other method then reads that commit
//! like any revision. The user's index, refs, reflog and stash stay as they
//! were; the only trace is unreachable objects, which `git gc` prunes.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{GitError, Repository, command_error, git_command, spawn_error, stderr_of, stdout_of};

/// Configuration for every snapshot command:
///
/// - Git runs the `post-index-change` hook on every index write, even of a
///   private index, so hooks are pointed at a directory that holds none.
/// - With `core.splitIndex`, writing the private index can create a new
///   `sharedindex.*` file in the git directory, which would outlive the
///   private index; it is written as one plain file instead.
const SNAPSHOT_CONFIG: [&str; 4] = [
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.splitIndex=false",
];

/// How many names [`PrivateIndex::reserve`] tries before giving up.
const RESERVE_ATTEMPTS: usize = 100;

/// How old a private index must be before a later snapshot removes it as
/// left behind by an interrupted run. No snapshot takes anywhere near this
/// long, so a live one is never removed.
const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

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
    /// were. No hook runs, except a configured file-system monitor
    /// (`core.fsmonitor`), which only tells git which files changed. The
    /// same working tree gives the same id.
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

/// A copy of the index in the git directory (where `--git-path` puts
/// per-work-tree files), removed with its lock file when dropped. It stays
/// in the repository, not the system temp directory, because it belongs to
/// the repository and is never visible in the work tree (design D1). A
/// copy that an interrupted run left behind is removed by a later run
/// once it is a day old.
struct PrivateIndex {
    path: PathBuf,
}

impl PrivateIndex {
    /// A private index holding what the index git would use holds, or
    /// `head`'s tree if there is no index yet.
    fn create(repo: &Repository, head: &str) -> Result<Self, GitError> {
        // `--git-path` follows `GIT_INDEX_FILE` and linked work trees.
        let args = [
            "rev-parse",
            "--git-path",
            "index",
            "--git-path",
            "qgate-index",
        ];
        let out = repo.git(&args)?;
        if !out.status.success() {
            return Err(command_error(&args, &out));
        }
        let mut lines = out.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty());
        let (Some(real), Some(stem)) = (lines.next(), lines.next()) else {
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
        let stem = absolute(stem);
        remove_stale(&stem);
        let (index, mut file) = Self::reserve(stem.as_os_str())?;
        let copy_error = |e: std::io::Error| GitError::WorkTree {
            detail: format!("cannot copy the index {}: {e}", real.display()),
        };
        match File::open(&real) {
            Ok(mut source) => {
                // Copied through the reserved handle, and dated now: the age
                // is how `remove_stale` tells a copy in use from one left
                // behind, and `std::fs::copy` keeps the source's (often
                // days-old) time on some platforms.
                std::io::copy(&mut source, &mut file).map_err(copy_error)?;
                // Writing through the handle already dates it now; this
                // makes it explicit. A file system that cannot set times
                // must not stop the check, so a failure is ignored.
                let _ = file.set_modified(SystemTime::now());
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                drop(file);
                // `read-tree` writes a new file, dated now.
                repo.snapshot_checked(&["read-tree", head], &index)?;
            }
            Err(e) => return Err(copy_error(e)),
        }
        Ok(index)
    }

    /// A new, empty file named `<stem>-<pid>-<nanoseconds>-<n>`, with its
    /// open handle. It is created exclusively, so no other process uses the
    /// same name, not even one with the same pid in another container that
    /// shares the repository; a name whose lock file exists is skipped.
    /// Another run's private index is only ever removed by
    /// [`remove_stale`], once it is a day old.
    fn reserve(stem: &OsStr) -> Result<(Self, File), GitError> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        for _ in 0..RESERVE_ATTEMPTS {
            let mut name = stem.to_os_string();
            name.push(format!(
                "-{}-{nanos}-{}",
                std::process::id(),
                NEXT_INDEX.fetch_add(1, Ordering::Relaxed)
            ));
            let path = PathBuf::from(name);
            if Path::new(&lock_of(&path)).exists() {
                continue;
            }
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok((Self { path }, file)),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
                Err(e) => {
                    return Err(GitError::WorkTree {
                        detail: format!("cannot create {}: {e}", path.display()),
                    });
                }
            }
        }
        Err(GitError::WorkTree {
            detail: format!(
                "no free name for a private index next to {}",
                Path::new(stem).display()
            ),
        })
    }

    /// Remove the private index and its lock file, if they exist.
    fn remove(&self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(lock_of(&self.path));
    }
}

impl Drop for PrivateIndex {
    fn drop(&mut self) {
        self.remove();
    }
}

/// Remove the private indexes, and their lock files, that interrupted runs
/// left next to `stem` more than [`STALE_AFTER`] ago. Errors are ignored:
/// a file that cannot be removed now can be removed by a later run.
fn remove_stale(stem: &Path) {
    let (Some(dir), Some(name)) = (stem.parent(), stem.file_name()) else {
        return;
    };
    let prefix = format!("{}-", name.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(&prefix) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > STALE_AFTER);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The lock file git writes next to an index while it updates it.
fn lock_of(path: &Path) -> std::ffi::OsString {
    let mut lock = path.as_os_str().to_os_string();
    lock.push(".lock");
    lock
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
        private_indexes_in(&repo.dir.join(".git"))
    }

    /// Files named like a private index in `git_dir`.
    fn private_indexes_in(git_dir: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(git_dir)
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
        repo.write("src/assumed.ts", "export const u = 1;\n");
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
        repo.git(&["update-index", "--assume-unchanged", "src/assumed.ts"]);
        repo.write("src/assumed.ts", "export const u = 2;\n");

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

        let first = snapshot(&repo);
        assert_eq!(snapshot(&repo), first);
        // The same id on any day: the date is fixed, not just the same second.
        let commit = repo.git(&["cat-file", "commit", &first]);
        assert!(
            commit.contains("\nauthor qgate <qgate@localhost> 0 +0000\n"),
            "{commit}"
        );
        assert!(
            commit.contains("\ncommitter qgate <qgate@localhost> 0 +0000\n"),
            "{commit}"
        );
    }

    #[test]
    fn snapshot_in_a_linked_worktree() {
        /// Removes the linked work tree even when an assertion fails.
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.write("src/kept.ts", "export const k = 1;\n");
        repo.commit("base");
        let linked = crate::test_support::scratch_dir("linked");
        let _cleanup = Cleanup(linked.clone());
        std::fs::remove_dir(&linked).expect("an empty directory to replace");
        let linked_arg = linked.to_str().expect("UTF-8");
        repo.git(&["worktree", "add", "-q", "-b", "side", linked_arg]);
        // Only the linked work tree's own index marks this file, so a
        // snapshot that copied the main index would report it.
        repo.git(&[
            "-C",
            linked_arg,
            "update-index",
            "--skip-worktree",
            "src/kept.ts",
        ]);
        std::fs::write(linked.join("src/kept.ts"), "export const k = 2;\n").expect("edit");
        std::fs::write(linked.join("src/a.ts"), "export const a = 2;\n").expect("edit");
        std::fs::write(linked.join("b.ts"), "export const b = 1;\n").expect("add");

        let in_linked = Repository::discover(&linked).expect("a repository");
        let taken = in_linked.worktree_snapshot().expect("a snapshot");
        let paths: Vec<String> = in_linked
            .changed_files("HEAD", &taken)
            .expect("changes")
            .into_iter()
            .map(|f| f.path)
            .collect();

        assert_eq!(paths, vec!["b.ts", "src/a.ts"]);
        let linked_git_dir = repo.git(&["-C", linked_arg, "rev-parse", "--absolute-git-dir"]);
        assert_eq!(
            private_indexes_in(std::path::Path::new(&linked_git_dir)),
            Vec::<String>::new()
        );
        // The main work tree's snapshot does not see the linked one's edits.
        assert_eq!(
            repository(&repo)
                .changed_files("HEAD", &snapshot(&repo))
                .expect("changes"),
            vec![]
        );
        repo.git(&["worktree", "remove", "--force", linked_arg]);
    }

    #[test]
    fn private_index_is_dated_now_even_when_the_index_is_old() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");
        let two_days_ago = SystemTime::now() - Duration::from_secs(2 * 24 * 60 * 60);
        File::options()
            .write(true)
            .open(repo.dir.join(".git/index"))
            .and_then(|f| f.set_modified(two_days_ago))
            .expect("date the index two days back");
        let head = repo.git(&["rev-parse", "HEAD"]);

        let index = PrivateIndex::create(&repository(&repo), &head).expect("a private index");

        let modified = std::fs::metadata(&index.path)
            .and_then(|m| m.modified())
            .expect("the private index's time");
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO);
        assert!(age < Duration::from_secs(60 * 60), "dated {age:?} ago");
        // So another run's clean-up leaves it alone.
        remove_stale(&index.path.with_file_name("qgate-index"));
        assert!(index.path.exists());
    }

    #[test]
    fn stale_private_indexes_are_removed_and_recent_ones_kept() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("base");
        let git_dir = repo.dir.join(".git");
        let two_days_ago = SystemTime::now() - Duration::from_secs(2 * 24 * 60 * 60);
        let file = |name: &str, modified: Option<SystemTime>| {
            let path = git_dir.join(name);
            std::fs::write(&path, "left behind").expect("write");
            if let Some(time) = modified {
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .and_then(|f| f.set_modified(time))
                    .expect("set the modification time");
            }
            path
        };
        let old = file("qgate-index-1-2-3", Some(two_days_ago));
        let old_lock = file("qgate-index-1-2-3.lock", Some(two_days_ago));
        let recent = file("qgate-index-4-5-6", None);
        let unrelated = file("qgate-indexer", Some(two_days_ago));

        snapshot(&repo);

        assert!(!old.exists(), "the stale private index is still there");
        assert!(!old_lock.exists(), "the stale lock is still there");
        assert!(recent.exists(), "a recent private index was removed");
        assert!(unrelated.exists(), "an unrelated file was removed");
    }

    #[test]
    fn snapshot_without_an_index_starts_from_head() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.write("src/b.ts", "export const b = 1;\n");
        repo.commit("base");
        std::fs::remove_file(repo.dir.join(".git/index")).expect("remove the index");
        repo.write("src/b.ts", "export const b = 2;\n");

        let snapshot = snapshot(&repo);
        let paths: Vec<String> = repository(&repo)
            .changed_files("HEAD", &snapshot)
            .expect("changes")
            .into_iter()
            .map(|f| f.path)
            .collect();

        assert_eq!(paths, vec!["src/b.ts"]);
        assert!(
            !repo.dir.join(".git/index").exists(),
            "an index was created"
        );
    }

    #[test]
    fn snapshot_with_a_split_index_leaves_no_shared_index() {
        let repo = TestRepo::new();
        repo.git(&["config", "core.splitIndex", "true"]);
        for i in 0..20 {
            repo.write(&format!("src/f{i}.ts"), format!("export const f = {i};\n"));
        }
        repo.commit("base");
        // More than splitIndex.maxPercentChange (20%) of the entries change.
        for i in 0..15 {
            repo.write(&format!("src/f{i}.ts"), format!("export const f = -{i};\n"));
        }
        for i in 0..10 {
            repo.write(&format!("src/n{i}.ts"), format!("export const n = {i};\n"));
        }
        let shared = || {
            let mut names: Vec<String> = std::fs::read_dir(repo.dir.join(".git"))
                .expect("the git directory")
                .map(|e| {
                    e.expect("an entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .filter(|name| name.starts_with("sharedindex."))
                .collect();
            names.sort();
            names
        };
        let before = shared();
        assert!(!before.is_empty(), "the repository has no split index");

        let snapshot = snapshot(&repo);

        assert_eq!(shared(), before);
        assert_eq!(
            repository(&repo)
                .changed_files("HEAD", &snapshot)
                .expect("changes")
                .len(),
            25
        );
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
