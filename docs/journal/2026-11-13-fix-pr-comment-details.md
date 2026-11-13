# 2026-11-13: fix-pr-comment-details

**Went well:**
- **Writing the tests found a gap in the approved rule.** "Leave out a
  related location the message names" as plain substring search would
  also drop `src/a.ts:1` next to a message naming `lib/src/a.ts:1-6`.
  The test for it came before the code, and the narrower rule went to
  the human with the pull request instead of being slipped in.
- **The review asked for an end-to-end test.** The unit tests copied
  the duplication message's form by hand, so the fix depended on two
  crates agreeing on a text format that nothing pinned. A CLI test on a
  real duplicate now fails if the old code comes back.
- **A patch release in half a day.** Two PRs, the tag between them, and
  one rehearsal run: the same path as 0.2.0 and 0.3.0, with no
  surprises in the release itself.

**Had to step in:** These were the human's calls:
- Gate 1, including not rehearsing the collection case in Azure DevOps;
- approving the rule that changed after Gate 1;
- letting the agent push the tag, and publishing.

**Surprise:**
- **The approved spec contradicted itself once implemented.** The
  scenario "A duplicate with its copies" promised both related
  locations; a real duplicate always names one of them in its message.
  The reviewer caught it, not the author.
- **The archive needed a spec split.** The requirement grew past the
  500 characters that `openspec validate --strict` accepts, so related
  locations became a requirement of their own.
- **The design claimed more than the code kept.** "Never loses a
  location" was false for a path with a space; the risk is recorded
  instead of fixed.

**Effort:** half a day, 2 PRs, 2 review rounds, one release, one
rehearsal run.

**Blog idea:** "The tests disagreed with the proposal." When a failing
test shows the approved rule is wrong, go back to the human with the
narrower rule instead of quietly picking one.
