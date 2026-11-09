//! End-to-end tests for `qgate check --base <rev> [--head <rev>]`, one per
//! scenario of the `revision-input` spec and the revision-mode scenarios of
//! the `check-command` spec. Each test builds a throwaway git repository and
//! runs the built binary in it (design D5).

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
fn isolate(command: &mut Command) -> &mut Command {
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
fn scratch_dir(kind: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("qgate-cli-{kind}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// A git repository in a fresh temporary directory, removed on drop.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new() -> Self {
        let repo = Self {
            dir: scratch_dir("repo"),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
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

    fn write(&self, path: &str, contents: impl AsRef<[u8]>) {
        let full = self.dir.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("create dirs");
        std::fs::write(full, contents).expect("write file");
    }

    fn remove(&self, path: &str) {
        std::fs::remove_file(self.dir.join(path)).expect("remove file");
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
    }

    /// Run qgate in the repository root.
    fn qgate(&self, args: &[&str]) -> Output {
        qgate_in(&self.dir, args)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn qgate_in(dir: &Path, args: &[&str]) -> Output {
    isolate(&mut Command::new(env!("CARGO_BIN_EXE_qgate")))
        .arg("check")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("qgate runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

/// A function named `name` with `decisions` decision points, so its
/// cyclomatic complexity is `decisions + 1`.
fn function(name: &str, decisions: usize) -> String {
    let mut text = format!("export function {name}(c: boolean[]): number {{\n");
    for i in 0..decisions {
        let _ = writeln!(text, "  if (c[{i}]) {{ return {i}; }}");
    }
    text.push_str("  return -1;\n}\n");
    text
}

/// `lines` comment lines, so the next line is `lines + 1`.
fn padding(lines: usize) -> String {
    (0..lines).fold(String::new(), |mut text, i| {
        let _ = writeln!(text, "// padding {i}");
        text
    })
}

/// A repository whose `feature` branch makes `f` in `src/a.ts` go from 2
/// to 4; `main` stays checked out.
fn feature_repo() -> Repo {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/a.ts", function("f", 3));
    repo.commit("feature makes f more complex");
    repo.git(&["checkout", "-q", "main"]);
    repo
}

const F_INCREASE: &str =
    "warning src/a.ts:1  f: cyclomatic complexity 2 -> 4 (+2)  [complexity/cyclomatic-increase]\n";

// check-command: Compare two git revisions

#[test]
fn pull_request_branch() {
    // Also revision-input "Head is a descendant of base".
    let repo = feature_repo();
    let out = repo.qgate(&["--base", "main", "--head", "feature"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn head_defaults_to_head() {
    let repo = feature_repo();
    repo.git(&["checkout", "-q", "feature"]);
    let implicit = repo.qgate(&["--base", "main"]);
    let explicit = repo.qgate(&["--base", "main", "--head", "HEAD"]);
    assert_eq!(stdout(&implicit), F_INCREASE, "{}", stderr(&implicit));
    assert_eq!(implicit.stdout, explicit.stdout);
}

// check-command: Finding line format

#[test]
fn path_in_revision_mode_from_a_subdirectory() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.write("top.ts", function("t", 1));
    repo.commit("base");
    repo.write("src/a.ts", function("f", 3));
    repo.write("top.ts", function("t", 2));
    repo.commit("head");
    let out = qgate_in(&repo.dir.join("src"), &["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        format!(
            "{F_INCREASE}warning top.ts:1  t: cyclomatic complexity 2 -> 3 (+1)  \
             [complexity/cyclomatic-increase]\n"
        ),
        "{}",
        stderr(&out)
    );
}

// check-command: Deterministic output order

#[test]
fn several_files_sort_by_path_first() {
    let repo = Repo::new();
    repo.write("src/a.ts", padding(39) + &function("late", 1));
    repo.write("src/b.ts", padding(4) + &function("early", 1));
    repo.commit("base");
    repo.write("src/a.ts", padding(39) + &function("late", 2));
    repo.write("src/b.ts", padding(4) + &function("early", 2));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    let locations: Vec<String> = stdout(&out)
        .lines()
        .map(|l| l.split("  ").next().unwrap_or_default())
        .map(|l| l.trim_start_matches("warning ").to_owned())
        .collect();
    assert_eq!(locations, ["src/a.ts:40", "src/b.ts:5"], "{}", stderr(&out));
}

#[test]
fn paths_sort_as_byte_strings_like_git() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("a", 1));
    repo.write("src-x.ts", function("x", 1));
    repo.commit("base");
    repo.write("src/a.ts", function("a", 2));
    repo.write("src-x.ts", function("x", 2));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    let paths: Vec<String> = stdout(&out)
        .lines()
        .map(|l| {
            l.split(':')
                .next()
                .unwrap_or_default()
                .trim_start_matches("warning ")
                .to_owned()
        })
        .collect();
    // `-` (0x2D) sorts before `/` (0x2F), as in `git diff` and `sort`.
    assert_eq!(paths, ["src-x.ts", "src/a.ts"], "{}", stderr(&out));
}

// revision-input: Merge-base comparison

#[test]
fn base_branch_moved_on() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 4));
    repo.write("src/c.ts", function("g", 0));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/c.ts", function("g", 0) + "// touched\n");
    repo.commit("feature leaves a.ts alone");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("src/a.ts", function("f", 1));
    repo.commit("main simplifies f");
    // A direct diff of main against feature would report f going 2 -> 5.
    let out = repo.qgate(&["--base", "main", "--head", "feature"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

// revision-input: Only changed, supported files are analyzed

#[test]
fn unchanged_and_unsupported_files_are_not_analyzed() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.write("src/b.ts", function("huge", 30));
    repo.write("README.md", "v1");
    repo.commit("base");
    repo.write("src/a.ts", function("f", 3));
    repo.write("README.md", "v2");
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
}

#[test]
fn no_changes() {
    let repo = feature_repo();
    let out = repo.qgate(&["--base", "HEAD", "--head", "HEAD"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

// revision-input: Added files

#[test]
fn new_file_with_a_complex_function() {
    let repo = Repo::new();
    repo.commit("base");
    repo.write("src/new.ts", function("big", 11) + &function("small", 2));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/new.ts:1  big: new function, cyclomatic complexity 12  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
}

#[cfg(unix)]
#[test]
fn symbolic_link_replaced_by_a_file() {
    let repo = Repo::new();
    repo.write("src/target.ts", "export const x = 1;\n");
    std::os::unix::fs::symlink("target.ts", repo.dir.join("src/link.ts")).expect("symlink");
    repo.commit("base");
    repo.remove("src/link.ts");
    repo.write("src/link.ts", function("big", 11));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/link.ts:1  big: new function, cyclomatic complexity 12  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
}

// revision-input: Renamed files

#[test]
fn renamed_and_modified() {
    let repo = Repo::new();
    repo.write("src/old.ts", function("f", 1) + &padding(20));
    repo.commit("base");
    repo.git(&["mv", "src/old.ts", "src/new.ts"]);
    repo.write("src/new.ts", function("f", 2) + &padding(20));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/new.ts:1  f: cyclomatic complexity 2 -> 3 (+1)  \
         [complexity/cyclomatic-increase]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn renamed_from_an_unsupported_extension_is_added() {
    let repo = Repo::new();
    let source = function("big", 11);
    repo.write("src/util.js", &source);
    repo.commit("base");
    repo.git(&["mv", "src/util.js", "src/util.ts"]);
    repo.commit("head");
    // Compared with the old .js version, nothing would have changed.
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/util.ts:1  big: new function, cyclomatic complexity 12  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
}

// revision-input: Skipped changes

#[test]
fn deleted_file() {
    let repo = Repo::new();
    repo.write("src/legacy.ts", function("huge", 30));
    repo.write("src/keep.ts", function("k", 0));
    repo.commit("base");
    repo.remove("src/legacy.ts");
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

// revision-input: Unparsable files are reported, not fatal

#[test]
fn syntax_error_in_the_head_version() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 0));
    repo.write("src/b.ts", function("g", 1));
    repo.commit("base");
    repo.write(
        "src/a.ts",
        padding(11) + "function broken(a: number {\n  return a;\n}\n",
    );
    repo.write("src/b.ts", function("g", 2));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/a.ts:12  file not analyzed: syntax error on line 12 (or syntax the \
         parser does not support yet)  [input/unparsable]\n\
         warning src/b.ts:1  g: cyclomatic complexity 2 -> 3 (+1)  \
         [complexity/cyclomatic-increase]\n",
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn syntax_error_only_in_the_base_version() {
    let repo = Repo::new();
    repo.write(
        "src/a.ts",
        padding(2) + "function broken(a: number {\n  return a;\n}\n",
    );
    repo.commit("base");
    repo.write("src/a.ts", function("f", 0));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/a.ts  file not analyzed: syntax error on line 3 of the base version \
         (or syntax the parser does not support yet)  [input/unparsable]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn base_version_not_valid_utf8() {
    let repo = Repo::new();
    repo.write("src/a.ts", b"export const s = \"\xff\xfe\";\n");
    repo.commit("base");
    repo.write("src/a.ts", function("f", 0));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/a.ts  file not analyzed: the base version is not valid UTF-8  \
         [input/unparsable]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn broken_on_both_sides_reports_the_head() {
    let repo = Repo::new();
    repo.write("src/a.ts", padding(2) + "function broken(a: number {\n");
    repo.commit("base");
    repo.write("src/a.ts", padding(6) + "function broken(a: number {\n");
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/a.ts:7  file not analyzed: syntax error on line 7 (or syntax the \
         parser does not support yet)  [input/unparsable]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn not_valid_utf8() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 0));
    repo.commit("base");
    repo.write("src/a.ts", b"export const s = \"\xff\xfe\";\n");
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "warning src/a.ts  file not analyzed: not valid UTF-8  [input/unparsable]\n",
        "{}",
        stderr(&out)
    );
}

// check-command: Failing on findings

#[test]
fn unparsable_file_in_revision_mode() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 0));
    repo.commit("base");
    repo.write("src/a.ts", "function broken(a: number {\n");
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert!(
        stdout(&out).contains("[input/unparsable]"),
        "{}",
        stderr(&out)
    );
    assert_eq!(stderr(&out), "");
    assert_eq!(out.status.code(), Some(0));

    let out = repo.qgate(&["--base", "HEAD~1", "--fail-on", "warning"]);
    assert!(
        stdout(&out).contains("[input/unparsable]"),
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn errors_that_stop_the_check_win() {
    let repo = feature_repo();
    let out = repo.qgate(&["--base", "no-such-branch", "--fail-on", "warning"]);
    assert_eq!(stdout(&out), "");
    assert!(!stderr(&out).contains("failing"), "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn error_finding_in_revision_mode() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 0));
    repo.commit("base");
    repo.write("src/a.ts", function("f", 6));
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert!(stdout(&out).starts_with("error "), "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(1));
    let out = repo.qgate(&["--base", "HEAD~1", "--fail-on", "never"]);
    assert_eq!(out.status.code(), Some(0));
}

// revision-input: Git errors

#[test]
fn unknown_base_revision() {
    let repo = feature_repo();
    let out = repo.qgate(&["--base", "no-such-branch"]);
    let err = stderr(&out);
    assert!(err.contains("unknown revision `no-such-branch`"), "{err}");
    assert!(err.contains("fetch-depth: 0"), "{err}");
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_head_or_option_like_base_gets_no_shallow_clone_hint() {
    let repo = feature_repo();
    for (args, revision) in [
        (
            ["--base", "main", "--head", "no-such"].as_slice(),
            "no-such",
        ),
        (&["--base=-x"], "-x"),
    ] {
        let out = repo.qgate(args);
        let err = stderr(&out);
        assert!(
            err.contains(&format!("unknown revision `{revision}`")),
            "{err}"
        );
        assert!(!err.contains("fetch-depth"), "{args:?}: {err}");
        assert_eq!(out.status.code(), Some(2));
    }
}

#[test]
fn not_a_repository() {
    let dir = scratch_dir("plain");
    let out = qgate_in(&dir, &["--base", "main"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        stderr(&out).contains("not a git repository"),
        "{}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unrelated_histories() {
    let repo = feature_repo();
    repo.git(&["checkout", "-q", "--orphan", "other"]);
    repo.commit("unrelated");
    let out = repo.qgate(&["--base", "main"]);
    assert!(stderr(&out).contains("no merge base"), "{}", stderr(&out));
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn git_cannot_be_run() {
    let repo = feature_repo();
    let out = isolate(&mut Command::new(env!("CARGO_BIN_EXE_qgate")))
        .args(["check", "--base", "main"])
        .current_dir(&repo.dir)
        .env("PATH", "")
        .output()
        .expect("qgate runs");
    assert!(
        stderr(&out).contains("git is not installed"),
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(2));
}

// revision-input: Revisions are never options

#[test]
fn revision_that_looks_like_an_option() {
    let repo = feature_repo();
    let target = repo.dir.join("written-by-git");
    let revision = format!("--output={}", target.display());
    let out = repo.qgate(&[&format!("--base={revision}")]);
    assert!(
        stderr(&out).contains(&format!("unknown revision `{revision}`")),
        "{}",
        stderr(&out)
    );
    assert!(!target.exists(), "git treated the revision as an option");
    assert_eq!(out.status.code(), Some(2));
}

// configuration: qgate.toml from the head revision

/// A valid forbid rule.
const RULE: &str = "[[architecture.forbid]]\nname = \"domain-is-pure\"\n\
                    from = [\"src/domain/**\"]\nto = [\"src/ui/**\"]\n";

/// A repository whose last commit makes `f` in `src/a.ts` go from 2 to 4
/// and sets `qgate.toml` to `config`.
fn repo_with_config(config: &str) -> Repo {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.commit("base");
    repo.write("src/a.ts", function("f", 3));
    repo.write("qgate.toml", config);
    repo.commit("head");
    repo
}

/// `qgate check --base HEAD~1` fails with exit code 2, empty standard
/// output and an error that names `qgate.toml` and each of `expected`.
fn assert_invalid(repo: &Repo, expected: &[&str]) {
    let out = repo.qgate(&["--base", "HEAD~1"]);
    let err = stderr(&out);
    assert!(err.contains("qgate.toml"), "{err}");
    for part in expected {
        assert!(err.contains(part), "missing {part:?} in: {err}");
    }
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn repository_without_qgate_toml() {
    let repo = feature_repo();
    let out = repo.qgate(&["--base", "main", "--head", "feature"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn valid_configuration_is_accepted() {
    let repo = repo_with_config(RULE);
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn configuration_syntax_error() {
    let repo = repo_with_config(
        "[[architecture.forbid]]\nfrom = [\"a\"]\nname = \"domain-is-pure\nto = [\"b\"]\n",
    );
    assert_invalid(&repo, &["qgate.toml: line 3"]);
}

#[test]
fn misspelled_configuration_table() {
    let repo = repo_with_config("[[architecture.forbd]]\nname = \"x\"\n");
    assert_invalid(&repo, &["forbd"]);
}

#[test]
fn duplicate_rule_name() {
    let repo = repo_with_config(&format!("{RULE}{RULE}"));
    assert_invalid(&repo, &["duplicate", "domain-is-pure"]);
}

#[test]
fn empty_pattern_list() {
    let repo = repo_with_config(&RULE.replace("to = [\"src/ui/**\"]", "to = []"));
    assert_invalid(&repo, &["domain-is-pure", "`to` is empty"]);
}

#[test]
fn configuration_that_is_not_a_regular_file() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.commit("base");
    repo.write("qgate.toml/inner", RULE);
    repo.commit("head");
    assert_invalid(&repo, &["not a regular file"]);
}

#[test]
fn uncommitted_configuration_is_ignored() {
    let repo = feature_repo();
    repo.git(&["checkout", "-q", "feature"]);
    // Neither revision has qgate.toml; an invalid one in the working tree
    // would fail the run if it were read.
    repo.write("qgate.toml", "not toml at all [");
    let out = repo.qgate(&["--base", "main"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn configuration_comes_from_the_head_revision() {
    let repo = feature_repo();
    // `feature` gets an invalid qgate.toml; `main` stays checked out.
    repo.git(&["checkout", "-q", "feature"]);
    repo.write("qgate.toml", "[[architecture.forbd]]\n");
    repo.commit("break the configuration");
    repo.git(&["checkout", "-q", "main"]);
    let out = repo.qgate(&["--base", "main", "--head", "feature"]);
    assert!(stderr(&out).contains("forbd"), "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(2));
    // The base revision's configuration is not read.
    let fixed = repo_with_config(RULE);
    fixed.write("qgate.toml", "broken [");
    fixed.git(&["commit", "-q", "-am", "break it"]);
    fixed.write("qgate.toml", RULE);
    fixed.git(&["commit", "-q", "-am", "fix it"]);
    let out = fixed.qgate(&["--base", "HEAD~1"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn unknown_base_wins_over_an_invalid_configuration() {
    let repo = repo_with_config("broken [");
    let out = repo.qgate(&["--base", "no-such-branch"]);
    let err = stderr(&out);
    assert!(err.contains("unknown revision `no-such-branch`"), "{err}");
    assert!(err.contains("fetch-depth: 0"), "{err}");
    assert!(!err.contains("qgate.toml"), "{err}");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn global_pathspec_settings_do_not_break_revision_mode() {
    let repo = repo_with_config(RULE);
    for variable in ["GIT_GLOB_PATHSPECS", "GIT_ICASE_PATHSPECS"] {
        let out = isolate(&mut Command::new(env!("CARGO_BIN_EXE_qgate")))
            .args(["check", "--base", "HEAD~1"])
            .env(variable, "1")
            .current_dir(&repo.dir)
            .output()
            .expect("qgate runs");
        assert_eq!(out.status.code(), Some(0), "{variable}: {}", stderr(&out));
    }
}
