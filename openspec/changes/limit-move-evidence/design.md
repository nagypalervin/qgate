# Design: limit-move-evidence

## Context

See proposal.md, "Why". In `crates/qgate-analyzers/src/complexity.rs`,
`moves` settles each own-name group in two passes. The first pass
matches pairs with the same fingerprint. The second matches pairs where
`shares_half(old, new)` holds. `shares_half` skips a pair whose sizes
differ more than twofold and then calls `common_in_order` on the two
units' whole own tokens: one row of dynamic programming, O(|a|·|b|)
time and O(|b|) memory, with no limit.

The evidence of inlining already has two limits on the same constant,
`ALIGNED_PAIRS` (2^26):

- `added` cuts off the common start and end of a grown unit's two
  versions and does not align a middle of more than 2^26 pairs;
- `evidence` does not compare a removed unit with added code when their
  sizes multiply to more than 2^26.

## Goals / Non-Goals

**Goals:**

- One comparison of the move evidence costs at most 2^26 cells of
  dynamic programming. That is a fraction of a second, where a function
  of 12,000 lines took about 15 s.
- A long unit that moved and changed only in a short stretch is still
  matched.
- For every pair below the limit, the result stays the same,
  bit for bit.

**Non-Goals:**

- Bounding the number of comparisons in a group (see Risks).
- Changing the limits of inlining.

## Decisions

### D1. Cut the common start and end, then limit the middle

`shares_half` keeps its first two checks: neither unit is empty, and
neither is more than twice the other. Then it cuts off the common start
and, from what is left, the common end of the two token lists, just as
`added` does. When the two middles multiply to more than
`ALIGNED_PAIRS`, the pair is not partners. Otherwise the common
subsequence is the cut length plus `common_in_order` of the middles,
and the test of half of the longer unit stays as it is.

Cutting does not change the result below the limit. Taking a common
first token greedily never shortens a longest common subsequence, and
the same holds for a common last token. So the length is the same,
and every pair the limit does not touch gets the same answer as before.

The cut is a shared helper, a function that returns the lengths of the
common start and end of two slices. `added` uses it in place of its
inline code, so both evidences cut in one way. This is a small
refactoring of `added`, and its existing tests (`added_code`,
`added_code_has_a_limit`) cover it.

*Why the middle (Ervin, 2026-11-17):* a limit on the whole units, like
that of `evidence`, would leave every moved and edited unit above about
5,800 own tokens unmatched. Because of the twofold size check, about
5,800 tokens is where the product first passes 2^26. Such a unit then
counts as new, and a large unit is likely to be reported as a false
Error. A move usually changes a few places, often near the signature, so
the middle stays short. With the cut, the limit bites only when the two
units differ along almost all of a long body.

*Alternative:* the whole-unit limit. It needs fewer lines but gives the
false Errors above. Another alternative is a faster subsequence
(bit-parallel, or Hunt–Szymanski). It would speed up every comparison
but still leave no bound, so it is not taken.

### D2. The same constant

The limit is `ALIGNED_PAIRS`, the constant of the inlining limits. Its
doc comment gains `shares_half` as a third user. A separate constant
would invite the two to drift apart with no reason.
`common_in_order` needs O(|b|) memory, so for the move the limit only
bounds time, as it does for `evidence`.

### D3. Tests and the mutation check

The tests live next to the code, in `complexity.rs`:

- **Unit:** `shares_half` returns false for a pair whose middles exceed
  the limit, even though half of each is shared. It returns true for a
  pair of the same size that differs only in one middle token. The
  boundary of exactly 2^26 is not pinned, for the same reason as in D3
  of `complexity-of-moved-code`: such a test would run 2^26 cells.
- **Analyzer** (`run_files`, C#, generated sources, one scenario each
  of the spec): a long `Run(int)` that moves to another file and
  changes at both ends is a Warning `complexity/cyclomatic-new`. One
  that changes in one token is matched, with no finding. One that moves
  unchanged is matched, with no finding.

Mutation check, by hand, each reverted after the test run:

- without the limit, the "both ends" test fails, because evidence
  matches it;
- without the cut, the "one place" test fails, because the limit
  rejects it;
- without the cut length in the sum, a test should fail (expected: the
  "one place" test, whose common subsequence is almost all cut).

The results go into "Verification" below.

### D4. Re-measurement

This repeats the 150-PR replay with the tools on
`change/evaluate-angular-dotnet` (`evaluation/run.py`, `commits.tsv`,
`config/*.toml`) and `compare.py` from the archived
`complexity-of-moved-code`, against the 0.9.0 reports. Expected:
complexity loses only SV19 and CL11 against 0.9.0, the same as after
PR 42. The 326 findings of the other rules stay byte-identical, and the
slowest run stays well within 30 s. A different result stops the change
for a decision.

## Risks / Trade-offs

- [A group of many large namesakes] Each pair is bounded, but a group of
  n new and m removed units makes n·m comparisons. Pairs with the same
  fingerprint are settled first, and pairs whose sizes differ more than
  twofold are skipped. → A known risk, left for later. The replay shows
  the run times.
- [A long unit changed at both ends] It is no longer matched, so it is
  reported as new: a Warning, or an Error above 20. → Its two versions
  differ over more than 8,192 tokens each, so a reader sees a large
  change anyway. No such unit is in the 150 pull requests. This is the
  cautious side for time. It is not the cautious side for severity, and
  the proposal records that trade.
- [The cut is greedy] For two lists that share only one long common
  start, the cut does all the work and the middle is empty. That is the
  intended case, not a risk.

## Verification

**Tests first (task 1.1), 2026-11-17.** Before the limit,
`move_evidence_has_a_limit` and `a_long_moved_function_changed_at_both_ends`
failed on their result (the pair was evidence, the unit matched), in
1.5 s; the "one place" and "unchanged" tests already passed.

**Mutation check (task 1.4), 2026-11-17.** Each mutant applied by hand
to `shares_half` and reverted; every one fails a test:

- without the limit: `move_evidence_has_a_limit` and
  `a_long_moved_function_changed_at_both_ends` fail;
- without the cut (the whole units as middles): `move_evidence_has_a_limit`
  and `a_long_moved_function_changed_in_one_place` fail;
- without the cut length in the sum: `move_evidence_has_a_limit`,
  `a_long_moved_function_changed_in_one_place`,
  `evidence_is_half_of_both_units_tokens_in_order`,
  `method_moved_to_another_file` and `moved_and_inlined` fail.

**Re-measurement (task 2.1), 2026-11-17.** The tools of
`change/evaluate-angular-dotnet` (identical to those of the PR 42
measurement), the same clones, a release build of this branch. All 150
reports parsed, every run exited 0.

- Against 0.9.0 (`compare.py`): complexity has 2 findings gone, none new
  or changed, 60 the same. The two that went are CL11 (clients 24645,
  `decryptMany`) and SV19 (server 8856, `RunSuccessTestAsync`), as after
  PR 42.
- Against the reports after PR 42: nothing gone, new or changed.
- The 326 findings of the other rules are identical to 0.9.0's, report
  by report (compared as JSON).
- Slowest runs: server 8812 7.0 s, server 8874 6.7 s, server 8886 6.7 s,
  server 8859 6.6 s and server 8856 6.6 s, well within 30 s.
- qgate on the change itself: one Warning, `shares_half` from 4 to 5
  (+1), the decision of the limit; nothing failing.
