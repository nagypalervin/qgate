# 2026-11-17: complexity-of-moved-code

**Went well:**
- **The replay caught what the tests could not.** PR 2's first rule for
  inlining (half of the removed function's tokens, in order, in the grown
  one) passed every fixture. Over the 150 evaluation pull requests it
  silenced CL11 as planned, and four more findings, all wrong: one of them
  judged defensible in the evaluation (`processMessageWithSender` 50 -> 52).
- **Diagnostic builds instead of guessing.** A throwaway worktree printed
  each candidate pair's shares, so the human's choice of rule came with a
  table: the true inlining at 0.86 and 1.00, the strongest false pair at
  0.55 and 0.56. Two thirds both ways sits between them.
- **Mutations kept the tests honest.** After the rule changed, an old
  test still passed for the wrong reason (its second unit fell under the
  new threshold instead of losing the tie), and the first "added code"
  test was settled by the size prefilter, not by the rule it named. Both
  fixtures were rebuilt until each mutation failed a test.
- **The review found what the replay could not.** No pull request of
  the 150 removes two alike helpers next to one grown function, yet the
  reviewer built that case: both were credited and the second one's
  growth went silent. It also timed a 12,000-line function changed at
  both ends at 33.6 s, and on the next round a 12,000-line helper
  inlined at 15.5 s. One-to-one pairing and a limit on both alignments
  fixed them, and the replay came out the same.
- **One change, two pull requests, the same verdict.** After both, the
  150 reports differ from 0.9.0 in exactly the two findings the
  evaluation named (SV19, CL11); the other 60 complexity findings and 326
  findings of other rules are unchanged.

**Had to step in:** These were the human's calls:
- Gate 1, with two pull requests and no release;
- on PR 1's reviews: code evidence for every move, JavaScript left out,
  and the evidence of a move counted both ways;
- on PR 2's re-measurement: inlining evidence two thirds both ways, on
  the code the change added;
- on PR 2's review: one-to-one pairing, the longest pair first, and no
  credit above 2^26 token pairs;
- reminding the agent to keep talking Hungarian after a context summary;
- merging.

**Surprise:**
- **Evidence has to be symmetric, twice.** The same mistake came back in
  a new place: a one-sided share lets a short function "match" a long
  one. PR 1's review found it for moves; PR 2's replay found it for
  inlining. A short function's common lines (`await _repo.X(org)`,
  `if (x == null) return`) are everywhere in a long one.
- **A short function is hard to credit.** Its signature and the lines
  around its body are own tokens that never come along when it is
  inlined, so two thirds is out of reach for a two-line helper. That is
  the cautious side.

**Effort:** a long day; 2 PRs, about 10 commits, the replay run seven
times plus four diagnostic runs, five review rounds.

**Blog idea:** "The fixture passed, the replay didn't." A rule written
from one real incident (CL11) fit its own fixtures and still fired on
four unrelated pull requests. Replaying real history is the test that
finds what a rule matches by accident.
