//! Helpers shared by the end-to-end tests: throwaway git repositories,
//! running the built binary in them, and source text with a known
//! complexity. Moved here from `revisions.rs` when the JSON tests needed
//! them too.

// Each test crate includes this module and uses only some of it.
#![allow(dead_code)]

pub mod schema;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Variables that would point git at another repository or inject config;
/// git sets several of them for hooks.
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
    // Would copy the developer's hooks into `git init`ed test repositories.
    "GIT_TEMPLATE_DIR",
];

/// Isolate a command (git, or qgate and the git it starts) from the
/// developer's environment.
pub fn isolate(command: &mut Command) -> &mut Command {
    for variable in REPOSITORY_VARIABLES {
        command.env_remove(variable);
    }
    let config = std::env::temp_dir().join("qgate-cli-empty.gitconfig");
    std::fs::write(&config, "").expect("write empty git config");
    let temp = std::fs::canonicalize(std::env::temp_dir()).expect("temp dir");
    command
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CEILING_DIRECTORIES", temp)
}

/// A new, empty directory under the system temp directory.
pub fn scratch_dir(kind: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("qgate-cli-{kind}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// A git repository in a fresh temporary directory, removed on drop.
pub struct Repo {
    /// The repository root.
    pub dir: PathBuf,
}

impl Repo {
    pub fn new() -> Self {
        let repo = Self {
            dir: scratch_dir("repo"),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    pub fn git(&self, args: &[&str]) -> String {
        let out = isolate(&mut Command::new("git"))
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

    pub fn write(&self, path: &str, contents: impl AsRef<[u8]>) {
        let full = self.dir.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("create dirs");
        std::fs::write(full, contents).expect("write file");
    }

    pub fn remove(&self, path: &str) {
        std::fs::remove_file(self.dir.join(path)).expect("remove file");
    }

    pub fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
    }

    /// Run qgate in the repository root.
    pub fn qgate(&self, args: &[&str]) -> Output {
        qgate_in(&self.dir, args)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub fn qgate_in(dir: &Path, args: &[&str]) -> Output {
    isolate(&mut Command::new(env!("CARGO_BIN_EXE_qgate")))
        .arg("check")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("qgate runs")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

/// A function named `name` with `decisions` decision points, so its
/// cyclomatic complexity is `decisions + 1`.
pub fn function(name: &str, decisions: usize) -> String {
    let mut text = format!("export function {name}(c: boolean[]): number {{\n");
    for i in 0..decisions {
        let _ = writeln!(text, "  if (c[{i}]) {{ return {i}; }}");
    }
    text.push_str("  return -1;\n}\n");
    text
}

/// `lines` comment lines, so the next line is `lines + 1`.
pub fn padding(lines: usize) -> String {
    (0..lines).fold(String::new(), |mut text, i| {
        let _ = writeln!(text, "// padding {i}");
        text
    })
}

/// A repository whose `feature` branch makes `f` in `src/a.ts` go from 2
/// to 4; `main` stays checked out.
pub fn feature_repo() -> Repo {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/a.ts", function("f", 3));
    repo.commit("feature makes f more complex");
    repo.git(&["checkout", "-q", "main"]);
    repo
}

/// A 68-token helper, lines 1-6.
pub const DATE_HELPER: &str = "export function formatDate(d: Date): string {
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, \"0\");
  const day = String(d.getDate()).padStart(2, \"0\");
  return `${year}-${month}-${day}`;
}
";

/// `DATE_HELPER` under other names, at lines 4-9.
pub const RENAMED_COPY: &str = "// Orders, with a copy of the date helper under other names.
import { Order } from \"./order\";

export function formatOrderDate(when: Date): string {
  const y = when.getFullYear();
  const m = String(when.getMonth() + 1).padStart(2, \"0\");
  const dd = String(when.getDate()).padStart(2, \"0\");
  return `${y}/${m}/${dd}`;
}
";
