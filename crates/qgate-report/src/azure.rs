//! Azure Pipelines annotations (`--format azure`): one logging command per
//! finding, which the agent shows as an issue of the build (the
//! `report-formats` spec, design D3 of `add-pr-comments`).

use std::fmt::Write as _;

use qgate_core::{Finding, Severity};

use crate::path_text;

/// Render Error and Warning findings, in the order given, as
/// `##vso[task.logissue]` lines; `No regressions found.` if there are none.
/// Info findings are left out: an issue is only `error` or `warning`.
#[must_use]
pub fn render_azure(findings: &[Finding]) -> String {
    let mut out = String::new();
    for f in findings {
        let kind = match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => continue,
        };
        let _ = write!(
            out,
            "##vso[task.logissue type={kind};sourcepath={}",
            property(&path_text(&f.location))
        );
        if let Some(line) = f.location.line {
            let _ = write!(out, ";linenumber={line}");
        }
        let _ = writeln!(out, ";code={}]{}", property(&f.rule), message(&f.message));
    }
    if out.is_empty() {
        out.push_str("No regressions found.\n");
    }
    out
}

/// A property value: `%` first, so the other escapes are not escaped again.
fn property(value: &str) -> String {
    message(value).replace(';', "%3B").replace(']', "%5D")
}

/// The message after `]`: only `%` and line breaks need escaping there.
fn message(value: &str) -> String {
    value
        .replace('%', "%AZP25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

#[cfg(test)]
mod tests {
    use super::*;
    use qgate_core::{Location, Severity};

    fn finding(
        severity: Severity,
        rule: &str,
        message: &str,
        path: &str,
        line: Option<u32>,
    ) -> Finding {
        Finding {
            rule: rule.into(),
            severity,
            message: message.into(),
            location: Location {
                path: path.into(),
                line,
                end_line: None,
            },
            metric: None,
            related: Vec::new(),
        }
    }

    #[test]
    fn one_error() {
        let f = finding(
            Severity::Error,
            "complexity/cyclomatic-increase",
            "f: cyclomatic complexity 2 -> 8 (+6)",
            "src/a.ts",
            Some(1),
        );
        assert_eq!(
            render_azure(&[f]),
            "##vso[task.logissue type=error;sourcepath=src/a.ts;linenumber=1;\
             code=complexity/cyclomatic-increase]f: cyclomatic complexity 2 -> 8 (+6)\n"
        );
    }

    #[test]
    fn a_warning_without_a_line() {
        let f = finding(
            Severity::Warning,
            "input/unparsable",
            "file not analyzed",
            "src/a.ts",
            None,
        );
        assert_eq!(
            render_azure(&[f]),
            "##vso[task.logissue type=warning;sourcepath=src/a.ts;code=input/unparsable]file not analyzed\n"
        );
    }

    #[test]
    fn findings_keep_their_order_and_info_is_left_out() {
        let e = finding(Severity::Error, "r/e", "e", "a.ts", Some(1));
        let i = finding(Severity::Info, "r/i", "i", "a.ts", Some(2));
        let w = finding(Severity::Warning, "r/w", "w", "b.ts", Some(3));
        assert_eq!(
            render_azure(&[e, i, w]),
            "##vso[task.logissue type=error;sourcepath=a.ts;linenumber=1;code=r/e]e\n\
             ##vso[task.logissue type=warning;sourcepath=b.ts;linenumber=3;code=r/w]w\n"
        );
    }

    #[test]
    fn nothing_to_annotate() {
        assert_eq!(render_azure(&[]), "No regressions found.\n");
        let i = finding(Severity::Info, "r/i", "i", "a.ts", Some(2));
        assert_eq!(render_azure(&[i]), "No regressions found.\n");
    }

    #[test]
    fn property_values_are_escaped() {
        let f = finding(
            Severity::Warning,
            "r/a;b]c%d",
            "m",
            "src/a;b]\r\n%.ts",
            Some(1),
        );
        assert_eq!(
            render_azure(&[f]),
            "##vso[task.logissue type=warning;sourcepath=src/a%3Bb%5D%0D%0A%AZP25.ts;\
             linenumber=1;code=r/a%3Bb%5Dc%AZP25d]m\n"
        );
    }

    #[test]
    fn the_message_is_escaped() {
        let f = finding(
            Severity::Warning,
            "r/x",
            "100% done\nnext\r] and ; stay",
            "a.ts",
            Some(1),
        );
        assert_eq!(
            render_azure(&[f]),
            "##vso[task.logissue type=warning;sourcepath=a.ts;linenumber=1;code=r/x]\
             100%AZP25 done%0Anext%0D] and ; stay\n"
        );
    }

    #[test]
    fn every_line_ends_with_one_newline() {
        let f = finding(Severity::Error, "r/x", "a\nb", "a.ts", Some(1));
        let out = render_azure(&[f.clone(), f]);
        assert_eq!(out.matches('\n').count(), 2, "{out}");
    }
}
