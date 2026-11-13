//! Cyclomatic complexity: reports functions that got more complex, and new
//! functions that are already complex.

use std::collections::HashMap;

use qgate_core::{Finding, Location, Metric, Severity};
use qgate_lang::Function;

use crate::{Analyzer, FileChange};

/// Rule id for a matched function whose complexity increased.
pub const INCREASE_RULE: &str = "complexity/cyclomatic-increase";

/// Rule id for a new function whose complexity is above the threshold.
pub const NEW_RULE: &str = "complexity/cyclomatic-new";

/// An increase of at least this much is an Error; smaller ones are Warnings.
const INCREASE_ERROR_DELTA: u32 = 5;

/// New functions above this are reported (the classic limit from the original cyclomatic complexity paper).
const NEW_WARNING_ABOVE: u32 = 10;

/// New functions above this are Errors (the default `max` of `ESLint`).
const NEW_ERROR_ABOVE: u32 = 20;

/// Compares per-function cyclomatic complexity between two file versions.
///
/// Functions are matched by qualified name. A matched function that got
/// more complex is reported under [`INCREASE_RULE`]; a new function above
/// the threshold under [`NEW_RULE`]. Removed, unchanged and simplified
/// functions are never reported.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComplexityAnalyzer;

impl Analyzer for ComplexityAnalyzer {
    fn name(&self) -> &'static str {
        "complexity"
    }

    fn analyze(&self, change: &FileChange) -> Vec<Finding> {
        let before: HashMap<&str, u32> = change
            .before
            .functions
            .iter()
            .map(|f| (f.name.as_str(), f.cyclomatic))
            .collect();
        // Matching relies on qgate-lang's guarantee that names are unique
        // within a file; a collision would silently drop a function.
        debug_assert_eq!(
            before.len(),
            change.before.functions.len(),
            "function names in a FileModel must be unique"
        );

        change
            .after
            .functions
            .iter()
            .filter_map(|after| match before.get(after.name.as_str()) {
                Some(&old) => increase(change, after, old),
                None => new_function(change, after),
            })
            .collect()
    }
}

/// Finding for a matched function, if it got more complex.
fn increase(change: &FileChange, after: &Function, before: u32) -> Option<Finding> {
    let delta = after.cyclomatic.checked_sub(before).filter(|&d| d > 0)?;
    let severity = if delta >= INCREASE_ERROR_DELTA {
        Severity::Error
    } else {
        Severity::Warning
    };
    Some(Finding {
        rule: INCREASE_RULE.to_owned(),
        severity,
        message: format!(
            "{}: cyclomatic complexity {before} -> {} (+{delta})",
            after.name, after.cyclomatic
        ),
        location: location(change, after),
        metric: Some(Metric {
            before: Some(f64::from(before)),
            after: Some(f64::from(after.cyclomatic)),
        }),
        related: Vec::new(),
    })
}

/// Finding for a new function, if it is already above the threshold.
fn new_function(change: &FileChange, after: &Function) -> Option<Finding> {
    let severity = match after.cyclomatic {
        c if c > NEW_ERROR_ABOVE => Severity::Error,
        c if c > NEW_WARNING_ABOVE => Severity::Warning,
        _ => return None,
    };
    Some(Finding {
        rule: NEW_RULE.to_owned(),
        severity,
        message: format!(
            "{}: new function, cyclomatic complexity {}",
            after.name, after.cyclomatic
        ),
        location: location(change, after),
        metric: Some(Metric {
            before: None,
            after: Some(f64::from(after.cyclomatic)),
        }),
        related: Vec::new(),
    })
}

fn location(change: &FileChange, after: &Function) -> Location {
    Location {
        path: change.path.clone(),
        line: Some(after.line),
        end_line: None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use qgate_core::{Location, Metric, Severity};
    use qgate_lang::{Language, parse};

    use super::*;

    const PATH: &str = "src/example.ts";

    /// Run the analyzer on a before/after fixture pair.
    fn run(before: &str, after: &str) -> Vec<Finding> {
        run_in(Language::TypeScript, before, after)
    }

    fn run_in(language: Language, before: &str, after: &str) -> Vec<Finding> {
        let change = FileChange {
            path: PathBuf::from(PATH),
            old_path: None,
            before: parse(language, before).expect("before parses"),
            after: parse(language, after).expect("after parses"),
        };
        ComplexityAnalyzer.analyze(&change)
    }

    macro_rules! fixture {
        ($case:literal) => {
            run(
                include_str!(concat!("../fixtures/complexity/", $case, "/before.ts")),
                include_str!(concat!("../fixtures/complexity/", $case, "/after.ts")),
            )
        };
    }

    fn increase(severity: Severity, name: &str, line: u32, before: u32, after: u32) -> Finding {
        Finding {
            rule: INCREASE_RULE.to_owned(),
            severity,
            message: format!(
                "{name}: cyclomatic complexity {before} -> {after} (+{})",
                after - before
            ),
            location: Location {
                path: PATH.into(),
                line: Some(line),
                end_line: None,
            },
            metric: Some(Metric {
                before: Some(f64::from(before)),
                after: Some(f64::from(after)),
            }),
            related: Vec::new(),
        }
    }

    fn new(severity: Severity, name: &str, line: u32, after: u32) -> Finding {
        Finding {
            rule: NEW_RULE.to_owned(),
            severity,
            message: format!("{name}: new function, cyclomatic complexity {after}"),
            location: Location {
                path: PATH.into(),
                line: Some(line),
                end_line: None,
            },
            metric: Some(Metric {
                before: None,
                after: Some(f64::from(after)),
            }),
            related: Vec::new(),
        }
    }

    #[test]
    fn small_increases_are_warnings() {
        assert_eq!(
            fixture!("small_increase"),
            vec![
                // +1; the line comes from the after file, which moved it
                increase(Severity::Warning, "applyDiscount", 6, 4, 5),
                // +4 is the largest increase that is still a warning
                increase(Severity::Warning, "shipping", 14, 2, 6),
            ]
        );
    }

    #[test]
    fn increase_of_five_is_an_error() {
        assert_eq!(
            fixture!("large_increase"),
            vec![increase(Severity::Error, "process", 3, 3, 8)]
        );
    }

    #[test]
    fn large_legacy_function_growing_a_little_is_a_warning() {
        assert_eq!(
            fixture!("legacy_growth"),
            vec![increase(Severity::Warning, "legacy", 3, 40, 41)]
        );
    }

    #[test]
    fn new_functions_are_reported_only_above_ten() {
        assert_eq!(
            fixture!("new_functions"),
            vec![
                // atTen (10) is not reported
                new(Severity::Warning, "atEleven", 21, 11),
                // 20 is the largest value that is still a warning
                new(Severity::Warning, "atTwenty", 35, 20),
                new(Severity::Error, "atTwentyOne", 58, 21),
            ]
        );
    }

    #[test]
    fn improvements_and_removals_are_not_findings() {
        assert_eq!(fixture!("improvements"), vec![]);
    }

    #[test]
    fn renamed_function_is_removed_plus_new() {
        assert_eq!(
            fixture!("rename"),
            vec![new(Severity::Warning, "calculateTotal", 3, 12)]
        );
    }

    // complexity-analysis: "A C# method that got more complex";
    // csharp-analysis: "An overload added above".
    #[test]
    fn csharp_methods() {
        assert_eq!(
            run_in(
                Language::CSharp,
                include_str!("../fixtures/complexity/csharp/before.cs"),
                include_str!("../fixtures/complexity/csharp/after.cs"),
            ),
            vec![
                // the new overload `Cart.Total(string)` (4) is not reported,
                // and `Cart.Total(int)` is still matched by its signature
                increase(Severity::Warning, "Cart.Total(int)", 14, 3, 5),
                increase(Severity::Error, "Cart.Shipping(string)", 23, 2, 7),
                new(Severity::Warning, "Cart.Report(int[])", 33, 11),
            ]
        );
    }

    #[test]
    fn identical_files_have_no_findings() {
        let source = include_str!("../fixtures/complexity/legacy_growth/after.ts");
        assert_eq!(run(source, source), vec![]);
    }
}
