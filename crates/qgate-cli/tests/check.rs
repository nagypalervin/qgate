//! End-to-end tests for `qgate check`, one per scenario of the
//! `check-command` spec. They run the built binary, because exit codes and
//! the split between standard output and standard error can only be
//! checked from outside the process (design D9).

use std::path::PathBuf;
use std::process::{Command, Output};

/// Run `qgate` with `args` from the fixture directory, so paths in the
/// output are exactly the relative paths passed in.
fn qgate(args: &[&str]) -> Output {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    Command::new(env!("CARGO_BIN_EXE_qgate"))
        .args(args)
        .current_dir(fixtures)
        .output()
        .expect("qgate runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

// Requirement: Compare two files

#[test]
fn a_function_got_more_complex() {
    let out = qgate(&["check", "grow/before.ts", "grow/after.ts"]);
    assert_eq!(
        stdout(&out),
        "warning grow/after.ts:1  f: cyclomatic complexity 2 -> 4 (+2)  \
         [complexity/cyclomatic-increase]\n"
    );
    assert_eq!(out.status.code(), Some(0));
}

// Requirement: Finding line format

#[test]
fn rendered_finding() {
    let out = qgate(&["check", "old/orders.ts", "src/orders.ts"]);
    assert_eq!(
        stdout(&out),
        "error   src/orders.ts:42  OrderService.process: cyclomatic complexity 3 -> 9 (+6)  \
         [complexity/cyclomatic-increase]\n"
    );
}

// Requirement: No regressions

#[test]
fn identical_files() {
    let out = qgate(&["check", "same.ts", "same.ts"]);
    assert_eq!(stdout(&out), "No regressions found.\n");
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn only_improvements() {
    let out = qgate(&["check", "improve/before.ts", "improve/after.ts"]);
    assert_eq!(stdout(&out), "No regressions found.\n");
    assert_eq!(out.status.code(), Some(0));
}

// Requirement: Deterministic output order

#[test]
fn several_findings_are_sorted_by_line_then_rule() {
    let out = qgate(&["check", "sort/before.ts", "sort/after.ts"]);
    let lines: Vec<String> = stdout(&out)
        .lines()
        .map(|l| l.split("  ").take(2).collect::<Vec<_>>().join(" | "))
        .collect();
    assert_eq!(
        lines,
        [
            "warning sort/after.ts:10 | grows: cyclomatic complexity 2 -> 3 (+1)",
            "warning sort/after.ts:30 | bigNew: new function, cyclomatic complexity 12",
            // Same line: the increase sorts before the new function by rule
            // id, although the new function comes first in the source.
            "warning sort/after.ts:40 | sameLineGrows: cyclomatic complexity 2 -> 3 (+1)",
            "warning sort/after.ts:40 | sameLineNew: new function, cyclomatic complexity 12",
        ]
    );
}

#[test]
fn output_is_byte_identical_across_runs() {
    let first = qgate(&["check", "sort/before.ts", "sort/after.ts"]);
    let second = qgate(&["check", "sort/before.ts", "sort/after.ts"]);
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn closed_output_pipe_is_not_a_crash() {
    use std::io::Write as _;
    use std::process::Stdio;

    // Enough findings to overflow any pipe buffer, so qgate is still
    // writing when the reader goes away (like `qgate check ... | head -n 1`).
    let dir = std::env::temp_dir().join(format!("qgate-pipe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let big = dir.join("big.ts");
    let mut source = std::fs::File::create(&big).expect("create big.ts");
    for i in 0..3_000 {
        write!(source, "function f{i}(c: boolean[]): number {{ ").expect("write");
        for j in 0..11 {
            write!(source, "if (c[{j}]) {{ return {j}; }} ").expect("write");
        }
        writeln!(source, "return -1; }}").expect("write");
    }
    drop(source);

    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut child = Command::new(env!("CARGO_BIN_EXE_qgate"))
        .args([
            "check".as_ref(),
            fixtures.join("same.ts").as_os_str(),
            big.as_os_str(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("qgate starts");
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("qgate finishes");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

// Requirement: Findings do not change the exit code

#[test]
fn error_finding_exits_zero() {
    let out = qgate(&["check", "old/orders.ts", "src/orders.ts"]);
    assert!(stdout(&out).starts_with("error "));
    assert_eq!(out.status.code(), Some(0));
}

// Requirement: Supported input files

#[test]
fn tsx_input() {
    let out = qgate(&["check", "tsx/before.tsx", "tsx/after.tsx"]);
    assert_eq!(
        stdout(&out),
        "warning tsx/after.tsx:1  Greeting: cyclomatic complexity 1 -> 2 (+1)  \
         [complexity/cyclomatic-increase]\n"
    );
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn different_supported_extensions() {
    let out = qgate(&["check", "same.ts", "tsx/after.tsx"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn unsupported_extension() {
    let out = qgate(&["check", "js/before.js", "js/after.js"]);
    assert!(stderr(&out).contains("js/before.js"), "{}", stderr(&out));
    assert!(stderr(&out).contains("unsupported file type"));
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

// Requirement: Unreadable input files

#[test]
fn missing_file() {
    let out = qgate(&["check", "grow/before.ts", "does-not-exist.ts"]);
    assert!(
        stderr(&out).contains("does-not-exist.ts"),
        "{}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

// Requirement: Files with syntax errors

#[test]
fn broken_after_file() {
    let out = qgate(&["check", "broken/before.ts", "broken/after.ts"]);
    assert!(
        stderr(&out).contains("broken/after.ts:7"),
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("syntax error"));
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(2));
}

// Requirement: Invalid invocation

#[test]
fn one_argument() {
    let out = qgate(&["check", "grow/after.ts"]);
    assert!(
        stderr(&out).contains("qgate check <before> <after>"),
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn three_arguments() {
    let out = qgate(&["check", "same.ts", "same.ts", "same.ts"]);
    assert!(stderr(&out).contains("Usage"), "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_argument() {
    let out = qgate(&["check", "--bogus", "same.ts", "same.ts"]);
    assert!(stderr(&out).contains("--bogus"), "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(2));
}

// Requirement: Help and version

#[test]
fn version() {
    let out = qgate(&["--version"]);
    assert_eq!(
        stdout(&out),
        format!("qgate {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn help_lists_check() {
    let out = qgate(&["--help"]);
    assert!(stdout(&out).contains("check"), "{}", stdout(&out));
    assert_eq!(out.status.code(), Some(0));
}
