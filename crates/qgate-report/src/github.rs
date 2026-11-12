//! GitHub Actions annotations (`--format github`): one workflow command
//! per finding, which GitHub shows as an annotation of the run and on the
//! pull request's changed files (the `report-formats` spec, design D1 of
//! `add-github-action`).

use std::fmt::Write as _;

use qgate_core::{Finding, Severity};

use crate::path_text;

/// Render Error and Warning findings, in the order given, as `::error` and
/// `::warning` workflow commands; `No regressions found.` if there are
/// none. Info findings are left out, as in the Azure format.
#[must_use]
pub fn render_github(findings: &[Finding]) -> String {
    let mut out = String::new();
    for f in findings {
        let kind = match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => continue,
        };
        let _ = write!(out, "::{kind} file={}", property(&path_text(&f.location)));
        if let Some(line) = f.location.line {
            let _ = write!(out, ",line={line}");
        }
        let _ = writeln!(out, ",title={}::{}", property(&f.rule), message(&f.message));
    }
    if out.is_empty() {
        out.push_str("No regressions found.\n");
    }
    out
}

/// A property value: the message escapes, then `:` and `,`, which end a
/// property (`escapeProperty` in `@actions/core`).
fn property(value: &str) -> String {
    message(value).replace(':', "%3A").replace(',', "%2C")
}

/// The message after `::`: `%` first, so the other escapes are not escaped
/// again, then the line breaks (`escapeData` in `@actions/core`).
fn message(value: &str) -> String {
    value
        .replace('%', "%25")
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
            render_github(&[f]),
            "::error file=src/a.ts,line=1,title=complexity/cyclomatic-increase::\
             f: cyclomatic complexity 2 -> 8 (+6)\n"
        );
    }

    #[test]
    fn a_warning_without_a_line() {
        let f = finding(
            Severity::Warning,
            "architecture/forbidden-import",
            "file not analyzed",
            "src/b.ts",
            None,
        );
        assert_eq!(
            render_github(&[f]),
            "::warning file=src/b.ts,title=architecture/forbidden-import::file not analyzed\n"
        );
    }

    #[test]
    fn findings_keep_their_order_and_info_is_left_out() {
        let e = finding(Severity::Error, "r/e", "e", "a.ts", Some(1));
        let i = finding(Severity::Info, "r/i", "i", "a.ts", Some(2));
        let w = finding(Severity::Warning, "r/w", "w", "b.ts", Some(3));
        assert_eq!(
            render_github(&[e, i, w]),
            "::error file=a.ts,line=1,title=r/e::e\n\
             ::warning file=b.ts,line=3,title=r/w::w\n"
        );
    }

    #[test]
    fn nothing_to_annotate() {
        assert_eq!(render_github(&[]), "No regressions found.\n");
        let i = finding(Severity::Info, "r/i", "i", "a.ts", Some(2));
        assert_eq!(render_github(&[i]), "No regressions found.\n");
    }

    #[test]
    fn property_values_are_escaped() {
        // The spike's read-back (design.md): `src/a,b:c.ts` and `r:x,y%z`.
        let f = finding(
            Severity::Warning,
            "r:x,y%z",
            "m",
            "src/a,b:c\r\n.ts",
            Some(1),
        );
        assert_eq!(
            render_github(&[f]),
            "::warning file=src/a%2Cb%3Ac%0D%0A.ts,line=1,title=r%3Ax%2Cy%25z::m\n"
        );
    }

    #[test]
    fn the_message_is_escaped() {
        // `:` and `,` stay raw in the message; the spike read them back.
        let f = finding(
            Severity::Warning,
            "r/x",
            "100% done\nnext: one, two\r",
            "a.ts",
            Some(1),
        );
        assert_eq!(
            render_github(&[f]),
            "::warning file=a.ts,line=1,title=r/x::100%25 done%0Anext: one, two%0D\n"
        );
    }

    #[test]
    fn a_message_cannot_start_a_command() {
        let f = finding(
            Severity::Error,
            "r/x",
            "a\n::set-env name=X::y",
            "a.ts",
            Some(1),
        );
        let out = render_github(&[f]);
        assert_eq!(out.lines().count(), 1, "{out}");
        assert!(!out.contains("\n::set-env"), "{out}");
    }
}
