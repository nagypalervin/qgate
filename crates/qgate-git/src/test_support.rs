//! Throwaway git repositories for tests (design D5).
//!
//! Every git command, the helper's and the code under test's (through
//! [`isolate`]), runs isolated from the developer's configuration and
//! repository variables; commits use fixed identities and dates, so results
//! do not depend on the machine.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Variables that point git at another repository or inject configuration.
/// Git sets several of them for hooks, so `cargo test` run from a hook
/// would otherwise write into the outer repository.
const REPOSITORY_VARIABLES: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
];

/// Isolate a git command from the developer's environment: no repository
/// variables, an empty global config, no system config, and no repository
/// discovery above the temp directory.
pub(crate) fn isolate(command: &mut Command) {
    for variable in REPOSITORY_VARIABLES {
        command.env_remove(variable);
    }
    command
        .env("GIT_CONFIG_GLOBAL", empty_config())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CEILING_DIRECTORIES", canonical(&std::env::temp_dir()));
}

/// An empty file to use as global git config, shared by all tests.
fn empty_config() -> &'static Path {
    static CONFIG: OnceLock<PathBuf> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let path =
            std::env::temp_dir().join(format!("qgate-git-empty-{}.gitconfig", std::process::id()));
        std::fs::write(&path, "").expect("write empty git config");
        path
    })
}

/// A repository in a fresh temporary directory, removed on drop.
pub(crate) struct TestRepo {
    /// The work tree.
    pub(crate) dir: PathBuf,
}

impl TestRepo {
    /// `git init` on branch `main` in a new temporary directory.
    pub(crate) fn new() -> Self {
        let repo = Self {
            dir: scratch_dir("repo"),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    /// Run git in the work tree; panics on failure; returns stdout.
    pub(crate) fn git(&self, args: &[&str]) -> String {
        let mut command = Command::new("git");
        isolate(&mut command);
        let out = command
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    /// Write a file relative to the work tree, creating directories.
    pub(crate) fn write(&self, path: &str, contents: impl AsRef<[u8]>) {
        let full = self.dir.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create dirs");
        }
        std::fs::write(full, contents).expect("write file");
    }

    /// Stage everything and commit; returns the commit id.
    pub(crate) fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A new, empty directory under the system temp directory.
pub(crate) fn scratch_dir(kind: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("qgate-git-{kind}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Canonical form of a path, for comparing with what git reports.
pub(crate) fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("canonicalize")
}
