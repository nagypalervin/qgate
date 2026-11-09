//! Blocks of code that a change newly duplicates.
//!
//! A repository-level analysis, not an [`Analyzer`](crate::Analyzer): it
//! compares the changed files with every other file of the head revision.
//! The caller builds a [`DuplicationIndex`] from the changed files, feeds
//! it the other head files one by one, and asks for the findings, so only
//! the changed files are kept in memory (design D3 of
//! `add-duplicate-detection`).
//!
//! - **Tokens:** code is compared as the normalized tokens of `qgate-lang`,
//!   so a copy with renamed variables matches.
//! - **Duplicates:** a duplicate block is a run of at least [`MIN_TOKENS`]
//!   tokens that occurs twice or more.
//! - **New duplication:** it is new when the changed files hold more
//!   occurrences after the change than before; unchanged files are the
//!   same in both revisions, so that is exactly the growth in the whole
//!   repository.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use qgate_core::{Finding, Location, Severity};
use qgate_lang::Token;

/// Rule id of a newly duplicated block.
pub const RULE: &str = "duplication/duplicate-block";

/// The smallest duplicate block, in normalized tokens.
pub const MIN_TOKENS: usize = 50;

/// One changed file: its path in the head revision and the tokens of both
/// versions (empty before, for an added file).
#[derive(Debug, Clone, Copy)]
pub struct ChangedSource<'a> {
    /// Path in the head revision, as findings report it.
    pub path: &'a str,
    /// Tokens of the base version.
    pub before: &'a [Token],
    /// Tokens of the head version.
    pub after: &'a [Token],
}

/// A window's identity: two independent 64-bit hashes of its token texts,
/// so two different windows practically never share one.
type Key = (u64, u64);

/// Where one window occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    /// Index into [`DuplicationIndex::paths`].
    path: usize,
    /// Index of the window's first token.
    start: usize,
    first_line: u32,
    last_line: u32,
}

/// Counts and places of the windows that the changed files contain.
#[derive(Debug, Clone)]
pub struct DuplicationIndex {
    /// Paths of the changed files first (in input order), then the others.
    paths: Vec<String>,
    /// The windows of each changed file's head version, by start.
    changed: Vec<Vec<(Key, Place)>>,
    /// Per changed file and key: occurrences in the base version.
    before: HashMap<(usize, Key), usize>,
    /// Per key: occurrences in the base versions of all changed files.
    before_total: HashMap<Key, usize>,
    /// Per key that a changed file contains: every place in the head
    /// revision, changed files included.
    places: HashMap<Key, Vec<Place>>,
    /// The token hash; replaceable in tests to force collisions.
    token_hash: fn(&str) -> u64,
}

impl DuplicationIndex {
    /// An index of the changed files. Their head versions count as head
    /// files: do not [`add`](Self::add) them again.
    #[must_use]
    pub fn new(changes: &[ChangedSource<'_>]) -> Self {
        Self::with_token_hash(changes, fnv1a)
    }

    fn with_token_hash(changes: &[ChangedSource<'_>], token_hash: fn(&str) -> u64) -> Self {
        let mut index = Self {
            paths: Vec::new(),
            changed: Vec::new(),
            before: HashMap::new(),
            before_total: HashMap::new(),
            places: HashMap::new(),
            token_hash,
        };
        for (file, change) in changes.iter().enumerate() {
            index.paths.push(change.path.to_owned());
            let windows = index.windows(file, change.after);
            for &(key, place) in &windows {
                index.places.entry(key).or_default().push(place);
            }
            index.changed.push(windows);
        }
        for (file, change) in changes.iter().enumerate() {
            let mut by_key: HashMap<Key, Vec<Place>> = HashMap::new();
            for (key, place) in index.windows(file, change.before) {
                by_key.entry(key).or_default().push(place);
            }
            for (key, places) in by_key {
                let count = apart(&places).len();
                index.before.insert((file, key), count);
                *index.before_total.entry(key).or_default() += count;
            }
        }
        index
    }

    /// Add one head file that the change did not touch.
    pub fn add(&mut self, path: &str, tokens: &[Token]) {
        if tokens.len() < MIN_TOKENS {
            return;
        }
        let file = self.paths.len();
        let mut added = false;
        for (key, place) in self.windows(file, tokens) {
            if let Some(places) = self.places.get_mut(&key) {
                places.push(place);
                added = true;
            }
        }
        // Keep the path only if it holds a place, to bound memory.
        if added {
            self.paths.push(path.to_owned());
        } else {
            self.paths.push(String::new());
        }
    }

    /// The `duplication/duplicate-block` findings, in changed-file order
    /// and then by line.
    #[must_use]
    pub fn findings(&self) -> Vec<Finding> {
        let reported = self.reported();
        let mut lookup = Lookup::default();
        let mut findings = Vec::new();
        for (file, windows) in self.changed.iter().enumerate() {
            for (first, last) in runs(windows, &reported, file) {
                findings.extend(self.blocks(file, first, last, &reported, &mut lookup));
            }
        }
        findings
    }

    /// The occurrences in changed files that the change added and that are
    /// reported (design D3, step 4), as (changed file, window start).
    fn reported(&self) -> BTreeSet<(usize, usize)> {
        let mut reported = BTreeSet::new();
        for (key, places) in &self.places {
            let places = apart(places);
            // A fast path: one place has no copy (the original rule below
            // would report nothing either).
            if places.len() < 2 {
                continue;
            }
            // Occurrences per changed file, in path and position order.
            let mut by_file: BTreeMap<(&str, usize), Vec<usize>> = BTreeMap::new();
            let mut in_changed: usize = 0;
            for place in places.iter().filter(|p| p.path < self.changed.len()) {
                by_file
                    .entry((&self.paths[place.path], place.path))
                    .or_default()
                    .push(place.start);
                in_changed += 1;
            }
            // Every changed file counts, also one the block left.
            let before_total = self.before_total.get(key).copied().unwrap_or(0);
            // How many occurrences the change added in all: code that only
            // moved between changed files adds none.
            let mut budget = in_changed.saturating_sub(before_total);
            if budget == 0 {
                continue;
            }
            let mut old = places.len() - in_changed;
            let mut new = Vec::new();
            for (&(_, file), starts) in &by_file {
                let kept = self.before.get(&(file, *key)).copied().unwrap_or(0);
                old += kept.min(starts.len());
                new.extend(starts.iter().skip(kept).map(|&start| (file, start)));
            }
            // With no copy older than the change, the first counts as the
            // original.
            let skip = usize::from(old == 0);
            for occurrence in new.into_iter().skip(skip) {
                if budget == 0 {
                    break;
                }
                reported.insert(occurrence);
                budget -= 1;
            }
        }
        reported
    }

    /// The findings for a run of reported windows `first..=last` of a
    /// changed file. A block extends only as far as one copy elsewhere
    /// continues it, so each finding names a copy of the whole block; where
    /// no copy continues, a new block starts.
    fn blocks(
        &self,
        file: usize,
        first: usize,
        last: usize,
        reported: &BTreeSet<(usize, usize)>,
        lookup: &mut Lookup,
    ) -> Vec<Finding> {
        let windows = &self.changed[file];
        let mut findings = Vec::new();
        let mut start = first;
        while start <= last {
            let (key, place) = windows[start];
            // Copies of the first window: apart from each other and not
            // reported. This occurrence is one of the places `apart` keeps
            // (only kept places are reported), so the reported filter also
            // removes it, and `apart` keeps no place overlapping it.
            let mut copies: Vec<Place> = apart(&self.places[&key])
                .into_iter()
                .filter(|p| !(p.path < self.changed.len() && reported.contains(&(p.path, p.start))))
                .collect();
            if copies.is_empty() {
                start += 1;
                continue;
            }
            // Extend while at least one copy continues the block.
            let mut end = start;
            while end < last {
                let offset = end + 1 - start;
                let (next_key, _) = windows[end + 1];
                let continuing: Vec<Place> = copies
                    .iter()
                    .copied()
                    .filter(|c| lookup.contains(self, next_key, c.path, c.start + offset))
                    .collect();
                if continuing.is_empty() {
                    break;
                }
                copies = continuing;
                end += 1;
            }
            copies.sort_by_key(|c| (&self.paths[c.path], c.first_line, c.start));
            let counterpart = copies[0];
            let offset = end - start;
            let counterpart_last_line = lookup
                .last_line(
                    self,
                    windows[end].0,
                    counterpart.path,
                    counterpart.start + offset,
                )
                .unwrap_or(counterpart.last_line);
            let more = match copies.len() - 1 {
                0 => String::new(),
                k => format!(", and {k} more"),
            };
            findings.push(Finding {
                rule: RULE.to_owned(),
                severity: Severity::Warning,
                message: format!(
                    "duplicates {}:{}-{} (lines {}-{}, {} tokens{more})",
                    self.paths[counterpart.path],
                    counterpart.first_line,
                    counterpart_last_line,
                    place.first_line,
                    windows[end].1.last_line,
                    offset + MIN_TOKENS,
                ),
                location: Location {
                    path: PathBuf::from(&self.paths[file]),
                    line: Some(place.first_line),
                },
                metric: None,
            });
            start = end + 1;
        }
        findings
    }

    /// Every window of `tokens` with its key and place.
    fn windows(&self, file: usize, tokens: &[Token]) -> Vec<(Key, Place)> {
        if tokens.len() < MIN_TOKENS {
            return Vec::new();
        }
        let hashes: Vec<(u64, u64)> = tokens
            .iter()
            .map(|t| {
                (
                    (self.token_hash)(&t.text),
                    fnv1a_seeded(&t.text, SECOND_SEED),
                )
            })
            .collect();
        let mut windows = Vec::with_capacity(tokens.len() - MIN_TOKENS + 1);
        let mut rolling = (Rolling::new(BASE_A), Rolling::new(BASE_B));
        for (i, &(a, b)) in hashes.iter().enumerate() {
            rolling
                .0
                .push(a, hashes.get(i.wrapping_sub(MIN_TOKENS)).map(|h| h.0));
            rolling
                .1
                .push(b, hashes.get(i.wrapping_sub(MIN_TOKENS)).map(|h| h.1));
            if i + 1 >= MIN_TOKENS {
                let start = i + 1 - MIN_TOKENS;
                windows.push((
                    (rolling.0.value, rolling.1.value),
                    Place {
                        path: file,
                        start,
                        first_line: tokens[start].line,
                        last_line: tokens[i].line,
                    },
                ));
            }
        }
        windows
    }
}

/// Places of a key by (file, start), built on first use, to follow a copy
/// window by window.
#[derive(Default)]
struct Lookup {
    by_key: HashMap<Key, HashMap<(usize, usize), u32>>,
}

impl Lookup {
    fn last_line(
        &mut self,
        index: &DuplicationIndex,
        key: Key,
        path: usize,
        start: usize,
    ) -> Option<u32> {
        self.by_key
            .entry(key)
            .or_insert_with(|| {
                index.places[&key]
                    .iter()
                    .map(|p| ((p.path, p.start), p.last_line))
                    .collect()
            })
            .get(&(path, start))
            .copied()
    }

    fn contains(&mut self, index: &DuplicationIndex, key: Key, path: usize, start: usize) -> bool {
        self.last_line(index, key, path, start).is_some()
    }
}

/// The places that count as separate copies: within one file, a place
/// that overlaps the previous one kept is the same code seen shifted (in
/// repetitive code a window can equal itself moved by one statement), not
/// a copy. Sorted by file and start.
fn apart(places: &[Place]) -> Vec<Place> {
    let mut sorted = places.to_vec();
    sorted.sort_by_key(|p| (p.path, p.start));
    let mut kept: Vec<Place> = Vec::with_capacity(sorted.len());
    for place in sorted {
        match kept.last() {
            Some(last) if last.path == place.path && place.start < last.start + MIN_TOKENS => {}
            _ => kept.push(place),
        }
    }
    kept
}

/// Runs of consecutive reported window starts in a changed file, as
/// indices into its windows: each run is one block.
fn runs(
    windows: &[(Key, Place)],
    reported: &BTreeSet<(usize, usize)>,
    file: usize,
) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (i, (_, place)) in windows.iter().enumerate() {
        if !reported.contains(&(file, place.start)) {
            continue;
        }
        match runs.last_mut() {
            Some((_, last)) if *last + 1 == i => *last = i,
            _ => runs.push((i, i)),
        }
    }
    runs
}

const BASE_A: u64 = 0x0100_0000_01b3;
const BASE_B: u64 = 0x9e37_79b9_7f4a_7c15;
const SECOND_SEED: u64 = 0x6a09_e667_f3bc_c908;

/// A polynomial hash of the last [`MIN_TOKENS`] token hashes, updated in
/// constant time per token, in wrapping `u64` arithmetic.
struct Rolling {
    base: u64,
    /// `base` to the power `MIN_TOKENS - 1`, to remove the oldest token.
    top: u64,
    value: u64,
}

impl Rolling {
    fn new(base: u64) -> Self {
        let top = (1..MIN_TOKENS).fold(1u64, |acc, _| acc.wrapping_mul(base));
        Self {
            base,
            top,
            value: 0,
        }
    }

    fn push(&mut self, token: u64, leaving: Option<u64>) {
        if let Some(old) = leaving {
            self.value = self.value.wrapping_sub(old.wrapping_mul(self.top));
        }
        self.value = self.value.wrapping_mul(self.base).wrapping_add(token);
    }
}

/// FNV-1a, 64 bit: fixed, so results do not change between runs.
fn fnv1a(text: &str) -> u64 {
    fnv1a_seeded(text, 0xcbf2_9ce4_8422_2325)
}

fn fnv1a_seeded(text: &str, seed: u64) -> u64 {
    text.bytes().fold(seed, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use qgate_lang::{Language, parse};

    use super::*;

    fn tokens(source: &str) -> Vec<Token> {
        parse(Language::TypeScript, source).expect("parses").tokens
    }

    /// Synthetic tokens `t0`, `t1`, ... on line 1, then `texts` one per
    /// line from line 2.
    fn synthetic(texts: &[String]) -> Vec<Token> {
        texts
            .iter()
            .enumerate()
            .map(|(i, text)| Token {
                text: text.clone(),
                line: u32::try_from(i).expect("small") + 1,
            })
            .collect()
    }

    fn texts(prefix: &str, n: usize) -> Vec<String> {
        (0..n).map(|i| format!("{prefix}{i}")).collect()
    }

    /// Run the index: `changes` as (path, before, after) token lists, then
    /// `others` as unchanged head files; returns (path, line, message).
    fn run(
        changes: &[(&str, Vec<Token>, Vec<Token>)],
        others: &[(&str, Vec<Token>)],
    ) -> Vec<(String, u32, String)> {
        run_with(changes, others, fnv1a)
    }

    fn run_with(
        changes: &[(&str, Vec<Token>, Vec<Token>)],
        others: &[(&str, Vec<Token>)],
        hash: fn(&str) -> u64,
    ) -> Vec<(String, u32, String)> {
        let sources: Vec<ChangedSource<'_>> = changes
            .iter()
            .map(|(path, before, after)| ChangedSource {
                path,
                before,
                after,
            })
            .collect();
        let mut index = DuplicationIndex::with_token_hash(&sources, hash);
        for (path, tokens) in others {
            index.add(path, tokens);
        }
        index
            .findings()
            .into_iter()
            .map(|f| {
                assert_eq!(f.rule, RULE);
                assert_eq!(f.severity, Severity::Warning);
                (
                    f.location.path.to_string_lossy().into_owned(),
                    f.location.line.expect("a line"),
                    f.message,
                )
            })
            .collect()
    }

    const DATE: &str = include_str!("../fixtures/duplication/date_helper.ts");
    const COPY: &str = include_str!("../fixtures/duplication/renamed_copy.ts");
    const OTHER: &str = include_str!("../fixtures/duplication/other_code.ts");

    // Requirement: Compared files, Duplicate block finding

    #[test]
    fn renamed_copy_of_an_unchanged_file() {
        assert_eq!(
            run(
                &[("src/orders/format.ts", vec![], tokens(COPY))],
                &[
                    ("src/utils/date.ts", tokens(DATE)),
                    ("src/x.ts", tokens(OTHER))
                ],
            ),
            [(
                "src/orders/format.ts".to_owned(),
                4,
                "duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn several_existing_copies() {
        let found = run(
            &[("src/orders/format.ts", vec![], tokens(COPY))],
            &[
                ("src/utils/date.ts", tokens(DATE)),
                ("src/b.ts", tokens(DATE)),
                ("src/a.ts", tokens(DATE)),
            ],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].2,
            "duplicates src/a.ts:1-6 (lines 4-9, 68 tokens, and 2 more)"
        );
    }

    // Requirement: Duplicate blocks

    #[test]
    fn threshold_is_fifty_tokens() {
        for (n, expected) in [(49, 0), (50, 1)] {
            let block = texts("t", n);
            let found = run(
                &[("new.ts", vec![], synthetic(&block))],
                &[("old.ts", synthetic(&block))],
            );
            assert_eq!(found.len(), expected, "{n} tokens");
        }
    }

    #[test]
    fn a_block_is_a_maximal_run_with_its_lines() {
        // 70 shared tokens surrounded by different ones.
        let shared = texts("s", 70);
        let mut new = texts("a", 5);
        new.extend(shared.iter().cloned());
        new.extend(texts("b", 5));
        let mut old = texts("c", 10);
        old.extend(shared.iter().cloned());
        assert_eq!(
            run(
                &[("new.ts", vec![], synthetic(&new))],
                &[("old.ts", synthetic(&old))]
            ),
            [(
                "new.ts".to_owned(),
                6,
                "duplicates old.ts:11-80 (lines 6-75, 70 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn a_duplicate_within_one_file() {
        let block = texts("s", 60);
        let mut before = block.clone();
        before.extend(texts("x", 3));
        let mut after = before.clone();
        after.extend(block.iter().cloned());
        assert_eq!(
            run(&[("a.ts", synthetic(&before), synthetic(&after))], &[]),
            [(
                "a.ts".to_owned(),
                64,
                "duplicates a.ts:1-60 (lines 64-123, 60 tokens)".to_owned()
            )]
        );
    }

    // Requirement: Only new duplication is reported

    #[test]
    fn existing_duplication_is_not_reported() {
        let block = texts("s", 60);
        let mut before = block.clone();
        before.push("x".to_owned());
        let mut after = block.clone();
        after.push("y".to_owned());
        assert_eq!(
            run(
                &[("a.ts", synthetic(&before), synthetic(&after))],
                &[("b.ts", synthetic(&block))]
            ),
            []
        );
    }

    #[test]
    fn moved_code_is_not_reported() {
        let block = texts("s", 60);
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&block), vec![]),
                    ("b.ts", vec![], synthetic(&block))
                ],
                &[]
            ),
            []
        );
    }

    #[test]
    fn moved_next_to_an_existing_copy_is_not_reported() {
        // The block leaves a.ts and arrives in b.ts; c.ts keeps its copy.
        let block = texts("s", 60);
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&block), vec![]),
                    ("b.ts", vec![], synthetic(&block))
                ],
                &[("c.ts", synthetic(&block))]
            ),
            []
        );
    }

    #[test]
    fn moved_into_a_file_that_already_has_a_copy_is_not_reported() {
        // b.ts already had the block once and gets it a second time, from
        // a.ts: two occurrences before, two after.
        let block = texts("s", 60);
        let mut twice = block.clone();
        twice.extend(texts("x", 3));
        twice.extend(block.iter().cloned());
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&block), vec![]),
                    ("b.ts", synthetic(&block), synthetic(&twice))
                ],
                &[]
            ),
            []
        );
    }

    #[test]
    fn only_as_many_copies_as_the_change_added_are_reported() {
        // b.ts gets two copies; one of them left a.ts, so one is new. c.ts
        // keeps the original.
        let block = texts("s", 60);
        let mut twice = block.clone();
        twice.extend(texts("x", 3));
        twice.extend(block.iter().cloned());
        let found = run(
            &[
                ("a.ts", synthetic(&block), vec![]),
                ("b.ts", vec![], synthetic(&twice)),
            ],
            &[("c.ts", synthetic(&block))],
        );
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn copy_within_the_change_reports_the_later_one() {
        let block = texts("s", 60);
        assert_eq!(
            run(
                &[
                    ("src/b.ts", vec![], synthetic(&block)),
                    ("src/a.ts", vec![], synthetic(&block))
                ],
                &[]
            ),
            [(
                "src/b.ts".to_owned(),
                1,
                "duplicates src/a.ts:1-60 (lines 1-60, 60 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn short_or_unrelated_files_change_nothing() {
        assert_eq!(
            run(
                &[("src/orders/format.ts", vec![], tokens(COPY))],
                &[("src/x.ts", tokens(OTHER)), ("tiny.ts", tokens("f();"))],
            ),
            []
        );
    }

    #[test]
    fn overlapping_repetition_is_not_a_copy() {
        // A statement of 5 tokens repeated: every window equals the one 5
        // tokens later, but they overlap.
        let periodic: Vec<String> = (0..60).map(|i| format!("p{}", i % 5)).collect();
        assert_eq!(run(&[("a.ts", vec![], synthetic(&periodic))], &[]), []);
    }

    #[test]
    fn repetition_far_enough_apart_is_a_copy() {
        // 100 tokens of a 5-token statement: the second half repeats the
        // first without overlapping it.
        let periodic: Vec<String> = (0..100).map(|i| format!("p{}", i % 5)).collect();
        let found = run(&[("a.ts", vec![], synthetic(&periodic))], &[]);
        // The first 50 tokens are the original; the copy starts where it
        // no longer overlaps them.
        assert_eq!(
            found,
            [(
                "a.ts".to_owned(),
                51,
                "duplicates a.ts:1-50 (lines 51-100, 50 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn shifted_views_are_not_counted_as_more_copies() {
        // A 50-token periodic block copied from a file that holds a
        // 60-token run of it: one copy, not three.
        let periodic =
            |n: usize| -> Vec<String> { (0..n).map(|i| format!("p{}", i % 5)).collect() };
        let found = run(
            &[("new.ts", vec![], synthetic(&periodic(50)))],
            &[("u.ts", synthetic(&periodic(60)))],
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(!found[0].2.contains("more"), "{found:?}");
    }

    #[test]
    fn a_block_names_a_copy_of_all_of_it() {
        // New X of 70 tokens; u1 holds its first 60, u2 its last 60: no
        // 70-token copy exists, so two blocks, each with its own copy.
        let x = texts("x", 70);
        assert_eq!(
            run(
                &[("new.ts", vec![], synthetic(&x))],
                &[
                    ("u1.ts", synthetic(&x[..60])),
                    ("u2.ts", synthetic(&x[10..]))
                ],
            ),
            [
                (
                    "new.ts".to_owned(),
                    1,
                    "duplicates u1.ts:1-60 (lines 1-60, 60 tokens)".to_owned()
                ),
                (
                    "new.ts".to_owned(),
                    12,
                    "duplicates u2.ts:2-60 (lines 12-70, 59 tokens)".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_full_copy_is_preferred_to_a_partial_one() {
        let x = texts("x", 70);
        assert_eq!(
            run(
                &[("new.ts", vec![], synthetic(&x))],
                &[
                    ("a_short.ts", synthetic(&x[..50])),
                    ("b_full.ts", synthetic(&x))
                ],
            ),
            [(
                "new.ts".to_owned(),
                1,
                "duplicates b_full.ts:1-70 (lines 1-70, 70 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn a_periodic_base_counts_as_one_copy() {
        // The base holds one 60-token run of a 5-token statement; the
        // change adds a second run far enough away: one new copy.
        let periodic: Vec<String> = (0..60).map(|i| format!("p{}", i % 5)).collect();
        let mut after = periodic.clone();
        after.extend(texts("x", 3));
        after.extend(periodic.iter().cloned());
        let found = run(&[("a.ts", synthetic(&periodic), synthetic(&after))], &[]);
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn within_one_file_position_decides_which_copy_is_new() {
        // A known limitation: a copy inserted above its original is
        // reported at the original, which now comes second.
        let x = texts("x", 60);
        let mut before = texts("o", 20);
        before.extend(x.iter().cloned());
        let mut after = x.clone();
        after.extend(texts("o", 20));
        after.extend(x.iter().cloned());
        assert_eq!(
            run(&[("a.ts", synthetic(&before), synthetic(&after))], &[]),
            [(
                "a.ts".to_owned(),
                81,
                "duplicates a.ts:1-60 (lines 81-140, 60 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn colliding_hashes_are_told_apart() {
        // Every token hashes alike under the first hash; the second one
        // still tells the windows apart.
        let constant: fn(&str) -> u64 = |_| 7;
        assert_eq!(
            run_with(
                &[("new.ts", vec![], synthetic(&texts("a", 60)))],
                &[("old.ts", synthetic(&texts("b", 60)))],
                constant,
            ),
            []
        );
    }
}
