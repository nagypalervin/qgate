//! `qgate hook claude-code`: Claude Code's hook protocol around a check of
//! the working tree (the `agent-hooks` spec, design D1 of
//! `add-agent-hooks`).
//!
//! Everything here is pure: `main` reads standard input, runs the check
//! and writes the [`Response`]. Every outcome is a response with exit code
//! 0, so nothing but a finding at or above the level reaches the model.

use std::fmt::Write as _;
use std::path::PathBuf;

use qgate_core::Finding;
use qgate_report::render_text;
use serde::{Deserialize, Serialize};

use crate::FailOn;

/// The fields of Claude Code's hook input that the hook reads. Others are
/// ignored, because Claude Code adds fields over time.
#[derive(Debug, Default, Deserialize)]
pub struct Input {
    hook_event_name: Option<String>,
    stop_hook_active: Option<bool>,
    cwd: Option<PathBuf>,
}

impl Input {
    /// Whether Claude is already continuing because a stop hook blocked:
    /// the round of fixing has been used.
    pub fn stop_hook_active(&self) -> bool {
        self.stop_hook_active == Some(true)
    }

    /// The directory the agent is in, if the input names one.
    pub fn cwd(&self) -> Option<&PathBuf> {
        self.cwd.as_ref()
    }
}

/// Parse the hook input: a JSON object for the `Stop` event. The error is
/// a message for the user.
pub fn parse_input(text: &str) -> Result<Input, String> {
    let invalid = |why: &dyn std::fmt::Display| format!("the input is not valid hook input: {why}");
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| invalid(&e))?;
    if !value.is_object() {
        return Err(invalid(&"expected a JSON object"));
    }
    let input: Input = serde_json::from_value(value).map_err(|e| invalid(&e))?;
    match input.hook_event_name.as_deref() {
        Some(STOP) => Ok(input),
        Some(other) => Err(format!(
            "hook claude-code supports only the Stop event, not {other}"
        )),
        None => Err(invalid(&"no hook_event_name")),
    }
}

/// The hook event this command answers.
const STOP: &str = "Stop";

/// The most findings a list shows (design D5).
const MAX_LISTED: usize = 20;

/// What the hook writes: whether to block the stop, the reason the model
/// gets, and a message the user sees. Absent fields are left out.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    system_message: Option<String>,
}

impl Response {
    /// A message for the user that does not block the stop.
    pub fn error(message: &str) -> Self {
        Self {
            system_message: Some(format!("qgate: {message}")),
            ..Self::default()
        }
    }

    /// Standard output: nothing when there is nothing to say, else one
    /// JSON object on one line.
    pub fn output(&self) -> String {
        if *self == Self::default() {
            return String::new();
        }
        // Strings and options only: serializing cannot fail.
        let mut out = serde_json::to_string(self).unwrap_or_default();
        out.push('\n');
        out
    }
}

/// The response to a check of the working tree against `base`: the
/// sorted findings, or the error message `qgate check` would print.
pub fn respond(
    result: Result<Vec<Finding>, String>,
    stop_hook_active: bool,
    base: &str,
    fail_on: FailOn,
) -> Response {
    let findings = match result {
        Ok(findings) => findings,
        Err(message) => return Response::error(&message),
    };
    if findings.is_empty() {
        return Response::default();
    }
    if stop_hook_active {
        return Response {
            system_message: Some(format!(
                "qgate: these findings remain after one round of fixing:\n{}",
                list(&findings, base)
            )),
            ..Response::default()
        };
    }
    let (failing, below): (Vec<Finding>, Vec<Finding>) = findings
        .into_iter()
        .partition(|f| fail_on.threshold().is_some_and(|t| f.severity >= t));
    let mut response = Response::default();
    if !failing.is_empty() {
        response.decision = Some("block");
        response.reason = Some(format!(
            "qgate: your changes made code worse (compared with {base}):\n{}\n\
             Fix these, or if a change is intended, tell the user why and stop.",
            list(&failing, base)
        ));
    }
    if !below.is_empty() {
        response.system_message = Some(format!(
            "qgate: below the --fail-on level \"{}\":\n{}",
            fail_on.name(),
            list(&below, base)
        ));
    }
    response
}

/// The text format's lines for at most [`MAX_LISTED`] findings, then a
/// line that says how many more there are.
fn list(findings: &[Finding], base: &str) -> String {
    let shown = &findings[..findings.len().min(MAX_LISTED)];
    let mut text = render_text(shown).trim_end_matches('\n').to_owned();
    let more = findings.len() - shown.len();
    if more > 0 {
        let _ = write!(
            text,
            "\n... and {more} more; run qgate check --base {base} --worktree to see all"
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use qgate_core::{Location, Severity};

    fn finding(path: &str, severity: Severity, message: &str) -> Finding {
        Finding {
            rule: "complexity/cyclomatic-increase".into(),
            severity,
            message: message.into(),
            location: Location {
                path: path.into(),
                line: Some(1),
                end_line: None,
            },
            metric: None,
            related: Vec::new(),
        }
    }

    fn error_f() -> Finding {
        finding(
            "src/a.ts",
            Severity::Error,
            "f: cyclomatic complexity 2 -> 9 (+7)",
        )
    }

    fn warning_g() -> Finding {
        finding(
            "src/b.ts",
            Severity::Warning,
            "g: cyclomatic complexity 2 -> 4 (+2)",
        )
    }

    const F_LINE: &str = "error   src/a.ts:1  f: cyclomatic complexity 2 -> 9 (+7)  \
                          [complexity/cyclomatic-increase]";
    const G_LINE: &str = "warning src/b.ts:1  g: cyclomatic complexity 2 -> 4 (+2)  \
                          [complexity/cyclomatic-increase]";

    fn reason(lines: &str) -> String {
        format!(
            "qgate: your changes made code worse (compared with HEAD):\n{lines}\n\
             Fix these, or if a change is intended, tell the user why and stop."
        )
    }

    fn first_round(findings: Vec<Finding>, fail_on: FailOn) -> Response {
        respond(Ok(findings), false, "HEAD", fail_on)
    }

    #[test]
    fn nothing_to_report() {
        let response = first_round(Vec::new(), FailOn::Error);
        assert_eq!(response, Response::default());
        assert_eq!(response.output(), "");
    }

    #[test]
    fn a_failing_finding_blocks() {
        let response = first_round(vec![error_f()], FailOn::Error);
        assert_eq!(
            response,
            Response {
                decision: Some("block"),
                reason: Some(reason(F_LINE)),
                system_message: None,
            }
        );
    }

    #[test]
    fn the_reason_names_the_base() {
        let response = respond(Ok(vec![error_f()]), false, "main", FailOn::Error);
        let reason = response.reason.expect("a reason");
        assert!(
            reason.starts_with("qgate: your changes made code worse (compared with main):\n"),
            "{reason}"
        );
    }

    #[test]
    fn no_block_after_the_round() {
        let response = respond(
            Ok(vec![error_f(), warning_g()]),
            true,
            "HEAD",
            FailOn::Error,
        );
        assert_eq!(
            response,
            Response {
                decision: None,
                reason: None,
                system_message: Some(format!(
                    "qgate: these findings remain after one round of fixing:\n{F_LINE}\n{G_LINE}"
                )),
            }
        );
    }

    #[test]
    fn fixed_in_the_round() {
        assert_eq!(
            respond(Ok(Vec::new()), true, "HEAD", FailOn::Error),
            Response::default()
        );
    }

    #[test]
    fn findings_below_the_level_go_to_the_user() {
        let response = first_round(vec![warning_g()], FailOn::Error);
        assert_eq!(
            response,
            Response {
                decision: None,
                reason: None,
                system_message: Some(format!(
                    "qgate: below the --fail-on level \"error\":\n{G_LINE}"
                )),
            }
        );
    }

    #[test]
    fn a_block_and_a_message_together() {
        let response = first_round(vec![error_f(), warning_g()], FailOn::Error);
        assert_eq!(
            response,
            Response {
                decision: Some("block"),
                reason: Some(reason(F_LINE)),
                system_message: Some(format!(
                    "qgate: below the --fail-on level \"error\":\n{G_LINE}"
                )),
            }
        );
    }

    #[test]
    fn failing_on_warnings() {
        let response = first_round(vec![error_f(), warning_g()], FailOn::Warning);
        assert_eq!(response.decision, Some("block"));
        assert_eq!(
            response.reason,
            Some(reason(&format!("{F_LINE}\n{G_LINE}")))
        );
        assert_eq!(response.system_message, None);
    }

    #[test]
    fn never_failing_never_blocks() {
        let response = first_round(vec![error_f()], FailOn::Never);
        assert_eq!(
            response,
            Response {
                decision: None,
                reason: None,
                system_message: Some(format!(
                    "qgate: below the --fail-on level \"never\":\n{F_LINE}"
                )),
            }
        );
    }

    /// `n` Error findings, one per file, in path order.
    fn many(n: usize) -> Vec<Finding> {
        (0..n)
            .map(|i| finding(&format!("src/f{i:02}.ts"), Severity::Error, "m"))
            .collect()
    }

    #[test]
    fn long_lists_stop_at_twenty() {
        for (n, more) in [(21, 1), (25, 5)] {
            let reason = first_round(many(n), FailOn::Error)
                .reason
                .expect("a reason");
            let lines: Vec<&str> = reason.lines().collect();
            // The heading, 20 findings, the "more" line and the request.
            assert_eq!(lines.len(), 23, "{reason}");
            assert!(lines[20].starts_with("error   src/f19.ts:1  "), "{reason}");
            assert_eq!(
                lines[21],
                format!("... and {more} more; run qgate check --base HEAD --worktree to see all")
            );
        }
        let twenty = first_round(many(20), FailOn::Error)
            .reason
            .expect("a reason");
        assert!(!twenty.contains("more;"), "{twenty}");
        let remaining = respond(Ok(many(25)), true, "main", FailOn::Error)
            .system_message
            .expect("a message");
        assert!(
            remaining
                .ends_with("\n... and 5 more; run qgate check --base main --worktree to see all"),
            "{remaining}"
        );
    }

    #[test]
    fn a_check_error_goes_to_the_user() {
        for active in [false, true] {
            let response = respond(
                Err("qgate.toml: line 3: invalid".into()),
                active,
                "HEAD",
                FailOn::Error,
            );
            assert_eq!(
                response,
                Response {
                    decision: None,
                    reason: None,
                    system_message: Some("qgate: qgate.toml: line 3: invalid".into()),
                }
            );
        }
    }

    #[test]
    fn stop_input() {
        let input = parse_input(
            r#"{"session_id":"s","hook_event_name":"Stop","stop_hook_active":true,"cwd":"/w"}"#,
        )
        .expect("valid input");
        assert!(input.stop_hook_active());
        assert_eq!(input.cwd(), Some(&PathBuf::from("/w")));
        let first = parse_input(r#"{"hook_event_name":"Stop"}"#).expect("valid input");
        assert!(!first.stop_hook_active());
        assert_eq!(first.cwd(), None);
    }

    #[test]
    fn input_that_is_not_hook_input() {
        for text in [
            "not json",
            "[]",
            "\"Stop\"",
            "{}",
            r#"{"hook_event_name":7}"#,
            "",
        ] {
            let message = parse_input(text).expect_err(text);
            assert!(
                message.contains("not valid hook input"),
                "{text}: {message}"
            );
        }
    }

    #[test]
    fn another_hook_event() {
        let message =
            parse_input(r#"{"hook_event_name":"PostToolUse"}"#).expect_err("not a Stop event");
        assert!(message.contains("only the Stop event"), "{message}");
        assert!(message.contains("PostToolUse"), "{message}");
    }

    #[test]
    fn output_is_one_line_without_null() {
        let response = first_round(vec![error_f(), warning_g()], FailOn::Error);
        let output = response.output();
        assert!(output.ends_with("}\n"), "{output}");
        assert_eq!(output.matches('\n').count(), 1, "{output}");
        assert!(!output.contains("null"), "{output}");
        assert!(
            output.starts_with("{\"decision\":\"block\",\"reason\":\"qgate: your changes"),
            "{output}"
        );
        let message_only = Response::error("x").output();
        assert_eq!(message_only, "{\"systemMessage\":\"qgate: x\"}\n");
    }
}
