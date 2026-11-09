//! Changed files and file versions between two git revisions.
//!
//! Status: types only; the implementation follows.

use std::fmt;
use std::path::{Path, PathBuf};

/// A git repository, identified by its top-level directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
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
        let _ = dir;
        Err(not_implemented())
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
        let _ = (base, head);
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

/// Placeholder until the methods are implemented.
fn not_implemented() -> GitError {
    GitError::Command {
        args: Vec::new(),
        status: None,
        stderr: "not implemented yet".to_owned(),
    }
}
