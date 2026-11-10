//! Reporters turn findings into output.
//!
//! Two formats exist: plain text for people ([`render_text`]) and the
//! versioned JSON report for programs ([`render_json`]). Markdown (for pull
//! request comments) and SARIF follow in M4.

use std::fmt::Write as _;

use qgate_core::{Finding, Location, Metric, Severity};
use serde::{Serialize, Serializer};

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

/// What the JSON report says about the run as a whole, beyond the
/// findings. The caller decides which findings fail the run.
#[derive(Debug, Clone, Copy)]
pub struct ReportSummary<'a> {
    /// How many findings are at or above the `fail_on` level.
    pub failing: usize,
    /// The level that fails the run, as the user wrote it.
    pub fail_on: &'a str,
}

/// Render findings, in the order given, as the JSON report, version 1:
/// one compact object on a single line, followed by a newline (see the
/// `report-formats` spec and `docs/schema/report-v1.json`).
///
/// # Panics
///
/// Never in practice: the report holds only strings, integers, finite
/// floats, arrays and structs, which `serde_json` always serializes.
#[must_use]
pub fn render_json(findings: &[Finding], summary: ReportSummary<'_>) -> String {
    let report = JsonReport {
        schema_version: 1,
        findings: findings.iter().map(JsonFinding::from).collect(),
        summary: JsonSummary {
            count: findings.len(),
            failing: summary.failing,
            fail_on: summary.fail_on,
        },
    };
    let mut out = serde_json::to_string(&report).expect("the JSON report always serializes");
    out.push('\n');
    out
}

// The JSON report's shape. Field names and their order are the public
// format (design D3 of `add-report-formats`), so they live here, apart
// from the core types, which may change freely.

/// The envelope.
#[derive(Serialize)]
struct JsonReport<'a> {
    schema_version: u32,
    findings: Vec<JsonFinding<'a>>,
    summary: JsonSummary<'a>,
}

/// One finding. Absent values are left out, never written as `null`.
#[derive(Serialize)]
struct JsonFinding<'a> {
    rule: &'a str,
    severity: &'static str,
    message: &'a str,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metric: Option<JsonMetric>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related: Vec<JsonLocation>,
}

impl<'a> From<&'a Finding> for JsonFinding<'a> {
    fn from(f: &'a Finding) -> Self {
        Self {
            rule: &f.rule,
            severity: severity_name(f.severity),
            message: &f.message,
            path: path_text(&f.location),
            line: f.location.line,
            // A range needs its start.
            end_line: f.location.line.and(f.location.end_line),
            metric: f.metric.as_ref().and_then(JsonMetric::new),
            related: f.related.iter().filter_map(JsonLocation::new).collect(),
        }
    }
}

/// A related location. It always has a line, as the schema requires.
#[derive(Serialize)]
struct JsonLocation {
    path: String,
    line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
}

impl JsonLocation {
    /// The location as JSON, or `None` if it has no line. The core type
    /// promises one; leaving out a location that breaks that promise keeps
    /// the report valid against the published schema.
    fn new(location: &Location) -> Option<Self> {
        Some(Self {
            path: path_text(location),
            line: location.line?,
            end_line: location.end_line,
        })
    }
}

/// The measured values. An absent `before` means a new item, an absent
/// `after` a removed one; nothing else (design D4).
#[derive(Serialize)]
struct JsonMetric {
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<JsonNumber>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<JsonNumber>,
}

impl JsonMetric {
    /// The metric as JSON, or `None` if it has no value or a value that is
    /// not finite: the whole metric is left out then, so an absent value
    /// keeps its one meaning.
    fn new(metric: &Metric) -> Option<Self> {
        let finite = |v: Option<f64>| v.is_none_or(f64::is_finite);
        if (metric.before.is_none() && metric.after.is_none())
            || !finite(metric.before)
            || !finite(metric.after)
        {
            return None;
        }
        Some(Self {
            before: metric.before.map(JsonNumber),
            after: metric.after.map(JsonNumber),
        })
    }
}

/// A finite number, written as an integer when it is whole and exactly
/// representable (below 2^53 in magnitude), else as a float.
struct JsonNumber(f64);

/// 2^53: from here on, not every integer is an `f64`.
const EXACT_INTEGER_LIMIT: f64 = 9_007_199_254_740_992.0;

impl Serialize for JsonNumber {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        if v.fract() == 0.0 && v.abs() < EXACT_INTEGER_LIMIT {
            // Exact: a whole number below 2^53 in magnitude fits an i64.
            #[allow(clippy::cast_possible_truncation)]
            serializer.serialize_i64(v as i64)
        } else {
            serializer.serialize_f64(v)
        }
    }
}

/// The summary.
#[derive(Serialize)]
struct JsonSummary<'a> {
    count: usize,
    failing: usize,
    fail_on: &'a str,
}

/// A severity as the JSON report writes it. Spelled out here, not taken
/// from `Display`, so the public values cannot change by accident.
fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// A path as the text format shows it; invalid UTF-8 becomes U+FFFD
/// (design D7).
fn path_text(location: &Location) -> String {
    location.path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use qgate_core::{Location, Metric, Severity};
    use std::path::PathBuf;

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
                end_line: None,
            },
            metric: None,
            related: Vec::new(),
        };
        assert_eq!(
            render_text(&[f]),
            "error   src/orders.ts:42  complexity 8 -> 23  [complexity/increase]\n"
        );
    }

    const DEFAULT: ReportSummary<'static> = ReportSummary {
        failing: 0,
        fail_on: "error",
    };

    fn at(path: impl Into<PathBuf>, line: Option<u32>, end_line: Option<u32>) -> Location {
        Location {
            path: path.into(),
            line,
            end_line,
        }
    }

    fn finding(rule: &str, message: &str, location: Location, metric: Option<Metric>) -> Finding {
        Finding {
            rule: rule.into(),
            severity: Severity::Warning,
            message: message.into(),
            location,
            metric,
            related: Vec::new(),
        }
    }

    fn metric(before: Option<f64>, after: Option<f64>) -> Metric {
        Metric { before, after }
    }

    /// The JSON of the only finding in a report.
    fn only(json: &str) -> &str {
        let start = json.find("\"findings\":[").expect("a findings array") + 12;
        let end = json
            .find("],\"summary\"")
            .expect("a summary after the findings");
        &json[start..end]
    }

    #[test]
    fn json_one_finding() {
        let f = finding(
            "complexity/cyclomatic-increase",
            "f: cyclomatic complexity 2 -> 4 (+2)",
            at("src/a.ts", Some(1), None),
            Some(metric(Some(2.0), Some(4.0))),
        );
        assert_eq!(
            render_json(&[f], DEFAULT),
            "{\"schema_version\":1,\"findings\":[{\"rule\":\"complexity/cyclomatic-increase\",\
             \"severity\":\"warning\",\"message\":\"f: cyclomatic complexity 2 -> 4 (+2)\",\
             \"path\":\"src/a.ts\",\"line\":1,\"metric\":{\"before\":2,\"after\":4}}],\
             \"summary\":{\"count\":1,\"failing\":0,\"fail_on\":\"error\"}}\n"
        );
    }

    #[test]
    fn json_no_findings() {
        assert_eq!(
            render_json(&[], DEFAULT),
            "{\"schema_version\":1,\"findings\":[],\
             \"summary\":{\"count\":0,\"failing\":0,\"fail_on\":\"error\"}}\n"
        );
    }

    #[test]
    fn json_new_function() {
        let f = finding(
            "complexity/cyclomatic-new",
            "big: new function, cyclomatic complexity 12",
            at("src/new.ts", Some(1), None),
            Some(metric(None, Some(12.0))),
        );
        assert_eq!(
            only(&render_json(&[f], DEFAULT)),
            "{\"rule\":\"complexity/cyclomatic-new\",\"severity\":\"warning\",\
             \"message\":\"big: new function, cyclomatic complexity 12\",\
             \"path\":\"src/new.ts\",\"line\":1,\"metric\":{\"after\":12}}"
        );
    }

    #[test]
    fn json_removed_item_has_no_after() {
        let f = finding(
            "r/x",
            "m",
            at("a.ts", Some(1), None),
            Some(metric(Some(3.0), None)),
        );
        assert!(only(&render_json(&[f], DEFAULT)).ends_with(",\"metric\":{\"before\":3}}"));
    }

    #[test]
    fn json_duplicate_block() {
        let mut f = finding(
            "duplication/duplicate-block",
            "duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)",
            at("src/orders/format.ts", Some(4), Some(9)),
            None,
        );
        f.related = vec![at("src/utils/date.ts", Some(1), Some(6))];
        assert_eq!(
            only(&render_json(&[f], DEFAULT)),
            "{\"rule\":\"duplication/duplicate-block\",\"severity\":\"warning\",\
             \"message\":\"duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)\",\
             \"path\":\"src/orders/format.ts\",\"line\":4,\"end_line\":9,\
             \"related\":[{\"path\":\"src/utils/date.ts\",\"line\":1,\"end_line\":6}]}"
        );
    }

    #[test]
    fn json_related_location_without_a_range() {
        let mut f = finding("r/x", "m", at("a.ts", Some(1), None), None);
        f.related = vec![at("b.ts", Some(3), None)];
        assert!(
            only(&render_json(&[f], DEFAULT))
                .ends_with(",\"related\":[{\"path\":\"b.ts\",\"line\":3}]}")
        );
    }

    #[test]
    fn json_related_location_without_a_line_is_left_out() {
        let mut f = finding("r/x", "m", at("a.ts", Some(1), None), None);
        f.related = vec![at("b.ts", None, Some(4)), at("c.ts", Some(2), None)];
        assert!(
            only(&render_json(&[f.clone()], DEFAULT))
                .ends_with(",\"related\":[{\"path\":\"c.ts\",\"line\":2}]}")
        );
        f.related.truncate(1);
        assert!(!render_json(&[f], DEFAULT).contains("related"));
    }

    #[test]
    fn json_end_line_without_a_line_is_left_out() {
        let f = finding("r/x", "m", at("a.ts", None, Some(4)), None);
        assert_eq!(
            only(&render_json(&[f], DEFAULT)),
            "{\"rule\":\"r/x\",\"severity\":\"warning\",\"message\":\"m\",\"path\":\"a.ts\"}"
        );
    }

    #[test]
    fn json_severities_and_summary() {
        let mut error = finding("r/x", "m", at("a.ts", None, None), None);
        error.severity = Severity::Error;
        let mut info = finding("r/x", "m", at("a.ts", None, None), None);
        info.severity = Severity::Info;
        let json = render_json(
            &[error, info],
            ReportSummary {
                failing: 1,
                fail_on: "warning",
            },
        );
        assert!(json.contains("\"severity\":\"error\""), "{json}");
        assert!(json.contains("\"severity\":\"info\""), "{json}");
        assert!(
            json.ends_with("\"summary\":{\"count\":2,\"failing\":1,\"fail_on\":\"warning\"}}\n"),
            "{json}"
        );
    }

    #[test]
    fn json_finding_without_a_line_has_no_null() {
        let f = finding(
            "input/unparsable",
            "file not analyzed: not valid UTF-8",
            at("src/a.ts", None, None),
            None,
        );
        let json = render_json(&[f], DEFAULT);
        assert!(!json.contains("null"), "{json}");
        assert_eq!(
            only(&json),
            "{\"rule\":\"input/unparsable\",\"severity\":\"warning\",\
             \"message\":\"file not analyzed: not valid UTF-8\",\"path\":\"src/a.ts\"}"
        );
    }

    #[test]
    fn json_whole_numbers_are_integers_and_fractions_are_floats() {
        let f = |before, after| {
            let f = finding(
                "r/x",
                "m",
                at("a.ts", Some(1), None),
                Some(metric(before, after)),
            );
            only(&render_json(&[f], DEFAULT)).to_owned()
        };
        assert!(f(Some(0.0), Some(-3.0)).ends_with("{\"before\":0,\"after\":-3}}"));
        assert!(f(Some(0.5), Some(2.25)).ends_with("{\"before\":0.5,\"after\":2.25}}"));
        // Just below 2^53 is still an integer; from 2^53 on it is a float.
        let below = f(None, Some(9_007_199_254_740_991.0));
        assert!(below.ends_with("{\"after\":9007199254740991}}"), "{below}");
        let at_limit = f(None, Some(9_007_199_254_740_992.0));
        assert!(
            !at_limit.ends_with("{\"after\":9007199254740992}}"),
            "{at_limit}"
        );
        let parsed: f64 = at_limit
            .rsplit(':')
            .next()
            .and_then(|v| v.trim_end_matches('}').parse().ok())
            .expect("a number");
        assert!((parsed - 9_007_199_254_740_992.0).abs() < 1.0, "{at_limit}");
    }

    #[test]
    fn json_leaves_out_a_metric_it_cannot_write() {
        for m in [
            Some(metric(None, Some(f64::NAN))),
            Some(metric(Some(2.0), Some(f64::INFINITY))),
            Some(metric(Some(f64::NEG_INFINITY), Some(2.0))),
            Some(metric(None, None)),
        ] {
            let f = finding("r/x", "m", at("a.ts", Some(1), None), m);
            let json = render_json(&[f], DEFAULT);
            assert!(!json.contains("metric"), "{m:?}: {json}");
            assert!(!json.contains("null"), "{m:?}: {json}");
            assert!(json.contains("\"line\":1}],"), "{m:?}: {json}");
        }
    }

    #[test]
    fn json_escapes_messages() {
        let f = finding(
            "architecture/forbidden-import",
            "imports \"../db\" with a \\ and a \u{1} here",
            at("a.ts", Some(1), None),
            None,
        );
        assert!(
            render_json(&[f], DEFAULT)
                .contains("\"message\":\"imports \\\"../db\\\" with a \\\\ and a \\u0001 here\"")
        );
    }

    #[cfg(unix)]
    #[test]
    fn json_path_that_is_not_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;

        let path = PathBuf::from(OsStr::from_bytes(b"src/\xffa.ts"));
        let f = finding("r/x", "m", at(path, None, None), None);
        assert!(render_json(&[f], DEFAULT).contains("\"path\":\"src/\u{fffd}a.ts\""));
    }

    #[test]
    fn json_is_one_line() {
        let f = finding("r/x", "a\nb", at("a.ts", Some(1), None), None);
        let json = render_json(&[f], DEFAULT);
        assert_eq!(json.matches('\n').count(), 1, "{json}");
        assert!(json.ends_with("}\n"), "{json}");
    }
}
