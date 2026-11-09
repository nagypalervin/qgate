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
//! - [`Repository::file_at`] reads one regular file, such as `qgate.toml`,
//!   as a revision has it; the path is taken literally.
//! - [`Repository::changes`] adds the deleted regular files, with their
//!   base contents, to the changed ones, from the same diff.
//! - [`Repository::files_at`] lists every regular file of a revision, and
//!   [`Repository::blob_reader`] reads many blobs through one
//!   `git cat-file --batch` process, for analyses that need a whole tree.
//!
//! User-supplied revisions starting with `-` are rejected, the rest only
//! reach `git rev-parse --verify`; later commands receive full commit ids,
//! so a revision cannot act as an option. Commands run at the top level of
//! the work tree, so settings such as `diff.relative` cannot narrow them.
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
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Output, Stdio};
use std::thread::JoinHandle;

/// A git repository, identified by its top-level directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    /// Where git commands run: the top level, reached as the discovery
    /// directory plus `git rev-parse --show-cdup` (only `../` parts), so a
    /// top-level path that is not valid UTF-8 never round-trips through a
    /// `String`, and configuration like `diff.relative` cannot narrow the
    /// diff to the current directory.
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

/// One regular file of a revision's tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeFile {
    /// Path from the repository root, `/`-separated. Bytes that are not
    /// UTF-8 are replaced, so two such paths can look alike: use it for
    /// display and extension checks, and [`TreeFile::id`] for contents.
    pub path: String,
    /// The file's contents, for [`BlobReader::read`] or
    /// [`Repository::read_blob`].
    pub id: BlobId,
}

/// Reads many blobs through one long-running `git cat-file --batch`
/// process (ADR 0002, "Consequences"). Dropping it ends the process.
///
/// After an error that leaves the process's answers out of step (an I/O
/// error, the process ending, or an answer it cannot parse) the reader is
/// closed, and every later `read` fails at once.
#[derive(Debug)]
pub struct BlobReader {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    /// Drains git's stderr while it runs, so git can never block on a full
    /// pipe, and keeps the last [`STDERR_KEPT`] bytes for error messages.
    stderr: Option<JoinHandle<Vec<u8>>>,
}

/// How much of git's stderr a [`BlobReader`] keeps: the end, which holds
/// the error.
const STDERR_KEPT: usize = 8 * 1024;

/// The largest blob a [`BlobReader`] reads; larger ones are skipped with
/// an error rather than allocated. Source files are far smaller.
#[cfg(not(test))]
const MAX_BLOB_SIZE: usize = 64 * 1024 * 1024;
/// Smaller in tests, so a test can exceed it without a huge file.
#[cfg(test)]
const MAX_BLOB_SIZE: usize = 2 * 1024 * 1024;

/// Arguments of the batch process, for error messages.
const BATCH_ARGS: [&str; 2] = ["cat-file", "--batch"];

/// What went wrong in one request.
enum Broken {
    /// Writing or reading failed, or the answers ended: the process is
    /// gone or going.
    Io(std::io::Error),
    /// An answer that does not fit the protocol.
    Protocol(String),
}

impl BlobReader {
    /// The raw contents of a blob.
    ///
    /// # Errors
    ///
    /// - [`GitError::UnexpectedOutput`] if `id` names no object or not a
    ///   blob (the reader stays usable), if git answers something else, or
    ///   if the reader was closed by an earlier error;
    /// - [`GitError::Command`] with git's exit status and standard error if
    ///   the process ended or could not be written to or read from.
    pub fn read(&mut self, id: &BlobId) -> Result<Vec<u8>, GitError> {
        let unexpected = |detail: String| GitError::UnexpectedOutput {
            args: BATCH_ARGS.iter().map(|&a| a.to_owned()).collect(),
            detail,
        };
        let (Some(stdin), Some(stdout)) = (self.stdin.as_mut(), self.stdout.as_mut()) else {
            return Err(unexpected(
                "the batch process was closed after an earlier error".to_owned(),
            ));
        };
        match Self::exchange(stdin, stdout, id) {
            Ok(Ok(contents)) => Ok(contents),
            // Still in step: the next request can go ahead.
            Ok(Err(detail)) => Err(unexpected(detail)),
            Err(Broken::Io(error)) => Err(self.fail(&error)),
            Err(Broken::Protocol(detail)) => {
                self.close();
                Err(unexpected(detail))
            }
        }
    }

    /// One request and its answer: the blob, or why `id` is not one.
    fn exchange(
        stdin: &mut ChildStdin,
        stdout: &mut BufReader<ChildStdout>,
        id: &BlobId,
    ) -> Result<Result<Vec<u8>, String>, Broken> {
        writeln!(stdin, "{}", id.as_str()).map_err(Broken::Io)?;
        stdin.flush().map_err(Broken::Io)?;

        // `<id> <type> <size>`, or `<id> missing`.
        let mut header = Vec::new();
        if stdout.read_until(b'\n', &mut header).map_err(Broken::Io)? == 0 {
            return Err(Broken::Io(ErrorKind::UnexpectedEof.into()));
        }
        let header = String::from_utf8(header)
            .map_err(|_| Broken::Protocol("an answer that is not UTF-8".to_owned()))?;
        let header = header.trim_end();
        let fields: Vec<&str> = header.split(' ').collect();
        if fields.first() != Some(&id.as_str()) {
            return Err(Broken::Protocol(format!(
                "asked for `{}`, got `{header}`",
                id.as_str()
            )));
        }
        let (kind, size) = match fields[..] {
            [_, "missing"] => return Ok(Err(format!("`{}` names no object", id.as_str()))),
            // `u64`, so a blob too large for `usize` is skipped like any
            // other oversized one.
            [_, kind, size] => match size.parse::<u64>() {
                Ok(size) => (kind, size),
                Err(_) => return Err(Broken::Protocol(format!("bad size in `{header}`"))),
            },
            _ => return Err(Broken::Protocol(format!("unexpected answer `{header}`"))),
        };
        let size = match usize::try_from(size) {
            Ok(size) if kind == "blob" && size <= MAX_BLOB_SIZE => size,
            _ => {
                // Skip the object and its newline without keeping it, so
                // the next answer starts cleanly.
                let skip = size.saturating_add(1);
                let skipped = std::io::copy(&mut stdout.by_ref().take(skip), &mut std::io::sink())
                    .map_err(Broken::Io)?;
                if skipped < skip {
                    return Err(Broken::Io(ErrorKind::UnexpectedEof.into()));
                }
                return Ok(Err(if kind == "blob" {
                    format!("`{}` is larger than {MAX_BLOB_SIZE} bytes", id.as_str())
                } else {
                    format!("`{}` is a {kind}, not a blob", id.as_str())
                }));
            }
        };
        let mut contents = vec![0; size + 1];
        stdout.read_exact(&mut contents).map_err(Broken::Io)?;
        // Each object ends with a newline that is not part of it.
        if contents.pop() != Some(b'\n') {
            return Err(Broken::Protocol(format!(
                "`{}`: no newline after the object",
                id.as_str()
            )));
        }
        Ok(Ok(contents))
    }

    /// Close the reader after `error`, wait for the process and report how
    /// it ended, with what it wrote on standard error.
    fn fail(&mut self, error: &std::io::Error) -> GitError {
        self.close();
        let status = self.child.wait().ok().and_then(|s| s.code());
        // The drain ends at end of file, once git has exited.
        let stderr = self
            .stderr
            .take()
            .and_then(|drain| drain.join().ok())
            .unwrap_or_default();
        let stderr = String::from_utf8_lossy(&stderr);
        let stderr = stderr.trim();
        GitError::Command {
            args: BATCH_ARGS.iter().map(|&a| a.to_owned()).collect(),
            status,
            stderr: if stderr.is_empty() {
                error.to_string()
            } else {
                stderr.to_owned()
            },
        }
    }

    /// Close both pipes: git exits on end of input, and a write to the
    /// closed output ends it too, so nothing can block on unread answers.
    fn close(&mut self) {
        drop(self.stdin.take());
        drop(self.stdout.take());
    }

    /// Whether an earlier error closed the reader, so every further
    /// [`read`](Self::read) fails.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.stdin.is_none() || self.stdout.is_none()
    }

    #[cfg(test)]
    fn process_id(&self) -> u32 {
        self.child.id()
    }

    /// Send a request without reading its answer.
    #[cfg(test)]
    fn send_only(&mut self, id: &BlobId) {
        let stdin = self.stdin.as_mut().expect("open");
        writeln!(stdin, "{}", id.as_str()).expect("write");
        stdin.flush().expect("flush");
    }

    #[cfg(test)]
    fn kill(&mut self) {
        self.child.kill().expect("kill");
        let _ = self.child.wait();
    }
}

impl Drop for BlobReader {
    fn drop(&mut self) {
        self.close();
        // Wait so no zombie process remains. The stderr drain is not
        // joined: a process git started may still hold stderr open, and
        // the thread ends on its own once nobody does.
        let _ = self.child.wait();
    }
}

/// Read `stderr` to its end on its own thread, keeping the last
/// [`STDERR_KEPT`] bytes.
fn drain_stderr(mut stderr: ChildStderr) -> std::io::Result<JoinHandle<Vec<u8>>> {
    std::thread::Builder::new()
        .name("git-stderr".to_owned())
        .spawn(move || {
            let mut kept = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                match stderr.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        kept.extend_from_slice(&chunk[..n]);
                        if kept.len() > STDERR_KEPT {
                            kept.drain(..kept.len() - STDERR_KEPT);
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            kept
        })
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
    /// Git printed output this crate does not understand.
    UnexpectedOutput {
        /// The arguments passed to `git`.
        args: Vec<String>,
        /// What was unexpected.
        detail: String,
    },
    /// A path names something other than a regular file (a directory, a
    /// symlink or a submodule) in a revision.
    NotARegularFile {
        /// The path, relative to the repository root.
        path: String,
        /// The revision as the caller gave it.
        revision: String,
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
            Self::NotARegularFile { path, revision } => {
                write!(f, "`{path}` is not a regular file in `{revision}`")
            }
            Self::UnexpectedOutput { args, detail } => {
                write!(
                    f,
                    "unexpected output from `git {}`: {detail}",
                    args.join(" ")
                )
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
    /// [`GitError::GitNotFound`] if `git` cannot be run,
    /// [`GitError::UnexpectedOutput`] if git's answer cannot be read.
    pub fn discover(dir: &Path) -> Result<Self, GitError> {
        if !dir.is_dir() {
            return Err(GitError::NotARepository {
                detail: format!("{} is not a directory", dir.display()),
            });
        }
        // Two lines: the way up to the top level (empty at the top), then
        // the top level itself.
        let out = run(dir, &["rev-parse", "--show-cdup", "--show-toplevel"])?;
        if !out.status.success() {
            return Err(GitError::NotARepository {
                detail: stderr_of(&out),
            });
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut lines = stdout.lines();
        let (Some(cdup), Some(top)) = (lines.next(), lines.next()) else {
            return Err(GitError::UnexpectedOutput {
                args: vec![
                    "rev-parse".into(),
                    "--show-cdup".into(),
                    "--show-toplevel".into(),
                ],
                detail: stdout.trim().to_owned(),
            });
        };
        Ok(Self {
            dir: dir.join(cdup),
            root: PathBuf::from(top),
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
        Ok(self.changes(base, head)?.changed)
    }

    /// The changed files, as [`Repository::changed_files`] gives them, and
    /// the regular files the head no longer has as regular files (deleted,
    /// or replaced by a symlink or submodule) with their contents at the
    /// merge base, both sorted by path, from one diff.
    ///
    /// # Errors
    ///
    /// As [`Repository::changed_files`].
    pub fn changes(&self, base: &str, head: &str) -> Result<Changes, GitError> {
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
        let mut diff =
            parse_raw_diff(&out.stdout).map_err(|detail| GitError::UnexpectedOutput {
                args: args.iter().map(|&a| a.to_owned()).collect(),
                detail,
            })?;
        diff.changed.sort_by(|a, b| a.path.cmp(&b.path));
        diff.deleted.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(diff)
    }

    /// The contents of the regular file at `path` (relative to the
    /// repository root, `/`-separated) in the commit `revision` names, or
    /// `None` if that commit has nothing at `path`. The path is taken
    /// literally, never as a pattern.
    ///
    /// # Errors
    ///
    /// [`GitError::UnknownRevision`] if `revision` does not name a commit,
    /// [`GitError::NotARegularFile`] if `path` is a directory, symlink or
    /// submodule, or another [`GitError`] if git fails.
    pub fn file_at(&self, revision: &str, path: &str) -> Result<Option<Vec<u8>>, GitError> {
        let commit = self.resolve(revision)?;
        let args = ["--literal-pathspecs", "ls-tree", "-z", &commit, "--", path];
        let out = self.git(&args)?;
        if !out.status.success() {
            return Err(command_error(&args, &out));
        }
        // Each entry: `<mode> SP <type> SP <id> TAB <path>`, NUL-terminated.
        let unexpected = |detail: String| GitError::UnexpectedOutput {
            args: args.iter().map(|&a| a.to_owned()).collect(),
            detail,
        };
        for entry in out.stdout.split(|&b| b == 0).filter(|e| !e.is_empty()) {
            let entry = String::from_utf8_lossy(entry);
            let (header, entry_path) = entry
                .split_once('\t')
                .ok_or_else(|| unexpected(format!("no tab in entry: {entry}")))?;
            if entry_path != path {
                continue;
            }
            let [mode, kind, id] = header.split(' ').collect::<Vec<_>>()[..] else {
                return Err(unexpected(format!("expected 3 fields: {header}")));
            };
            if kind != "blob" || !REGULAR_MODES.contains(&mode) {
                return Err(GitError::NotARegularFile {
                    path: path.to_owned(),
                    revision: revision.to_owned(),
                });
            }
            return self.read_blob(&BlobId(id.to_owned())).map(Some);
        }
        Ok(None)
    }

    /// The regular files of the commit `revision` names, sorted by path as
    /// a byte string. Symlinks and submodules are left out.
    ///
    /// # Errors
    ///
    /// [`GitError::UnknownRevision`] if `revision` does not name a commit,
    /// or another [`GitError`] if git fails.
    pub fn files_at(&self, revision: &str) -> Result<Vec<TreeFile>, GitError> {
        let commit = self.resolve(revision)?;
        let args = ["ls-tree", "-r", "-z", "--full-tree", &commit];
        let out = self.git(&args)?;
        if !out.status.success() {
            return Err(command_error(&args, &out));
        }
        let mut files = Vec::new();
        // Each entry: `<mode> SP <type> SP <id> TAB <path>`, NUL-terminated.
        for entry in out.stdout.split(|&b| b == 0).filter(|e| !e.is_empty()) {
            let entry = String::from_utf8_lossy(entry);
            let unexpected = |detail: String| GitError::UnexpectedOutput {
                args: args.iter().map(|&a| a.to_owned()).collect(),
                detail,
            };
            let (header, path) = entry
                .split_once('\t')
                .ok_or_else(|| unexpected(format!("no tab in entry: {entry}")))?;
            let [mode, kind, id] = header.split(' ').collect::<Vec<_>>()[..] else {
                return Err(unexpected(format!("expected 3 fields: {header}")));
            };
            if kind == "blob" && REGULAR_MODES.contains(&mode) {
                files.push(TreeFile {
                    path: path.to_owned(),
                    id: BlobId(id.to_owned()),
                });
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// A reader for many blobs, through one `git cat-file --batch`
    /// process, for reading a whole tree.
    ///
    /// # Errors
    ///
    /// [`GitError::GitNotFound`] or another [`GitError`] if the process
    /// cannot be started.
    pub fn blob_reader(&self) -> Result<BlobReader, GitError> {
        self.spawn_blob_reader(&[])
    }

    /// [`Repository::blob_reader`] with extra environment variables, which
    /// tests use to make git write to stderr.
    fn spawn_blob_reader(&self, env: &[(&str, &str)]) -> Result<BlobReader, GitError> {
        let mut command = git_command(&self.dir, &BATCH_ARGS);
        command.envs(env.iter().copied());
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Drained on a thread while git runs (`GIT_TRACE*` settings can
            // make it write a lot), and kept for error messages.
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| spawn_error(&self.dir, &BATCH_ARGS, &e))?;
        let stdin = child.stdin.take();
        let stderr = match child.stderr.take().map(drain_stderr).transpose() {
            Ok(stderr) => stderr,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(spawn_error(&self.dir, &BATCH_ARGS, &e));
            }
        };
        let stdout = child.stdout.take().map(BufReader::new);
        if stdout.is_some() {
            Ok(BlobReader {
                child,
                stdin,
                stdout,
                stderr,
            })
        } else {
            let _ = child.kill();
            let _ = child.wait();
            Err(GitError::UnexpectedOutput {
                args: BATCH_ARGS.iter().map(|&a| a.to_owned()).collect(),
                detail: "no stdout".to_owned(),
            })
        }
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

    /// The full commit id `revision` names. Every later git command only
    /// sees the returned id (ADR 0002).
    ///
    /// A revision that starts with `-` is rejected before git sees it: no
    /// ref name or revision syntax starts with `-`, so it could only be read
    /// as an option. This works on every git version, unlike
    /// `--end-of-options`, which `rev-parse` only accepts from git 2.30.
    fn resolve(&self, revision: &str) -> Result<String, GitError> {
        if revision.is_empty() || revision.starts_with('-') {
            return Err(GitError::UnknownRevision {
                revision: revision.to_owned(),
            });
        }
        let spec = format!("{revision}^{{commit}}");
        let args = ["rev-parse", "--verify", "--quiet", &spec];
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

/// What a diff says about regular files: see [`Repository::changes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    /// Added, modified and renamed regular files.
    pub changed: Vec<ChangedFile>,
    /// Regular files that are gone, with their base contents.
    pub deleted: Vec<TreeFile>,
}

/// Parse `git diff --raw -z` output into the changed and the deleted
/// regular files.
///
/// Each record is `:<old-mode> <new-mode> <old-id> <new-id> <status>`,
/// NUL, path, NUL; renames and copies (`R<score>`, `C<score>`) carry the
/// old path and then the new path.
fn parse_raw_diff(raw: &[u8]) -> Result<Changes, String> {
    let mut fields = raw.split(|&b| b == 0);
    let mut files = Vec::new();
    let mut deleted = Vec::new();
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
            // Added, or a symlink or submodule replaced by a regular file:
            // there is no earlier version of this file's code.
            Some('A' | 'T') if regular(new_mode) => None,
            Some('M' | 'R' | 'C') if regular(old_mode) && regular(new_mode) => {
                Some(BlobId(old_id.to_owned()))
            }
            // A regular file that is gone, or no longer a regular file.
            Some('D' | 'T') if regular(old_mode) => {
                deleted.push(TreeFile {
                    path,
                    id: BlobId(old_id.to_owned()),
                });
                continue;
            }
            // Symlinks, submodules, a file that became one, unmerged.
            _ => continue,
        };
        files.push(ChangedFile {
            path,
            old_path,
            before,
            after: BlobId(new_id.to_owned()),
        });
    }
    Ok(Changes {
        changed: files,
        deleted,
    })
}

/// Run `git` with `args` in `dir`. Prompts are disabled, so a command can
/// never wait for input.
fn run<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Result<Output, GitError> {
    git_command(dir, args)
        .output()
        .map_err(|e| spawn_error(dir, args, &e))
}

/// A `git` command with `args` in `dir`, with prompts disabled, so it can
/// never wait for input.
fn git_command<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Command {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0");
    // `file_at` passes `--literal-pathspecs`, which git refuses next to
    // any other global pathspec setting a user may have exported.
    for variable in [
        "GIT_LITERAL_PATHSPECS",
        "GIT_GLOB_PATHSPECS",
        "GIT_NOGLOB_PATHSPECS",
        "GIT_ICASE_PATHSPECS",
    ] {
        command.env_remove(variable);
    }
    // Tests run the code under test isolated from the developer's git
    // configuration and repository variables, like the fixtures (design D5).
    #[cfg(test)]
    test_support::isolate(&mut command);
    command
}

/// The error for a `git` command that could not be started.
fn spawn_error<S: AsRef<OsStr>>(dir: &Path, args: &[S], e: &std::io::Error) -> GitError {
    match e.kind() {
        // Spawning also fails with NotFound when the directory is gone.
        ErrorKind::NotFound if !dir.is_dir() => GitError::NotARepository {
            detail: format!("{} is not a directory", dir.display()),
        },
        ErrorKind::NotFound => GitError::GitNotFound,
        _ => GitError::Command {
            args: args
                .iter()
                .map(|a| a.as_ref().to_string_lossy().into_owned())
                .collect(),
            status: None,
            stderr: e.to_string(),
        },
    }
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

    /// Names of the entries in a directory, sorted.
    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn option_like_revisions_are_unknown_and_write_nothing() {
        let repo = TestRepo::new();
        repo.commit("first");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let before = entries(&repo.dir);
        for revision in ["--output=written-by-git", "--default=HEAD", "-", "-HEAD"] {
            let unknown = Err(GitError::UnknownRevision {
                revision: revision.to_owned(),
            });
            assert_eq!(found.changed_files(revision, "HEAD"), unknown, "as base");
            assert_eq!(found.changed_files("HEAD", revision), unknown, "as head");
        }
        assert_eq!(entries(&repo.dir), before, "git wrote into the work tree");
    }

    #[test]
    fn missing_directory_is_not_a_repository() {
        let dir = scratch_dir("missing");
        std::fs::remove_dir_all(&dir).expect("remove");
        let result = Repository::discover(&dir);
        assert!(
            matches!(result, Err(GitError::NotARepository { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn changed_files_from_a_subdirectory_are_root_relative_and_complete() {
        let repo = TestRepo::new();
        repo.write("src/a.ts", "v1");
        repo.write("top.ts", "v1");
        repo.commit("base");
        repo.write("src/a.ts", "v2");
        repo.write("top.ts", "v2");
        repo.commit("head");
        // With diff.relative, a plain `git diff` from src/ would list only
        // `a.ts`; the repository-level setting reaches the code under test.
        repo.git(&["config", "diff.relative", "true"]);
        let paths: Vec<String> = Repository::discover(&repo.dir.join("src"))
            .expect("a repository")
            .changed_files("HEAD~1", "HEAD")
            .expect("changed files")
            .into_iter()
            .map(|c| c.path)
            .collect();
        assert_eq!(paths, ["src/a.ts", "top.ts"]);
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

    #[test]
    fn regular_file_replacing_a_submodule_is_added() {
        let repo = TestRepo::new();
        let first = repo.commit("base");
        repo.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{first},vendor.ts"),
        ]);
        repo.git(&["commit", "-q", "-m", "submodule"]);
        repo.git(&["rm", "-q", "--cached", "vendor.ts"]);
        repo.write("vendor.ts", "now a regular file");
        repo.commit("file replaces submodule");
        assert_eq!(
            changes(&repo, "HEAD~1", "HEAD"),
            vec![ChangedFile {
                path: "vendor.ts".to_owned(),
                old_path: None,
                before: None,
                after: blob(&repo, "HEAD", "vendor.ts"),
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_skipped_and_files_replacing_them_are_added() {
        let repo = TestRepo::new();
        repo.write("target.ts", "x");
        repo.write("became-link.ts", "a regular file first");
        std::os::unix::fs::symlink("target.ts", repo.dir.join("became-file.ts")).expect("symlink");
        repo.commit("base");
        std::os::unix::fs::symlink("target.ts", repo.dir.join("link.ts")).expect("symlink");
        std::fs::remove_file(repo.dir.join("became-link.ts")).expect("delete");
        std::os::unix::fs::symlink("target.ts", repo.dir.join("became-link.ts")).expect("symlink");
        std::fs::remove_file(repo.dir.join("became-file.ts")).expect("delete");
        repo.write("became-file.ts", "now a regular file");
        repo.commit("links");
        // Only the symlink that became a regular file is reported, as added:
        // its old version was not code.
        assert_eq!(
            changes(&repo, "HEAD~1", "HEAD"),
            vec![ChangedFile {
                path: "became-file.ts".to_owned(),
                old_path: None,
                before: None,
                after: blob(&repo, "HEAD", "became-file.ts"),
            }]
        );
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
    fn file_at_reads_regular_files_of_the_named_revision() {
        let repo = TestRepo::new();
        repo.write("qgate.toml", "old\n");
        repo.commit("base");
        repo.write("qgate.toml", b"new \xff\n");
        repo.write("run.sh", "#!/bin/sh\n");
        repo.git(&["add", "--chmod=+x", "run.sh"]);
        repo.commit("head");
        repo.write("qgate.toml", "uncommitted\n");
        let found = Repository::discover(&repo.dir).expect("a repository");
        assert_eq!(
            found.file_at("HEAD~1", "qgate.toml").expect("read"),
            Some(b"old\n".to_vec())
        );
        assert_eq!(
            found.file_at("HEAD", "qgate.toml").expect("read"),
            Some(b"new \xff\n".to_vec()),
            "the commit, not the working tree"
        );
        assert_eq!(
            found.file_at("HEAD", "run.sh").expect("read"),
            Some(b"#!/bin/sh\n".to_vec()),
            "an executable file is a regular file"
        );
    }

    #[test]
    fn file_at_missing_file_is_none_and_paths_are_literal() {
        let repo = TestRepo::new();
        repo.write("qgate.toml", "x");
        repo.write("sub/qgate.toml", "y");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        for path in [
            "absent.toml",
            "*.toml",
            "qgate.tom?",
            ":(glob)*.toml",
            "qgate",
        ] {
            assert_eq!(found.file_at("HEAD", path), Ok(None), "{path}");
        }
        assert_eq!(
            found.file_at("HEAD", "sub/qgate.toml"),
            Ok(Some(b"y".to_vec()))
        );
    }

    #[test]
    #[cfg(unix)]
    fn file_at_rejects_directories_and_symlinks() {
        let repo = TestRepo::new();
        repo.write("qgate.toml/inner", "x");
        repo.write("target.toml", "x");
        std::os::unix::fs::symlink("target.toml", repo.dir.join("link.toml")).expect("symlink");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        for path in ["qgate.toml", "link.toml"] {
            assert_eq!(
                found.file_at("HEAD", path),
                Err(GitError::NotARegularFile {
                    path: path.to_owned(),
                    revision: "HEAD".to_owned(),
                })
            );
        }
    }

    #[test]
    fn file_at_option_like_revision_is_unknown_and_writes_nothing() {
        let repo = TestRepo::new();
        repo.write("qgate.toml", "x");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let before = entries(&repo.dir);
        for revision in ["--output=written-by-git", "-", "no-such-branch"] {
            assert_eq!(
                found.file_at(revision, "qgate.toml"),
                Err(GitError::UnknownRevision {
                    revision: revision.to_owned(),
                })
            );
        }
        assert_eq!(entries(&repo.dir), before, "git wrote into the work tree");
    }

    #[test]
    #[cfg(unix)]
    fn files_at_lists_regular_files_by_path() {
        use std::os::unix::fs::PermissionsExt;

        let repo = TestRepo::new();
        repo.write("b.ts", "b");
        repo.write("a/z.ts", "z");
        repo.write("a/b/c.ts", "c");
        repo.write("a.b", "dot sorts before slash");
        repo.write("run.sh", "#!/bin/sh\n");
        std::fs::set_permissions(
            repo.dir.join("run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("chmod");
        std::os::unix::fs::symlink("b.ts", repo.dir.join("link.ts")).expect("symlink");
        let first = repo.commit("files");
        repo.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{first},vendor/lib"),
        ]);
        repo.git(&["commit", "-q", "-m", "submodule"]);
        repo.write("b.ts", "uncommitted");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let files = found.files_at("HEAD").expect("files");
        let expected: Vec<TreeFile> = ["a.b", "a/b/c.ts", "a/z.ts", "b.ts", "run.sh"]
            .iter()
            .map(|&path| TreeFile {
                path: path.to_owned(),
                id: blob(&repo, "HEAD", path),
            })
            .collect();
        // Symlinks and submodules are left out; the commit, not the
        // working tree, is listed.
        assert_eq!(files, expected);
    }

    #[test]
    fn files_at_option_like_revision_is_unknown_and_writes_nothing() {
        let repo = TestRepo::new();
        repo.write("a.ts", "x");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let before = entries(&repo.dir);
        for revision in ["--output=written-by-git", "-", "no-such-branch"] {
            assert_eq!(
                found.files_at(revision),
                Err(GitError::UnknownRevision {
                    revision: revision.to_owned(),
                })
            );
        }
        assert_eq!(entries(&repo.dir), before, "git wrote into the work tree");
    }

    #[test]
    fn blob_reader_reads_many_blobs_exactly() {
        let repo = TestRepo::new();
        let contents: [&[u8]; 4] = [
            b"first\n",
            b"",
            b"not \xff\xfe UTF-8\r\n",
            b"no trailing newline",
        ];
        for (i, bytes) in contents.iter().enumerate() {
            repo.write(&format!("f{i}.ts"), bytes);
        }
        repo.commit("blobs");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let mut reader = found.blob_reader().expect("reader");
        // Twice through, in a different order, with one reader.
        for i in [0, 1, 2, 3, 2, 0, 3, 1] {
            let id = blob(&repo, "HEAD", &format!("f{i}.ts"));
            assert_eq!(reader.read(&id).expect("read"), contents[i], "f{i}.ts");
        }
    }

    #[test]
    fn blob_reader_missing_id_is_an_error_and_the_reader_goes_on() {
        let repo = TestRepo::new();
        repo.write("a.ts", "a");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let mut reader = found.blob_reader().expect("reader");
        let missing = BlobId("0".repeat(40));
        assert!(matches!(
            reader.read(&missing),
            Err(GitError::UnexpectedOutput { .. })
        ));
        // A missing object keeps the reader in step.
        assert!(!reader.is_closed());
        // A tree is not a blob either.
        let tree = BlobId(repo.git(&["rev-parse", "HEAD^{tree}"]));
        assert!(matches!(
            reader.read(&tree),
            Err(GitError::UnexpectedOutput { .. })
        ));
        assert_eq!(
            reader.read(&blob(&repo, "HEAD", "a.ts")).expect("read"),
            b"a"
        );
    }

    #[test]
    fn blob_reader_reads_large_blobs_with_nul_and_newlines() {
        let repo = TestRepo::new();
        let large: Vec<u8> = (0..1_500_000u32)
            .map(|i| match i % 7 {
                0 => 0,
                1 => b'\n',
                _ => b'a' + u8::try_from(i % 26).expect("small"),
            })
            .collect();
        repo.write("large.bin", &large);
        repo.write("small.ts", "small");
        let first = repo.commit("blobs");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let mut reader = found.blob_reader().expect("reader");
        let large_id = blob(&repo, "HEAD", "large.bin");
        let small_id = blob(&repo, "HEAD", "small.ts");
        for _ in 0..2 {
            assert_eq!(reader.read(&large_id).expect("large"), large);
            assert_eq!(reader.read(&small_id).expect("small"), b"small");
        }
        // A commit is not a blob, and the reader stays in step.
        assert!(matches!(
            reader.read(&BlobId(first)),
            Err(GitError::UnexpectedOutput { .. })
        ));
        assert_eq!(reader.read(&small_id).expect("small"), b"small");
    }

    #[test]
    fn blob_reader_reports_how_git_ended() {
        let repo = TestRepo::new();
        repo.write("a.ts", "a");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let id = blob(&repo, "HEAD", "a.ts");
        std::fs::remove_dir_all(repo.dir.join(".git")).expect("remove .git");
        let mut reader = found.blob_reader().expect("the process starts");
        match reader.read(&id) {
            Err(GitError::Command { status, stderr, .. }) => {
                assert_eq!(status, Some(128));
                assert!(stderr.contains("not a git repository"), "{stderr}");
            }
            other => panic!("expected a command error, got {other:?}"),
        }
        // Closed for good.
        assert!(matches!(
            reader.read(&id),
            Err(GitError::UnexpectedOutput { .. })
        ));
    }

    #[test]
    fn an_answer_out_of_step_closes_the_reader() {
        let repo = TestRepo::new();
        repo.write("a.ts", "a");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let mut reader = found.blob_reader().expect("reader");
        // git answers `not an id missing`, which does not start with the
        // id that was asked for.
        assert!(matches!(
            reader.read(&BlobId("not an id".to_owned())),
            Err(GitError::UnexpectedOutput { .. })
        ));
        assert!(reader.is_closed());
        match reader.read(&blob(&repo, "HEAD", "a.ts")) {
            Err(GitError::UnexpectedOutput { detail, .. }) => {
                assert!(detail.contains("closed"), "{detail}");
            }
            other => panic!("expected the reader to be closed, got {other:?}"),
        }
    }

    #[test]
    fn a_blob_above_the_limit_is_skipped_and_the_reader_goes_on() {
        let repo = TestRepo::new();
        repo.write("huge.bin", vec![b'x'; MAX_BLOB_SIZE + 1]);
        repo.write("limit.bin", vec![b'y'; MAX_BLOB_SIZE]);
        repo.write("a.ts", "a");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let mut reader = found.blob_reader().expect("reader");
        // Exactly at the limit is still read.
        assert_eq!(
            reader
                .read(&blob(&repo, "HEAD", "limit.bin"))
                .expect("read")
                .len(),
            MAX_BLOB_SIZE
        );
        match reader.read(&blob(&repo, "HEAD", "huge.bin")) {
            Err(GitError::UnexpectedOutput { detail, .. }) => {
                assert!(detail.contains("larger than"), "{detail}");
            }
            other => panic!("expected the blob to be refused, got {other:?}"),
        }
        assert_eq!(
            reader.read(&blob(&repo, "HEAD", "a.ts")).expect("read"),
            b"a"
        );
    }

    #[test]
    fn a_lot_of_stderr_does_not_block_the_reader() {
        let repo = TestRepo::new();
        for i in 0..1500 {
            repo.write(&format!("f{i}.ts"), format!("{i}"));
        }
        repo.commit("many blobs");
        // Pack-access tracing writes a line to stderr for every object
        // read from a pack: about 170 KB here, far more than a pipe holds.
        repo.git(&["repack", "-adq"]);
        let found = Repository::discover(&repo.dir).expect("a repository");
        let ids: Vec<BlobId> = (0..1500)
            .map(|i| blob(&repo, "HEAD", &format!("f{i}.ts")))
            .collect();
        let mut reader = found
            .spawn_blob_reader(&[("GIT_TRACE_PACK_ACCESS", "2")])
            .expect("reader");
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (i, id) in ids.iter().enumerate() {
                assert_eq!(reader.read(id).expect("read"), format!("{i}").into_bytes());
            }
            let _ = done.send(());
        });
        assert!(
            finished
                .recv_timeout(std::time::Duration::from_secs(30))
                .is_ok(),
            "reading hung or failed"
        );
    }

    #[test]
    fn blob_reader_killed_process() {
        let repo = TestRepo::new();
        repo.write("a.ts", "a");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let id = blob(&repo, "HEAD", "a.ts");
        let mut reader = found.blob_reader().expect("reader");
        reader.kill();
        // Ended by a signal: no exit status.
        assert!(matches!(
            reader.read(&id),
            Err(GitError::Command { status: None, .. })
        ));
    }

    #[test]
    fn dropping_a_reader_with_an_unread_answer_does_not_hang() {
        let repo = TestRepo::new();
        repo.write("large.bin", vec![b'x'; 2_000_000]);
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let id = blob(&repo, "HEAD", "large.bin");
        let mut reader = found.blob_reader().expect("reader");
        reader.send_only(&id);
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            drop(reader);
            let _ = done.send(());
        });
        assert!(
            finished
                .recv_timeout(std::time::Duration::from_secs(20))
                .is_ok(),
            "dropping the reader hung"
        );
    }

    #[test]
    fn blob_reader_ends_its_process_when_dropped() {
        let repo = TestRepo::new();
        repo.write("a.ts", "a");
        repo.commit("base");
        let found = Repository::discover(&repo.dir).expect("a repository");
        let reader = found.blob_reader().expect("reader");
        let pid = reader.process_id();
        drop(reader);
        // `/proc/<pid>` is gone once the child has been waited for; a
        // zombie would keep it. Linux only on purpose: CI runs on Linux.
        #[cfg(target_os = "linux")]
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        let _ = pid;
    }

    #[test]
    #[cfg(unix)]
    fn deleted_files_have_their_base_contents() {
        let repo = TestRepo::new();
        repo.write("a.ts", "a");
        repo.write("keep.ts", "k");
        repo.write("to-link.ts", "l");
        repo.commit("base");
        let a = blob(&repo, "HEAD", "a.ts");
        let to_link = blob(&repo, "HEAD", "to-link.ts");
        std::fs::remove_file(repo.dir.join("a.ts")).expect("delete");
        std::fs::remove_file(repo.dir.join("to-link.ts")).expect("delete");
        std::os::unix::fs::symlink("keep.ts", repo.dir.join("to-link.ts")).expect("symlink");
        repo.write("keep.ts", "changed");
        repo.commit("head");
        let found = Repository::discover(&repo.dir).expect("a repository");
        assert_eq!(
            found.changes("HEAD~1", "HEAD").expect("changes").deleted,
            vec![
                TreeFile {
                    path: "a.ts".to_owned(),
                    id: a,
                },
                TreeFile {
                    path: "to-link.ts".to_owned(),
                    id: to_link,
                },
            ]
        );
        // The changed files stay as they were.
        assert_eq!(changes(&repo, "HEAD~1", "HEAD").len(), 1);
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
