//! End-to-end tests for `qgate check --format markdown` and `--format
//! azure`, following the scenarios of the `report-formats` spec that the
//! `add-pr-comments` change adds. Each test builds a throwaway git
//! repository and runs the built binary in it, as `json.rs` does.

mod support;

use std::fmt::Write as _;

use support::{Repo, function, stderr, stdout};

/// A repository with `base` committed on `main` and `head` committed on
/// `feature`, which stays checked out, so `--base main` compares them.
fn change(base: &[(&str, &str)], head: &[(&str, &str)]) -> Repo {
    let repo = Repo::new();
    for (path, contents) in base {
        repo.write(path, contents);
    }
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    for (path, contents) in head {
        repo.write(path, contents);
    }
    repo.commit("head");
    repo
}

/// `f` in `src/a.ts` goes from 2 to 4 between `main` and `HEAD`: one
/// Warning.
fn f_got_worse() -> Repo {
    change(
        &[("src/a.ts", &function("f", 1))],
        &[("src/a.ts", &function("f", 3))],
    )
}

/// A forbid rule that makes an import from `src/ui` in `src/domain` an
/// Error finding.
const RULE: &str = "[[architecture.forbid]]\nname = \"domain-is-pure\"\n\
                    from = [\"src/domain/**\"]\nto = [\"src/ui/**\"]\n";

/// A change that adds one forbidden import to `src/domain/order.ts` (an
/// Error on line 1) and makes `f` in `src/a.ts` worse (a Warning).
fn error_and_warning() -> Repo {
    let mut import = String::new();
    let _ = writeln!(import, "import {{ W }} from \"../ui/W\";");
    change(
        &[
            ("qgate.toml", RULE),
            ("src/domain/order.ts", ""),
            ("src/a.ts", &function("f", 1)),
        ],
        &[
            ("src/domain/order.ts", &import),
            ("src/a.ts", &function("f", 3)),
        ],
    )
}

const SUMMARY_LINE: &str = "qgate: failing: 1 finding at or above \"error\" (see --fail-on)\n";

// Requirement: Format option

#[test]
fn markdown_and_azure_in_every_form() {
    let repo = f_got_worse();
    repo.write("old.ts", function("f", 1));
    repo.write("new.ts", function("f", 3));
    repo.write("src/a.ts", function("f", 5));
    for form in [
        &["old.ts", "new.ts"][..],
        &["--base", "main"],
        &["--base", "main", "--worktree"],
    ] {
        let run = |format: &str| {
            let mut args = form.to_vec();
            args.extend(["--format", format]);
            let out = repo.qgate(&args);
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
            stdout(&out)
        };
        let md = run("markdown");
        assert!(
            md.starts_with("<!-- qgate count=1 failing=0 fail_on=error -->\n"),
            "{form:?}: {md}"
        );
        let azure = run("azure");
        assert!(
            azure.starts_with("##vso[task.logissue type=warning;sourcepath="),
            "{form:?}: {azure}"
        );
        assert!(
            azure.contains(";code=complexity/cyclomatic-increase]f: cyclomatic complexity 2 -> "),
            "{form:?}: {azure}"
        );
    }
}

// Requirement: Markdown report document

#[test]
fn markdown_report() {
    let repo = error_and_warning();
    let out = repo.qgate(&["--base", "main", "--format", "markdown"]);
    assert_eq!(
        stdout(&out),
        "<!-- qgate count=2 failing=1 fail_on=error -->\n\
         \n\
         ### qgate\n\
         \n\
         2 findings, 1 at or above `error`.\n\
         \n\
         | Severity | Location | Finding | Rule |\n\
         |---|---|---|---|\n\
         | warning | src/a.ts:1 | f: cyclomatic complexity 2 -\\> 4 (+2) | complexity/cyclomatic-increase |\n\
         | error | src/domain/order.ts:1 | imports src/ui/W (\"../ui/W\"), forbidden by \"domain-is-pure\" | architecture/forbidden-import |\n",
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).ends_with(SUMMARY_LINE), "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(1));
    let again = repo.qgate(&["--base", "main", "--format", "markdown"]);
    assert_eq!(stdout(&again), stdout(&out));
}

#[test]
fn markdown_no_findings() {
    let repo = change(
        &[("src/a.ts", &function("f", 1))],
        &[("src/a.ts", &function("f", 1)), ("README.md", "x\n")],
    );
    let out = repo.qgate(&["--base", "main", "--format", "markdown"]);
    assert_eq!(
        stdout(&out),
        "<!-- qgate count=0 failing=0 fail_on=error -->\n\n### qgate\n\nNo regressions found.\n"
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

// Requirement: Exit codes and errors in every format

#[test]
fn markdown_never_failing() {
    let repo = error_and_warning();
    let out = repo.qgate(&[
        "--base",
        "main",
        "--format",
        "markdown",
        "--fail-on",
        "never",
    ]);
    let md = stdout(&out);
    assert!(
        md.starts_with("<!-- qgate count=2 failing=0 fail_on=never -->\n"),
        "{md}"
    );
    assert!(md.contains("| error | src/domain/order.ts:1 |"), "{md}");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

// Scenario "A finding at the level", end to end: the CLI counts the
// failing findings at the level it was given.
#[test]
fn markdown_at_warning() {
    let repo = error_and_warning();
    let out = repo.qgate(&[
        "--base",
        "main",
        "--format",
        "markdown",
        "--fail-on",
        "warning",
    ]);
    let md = stdout(&out);
    assert!(
        md.starts_with("<!-- qgate count=2 failing=2 fail_on=warning -->\n"),
        "{md}"
    );
    assert!(
        md.contains("\n2 findings, 2 at or above `warning`.\n"),
        "{md}"
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
}

#[test]
fn unknown_base_revision_in_markdown_and_azure() {
    let repo = f_got_worse();
    for format in ["markdown", "azure"] {
        let out = repo.qgate(&["--base", "no-such-branch", "--format", format]);
        assert_eq!(stdout(&out), "", "{format}");
        assert!(
            stderr(&out).contains("unknown revision `no-such-branch`"),
            "{format}: {}",
            stderr(&out)
        );
        assert_eq!(out.status.code(), Some(2), "{format}");
    }
}

// Requirement: Azure Pipelines annotations

#[test]
fn azure_annotations() {
    let repo = error_and_warning();
    let out = repo.qgate(&["--base", "main", "--format", "azure"]);
    assert_eq!(
        stdout(&out),
        "##vso[task.logissue type=warning;sourcepath=src/a.ts;linenumber=1;\
         code=complexity/cyclomatic-increase]f: cyclomatic complexity 2 -> 4 (+2)\n\
         ##vso[task.logissue type=error;sourcepath=src/domain/order.ts;linenumber=1;\
         code=architecture/forbidden-import]imports src/ui/W (\"../ui/W\"), forbidden by \"domain-is-pure\"\n",
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).ends_with(SUMMARY_LINE), "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn azure_no_findings() {
    let repo = change(
        &[("src/a.ts", &function("f", 1))],
        &[("src/a.ts", &function("f", 1)), ("README.md", "x\n")],
    );
    let out = repo.qgate(&["--base", "main", "--format", "azure"]);
    assert_eq!(stdout(&out), "No regressions found.\n");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}
