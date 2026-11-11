//! End-to-end tests for `qgate check --format json`, one per scenario of
//! the `report-formats` spec, plus the JSON scenario of the
//! `duplication-analysis` spec. Each test builds a throwaway git repository
//! and runs the built binary in it, as `revisions.rs` does.

mod support;

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Output;

use serde_json::{Value, json};
use support::schema::{report_v1, validate};
use support::{DATE_HELPER, RENAMED_COPY, Repo, function, padding, qgate_in, stderr, stdout};

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

/// `f` in `src/a.ts` goes from 2 to 4 between `main` and `HEAD`.
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

/// `count` forbidden imports, one per line from line 1.
fn forbidden_imports(count: usize) -> String {
    (0..count).fold(String::new(), |mut text, i| {
        let _ = writeln!(text, "import {{ W{i} }} from \"../ui/W{i}\";");
        text
    })
}

/// A change that adds `count` forbidden imports to `src/domain/order.ts`.
fn errors(count: usize) -> Repo {
    change(
        &[("qgate.toml", RULE), ("src/domain/order.ts", "")],
        &[("src/domain/order.ts", &forbidden_imports(count))],
    )
}

/// Standard output as JSON. Fails unless it is one line, ends with a
/// newline and parses.
fn report(out: &Output) -> Value {
    let text = stdout(out);
    assert!(text.ends_with('\n'), "{text:?} {}", stderr(out));
    assert_eq!(text.matches('\n').count(), 1, "{text:?}");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text:?}"))
}

/// The only finding of a report.
fn only_finding(out: &Output) -> Value {
    let report = report(out);
    let findings = report["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1, "{report}");
    findings[0].clone()
}

/// The JSON text of the only finding, exactly as written.
fn only_finding_text(out: &Output) -> String {
    let text = stdout(out);
    let start = text.find("\"findings\":[").expect("findings") + "\"findings\":[".len();
    let end = text.rfind("],\"summary\":").expect("summary");
    let finding = &text[start..end];
    // One finding: no second object at the top level of the array.
    assert_eq!(
        only_finding(out),
        serde_json::from_str::<Value>(finding).expect("one object")
    );
    finding.to_owned()
}

/// The finding with `rule`, which must be the only one with it.
fn finding_with_rule(report: &Value, rule: &str) -> Value {
    let matching: Vec<&Value> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["rule"] == rule)
        .collect();
    assert_eq!(matching.len(), 1, "{rule}: {report}");
    matching[0].clone()
}

const ONE_FINDING: &str = "{\"schema_version\":1,\"findings\":[{\"rule\":\"complexity/cyclomatic-increase\",\
     \"severity\":\"warning\",\"message\":\"f: cyclomatic complexity 2 -> 4 (+2)\",\
     \"path\":\"src/a.ts\",\"line\":1,\"metric\":{\"before\":2,\"after\":4}}],\
     \"summary\":{\"count\":1,\"failing\":0,\"fail_on\":\"error\"}}\n";

// Requirement: Format option

#[test]
fn default_is_text() {
    let repo = f_got_worse();
    let text = repo.qgate(&["--base", "main", "--format", "text"]);
    let default = repo.qgate(&["--base", "main"]);
    assert_eq!(text.status.code(), Some(0), "{}", stderr(&text));
    assert_eq!(stdout(&text), stdout(&default));
    assert!(
        stdout(&text).starts_with("warning src/a.ts:1  "),
        "{}",
        stdout(&text)
    );
}

#[test]
fn json_in_every_form() {
    let repo = f_got_worse();
    repo.write("old.ts", function("f", 1));
    repo.write("new.ts", function("f", 3));
    repo.write("src/a.ts", function("f", 5));
    for args in [
        &["old.ts", "new.ts", "--format", "json"][..],
        &["--base", "main", "--format", "json"],
        &["--base", "main", "--worktree", "--format", "json"],
    ] {
        let out = repo.qgate(args);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        let report = report(&out);
        assert_eq!(report["schema_version"], 1, "{args:?}: {report}");
        assert!(report["findings"].is_array(), "{args:?}: {report}");
    }
}

// Requirement: JSON report document

#[test]
fn one_finding() {
    let repo = f_got_worse();
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    assert_eq!(stdout(&out), ONE_FINDING, "{}", stderr(&out));
    assert_eq!(out.status.code(), Some(0));
    // The same inputs give the same bytes.
    let again = repo.qgate(&["--base", "main", "--format", "json"]);
    assert_eq!(stdout(&again), ONE_FINDING);
}

#[test]
fn no_findings() {
    let repo = change(
        &[("src/a.ts", &function("f", 3))],
        &[("src/a.ts", &function("f", 1))],
    );
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    assert_eq!(
        stdout(&out),
        "{\"schema_version\":1,\"findings\":[],\
         \"summary\":{\"count\":0,\"failing\":0,\"fail_on\":\"error\"}}\n",
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(0));
}

// Requirement: Finding fields

#[test]
fn new_function() {
    let repo = change(&[("src/a.ts", "")], &[("src/new.ts", &function("big", 11))]);
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    assert_eq!(
        only_finding_text(&out),
        "{\"rule\":\"complexity/cyclomatic-new\",\"severity\":\"warning\",\
         \"message\":\"big: new function, cyclomatic complexity 12\",\
         \"path\":\"src/new.ts\",\"line\":1,\"metric\":{\"after\":12}}",
        "{}",
        stderr(&out)
    );
}

#[test]
fn finding_without_a_line() {
    let repo = Repo::new();
    repo.write("src/a.ts", function("f", 0));
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("src/a.ts", b"export const s = \"\xff\xfe\";\n");
    repo.commit("head");
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    let finding = only_finding(&out);
    assert_eq!(finding["rule"], "input/unparsable", "{finding}");
    assert_eq!(finding["path"], "src/a.ts", "{finding}");
    for key in ["line", "end_line", "metric", "related"] {
        assert!(finding.get(key).is_none(), "{key}: {finding}");
    }
    assert!(!stdout(&out).contains("null"), "{}", stdout(&out));
}

#[test]
fn duplicate_block() {
    let repo = change(
        &[("src/utils/date.ts", DATE_HELPER)],
        &[("src/orders/format.ts", RENAMED_COPY)],
    );
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    assert_eq!(
        only_finding_text(&out),
        "{\"rule\":\"duplication/duplicate-block\",\"severity\":\"warning\",\
         \"message\":\"duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)\",\
         \"path\":\"src/orders/format.ts\",\"line\":4,\"end_line\":9,\
         \"related\":[{\"path\":\"src/utils/date.ts\",\"line\":1,\"end_line\":6}]}",
        "{}",
        stderr(&out)
    );
}

#[test]
fn characters_json_must_escape() {
    let repo = errors(1);
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    let message = "imports src/ui/W0 (\"../ui/W0\"), forbidden by \"domain-is-pure\"";
    assert!(
        stdout(&out).contains(
            "\"message\":\"imports src/ui/W0 (\\\"../ui/W0\\\"), forbidden by \\\"domain-is-pure\\\"\""
        ),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert_eq!(only_finding(&out)["message"], message);
}

// Requirement: Numbers in the JSON report

#[test]
fn complexity_values() {
    let repo = f_got_worse();
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    assert!(
        only_finding_text(&out).ends_with(",\"metric\":{\"before\":2,\"after\":4}}"),
        "{}",
        stdout(&out)
    );
}

// Requirement: Order of findings in the JSON report

#[test]
fn several_files() {
    // A copy in a.ts and a complexity increase in b.ts: duplication
    // findings come after every file's own findings, so only sorting puts
    // a.ts first.
    let repo = change(
        &[
            ("src/utils/date.ts", DATE_HELPER),
            ("src/b.ts", &format!("{}{}", padding(4), function("g", 1))),
        ],
        &[
            ("src/a.ts", &format!("{}{RENAMED_COPY}", padding(36))),
            ("src/b.ts", &format!("{}{}", padding(4), function("g", 3))),
        ],
    );
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    let report = report(&out);
    let places: Vec<(&str, u64)> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| {
            (
                f["path"].as_str().expect("path"),
                f["line"].as_u64().expect("line"),
            )
        })
        .collect();
    assert_eq!(places, [("src/a.ts", 40), ("src/b.ts", 5)], "{report}");
}

// Requirement: JSON summary

#[test]
fn failing_run() {
    let repo = change(
        &[
            ("qgate.toml", RULE),
            ("src/domain/order.ts", ""),
            ("src/a.ts", &function("f", 1)),
        ],
        &[
            ("src/domain/order.ts", &forbidden_imports(2)),
            ("src/a.ts", &function("f", 3)),
        ],
    );
    let out = repo.qgate(&["--base", "main", "--format", "json", "--fail-on", "warning"]);
    let report = report(&out);
    let severities: Vec<&str> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| f["severity"].as_str().expect("severity"))
        .collect();
    assert_eq!(severities, ["warning", "error", "error"], "{report}");
    assert_eq!(
        report["summary"],
        json!({"count": 3, "failing": 3, "fail_on": "warning"})
    );
    assert!(
        stdout(&out)
            .ends_with(",\"summary\":{\"count\":3,\"failing\":3,\"fail_on\":\"warning\"}}\n")
    );
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn never_failing() {
    let repo = errors(1);
    let out = repo.qgate(&["--base", "main", "--format", "json", "--fail-on", "never"]);
    assert!(
        stdout(&out).ends_with(",\"summary\":{\"count\":1,\"failing\":0,\"fail_on\":\"never\"}}\n"),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(0));
}

// Requirement: Exit codes and errors in every format

#[test]
fn unknown_base_revision() {
    let repo = f_got_worse();
    let out = repo.qgate(&["--base", "no-such-branch", "--format", "json"]);
    assert_eq!(stdout(&out), "");
    assert!(
        stderr(&out).contains("unknown revision `no-such-branch`"),
        "{}",
        stderr(&out)
    );
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn summary_line_on_standard_error() {
    let repo = errors(1);
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    assert!(
        stderr(&out).ends_with("qgate: failing: 1 finding at or above \"error\" (see --fail-on)\n"),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    assert_eq!(report(&out)["summary"]["failing"], 1);
    assert_eq!(out.status.code(), Some(1));
}

// Requirement: Versioned JSON format

#[test]
fn reports_match_the_published_schema() {
    let schema = report_v1();
    let repo = change(
        &[
            ("qgate.toml", RULE),
            ("src/a.ts", &function("f", 1)),
            ("src/domain/order.ts", ""),
            ("src/utils/date.ts", DATE_HELPER),
            ("src/broken.ts", ""),
        ],
        &[
            ("src/a.ts", &function("f", 3)),
            ("src/domain/order.ts", &forbidden_imports(1)),
            ("src/orders/format.ts", RENAMED_COPY),
            ("src/broken.ts", "function broken(a: number {\n"),
        ],
    );
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    let report = report(&out);
    for rule in [
        "complexity/cyclomatic-increase",
        "architecture/forbidden-import",
        "duplication/duplicate-block",
        "input/unparsable",
    ] {
        finding_with_rule(&report, rule);
    }
    assert_eq!(validate(&schema, &report), Ok(()), "{report}");

    let empty = report_of(&repo, &["--base", "HEAD", "--format", "json"]);
    assert_eq!(empty["findings"], json!([]), "{empty}");
    assert_eq!(validate(&schema, &empty), Ok(()), "{empty}");
}

fn report_of(repo: &Repo, args: &[&str]) -> Value {
    report(&repo.qgate(args))
}

// Requirement: Paths in the JSON report

#[test]
fn path_from_the_two_file_form() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = qgate_in(
        &fixtures,
        &["old/orders.ts", "src/orders.ts", "--format", "json"],
    );
    let report = report(&out);
    let findings = report["findings"].as_array().expect("findings");
    assert!(!findings.is_empty(), "{report}");
    for finding in findings {
        assert_eq!(finding["path"], "src/orders.ts", "{report}");
    }
}

// duplication-analysis: Duplicate block finding, "Range and related locations"

/// A seven-line helper, so the copies span lines 3-9 and 12-18 as in the
/// spec's scenarios.
const HELPER: &str = "export function formatDate(d: Date): string {
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, \"0\");
  const day = String(d.getDate()).padStart(2, \"0\");
  const hour = String(d.getHours()).padStart(2, \"0\");
  return `${year}-${month}-${day} ${hour}`;
}
";

#[test]
fn range_and_related_locations() {
    let renamed = HELPER
        .replace("formatDate", "formatOrderDate")
        .replace("year", "y")
        .replace("month", "m")
        .replace("day", "dd")
        .replace("hour", "h");
    let at = |line: usize, text: &str| format!("{}{text}", padding(line - 1));
    let repo = change(
        &[
            ("src/utils/date.ts", &at(3, HELPER)),
            ("src/a.ts", &at(1, HELPER)),
            ("src/b.ts", &at(20, HELPER)),
        ],
        &[("src/orders/format.ts", &at(12, &renamed))],
    );
    let out = repo.qgate(&["--base", "main", "--format", "json"]);
    let finding = only_finding(&out);
    assert!(
        only_finding_text(&out).contains("\"line\":12,\"end_line\":18"),
        "{}",
        stdout(&out)
    );
    assert_eq!(
        finding["related"],
        json!([
            {"path": "src/a.ts", "line": 1, "end_line": 7},
            {"path": "src/b.ts", "line": 20, "end_line": 26},
            {"path": "src/utils/date.ts", "line": 3, "end_line": 9},
        ]),
        "{finding}"
    );
    assert!(
        finding["message"].as_str().is_some_and(
            |m| m.starts_with("duplicates src/a.ts:1-7 ") && m.ends_with(", and 2 more)")
        ),
        "{finding}"
    );
}
