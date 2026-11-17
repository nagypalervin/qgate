//! Cyclomatic complexity: reports functions that got more complex, and new
//! functions that are already complex.
//!
//! [`analyze`] compares the files of one change at once, so a function
//! moved to another class or file is still compared with its old version
//! (design D2 of `complexity-of-moved-code`); [`ComplexityAnalyzer`] does
//! the same for one file.

use std::collections::{BTreeMap, HashMap, HashSet};

use qgate_core::{Finding, Location, Metric, Severity};
use qgate_lang::{FileModel, Function};

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

/// Compares per-function cyclomatic complexity between two file versions:
/// [`analyze`] with a change of one file.
///
/// Functions are matched by qualified name, then as moved units by their
/// own name. A matched function that got more complex is reported under
/// [`INCREASE_RULE`]; a new function above the threshold under
/// [`NEW_RULE`]. Removed, unchanged and simplified functions are never
/// reported.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComplexityAnalyzer;

impl Analyzer for ComplexityAnalyzer {
    fn name(&self) -> &'static str {
        "complexity"
    }

    fn analyze(&self, change: &FileChange) -> Vec<Finding> {
        analyze(&[change], &[])
    }
}

/// Compare the units of one language family's changed files, and of the
/// base versions of its files the change removed (deleted, generated, or
/// renamed out of the family), and return what got worse (design D1 of
/// `complexity-of-moved-code`).
///
/// An after unit is matched with the unit of the same qualified name in
/// its file's before version. What is left on both sides is matched by
/// own name, the last segment of the qualified name, across all files
/// (design D2): a unit moved to another class or file is compared with
/// its old version, and its finding says where it came from. Findings
/// come in the order of `changes`, then of each file's after units.
#[must_use]
pub fn analyze(changes: &[&FileChange], gone: &[&FileModel]) -> Vec<Finding> {
    // The before version of each after unit, by change and unit index.
    let mut origins: Vec<Vec<Option<Origin<'_>>>> = Vec::with_capacity(changes.len());
    let mut unmatched: Vec<(usize, usize)> = Vec::new();
    let mut removed: Vec<&Function> = Vec::new();
    for (c, change) in changes.iter().enumerate() {
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
        let mut file = Vec::with_capacity(change.after.functions.len());
        for (u, after) in change.after.functions.iter().enumerate() {
            let origin = before
                .get(after.name.as_str())
                .map(|&old| Origin::Same(old));
            if origin.is_none() {
                unmatched.push((c, u));
            }
            file.push(origin);
        }
        origins.push(file);
        let names: HashSet<&str> = change
            .after
            .functions
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        removed.extend(
            change
                .before
                .functions
                .iter()
                .filter(|f| !names.contains(f.name.as_str())),
        );
    }
    removed.extend(gone.iter().flat_map(|model| &model.functions));

    let after = |(c, u): (usize, usize)| &changes[c].after.functions[u];
    for ((c, u), old) in moves(&unmatched, &removed, after) {
        origins[c][u] = Some(Origin::Moved(old));
    }

    let mut findings = Vec::new();
    for (change, file) in changes.iter().zip(&origins) {
        for (unit, origin) in change.after.functions.iter().zip(file) {
            findings.extend(match origin {
                Some(Origin::Same(old)) => increase(change, unit, *old, None),
                Some(Origin::Moved(old)) => increase(change, unit, old.cyclomatic, Some(&old.name)),
                None => new_function(change, unit),
            });
        }
    }
    findings
}

/// Where an after unit's before version is.
#[derive(Clone, Copy)]
enum Origin<'a> {
    /// In its own file, under the same name, with this complexity.
    Same(u32),
    /// A removed unit elsewhere: the unit moved.
    Moved(&'a Function),
}

/// Pair unmatched after units with removed units by own name (design D2):
/// a group of one of each is a pair; in a larger group, two units with
/// the same fingerprint are, when neither has another such partner.
fn moves<'a, K: Copy>(
    unmatched: &[K],
    removed: &[&'a Function],
    after: impl Fn(K) -> &'a Function,
) -> Vec<(K, &'a Function)> {
    let mut groups: BTreeMap<&str, (Vec<K>, Vec<&Function>)> = BTreeMap::new();
    for &key in unmatched {
        groups
            .entry(own_name(&after(key).name))
            .or_default()
            .0
            .push(key);
    }
    for &old in removed {
        if let Some(group) = groups.get_mut(own_name(&old.name)) {
            group.1.push(old);
        }
    }
    let mut pairs = Vec::new();
    for (new, old) in groups.into_values() {
        if let ([key], [old]) = (new.as_slice(), old.as_slice()) {
            pairs.push((*key, *old));
            continue;
        }
        for &key in &new {
            let fingerprint = after(key).fingerprint;
            let mut partners = old.iter().filter(|o| o.fingerprint == fingerprint);
            let (Some(&partner), None) = (partners.next(), partners.next()) else {
                continue;
            };
            let claimants = new
                .iter()
                .filter(|&&k| after(k).fingerprint == fingerprint)
                .count();
            if claimants == 1 {
                pairs.push((key, partner));
            }
        }
    }
    pairs
}

/// The own name of a unit: the last segment of its qualified name, after
/// the last `.` outside `()`, `<>` and `[]` (design D2), so a C# method
/// keeps its parameter types and a Rust `impl` segment its type
/// arguments. The `>` of a `->` closes nothing.
fn own_name(name: &str) -> &str {
    let mut depth = 0usize;
    let mut start = 0;
    for (i, c) in name.char_indices() {
        if c == '.' && depth == 0 {
            start = i + 1;
        }
        depth = bracket(c, name[..i].ends_with('-')).apply(depth);
    }
    &name[start..]
}

/// What a character of a qualified name does to the bracket depth.
enum Bracket {
    Open,
    Close,
    Other,
}

impl Bracket {
    fn apply(self, depth: usize) -> usize {
        match self {
            Self::Open => depth + 1,
            Self::Close => depth.saturating_sub(1),
            Self::Other => depth,
        }
    }
}

/// `c` as a bracket; the `>` of a `->` (`after_dash`) is none.
fn bracket(c: char, after_dash: bool) -> Bracket {
    match c {
        '(' | '<' | '[' => Bracket::Open,
        ')' | ']' => Bracket::Close,
        '>' if !after_dash => Bracket::Close,
        _ => Bracket::Other,
    }
}

/// Finding for a matched function, if it got more complex; `moved_from`
/// names its old version when it moved.
fn increase(
    change: &FileChange,
    after: &Function,
    before: u32,
    moved_from: Option<&str>,
) -> Option<Finding> {
    let delta = after.cyclomatic.checked_sub(before).filter(|&d| d > 0)?;
    let severity = if delta >= INCREASE_ERROR_DELTA {
        Severity::Error
    } else {
        Severity::Warning
    };
    let mut message = format!(
        "{}: cyclomatic complexity {before} -> {} (+{delta})",
        after.name, after.cyclomatic
    );
    if let Some(old) = moved_from {
        message.push_str(", moved from ");
        message.push_str(old);
    }
    Some(Finding {
        rule: INCREASE_RULE.to_owned(),
        severity,
        message,
        location: location(change, after),
        metric: Some(Metric {
            before: Some(f64::from(before)),
            after: Some(f64::from(after.cyclomatic)),
        }),
        related: Vec::new(),
        hotspot: None,
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
        hotspot: None,
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
            hotspot: None,
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
            hotspot: None,
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

    // complexity-analysis: "A Rust method that got more complex"; a new
    // complex function; and design D2 of `add-rust-analysis`: the two
    // `fmt` of `Display` and `Debug` keep their names when they swap
    // places, so neither is reported.
    #[test]
    fn rust_functions() {
        assert_eq!(
            run_in(
                Language::Rust,
                include_str!("../fixtures/complexity/rust/before.rs"),
                include_str!("../fixtures/complexity/rust/after.rs"),
            ),
            vec![
                increase(Severity::Error, "Cart.total", 8, 3, 8),
                new(Severity::Warning, "Cart.report", 25, 11),
            ]
        );
    }

    /// One file of a change: its path, and its before and after source;
    /// `None` where the file does not exist.
    type File<'a> = (&'a str, Option<&'a str>, Option<&'a str>);

    /// Run [`analyze`] on a change of several files: a file with an after
    /// version is a changed file (added, if it has no before version), a
    /// file without one is gone.
    fn run_files(language: Language, files: &[File<'_>]) -> Vec<Finding> {
        let model = |source: Option<&str>| parse(language, source.unwrap_or("")).expect("parses");
        let mut changes = Vec::new();
        let mut gone = Vec::new();
        for &(path, before, after) in files {
            match after {
                Some(_) => changes.push(FileChange {
                    path: PathBuf::from(path),
                    old_path: None,
                    before: model(before),
                    after: model(after),
                }),
                None => gone.push(model(before)),
            }
        }
        let changes: Vec<&FileChange> = changes.iter().collect();
        let gone: Vec<&FileModel> = gone.iter().collect();
        analyze(&changes, &gone)
    }

    macro_rules! moved {
        ($case:literal, $file:literal) => {
            include_str!(concat!("../fixtures/complexity/moved/", $case, "/", $file))
        };
    }

    // complexity-analysis: "Method moved to a class in another file",
    // "Moved and grown" and "Moved function message". `Run(int)` moved
    // unchanged; `Check(int)` moved and got two more `if` statements.
    #[test]
    fn method_moved_to_another_file() {
        let findings = run_files(
            Language::CSharp,
            &[
                (
                    "src/Tests.cs",
                    Some(moved!("csharp", "before/Tests.cs")),
                    Some(moved!("csharp", "after/Tests.cs")),
                ),
                (
                    "src/Helpers.cs",
                    None,
                    Some(moved!("csharp", "after/Helpers.cs")),
                ),
            ],
        );
        let mut expected = increase(Severity::Warning, "Helpers.Check(int)", 21, 12, 14);
        expected.location.path = "src/Helpers.cs".into();
        expected.message.push_str(", moved from Tests.Check(int)");
        assert_eq!(findings, vec![expected]);
    }

    // complexity-analysis: "Moved out of a deleted file".
    #[test]
    fn moved_out_of_a_deleted_file() {
        let findings = run_files(
            Language::TypeScript,
            &[
                (
                    "src/legacy.ts",
                    Some(moved!("deleted", "before/legacy.ts")),
                    None,
                ),
                (
                    "src/store.ts",
                    None,
                    Some(moved!("deleted", "after/store.ts")),
                ),
            ],
        );
        assert_eq!(findings, vec![]);
    }

    // complexity-analysis: "Moved to another class in the same file"; also
    // through the one-file `Analyzer`.
    #[test]
    fn moved_to_another_class_in_the_same_file() {
        let (before, after) = (
            moved!("same_file", "before/cart.ts"),
            moved!("same_file", "after/cart.ts"),
        );
        assert_eq!(
            run_files(Language::TypeScript, &[(PATH, Some(before), Some(after))]),
            vec![]
        );
        assert_eq!(run(before, after), vec![]);
    }

    // complexity-analysis: "Ambiguous own name".
    #[test]
    fn ambiguous_own_name_is_new() {
        let findings = run_files(
            Language::TypeScript,
            &[
                ("src/a.ts", Some(moved!("ambiguous", "before/a.ts")), None),
                ("src/b.ts", Some(moved!("ambiguous", "before/b.ts")), None),
                ("src/c.ts", None, Some(moved!("ambiguous", "after/c.ts"))),
            ],
        );
        let mut expected = new(Severity::Warning, "C.load", 2, 12);
        expected.location.path = "src/c.ts".into();
        assert_eq!(findings, vec![expected]);
    }

    // complexity-analysis: "Same code settles an ambiguous own name".
    #[test]
    fn same_code_settles_an_ambiguous_own_name() {
        let findings = run_files(
            Language::TypeScript,
            &[
                ("src/a.ts", Some(moved!("settled", "before/a.ts")), None),
                ("src/b.ts", Some(moved!("settled", "before/b.ts")), None),
                ("src/c.ts", None, Some(moved!("settled", "after/c.ts"))),
            ],
        );
        assert_eq!(findings, vec![]);
    }

    // Design D2: the split at the last `.` outside brackets.
    #[test]
    fn own_names() {
        assert_eq!(own_name("Cart.Total(System.Int32)"), "Total(System.Int32)");
        assert_eq!(own_name("<Id as From<a::B>>.from"), "from");
        assert_eq!(own_name("shop.<Cart as fmt::Display>.fmt"), "fmt");
        assert_eq!(own_name("Cart.get size"), "get size");
        assert_eq!(own_name("o.[a.b]"), "[a.b]");
        assert_eq!(own_name("A.create#2"), "create#2");
        assert_eq!(own_name("total"), "total");
    }

    #[test]
    fn identical_files_have_no_findings() {
        let source = include_str!("../fixtures/complexity/legacy_growth/after.ts");
        assert_eq!(run(source, source), vec![]);
    }
}
