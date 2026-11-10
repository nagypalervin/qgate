//! Core types shared by every qgate crate.
//!
//! qgate measures how a codebase changes between two revisions. Analyzers
//! produce [`Finding`]s; reporters render them. Nothing in this crate knows
//! about parsing, git or output formats.

use std::fmt;
use std::path::PathBuf;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Worth knowing, never fails a check.
    Info,
    /// Something got worse, but within the configured tolerance.
    Warning,
    /// Something got worse beyond the configured threshold.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        };
        // `pad` honours width/alignment, so `{:<7}` lines up columns.
        f.pad(s)
    }
}

/// A location in the analyzed source tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    /// Path relative to the repository root.
    pub path: PathBuf,
    /// 1-based line number, if the finding points at a specific line.
    pub line: Option<u32>,
    /// 1-based last line, if the location spans lines from `line` to
    /// here; `None` for a single line or no line.
    pub end_line: Option<u32>,
}

/// A measured value before and after a change.
///
/// qgate cares about the *delta*, not the absolute value: a legacy
/// function with complexity 40 is not a finding, but one that went from
/// 8 to 23 in a single pull request is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metric {
    /// Value at the base revision. `None` if the item is new.
    pub before: Option<f64>,
    /// Value at the head revision. `None` if the item was removed.
    pub after: Option<f64>,
}

impl Metric {
    /// Change from `before` to `after`. A new item counts from zero,
    /// a removed item counts down to zero.
    #[must_use]
    pub fn delta(&self) -> f64 {
        self.after.unwrap_or(0.0) - self.before.unwrap_or(0.0)
    }

    /// Whether the value increased. For every metric qgate tracks,
    /// higher means worse.
    #[must_use]
    pub fn got_worse(&self) -> bool {
        self.delta() > 0.0
    }
}

/// One thing an analyzer wants to tell the user.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// Stable identifier of the rule that produced this, e.g. `complexity/increase`.
    pub rule: String,
    /// How serious it is.
    pub severity: Severity,
    /// Human-readable, one-line explanation.
    pub message: String,
    /// Where it happened.
    pub location: Location,
    /// The measured change, if the rule is metric-based.
    pub metric: Option<Metric>,
    /// Other places the finding refers to, in an order the rule defines
    /// (for a duplicate block: the existing copies). Empty for most rules.
    pub related: Vec<Location>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_of_changed_item() {
        let m = Metric {
            before: Some(8.0),
            after: Some(23.0),
        };
        assert!((m.delta() - 15.0).abs() < f64::EPSILON);
        assert!(m.got_worse());
    }

    #[test]
    fn new_item_counts_from_zero() {
        let m = Metric {
            before: None,
            after: Some(5.0),
        };
        assert!((m.delta() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn removed_item_is_an_improvement() {
        let m = Metric {
            before: Some(12.0),
            after: None,
        };
        assert!(!m.got_worse());
    }

    #[test]
    fn severities_are_ordered() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }
}
