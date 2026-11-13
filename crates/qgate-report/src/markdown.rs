//! The Markdown report (`--format markdown`): the body of a pull request
//! comment (the `report-formats` spec, design D2 of `add-pr-comments`).

use std::fmt::Write as _;

use qgate_core::{Finding, Location};

use crate::{ReportSummary, path_text, severity_name};

/// The most findings the table shows; the rest are counted below it.
pub const MAX_ROWS: usize = 50;

/// Render findings, in the order given, as the Markdown report: a marker
/// line with the counts, a summary sentence and a table of at most
/// [`MAX_ROWS`] findings.
#[must_use]
pub fn render_markdown(findings: &[Finding], summary: ReportSummary<'_>) -> String {
    let count = findings.len();
    // `fail_on` is one of qgate's level names, so it needs no escaping.
    let mut out = format!(
        "<!-- qgate count={count} failing={} fail_on={} -->\n\n### qgate\n\n",
        summary.failing, summary.fail_on
    );
    if count == 0 {
        out.push_str("No regressions found.\n");
        return out;
    }

    let at_level = match summary.failing {
        0 => "none".to_owned(),
        n => n.to_string(),
    };
    let _ = writeln!(
        out,
        "{count} {}, {at_level} at or above `{}`.",
        findings_word(count),
        summary.fail_on
    );
    out.push_str("\n| Severity | Location | Finding | Rule |\n|---|---|---|---|\n");
    for f in findings.iter().take(MAX_ROWS) {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            severity_name(f.severity),
            cell(&location_text(&f.location)),
            cell(&finding_text(f)),
            cell(&f.rule)
        );
    }
    let hidden = count.saturating_sub(MAX_ROWS);
    if hidden > 0 {
        let _ = writeln!(out, "\n{hidden} more {} not shown.", findings_word(hidden));
    }
    out
}

fn findings_word(n: usize) -> &'static str {
    if n == 1 { "finding" } else { "findings" }
}

/// `<path>:<line>`, or the path alone for a finding without a line.
fn location_text(location: &Location) -> String {
    match location.line {
        Some(line) => format!("{}:{line}", path_text(location)),
        None => path_text(location),
    }
}

/// The message, followed by every related location the message does not
/// already name (the path alone for one without a line).
fn finding_text(f: &Finding) -> String {
    let related: Vec<String> = f
        .related
        .iter()
        .filter_map(|location| {
            let text = location_text(location);
            (location.line.is_none() || !names(&f.message, &text)).then_some(text)
        })
        .collect();
    if related.is_empty() {
        f.message.clone()
    } else {
        format!("{}; also at {}", f.message, related.join(", "))
    }
}

/// Whether `message` names the location `text` (`<path>:<line>`): the
/// text starts the message or follows whitespace, and no digit follows it,
/// so `src/a.ts:1` is not named by `src/a.ts:10-19` or `lib/src/a.ts:1`
/// (design D1 of `fix-pr-comment-details`).
fn names(message: &str, text: &str) -> bool {
    message.char_indices().any(|(i, _)| {
        message[i..].starts_with(text)
            && message[..i]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
            && !message[i + text.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
    })
}

/// Text that renders as itself in a GFM table cell: the characters that
/// can start inline markup, an entity, HTML or a cell boundary get a
/// backslash, and a line break becomes a space (design D2).
fn cell(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' | '~' | '&' => {
                out.push('\\');
                out.push(c);
            }
            '\r' => {
                // CRLF is one line break.
                chars.next_if_eq(&'\n');
                out.push(' ');
            }
            '\n' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use qgate_core::{Location, Severity};

    const DEFAULT: ReportSummary<'static> = ReportSummary {
        failing: 0,
        fail_on: "error",
    };

    fn at(path: &str, line: Option<u32>) -> Location {
        Location {
            path: path.into(),
            line,
            end_line: None,
        }
    }

    fn finding(severity: Severity, rule: &str, message: &str, location: Location) -> Finding {
        Finding {
            rule: rule.into(),
            severity,
            message: message.into(),
            location,
            metric: None,
            related: Vec::new(),
        }
    }

    fn summary(failing: usize, fail_on: &str) -> ReportSummary<'_> {
        ReportSummary { failing, fail_on }
    }

    /// The table rows of a report.
    fn rows(md: &str) -> Vec<&str> {
        md.lines()
            .filter(|l| l.starts_with("| ") && !l.starts_with("| Severity "))
            .collect()
    }

    #[test]
    fn no_findings() {
        assert_eq!(
            render_markdown(&[], DEFAULT),
            "<!-- qgate count=0 failing=0 fail_on=error -->\n\n### qgate\n\nNo regressions found.\n"
        );
    }

    #[test]
    fn one_error() {
        let f = finding(
            Severity::Error,
            "complexity/cyclomatic-increase",
            "f: cyclomatic complexity 2 -> 8 (+6)",
            at("src/a.ts", Some(1)),
        );
        assert_eq!(
            render_markdown(&[f], summary(1, "error")),
            "<!-- qgate count=1 failing=1 fail_on=error -->\n\
             \n\
             ### qgate\n\
             \n\
             1 finding, 1 at or above `error`.\n\
             \n\
             | Severity | Location | Finding | Rule |\n\
             |---|---|---|---|\n\
             | error | src/a.ts:1 | f: cyclomatic complexity 2 -\\> 8 (+6) | complexity/cyclomatic-increase |\n"
        );
    }

    #[test]
    fn summary_sentence_none_at_the_level() {
        let w = finding(Severity::Warning, "r/x", "m", at("a.ts", Some(1)));
        let md = render_markdown(&[w.clone(), w], DEFAULT);
        assert!(
            md.contains("\n2 findings, none at or above `error`.\n"),
            "{md}"
        );
        assert!(md.starts_with("<!-- qgate count=2 failing=0 fail_on=error -->\n"));
    }

    #[test]
    fn summary_sentence_all_at_the_level() {
        let e = finding(Severity::Error, "r/x", "m", at("a.ts", Some(1)));
        let w = finding(Severity::Warning, "r/y", "m", at("a.ts", Some(2)));
        let md = render_markdown(&[e, w], summary(2, "warning"));
        assert!(
            md.contains("\n2 findings, 2 at or above `warning`.\n"),
            "{md}"
        );
    }

    #[test]
    fn marker_line_counts() {
        let e = finding(Severity::Error, "r/x", "m", at("a.ts", Some(1)));
        let w = finding(Severity::Warning, "r/y", "m", at("a.ts", Some(2)));
        let md = render_markdown(&[e, w], summary(1, "error"));
        assert_eq!(
            md.lines().next(),
            Some("<!-- qgate count=2 failing=1 fail_on=error -->")
        );
        let never = render_markdown(&[], summary(0, "never"));
        assert!(never.starts_with("<!-- qgate count=0 failing=0 fail_on=never -->\n"));
    }

    #[test]
    fn every_special_character_is_escaped() {
        for c in ['\\', '`', '*', '_', '[', ']', '<', '>', '|', '~', '&'] {
            let f = finding(
                Severity::Warning,
                &format!("r/{c}"),
                &format!("a{c}b"),
                at(&format!("p{c}.ts"), Some(1)),
            );
            let md = render_markdown(&[f], DEFAULT);
            assert_eq!(
                rows(&md),
                [format!("| warning | p\\{c}.ts:1 | a\\{c}b | r/\\{c} |").as_str()],
                "{c:?}"
            );
        }
    }

    #[test]
    fn table_and_html_characters_keep_four_cells() {
        let f = finding(
            Severity::Warning,
            "r/x",
            "a | b <script> *c*",
            at("a.ts", Some(1)),
        );
        let md = render_markdown(&[f], DEFAULT);
        assert_eq!(
            rows(&md),
            ["| warning | a.ts:1 | a \\| b \\<script\\> \\*c\\* | r/x |"]
        );
    }

    #[test]
    fn line_breaks_become_spaces() {
        let f = finding(
            Severity::Warning,
            "r/x",
            "one\ntwo\rthree\r\nfour",
            at("a.ts", Some(1)),
        );
        let md = render_markdown(&[f], DEFAULT);
        assert_eq!(
            rows(&md),
            ["| warning | a.ts:1 | one two three four | r/x |"]
        );
    }

    #[test]
    fn finding_without_a_line() {
        let f = finding(
            Severity::Warning,
            "input/unparsable",
            "file not analyzed",
            at("src/a.ts", None),
        );
        let md = render_markdown(&[f], DEFAULT);
        assert_eq!(
            rows(&md),
            ["| warning | src/a.ts | file not analyzed | input/unparsable |"]
        );
    }

    #[test]
    fn related_locations_follow_the_message() {
        let mut f = finding(
            Severity::Warning,
            "duplication/duplicate-block",
            "12 duplicated lines",
            at("src/b.ts", Some(4)),
        );
        f.related = vec![
            Location {
                path: "src/a.ts".into(),
                line: Some(10),
                end_line: Some(21),
            },
            at("src/c.ts", Some(3)),
            at("src/no-line.ts", None),
        ];
        let md = render_markdown(&[f], DEFAULT);
        assert_eq!(
            rows(&md),
            [
                "| warning | src/b.ts:4 | 12 duplicated lines; also at src/a.ts:10, src/c.ts:3, src/no-line.ts | duplication/duplicate-block |"
            ]
        );
    }

    /// The Finding cell of a one-finding report.
    fn finding_cell(message: &str, related: Vec<Location>) -> String {
        let mut f = finding(
            Severity::Warning,
            "duplication/duplicate-block",
            message,
            at("src/b.ts", Some(4)),
        );
        f.related = related;
        let md = render_markdown(&[f], DEFAULT);
        let row = rows(&md)[0];
        row.split(" | ").nth(2).expect("four cells").to_owned()
    }

    #[test]
    fn a_copy_the_message_names_is_not_repeated() {
        assert_eq!(
            finding_cell(
                "duplicates src/grade.ts:1-19 (lines 1-19, 85 tokens)",
                vec![at("src/grade.ts", Some(1))],
            ),
            "duplicates src/grade.ts:1-19 (lines 1-19, 85 tokens)"
        );
    }

    #[test]
    fn the_copies_the_message_does_not_name_are_listed() {
        assert_eq!(
            finding_cell(
                "duplicates src/a.ts:1-6 (lines 4-9, 68 tokens, and 2 more)",
                vec![
                    at("src/a.ts", Some(1)),
                    at("src/c.ts", Some(3)),
                    at("src/d.ts", Some(7)),
                ],
            ),
            "duplicates src/a.ts:1-6 (lines 4-9, 68 tokens, and 2 more); also at src/c.ts:3, src/d.ts:7"
        );
    }

    #[test]
    fn a_line_that_only_starts_the_named_line_is_listed() {
        assert_eq!(
            finding_cell(
                "duplicates src/a.ts:10-19",
                vec![at("src/a.ts", Some(1)), at("src/a.ts", Some(10))],
            ),
            "duplicates src/a.ts:10-19; also at src/a.ts:1"
        );
    }

    #[test]
    fn a_location_at_the_end_of_the_message_is_named() {
        assert_eq!(
            finding_cell("moved from src/a.ts:12", vec![at("src/a.ts", Some(12))]),
            "moved from src/a.ts:12"
        );
    }

    #[test]
    fn a_path_that_only_ends_the_named_path_is_listed() {
        assert_eq!(
            finding_cell(
                "duplicates lib/src/a.ts:1-6",
                vec![at("lib/src/a.ts", Some(1)), at("src/a.ts", Some(1))],
            ),
            "duplicates lib/src/a.ts:1-6; also at src/a.ts:1"
        );
    }

    #[test]
    fn a_location_without_a_line_is_always_listed() {
        assert_eq!(
            finding_cell("duplicates src/n.ts", vec![at("src/n.ts", None)]),
            "duplicates src/n.ts; also at src/n.ts"
        );
    }

    #[test]
    fn sixty_findings_show_fifty_rows() {
        let findings: Vec<Finding> = (1..=60)
            .map(|i| finding(Severity::Warning, "r/x", "m", at("a.ts", Some(i))))
            .collect();
        let md = render_markdown(&findings, DEFAULT);
        let shown = rows(&md);
        assert_eq!(shown.len(), MAX_ROWS);
        assert_eq!(shown[0], "| warning | a.ts:1 | m | r/x |");
        assert_eq!(shown[49], "| warning | a.ts:50 | m | r/x |");
        assert!(md.ends_with("|\n\n10 more findings not shown.\n"), "{md}");
        assert!(md.starts_with("<!-- qgate count=60 failing=0 fail_on=error -->\n"));
        assert!(md.contains("\n60 findings, none at or above `error`.\n"));
    }

    #[test]
    fn one_finding_not_shown_is_singular() {
        let findings: Vec<Finding> = (1..=51)
            .map(|i| finding(Severity::Warning, "r/x", "m", at("a.ts", Some(i))))
            .collect();
        assert!(render_markdown(&findings, DEFAULT).ends_with("\n\n1 more finding not shown.\n"));
    }

    #[test]
    fn fifty_findings_have_no_more_line() {
        let findings: Vec<Finding> = (1..=50)
            .map(|i| finding(Severity::Warning, "r/x", "m", at("a.ts", Some(i))))
            .collect();
        let md = render_markdown(&findings, DEFAULT);
        assert!(!md.contains("not shown"), "{md}");
        assert!(md.ends_with("| warning | a.ts:50 | m | r/x |\n"));
    }

    #[test]
    fn same_input_same_output() {
        let f = finding(Severity::Error, "r/x", "m", at("a.ts", Some(1)));
        assert_eq!(
            render_markdown(std::slice::from_ref(&f), summary(1, "error")),
            render_markdown(&[f], summary(1, "error"))
        );
    }
}
