# Proposal: limit-move-evidence

## Why

`complexity-of-moved-code` (PR 41, PR 42) matches a moved unit with a
removed unit of the same own name. When their fingerprints differ, it
looks for code evidence: their own tokens must share a common subsequence
in order. That subsequence has no limit. It costs O(|a|·|b|), and by the
reviewer's estimate one function of 12,000 lines that moves and changes
takes about 15 s, half of the 30-second budget. Two such units in one
pull request would use all of it. The evidence of inlining already stops
above 2^26 pairs of tokens (design D3 of that change). Its design names
this follow-up under "Run time of moves". The evidence of a move gets
the same limit here.

## What Changes

- **The evidence of a move has a limit.** Before the subsequence is
  computed, the common start and the common end of the two units' own
  tokens are cut off. Cutting them off does not change the length of
  the longest common subsequence. When the rest of the two units forms
  more than 2^26 pairs of tokens, the pair is not compared and has no
  evidence. Then the following holds:
  - a pair with the same fingerprint is still matched, as before (the
    first pass needs no subsequence);
  - a pair without evidence is not matched. The after unit stays new
    and can be reported under `complexity/cyclomatic-new`. The removed
    unit stays removed and can still count as inlined, within that
    evidence's own limit.
  - a long unit that moved and changed in one place, such as a modifier
    (SV19: `private` to `public`), is still matched, because its two
    versions differ only in a short middle.
- **Tests at the analyzer level.** One test checks that a long moved
  unit changed at both ends is new. Another checks that a long moved
  unit changed in one place is matched. A unit test checks the limit of
  the comparison itself. A mutation check confirms that leaving out the
  limit fails a test, and so does leaving out the cut.
- **The 150-PR replay is repeated.** Against the 0.9.0 reports, still
  only SV19 and CL11 should go away. Nothing else should change, and
  the slowest run should stay well within 30 s.

### Out of scope

- **No release.** This is one pull request, with no version bump. The
  next release carries it.
- **The number of pairs in an own-name group.** Each comparison is
  bounded. A group with many large units of one own name (`constructor`,
  `Dispose()`) still makes one comparison per pair. No group in the
  replay comes near this, so it is left as a known risk (design).
- **The limit of inlining.** It does not change. The evidence of
  inlining does not cut off a common start and end: there the added
  code is compared with a removed unit, and they rarely share an end.
- **The share needed for evidence.** Half of each unit's own tokens
  stays as it is.

### Decisions so far

- One small change, one pull request, no release (Ervin, 2026-11-17).
- The limit applies to the middle that is left after the common start
  and end are cut off, not to the units as a whole. A whole-unit limit
  would make a moved and edited unit new from somewhere between about
  5,800 and 8,192 own tokens, depending on its namesake's size, and such
  a unit would likely be reported as a false Error
  `complexity/cyclomatic-new` (Ervin, 2026-11-17, at proposal time).
  With the cut, a long unit changed near both its start and its end can
  still exceed the limit and count as new (design, Risks).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `complexity-analysis`: a new requirement, "Limit of the move
  evidence", bounds the code evidence of "Moved functions". Above the
  limit, a pair has no evidence. A pair with the same code is still
  matched. The text of "Moved functions" does not change. The new
  requirement makes it more precise, as "Limit of the alignment" does
  for "Evidence of inlining".

## Impact

- `crates/qgate-analyzers/src/complexity.rs`: `shares_half` (and the
  constant `ALIGNED_PAIRS`, whose doc names the new user), with new
  tests. No other crate changes, and no public API changes.
- Output: no change for any unit below the limit. Above it, a unit
  that moved and changed at both ends of a long body is reported as
  new.
- `docs/journal/`: a short entry.
