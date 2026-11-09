# 2026-11-09: analyze-wrapped-functions

**Went well:** Exploration that settled four questions one at a time.
Each one depended on the one before it: where the rule applies, which
call forms, several functions in a call, then objects. The design fitted
the existing single walk with one stack and two small helpers. Mutation
checks caught all twelve deliberate breaks across two rounds.

**Had to step in:** The four exploration choices, approving with
`/opsx:apply`, and one review question: should the rule also apply inside
`describe`/`it` bodies and static blocks, where the code applied it but
the spec did not? The human chose the spec.

**Surprise:**
- The code drifted from the spec in a way that read like a bug fix: "where
  no unit collects" was simpler than "module level or a class field
  initializer", and every fixture passed, because no fixture had a bound
  call inside a test callback. The reviewer found it with a two-test
  `describe` whose `res` and `res#2` would have produced false findings.
- The design said "compare the node id of the bound call", which fails as
  soon as a type wrapper sits around the call. Found during the first test
  run, not in review.
- The most common rename (helpers inside a `memo` component get the
  component's name as prefix) was in the design but in no test. The
  second review asked for it to be pinned.

**Effort:** one session; 1 PR, 2 review rounds.

**Blog idea:** "Your fixtures are your spec's blind spots": two bugs in
this change lived exactly where no fixture looked.
