//! Which elements two sequences keep in common: a shortest edit script
//! by Myers' algorithm, with a limit (design D5 of
//! `duplication-of-new-code`).

/// The most inserted plus deleted elements [`kept`] looks for. The trace
/// of a script with `d` edits has about `d²` entries, so at the limit it
/// takes about 32 MB, and the time grows with `(n + m) · d`.
// The duplication analyzer uses it from PR 2 of `duplication-of-new-code`.
#[allow(dead_code)]
pub(crate) const MAX_EDITS: usize = 1 << 11;

/// The number of elements cut from the start of each of `a` and `b`, the
/// number then cut from the end, and the middles left between: what is
/// left of each when their common start and then, of the rest, their
/// common end are cut off. A longest common subsequence of `a` and `b` is
/// the cut elements plus a longest common subsequence of the middles.
pub(crate) fn middles<'t, T: PartialEq>(
    a: &'t [T],
    b: &'t [T],
) -> (usize, usize, &'t [T], &'t [T]) {
    let start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[start..], &b[start..]);
    let end = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (start, end, &a[..a.len() - end], &b[..b.len() - end])
}

/// Per element of `a` and of `b`, whether it is kept: part of the common
/// subsequence of a shortest edit script from `a` to `b`. The common
/// start and end are kept; between them, the search for a script gives
/// up after [`MAX_EDITS`] edits, and then every element there counts as
/// changed.
// The duplication analyzer uses it from PR 2 of `duplication-of-new-code`.
#[allow(dead_code)]
pub(crate) fn kept<T: PartialEq>(a: &[T], b: &[T]) -> (Vec<bool>, Vec<bool>) {
    let (start, end, a_middle, b_middle) = middles(a, b);
    let mut kept_a = vec![true; a.len()];
    let mut kept_b = vec![true; b.len()];
    kept_a[start..a.len() - end].fill(false);
    kept_b[start..b.len() - end].fill(false);
    if let Some(pairs) = myers(a_middle, b_middle) {
        for (x, y) in pairs {
            kept_a[start + x] = true;
            kept_b[start + y] = true;
        }
    }
    (kept_a, kept_b)
}

/// The pairs `(x, y)` with `a[x] == b[y]` of a shortest edit script from
/// `a` to `b`, by Myers' greedy algorithm; `None` when it needs more than
/// [`MAX_EDITS`] edits.
///
/// After `step` edits, `furthest[k]` is the furthest `x` reached on
/// diagonal `k = x - y`. The trace keeps it for the diagonals
/// `-step..=step` after each step, at `step² + step + k`, to walk the
/// script back from the end.
fn myers<T: PartialEq>(a: &[T], b: &[T]) -> Option<Vec<(usize, usize)>> {
    let (a_len, b_len) = (signed(a.len()), signed(b.len()));
    let limit = signed((a.len() + b.len()).min(MAX_EDITS));
    // furthest[diagonal + offset], for diagonal in -limit - 1..=limit + 1.
    let offset = limit + 1;
    let mut furthest = vec![0; unsigned(2 * offset + 1)];
    let reach = |furthest: &[isize], diagonal: isize| furthest[unsigned(diagonal + offset)];
    let mut trace: Vec<isize> = Vec::new();
    let mut edits = None;
    for step in 0..=limit {
        let mut reached = false;
        for diagonal in (-step..=step).step_by(2) {
            let mut x = if diagonal == -step
                || (diagonal != step
                    && reach(&furthest, diagonal - 1) < reach(&furthest, diagonal + 1))
            {
                reach(&furthest, diagonal + 1)
            } else {
                reach(&furthest, diagonal - 1) + 1
            };
            let mut y = x - diagonal;
            while x < a_len && y < b_len && a[unsigned(x)] == b[unsigned(y)] {
                x += 1;
                y += 1;
            }
            furthest[unsigned(diagonal + offset)] = x;
            reached |= x >= a_len && y >= b_len;
        }
        trace.extend((-step..=step).map(|diagonal| reach(&furthest, diagonal)));
        if reached {
            edits = Some(step);
            break;
        }
    }
    let edits = edits?;
    // Walk back from the end: the last edit of each step left the
    // diagonal that the step before reached furthest.
    let traced = |step: isize, diagonal: isize| trace[unsigned(step * step + step + diagonal)];
    let mut pairs = Vec::new();
    let (mut x, mut y) = (a_len, b_len);
    for step in (0..=edits).rev() {
        let (prev_x, prev_y) = if step == 0 {
            (0, 0)
        } else {
            let diagonal = x - y;
            let prev_diagonal = if diagonal == -step
                || (diagonal != step
                    && traced(step - 1, diagonal - 1) < traced(step - 1, diagonal + 1))
            {
                diagonal + 1
            } else {
                diagonal - 1
            };
            (
                traced(step - 1, prev_diagonal),
                traced(step - 1, prev_diagonal) - prev_diagonal,
            )
        };
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            pairs.push((unsigned(x), unsigned(y)));
        }
        (x, y) = (prev_x, prev_y);
    }
    Some(pairs)
}

/// A slice index or length as `isize`, which every one fits.
fn signed(n: usize) -> isize {
    isize::try_from(n).expect("a slice length fits in isize")
}

/// A non-negative position as an index.
fn unsigned(n: isize) -> usize {
    usize::try_from(n).expect("a position is not negative")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The length of a longest common subsequence, by dynamic programming.
    fn lcs(a: &[u8], b: &[u8]) -> usize {
        let mut row = vec![0; b.len() + 1];
        for x in a {
            let mut diagonal = 0;
            for (j, y) in b.iter().enumerate() {
                let above = row[j + 1];
                row[j + 1] = if x == y {
                    diagonal + 1
                } else {
                    above.max(row[j])
                };
                diagonal = above;
            }
        }
        row[b.len()]
    }

    /// The kept elements of `s`, in order.
    fn kept_of(s: &[u8], kept: &[bool]) -> Vec<u8> {
        s.iter()
            .zip(kept)
            .filter(|(_, k)| **k)
            .map(|(x, _)| *x)
            .collect()
    }

    /// A fixed linear congruential generator: no dependency, same runs.
    fn next(state: &mut u64, bound: u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (*state >> 33) % bound
    }

    /// A sequence of up to 12 letters from the first `alphabet` ones.
    fn sequence(state: &mut u64, alphabet: u64) -> Vec<u8> {
        let len = next(state, 13);
        (0..len)
            .map(|_| b'a' + u8::try_from(next(state, alphabet)).expect("small"))
            .collect()
    }

    #[test]
    fn keeps_a_longest_common_subsequence() {
        let mut state = 20_261_117;
        for _ in 0..2000 {
            let alphabet = next(&mut state, 4) + 1;
            let a = sequence(&mut state, alphabet);
            let b = sequence(&mut state, alphabet);
            let (kept_a, kept_b) = kept(&a, &b);
            assert_eq!(kept_a.len(), a.len());
            assert_eq!(kept_b.len(), b.len());
            let common = kept_of(&a, &kept_a);
            assert_eq!(common, kept_of(&b, &kept_b), "{a:?} {b:?}");
            assert_eq!(common.len(), lcs(&a, &b), "{a:?} {b:?}");
        }
    }

    #[test]
    fn empty_and_equal_sequences() {
        assert_eq!(kept::<u8>(&[], &[]), (vec![], vec![]));
        assert_eq!(kept(b"ab", b""), (vec![false, false], vec![]));
        assert_eq!(kept(b"", b"ab"), (vec![], vec![false, false]));
        assert_eq!(kept(b"ab", b"ab"), (vec![true, true], vec![true, true]));
    }

    #[test]
    fn middles_cut_the_common_start_then_end() {
        assert_eq!(middles(b"abXcd", b"abYYcd"), (2, 2, &b"X"[..], &b"YY"[..]));
        // The start is cut first: of `aa` and `a`, nothing is left for
        // the end.
        assert_eq!(middles(b"aa", b"a"), (1, 0, &b"a"[..], &b""[..]));
    }

    /// `a` and `b` around one common element `c`, with `in_a` and `in_b`
    /// elements no other has, and a common first and last element: the
    /// middles need `in_a + in_b` edits.
    fn around(in_a: usize, in_b: usize) -> (Vec<usize>, Vec<usize>) {
        let (first, c, last) = (0, 1, 2);
        let a: Vec<usize> = [first]
            .into_iter()
            .chain((0..in_a).map(|i| 10 + i))
            .chain([c, last])
            .collect();
        let b: Vec<usize> = [first, c]
            .into_iter()
            .chain((0..in_b).map(|i| 10 + in_a + i))
            .chain([last])
            .collect();
        (a, b)
    }

    #[test]
    fn edits_up_to_the_limit_are_searched() {
        let (a, b) = around(MAX_EDITS / 2, MAX_EDITS / 2);
        let (kept_a, kept_b) = kept(&a, &b);
        assert_eq!(kept_a.iter().filter(|k| **k).count(), 3);
        assert!(kept_a[a.len() - 2] && kept_b[1]);
    }

    #[test]
    fn beyond_the_limit_only_the_common_start_and_end_are_kept() {
        // One edit past the limit.
        let (a, b) = around(MAX_EDITS / 2 + 1, MAX_EDITS / 2);
        let (kept_a, kept_b) = kept(&a, &b);
        let only_ends = |kept: &[bool]| {
            kept.iter()
                .enumerate()
                .all(|(i, k)| *k == (i == 0 || i == kept.len() - 1))
        };
        assert!(only_ends(&kept_a) && only_ends(&kept_b));
    }
}
