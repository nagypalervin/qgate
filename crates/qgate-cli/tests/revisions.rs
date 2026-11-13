//! End-to-end tests for `qgate check --base <rev> [--head <rev> |
//! --worktree]`, one per scenario of the `revision-input` spec and the
//! revision-mode scenarios of the `check-command` spec. Each test builds a
//! throwaway git repository and runs the built binary in it (design D5).

mod support;

use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

use support::{
    DATE_HELPER, RENAMED_COPY, Repo, feature_repo, function, isolate, padding, qgate_in,
    scratch_dir, stderr, stdout,
};

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
fn renamed_from_javascript_has_no_earlier_functions() {
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

// architecture-analysis, end to end

const ORDER: &str = "import { money } from \"./money\";\n\nexport const total = money(1);\n";
const ORDER_WITH_BUTTON: &str = "import { money } from \"./money\";\n\n\
                                 import { Button } from \"../ui/Button\";\n\
                                 export const total = money(1);\n";
const BUTTON_FINDING: &str = "error   src/domain/order.ts:3  imports src/ui/Button \
                              (\"../ui/Button\"), forbidden by \"domain-is-pure\"  \
                              [architecture/forbidden-import]\n";

#[test]
fn domain_importing_the_ui() {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    repo.write("src/domain/order.ts", ORDER);
    repo.commit("base");
    repo.write("src/domain/order.ts", ORDER_WITH_BUTTON);
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), BUTTON_FINDING, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn rules_come_from_the_head_revision() {
    let repo = Repo::new();
    repo.write("src/domain/order.ts", ORDER);
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("qgate.toml", RULE);
    repo.write("src/domain/order.ts", ORDER_WITH_BUTTON);
    repo.commit("rule and import");
    repo.git(&["checkout", "-q", "main"]);
    let out = repo.qgate(&["--base", "main", "--head", "feature"]);
    assert_eq!(stdout(&out), BUTTON_FINDING, "{}", stderr(&out));
}

#[test]
fn moved_into_the_domain() {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    repo.write("src/app/order.ts", ORDER_WITH_BUTTON);
    repo.commit("base");
    repo.remove("src/app/order.ts");
    repo.write("src/domain/order.ts", ORDER_WITH_BUTTON);
    repo.commit("move into the domain");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), BUTTON_FINDING, "{}", stderr(&out));
}

#[test]
fn moved_within_the_domain() {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    repo.write("src/domain/order.ts", ORDER_WITH_BUTTON);
    repo.commit("base");
    repo.remove("src/domain/order.ts");
    repo.write(
        "src/domain/sales/order.ts",
        ORDER_WITH_BUTTON.replace("../ui/Button", "../../ui/Button"),
    );
    repo.commit("move within the domain");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

#[test]
fn rule_added_in_the_same_change() {
    let repo = Repo::new();
    repo.write("src/domain/order.ts", "import { A } from \"../ui/A\";\n");
    repo.commit("base");
    repo.write("qgate.toml", RULE);
    repo.write(
        "src/domain/order.ts",
        "import { A } from \"../ui/A\";\nimport { B } from \"../ui/B\";\n",
    );
    repo.commit("rule and a new import");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "error   src/domain/order.ts:2  imports src/ui/B (\"../ui/B\"), forbidden by \
         \"domain-is-pure\"  [architecture/forbidden-import]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn invalid_pattern() {
    let repo = repo_with_config(&RULE.replace("src/domain/**", "src/[domain"));
    assert_invalid(&repo, &["domain-is-pure", "src/[domain"]);
}

#[test]
fn syntax_error_in_a_file_with_a_forbidden_import() {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    repo.write("src/domain/a.ts", "export const a = 1;\n");
    repo.commit("base");
    repo.write(
        "src/domain/a.ts",
        "import { Button } from \"../ui/Button\";\nfunction broken(a: number {\n",
    );
    repo.commit("head");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    let text = stdout(&out);
    assert!(text.contains("[input/unparsable]"), "{text}");
    assert!(!text.contains("architecture/"), "{text}");
}

#[test]
fn renamed_from_javascript_keeps_its_old_imports() {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    repo.write("src/domain/order.js", ORDER_WITH_BUTTON);
    repo.commit("base");
    repo.remove("src/domain/order.js");
    // A JS-to-TS migration that also adds one forbidden import.
    repo.write(
        "src/domain/order.ts",
        format!("{ORDER_WITH_BUTTON}import {{ Modal }} from \"../ui/Modal\";\n"),
    );
    repo.commit("migrate to TypeScript");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "error   src/domain/order.ts:5  imports src/ui/Modal (\"../ui/Modal\"), forbidden by \
         \"domain-is-pure\"  [architecture/forbidden-import]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn absolute_pattern_is_invalid() {
    let repo = repo_with_config(&RULE.replace("src/domain/**", "/src/domain/**"));
    assert_invalid(&repo, &["domain-is-pure", "/src/domain/**", "leading `/`"]);
}

/// `contents` with a Latin-1 comment, so it is not valid UTF-8.
fn latin1(contents: &str) -> Vec<u8> {
    let mut bytes = contents.as_bytes().to_vec();
    bytes.extend_from_slice(b"// caf\xe9\n");
    bytes
}

/// Rename the unreadable JavaScript `old` to `new` with `contents` (plus
/// a comment), under the forbid rule, and run `qgate check --base HEAD~1`.
fn rename_unreadable_javascript(old: &str, new: &str, contents: &str) -> Output {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    repo.write(old, latin1(contents));
    repo.commit("base");
    repo.remove(old);
    repo.write(new, format!("{contents}// cafe\n"));
    repo.commit("migrate to TypeScript");
    repo.qgate(&["--base", "HEAD~1"])
}

#[test]
fn renamed_from_unreadable_javascript_reports_no_import() {
    // Functions are new as for any added file; no import counts as new.
    let contents = format!("{ORDER_WITH_BUTTON}{}", function("big", 11));
    let out = rename_unreadable_javascript("src/domain/order.js", "src/domain/order.ts", &contents);
    assert_eq!(
        stdout(&out),
        "warning src/domain/order.ts:5  big: new function, cyclomatic complexity 12  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn unreadable_javascript_moved_to_another_directory_reports_no_import() {
    let out = rename_unreadable_javascript(
        "src/domain/order.js",
        "src/domain/sales/order.ts",
        &ORDER_WITH_BUTTON.replace("../ui/Button", "../../ui/Button"),
    );
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

#[test]
fn unreadable_javascript_moved_into_the_domain_reports_no_import() {
    let out =
        rename_unreadable_javascript("src/app/order.js", "src/domain/order.ts", ORDER_WITH_BUTTON);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

#[test]
fn renamed_from_unreadable_javascript_without_configuration_is_unchanged() {
    // As before qgate.toml existed: the functions are analyzed as new.
    let repo = Repo::new();
    repo.write("src/util.js", latin1(&function("big", 24)));
    repo.commit("base");
    repo.remove("src/util.js");
    repo.write("src/util.ts", function("big", 24) + "// cafe\n");
    repo.commit("migrate to TypeScript");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        "error   src/util.ts:1  big: new function, cyclomatic complexity 25  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn renamed_from_another_unsupported_extension_is_added() {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    let contents = format!("{ORDER_WITH_BUTTON}{}", function("big", 11));
    repo.write("src/domain/order.coffee", &contents);
    repo.commit("base");
    repo.git(&["mv", "src/domain/order.coffee", "src/domain/order.ts"]);
    repo.commit("rename");
    // No earlier version: the import and the function are new.
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(
        stdout(&out),
        format!(
            "{BUTTON_FINDING}warning src/domain/order.ts:5  big: new function, cyclomatic \
             complexity 12  [complexity/cyclomatic-new]\n"
        ),
        "{}",
        stderr(&out)
    );
}

// duplication-analysis, end to end

const COPY_FINDING: &str = "warning src/orders/format.ts:4  duplicates src/utils/date.ts:1-6 \
                            (lines 4-9, 68 tokens)  [duplication/duplicate-block]\n";

/// A repository whose last commit adds a renamed copy of an unchanged
/// helper.
fn repo_with_a_copy() -> Repo {
    let repo = Repo::new();
    repo.write("src/utils/date.ts", DATE_HELPER);
    repo.commit("base");
    repo.write("src/orders/format.ts", RENAMED_COPY);
    repo.commit("copy");
    repo
}

#[test]
fn copy_of_an_existing_file_the_change_does_not_touch() {
    let repo = repo_with_a_copy();
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), COPY_FINDING, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn failing_on_duplication() {
    let repo = repo_with_a_copy();
    let out = repo.qgate(&["--base", "HEAD~1", "--fail-on", "warning"]);
    assert_eq!(stdout(&out), COPY_FINDING, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn an_unparsable_unchanged_file_takes_no_part() {
    let repo = Repo::new();
    repo.write("src/utils/date.ts", DATE_HELPER);
    repo.write("src/broken.ts", "function broken(a: number {\n");
    repo.commit("base");
    repo.write("src/orders/format.ts", RENAMED_COPY);
    repo.commit("copy");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), COPY_FINDING, "{}", stderr(&out));
}

#[test]
fn renamed_from_javascript_is_not_a_new_copy() {
    for unreadable in [false, true] {
        let repo = Repo::new();
        repo.write("src/a.ts", DATE_HELPER);
        let mut js = DATE_HELPER.as_bytes().to_vec();
        if unreadable {
            js.extend_from_slice(b"// caf\xe9\n");
        }
        repo.write("src/util.js", js);
        repo.commit("base");
        repo.remove("src/util.js");
        repo.write("src/util.ts", format!("{DATE_HELPER}// cafe\n"));
        repo.commit("migrate to TypeScript");
        let out = repo.qgate(&["--base", "HEAD~1"]);
        assert_eq!(
            stdout(&out),
            "No regressions found.\n",
            "unreadable: {unreadable}: {}",
            stderr(&out)
        );
    }
}

#[test]
fn code_moved_out_of_a_deleted_file_is_not_a_new_copy() {
    // a.ts and c.ts share the helper; the change deletes a.ts and adds
    // b.ts with the helper and more code, so git sees no rename.
    let repo = Repo::new();
    repo.write("src/a.ts", DATE_HELPER);
    repo.write("src/c.ts", DATE_HELPER);
    repo.commit("base");
    repo.remove("src/a.ts");
    repo.write(
        "src/b.ts",
        format!("{}{DATE_HELPER}{}", padding(30), function("other", 1)),
    );
    repo.commit("move");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

#[test]
fn code_moved_out_of_a_file_renamed_to_another_language_is_not_a_new_copy() {
    let repo = Repo::new();
    repo.write("src/a.ts", DATE_HELPER);
    repo.write("src/c.ts", DATE_HELPER);
    repo.commit("base");
    repo.git(&["mv", "src/a.ts", "src/a.js"]);
    repo.write("src/b.ts", format!("{}{DATE_HELPER}", padding(30)));
    repo.commit("move");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

#[test]
fn code_moved_out_of_a_deleted_javascript_file_is_not_a_new_copy() {
    let repo = Repo::new();
    repo.write("src/a.js", DATE_HELPER);
    repo.write("src/c.ts", DATE_HELPER);
    repo.commit("base");
    repo.remove("src/a.js");
    repo.write("src/b.ts", format!("{}{DATE_HELPER}", padding(30)));
    repo.commit("move");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), "No regressions found.\n", "{}", stderr(&out));
}

#[test]
fn a_javascript_rename_does_not_hide_a_new_copy() {
    // u.ts and util.js hold the helper; the change renames util.js to
    // util2.js and adds a renamed copy in orders/format.ts.
    let repo = Repo::new();
    repo.write("src/utils/date.ts", DATE_HELPER);
    repo.write("src/util.js", DATE_HELPER);
    repo.commit("base");
    repo.git(&["mv", "src/util.js", "src/util2.js"]);
    repo.write("src/orders/format.ts", RENAMED_COPY);
    repo.commit("rename and copy");
    let out = repo.qgate(&["--base", "HEAD~1"]);
    assert_eq!(stdout(&out), COPY_FINDING, "{}", stderr(&out));
}

// revision-input: Working tree as head (add-worktree-input)

/// A repository with `f` in `src/a.ts` at complexity 2 committed on
/// `main`, which stays checked out.
fn worktree_repo() -> Repo {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.commit("base");
    repo
}

const NO_REGRESSIONS: &str = "No regressions found.\n";

#[test]
fn uncommitted_edit() {
    // Also check-command "Working tree".
    let repo = worktree_repo();
    repo.write("src/a.ts", function("f", 3));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn without_the_flag_uncommitted_edits_are_not_seen() {
    let repo = worktree_repo();
    repo.write("src/a.ts", function("f", 3));
    let out = repo.qgate(&["--base", "main"]);
    assert_eq!(stdout(&out), NO_REGRESSIONS, "{}", stderr(&out));
}

#[test]
fn committed_and_uncommitted_changes_together() {
    let repo = Repo::new();
    repo.write("qgate.toml", RULE);
    repo.write("src/domain/order.ts", ORDER);
    repo.write("src/b.ts", function("g", 1));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/domain/order.ts", ORDER_WITH_BUTTON);
    repo.commit("committed import");
    repo.write("src/b.ts", function("g", 3));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(
        stdout(&out),
        format!(
            "warning src/b.ts:1  g: cyclomatic complexity 2 -> 4 (+2)  \
             [complexity/cyclomatic-increase]\n{BUTTON_FINDING}"
        ),
        "{}",
        stderr(&out)
    );
}

#[test]
fn untracked_new_file() {
    let repo = worktree_repo();
    repo.write("src/new.ts", function("big", 11) + &function("small", 2));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(
        stdout(&out),
        "warning src/new.ts:1  big: new function, cyclomatic complexity 12  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn ignored_file() {
    let repo = worktree_repo();
    repo.write(".gitignore", "dist/\n");
    repo.commit("ignore dist");
    repo.write("dist/out.ts", function("huge", 29));
    // An edit next to it shows that the working tree was read.
    repo.write("src/a.ts", function("f", 3));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
}

#[test]
fn the_disk_version_wins_over_the_staged_one() {
    let repo = worktree_repo();
    repo.write("src/a.ts", function("f", 2));
    repo.git(&["add", "src/a.ts"]);
    repo.write("src/a.ts", function("f", 8));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    let report = stdout(&out);
    assert!(
        report.contains("f: cyclomatic complexity 2 -> 9 (+7)"),
        "{report}{}",
        stderr(&out)
    );
}

#[test]
fn file_moved_without_git() {
    let repo = Repo::new();
    repo.write(
        "src/old.ts",
        function("f", 1) + &function("g", 2) + &padding(20),
    );
    repo.commit("base");
    std::fs::rename(repo.dir.join("src/old.ts"), repo.dir.join("src/new.ts")).expect("move");
    repo.write(
        "src/new.ts",
        function("f", 2) + &function("g", 2) + &padding(20),
    );
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(
        stdout(&out),
        "warning src/new.ts:1  f: cyclomatic complexity 2 -> 3 (+1)  \
         [complexity/cyclomatic-increase]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn deleted_from_disk() {
    // `main` has no legacy.ts, so a missed deletion would report it.
    let repo = worktree_repo();
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/legacy.ts", function("huge", 30));
    repo.commit("legacy");
    repo.remove("src/legacy.ts");
    // An edit next to the deletion shows that the working tree was read.
    repo.write("src/a.ts", function("f", 3));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
}

#[test]
fn files_git_does_not_look_at() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.write("src/b.ts", function("g", 1));
    repo.write("src/c.ts", function("h", 1));
    repo.commit("base");
    repo.git(&["update-index", "--assume-unchanged", "src/a.ts"]);
    repo.git(&["update-index", "--skip-worktree", "src/b.ts"]);
    repo.write("src/a.ts", function("f", 3));
    repo.write("src/b.ts", function("g", 3));
    repo.write("src/c.ts", function("h", 3));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(
        stdout(&out),
        "warning src/c.ts:1  h: cyclomatic complexity 2 -> 4 (+2)  \
         [complexity/cyclomatic-increase]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn an_edit_in_the_same_instant_as_the_index_write() {
    // Set up as in design D4 of `fix-racy-worktree-snapshot`: git sees the
    // edit as racily clean. Ctime, which cannot be set, is not trusted.
    let repo = Repo::new();
    let set_modified = |path: &str, time: SystemTime| {
        std::fs::File::options()
            .write(true)
            .open(repo.dir.join(path))
            .and_then(|f| f.set_modified(time))
            .expect("set the modification time");
    };
    repo.git(&["config", "core.trustctime", "false"]);
    let after = function("f", 3);
    let before = function("f", 1);
    let before = format!("{before}//{}\n", " ".repeat(after.len() - before.len() - 3));
    assert_eq!(before.len(), after.len());
    let then = SystemTime::now() - Duration::from_secs(60 * 60);
    repo.write("src/a.ts", &before);
    set_modified("src/a.ts", then);
    repo.commit("base");
    // Written in place: the size and the inode stay the same.
    repo.write("src/a.ts", &after);
    set_modified("src/a.ts", then);
    set_modified(".git/index", then);

    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(stdout(&out), F_INCREASE, "{}", stderr(&out));
}

// revision-input: The working tree replaces the head everywhere

#[test]
fn copy_of_an_uncommitted_file() {
    let repo = Repo::new();
    repo.write("src/orders/format.ts", ORDER);
    repo.commit("base");
    repo.write("src/utils/date.ts", DATE_HELPER);
    repo.write("src/orders/format.ts", RENAMED_COPY);
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    let report = stdout(&out);
    assert!(
        report.contains("src/orders/format.ts")
            && report.contains("src/utils/date.ts")
            && report.contains("[duplication/duplicate-block]"),
        "{report}{}",
        stderr(&out)
    );
}

#[test]
fn duplication_compares_against_the_working_tree() {
    // HEAD has the helper in old.ts, which is gone from disk: a block
    // pasted into a new file is no copy of anything in the working tree.
    let repo = Repo::new();
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/old.ts", DATE_HELPER);
    repo.commit("helper");
    repo.remove("src/old.ts");
    repo.write("src/b.ts", format!("// moved\n{DATE_HELPER}"));
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(stdout(&out), NO_REGRESSIONS, "{}", stderr(&out));
    // Back on disk, the two files share the block.
    repo.git(&["checkout", "--", "src/old.ts"]);
    let restored = repo.qgate(&["--base", "main", "--worktree"]);
    assert!(
        stdout(&restored).contains("[duplication/duplicate-block]"),
        "{}",
        stdout(&restored)
    );
}

#[test]
fn only_the_uncommitted_changes() {
    let repo = Repo::new();
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/a.ts", function("committed", 11));
    repo.commit("committed function");
    // Not `function` again: a second copy of its shape is a duplicate.
    let chain: Vec<String> = (0..12).map(|i| format!("c[{i}]")).collect();
    repo.write(
        "src/b.ts",
        format!(
            "export function uncommitted(c: boolean[]): boolean {{\n  return {};\n}}\n",
            chain.join(" && ")
        ),
    );
    let out = repo.qgate(&["--base", "HEAD", "--worktree"]);
    assert_eq!(
        stdout(&out),
        "warning src/b.ts:1  uncommitted: new function, cyclomatic complexity 12  \
         [complexity/cyclomatic-new]\n",
        "{}",
        stderr(&out)
    );
}

#[test]
fn clean_working_tree() {
    let repo = feature_repo();
    repo.git(&["checkout", "-q", "feature"]);
    let worktree = repo.qgate(&["--base", "main", "--worktree"]);
    let head = repo.qgate(&["--base", "main"]);
    assert_eq!(stdout(&worktree), F_INCREASE, "{}", stderr(&worktree));
    assert_eq!(worktree.stdout, head.stdout);
    assert_eq!(worktree.status.code(), head.status.code());
}

// revision-input: Checking the working tree changes nothing

/// A repository with a staged, an unstaged and an untracked change.
fn repo_with_every_kind_of_change() -> Repo {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 1));
    repo.write("src/b.ts", function("g", 1));
    repo.commit("base");
    repo.write("src/a.ts", function("f", 3));
    repo.git(&["add", "src/a.ts"]);
    repo.write("src/b.ts", function("g", 3));
    repo.write("src/c.ts", function("h", 11));
    repo
}

/// The check ran and saw the staged, the unstaged and the untracked
/// change of `repo_with_every_kind_of_change`.
fn assert_saw_every_change(out: &Output) {
    assert_eq!(out.status.code(), Some(0), "{}", stderr(out));
    let report = stdout(out);
    for expected in [
        "src/a.ts:1  f: cyclomatic complexity 2 -> 4",
        "src/b.ts:1  g: cyclomatic complexity 2 -> 4",
        "src/c.ts:1  h: new function",
    ] {
        assert!(report.contains(expected), "missing {expected:?}: {report}");
    }
}

#[test]
fn staged_changes_stay_staged() {
    let repo = repo_with_every_kind_of_change();
    let state = || {
        (
            repo.git(&["status", "--porcelain"]),
            repo.git(&["diff", "--cached"]),
        )
    };
    let before = state();
    assert_saw_every_change(&repo.qgate(&["--base", "main", "--worktree"]));
    assert_eq!(state(), before);
}

#[test]
fn no_commit_appears() {
    let repo = repo_with_every_kind_of_change();
    let state = || {
        (
            repo.git(&["rev-parse", "HEAD"]),
            repo.git(&["for-each-ref"]),
            repo.git(&["reflog", "HEAD"]),
        )
    };
    let before = state();
    assert_saw_every_change(&repo.qgate(&["--base", "main", "--worktree"]));
    assert_eq!(state(), before);
}

#[cfg(unix)]
#[test]
fn hooks_do_not_run() {
    use std::os::unix::fs::PermissionsExt as _;

    let repo = repo_with_every_kind_of_change();
    let mut markers = Vec::new();
    // post-index-change is the one `git add` would run.
    for hook in ["pre-commit", "post-commit", "post-index-change"] {
        let marker = repo.dir.join(format!("{hook}.ran"));
        let script = repo.dir.join(".git/hooks").join(hook);
        repo.write(
            &format!(".git/hooks/{hook}"),
            format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        );
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("make the hook executable");
        markers.push(marker);
    }
    assert_saw_every_change(&repo.qgate(&["--base", "main", "--worktree"]));
    for marker in &markers {
        assert!(!marker.exists(), "{} exists", marker.display());
    }
    // The hooks work: a plain `git add` runs post-index-change.
    repo.git(&["add", "-A"]);
    assert!(markers[2].exists(), "the hook never runs");
}

// revision-input: Working tree errors

/// The check failed with exit code 2 and empty standard output; returns
/// standard error.
fn failed(out: &Output) -> String {
    assert_eq!(stdout(out), "", "{}", stderr(out));
    assert_eq!(out.status.code(), Some(2), "{}", stderr(out));
    stderr(out)
}

/// Whether `text` holds a full object id, which no message should show.
fn has_object_id(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_hexdigit())
        .any(|word| word.len() >= 40)
}

#[test]
fn repository_without_commits() {
    for base in ["main", "HEAD"] {
        let repo = Repo::new();
        repo.write("src/a.ts", function("f", 1));
        let out = repo.qgate(&["--base", base, "--worktree"]);
        let err = failed(&out);
        assert!(err.contains("unknown revision `HEAD`"), "{base}: {err}");
        assert!(!err.contains("fetch-depth"), "{base}: {err}");
    }
}

#[test]
fn qgate_toml_is_not_a_regular_file_on_disk() {
    let repo = repo_with_config(RULE);
    repo.remove("qgate.toml");
    repo.write("qgate.toml/inner", RULE);
    let out = repo.qgate(&["--base", "HEAD~1", "--worktree"]);
    let err = failed(&out);
    assert!(
        err.contains("`qgate.toml` is not a regular file in the working tree"),
        "{err}"
    );
    assert!(!has_object_id(&err), "{err}");
}

#[cfg(unix)]
#[test]
fn unreadable_file_in_the_working_tree() {
    use std::os::unix::fs::PermissionsExt as _;

    let repo = worktree_repo();
    repo.write("src/secret.ts", function("s", 1));
    let secret = repo.dir.join("src/secret.ts");
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    if std::fs::read(&secret).is_ok() {
        // Running as root: nothing is unreadable.
        return;
    }
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    let err = failed(&out);
    assert!(
        err.contains("cannot take a snapshot of the working tree") && err.contains("src/secret.ts"),
        "{err}"
    );
}

#[test]
fn unrelated_histories_with_the_working_tree() {
    let repo = worktree_repo();
    repo.git(&["checkout", "-q", "--orphan", "other"]);
    repo.commit("unrelated");
    repo.git(&["checkout", "-q", "main"]);
    let out = repo.qgate(&["--base", "other", "--worktree"]);
    let err = failed(&out);
    assert!(
        err.contains("`other` and `HEAD` have no merge base"),
        "{err}"
    );
    assert!(!has_object_id(&err), "{err}");
}

// configuration: Configuration from the head revision, with --worktree

#[test]
fn uncommitted_configuration_counts_with_the_working_tree() {
    let repo = Repo::new();
    repo.write("src/domain/order.ts", ORDER);
    repo.commit("base");
    repo.write("qgate.toml", RULE);
    repo.write("src/domain/order.ts", ORDER_WITH_BUTTON);
    let out = repo.qgate(&["--base", "main", "--worktree"]);
    assert_eq!(stdout(&out), BUTTON_FINDING, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn invalid_configuration_on_disk() {
    let repo = repo_with_config(RULE);
    repo.write("qgate.toml", "broken [");
    let err = failed(&repo.qgate(&["--base", "HEAD~1", "--worktree"]));
    assert!(err.contains("qgate.toml"), "{err}");
}
