//! Changed files and file versions between two git revisions.
//!
//! Status: types only; the implementation follows.

use std::ffi::OsStr;
use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A git repository, identified by its top-level directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    /// Where git commands run: the directory the repository was found
    /// from. Using it instead of `root` avoids round-tripping a path that
    /// may not be valid UTF-8.
    dir: PathBuf,
    /// The top level of the work tree, for display.
    root: PathBuf,
}

/// The id of a blob (one version of a file's contents), as git prints it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobId(String);

impl BlobId {
    /// The id as a hexadecimal string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A regular file that differs between the two compared revisions.
///
/// Deleted files, symbolic links, submodules and type changes are never
/// reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// Path in the head revision, relative to the repository root. Invalid
    /// UTF-8 in the path is replaced, so it is for display and extension
    /// checks only; contents are read by blob id.
    pub path: String,
    /// Path in the base revision, if git detected a rename.
    pub old_path: Option<String>,
    /// The base version, or `None` for an added file.
    pub before: Option<BlobId>,
    /// The head version.
    pub after: BlobId,
}

/// Why a git operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitError {
    /// The `git` program could not be started.
    GitNotFound,
    /// The directory is not inside a git work tree.
    NotARepository {
        /// What git said.
        detail: String,
    },
    /// A revision does not name a commit.
    UnknownRevision {
        /// The revision as the user gave it.
        revision: String,
    },
    /// The two revisions share no history.
    NoMergeBase {
        /// The base revision as the user gave it.
        base: String,
        /// The head revision as the user gave it.
        head: String,
    },
    /// Any other failure of a git command.
    Command {
        /// The arguments passed to `git`.
        args: Vec<String>,
        /// The exit code, if git exited normally.
        status: Option<i32>,
        /// What git printed on standard error.
        stderr: String,
    },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitNotFound => write!(f, "git is not installed or not on the PATH"),
            Self::NotARepository { detail } => write!(f, "not a git repository: {detail}"),
            Self::UnknownRevision { revision } => write!(f, "unknown revision `{revision}`"),
            Self::NoMergeBase { base, head } => {
                write!(f, "`{base}` and `{head}` have no merge base")
            }
            Self::Command {
                args,
                status,
                stderr,
            } => {
                let code = status.map_or_else(|| "a signal".to_owned(), |c| c.to_string());
                write!(f, "`git {}` failed with {code}: {stderr}", args.join(" "))
            }
        }
    }
}

impl std::error::Error for GitError {}

impl Repository {
    /// The repository whose work tree contains `dir`.
    ///
    /// # Errors
    ///
    /// [`GitError::NotARepository`] if `dir` is not inside a work tree,
    /// [`GitError::GitNotFound`] if `git` cannot be run.
    pub fn discover(dir: &Path) -> Result<Self, GitError> {
        let out = run(dir, &["rev-parse", "--show-toplevel"])?;
        if !out.status.success() {
            return Err(GitError::NotARepository {
                detail: stderr_of(&out),
            });
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            root: PathBuf::from(stdout_of(&out)),
        })
    }

    /// The top-level directory of the work tree.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Regular files changed from the merge base of `base` and `head` to
    /// `head`, sorted by path.
    ///
    /// # Errors
    ///
    /// [`GitError::UnknownRevision`] if a revision does not name a commit,
    /// [`GitError::NoMergeBase`] if they share no history, or another
    /// [`GitError`] if git fails.
    pub fn changed_files(&self, base: &str, head: &str) -> Result<Vec<ChangedFile>, GitError> {
        let (_merge_base, _head) = self.merge_base_of(base, head)?;
        Err(not_implemented())
    }

    /// The raw contents of a blob.
    ///
    /// # Errors
    ///
    /// A [`GitError`] if git fails.
    pub fn read_blob(&self, id: &BlobId) -> Result<Vec<u8>, GitError> {
        let _ = id;
        Err(not_implemented())
    }
}

impl Repository {
    /// Run git in the repository directory.
    fn git(&self, args: &[&str]) -> Result<Output, GitError> {
        run(&self.dir, args)
    }

    /// The full commit id `revision` names. The revision goes after
    /// `--end-of-options`, so it can never act as an option, and every
    /// later git command only sees the returned id (ADR 0002).
    fn resolve(&self, revision: &str) -> Result<String, GitError> {
        let spec = format!("{revision}^{{commit}}");
        let args = [
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ];
        let out = self.git(&args)?;
        match out.status.code() {
            Some(0) => Ok(stdout_of(&out)),
            // `--verify --quiet` exits with 1 for anything that is not a commit.
            Some(1) => Err(GitError::UnknownRevision {
                revision: revision.to_owned(),
            }),
            _ => Err(command_error(&args, &out)),
        }
    }

    /// The commit ids of merge-base(`base`, `head`) and of `head`.
    fn merge_base_of(&self, base: &str, head: &str) -> Result<(String, String), GitError> {
        let base_id = self.resolve(base)?;
        let head_id = self.resolve(head)?;
        let args = ["merge-base", &base_id, &head_id];
        let out = self.git(&args)?;
        match out.status.code() {
            Some(0) => Ok((stdout_of(&out), head_id)),
            // merge-base exits with 1 and prints nothing when there is none.
            Some(1) => Err(GitError::NoMergeBase {
                base: base.to_owned(),
                head: head.to_owned(),
            }),
            _ => Err(command_error(&args, &out)),
        }
    }
}

/// Run `git` with `args` in `dir`. Prompts are disabled, so a command can
/// never wait for input.
fn run<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Result<Output, GitError> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => GitError::GitNotFound,
            _ => GitError::Command {
                args: args
                    .iter()
                    .map(|a| a.as_ref().to_string_lossy().into_owned())
                    .collect(),
                status: None,
                stderr: e.to_string(),
            },
        })
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim_end().to_owned()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_owned()
}

fn command_error(args: &[&str], out: &Output) -> GitError {
    GitError::Command {
        args: args.iter().map(|&a| a.to_owned()).collect(),
        status: out.status.code(),
        stderr: stderr_of(out),
    }
}

/// Placeholder until the methods are implemented.
fn not_implemented() -> GitError {
    GitError::Command {
        args: Vec::new(),
        status: None,
        stderr: "not implemented yet".to_owned(),
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestRepo, canonical, scratch_dir};

    #[test]
    fn discover_at_the_root() {
        let repo = TestRepo::new();
        let found = Repository::discover(&repo.dir).expect("a repository");
        assert_eq!(canonical(found.root()), canonical(&repo.dir));
    }

    #[test]
    fn discover_from_a_subdirectory() {
        let repo = TestRepo::new();
        repo.write("src/deep/a.ts", "");
        let found = Repository::discover(&repo.dir.join("src/deep")).expect("a repository");
        assert_eq!(canonical(found.root()), canonical(&repo.dir));
    }

    #[test]
    fn discover_outside_a_repository() {
        let dir = scratch_dir("plain");
        let result = Repository::discover(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(result, Err(GitError::NotARepository { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn unknown_revision() {
        let repo = TestRepo::new();
        repo.commit("first");
        let found = Repository::discover(&repo.dir).expect("a repository");
        assert_eq!(
            found.changed_files("no-such-branch", "HEAD"),
            Err(GitError::UnknownRevision {
                revision: "no-such-branch".to_owned()
            })
        );
        assert_eq!(
            found.changed_files("HEAD", "no-such-head"),
            Err(GitError::UnknownRevision {
                revision: "no-such-head".to_owned()
            })
        );
    }

    #[test]
    fn option_like_revision_is_not_an_option() {
        let repo = TestRepo::new();
        repo.commit("first");
        let target = repo.dir.join("written-by-git");
        let revision = format!("--output={}", target.display());
        let found = Repository::discover(&repo.dir).expect("a repository");
        assert_eq!(
            found.changed_files(&revision, "HEAD"),
            Err(GitError::UnknownRevision { revision })
        );
        assert!(!target.exists(), "git treated the revision as an option");
    }

    #[test]
    fn unrelated_histories_have_no_merge_base() {
        let repo = TestRepo::new();
        repo.commit("first");
        repo.git(&["checkout", "-q", "--orphan", "other"]);
        repo.commit("unrelated");
        let found = Repository::discover(&repo.dir).expect("a repository");
        assert_eq!(
            found.changed_files("main", "other"),
            Err(GitError::NoMergeBase {
                base: "main".to_owned(),
                head: "other".to_owned()
            })
        );
    }

    #[test]
    fn merge_base_ignores_later_commits_on_base() {
        let repo = TestRepo::new();
        let fork = repo.commit("fork point");
        repo.git(&["checkout", "-q", "-b", "feature"]);
        repo.write("f.ts", "feature");
        let head = repo.commit("feature work");
        repo.git(&["checkout", "-q", "main"]);
        repo.write("m.ts", "main moved on");
        repo.commit("main work");
        let found = Repository::discover(&repo.dir).expect("a repository");
        assert_eq!(found.merge_base_of("main", "feature"), Ok((fork, head)));
    }
}
