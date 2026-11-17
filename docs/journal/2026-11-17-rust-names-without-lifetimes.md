# 2026-11-17: rust-names-without-lifetimes

**Went well:**
- **A follow-up while the lesson was fresh.** The day after
  `share-adapter-walker` ran into it, the naming rule changed. The
  proposal, the spec delta and the code fit in one short session.
- **The test proved itself.** The CLI test for a renamed lifetime was run
  against the old adapter as well. It failed there with exactly the
  false "new function" of the day before, then passed with the fix.
- **The replay as a safety net again.** 56 merges, 234 findings before
  and after. 14 messages changed, all only by a lifetime leaving a name.
  Nothing else moved.
- **qgate reviewed its own fix.** Run on the branch, it flagged the new
  `left_out_of_name` at complexity 13: a match whose `|` alternatives
  and inline closures held two rules of their own. Two named helpers
  brought it below the threshold of 11. It also flagged two test
  passages (a throwaway repo, two commits, a run, an assert) as copies
  of other CLI tests: the idiom the shared `Repo` follow-up is about,
  left as it is.

**Had to step in:** These were the human's calls:
- choosing this follow-up first;
- Gate 1, approving the three recommendations: lifetimes only, every
  lifetime, one pull request without a release;
- accepting the two test-idiom copies qgate reported in the pull request
  (the throwaway-repo setup of the CLI tests), and merging.

**Surprise:**
- **The history had nothing to show.** The rename that motivated the
  change was undone before its pull request merged, so the replay could
  only show that names got shorter, not that a false finding went away.
  The one place that proves the fix is a test built for it.
- **The grammar has three ways to say `for<'a>`.** In a function
  pointer type it is `for_lifetimes`. In `dyn for<'a> Fn(...)` it is a
  `higher_ranked_trait_bound` with ordinary `type_parameters`. Probing
  the tree before writing the rule found these two; the reviewer found
  the third, `impl for<'a> Fn(...)`, along with lists of bounds and a
  separator rule that broke on two lifetimes in a row. Nothing in 840
  real files hit any of them: they are invalid in an `impl` header.
- **A miscount in the record.** The verification first said 16 changed
  messages; the reviewer counted 14 from the same reports. The number
  was typed from a skim of the diff, not counted.

**Effort:** about two hours; 1 PR, 7 commits, the replay run three
times, one review round.

**Follow-ups for Ervin to order:**
- a negative impl, `impl !Sync for Foo`, is named like a positive one
  (`<Foo as Sync>`): the `!` stands outside the `trait` field. Found by
  the reviewer, older than this change;
- the `Repo` test helper in one place (still open from
  `add-rust-analysis`).

**Blog idea:** "When the history can't prove a fix." A replay over
merged pull requests only sees what was merged. A false positive caught
and worked around during review leaves no trace there, so the fix needs a
test built from the incident itself.
