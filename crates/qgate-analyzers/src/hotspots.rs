//! Hotspot context: which files are both complex and changed often, and
//! the note their findings carry (design D3 of `add-hotspot-context`).
//!
//! A repository-level step after the analyzers, fed by the CLI with each
//! ranked file's churn (commits in the window, from git) and complexity
//! (from its head model). It reports nothing of its own and never changes
//! a severity: qgate reports deltas, and a hotspot is context for the
//! reviewer on something that already got worse.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::BuildHasher;

use qgate_core::{Finding, Hotspot};

/// Days of history before the head commit that count as churn: 26 weeks,
/// "6 months" in the note.
pub const WINDOW_DAYS: i64 = 182;

/// The fewest commits in the window that make a file changed often.
pub const MIN_COMMITS: u32 = 5;

/// Rule id whose findings never get the note: a file that could not be
/// read has no complexity to show.
const UNPARSABLE_RULE: &str = "input/unparsable";

/// One ranked file of a language family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankedFile<'a> {
    /// Path in the head revision.
    pub path: &'a str,
    /// Commits that changed it in the window.
    pub churn: u32,
    /// The sum of the cyclomatic complexity of its units.
    pub complexity: u32,
}

/// The hotspots among the ranked files of one language family, by path.
///
/// A file is a hotspot when it changed in at least [`MIN_COMMITS`]
/// commits, fewer than 10% of the files have a higher churn, its
/// complexity is at least 1, and fewer than 25% of the files are more
/// complex. Ties at the top all count.
#[must_use]
pub fn hotspots(files: &[RankedFile<'_>]) -> HashMap<String, Hotspot> {
    // "Fewer than k% of the n files above it": above * 100 < k * n, in
    // integers, so a tie at the top never needs a rounding rule.
    let fewer_than = |percent: usize, above: usize| above * 100 < percent * files.len();
    files
        .iter()
        .filter(|f| f.churn >= MIN_COMMITS && f.complexity >= 1)
        .filter(|f| fewer_than(10, files.iter().filter(|g| g.churn > f.churn).count()))
        .filter(|f| {
            fewer_than(
                25,
                files.iter().filter(|g| g.complexity > f.complexity).count(),
            )
        })
        .map(|f| {
            (
                f.path.to_owned(),
                Hotspot {
                    commits: f.churn,
                    complexity: f.complexity,
                },
            )
        })
        .collect()
}

/// Add the hotspot note to every finding at a hotspot path, except
/// `input/unparsable`: set [`Finding::hotspot`] and append `; hotspot:
/// <n> commits in 6 months, complexity <c>` to the message.
pub fn annotate<S: BuildHasher>(findings: &mut [Finding], hotspots: &HashMap<String, Hotspot, S>) {
    for finding in findings.iter_mut().filter(|f| f.rule != UNPARSABLE_RULE) {
        let Some(path) = finding.location.path.to_str() else {
            continue;
        };
        if let Some(&hotspot) = hotspots.get(path) {
            let _ = write!(
                finding.message,
                "; hotspot: {} commits in 6 months, complexity {}",
                hotspot.commits, hotspot.complexity
            );
            finding.hotspot = Some(hotspot);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use qgate_core::{Location, Severity};

    use super::*;

    fn file(path: &str, churn: u32, complexity: u32) -> RankedFile<'_> {
        RankedFile {
            path,
            churn,
            complexity,
        }
    }

    /// `n` quiet, simple files, then `extra`.
    fn family<'a>(n: usize, names: &'a [String], extra: &[RankedFile<'a>]) -> Vec<RankedFile<'a>> {
        let mut files: Vec<RankedFile<'a>> = names[..n].iter().map(|p| file(p, 1, 2)).collect();
        files.extend_from_slice(extra);
        files
    }

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("src/f{i}.cs")).collect()
    }

    fn paths(found: &HashMap<String, Hotspot>) -> Vec<&str> {
        let mut paths: Vec<&str> = found.keys().map(String::as_str).collect();
        paths.sort_unstable();
        paths
    }

    // hotspot-analysis: "Hotspot files", "Changed often and complex"
    #[test]
    fn changed_often_and_complex() {
        let names = names(34);
        // 40 files; the cart has the second highest churn and the fifth
        // highest complexity, as in the spec's scenario.
        let files = family(
            34,
            &names,
            &[
                file("src/Busy.cs", 40, 30),
                file("src/Cart.cs", 31, 87),
                file("src/X1.cs", 1, 90),
                file("src/X2.cs", 1, 91),
                file("src/X3.cs", 1, 92),
                file("src/X4.cs", 1, 93),
            ],
        );
        let found = hotspots(&files);
        assert_eq!(paths(&found), ["src/Busy.cs", "src/Cart.cs"]);
        assert_eq!(
            found["src/Cart.cs"],
            Hotspot {
                commits: 31,
                complexity: 87
            }
        );
    }

    // "Changed often but simple", "Complex but rarely changed"
    #[test]
    fn one_condition_is_not_enough() {
        let names = names(36);
        // 38 files: the busiest one is the simplest, and the most complex
        // one changed in only 4 commits.
        let mut files: Vec<RankedFile<'_>> = names[..25]
            .iter()
            .map(|p| file(p, 1, 2))
            .chain(names[25..].iter().map(|p| file(p, 1, 50)))
            .collect();
        files.push(file("src/Dto.cs", 50, 1));
        files.push(file("src/Legacy.cs", 4, 500));
        let found = hotspots(&files);
        assert_eq!(paths(&found), Vec::<&str>::new());
    }

    // "Ties"
    #[test]
    fn ties_at_the_top_all_count() {
        let names = names(18);
        let files = family(
            18,
            &names,
            &[file("src/A.ts", 9, 40), file("src/B.ts", 9, 40)],
        );
        assert_eq!(paths(&hotspots(&files)), ["src/A.ts", "src/B.ts"]);
    }

    // "Fewer than k%" at its edges (design D3): with 20 files, 2 above is
    // 10% and not fewer; 5 above is 25% and not fewer.
    #[test]
    fn the_edges_of_fewer_than() {
        let names = names(20);
        let with = |extra: &[RankedFile<'static>]| {
            let mut files: Vec<RankedFile<'_>> = names[..20 - extra.len() - 1]
                .iter()
                .map(|p| file(p, 1, 2))
                .collect();
            files.extend_from_slice(extra);
            files.push(file("src/T.cs", 9, 40));
            assert_eq!(files.len(), 20);
            paths(&hotspots(&files)).contains(&"src/T.cs")
        };
        let busier = file("src/B1.cs", 10, 1);
        let busier2 = file("src/B2.cs", 10, 1);
        assert!(with(&[busier]), "1 of 20 above in churn");
        assert!(!with(&[busier, busier2]), "2 of 20 above in churn");
        let complex: Vec<RankedFile<'static>> = [
            "src/C1.cs",
            "src/C2.cs",
            "src/C3.cs",
            "src/C4.cs",
            "src/C5.cs",
        ]
        .iter()
        .map(|p| file(p, 1, 50))
        .collect();
        assert!(with(&complex[..4]), "4 of 20 above in complexity");
        assert!(!with(&complex[..5]), "5 of 20 above in complexity");
    }

    #[test]
    fn the_minimum_of_five_commits_and_of_complexity_one() {
        let names = names(20);
        let quiet = family(20, &names, &[file("src/Top.cs", 4, 40)]);
        assert_eq!(paths(&hotspots(&quiet)), Vec::<&str>::new());
        let names0: Vec<String> = (0..20).map(|i| format!("src/m{i}.cs")).collect();
        let mut flat: Vec<RankedFile<'_>> = names0.iter().map(|p| file(p, 1, 0)).collect();
        flat.push(file("src/Models.cs", 9, 0));
        assert_eq!(paths(&hotspots(&flat)), Vec::<&str>::new());
    }

    #[test]
    fn a_tiny_repository() {
        let files = [
            file("src/a.ts", 6, 9),
            file("src/b.ts", 2, 3),
            file("src/c.ts", 1, 1),
        ];
        assert_eq!(paths(&hotspots(&files)), ["src/a.ts"]);
        assert_eq!(paths(&hotspots(&[])), Vec::<&str>::new());
    }

    fn finding(rule: &str, path: &str, message: &str) -> Finding {
        Finding {
            rule: rule.to_owned(),
            severity: Severity::Warning,
            message: message.to_owned(),
            location: Location {
                path: PathBuf::from(path),
                line: Some(12),
                end_line: None,
            },
            metric: None,
            related: Vec::new(),
            hotspot: None,
        }
    }

    // "The hotspot note", "Hotspot context in revision mode"
    #[test]
    fn annotate_findings_in_hotspots() {
        let cart = Hotspot {
            commits: 31,
            complexity: 87,
        };
        let found = HashMap::from([("src/Cart.cs".to_owned(), cart)]);
        let mut findings = vec![
            finding(
                "complexity/cyclomatic-increase",
                "src/Cart.cs",
                "Cart.Total(): cyclomatic complexity 4 -> 6 (+2)",
            ),
            finding(
                "input/unparsable",
                "src/Cart.cs",
                "file not analyzed: syntax error",
            ),
            finding(
                "tests/unchanged-test",
                "src/Other.cs",
                "Other.Run() changed",
            ),
        ];
        let before = findings.clone();
        annotate(&mut findings, &found);
        assert_eq!(
            findings[0].message,
            "Cart.Total(): cyclomatic complexity 4 -> 6 (+2); hotspot: 31 commits in 6 months, complexity 87"
        );
        assert_eq!(findings[0].hotspot, Some(cart));
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[1..], before[1..]);
    }
}
