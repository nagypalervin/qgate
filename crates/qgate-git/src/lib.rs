//! Changed files and file versions between two git revisions.
//!
//! Everything goes through the `git` command (ADR 0002): no git library,
//! no dependencies. The crate returns plain data and errors and never
//! prints; it knows nothing about languages or findings.
//!
//! - [`Repository::discover`] finds the work tree containing a directory.
//! - [`Repository::changed_files`] lists the regular files changed from the
//!   merge base of two revisions to the second one, as a pull request shows
//!   them. Renames carry their old path; deleted files, symlinks,
//!   submodules and type changes are left out.
//! - [`Repository::read_blob`] returns the exact bytes of one version.
//!
//! User-supplied revisions only ever reach `git rev-parse --verify
//! --end-of-options`; later commands receive full commit ids, so a revision
//! cannot act as an option. Requires git 2.24 or newer.
//!
//! ```no_run
//! use std::path::Path;
//! use qgate_git::Repository;
//!
//! let repo = Repository::discover(Path::new("."))?;
//! for file in repo.changed_files("origin/main", "HEAD")? {
//!     let after = repo.read_blob(&file.after)?;
//!     println!("{}: {} bytes", file.path, after.len());
//! }
//! # Ok::<(), qgate_git::GitError>(())
//! ```

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
        let (merge_base, head) = self.merge_base_of(base, head)?;
        let args = [
            "diff",
            "--raw",
            "-z",
            "-M",
            "--no-abbrev",
            "--no-ext-diff",
            "--no-textconv",
            &merge_base,
            &head,
        ];
        let out = self.git(&args)?;
        if !out.status.success() {
            return Err(command_error(&args, &out));
        }
        let mut files = parse_raw_diff(&out.stdout).map_err(|detail| GitError::Command {
            args: args.iter().map(|&a| a.to_owned()).collect(),
            status: out.status.code(),
            stderr: format!("unexpected `git diff --raw` output: {detail}"),
        })?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// The raw contents of a blob.
    ///
    /// # Errors
    ///
    /// A [`GitError`] if git fails.
    pub fn read_blob(&self, id: &BlobId) -> Result<Vec<u8>, GitError> {
        // `blob` instead of `-p`: raw bytes, no textconv, and the id comes
        // from git itself, never from the user.
        let args = ["cat-file", "blob", id.as_str()];
        let out = self.git(&args)?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(command_error(&args, &out))
        }
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

/// Modes of regular files; everything else (symlinks `120000`, submodules
/// `160000`) is skipped.
const REGULAR_MODES: [&str; 2] = ["100644", "100755"];

/// Parse `git diff --raw -z` output into the changed regular files.
///
/// Each record is `:<old-mode> <new-mode> <old-id> <new-id> <status>`,
/// NUL, path, NUL; renames and copies (`R<score>`, `C<score>`) carry the
/// old path and then the new path.
fn parse_raw_diff(raw: &[u8]) -> Result<Vec<ChangedFile>, String> {
    let mut fields = raw.split(|&b| b == 0);
    let mut files = Vec::new();
    while let Some(header) = fields.next() {
        if header.is_empty() {
            break;
        }
        let header = String::from_utf8_lossy(header);
        let parts: Vec<&str> = header
            .strip_prefix(':')
            .ok_or_else(|| format!("record does not start with `:`: {header}"))?
            .split(' ')
            .collect();
        let [old_mode, new_mode, old_id, new_id, status] = parts[..] else {
            return Err(format!("expected 5 fields: {header}"));
        };
        let mut path = || {
            fields
                .next()
                .map(|p| String::from_utf8_lossy(p).into_owned())
                .ok_or_else(|| format!("missing path after {header}"))
        };
        let (old_path, path) = match status.chars().next() {
            Some('R' | 'C') => {
                let old = path()?;
                (Some(old), path()?)
            }
            _ => (None, path()?),
        };

        let regular = |mode: &str| REGULAR_MODES.contains(&mode);
        let before = match status.chars().next() {
            Some('A') if regular(new_mode) => None,
            Some('M' | 'R' | 'C') if regular(old_mode) && regular(new_mode) => {
                Some(BlobId(old_id.to_owned()))
            }
            // Deleted, type change, symlink, submodule, unmerged: skipped.
            _ => continue,
        };
        files.push(ChangedFile {
            path,
            old_path,
            before,
            after: BlobId(new_id.to_owned()),
        });
    }
    Ok(files)
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

    /// Blob id of `path` at `revision`, as git reports it.
    fn blob(repo: &TestRepo, revision: &str, path: &str) -> BlobId {
        BlobId(repo.git(&["rev-parse", &format!("{revision}:{path}")]))
    }

    fn changes(repo: &TestRepo, base: &str, head: &str) -> Vec<ChangedFile> {
        Repository::discover(&repo.dir)
            .expect("a repository")
            .changed_files(base, head)
            .expect("changed files")
    }

    #[test]
    fn modified_added_deleted_and_renamed_files() {
        let repo = TestRepo::new();
        repo.write("src/mod.ts", "export const a = 1;\n");
        repo.write("src/gone.ts", "export const g = 1;\n");
        repo.write(
            "src/old.ts",
            "export function f() {\n  return 1;\n}\n// padding\n// padding\n",
        );
        repo.write("src/util.js", "module.exports = { a: 1, b: 2, c: 3 };\n");
        repo.commit("base");
        repo.write("src/mod.ts", "export const a = 2;\n");
        repo.write("src/added.ts", "export const n = 1;\n");
        std::fs::remove_file(repo.dir.join("src/gone.ts")).expect("delete");
        repo.git(&["mv", "src/old.ts", "src/new.ts"]);
        repo.write(
            "src/new.ts",
            "export function f() {\n  return 2;\n}\n// padding\n// padding\n",
        );
        repo.git(&["mv", "src/util.js", "src/util.ts"]);
        repo.commit("head");

        assert_eq!(
            changes(&repo, "HEAD~1", "HEAD"),
            vec![
                ChangedFile {
                    path: "src/added.ts".to_owned(),
                    old_path: None,
                    before: None,
                    after: blob(&repo, "HEAD", "src/added.ts"),
                },
                ChangedFile {
                    path: "src/mod.ts".to_owned(),
                    old_path: None,
                    before: Some(blob(&repo, "HEAD~1", "src/mod.ts")),
                    after: blob(&repo, "HEAD", "src/mod.ts"),
                },
                // renamed and modified: compared with the old path
                ChangedFile {
                    path: "src/new.ts".to_owned(),
                    old_path: Some("src/old.ts".to_owned()),
                    before: Some(blob(&repo, "HEAD~1", "src/old.ts")),
                    after: blob(&repo, "HEAD", "src/new.ts"),
                },
                // renamed from an unsupported extension: still a rename here;
                // the CLI decides how to treat it
                ChangedFile {
                    path: "src/util.ts".to_owned(),
                    old_path: Some("src/util.js".to_owned()),
                    before: Some(blob(&repo, "HEAD~1", "src/util.js")),
                    after: blob(&repo, "HEAD", "src/util.ts"),
                },
                // src/gone.ts is deleted: not reported
            ]
        );
    }

    #[test]
    fn no_changes() {
        let repo = TestRepo::new();
        repo.write("a.ts", "x");
        repo.commit("only");
        assert_eq!(changes(&repo, "HEAD", "HEAD"), vec![]);
    }

    #[test]
    fn changes_on_base_after_the_fork_are_not_reported() {
        let repo = TestRepo::new();
        repo.write("a.ts", "v1");
        repo.commit("fork point");
        repo.git(&["checkout", "-q", "-b", "feature"]);
        repo.write("b.ts", "feature");
        repo.commit("feature work");
        repo.git(&["checkout", "-q", "main"]);
        repo.write("a.ts", "v2 on main");
        repo.commit("main work");
        let paths: Vec<String> = changes(&repo, "main", "feature")
            .into_iter()
            .map(|c| c.path)
            .collect();
        assert_eq!(paths, ["b.ts"]);
    }

    #[test]
    fn executable_files_are_regular_files() {
        let repo = TestRepo::new();
        repo.write("run.ts", "v1");
        repo.commit("base");
        repo.write("run.ts", "v2");
        repo.git(&["update-index", "--chmod=+x", "run.ts"]);
        repo.git(&["commit", "-q", "-m", "executable"]);
        let paths: Vec<String> = changes(&repo, "HEAD~1", "HEAD")
            .into_iter()
            .map(|c| c.path)
            .collect();
        assert_eq!(paths, ["run.ts"]);
    }

    #[test]
    fn submodule_entries_are_skipped() {
        let repo = TestRepo::new();
        let first = repo.commit("base");
        // A gitlink entry without a second repository (design D5).
        repo.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{first},vendor/lib"),
        ]);
        repo.git(&["commit", "-q", "-m", "submodule"]);
        assert_eq!(changes(&repo, "HEAD~1", "HEAD"), vec![]);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_type_changes_are_skipped() {
        let repo = TestRepo::new();
        repo.write("target.ts", "x");
        repo.write("became-link.ts", "a regular file first");
        repo.commit("base");
        std::os::unix::fs::symlink("target.ts", repo.dir.join("link.ts")).expect("symlink");
        std::fs::remove_file(repo.dir.join("became-link.ts")).expect("delete");
        std::os::unix::fs::symlink("target.ts", repo.dir.join("became-link.ts")).expect("symlink");
        repo.commit("links");
        assert_eq!(changes(&repo, "HEAD~1", "HEAD"), vec![]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_paths_are_reported_lossily() {
        use std::os::unix::ffi::OsStrExt as _;
        let repo = TestRepo::new();
        repo.commit("base");
        let name = std::ffi::OsStr::from_bytes(b"bad-\xff.ts");
        std::fs::write(repo.dir.join(name), "x").expect("write");
        repo.commit("odd name");
        let found = changes(&repo, "HEAD~1", "HEAD");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "bad-\u{fffd}.ts");
    }

    #[test]
    fn read_blob_returns_exact_bytes_of_both_versions() {
        let repo = TestRepo::new();
        repo.write("a.ts", "old\r\nversion\n");
        repo.commit("base");
        let new_bytes: &[u8] = b"new \xff\xfe not UTF-8\n";
        repo.write("a.ts", new_bytes);
        repo.commit("head");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let change = found
            .changed_files("HEAD~1", "HEAD")
            .expect("changed files")
            .pop()
            .expect("one change");
        let before = change.before.expect("a modified file has a before blob");
        assert_eq!(
            found.read_blob(&before).expect("old blob"),
            b"old\r\nversion\n"
        );
        assert_eq!(found.read_blob(&change.after).expect("new blob"), new_bytes);
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
