# 2026-11-09: add-duplicate-detection

**Went well:** The first two pull requests:
- **Tokens:** the normalized tokens in `qgate-lang`.
- **Git reader:** the batch blob reader in `qgate-git`.

Both had a clear contract and converged after a few review rounds. The streaming index kept memory tied to the change, not the repository: 21 MB for a 20-file change in a 224,000-line repository. Mutation checks again showed which tests actually guard which rule.

**Had to step in:** The three exploration choices, Gate 1 and 2, and five review questions:
- type-level code and JS renames;
- copies inside one file;
- "position decides within a file", later superseded;
- the 50-new-token rule;
- where the roadmap edit should go.

The pull request also needed a call to stop iterating.

**Surprise:**
- **Defining "new duplication" was the hard part, not finding duplicates.** PR 3 took five review rounds. Each round found a new edge case in what "a change added a copy" means:
  - a block overlapping itself;
  - a number table matching itself;
  - code moved out of a deleted file;
  - a window of one new token and 49 shared ones;
  - a copy whose original is another new file.

  Several of my fixes created the next problem. In the end the rules were tied to the spec's own scenarios ("Existing duplication", "Moved code"), and the remaining overlap cases are recorded as limitations for a follow-up.
- **The batch reader looked trivial, but three pipes and a thread hid a deadlock at every step:**
  - dropping it with an unread answer;
  - reading stderr only after git had exited;
  - joining a thread a grandchild process could keep open.

  Each fix was reasonable, and each needed the next review.
- **I committed the human's uncommitted roadmap rewrite inside an unrelated commit.** I had staged the whole README. It was moved to its own pull request without a force-push, and there is now a rule to check every hunk before staging.
- **A nested heredoc turned a pull request body into a pushed commit message.** It was redone on a fresh branch. Commit texts now go through files.
- **Type-2 normalization has a known blind spot:** any two long number tables look alike. It is recorded rather than patched.

**Effort:** one long session; 3 PRs (plus a tooling PR for the MSRV job and a docs PR for the roadmap), with 3, 4 and 5 review rounds.

**Blog idea:** "The spec scenarios knew better than the spec text". In three of five rounds the requirement's wording allowed a result that its own scenarios ruled out. Writing the scenarios first, and checking every rule against them, would have saved rounds.
