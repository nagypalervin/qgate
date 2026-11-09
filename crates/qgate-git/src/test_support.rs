//! Throwaway git repositories for tests (design D5).
//!
//! Every git command the helper runs is isolated from the developer's own
//! configuration and uses fixed identities and dates, so results do not
//! depend on the machine.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A repository in a fresh temporary directory, removed on drop.
pub(crate) struct TestRepo {
    /// The work tree.
    pub(crate) dir: PathBuf,
    /// An empty file used as global git config.
    config: PathBuf,
}

impl TestRepo {
    /// `git init` on branch `main` in a new temporary directory.
    pub(crate) fn new() -> Self {
        let dir = scratch_dir("repo");
        let config = dir.with_extension("gitconfig");
        std::fs::write(&config, "").expect("write empty git config");
        let repo = Self { dir, config };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    /// Run git in the work tree; panics on failure; returns stdout.
    pub(crate) fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_CONFIG_GLOBAL", &self.config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
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
        let _ = std::fs::remove_file(&self.config);
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
