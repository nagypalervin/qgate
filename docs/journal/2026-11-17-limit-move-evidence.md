# 2026-11-17: limit-move-evidence

**Went well:**
- **A follow-up named in advance.** The design of
  `complexity-of-moved-code` listed the missing limit under "Run time of
  moves". This change only had to carry it out, in one short session.
- **Tests first, failing on the result.** Before the limit, the unit
  test and the "both ends" test failed because the pair matched, not by
  a timeout (1.5 s). Five mutants (no limit, no cut, no cut length in
  the sum, no start cut, no end cut) each fail a test.
- **The replay as a safety net again.** Against 0.9.0 only CL11 and
  SV19 go, the same as after PR 42. The 326 other findings are
  identical, and the slowest run takes 7.0 s.

**Had to step in:** These were the human's calls:
- Gate 1, with the choice between two limits. A limit on the whole
  units is simple but would make large moved and edited units new. The
  human chose the limit on the middle, after the common start and end
  are cut off;
- merging.

**Surprise:**
- **The design oversold its own trade-off.** It said the limit bites
  only when two long units "differ almost throughout", and that a reader
  "sees a large change anyway". The reviewer pointed at the branch's own
  test: two changed operators, one near each end of a long body, are
  enough. The spec had it right; the prose did not. Now the risk says
  so, and it names a diff whose cost grows with the differences (Myers)
  as the way out if a real case shows up.
- **No move test needed the start cut.** The shared helper was covered
  only through `added`. One more variant of the "one place" test, with
  the change near the end, pins each cut for moves too.

**Effort:** about 20 minutes from Gate 1 to review; 1 PR, 2 commits,
the replay run once, one review round.

**Follow-ups for Ervin to order:**
- the number of comparisons in a large own-name group is still
  unbounded (each comparison is bounded now);
- if a real move hits the limit: an O((N+M)·D) diff for the move
  evidence.

**Blog idea:** "The test disagreed with the design." The reviewer did
not need a new case to refute the design's argument. The branch's own
test already showed it, but its comment repeated the design's framing
("differ over more than 8,192 tokens"), so nobody noticed.
