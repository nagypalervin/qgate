//! Reporters turn findings into output.
//!
//! Only plain-text terminal output exists so far. JSON, SARIF and Markdown
//! (for pull request comments) follow in M3.

use std::fmt::Write as _;

use qgate_core::Finding;

/// Render findings as plain text, one line per finding.
#[must_use]
pub fn render_text(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "No regressions found.\n".to_owned();
    }

    let mut out = String::new();
    for f in findings {
        let loc = match f.location.line {
            Some(line) => format!("{}:{line}", f.location.path.display()),
            None => f.location.path.display().to_string(),
        };
        let _ = writeln!(out, "{:<7} {loc}  {}  [{}]", f.severity, f.message, f.rule);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use qgate_core::{Location, Severity};

    #[test]
    fn empty_report() {
        assert_eq!(render_text(&[]), "No regressions found.\n");
    }

    #[test]
    fn one_finding() {
        let f = Finding {
            rule: "complexity/increase".into(),
            severity: Severity::Error,
            message: "complexity 8 -> 23".into(),
            location: Location {
                path: "src/orders.ts".into(),
                line: Some(42),
            },
            metric: None,
        };
        assert_eq!(
            render_text(&[f]),
            "error   src/orders.ts:42  complexity 8 -> 23  [complexity/increase]\n"
        );
    }
}
