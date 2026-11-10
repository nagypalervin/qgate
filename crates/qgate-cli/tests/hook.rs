//! End-to-end tests for `qgate hook claude-code`, one per scenario of the
//! `agent-hooks` spec. Each test builds a throwaway git repository, feeds
//! Claude Code's hook input on standard input and reads the response
//! (design D7 of `add-agent-hooks`).

mod support;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};
use support::{DATE_HELPER, RENAMED_COPY, Repo, function, isolate, scratch_dir, stderr, stdout};

const FIRST_STOP: &str = r#"{"hook_event_name":"Stop","stop_hook_active":false}"#;
const SECOND_STOP: &str = r#"{"hook_event_name":"Stop","stop_hook_active":true}"#;

/// How to start the hook.
struct Run<'a> {
    /// The process's current directory.
    dir: &'a Path,
    /// Options after `qgate hook claude-code`.
    args: &'a [&'a str],
    /// Standard input.
    input: &'a str,
    /// `CLAUDE_PROJECT_DIR`, removed when `None`.
    project_dir: Option<&'a Path>,
}

fn run(run: &Run<'_>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qgate"));
    isolate(&mut command)
        .args(["hook", "claude-code"])
        .args(run.args)
        .current_dir(run.dir)
        .env_remove("CLAUDE_PROJECT_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(project) = run.project_dir {
        command.env("CLAUDE_PROJECT_DIR", project);
    }
    let mut child = command.spawn().expect("qgate runs");
    // The hook may exit before reading (a usage error): ignore a closed pipe.
    let _ = child
        .stdin
        .take()
        .expect("stdin")
        .write_all(run.input.as_bytes());
    child.wait_with_output().expect("qgate finishes")
}

/// Run the hook in the repository root with `args` and `input`.
fn hook(repo: &Repo, args: &[&str], input: &str) -> Output {
    run(&Run {
        dir: &repo.dir,
        args,
        input,
        project_dir: None,
    })
}

/// The response as JSON. Fails unless the exit code is 0 and standard
/// output is one JSON object on one line.
fn response(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(0), "{}", stderr(out));
    let text = stdout(out);
    assert!(text.ends_with("}\n"), "{text:?} {}", stderr(out));
    assert_eq!(text.matches('\n').count(), 1, "{text:?}");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text:?}"))
}

/// A repository with `files` committed on `main`.
fn committed(files: &[(&str, &str)]) -> Repo {
    let repo = Repo::new();
    for (path, contents) in files {
        repo.write(path, contents);
    }
    repo.commit("base");
    repo
}

/// `f` in `src/a.ts` at complexity 2, committed; on disk at `after`.
fn f_goes_to(after: usize) -> Repo {
    let repo = committed(&[("src/a.ts", &function("f", 1))]);
    repo.write("src/a.ts", function("f", after - 1));
    repo
}

const F_ERROR: &str = "error   src/a.ts:1  f: cyclomatic complexity 2 -> 9 (+7)  \
                       [complexity/cyclomatic-increase]";

fn reason(lines: &str) -> String {
    format!(
        "qgate: your changes made code worse (compared with HEAD):\n{lines}\n\
         Fix these, or if a change is intended, tell the user why and stop."
    )
}

/// The `reason` of a response that blocks.
fn block_reason(out: &Output) -> String {
    let response = response(out);
    assert_eq!(response["decision"], "block", "{response}");
    response["reason"].as_str().expect("a reason").to_owned()
}

/// The `systemMessage` of a response that does not block.
fn message_only(out: &Output) -> String {
    let response = response(out);
    assert!(response.get("decision").is_none(), "{response}");
    assert!(response.get("reason").is_none(), "{response}");
    response["systemMessage"]
        .as_str()
        .expect("a systemMessage")
        .to_owned()
}

// Requirement: Hook command

#[test]
fn nothing_to_report() {
    let repo = committed(&[("src/a.ts", &function("f", 1))]);
    let out = hook(&repo, &[], FIRST_STOP);
    assert_eq!(stdout(&out), "", "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn help_lists_the_hook_command() {
    let out = Command::new(env!("CARGO_BIN_EXE_qgate"))
        .arg("--help")
        .output()
        .expect("qgate runs");
    assert!(stdout(&out).contains("hook"), "{}", stdout(&out));
    let hook_help = Command::new(env!("CARGO_BIN_EXE_qgate"))
        .args(["hook", "claude-code", "--help"])
        .output()
        .expect("qgate runs");
    assert_eq!(hook_help.status.code(), Some(0));
    for option in ["--base <rev>", "--fail-on <level>"] {
        assert!(
            stdout(&hook_help).contains(option),
            "{}",
            stdout(&hook_help)
        );
    }
}

// Requirement: What the hook checks

/// `main` has `f` and `g` at complexity 2; the `feature` branch commits
/// `f` at 9, and `g` is at 9 on disk only.
fn committed_and_uncommitted() -> Repo {
    let repo = committed(&[
        ("src/a.ts", &function("f", 1)),
        ("src/b.ts", &function("g", 1)),
    ]);
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/a.ts", function("f", 8));
    repo.commit("f gets complex");
    repo.write("src/b.ts", function("g", 8));
    repo
}

#[test]
fn uncommitted_work_only() {
    let repo = committed_and_uncommitted();
    let reason = block_reason(&hook(&repo, &[], FIRST_STOP));
    assert!(reason.contains("src/b.ts:1  g: "), "{reason}");
    assert!(!reason.contains("src/a.ts"), "{reason}");
}

#[test]
fn same_findings_as_check() {
    let repo = committed(&[
        ("src/a.ts", &(function("f", 1) + &function("h", 1))),
        ("src/b.ts", &function("g", 1)),
        ("src/utils/date.ts", DATE_HELPER),
    ]);
    repo.write("src/a.ts", function("f", 3) + &function("h", 8));
    repo.write("src/b.ts", function("g", 8));
    repo.write("src/orders/format.ts", RENAMED_COPY);
    let message = message_only(&hook(&repo, &["--fail-on", "never"], FIRST_STOP));
    let hook_lines: Vec<&str> = message.lines().skip(1).collect();

    let check = repo.qgate(&["--base", "HEAD", "--worktree", "--fail-on", "never"]);
    let text = stdout(&check);
    let check_lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("error ") || line.starts_with("warning "))
        .collect();
    assert!(check_lines.len() >= 4, "{text}");
    assert!(
        check_lines.iter().any(|l| l.starts_with("warning ")),
        "{text}"
    );
    assert!(
        check_lines.iter().any(|l| l.starts_with("error ")),
        "{text}"
    );
    assert_eq!(hook_lines, check_lines, "{message}\n---\n{text}");
}

#[test]
fn another_base() {
    let repo = committed_and_uncommitted();
    let reason = block_reason(&hook(&repo, &["--base", "main"], FIRST_STOP));
    assert!(
        reason.starts_with("qgate: your changes made code worse (compared with main):\n"),
        "{reason}"
    );
    assert!(reason.contains("src/a.ts:1  f: "), "{reason}");
    assert!(reason.contains("src/b.ts:1  g: "), "{reason}");
}

#[test]
fn project_directory() {
    let repo = f_goes_to(9);
    let outside = scratch_dir("outside");
    let elsewhere = format!(
        r#"{{"hook_event_name":"Stop","stop_hook_active":false,"cwd":{}}}"#,
        json!(outside)
    );
    let from_variable = run(&Run {
        dir: &outside,
        args: &[],
        input: &elsewhere,
        project_dir: Some(&repo.dir),
    });
    assert_eq!(block_reason(&from_variable), reason(F_ERROR));

    // Without the variable, the input's cwd counts.
    let in_repo = format!(
        r#"{{"hook_event_name":"Stop","stop_hook_active":false,"cwd":{}}}"#,
        json!(repo.dir.join("src"))
    );
    let from_cwd = run(&Run {
        dir: &outside,
        args: &[],
        input: &in_repo,
        project_dir: None,
    });
    assert_eq!(block_reason(&from_cwd), reason(F_ERROR));

    // The variable wins over the input's cwd: a clean repository there.
    let clean = committed(&[("src/a.ts", &function("f", 1))]);
    let both = run(&Run {
        dir: &outside,
        args: &[],
        input: &in_repo,
        project_dir: Some(&clean.dir),
    });
    assert_eq!(stdout(&both), "", "{}", stderr(&both));

    // Without either, the current directory.
    let from_dir = run(&Run {
        dir: &repo.dir,
        args: &[],
        input: FIRST_STOP,
        project_dir: None,
    });
    assert_eq!(block_reason(&from_dir), reason(F_ERROR));
    let _ = std::fs::remove_dir_all(outside);
}

// Requirement: Sending the agent back

#[test]
fn one_failing_finding() {
    let repo = f_goes_to(9);
    let out = hook(&repo, &[], FIRST_STOP);
    assert_eq!(
        response(&out),
        json!({"decision": "block", "reason": reason(F_ERROR)})
    );
    assert_eq!(
        stdout(&out),
        format!(
            "{{\"decision\":\"block\",\"reason\":{}}}\n",
            json!(reason(F_ERROR))
        )
    );
}

#[test]
fn failing_on_warnings() {
    let repo = committed(&[("src/utils/date.ts", DATE_HELPER)]);
    repo.write("src/orders/format.ts", RENAMED_COPY);
    let reason = block_reason(&hook(&repo, &["--fail-on", "warning"], FIRST_STOP));
    assert!(reason.contains("[duplication/duplicate-block]"), "{reason}");
    // The same change does not block at the default level.
    let default = message_only(&hook(&repo, &[], FIRST_STOP));
    assert!(
        default.contains("[duplication/duplicate-block]"),
        "{default}"
    );
}

// Requirement: One round of fixing

#[test]
fn findings_remain_after_the_round() {
    let repo = f_goes_to(9);
    let out = hook(&repo, &[], SECOND_STOP);
    assert_eq!(
        stdout(&out),
        format!(
            "{{\"systemMessage\":{}}}\n",
            json!(format!(
                "qgate: these findings remain after one round of fixing:\n{F_ERROR}"
            ))
        ),
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn fixed_in_the_round() {
    let repo = f_goes_to(9);
    assert!(stdout(&hook(&repo, &[], FIRST_STOP)).contains("block"));
    repo.write("src/a.ts", function("f", 1));
    let out = hook(&repo, &[], SECOND_STOP);
    assert_eq!(stdout(&out), "", "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
}

// Requirement: Findings below the level

#[test]
fn only_a_warning() {
    let repo = f_goes_to(4);
    let out = hook(&repo, &[], FIRST_STOP);
    assert_eq!(
        stdout(&out),
        format!(
            "{{\"systemMessage\":{}}}\n",
            json!(
                "qgate: below the --fail-on level \"error\":\nwarning src/a.ts:1  f: \
                 cyclomatic complexity 2 -> 4 (+2)  [complexity/cyclomatic-increase]"
            )
        ),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_warning_next_to_an_error() {
    let repo = committed(&[
        ("src/a.ts", &function("f", 1)),
        ("src/b.ts", &function("g", 1)),
    ]);
    repo.write("src/a.ts", function("f", 8));
    repo.write("src/b.ts", function("g", 3));
    let response = response(&hook(&repo, &[], FIRST_STOP));
    assert_eq!(response["decision"], "block", "{response}");
    let reason = response["reason"].as_str().expect("a reason");
    let message = response["systemMessage"].as_str().expect("a message");
    assert!(
        reason.contains("f: ") && !reason.contains("g: "),
        "{reason}"
    );
    assert!(
        message.contains("g: ") && !message.contains("f: "),
        "{message}"
    );
}

#[test]
fn never_failing() {
    let repo = f_goes_to(9);
    let message = message_only(&hook(&repo, &["--fail-on", "never"], FIRST_STOP));
    assert!(message.contains(F_ERROR), "{message}");
}

// Requirement: Long lists

#[test]
fn twenty_five_failing_findings() {
    let files: Vec<(String, String)> = (0..25)
        .map(|i| (format!("src/f{i:02}.ts"), function(&format!("f{i:02}"), 1)))
        .collect();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    let repo = committed(&refs);
    for (path, _) in &files {
        let name = &path[4..7];
        repo.write(path, function(name, 8));
    }
    let reason = block_reason(&hook(&repo, &[], FIRST_STOP));
    let lines: Vec<&str> = reason.lines().collect();
    assert_eq!(lines.len(), 23, "{reason}");
    for (i, line) in lines[1..21].iter().enumerate() {
        assert!(
            line.starts_with(&format!("error   src/f{i:02}.ts:1  ")),
            "{reason}"
        );
    }
    assert_eq!(
        lines[21],
        "... and 5 more; run qgate check --base HEAD --worktree to see all"
    );
}

// Requirement: Errors go to the user

#[test]
fn invalid_configuration() {
    let repo = f_goes_to(9);
    repo.write("qgate.toml", "[architecture\n");
    let message = message_only(&hook(&repo, &[], FIRST_STOP));
    assert!(message.starts_with("qgate: "), "{message}");
    assert!(message.contains("qgate.toml"), "{message}");
}

#[test]
fn outside_a_repository() {
    let outside = scratch_dir("no-repo");
    let out = run(&Run {
        dir: &outside,
        args: &[],
        input: FIRST_STOP,
        project_dir: None,
    });
    let message = message_only(&out);
    assert!(message.starts_with("qgate: "), "{message}");
    let _ = std::fs::remove_dir_all(outside);
}

#[test]
fn input_that_is_not_hook_input() {
    let repo = f_goes_to(9);
    let message = message_only(&hook(&repo, &[], "not json"));
    assert!(message.contains("not valid hook input"), "{message}");
}

#[test]
fn another_hook_event() {
    let repo = f_goes_to(9);
    let message = message_only(&hook(&repo, &[], r#"{"hook_event_name":"PostToolUse"}"#));
    assert!(message.contains("only the Stop event"), "{message}");
}

#[test]
fn unknown_fail_on_level() {
    let repo = f_goes_to(9);
    let out = hook(&repo, &["--fail-on", "info"], FIRST_STOP);
    assert!(
        stderr(&out).contains("[possible values: error, warning, never]"),
        "{}",
        stderr(&out)
    );
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn unknown_option() {
    let repo = f_goes_to(9);
    let out = hook(&repo, &["--worktree"], FIRST_STOP);
    assert_eq!(stdout(&out), "");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
}

// Requirement: The hook changes nothing

#[test]
fn staged_work_stays_staged() {
    let repo = committed(&[
        ("src/a.ts", &function("f", 1)),
        ("src/b.ts", &function("g", 1)),
    ]);
    repo.write("src/a.ts", function("f", 8));
    repo.git(&["add", "src/a.ts"]);
    repo.write("src/b.ts", function("g", 8));
    let before = repo.git(&["status", "--porcelain"]);
    let reason = block_reason(&hook(&repo, &[], FIRST_STOP));
    assert!(reason.contains("f: ") && reason.contains("g: "), "{reason}");
    assert_eq!(repo.git(&["status", "--porcelain"]), before);
}
