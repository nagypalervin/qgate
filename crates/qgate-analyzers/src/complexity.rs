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
/// own name, the last segment of the qualified name, across all files,
/// when the two have the same code or code evidence (design D2): a unit
/// moved to another class or file is compared with its old version, and
/// its finding says where it came from. Findings
/// come in the order of `changes`, then of each file's after units.
#[must_use]
pub fn analyze(changes: &[&FileChange], gone: &[&FileModel]) -> Vec<Finding> {
    // The before version of each after unit, by change and unit index.
    let mut origins: Vec<Vec<Option<Origin<'_>>>> = Vec::with_capacity(changes.len());
    let mut unmatched: Vec<((usize, usize), Unit<'_>)> = Vec::new();
    let mut removed: Vec<Unit<'_>> = Vec::new();
    for (c, change) in changes.iter().enumerate() {
        let before: HashMap<&str, usize> = change
            .before
            .functions
            .iter()
            .enumerate()
            .map(|(b, f)| (f.name.as_str(), b))
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
                .map(|&b| Origin::Same(Unit::new(&change.before, b)));
            if origin.is_none() {
                unmatched.push(((c, u), Unit::new(&change.after, u)));
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
            (0..change.before.functions.len())
                .map(|u| Unit::new(&change.before, u))
                .filter(|unit| !names.contains(unit.function().name.as_str())),
        );
    }
    removed.extend(
        gone.iter()
            .flat_map(|&model| (0..model.functions.len()).map(move |u| Unit::new(model, u))),
    );

    let pairs = moves(&unmatched, &removed);
    removed.retain(|old| !pairs.iter().any(|(_, moved)| moved.is(*old)));
    for ((c, u), old) in pairs {
        origins[c][u] = Some(Origin::Moved(old));
    }
    let credits = inlined(changes, &origins, &removed);

    let mut findings = Vec::new();
    for (c, (change, file)) in changes.iter().zip(&origins).enumerate() {
        for (u, (unit, origin)) in change.after.functions.iter().zip(file).enumerate() {
            let inlined = credits.get(&(c, u)).copied();
            findings.extend(match origin {
                Some(origin) => {
                    let old = origin.unit().function();
                    let moved_from =
                        matches!(origin, Origin::Moved(_)).then_some(old.name.as_str());
                    increase(change, unit, old.cyclomatic, moved_from, inlined)
                }
                None => new_function(change, unit),
            });
        }
    }
    findings
}

/// The unit each matched unit that got more complex inlined, by change
/// and unit index (design D3). A removed unit with decision points and a
/// grown unit qualify as a pair when the longest common subsequence of
/// the removed unit's own tokens and the code the change added to the
/// grown unit is two thirds of both. Pairs are taken one to one, the
/// longest subsequence first; ties go to the first grown unit by path,
/// then by line, and then to the first removed unit.
fn inlined<'a>(
    changes: &[&'a FileChange],
    origins: &[Vec<Option<Origin<'a>>>],
    removed: &[Unit<'a>],
) -> HashMap<(usize, usize), &'a Function> {
    let mut interner = HashMap::new();
    let removed: Vec<(&Function, Vec<usize>)> = removed
        .iter()
        .filter(|old| old.function().cyclomatic > 1)
        .map(|old| (old.function(), old.tokens(&mut interner)))
        .filter(|(_, tokens)| !tokens.is_empty())
        .collect();
    let mut candidates = candidates(changes, origins, &removed, &mut interner);
    candidates.sort_by_key(|&(common, g, r, _)| (std::cmp::Reverse(common), g, r));
    let mut credits = HashMap::new();
    let mut taken = HashSet::new();
    for (_, _, r, key) in candidates {
        if !credits.contains_key(&key) && taken.insert(r) {
            credits.insert(key, removed[r].0);
        }
    }
    credits
}

/// The pairs of a grown unit and a removed unit with evidence of
/// inlining, as (length of the common subsequence, index of the grown
/// unit in [`grown`]'s order, index in `removed`, key of the grown unit).
fn candidates<'a>(
    changes: &[&'a FileChange],
    origins: &[Vec<Option<Origin<'a>>>],
    removed: &[(&Function, Vec<usize>)],
    interner: &mut HashMap<&'a str, usize>,
) -> Vec<(usize, usize, usize, (usize, usize))> {
    let mut candidates = Vec::new();
    let Some(largest) = removed.iter().map(|(_, tokens)| tokens.len()).max() else {
        return candidates;
    };
    for (g, (key, after, before)) in grown(changes, origins).into_iter().enumerate() {
        let (after, before) = (after.tokens(interner), before.tokens(interner));
        // The added code has at least this many tokens; above 3/2 of the
        // largest removed unit, no pair can reach two thirds of both.
        if 2 * after.len().saturating_sub(before.len()) > 3 * largest {
            continue;
        }
        let Some(added) = added(&before, &after) else {
            continue;
        };
        for (r, (_, tokens)) in removed.iter().enumerate() {
            if let Some(common) = evidence(tokens, &added) {
                candidates.push((common, g, r, key));
            }
        }
    }
    candidates
}

/// The length of the longest common subsequence of a removed unit's own
/// `tokens` and the `added` code of a grown unit, if it is two thirds of
/// both (design D3). `None` above [`ALIGNED_PAIRS`] pairs of tokens.
fn evidence(tokens: &[usize], added: &[usize]) -> Option<usize> {
    let (short, long) = (tokens.len().min(added.len()), tokens.len().max(added.len()));
    // Two thirds of both needs sizes within a factor of 3/2.
    if 3 * short < 2 * long || short.saturating_mul(long) > ALIGNED_PAIRS {
        return None;
    }
    let common = common_in_order(tokens, added);
    (3 * common >= 2 * long).then_some(common)
}

/// The matched units that got more complex, by change and unit index,
/// with their after and before versions, first by path, then by line.
fn grown<'a>(
    changes: &[&'a FileChange],
    origins: &[Vec<Option<Origin<'a>>>],
) -> Vec<((usize, usize), Unit<'a>, Unit<'a>)> {
    let mut grown = Vec::new();
    for (c, (change, file)) in changes.iter().zip(origins).enumerate() {
        for (u, (unit, origin)) in change.after.functions.iter().zip(file).enumerate() {
            let before = origin.map(Origin::unit);
            if let Some(before) = before.filter(|b| unit.cyclomatic > b.function().cyclomatic) {
                grown.push(((c, u), Unit::new(&change.after, u), before));
            }
        }
    }
    grown.sort_by_key(|&((c, _), unit, _)| (&changes[c].path, unit.function().line));
    grown
}

/// The largest number of token pairs [`added`] aligns (2^26 bits, 8 MiB)
/// and [`evidence`] compares (design D3), and that [`shares_half`]
/// compares for a move (design D2 of `limit-move-evidence`).
const ALIGNED_PAIRS: usize = 1 << 26;

/// The tokens of `after` outside a longest common subsequence with
/// `before`: the code a change added to a unit (design D3). The common
/// start and end are cut off first; for the middle, one bit per pair of
/// tokens records the choice of the dynamic programming, and a tie keeps
/// the `after` token, so within the middle added tokens come as late as
/// they can. `None` when the middle has more than [`ALIGNED_PAIRS`] pairs.
fn added(before: &[usize], after: &[usize]) -> Option<Vec<usize>> {
    let (_, before, after) = middles(before, after);
    let (n, m) = (before.len(), after.len());
    if n.saturating_mul(m) > ALIGNED_PAIRS {
        return None;
    }
    // skip[i * m + j]: with before[i] and after[j] unequal, dropping
    // before[i] keeps the longest common subsequence of before[i..] and
    // after[j..].
    let mut skip = vec![0u64; (n * m).div_ceil(64)];
    // below[j] and row[j]: the longest common subsequence of
    // before[i + 1..] and before[i..] with after[j..].
    let mut below = vec![0usize; m + 1];
    let mut row = vec![0usize; m + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            row[j] = if before[i] == after[j] {
                below[j + 1] + 1
            } else if below[j] >= row[j + 1] {
                let bit = i * m + j;
                skip[bit / 64] |= 1 << (bit % 64);
                below[j]
            } else {
                row[j + 1]
            };
        }
        std::mem::swap(&mut row, &mut below);
    }
    Some(outside(before, after, &skip))
}

/// The middles of `a` and `b`: what is left of each when their common
/// start and then, of the rest, their common end are cut off, after the
/// number of tokens cut from each. A longest common subsequence of `a`
/// and `b` is the cut tokens plus one of the middles.
fn middles<'t>(a: &'t [usize], b: &'t [usize]) -> (usize, &'t [usize], &'t [usize]) {
    let start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[start..], &b[start..]);
    let end = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (start + end, &a[..a.len() - end], &b[..b.len() - end])
}

/// The tokens of `after` outside the common subsequence with `before`
/// that the `skip` bits of [`added`] trace.
fn outside(before: &[usize], after: &[usize], skip: &[u64]) -> Vec<usize> {
    let (n, m) = (before.len(), after.len());
    let (mut i, mut added) = (0, Vec::new());
    let mut j = 0;
    while j < m {
        if i < n && before[i] == after[j] {
            i += 1;
            j += 1;
        } else if i < n && skip[(i * m + j) / 64] & (1 << ((i * m + j) % 64)) != 0 {
            i += 1;
        } else {
            added.push(after[j]);
            j += 1;
        }
    }
    added
}

/// Where an after unit's before version is.
#[derive(Clone, Copy)]
enum Origin<'a> {
    /// In its own file, under the same name.
    Same(Unit<'a>),
    /// A removed unit elsewhere: the unit moved.
    Moved(Unit<'a>),
}

impl<'a> Origin<'a> {
    /// The before version.
    fn unit(self) -> Unit<'a> {
        match self {
            Self::Same(unit) | Self::Moved(unit) => unit,
        }
    }
}

/// One unit of a file version: its function and, through the file's
/// tokens, its own code.
#[derive(Clone, Copy)]
struct Unit<'a> {
    model: &'a FileModel,
    index: usize,
}

impl<'a> Unit<'a> {
    fn new(model: &'a FileModel, index: usize) -> Self {
        Self { model, index }
    }

    fn function(self) -> &'a Function {
        &self.model.functions[self.index]
    }

    /// Whether `self` and `other` are the same unit of the same version.
    fn is(self, other: Self) -> bool {
        std::ptr::eq(self.model, other.model) && self.index == other.index
    }

    /// The unit's own tokens, as ids of `interner`.
    fn tokens(self, interner: &mut HashMap<&'a str, usize>) -> Vec<usize> {
        self.model
            .tokens
            .iter()
            .filter(|token| token.unit.and_then(|u| usize::try_from(u).ok()) == Some(self.index))
            .map(|token| {
                let next = interner.len();
                *interner.entry(token.text.as_str()).or_insert(next)
            })
            .collect()
    }
}

/// Pair unmatched after units with removed units of the same own name
/// (design D2), in two passes over each group: first units with the same
/// code (fingerprint), then, among the rest, units with code evidence
/// ([`shares_half`], which has a limit, so two long units that differ
/// almost throughout are matched only by the same code).
fn moves<'a, K: Copy>(unmatched: &[(K, Unit<'a>)], removed: &[Unit<'a>]) -> Vec<(K, Unit<'a>)> {
    let mut groups: BTreeMap<&str, Group<'a, K>> = BTreeMap::new();
    for &(key, unit) in unmatched {
        groups
            .entry(own_name(&unit.function().name))
            .or_default()
            .0
            .push((key, unit));
    }
    for &old in removed {
        if let Some(group) = groups.get_mut(own_name(&old.function().name)) {
            group.1.push(old);
        }
    }
    let mut interner = HashMap::new();
    let mut pairs = Vec::new();
    for (mut new, mut old) in groups.into_values() {
        if old.is_empty() {
            continue;
        }
        let new_code: Vec<u64> = new.iter().map(|n| n.1.function().fingerprint).collect();
        let old_code: Vec<u64> = old.iter().map(|o| o.function().fingerprint).collect();
        settle(
            &mut new,
            &mut old,
            |i, j| new_code[i] == old_code[j],
            &mut pairs,
        );
        if new.is_empty() || old.is_empty() {
            continue;
        }
        let new_tokens: Vec<Vec<usize>> = new.iter().map(|n| n.1.tokens(&mut interner)).collect();
        let old_tokens: Vec<Vec<usize>> = old.iter().map(|o| o.tokens(&mut interner)).collect();
        settle(
            &mut new,
            &mut old,
            |i, j| shares_half(&old_tokens[j], &new_tokens[i]),
            &mut pairs,
        );
    }
    pairs
}

/// The unmatched after units (with their keys) and the removed units of
/// one own name.
type Group<'a, K> = (Vec<(K, Unit<'a>)>, Vec<Unit<'a>>);

/// Pair `new[i]` with `old[j]` where `partners(i, j)` holds and neither
/// has another partner, add the pairs to `pairs`, and take the paired
/// units out of `new` and `old`.
fn settle<'a, K: Copy>(
    new: &mut Vec<(K, Unit<'a>)>,
    old: &mut Vec<Unit<'a>>,
    partners: impl Fn(usize, usize) -> bool,
    pairs: &mut Vec<(K, Unit<'a>)>,
) {
    let matrix: Vec<Vec<bool>> = (0..new.len())
        .map(|i| (0..old.len()).map(|j| partners(i, j)).collect())
        .collect();
    let mut paired_new = vec![false; new.len()];
    let mut paired_old = vec![false; old.len()];
    for (i, row) in matrix.iter().enumerate() {
        let mut found = row.iter().enumerate().filter(|&(_, &p)| p).map(|(j, _)| j);
        let (Some(j), None) = (found.next(), found.next()) else {
            continue;
        };
        if matrix.iter().filter(|row| row[j]).count() == 1 {
            pairs.push((new[i].0, old[j]));
            paired_new[i] = true;
            paired_old[j] = true;
        }
    }
    let mut keep = paired_new.iter().map(|&p| !p);
    new.retain(|_| keep.next().unwrap_or(true));
    let mut keep = paired_old.iter().map(|&p| !p);
    old.retain(|_| keep.next().unwrap_or(true));
}

/// Whether the tokens of `a` and `b` have a common subsequence of at
/// least half of each one's length: the evidence of a move (design D2).
/// A unit without tokens is no evidence, nor is one more than twice as
/// long as the other, which is checked before the subsequence. The
/// subsequence is the common start and end plus one of the [`middles`];
/// when those form more than [`ALIGNED_PAIRS`] pairs, they are not
/// compared and there is no evidence (design D1 of `limit-move-evidence`).
fn shares_half(a: &[usize], b: &[usize]) -> bool {
    let (shorter, longer) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if shorter.is_empty() || 2 * shorter.len() < longer.len() {
        return false;
    }
    let (cut, longer_middle, shorter_middle) = middles(longer, shorter);
    if longer_middle.len().saturating_mul(shorter_middle.len()) > ALIGNED_PAIRS {
        return false;
    }
    2 * (cut + common_in_order(longer_middle, shorter_middle)) >= longer.len()
}

/// The length of the longest common subsequence of `a` and `b`, by one
/// row of dynamic programming, in O(|a|·|b|) time and O(|b|) memory.
fn common_in_order(a: &[usize], b: &[usize]) -> usize {
    // row[j]: the longest common subsequence of the tokens of `a` so far
    // and b[..j].
    let mut row = vec![0usize; b.len() + 1];
    for &token in a {
        // The previous row's row[j], while row[j] is being overwritten.
        let mut diagonal = 0;
        for (j, &other) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if token == other {
                diagonal + 1
            } else {
                above.max(row[j])
            };
            diagonal = above;
        }
    }
    row[b.len()]
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
        depth = bracket(c, &name[..i]).apply(depth);
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

/// `c`, which follows `before` in a name, as a bracket. The `>` of a
/// `->`, and the `<` and `>` of a C# operator such as `operator <=` or
/// `operator >>`, are none.
fn bracket(c: char, before: &str) -> Bracket {
    let in_operator = || {
        let operator = before.trim_end_matches(['<', '>']);
        operator.ends_with("operator ") && before.len() - operator.len() < 3
    };
    match c {
        '(' | '[' => Bracket::Open,
        ')' | ']' => Bracket::Close,
        '<' if !in_operator() => Bracket::Open,
        '>' if !before.ends_with('-') && !in_operator() => Bracket::Close,
        _ => Bracket::Other,
    }
}

/// Finding for a matched function, if it got more complex beyond the
/// unit it `inlined` (design D3); `moved_from` names its old version
/// when it moved. The severity comes from the growth beyond the inlined
/// unit, the metric keeps the real values (design D4).
fn increase(
    change: &FileChange,
    after: &Function,
    before: u32,
    moved_from: Option<&str>,
    inlined: Option<&Function>,
) -> Option<Finding> {
    let delta = after.cyclomatic.checked_sub(before).filter(|&d| d > 0)?;
    let credit = inlined.map_or(0, |f| f.cyclomatic - 1);
    let growth = delta.checked_sub(credit).filter(|&g| g > 0)?;
    let severity = if growth >= INCREASE_ERROR_DELTA {
        Severity::Error
    } else {
        Severity::Warning
    };
    let mut message = format!(
        "{}: cyclomatic complexity {before} -> {} (+{delta}",
        after.name, after.cyclomatic
    );
    if let Some(inlined) = inlined {
        message.push_str(", +");
        message.push_str(&growth.to_string());
        message.push_str(" beyond the inlined ");
        message.push_str(&inlined.name);
    }
    message.push(')');
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
    use std::fmt::Write as _;
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

    // complexity-analysis: "Same own name, different code". The one
    // removed and the one added `ngOnInit` share less than half of the
    // removed one's tokens, so they are no move (design D2).
    #[test]
    fn same_own_name_with_different_code_is_new() {
        let findings = run_files(
            Language::TypeScript,
            &[
                (
                    "src/a.component.ts",
                    Some(moved!("different", "before/a.component.ts")),
                    None,
                ),
                (
                    "src/b.component.ts",
                    None,
                    Some(moved!("different", "after/b.component.ts")),
                ),
            ],
        );
        let mut expected = new(Severity::Warning, "BComponent.ngOnInit", 2, 14);
        expected.location.path = "src/b.component.ts".into();
        assert_eq!(findings, vec![expected]);
    }

    // Design D2: the evidence of a move, a common subsequence of at
    // least half of each unit's tokens.
    #[test]
    fn evidence_is_half_of_both_units_tokens_in_order() {
        assert_eq!(common_in_order(&[1, 2, 3, 4], &[9, 1, 9, 2, 9]), 2);
        assert_eq!(common_in_order(&[1, 2, 3, 4], &[4, 3, 2, 1]), 1);
        assert_eq!(common_in_order(&[], &[1]), 0);
        assert!(shares_half(&[1, 2, 3, 4], &[1, 2, 9, 9]));
        assert!(shares_half(&[1, 2, 3], &[1, 2, 3, 4, 5, 6]));
        assert!(shares_half(&[1, 2, 3, 4, 5, 6], &[1, 2, 3]));
        // A short removed namesake is no evidence for a long after unit,
        // and the other way round.
        assert!(!shares_half(&[1, 2], &[1, 2, 3, 4, 5]));
        assert!(!shares_half(&[1, 2, 3, 4, 5], &[1, 2]));
        assert!(!shares_half(&[1, 2, 3, 4], &[9, 1, 9, 2, 9]));
        assert!(!shares_half(&[1, 2, 3, 4], &[4, 3, 2, 1]));
        assert!(!shares_half(&[], &[]));
        assert!(!shares_half(&[1, 2], &[]));
    }

    // Design D1 of `limit-move-evidence`: two units whose middles, without
    // the common start and end, form more than 2^26 pairs are no partners,
    // though they share almost all their tokens; a change in one place
    // leaves a middle of one token.
    #[test]
    fn move_evidence_has_a_limit() {
        let long: Vec<usize> = (0..9000).collect();
        let mut ends = long.clone();
        ends[0] = usize::MAX;
        ends[8999] = usize::MAX;
        assert!(!shares_half(&long, &ends));
        let mut one = long.clone();
        one[4500] = usize::MAX;
        assert!(shares_half(&long, &one));
    }

    /// A C# class `class` whose method `Run(int)`, on line 5, has
    /// complexity 12: eleven `if` statements, then the statement `first`,
    /// 3,000 assignments (more than 8,192 tokens) and the statement `last`.
    fn long_run(class: &str, first: &str, last: &str) -> String {
        let mut body = String::new();
        for i in 1..=11 {
            let _ = writeln!(body, "        if (d > {i}) {{ d++; }}");
        }
        let _ = writeln!(body, "        {first};");
        for i in 0..3000 {
            let _ = writeln!(body, "        d += {i};");
        }
        let _ = writeln!(body, "        {last};");
        format!(
            "namespace Shop;\n\npublic class {class}\n{{\n    private static int Run(int d)\n    {{\n{body}        return d;\n    }}\n}}\n"
        )
    }

    /// The findings when `src/Tests.cs`, with `before`, is deleted and
    /// `src/Helpers.cs`, with `after`, is added.
    fn moved_run(before: &str, after: &str) -> Vec<Finding> {
        run_files(
            Language::CSharp,
            &[
                ("src/Tests.cs", Some(before), None),
                ("src/Helpers.cs", None, Some(after)),
            ],
        )
    }

    // complexity-analysis: "A long moved function changed at both ends".
    // The two versions differ over more than 8,192 tokens each, so they
    // are not compared, and `Helpers.Run(int)` is new.
    #[test]
    fn a_long_moved_function_changed_at_both_ends() {
        let findings = moved_run(
            &long_run("Tests", "d += 1", "d += 2"),
            &long_run("Helpers", "d -= 1", "d -= 2"),
        );
        let mut expected = new(Severity::Warning, "Helpers.Run(int)", 5, 12);
        expected.location.path = "src/Helpers.cs".into();
        assert_eq!(findings, vec![expected]);
    }

    // complexity-analysis: "A long moved function changed in one place".
    // The fingerprints differ; the common start and end leave a middle
    // of one token, so the code evidence matches the two.
    #[test]
    fn a_long_moved_function_changed_in_one_place() {
        let findings = moved_run(
            &long_run("Tests", "d += 1", "d += 2"),
            &long_run("Helpers", "d -= 1", "d += 2"),
        );
        assert_eq!(findings, vec![]);
    }

    // complexity-analysis: "A long function moved unchanged". The same
    // code matches without code evidence.
    #[test]
    fn a_long_function_moved_unchanged() {
        let findings = moved_run(
            &long_run("Tests", "d += 1", "d += 2"),
            &long_run("Helpers", "d += 1", "d += 2"),
        );
        assert_eq!(findings, vec![]);
    }

    // complexity-analysis: "Short removed namesake". A one-line removed
    // `ngOnInit` is no evidence for a long new one that contains it.
    #[test]
    fn short_removed_namesake_is_no_evidence() {
        let findings = run_files(
            Language::TypeScript,
            &[
                (
                    "src/a.component.ts",
                    Some(moved!("short", "before/a.component.ts")),
                    None,
                ),
                (
                    "src/b.component.ts",
                    None,
                    Some(moved!("short", "after/b.component.ts")),
                ),
            ],
        );
        let mut expected = new(Severity::Warning, "BComponent.ngOnInit", 2, 14);
        expected.location.path = "src/b.component.ts".into();
        assert_eq!(findings, vec![expected]);
    }

    // Design D2: the second pass sees only what the first left. `C.load`
    // has the code of `A.load`; `D.load` has evidence for both `A.load`
    // and `B.load`, so it pairs with `B.load` only once `A.load` is taken.
    #[test]
    fn evidence_only_settles_what_the_same_code_left() {
        let mut files = vec![
            ("src/a.ts", Some(moved!("ambiguous", "before/a.ts")), None),
            ("src/b.ts", Some(moved!("ambiguous", "before/b.ts")), None),
            ("src/c.ts", None, Some(moved!("leftovers", "after/c.ts"))),
            ("src/d.ts", None, Some(moved!("leftovers", "after/d.ts"))),
        ];
        assert_eq!(run_files(Language::TypeScript, &files), vec![]);
        files.reverse();
        assert_eq!(run_files(Language::TypeScript, &files), vec![]);
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

    // complexity-analysis: "Moved functions", a larger group: `C.load`
    // has the code of `A.load` (12), `D.load` that of `B.load` (13), so
    // both pair up and nothing is reported; swapped, `D.load` would be
    // 12 -> 13. Design D2: the result does not depend on file order.
    #[test]
    fn a_group_forms_two_pairs_in_any_order() {
        let mut files = vec![
            ("src/a.ts", Some(moved!("pairs", "before/a.ts")), None),
            ("src/b.ts", Some(moved!("pairs", "before/b.ts")), None),
            ("src/c.ts", None, Some(moved!("pairs", "after/c.ts"))),
            ("src/d.ts", None, Some(moved!("pairs", "after/d.ts"))),
        ];
        assert_eq!(run_files(Language::TypeScript, &files), vec![]);
        files.reverse();
        assert_eq!(run_files(Language::TypeScript, &files), vec![]);
    }

    // complexity-analysis: "Moved functions", "when neither has another
    // such partner": `C.load` and `D.load` both have the code of the one
    // removed `A.load`, so neither is matched, and a unit is matched at
    // most once (design D2).
    #[test]
    fn two_claimants_of_one_removed_unit_are_new() {
        let mut files = vec![
            ("src/a.ts", Some(moved!("claimants", "before/a.ts")), None),
            ("src/c.ts", None, Some(moved!("claimants", "after/c.ts"))),
            ("src/d.ts", None, Some(moved!("claimants", "after/d.ts"))),
        ];
        let at = |path: &str| {
            let mut finding = new(Severity::Warning, "", 2, 12);
            finding.location.path = path.into();
            finding.message = format!(
                "{}.load: new function, cyclomatic complexity 12",
                if path == "src/c.ts" { "C" } else { "D" }
            );
            finding
        };
        assert_eq!(
            run_files(Language::TypeScript, &files),
            vec![at("src/c.ts"), at("src/d.ts")]
        );
        files.reverse();
        assert_eq!(
            run_files(Language::TypeScript, &files),
            vec![at("src/d.ts"), at("src/c.ts")]
        );
    }

    macro_rules! inlined {
        ($file:literal) => {
            include_str!(concat!("../fixtures/complexity/inlined/", $file))
        };
    }

    // complexity-analysis: "Inlined behind a feature flag".
    // `decryptMany` (4) holds the code of the removed `decryptManyV2`
    // (10) and no more: 12 does not exceed 4 + 9.
    #[test]
    fn inlined_behind_a_feature_flag() {
        assert_eq!(
            run(inlined!("before.ts"), inlined!("after_flag.ts")),
            vec![]
        );
    }

    // complexity-analysis: "Inlined and grown beyond it" and "Inlined
    // function message". Three more `if` statements: 15 is 2 beyond
    // 4 + 9, a Warning, while the metric keeps 4 and 15.
    #[test]
    fn inlined_and_grown_beyond_it() {
        let mut expected = increase(Severity::Warning, "CipherService.decryptMany", 2, 4, 15);
        expected.message = "CipherService.decryptMany: cyclomatic complexity 4 -> 15 \
                            (+11, +2 beyond the inlined CipherService.decryptManyV2)"
            .to_owned();
        assert_eq!(
            run(inlined!("before.ts"), inlined!("after_grown.ts")),
            vec![expected]
        );
    }

    // complexity-analysis: "Removed but not inlined". `legacy` is gone,
    // but `process` grew by other code.
    #[test]
    fn removed_but_not_inlined() {
        assert_eq!(
            run(
                inlined!("not_inlined_before.ts"),
                inlined!("not_inlined_after.ts")
            ),
            vec![increase(Severity::Error, "process", 1, 3, 8)]
        );
    }

    // complexity-analysis: "Removed code that was already there". The
    // tokens of `helper` stand in `handle`, but in its old code: what the
    // change added to `handle` is one other `if` statement.
    #[test]
    fn removed_code_that_was_already_there() {
        assert_eq!(
            run(
                inlined!("already_there_before.ts"),
                inlined!("already_there_after.ts")
            ),
            vec![increase(Severity::Warning, "handle", 1, 3, 4)]
        );
    }

    // complexity-analysis: "Inlined among much new code". `process`
    // holds all of `helper` (7), more than two thirds of its tokens, but
    // that is less than two thirds of the code added to `process`, so
    // nothing is credited. Credited, the growth would be 3, a Warning.
    #[test]
    fn inlined_among_much_new_code() {
        assert_eq!(
            run(
                inlined!("among_new_before.ts"),
                inlined!("among_new_after.ts")
            ),
            vec![increase(Severity::Error, "process", 1, 3, 12)]
        );
    }

    // Design D3: a removed unit counts as inlined into one unit, the one
    // with the longer common subsequence, though it comes later. `a` and
    // `b` both hold all five checks of `helper` (6), but `a` pushes another
    // value in the last one, so only `b` is credited and `a` keeps its
    // whole increase.
    #[test]
    fn a_removed_unit_is_credited_once() {
        assert_eq!(
            run(inlined!("two_before.ts"), inlined!("two_after.ts")),
            vec![increase(Severity::Error, "a", 1, 2, 7)]
        );
    }

    /// TypeScript `if` statements, one per name, each pushing its name.
    fn checks(names: &[&str]) -> String {
        names.iter().fold(String::new(), |mut checks, name| {
            let _ = writeln!(checks, "    if (input.{name}) {{ out.push('{name}'); }}");
            checks
        })
    }

    /// A TypeScript method of `Input`, returning the `out` its `body` fills.
    fn method(name: &str, body: &str) -> String {
        format!(
            "  {name}(input: Input): string[] {{\n    const out: string[] = [];\n{body}    return out;\n  }}\n"
        )
    }

    // Design D3: a grown unit is credited with one removed unit at most.
    // `a.ts` and `b.ts` each inline their own copy of the same helper; the
    // copies are alike, yet each `run` is credited with one.
    #[test]
    fn each_grown_unit_takes_one_removed_unit() {
        let helper = checks(&["alpha", "beta", "gamma", "delta", "epsilon"]);
        let start = "    if (input.empty) { return out; }\n";
        let before = format!(
            "export class Job {{\n{}\n{}}}\n",
            method("run", start),
            method("helper", &helper)
        );
        let after = format!(
            "export class Job {{\n{}}}\n",
            method("run", &format!("{start}{helper}"))
        );
        let files = [
            ("src/a.ts", Some(before.as_str()), Some(after.as_str())),
            ("src/b.ts", Some(before.as_str()), Some(after.as_str())),
        ];
        assert_eq!(run_files(Language::TypeScript, &files), vec![]);
    }

    // Design D3: a removed unit is credited once, and so is a grown unit.
    // `process` takes the code of one of two alike checks; the other one's
    // decision points do not hide the five of the new condition.
    #[test]
    fn one_of_two_alike_removed_units() {
        let check = checks(&["alpha", "beta", "gamma", "delta", "epsilon"]);
        let start = "    if (input.empty) { return out; }\n";
        let before = format!(
            "export class Job {{\n{}\n{}\n{}}}\n",
            method("process", start),
            method("checkA", &check),
            method("checkB", &check)
        );
        let condition =
            "    if (input.a && input.b && input.c && input.d && input.e) { return out; }\n";
        let after = format!(
            "export class Job {{\n{}}}\n",
            method("process", &format!("{start}{check}{condition}"))
        );
        let mut expected = increase(Severity::Error, "Job.process", 2, 2, 12);
        expected.message = "Job.process: cyclomatic complexity 2 -> 12 \
                            (+10, +5 beyond the inlined Job.checkA)"
            .to_owned();
        assert_eq!(run(&before, &after), vec![expected]);
    }

    // Design D3 and D4: a moved unit's added code is measured against the
    // unit it moved from, and its message has both forms.
    #[test]
    fn moved_and_inlined() {
        let names = [
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o",
        ];
        let body = checks(&names);
        let helper = checks(&["alpha", "beta", "gamma", "delta", "epsilon"]);
        let old = format!(
            "export class A {{\n{}\n{}}}\n",
            method("run", &body),
            method("helper", &helper)
        );
        let new = format!(
            "export class B {{\n{}}}\n",
            method(
                "run",
                &format!("{body}{helper}    if (input.zeta) {{ return out; }}\n")
            )
        );
        let files = [
            ("src/a.ts", Some(old.as_str()), None),
            ("src/b.ts", None, Some(new.as_str())),
        ];
        let mut expected = increase(Severity::Warning, "B.run", 2, 16, 22);
        expected.location.path = "src/b.ts".into();
        expected.message = "B.run: cyclomatic complexity 16 -> 22 \
                            (+6, +1 beyond the inlined A.helper), moved from A.run"
            .to_owned();
        assert_eq!(run_files(Language::TypeScript, &files), vec![expected]);
    }

    // Design D3: a unit a move took is no longer removed, so it is not
    // credited as inlined, though `run` gained the same code.
    #[test]
    fn a_moved_unit_is_not_inlined() {
        let load = checks(&["alpha", "beta", "gamma", "delta"]);
        let start = "    if (input.empty) { return out; }\n";
        let a = format!("export class A {{\n{}}}\n", method("load", &load));
        let b_before = format!("export class B {{\n{}}}\n", method("run", start));
        let b_after = format!(
            "export class B {{\n{}\n{}}}\n",
            method("load", &load),
            method("run", &format!("{start}{load}"))
        );
        let files = [
            ("src/a.ts", Some(a.as_str()), None),
            ("src/b.ts", Some(b_before.as_str()), Some(b_after.as_str())),
        ];
        let mut expected = increase(Severity::Warning, "B.run", 11, 2, 6);
        expected.location.path = "src/b.ts".into();
        assert_eq!(run_files(Language::TypeScript, &files), vec![expected]);
    }

    // Design D3: when two grown units hold the same share of a removed
    // one, the first by path is credited, whatever the order of files.
    #[test]
    fn a_tie_goes_to_the_first_path() {
        let checks = (0..5).fold(String::new(), |mut checks, i| {
            let _ = writeln!(checks, "  if (input.flag{i}) {{ out.push('flag {i}'); }}");
            checks
        });
        let run_fn = |body: &str| {
            format!(
                "export function run(input: Input): string[] {{\n  const out: string[] = [];\n  if (input.empty) {{ return out; }}\n{body}  return out;\n}}\n"
            )
        };
        let helper = format!(
            "export function helper(input: Input): string[] {{\n  const out: string[] = [];\n{checks}  return out;\n}}\n"
        );
        let (before, after) = (run_fn(""), run_fn(&checks));
        let mut files = vec![
            ("src/a.ts", Some(before.as_str()), Some(after.as_str())),
            ("src/b.ts", Some(before.as_str()), Some(after.as_str())),
            ("src/c.ts", Some(helper.as_str()), None),
        ];
        let mut expected = increase(Severity::Error, "run", 1, 2, 7);
        expected.location.path = "src/b.ts".into();
        assert_eq!(
            run_files(Language::TypeScript, &files),
            vec![expected.clone()]
        );
        files.reverse();
        assert_eq!(run_files(Language::TypeScript, &files), vec![expected]);
    }

    // complexity-analysis: a removed unit counts as inlined only with
    // decision points. `helper` (1) went into `process` whole, yet the
    // message names nothing inlined.
    #[test]
    fn a_unit_without_decision_points_is_not_credited() {
        let helper = "  const total = input.items.length * input.price;\n  const tax = total * input.rate;\n  out.push(String(total + tax));\n";
        let before = format!(
            "export function process(input: Input): string[] {{\n  const out: string[] = [];\n  if (input.empty) {{ return out; }}\n  return out;\n}}\n\nexport function helper(input: Input, out: string[]): void {{\n{helper}}}\n"
        );
        let after = format!(
            "export function process(input: Input): string[] {{\n  const out: string[] = [];\n  if (input.empty) {{ return out; }}\n{helper}  if (input.gift) {{ out.push('gift'); }}\n  return out;\n}}\n"
        );
        assert_eq!(
            run(&before, &after),
            vec![increase(Severity::Warning, "process", 1, 2, 3)]
        );
    }

    // Design D3: the code a change added is what lies outside a longest
    // common subsequence with the before version.
    #[test]
    fn added_code() {
        let added = |before: &[usize], after: &[usize]| added(before, after).unwrap();
        assert_eq!(added(&[1, 2, 3, 4], &[1, 5, 2, 3, 6, 4]), vec![5, 6]);
        assert_eq!(added(&[1, 2, 3], &[3, 1, 2]), vec![3]);
        assert_eq!(added(&[1, 2], &[1, 2]), Vec::<usize>::new());
        assert_eq!(added(&[], &[1, 2]), vec![1, 2]);
        assert_eq!(added(&[1, 2, 3], &[]), Vec::<usize>::new());
        assert_eq!(added(&[7, 1, 7, 2, 7], &[1, 8, 2, 9]), vec![8, 9]);
        // A tie keeps the after token: of the two `1`s, the later is added.
        assert_eq!(added(&[1], &[1, 1]), vec![1]);
        assert_eq!(added(&[2, 1, 3], &[2, 1, 4, 1, 3]), vec![4, 1]);
        // A tie in the middle: matching `2` rather than `1` adds the `1`.
        assert_eq!(added(&[1, 2], &[3, 2, 1, 3]), vec![3, 1, 3]);
    }

    // Design D3: above 2^26 pairs of tokens, the alignment is not made,
    // so the unit is credited with nothing. The common start and end do
    // not count.
    #[test]
    fn added_code_has_a_limit() {
        let before: Vec<usize> = (0..8193).collect();
        let after: Vec<usize> = (1..8194).collect();
        assert_eq!(added(&before, &after), None);
        let mark = vec![usize::MAX; 100_000];
        let wrapped = [&mark[..], &after[..10], &mark[..]].concat();
        let around = [&mark[..], &before[..10], &mark[..]].concat();
        assert_eq!(added(&around, &wrapped), Some(vec![10]));
    }

    // Design D3: above 2^26 pairs of tokens, a removed unit and the added
    // code are not compared, so the pair is not credited.
    #[test]
    fn evidence_has_a_limit() {
        assert_eq!(evidence(&[1, 2, 3], &[1, 2, 3]), Some(3));
        let long = vec![1; 8193];
        assert_eq!(evidence(&long, &long), None);
    }

    // Design D3: a removed unit of more than 8,192 own tokens is not
    // compared with the code added to a unit, so it is not credited, and
    // the increase is reported in full.
    #[test]
    fn a_very_long_inlined_function() {
        let helper = format!(
            "{}{}",
            checks(&["alpha", "beta", "gamma", "delta", "epsilon"]),
            "    out.push(input.x);\n".repeat(1200)
        );
        let start = "    if (input.empty) { return out; }\n";
        let before = format!(
            "export class Job {{\n{}\n{}}}\n",
            method("process", start),
            method("helper", &helper)
        );
        let after = format!(
            "export class Job {{\n{}}}\n",
            method("process", &format!("{start}{helper}"))
        );
        assert_eq!(
            run(&before, &after),
            vec![increase(Severity::Error, "Job.process", 2, 2, 7)]
        );
    }

    // Design D3: one long middle is enough for the limit. `process` loses
    // a long body and takes a helper's code; its after version is small
    // enough to compare with the helper, yet nothing is credited.
    #[test]
    fn a_long_function_that_shrank_around_an_inlined_one() {
        let helper = format!(
            "{}{}",
            checks(&["alpha", "beta", "gamma", "delta", "epsilon"]),
            "    out.push(input.y);\n".repeat(800)
        );
        let process = |first: &str, middle: &str, last: &str| {
            let first = format!("    if (input.{first}) {{ return out; }}\n");
            let last = format!("    if (input.{last}) {{ return out; }}\n");
            method("process", &format!("{first}{middle}{last}"))
        };
        let before = format!(
            "export class Job {{\n{}\n{}}}\n",
            process("first", &"    out.push(input.x);\n".repeat(1500), "last"),
            method("helper", &helper)
        );
        let after = format!(
            "export class Job {{\n{}}}\n",
            process("start", &helper, "end")
        );
        assert_eq!(
            run(&before, &after),
            vec![increase(Severity::Error, "Job.process", 2, 3, 8)]
        );
    }

    // Design D3: a unit whose two versions differ at both ends of more
    // than 8,192 tokens is not aligned, so the helper it inlined is not
    // credited and its increase is reported in full.
    #[test]
    fn a_long_function_changed_at_both_ends() {
        let helper = checks(&["alpha", "beta", "gamma", "delta", "epsilon"]);
        let body = "    out.push(input.x);\n".repeat(1200);
        let process = |first: &str, middle: &str, last: &str| {
            let first = format!("    if (input.{first}) {{ return out; }}\n");
            let last = format!("    if (input.{last}) {{ return out; }}\n");
            method("process", &format!("{first}{middle}{last}"))
        };
        let before = format!(
            "export class Job {{\n{}\n{}}}\n",
            process("first", &body, "last"),
            method("helper", &helper)
        );
        let after = format!(
            "export class Job {{\n{}}}\n",
            process("start", &format!("{body}{helper}"), "end")
        );
        assert_eq!(
            run(&before, &after),
            vec![increase(Severity::Error, "Job.process", 2, 3, 8)]
        );
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
        // C# operators: their `<` and `>` are no brackets.
        assert_eq!(
            own_name("Cart.operator <(Cart, Cart).Local(int)"),
            "Local(int)"
        );
        assert_eq!(own_name("Cart.operator <<(Cart, int).Local()"), "Local()");
        assert_eq!(own_name("Cart.operator >>>(Cart, int).Local()"), "Local()");
        assert_eq!(
            own_name("Cart.operator <=(Cart, Cart)"),
            "operator <=(Cart, Cart)"
        );
        assert_eq!(own_name("Box<dyn Fn() -> u8>.call"), "call");
        assert_eq!(own_name("total"), "total");
    }

    #[test]
    fn identical_files_have_no_findings() {
        let source = include_str!("../fixtures/complexity/legacy_growth/after.ts");
        assert_eq!(run(source, source), vec![]);
    }
}
