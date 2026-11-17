# 2026-11-17: rust-names-without-lifetimes

**Went well:**
- **A follow-up while the lesson was fresh.** The day after
  `share-adapter-walker` ran into it, the naming rule changed. The
  proposal, the spec delta and the code fit in one short session.
- **The test proved itself.** The CLI test for a renamed lifetime was run
  against the old adapter as well. It failed there with exactly the
  false "new function" of the day before, then passed with the fix.
- **The replay as a safety net again.** 56 merges, 234 findings before
  and after. 16 messages changed, all only by a lifetime leaving a name.
  Nothing else moved.

**Had to step in:** These were the human's calls:
- choosing this follow-up first;
- Gate 1, approving the three recommendations: lifetimes only, every
  lifetime, one pull request without a release.

**Surprise:**
- **The history had nothing to show.** The rename that motivated the
  change was undone before its pull request merged, so the replay could
  only show that names got shorter, not that a false finding went away.
  The one place that proves the fix is a test built for it.
- **The grammar has two ways to say `for<'a>`.** In a function pointer
  type it is `for_lifetimes`. In `dyn for<'a> Fn(...)` it is a
  `higher_ranked_trait_bound` with ordinary `type_parameters`. Probing
  the tree before writing the rule found both.

**Effort:** about an hour; 1 PR, 5 commits, the replay run twice.

**Follow-ups for Ervin to order:**
- the `Repo` test helper in one place (still open from
  `add-rust-analysis`).

**Blog idea:** "When the history can't prove a fix." A replay over
merged pull requests only sees what was merged. A false positive caught
and worked around during review leaves no trace there, so the fix needs a
test built from the incident itself.
