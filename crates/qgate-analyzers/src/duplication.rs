//! Blocks of code that a change newly copies from another file.
//!
//! A repository-level analysis, not an [`Analyzer`](crate::Analyzer): it
//! compares the changed files with every other file of the head revision.
//! The caller builds a [`DuplicationIndex`] from the changed files (and the
//! files the change removed), feeds it the other head files one by one,
//! and asks for the findings, so only the changed files are kept in memory
//! (design D3 of `add-duplicate-detection`).
//!
//! - **Tokens:** code is compared as the normalized tokens of `qgate-lang`,
//!   so a copy with renamed variables matches.
//! - **Duplicates:** a duplicate block is a run of at least [`MIN_TOKENS`]
//!   tokens that occurs in two or more files. Repetition within one file
//!   (a data table, consecutive test cases) is not duplication.
//! - **New duplication:** a block is new duplication when the change
//!   increased the number of files that hold it. Unchanged files are the
//!   same in both revisions, so the changed files decide.
//! - **Written code:** only tokens on lines the change wrote count
//!   towards a block. A line diff of each changed file gives its added
//!   lines, and added lines that match removed ones by their trimmed text,
//!   in runs of three, moved (design D3 of `duplication-of-new-code`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use qgate_core::{Finding, Location, Severity};
use qgate_lang::Token;

use crate::diff::kept;

/// Rule id of a newly duplicated block.
pub const RULE: &str = "duplication/duplicate-block";

/// The smallest duplicate block, in normalized tokens.
pub const MIN_TOKENS: usize = 50;

/// One changed file: its path, and the tokens and line hashes of both
/// versions (empty before for an added file, empty after for a file the
/// change removed).
#[derive(Debug, Clone, Copy)]
pub struct ChangedSource<'a> {
    /// Path in the head revision, as findings report it (the old path for
    /// a removed file).
    pub path: &'a str,
    /// Tokens of the base version.
    pub before: &'a [Token],
    /// Tokens of the head version.
    pub after: &'a [Token],
    /// Line hashes of the base version, as [`FileModel::lines`]: at
    /// index `line - 1`, so they cover the line of every token in
    /// `before`.
    ///
    /// [`FileModel::lines`]: qgate_lang::FileModel::lines
    pub before_lines: &'a [u64],
    /// Line hashes of the head version, covering the line of every token
    /// in `after`.
    pub after_lines: &'a [u64],
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

/// Which files hold the windows that the changed files contain.
#[derive(Debug, Clone)]
pub struct DuplicationIndex {
    /// Paths of the changed files first (in input order), then the others.
    paths: Vec<String>,
    /// The windows of each changed file's head version, by start.
    changed: Vec<Vec<(Key, Place)>>,
    /// The changed files whose base version holds a key.
    before: HashSet<(usize, Key)>,
    /// Per key: how many changed files hold it in their base version.
    before_files: HashMap<Key, usize>,
    /// Per key that a changed file contains: every place in the head
    /// revision, changed files included.
    places: HashMap<Key, Vec<Place>>,
    /// Per changed file, per token of its head version: whether the
    /// token is on a line the change wrote (see [`written_lines`]).
    written: Vec<Vec<bool>>,
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
            before: HashSet::new(),
            before_files: HashMap::new(),
            places: HashMap::new(),
            written: Vec::new(),
            token_hash,
        };
        for (change, lines) in changes.iter().zip(written_lines(changes)) {
            // A token past the known lines counts as written, as every
            // token did before lines were known, so unknown lines behave
            // as before.
            index.written.push(
                change
                    .after
                    .iter()
                    .map(|t| {
                        (t.line as usize)
                            .checked_sub(1)
                            .and_then(|i| lines.get(i))
                            .is_none_or(|&written| written)
                    })
                    .collect(),
            );
        }
        for (file, change) in changes.iter().enumerate() {
            index.paths.push(change.path.to_owned());
            let windows = index.windows(file, change.after);
            for &(key, place) in &windows {
                index.places.entry(key).or_default().push(place);
            }
            index.changed.push(windows);
        }
        for (file, change) in changes.iter().enumerate() {
            for (key, _) in index.windows(file, change.before) {
                if index.before.insert((file, key)) {
                    *index.before_files.entry(key).or_default() += 1;
                }
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
        self.paths.push(if added {
            path.to_owned()
        } else {
            String::new()
        });
    }

    /// The `duplication/duplicate-block` findings, in changed-file order
    /// and then by line.
    #[must_use]
    pub fn findings(&self) -> Vec<Finding> {
        let counts = self.counts();
        let mut lookup = Lookup::default();
        let mut findings = Vec::new();
        for (file, windows) in self.changed.iter().enumerate() {
            let coverage = self.coverage(file, &counts);
            let mut used = HashSet::new();
            for (first, last) in runs(windows, &counts.shared) {
                // A run without a window reported for this file has no new
                // tokens: skip it before the costly block search (a long
                // table the file already shared would cost places x length).
                if !windows[first..=last]
                    .iter()
                    .any(|(key, _)| counts.reported.contains(&(file, *key)))
                {
                    continue;
                }
                findings.extend(self.blocks(
                    file,
                    first,
                    last,
                    &counts,
                    &coverage,
                    &mut used,
                    &mut lookup,
                ));
            }
        }
        findings
    }

    /// Which keys two or more files share, which of them grew, and which
    /// changed files newly duplicate them (design D3, step 4): per growing
    /// key, the changed files that did not hold it before, as many as the
    /// change added files that hold it, skipping those whose copies moved
    /// (design D1 of `duplication-of-new-code`), or else the first by path
    /// when every holder is new (the original).
    fn counts(&self) -> Counts {
        let mut counts = Counts::default();
        for (key, places) in &self.places {
            let holding: BTreeSet<usize> = places.iter().map(|p| p.path).collect();
            if holding.len() < 2 {
                continue;
            }
            counts.shared.insert(*key);
            let changed_holding = holding.iter().filter(|&&f| f < self.changed.len()).count();
            // Files the block left (deleted, or edited away) count too.
            let before_files = self.before_files.get(key).copied().unwrap_or(0);
            let mut budget = changed_holding.saturating_sub(before_files);
            if budget == 0 {
                continue;
            }
            counts.growing.insert(*key);
            // A new holder is moved when none of its places of the key has
            // a token on a written line: it continues a holder the change
            // removed, so it is no new copy. Moved holders come first and
            // are skipped; without one, a holder is the original when
            // every holder is new.
            let mut new: Vec<(bool, usize)> = holding
                .iter()
                .copied()
                .filter(|&f| f < self.changed.len() && !self.before.contains(&(f, *key)))
                .map(|f| {
                    let written = places.iter().any(|p| {
                        p.path == f && self.written[f][p.start..][..MIN_TOKENS].contains(&true)
                    });
                    (written, f)
                })
                .collect();
            new.sort_by_key(|&(written, f)| (written, &self.paths[f]));
            let moved = new.iter().filter(|(written, _)| !written).count();
            let skip = moved.max(usize::from(holding.len() == new.len()));
            for (_, file) in new.into_iter().skip(skip) {
                if budget == 0 {
                    break;
                }
                counts.reported.insert((file, *key));
                budget -= 1;
            }
        }
        counts
    }

    /// Per token of a changed file: whether a reported window covers it
    /// (`new`), and whether duplicated code the file already had covers it
    /// (`old`): a shared window the file held before, or one whose block
    /// did not grow.
    fn coverage(&self, file: usize, counts: &Counts) -> Coverage {
        let windows = &self.changed[file];
        let len = windows.last().map_or(0, |(_, p)| p.start + MIN_TOKENS);
        let mut coverage = Coverage {
            new: vec![false; len],
            old: vec![false; len],
        };
        for &(key, place) in windows {
            let range = place.start..place.start + MIN_TOKENS;
            if counts.reported.contains(&(file, key)) {
                coverage.new[range].fill(true);
            } else if counts.shared.contains(&key)
                && (self.before.contains(&(file, key)) || !counts.growing.contains(&key))
            {
                coverage.old[range].fill(true);
            }
        }
        coverage
    }

    /// The findings for a run of shared windows `first..=last` of a
    /// changed file. A block extends only as far as one copy in another
    /// file continues it, so each finding names a copy of the whole block;
    /// where no copy continues, a new block starts. A block is reported
    /// only if at least [`MIN_TOKENS`] of its tokens are new duplicated
    /// code: on a line the change wrote, covered by a reported window, not
    /// by duplicated code the file already had, and not by a block already
    /// reported in this file.
    #[allow(clippy::too_many_arguments)] // The per-file state of one pass.
    fn blocks(
        &self,
        file: usize,
        first: usize,
        last: usize,
        counts: &Counts,
        coverage: &Coverage,
        used: &mut HashSet<Key>,
        lookup: &mut Lookup,
    ) -> Vec<Finding> {
        let windows = &self.changed[file];
        let mut findings = Vec::new();
        let mut start = first;
        while start <= last {
            let (key, place) = windows[start];
            // Copies of the first window in other files that are not new
            // copies of it themselves.
            let mut copies: Vec<Place> = self.places[&key]
                .iter()
                .copied()
                .filter(|p| p.path != file && !counts.reported.contains(&(p.path, key)))
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
            let offset = end - start;
            let tokens = offset + MIN_TOKENS;
            // Tokens of windows whose keys an earlier block in this file
            // already reported are not new again.
            let mut seen = vec![false; tokens];
            for i in start..=end {
                if used.contains(&windows[i].0) {
                    seen[i - start..i - start + MIN_TOKENS].fill(true);
                }
            }
            let new_tokens = (0..tokens)
                .filter(|&t| {
                    let at = place.start + t;
                    coverage.new[at] && !coverage.old[at] && !seen[t] && self.written[file][at]
                })
                .count();
            if new_tokens >= MIN_TOKENS {
                used.extend(windows[start..=end].iter().map(|(k, _)| *k));
                copies.sort_by_key(|c| (&self.paths[c.path], c.first_line, c.start));
                // The first copy in each file, in path order, with the
                // lines it covers over the whole block; the first one is
                // the copy the message names.
                let mut related: Vec<Location> = Vec::new();
                let mut last_path = None;
                for copy in &copies {
                    if last_path == Some(copy.path) {
                        continue;
                    }
                    last_path = Some(copy.path);
                    let last_line = lookup
                        .last_line(self, windows[end].0, copy.path, copy.start + offset)
                        .unwrap_or(copy.last_line);
                    related.push(Location {
                        path: PathBuf::from(&self.paths[copy.path]),
                        line: Some(copy.first_line),
                        end_line: Some(last_line),
                    });
                }
                let counterpart = &related[0];
                let more = match related.len() - 1 {
                    0 => String::new(),
                    k => format!(", and {k} more"),
                };
                findings.push(Finding {
                    rule: RULE.to_owned(),
                    severity: Severity::Warning,
                    message: format!(
                        "duplicates {}:{}-{} (lines {}-{}, {tokens} tokens{more})",
                        counterpart.path.display(),
                        counterpart.line.unwrap_or_default(),
                        counterpart.end_line.unwrap_or_default(),
                        place.first_line,
                        windows[end].1.last_line,
                    ),
                    location: Location {
                        path: PathBuf::from(&self.paths[file]),
                        line: Some(place.first_line),
                        end_line: Some(windows[end].1.last_line),
                    },
                    metric: None,
                    related,
                    hotspot: None,
                });
            }
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

/// Every place of a key by (file, start), built on first use, to follow a
/// copy window by window.
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

/// What [`DuplicationIndex::counts`] found.
#[derive(Default)]
struct Counts {
    /// Keys that two or more files hold.
    shared: HashSet<Key>,
    /// Shared keys whose number of holding files grew.
    growing: HashSet<Key>,
    /// (changed file, key) pairs the change newly duplicates.
    reported: HashSet<(usize, Key)>,
}

/// Per token of one changed file, see [`DuplicationIndex::coverage`].
struct Coverage {
    new: Vec<bool>,
    old: Vec<bool>,
}

/// The hashes of three consecutive lines.
type LineRun = [u64; 3];

/// Per changed file, per line of its head version: whether the change
/// wrote it, an added line that did not move (design D3 of
/// `duplication-of-new-code`).
///
/// - **Added and removed lines** come from a line diff of each file's
///   two versions; every line of a deleted file is removed.
/// - **Runs:** three consecutive added (or removed) lines of one file,
///   not all empty. They overlap, one per first line.
/// - **Pairing:** each removed run moves at most one added run with the
///   same hashes: first one of its own file, then, in path and line order,
///   one of another file. The lines of an added run that moved are not
///   written.
fn written_lines(changes: &[ChangedSource<'_>]) -> Vec<Vec<bool>> {
    let diffs: Vec<(Vec<bool>, Vec<bool>)> = changes
        .iter()
        .map(|c| kept(c.before_lines, c.after_lines))
        .collect();
    let mut order: Vec<usize> = (0..changes.len()).collect();
    order.sort_by_key(|&file| changes[file].path);
    let mut pool = removed_runs(changes, &diffs, &order);
    let mut written: Vec<Vec<bool>> = diffs
        .iter()
        .map(|(_, kept_after)| kept_after.iter().map(|kept| !kept).collect())
        .collect();
    let mut take = |file: usize, start: usize, run: LineRun, own: bool| {
        let found = pool.get_mut(&run).and_then(|files| {
            files
                .iter_mut()
                .find(|(from, count)| *count > 0 && (!own || *from == file))
        });
        found.map(|(_, count)| {
            *count -= 1;
            written[file][start..start + 3].fill(false);
        })
    };
    let mut unpaired = Vec::new();
    for &file in &order {
        for (start, run) in line_runs(changes[file].after_lines, &diffs[file].1) {
            if take(file, start, run, true).is_none() {
                unpaired.push((file, start, run));
            }
        }
    }
    for (file, start, run) in unpaired {
        take(file, start, run, false);
    }
    written
}

/// Per removed run: the files that removed it, in the given (path)
/// order, and how many times each.
fn removed_runs(
    changes: &[ChangedSource<'_>],
    diffs: &[(Vec<bool>, Vec<bool>)],
    order: &[usize],
) -> HashMap<LineRun, Vec<(usize, usize)>> {
    let mut pool: HashMap<LineRun, Vec<(usize, usize)>> = HashMap::new();
    for &file in order {
        for (_, run) in line_runs(changes[file].before_lines, &diffs[file].0) {
            let files = pool.entry(run).or_default();
            match files.last_mut() {
                Some((last, count)) if *last == file => *count += 1,
                _ => files.push((file, 1)),
            }
        }
    }
    pool
}

/// Every run of three consecutive lines that are all changed (not
/// `kept`) and not all empty, with its first line's index.
fn line_runs<'l>(
    lines: &'l [u64],
    kept: &'l [bool],
) -> impl Iterator<Item = (usize, LineRun)> + 'l {
    lines
        .windows(3)
        .zip(kept.windows(3))
        .enumerate()
        .filter(|(_, (run, kept))| !kept.contains(&true) && run.iter().any(|&h| h != 0))
        .map(|(start, (run, _))| (start, [run[0], run[1], run[2]]))
}

/// Runs of consecutive windows of a changed file whose keys another file
/// shares, as indices into its windows: each run is one or more blocks.
fn runs(windows: &[(Key, Place)], shared: &HashSet<Key>) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (i, (key, _)) in windows.iter().enumerate() {
        if !shared.contains(key) {
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
                unit: None,
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

    /// Line hashes for `tokens`: per line up to the last token's, the hash
    /// of the texts of its tokens, 0 for a line without any.
    fn lines_of(tokens: &[Token]) -> Vec<u64> {
        let count = tokens.last().map_or(0, |t| t.line as usize);
        let mut texts = vec![String::new(); count];
        for token in tokens {
            texts[token.line as usize - 1].push_str(&token.text);
        }
        texts
            .iter()
            .map(|text| if text.is_empty() { 0 } else { fnv1a(text) })
            .collect()
    }

    /// The findings themselves, for what `run` leaves out.
    fn findings_with(
        changes: &[(&str, Vec<Token>, Vec<Token>)],
        others: &[(&str, Vec<Token>)],
        hash: fn(&str) -> u64,
    ) -> Vec<Finding> {
        let changes: Vec<(&str, Code, Code)> = changes
            .iter()
            .map(|(path, before, after)| {
                (
                    *path,
                    (before.clone(), lines_of(before)),
                    (after.clone(), lines_of(after)),
                )
            })
            .collect();
        findings_of(&changes, others, hash)
    }

    /// A version of a changed file: its tokens and line hashes.
    type Code = (Vec<Token>, Vec<u64>);

    fn findings_of(
        changes: &[(&str, Code, Code)],
        others: &[(&str, Vec<Token>)],
        hash: fn(&str) -> u64,
    ) -> Vec<Finding> {
        let sources: Vec<ChangedSource<'_>> = changes
            .iter()
            .map(|(path, before, after)| ChangedSource {
                path,
                before: &before.0,
                after: &after.0,
                before_lines: &before.1,
                after_lines: &after.1,
            })
            .collect();
        let mut index = DuplicationIndex::with_token_hash(&sources, hash);
        for (path, tokens) in others {
            index.add(path, tokens);
        }
        index.findings()
    }

    /// Like `run`, for TypeScript sources, with the line hashes of
    /// `parse`: `changes` as (path, before, after), "" for a version the
    /// file does not have.
    fn run_ts(
        changes: &[(&str, &str, &str)],
        others: &[(&str, &str)],
    ) -> Vec<(String, u32, String)> {
        let code = |source: &str| {
            let model = parse(Language::TypeScript, source).expect("parses");
            (model.tokens, model.lines)
        };
        let changes: Vec<(&str, Code, Code)> = changes
            .iter()
            .map(|(path, before, after)| (*path, code(before), code(after)))
            .collect();
        let others: Vec<(&str, Vec<Token>)> = others
            .iter()
            .map(|(path, source)| (*path, tokens(source)))
            .collect();
        summary(findings_of(&changes, &others, fnv1a))
    }

    fn run_with(
        changes: &[(&str, Vec<Token>, Vec<Token>)],
        others: &[(&str, Vec<Token>)],
        hash: fn(&str) -> u64,
    ) -> Vec<(String, u32, String)> {
        summary(findings_with(changes, others, hash))
    }

    /// (path, line, message) of each finding.
    fn summary(findings: Vec<Finding>) -> Vec<(String, u32, String)> {
        findings
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

    const DATE_CS: &str = include_str!("../fixtures/duplication/date_helper.cs");
    const COPY_CS: &str = include_str!("../fixtures/duplication/renamed_copy.cs");

    // duplication-analysis: "Copy of an existing C# file". The block runs
    // from the method to the closing brace of its class in both files.
    #[test]
    fn renamed_copy_of_an_unchanged_csharp_file() {
        let cs = |source| parse(Language::CSharp, source).expect("parses").tokens;
        assert_eq!(
            run(
                &[("src/Orders/Format.cs", vec![], cs(COPY_CS))],
                &[("src/Utils/Dates.cs", cs(DATE_CS))],
            ),
            [(
                "src/Orders/Format.cs".to_owned(),
                10,
                "duplicates src/Utils/Dates.cs:5-12 (lines 10-17, 66 tokens)".to_owned()
            )]
        );
    }

    const DATE_RS: &str = include_str!("../fixtures/duplication/date_helper.rs");
    const COPY_RS: &str = include_str!("../fixtures/duplication/renamed_copy.rs");

    // duplication-analysis: "Copy of an existing Rust file". `use` lines
    // and doc comments are no part of the copy.
    #[test]
    fn renamed_copy_of_an_unchanged_rust_file() {
        let rs = |source| parse(Language::Rust, source).expect("parses").tokens;
        assert_eq!(
            run(
                &[("crates/b/src/format.rs", vec![], rs(COPY_RS))],
                &[("crates/a/src/dates.rs", rs(DATE_RS))],
            ),
            [(
                "crates/b/src/format.rs".to_owned(),
                8,
                "duplicates crates/a/src/dates.rs:6-14 (lines 8-16, 67 tokens)".to_owned()
            )]
        );
    }

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

    // Requirement: Duplicate block finding, the range and related locations
    // (add-report-formats, design D2)

    /// A location from `path` over lines `line` to `end_line`.
    fn span(path: &str, line: u32, end_line: u32) -> Location {
        Location {
            path: PathBuf::from(path),
            line: Some(line),
            end_line: Some(end_line),
        }
    }

    /// The only finding of a run, with its message checked.
    fn only_finding(
        changes: &[(&str, Vec<Token>, Vec<Token>)],
        others: &[(&str, Vec<Token>)],
        message: &str,
    ) -> Finding {
        let mut found = findings_with(changes, others, fnv1a);
        assert_eq!(found.len(), 1, "{found:?}");
        let finding = found.remove(0);
        assert_eq!(finding.message, message);
        finding
    }

    #[test]
    fn a_copy_spans_its_lines_and_names_the_original() {
        let finding = only_finding(
            &[("src/orders/format.ts", vec![], tokens(COPY))],
            &[("src/utils/date.ts", tokens(DATE))],
            "duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)",
        );
        assert_eq!(finding.location, span("src/orders/format.ts", 4, 9));
        assert_eq!(finding.related, [span("src/utils/date.ts", 1, 6)]);
    }

    #[test]
    fn related_locations_are_one_per_file_in_path_order() {
        let finding = only_finding(
            &[("src/orders/format.ts", vec![], tokens(COPY))],
            &[
                ("src/utils/date.ts", tokens(DATE)),
                ("src/b.ts", tokens(DATE)),
                ("src/a.ts", tokens(DATE)),
            ],
            "duplicates src/a.ts:1-6 (lines 4-9, 68 tokens, and 2 more)",
        );
        assert_eq!(
            finding.related,
            [
                span("src/a.ts", 1, 6),
                span("src/b.ts", 1, 6),
                span("src/utils/date.ts", 1, 6),
            ]
        );
    }

    #[test]
    fn a_file_with_two_copies_is_named_once_by_its_first() {
        let x = texts("x", 60);
        let mut u = x.clone();
        u.extend(texts("j", 10));
        u.extend(x.iter().cloned());
        let finding = only_finding(
            &[("new.ts", vec![], synthetic(&x))],
            &[("u.ts", synthetic(&u))],
            "duplicates u.ts:1-60 (lines 1-60, 60 tokens)",
        );
        assert_eq!(finding.related, [span("u.ts", 1, 60)]);
    }

    #[test]
    fn related_lines_cover_the_whole_block() {
        // 70 shared tokens: every copy's first window ends after 50 of
        // them, the block after all 70.
        let shared = texts("s", 70);
        let mut new = texts("a", 5);
        new.extend(shared.iter().cloned());
        new.extend(texts("b", 5));
        let mut old = texts("c", 10);
        old.extend(shared.iter().cloned());
        let finding = only_finding(
            &[("new.ts", vec![], synthetic(&new))],
            &[("old.ts", synthetic(&old))],
            "duplicates old.ts:11-80 (lines 6-75, 70 tokens)",
        );
        assert_eq!(finding.location, span("new.ts", 6, 75));
        assert_eq!(finding.related, [span("old.ts", 11, 80)]);
    }

    #[test]
    fn related_copies_cover_the_whole_block_like_the_named_one() {
        // u.ts has a partial copy first and a full one later; only the
        // full one counts, as in the message.
        let x = texts("x", 70);
        let mut u = x[..50].to_vec();
        u.extend(texts("j", 6));
        u.extend(x.iter().cloned());
        let finding = only_finding(
            &[("new.ts", vec![], synthetic(&x))],
            &[("u.ts", synthetic(&u)), ("u2.ts", synthetic(&x))],
            "duplicates u.ts:57-126 (lines 1-70, 70 tokens, and 1 more)",
        );
        assert_eq!(
            finding.related,
            [span("u.ts", 57, 126), span("u2.ts", 1, 70)]
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
    fn copies_within_one_file_are_not_reported() {
        // The same block three times in one new file, nowhere else.
        let block = texts("s", 60);
        let mut after = block.clone();
        after.extend(block.iter().cloned());
        after.extend(block.iter().cloned());
        assert_eq!(run(&[("a.ts", vec![], synthetic(&after))], &[]), []);
    }
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
    fn only_as_many_files_as_the_change_added_are_reported() {
        // The block leaves a.ts and arrives in b.ts and d.ts; c.ts keeps
        // it. One more file holds it than before: one finding. b.ts comes
        // first by path, so the block moved there; d.ts is the new copy
        // (duplication-analysis: "Moved and copied next to an existing
        // copy", design D1 of duplication-of-new-code).
        let block = texts("s", 60);
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&block), vec![]),
                    ("b.ts", vec![], synthetic(&block)),
                    ("d.ts", vec![], synthetic(&block))
                ],
                &[("c.ts", synthetic(&block))],
            ),
            [(
                "d.ts".to_owned(),
                1,
                "duplicates b.ts:1-60 (lines 1-60, 60 tokens, and 1 more)".to_owned()
            )]
        );
    }

    #[test]
    fn a_block_that_moved_between_files_is_not_new() {
        // b.ts gets the block twice, a.ts loses it, c.ts keeps it: two
        // files held it before and two after.
        let block = texts("s", 60);
        let mut twice = block.clone();
        twice.extend(texts("x", 3));
        twice.extend(block.iter().cloned());
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&block), vec![]),
                    ("b.ts", vec![], synthetic(&twice))
                ],
                &[("c.ts", synthetic(&block))],
            ),
            []
        );
    }
    #[test]
    fn an_edited_file_that_had_the_block_is_not_the_new_copy() {
        // a.ts had the block and is edited elsewhere; b.ts gets a copy;
        // c.ts keeps one, so no new file counts as the original.
        let block = texts("s", 60);
        let mut before = block.clone();
        before.push("x".to_owned());
        let mut after = block.clone();
        after.push("y".to_owned());
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&before), synthetic(&after)),
                    ("b.ts", vec![], synthetic(&block))
                ],
                &[("c.ts", synthetic(&block))],
            ),
            [(
                "b.ts".to_owned(),
                1,
                "duplicates a.ts:1-60 (lines 1-60, 60 tokens, and 1 more)".to_owned()
            )]
        );
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
    fn repetition_within_one_file_is_not_a_copy() {
        // A data table: 1,000 tokens of a 5-token pattern.
        let periodic: Vec<String> = (0..1000).map(|i| format!("p{}", i % 5)).collect();
        assert_eq!(run(&[("a.ts", vec![], synthetic(&periodic))], &[]), []);
    }

    #[test]
    fn a_table_copied_from_another_file_is_one_finding() {
        let periodic: Vec<String> = (0..1000).map(|i| format!("p{}", i % 5)).collect();
        let found = run(
            &[("new.ts", vec![], synthetic(&periodic))],
            &[("u.ts", synthetic(&periodic))],
        );
        assert_eq!(found.len(), 1, "{found:?}");
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
    fn a_copy_inserted_in_the_same_file_is_not_reported() {
        let x = texts("x", 60);
        let mut before = texts("o", 20);
        before.extend(x.iter().cloned());
        let mut after = x.clone();
        after.extend(texts("o", 20));
        after.extend(x.iter().cloned());
        assert_eq!(
            run(&[("a.ts", synthetic(&before), synthetic(&after))], &[]),
            []
        );
    }
    /// `head`, then `body` repeated `n` times, then `tail`.
    fn table(head: &[&str], body: &[&str], n: usize, tail: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = head.iter().map(|&t| t.to_owned()).collect();
        for _ in 0..n {
            out.extend(body.iter().map(|&t| t.to_owned()));
        }
        out.extend(tail.iter().map(|&t| t.to_owned()));
        out
    }

    #[test]
    fn a_copied_table_with_a_head_and_a_tail_is_one_finding() {
        let t = table(
            &["export", "const", "$id", "=", "["],
            &["$lit", ","],
            256,
            &["]"],
        );
        let found = run(
            &[("copy.ts", vec![], synthetic(&t))],
            &[("codes.ts", synthetic(&t))],
        );
        assert_eq!(
            found,
            [(
                "copy.ts".to_owned(),
                1,
                format!(
                    "duplicates codes.ts:1-{n} (lines 1-{n}, {n} tokens)",
                    n = t.len()
                )
            )]
        );
    }

    #[test]
    fn an_edit_next_to_shared_code_is_not_a_new_copy() {
        // a.ts and u.ts share H; u.ts has a `}` before it. The change adds
        // code ending in `}` above H in a.ts: the window `}` + 49 tokens of
        // H is new, but only one of its tokens is new code.
        let h = texts("h", 60);
        let mut u = texts("z", 10);
        u.push("}".to_owned());
        u.extend(h.iter().cloned());
        let mut after = texts("y", 10);
        after.push("}".to_owned());
        after.extend(h.iter().cloned());
        assert_eq!(
            run(
                &[("a.ts", synthetic(&h), synthetic(&after))],
                &[("u.ts", synthetic(&u))]
            ),
            []
        );
    }

    #[test]
    fn an_edit_next_to_shared_code_that_also_grows_elsewhere() {
        // As above, and n.ts gets a copy of H in the same change: n.ts is
        // reported, a.ts still is not.
        let h = texts("h", 60);
        let mut u = texts("z", 10);
        u.push("}".to_owned());
        u.extend(h.iter().cloned());
        let mut after = texts("y", 10);
        after.push("}".to_owned());
        after.extend(h.iter().cloned());
        let found = run(
            &[
                ("a.ts", synthetic(&h), synthetic(&after)),
                ("n.ts", vec![], synthetic(&h)),
            ],
            &[("u.ts", synthetic(&u))],
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, "n.ts");
    }

    #[test]
    fn moved_code_with_a_new_neighbour_is_not_a_new_copy() {
        // H leaves b.ts and arrives in c.ts after other code ending in
        // `}`; a third file has H after a `}` too.
        let h = texts("h", 60);
        let mut third = texts("z", 10);
        third.push("}".to_owned());
        third.extend(h.iter().cloned());
        let mut c = texts("y", 10);
        c.push("}".to_owned());
        c.extend(h.iter().cloned());
        assert_eq!(
            run(
                &[
                    ("b.ts", synthetic(&h), vec![]),
                    ("c.ts", vec![], synthetic(&c))
                ],
                &[("thrice.ts", synthetic(&third))],
            ),
            []
        );
    }

    #[test]
    fn a_full_copy_after_a_partial_one_in_the_same_file_is_used() {
        let x = texts("x", 70);
        let mut u = x[..50].to_vec();
        u.extend(texts("j", 6));
        u.extend(x.iter().cloned());
        assert_eq!(
            run(
                &[("new.ts", vec![], synthetic(&x))],
                &[("u.ts", synthetic(&u))]
            ),
            [(
                "new.ts".to_owned(),
                1,
                "duplicates u.ts:57-126 (lines 1-70, 70 tokens)".to_owned()
            )]
        );
        assert_eq!(
            run(
                &[("new.ts", vec![], synthetic(&x))],
                &[("u.ts", synthetic(&u)), ("u2.ts", synthetic(&x))]
            )[0]
            .2,
            "duplicates u.ts:57-126 (lines 1-70, 70 tokens, and 1 more)"
        );
    }

    #[test]
    fn when_every_holder_is_new_the_first_by_path_is_the_original() {
        // a.ts held the block and is deleted; b.ts and d.ts get it.
        let block = texts("s", 60);
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&block), vec![]),
                    ("b.ts", vec![], synthetic(&block)),
                    ("d.ts", vec![], synthetic(&block))
                ],
                &[],
            ),
            [(
                "d.ts".to_owned(),
                1,
                "duplicates b.ts:1-60 (lines 1-60, 60 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn a_copy_of_code_from_an_edited_file_is_reported() {
        // a.ts keeps H and is edited elsewhere; n.ts gets a copy of H.
        let h = texts("h", 60);
        let mut after = h.clone();
        after.push("edit".to_owned());
        assert_eq!(
            run(
                &[
                    ("a.ts", synthetic(&h), synthetic(&after)),
                    ("n.ts", vec![], synthetic(&h))
                ],
                &[],
            ),
            [(
                "n.ts".to_owned(),
                1,
                "duplicates a.ts:1-60 (lines 1-60, 60 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn the_original_of_a_copy_within_the_change_still_reports_its_own_copy() {
        // a.ts and b.ts are new and equal (P+C+Q); v.ts already holds C.
        let (p, c, q) = (texts("p", 60), texts("c", 60), texts("q", 60));
        let file: Vec<String> = [p, c.clone(), q].concat();
        let found = run(
            &[
                ("a.ts", vec![], synthetic(&file)),
                ("b.ts", vec![], synthetic(&file)),
            ],
            &[("v.ts", synthetic(&c))],
        );
        assert_eq!(
            found,
            [
                (
                    "a.ts".to_owned(),
                    61,
                    "duplicates v.ts:1-60 (lines 61-120, 60 tokens)".to_owned()
                ),
                (
                    "b.ts".to_owned(),
                    1,
                    "duplicates a.ts:1-180 (lines 1-180, 180 tokens)".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_block_pasted_twice_into_a_new_file_is_one_finding() {
        let b = texts("b", 60);
        let file: Vec<String> = [b.clone(), texts("x", 5), b.clone()].concat();
        assert_eq!(
            run(
                &[("n.ts", vec![], synthetic(&file))],
                &[("u.ts", synthetic(&b))]
            ),
            [(
                "n.ts".to_owned(),
                1,
                "duplicates u.ts:1-60 (lines 1-60, 60 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn a_copy_wrapped_around_code_the_file_already_shared_is_one_block() {
        // a.ts had S (shared with u.ts); the change wraps it into a copy of
        // v.ts: 30 new tokens before, 30 after.
        let (pre, s, post) = (texts("pre", 30), texts("s", 60), texts("post", 30));
        let whole: Vec<String> = [pre, s.clone(), post].concat();
        assert_eq!(
            run(
                &[("a.ts", synthetic(&s), synthetic(&whole))],
                &[("u.ts", synthetic(&s)), ("v.ts", synthetic(&whole))]
            ),
            [(
                "a.ts".to_owned(),
                1,
                "duplicates v.ts:1-120 (lines 1-120, 120 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn old_shared_code_is_not_searched_again() {
        // An edit appends to a file whose 2,000-token table 100 other
        // files share: there is nothing new to report, and it must stay
        // fast (the block search would be places x length).
        let table: Vec<String> = (0..2000).map(|i| format!("p{}", i % 2)).collect();
        let mut after = table.clone();
        after.extend(texts("e", 3));
        let others: Vec<(String, Vec<Token>)> = (0..100)
            .map(|i| (format!("u{i}.ts"), synthetic(&table)))
            .collect();
        let others: Vec<(&str, Vec<Token>)> = others
            .iter()
            .map(|(p, t)| (p.as_str(), t.clone()))
            .collect();
        let started = std::time::Instant::now();
        assert_eq!(
            run(&[("a.ts", synthetic(&table), synthetic(&after))], &others),
            []
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
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

    // Requirement: Lines the change wrote, Moved lines
    // (duplication-of-new-code)

    #[test]
    fn moved_lines_are_paired_in_runs_of_three_one_to_one_own_file_first() {
        // (path, before, after) as line hashes, 0 for an empty line.
        let files: [(&str, &[u64], &[u64]); 9] = [
            // Own file first: a.ts moves 1-2-3 below its rest, and 0.ts,
            // first by path, gets a copy.
            ("a.ts", &[1, 2, 3, 7, 8, 9, 10], &[7, 8, 9, 10, 1, 2, 3]),
            ("0.ts", &[], &[1, 2, 3]),
            // One to one: old.ts held 4-5-6 once, b.ts and c.ts get it.
            ("c.ts", &[], &[4, 5, 6]),
            ("b.ts", &[], &[4, 5, 6]),
            ("old.ts", &[4, 5, 6], &[]),
            // Runs of three: each line was removed, but not the three in a
            // row.
            ("d.ts", &[], &[20, 21, 22]),
            ("gone.ts", &[20, 0, 21, 0, 22], &[]),
            // Three empty lines are no run.
            ("e.ts", &[], &[0, 0, 0, 30]),
            ("blank.ts", &[0, 0, 0, 31], &[]),
        ];
        let sources: Vec<ChangedSource<'_>> = files
            .iter()
            .map(|&(path, before_lines, after_lines)| ChangedSource {
                path,
                before: &[],
                after: &[],
                before_lines,
                after_lines,
            })
            .collect();
        let (f, t) = (false, true);
        assert_eq!(
            written_lines(&sources),
            [
                vec![f, f, f, f, f, f, f],
                vec![t, t, t],
                vec![t, t, t],
                vec![f, f, f],
                vec![],
                vec![t, t, t],
                vec![],
                vec![t, t, t, t],
                vec![],
            ]
        );
    }

    /// TypeScript lines `v<i> = alpha.<tag><i>(beta, 1);` for `i` in
    /// `range`, each after `indent`: ten normalized tokens a line, and no
    /// two tags alike.
    fn statements(tag: &str, range: std::ops::Range<usize>, indent: &str) -> String {
        let lines: Vec<String> = range
            .map(|i| format!("{indent}v{i} = alpha.{tag}{i}(beta, 1);\n"))
            .collect();
        lines.concat()
    }

    /// `block` as the body of a method of a new class `name`, four spaces
    /// deep.
    fn in_class(name: &str, block: &str) -> String {
        format!("export class {name} {{\n  run() {{\n{block}  }}\n}}\n")
    }

    #[test]
    fn the_same_small_edit_in_existing_copies_is_not_reported() {
        // One argument added on the middle line: every window of the block
        // holds the edit, so every window is new.
        let block = statements("s", 0..6, "");
        let edited = block.replace("alpha.s2(beta, 1)", "alpha.s2(beta, 1, mock())");
        assert_eq!(
            run_ts(
                &[
                    ("src/a.spec.ts", &block, &edited),
                    ("src/b.spec.ts", &block, &edited)
                ],
                &[]
            ),
            []
        );
    }

    #[test]
    fn a_copy_completed_by_a_few_written_lines_is_not_reported() {
        let block = statements("s", 0..6, "");
        assert_eq!(
            run_ts(
                &[("src/a.ts", &statements("s", 0..3, ""), &block)],
                &[("src/u.ts", &block)]
            ),
            []
        );
    }

    #[test]
    fn a_new_copy_in_an_edited_file_is_reported() {
        let block = statements("s", 0..6, "");
        let other = statements("o", 0..6, "");
        assert_eq!(
            run_ts(
                &[("src/a.ts", &other, &format!("{other}{block}"))],
                &[("src/u.ts", &block)]
            ),
            [(
                "src/a.ts".to_owned(),
                7,
                "duplicates src/u.ts:1-6 (lines 7-12, 60 tokens)".to_owned()
            )]
        );
    }

    /// `src/big.ts` with the block of `statements("s", 0..6, ...)` twice,
    /// in two functions, and six more lines.
    fn big() -> String {
        let block = statements("s", 0..6, "  ");
        format!(
            "function first() {{\n{block}}}\nfunction second() {{\n{block}}}\n{}",
            statements("m", 0..6, "")
        )
    }

    #[test]
    fn a_split_file_is_not_reported() {
        // Each copy moves one level deeper, into a class of its own.
        let block = statements("s", 0..6, "    ");
        let one = in_class("One", &block) + &statements("m", 0..3, "");
        let two = in_class("Two", &block) + &statements("m", 3..6, "");
        assert_eq!(
            run_ts(
                &[
                    ("src/big.ts", &big(), ""),
                    ("src/one.ts", "", &one),
                    ("src/two.ts", "", &two)
                ],
                &[]
            ),
            []
        );
    }

    #[test]
    fn a_helper_copied_into_two_files_is_reported_once() {
        // Out of path order on purpose: a.ts takes the moved lines.
        let helper = statements("s", 0..6, "");
        assert_eq!(
            run_ts(
                &[
                    ("src/b.ts", "", &helper),
                    ("src/old.ts", &helper, ""),
                    ("src/a.ts", "", &helper)
                ],
                &[]
            ),
            [(
                "src/b.ts".to_owned(),
                1,
                "duplicates src/a.ts:1-6 (lines 1-6, 60 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn moved_within_a_file_and_copied_is_reported_for_the_copy() {
        // The rest of a.ts is longer than the block, so the diff keeps the
        // rest and the block is removed and added. src/0.ts comes first by
        // path; only the own-file pass leaves the block to a.ts.
        let block = statements("s", 0..6, "");
        let rest = statements("x", 0..8, "");
        assert_eq!(
            run_ts(
                &[
                    (
                        "src/a.ts",
                        &format!("{block}{rest}"),
                        &format!("{rest}{block}")
                    ),
                    ("src/0.ts", "", &block)
                ],
                &[]
            ),
            [(
                "src/0.ts".to_owned(),
                1,
                "duplicates src/a.ts:9-14 (lines 1-6, 60 tokens)".to_owned()
            )]
        );
    }

    #[test]
    fn moved_and_renamed_is_reported() {
        let block = statements("s", 0..6, "    ");
        let moved = in_class("One", &block) + &statements("m", 0..3, "");
        let renamed =
            in_class("Two", &block.replace("alpha", "gamma")) + &statements("m", 3..6, "");
        // The moved copy is the original whichever comes first by path.
        for (moved_path, renamed_path) in
            [("src/one.ts", "src/two.ts"), ("src/two.ts", "src/one.ts")]
        {
            let found = run_ts(
                &[
                    ("src/big.ts", &big(), ""),
                    (moved_path, "", &moved),
                    (renamed_path, "", &renamed),
                ],
                &[],
            );
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].0, renamed_path);
            assert!(
                found[0].2.starts_with(&format!("duplicates {moved_path}:")),
                "{found:?}"
            );
        }
    }
}
