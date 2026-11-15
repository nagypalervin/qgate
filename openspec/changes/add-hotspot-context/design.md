# Design: add-hotspot-context

**Approved at Gate 2 (2026-11-14):** with the proposal, as written.

## Context

See proposal.md for why and for what the explore session decided; the
specs `hotspot-analysis` and `report-formats` say what. What the design
works with:

- **`qgate-git`** reads two revisions only: `changes`, `files_at`,
  blobs (ADR 0002, subprocess). History is new to it.
- **The revision check** (`check_revisions` in `qgate-cli`) already
  reads the head file list once (0.7.0, D8 of `add-test-gap-analysis`).
  Duplication parses every supported, non-excluded, non-generated head
  file of each family that has an analyzed change, so the head models
  are at hand.
- **`Finding`** in `qgate-core` has `rule`, `severity`, `message`,
  `location`, `metric` and `related`. Findings are built as struct
  literals in about 30 places across six crates.
- **The JSON schema** (`docs/schema/report-v1.json`) lets version 1 add
  keys; the `message` is documented as "do not parse".
- **git 2.43 here; CI runners are newer.**
  - `git rev-parse --is-shallow-repository` answers `true` on a
    `--depth` clone.
  - `git log --name-status --find-renames` lists a rename as
    `R<score>\t<old>\t<new>`.

## Goals / Non-Goals

**Goals:**

- One extra `git log` process per check, no extra blob reads: the
  complexity comes from the models already parsed.
- The ranking is a pure function of numbers per path, testable without
  git.

**Non-Goals:**

- History of functions, authors, coupling.
- Caching history between runs; the MCP server pays the `git log` per
  call, and the evaluation measures what that costs.

## Decisions

### D1. History in `qgate-git`: commit count per head path

`Repository::churn(head, since)` returns, per path, the number of
non-merge commits reachable from `head` whose committer time is after
`since`, and that add, modify, rename or copy the file, counted under
the name `head` has. `commit_time` and `is_shallow` go with it. (As
built in PR 1, after its review; the first draft of this section
described a simpler walk.)

- **Command:**
  ```
  git log --topo-order --find-renames --name-status -z
    --no-ext-diff --no-show-signature
    --format='commit %H %ct %P' --since=@<since> <head>
  ```
  `-z` keeps unusual path names intact. `--no-show-signature` keeps a
  user's `log.showSignature` from putting text before the headers. The
  same goes for `commit_time` (`git show -s --no-show-signature
  --format=%ct`).
- **Merges are walked but count nothing:** without `-m` they show no
  files. `--no-merges` would cut the graph the renames travel along.
- **Renames follow the commit graph:**
  - with `--topo-order`, children come before parents;
  - each commit receives from its children a map from older names to
    head names, counts its `A`, `M`, `R` and `C` lines under the mapped
    name (once per commit), and passes the map, plus its own renames,
    to its parents;
  - a rename made on the same line of history as the edits is followed,
    also through the merge that brings them in.

  **Known limit:** a rename on *another* branch than the edits is joined
  only in the merge, which shows no files, so those edits keep the old
  name. `git log -m` would show it, but it omits the block for a parent
  whose diff is empty, and a custom format does not say which parent a
  block belongs to. One `git diff` per merge commit would cost hundreds
  of processes in a merge-per-pull-request history. The cost of the
  limit is an undercount for a file renamed while another branch edited
  it, so at worst a missing note. Accepted; tests pin both cases.
- **Paths the head does not have** keep their own entries; the CLI ranks
  only the head's files, so the git crate stays ignorant of languages
  (ADR 0002).
- **The window and clock skew:** `--since` stops the walk at commits
  dated before `since`, and with `--topo-order` it also hides what lies
  behind them. So a commit in the window that only an older-dated
  commit leads to is not seen. Walking the whole history instead would
  cost a full `git log --find-renames` on every check. Accepted for
  skewed clocks, which are rare; the spec says so. The Rust filter
  (`time > since`) makes the start of the window exact for every commit
  git does print.
- **Shallow:** `is_shallow()` runs `git rev-parse
  --is-shallow-repository`.
- **Errors:** as other git calls: exit codes, not localized messages.
- **Alternatives:**
  - `git log --follow` per file: one process per file, too slow;
  - a git library: rejected by ADR 0002.

### D2. The window (Gate 1, question 1)

- **Start:** 182 days before the committer date of the head commit
  (`commit_time`). A commit counts when its committer date is after the
  start; the head's own date ends the window.
- **With `--worktree`,** the head is a snapshot commit dated now, so the
  CLI walks from `HEAD` itself and dates the window by `HEAD`. The
  snapshot is never in the walk, whatever its time. (The review of PR 1
  showed that excluding it by time alone fails when it shares `HEAD`'s
  second.) Uncommitted work is one edit, not history.
- **The note says "6 months":** 182 days is 26 weeks, and the note is
  for people. The spec fixes the number of days.

### D3. Ranking in `qgate_analyzers::hotspots`

A pure function takes, per language family, the ranked files with their
churn and complexity, and returns the hotspot set.

- **Population** (Gate 1, question 3): the head files duplication
  reads, that is supported, not excluded and not generated, per family.
  It is computed only for families with an analyzed change, and only
  those can have findings.
- **Thresholds** (fixed in 0.8.0, Gate 1, question 6):
  - churn at least 5, with fewer than 10% of the family's files above
    it;
  - complexity at least 1, with fewer than 25% of the family's files
    above it.

  "Fewer than k% above it" includes ties at the top and needs no
  rounding rule: with 20 files, a file is in the top 10% when at most
  one file has a higher churn.
- **Complexity:** the sum of `Function::cyclomatic` over the file's head
  model. Changed files use their analyzed head model, unchanged ones
  the model duplication parsed.
- **Annotation:** `annotate(findings, hotspots)` sets
  `Finding::hotspot` and appends `; hotspot: <n> commits in 6 months,
  complexity <c>` to the message of every finding at a hotspot path,
  except `input/unparsable`. It is a separate step at the end of the
  check, so no analyzer knows about history.

**Tests are not ranked (decided after the evaluation, 2026-11-15,
Ervin).** In `bitwarden/server`, 20 of 48 notes fell on test or test
support code, mostly duplication in large test files from one wide
commit. Files that are tests by the conventions of `test-analysis` now
leave the ranking: their findings get no note, and their churn cannot
push sources out. `qgate_analyzers::tests::tests_among` exposes the
conventions; `duplication()` drops tests from the complexity map it
returns.

### D4. `Finding::hotspot` in `qgate-core` (Gate 1, question 7)

`pub hotspot: Option<Hotspot>`, with `Hotspot { commits: u32,
complexity: u32 }`. Every struct literal gets `hotspot: None`, a
mechanical edit in PR 1 that changes no output. (As built: 16 literals,
not "about 30": the earlier count included return types such as `->
Finding {`.) The JSON renderer
writes the key when set; the other renderers need nothing, because
the note is in the message.

- **Alternative:** the note only in the message, with no field. That
  would leave out the core change, but JSON consumers (the pilot
  evaluation, an MCP agent) would have to parse a message the schema
  tells them not to parse.
- **Alternative:** `Finding` with `#[non_exhaustive]` and a
  constructor. That is a larger refactor than this change needs.

### D5. CLI wiring

- **Order:**
  - after the analyzers and duplication, and before the sort;
  - only when the check has an analyzed change, as for duplication and
    tests;
  - shallow check first. If shallow: no history read, and a note
    returned to `qgate check`, which prints it once on standard error.
    The hook and the MCP server drop it.
- **Models:** `duplication()` today parses unchanged head files and
  drops their models after indexing. It will also record each file's
  complexity sum, a `HashMap<String, u32>` per family, so the models
  are parsed once.
- **Two-file form:** untouched.
- **After the review of PR 2:**
  - the shallow note is printed when the check has a file to analyze,
    as hotspot context would otherwise be computed; the spec and the
    README now say so;
  - a changed file reported as `input/unparsable`, at its base or its
    head, is not ranked;
  - with `--worktree`, a file moved with an uncommitted `git mv` has
    no history under its new path yet, so it has no note until the
    move is committed. Accepted, and in the README.
- **As built (PR 2):**
  - `check_revisions` keeps its signature for the hook and the MCP
    server and wraps `revision_check`, which also returns the shallow
    flag that only `qgate check` prints;
  - `duplication()` returns the complexity of every file it compares,
    changed (from the analyzed head model) or not, so `hotspot_context`
    gets one map and the revision check stays under clippy's length
    limit without an `#[allow]`;
  - in Markdown the note is escaped like the rest of the cell (`->`
    becomes `-\>`); the spec's Markdown scenario now shows that.

### D6. Evaluation and rehearsal (Gate 1, questions 8 and 9)

- **Evaluation:**
  - **Repositories:** the four of 0.7.0, re-cloned with full history
    and all blobs. These are not shallow, unlike a `--depth` or
    `--shallow-since` clone, which qgate would rightly refuse to rank.
    They are not partial (`--filter=blob:none`) either: rename detection
    reads blobs, and a partial clone would fetch them over the network
    during the timed run (review of PR 1).
  - **Run:** the last 30 code commits again, with the 0.7.0 scripts
    extended.
  - **Record:**
    - hotspots per family and repository;
    - every annotated finding, sorted by hand: does it sit in code a
      reviewer would call fragile?
    - how many Warnings option 2 (severity raising) would have made
      Errors;
    - the time per commit against 0.7.0, and the `git log` share of it.
  - **Thresholds:** they may change before the release; Ervin decides.
- **Azure rehearsal:**
  - **Setup:** a branch with six commits that each touch one TypeScript
    file holding the complex units, and a pull request that raises one
    unit's complexity. Hotspot status comes from history reachable from
    the pull request's merge commit, so the rehearsal `main` stays
    untouched.
  - **Expected:** the Warning row and the build issue end with the
    hotspot note.

### D7. Three pull requests (Gate 1, question 10)

- **PR 1** (`change/add-hotspot-context`): D1 in `qgate-git` with tests
  on dated temporary repositories, and D4's field with `hotspot: None`
  everywhere. No output changes.
- **PR 2** (`change/add-hotspot-context-2`): D2, D3, D5, the JSON key and
  schema, the README, 0.8.0 and the evaluation.
- **Release 0.8.0.**
- **PR 3** (`change/add-hotspot-context-3`): the README versions, the
  roadmap, the rehearsal, the journal and the archive.

## Evaluation (task 2.5, 2026-11-15)

**Repositories,** full clones (all history and blobs, not shallow,
nothing of theirs built or run):

| | commits | head |
|---|---|---|
| `jasontaylordev/CleanArchitecture` | 984 | 2026-11-09 |
| `dotnet/eShop` | 349 | 2026-10-15 |
| `bitwarden/server` | 8,167 | 2026-11-13 |
| `bitwarden/clients` | 23,835 | 2026-11-13 |

**Binaries:** release builds of this branch (9b17616 for the first
run; the second run's build of the code in the commit that records it)
against the published 0.7.0.

**Run:** the last 30 non-merge commits touching `.cs` (server, eShop),
`.ts` (clients) or both (CleanArchitecture). Each is checked out with an
untracked `qgate.toml` that excludes `**/Migrations/**`, and both
binaries run `check --base <commit>^ --worktree --format json`. The
`git log` qgate runs is timed on its own.

**Nothing else changed:** in all 120 commits, every finding equals
0.7.0's once the note and the `hotspot` key are removed. No run failed,
and nothing reported a shallow history.

**Time per commit,** this branch against 0.7.0, mean (max):

| | this branch | 0.7.0 | its `git log` |
|---|---|---|---|
| CleanArchitecture | 0.06 s (0.11) | 0.06 s (0.09) | 0.00 s |
| eShop | 0.17 s (0.23) | 0.18 s (0.26) | 0.00 s |
| server | 4.77 s (5.83) | 5.20 s (6.13) | 0.08 s (0.10) |
| clients | 4.93 s (5.76) | 5.05 s (5.72) | 0.14 s (0.17) |

The history read costs at most 0.17 s, about 3% of a large check;
duplication over the whole tree dominates both versions.

**First run (tests ranked):**

| | findings | with a note | in files | in test or test support code |
|---|---|---|---|---|
| server | 187 | 48 (26%) | 22 | 20 |
| clients | 177 | 28 (16%) | 20 | 3 |

CleanArchitecture and eShop got no note. That is right: in their
windows no file changed in more than one (CleanArchitecture) or two
(eShop) commits, so the 5-commit minimum is never met
([`evaluation/churn_hs.py`](evaluation/churn_hs.py)). Twenty of the
server's 48 notes were in test or test-support code: 18 in tests, mostly
duplication in large test files from one 132-file commit, and 2 in
seeders. That led to the decision under D3: tests are not ranked. The
seeders are no tests by any convention and stay.

**Hotspots per family** (D6 asks for it): qgate exposes no list of
hotspots, by design, so `churn_hs.py` counts the churn half of the
definition per family at each repository's newest evaluated commit, an
upper bound on the hotspots:

| | family | ranked files | highest churn | churn-qualified |
|---|---|---|---|---|
| CleanArchitecture | TypeScript | 21 | 1 | 0 |
| CleanArchitecture | C# | 78 | 1 | 0 |
| eShop | C# | 476 | 2 | 0 |
| server | C# | 3,689 | 111 | 147 |
| clients | TypeScript | 5,563 | 112 | 373 |

The complexity condition takes out the simple ones among these; the
evaluated commits touched 14 (server) and 18 (clients) of the rest.

**Second run (tests not ranked),** the result to ship, every finding in
[`evaluation/findings.md`](evaluation/findings.md):

| | with a note | central code | composition root | small file | support code |
|---|---|---|---|---|---|
| server | 30 in 14 files | 23 | 0 | 5 | 2 |
| clients | 26 in 18 files | 16 | 7 | 2 | 1 |

- **The diff between the runs is exactly the intended one:**
  - 18 notes on test code are gone in server and 2 in clients;
  - nothing new: every note of the second run was in the first.
- **How the sort was made:** `classify_hs.py` assigns the categories by
  rule: seeders and story helpers by name, the two composition roots by
  name, then complexity of 50 or more against less. Each file was then
  read by its name and role to check that the label fits; no label was
  changed. (The review of PR 2 asked for this to be said plainly.)
- **Central code** (complexity of 50 or more): 39 notes. They are on
  services, controllers, repositories, commands, background scripts and
  large components, all of them code a reviewer would treat with care:
  - server: `CipherService.cs` (216), `OrganizationService.cs` (245),
    `CiphersController.cs` (325), `AccountController.cs` (145),
    `RestoreOrganizationUserCommand.cs` (94);
  - clients: `collect-autofill-content.service.ts` (387),
    `cipher.service.ts` (322), `vault.component.ts` (235),
    `runtime.background.ts` (129), `autofill-init.ts` (70).
- **Composition roots:** `main.background.ts` (90 commits, complexity
  102) and `service-container.ts` (64 commits, complexity 24) change in
  nearly every feature, because every feature wires a service there.
  They are hotspots by any measure, and Tornhill's too. Their notes sit
  on `tests/unchanged-test` and complexity findings for that wiring.
  Accepted; `[tests] exclude` already quiets the tests part.
- **Small files** (complexity 21 to 42): two busy Angular components,
  two C# invite-link commands and a SAML options extension. They rank in
  the top quarter because most files of their family are simpler.
  Plausible, not wrong.
- **Support code:** two C# seeders and a Storybook helper, outside every
  test convention. Few enough to accept.

**Option 2, severity raising,** would have turned 24 (server) and 25
(clients) Warnings into Errors in 60 commits. That supports annotation
only.

**Thresholds** stay as specified: nothing in the sort calls for another
value, and the stop rule (a slow `git log`, or thresholds that look
wrong) is not met.

**To reproduce:** [`evaluation/run_hs.py`](evaluation/run_hs.py) on each
full clone (`<repo> <out> <qgate> <qgate-0.7.0> '*.cs'`, or `'*.ts'`,
or both), then `summary_hs.py` and `classify_hs.py <first run>
<second run>`.

## Review of PR 2

The reviewer found no blocker; it checked the evaluation's counts
against `findings.md` and mutated the code to see which tests fail.
Fixed:
- `families_are_ranked_apart` could not fail: the change touched only
  TypeScript, so C# was never ranked. It now changes both families and
  checks the C# note; a mutation that ranks all files together fails
  it;
- the "fewer than k%" rule was not pinned at its edges: a unit test
  now has exactly 10% and 25% above (`<=` fails it), and the scenario
  "Changed often and complex" has the cart fifth in complexity, as the
  spec says;
- the window's edge had no end-to-end test: one now dates a commit
  exactly 182 days before the head (out) and a second later (in); a
  250-day window fails it;
- generated C# files not being ranked had no test; one now has a busy,
  complex `.g.cs` that would otherwise push a source out;
- the evaluation lacked hotspots per family (now an upper bound, with
  `churn_hs.py`), described its rule-based sort as a hand sort, and
  counted 20 notes "in tests" where 18 were tests and 2 seeders;
- the spec and README are aligned with the code on the shallow note and
  unparsable files, the README names "complexity at least 1", and the
  uncommitted `git mv` limit is recorded (D5);
- `an_unparsable_hotspot` says that the filter is pinned by a unit
  test, and two test comments are corrected.

## Release notes for 0.8.0 (draft, task 2.4)

To go above GitHub's generated list when the release is published:

```markdown
qgate now adds hotspot context from git history (M5b): in `--base` and
`--worktree` mode, the Claude Code hook and the MCP server, a finding in
a file that is both complex and changed often ends with
`; hotspot: <n> commits in 6 months, complexity <c>`.

- Context only: no severity, `--fail-on` result or exit code changes,
  and there is no list of hotspots.
- The JSON report gets an optional `hotspot` key on such findings,
  `{"commits": <n>, "complexity": <c>}`; within `schema_version` 1,
  consumers that ignore unknown keys need no change.
- History must be complete: in a shallow clone `qgate check` says
  `qgate: the history is shallow; hotspots not computed` on standard
  error and adds no note. CI jobs need `fetch-depth: 0` / `fetchDepth: 0`,
  as the README's already have.

See "Hotspots" in the README.
```

## Review of PR 1

The reviewer found no blocker and reproduced each finding in scratch
repositories. Fixed:
- `log.showSignature=true` put signature text before the headers and
  broke `churn` and `commit_time`; both pass `--no-show-signature`;
- renames on a parallel branch were lost by a single "newest name" map;
  the walk now follows the commit graph, merges included, and the case
  git cannot show cheaply (a rename on another branch than the edits)
  is a documented, tested limit (D1);
- "exact window with clock skew" was false: `--since` with
  `--topo-order` hides history behind an older-dated commit. D1, the
  spec and the commit record now say so;
- D1, D2 and task 1.1 now describe what was built, and the window's
  start is defined (after it, not at it) and tested at the second;
- the `--worktree` snapshot is excluded by walking from `HEAD`, not by
  its time, and `churn` lost its `until` argument;
- only the token after a header drops its newline; paths are kept as
  they are, and an empty path token is an error;
- the evaluation uses full clones, so its timing has no network fetch;
- the proposal's example names `Cart.Total()`;
- `day()` is arithmetic, merges share a `merge_at` helper, and
  `parse_log` has tests on synthetic input (copies, merges, errors).

## Risks / Trade-offs

- **[`git log --find-renames` is slow on large windows]** It is O(changed
  files) per commit, with rename detection on top. Mitigation: the
  evaluation times it on `bitwarden/clients` (thousands of commits in 6
  months). If it is too slow, drop `--find-renames` and accept that a
  renamed file starts over, decided with Ervin before the release.
- **[Relative thresholds in tiny repositories]** With 3 files the most
  changed one is always in the top 10%; the minimum of 5 commits
  guards against noise. Accepted.
- **[Bulk commits]** A formatting commit that touches every file adds
  one to every file. It shifts no ranking, since all move together.
  Accepted.
- **[Branches that rewrite history]** Only what is reachable from the
  head counts, so a squash-merge workflow counts one commit per pull
  request. That is consistent and arguably right; the README says so.
- **[The MCP server runs `git log` on every call]** Acceptable if the
  evaluation's timing holds. Caching is a later change.
- **[A new optional JSON key]** Consumers that reject unknown keys
  break. The version 1 contract allows it (`report-formats`), and the
  release notes name it.
- **[The note makes messages longer]** Azure and GitHub annotations
  and the Markdown table get longer lines. Only findings in hotspots
  get the note, a minority by construction.

## Migration Plan

Nothing to migrate. Rolling back means pinning 0.7.0.
