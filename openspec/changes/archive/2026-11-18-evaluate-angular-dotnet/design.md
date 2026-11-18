# Design: evaluate-angular-dotnet

## Context

See proposal.md for why. Earlier evaluations measured one analyzer at a
time, on these repositories:
- `jasontaylordev/CleanArchitecture`, `dotnet/eShop`, `bitwarden/clients`
  and `bitwarden/server`;
- 30 commits each, or a whole tree.

Their scripts are kept in `openspec/changes/archive/*/evaluation/`. The
hotspot evaluation already ran each commit with an untracked `qgate.toml`
and `--worktree`, because qgate reads `qgate.toml` from the revision it
checks.

What a team sees in Azure DevOps:
- a pull request build checks the merge of the pull request into its
  target;
- `qgate report` writes one comment, and only when there are findings;
- `qgate check` is the gate.

With `--fail-on never` on both lines, the comment stays and the gate
never fails: this is the comment-only mode the pilot starts in.

The three repositories squash pull requests into `main`, so `C^..C` of a
commit `C` on `main` is exactly what its pull request changed.

## Goals / Non-Goals

**Goals:**

- Numbers for each go criterion, from a run anyone can repeat.
- A judgement with a reason for every sampled finding.
- A recipe and a pilot plan that follow from the numbers.

**Non-Goals:**

- Tuning qgate during the evaluation. A fix needs its own change.
- Judging the repositories' code quality. The question is only whether
  qgate's findings are worth a reviewer's attention.

## Decisions

### D1. One check per pull request, as the pipeline would run it

For each of the last 50 squash commits `C` on `main`:
- check out `C`;
- place the repository's frozen `qgate.toml`, untracked;
- run `qgate check --base C^ --worktree --format json --fail-on never`.

A pull request counts as **commented** when it has at least one finding,
since that is when the Azure job writes a comment. It **would fail** when
it has at least one Error, which is the gate at the default `fail-on`.
Run time is the wall time of that one command.

As built (task 1.1): a pull request is a first-parent commit with one
parent and `(#N)` at the end of its subject, which GitHub appends to a
squash merge. Other commits on `main` are direct pushes and are skipped:
one in `bitwarden/server`, none in the other two. Bot pull requests are
those whose author names `[bot]`, renovate or dependabot. In
`jasontaylordev/CleanArchitecture` they are 41 of the 50, in the other two
7 each.

**Alternative:** `--head C` without `--worktree`. qgate would then look for
`qgate.toml` in `C`, where none exists, so no rule or exclusion would
apply.

### D2. A configuration a team would write, frozen before the first run

Each repository's `qgate.toml` is written from the rules the repository
already enforces, before any finding is seen:
- `no-restricted-imports` in the ESLint configuration;
- .NET architecture tests;
- which projects reference which.

That gives us the rules a team would write, not the rules that make the
numbers look good. The starting points, to be confirmed from the
repositories in task 1.2:
- **`bitwarden/server`:** `Bit.Core` must not use `Bit.Infrastructure`,
  the partner's own kind of rule. Exclude `**/Migrations/**`.
- **`jasontaylordev/CleanArchitecture`:** `Domain` uses nothing of
  `Application`, `Infrastructure` or `Web`; `Application` nothing of
  `Infrastructure` or `Web`. Exclude `**/Migrations/**`.
- **`bitwarden/clients`:** the library boundaries its lint rules state,
  for example that `libs/common` does not import Angular or `apps/`.

As built (task 1.2), none of the three has architecture tests, so the
rules are:
- **the two .NET repositories:** the project references, with namespaces
  as `to_namespaces`;
- **`bitwarden/clients`:** every package-list entry of
  `no-restricted-imports` in `eslint.config.mjs`. The entries a qgate
  pattern cannot express are named in the file.

Each file names the source of each rule. Since every merged pull request
already compiled and passed lint, these rules can only fire on what the
compiler and ESLint let through, so "no signal" is the expected
result for them (see Risks).

The files are committed in `evaluation/config/` before the first run. If
the results suggest a better configuration, the change is a second run
on the same pull requests, reported next to the first.

### D3. Judging a sample, with reasons

- **The sample:** every Error, and up to 30 Warnings per repository,
  drawn with Python's `random.Random(20261117)` from the Warnings in
  report order.
- **The classes:** each sampled finding gets one, with a sentence of
  reason in `evaluation/findings.md`:
  - **useful:** a reviewer would act on it, or want to know;
  - **defensible:** correct under the rule, though a reviewer might let
    it pass;
  - **noise:** wrong, or about code nobody reviews (generated,
    vendored, a rename taken for new code).

The agent judging its own tool is a bias, so two things check it. Every
reason is written down, and Ervin spot-checks 10 findings drawn at
random from the sample before the go/no-go. If he disagrees on more
than 2, the sample is judged again with his corrections as the
standard.

### D4. How each criterion is read

| Criterion | Read from | Pass |
|---|---|---|
| False Errors | sampled Errors classed noise | 0 |
| Commented pull requests | commented ÷ 50, per repository, with and without bot pull requests | at most about 1 in 3 in each repository |
| Useful or defensible | (useful + defensible) ÷ sample, all repositories | at least 70% |
| Run time | the slowest of the 50 runs in `bitwarden/clients`, the largest repository | at most 30 s |

Every count is also reported per rule, so a failed criterion points at
its cause. A criterion that fails is not turned into a no-go by itself.
It names the fix change to propose, and after that fix the run is
repeated.

### D5. The binary and the machine

The published 0.9.0 release binary: that is what a pilot would install,
and it is the same for TypeScript and C# as `main`. It runs on this
machine (aarch64, Ubuntu 26.04) from full clones, with no network. A
Microsoft-hosted agent is a different machine, so the run time measured
here is a guide, not a promise. The pilot measures it on the partner's
agents.

### D6. The pilot plan

Written in task 3.2, from the numbers:
- **Comment-only start:** `--fail-on never` on both `qgate check`
  lines, with the recipe's `qgate.toml` adapted to the partner's
  projects.
- **Week 1:** the measurement of D1–D4 on the partner's last 50 pull
  requests, run by us with read access or by them with the scripts. It
  produces the same criteria table.
- **Switch to blocking:** only after 0 false Errors over the pilot's
  pull requests so far, and with the partner's agreement. Then use the
  default `fail-on` (Errors).
- **Feedback:** where the team reports a finding it disagrees with, and
  how such a report turns into a fix change.

As written: "Pilot plan (task 3.2)" below. Beyond this outline:
- a test pull request before the start, since the qgate job runs in the
  pipeline whose policy already gates the build;
- every Error on the pilot's pull requests is judged, and blocking waits
  at least four weeks;
- `list.py` learned the Azure Repos form of a pull request commit, and
  `sample.py` names any repository. On the three repositories, both
  still give the same list and sample as before.

## Evaluation (task 3.1)

Run on 2026-11-17 with the 0.9.0 release binary
(`aarch64-unknown-linux-musl`). The heads of `main` were:
- `jasontaylordev/CleanArchitecture` `57c9d5d`;
- `bitwarden/server` `5770319`;
- `bitwarden/clients` `162f1fa`.

The numbers come from `summary.py` and `judge.py` over the saved
reports. The per-finding judgements are in `evaluation/findings.md`.

### The criteria

| Criterion | Measured | Pass |
|---|---|---|
| False Errors | 1 of the 17 Errors (CL11, `bitwarden/clients` PR 24645) | **no** |
| Commented pull requests | see below | **no** for `bitwarden/clients`; `bitwarden/server` passes only with its bot pull requests counted |
| Useful or defensible | 54 of 78 (69.2%): Errors 16 of 17, Warnings 38 of 61 | **no**, narrowly |
| Run time, `bitwarden/clients` | median 5.8 s, slowest 10.9 s in the run | yes |

Commented pull requests per repository:

| Repository | All | Human | Bot |
|---|---|---|---|
| `jasontaylordev/CleanArchitecture` | 1 of 50 (2%) | 1 of 9 | 0 of 41 |
| `bitwarden/server` | 17 of 50 (34%) | 17 of 43 (40%) | 0 of 7 |
| `bitwarden/clients` | 22 of 50 (44%) | 22 of 43 (51%) | 0 of 7 |

Run time:
- The same pull request (24475) took 5.5 s, 8.5 s and 15.7 s in three
  runs, so this machine's timings vary up to threefold.
- With `--head` instead of `--worktree`, as the pipeline runs it, it
  took 7.2 s.
- Every timing is under the 30 s threshold.

A pull request would fail when it has an Error. That happened 3 times in
`bitwarden/server` (PRs 8781, 8812, 8838) and 5 times in
`bitwarden/clients` (24324, 24427, 24475, 24645, 24690).

### Per rule

| Rule | `server` findings / PRs / only rule in | `clients` findings / PRs / only rule in |
|---|---|---|
| `complexity/cyclomatic-increase` | 17 (4 Errors) / 8 / 1 | 29 (4 Errors) / 15 / 4 |
| `complexity/cyclomatic-new` | 5 (2 Errors) / 4 / 0 | 10 / 4 / 0 |
| `duplication/duplicate-block` | 181 / 15 / 7 | 98 / 5 / 0 |
| `tests/missing-test` | 2 / 2 / 0 | 16 / 1 / 0 |
| `tests/unchanged-test` | 1 / 1 / 0 | 20 / 14 / 5 |
| `architecture/forbidden-import` | 0 | 7 (Errors) / 1 / 1 |
| `input/unparsable` | 1 / 1 / 0 | 0 |

`jasontaylordev/CleanArchitecture` had one finding: a +1 complexity
Warning.

- **What drives the comment rate differs per repository.** Human pull
  requests that would still be commented:
  - `bitwarden/server` without duplication findings: 10 of 43, against
    17 of 43;
  - `bitwarden/clients` without `tests/unchanged-test` and without
    complexity Warnings of +1 or +2: 12 of 43, against 22 of 43.
- **Where the noise comes from.** Duplication accounts for 22 of the 24
  noise findings: a file split into several, the same edit to blocks
  that were copies already, and code moved out of a deleted file. A
  function moved to another class (SV19) accounts for one more.
- **The false Error** (CL11) is a function inlined while a feature flag
  was removed. The patterns are in `evaluation/findings.md`.

### No signal

- **No architecture rule fired in the two .NET repositories,** and
  in `bitwarden/clients` only `angular` did. This was expected (D2):
  every merged pull request had compiled and passed lint. The one that
  fired (PR 24475) shows the remaining case. A file moved into
  `libs/angular` kept imports that the team had allowed with
  `eslint-disable-next-line`. qgate reports them correctly, but it has
  no way to accept a known violation, so in blocking mode that pull
  request could not pass.
- **41 of the 50 pull requests of `jasontaylordev/CleanArchitecture`
  are renovate dependency updates.** It had one finding in 9 human pull
  requests, so it shows only that qgate stays quiet on a small, stable
  .NET code base. `bitwarden/server` carries the .NET result.
- **No bot pull request got a comment** in any repository.

### Found on the way

- **C# 14 extension members.** PR 8871 of `bitwarden/server` adopts C# 14
  `extension(...)` blocks, and qgate does not handle them yet:
  - one file is `input/unparsable`;
  - in another, a moved method is named
    `RecipeBuilderExtensions.extension(RecipeBuilder).Validate()` and
    reported as a new function.

  A .NET 10 partner will write these.
- **A function that only gains a parameter counts as a new function.**
  In SV6 a parameter was added, so the method was reported as new. The
  verdict was right: as an increase it is 14 -> 25, still an Error.

### Verdict: no-go until two fixes are measured

Three criteria fail; none of them points at the idea itself. The Errors
qgate raised were right 16 times out of 17, and the useful findings sit
exactly where a reviewer would look: autofill, seat reservation, SAML
hotspots. What fails comes from a few specific mechanisms. The fix
changes to propose, each a small change of its own:

1. **Duplication in moved and edited code** (P1, P2 and SV10; 22 of the
   24 noise findings). A block counts as existing when the base had it
   anywhere, also repeated inside one file or in a file the change
   deletes. A block that was a copy before and is edited on both sides
   is not a new copy.
2. **Complexity of moved code** (CL11, the false Error, and SV19). An
   increase made of code that the same change removed from another
   function of the file, such as an inlined function after a feature
   flag is removed, is not new complexity. The same goes for a function
   that moved to another class or file.

After both, the same 150 pull requests are measured again with the
fixed binary and the frozen configurations, and this section gets the
second table.

Two things are Ervin's to decide:
- **The comment rate of `bitwarden/clients`.** It is driven by
  `tests/unchanged-test` (including changes like `readonly` or an i18n
  key) and by +1/+2 complexity Warnings, not by noise. Neither
  `qgate.toml` nor a flag can turn a rule down today. The options:
  - accept the rate and judge the threshold on the partner's own pull
    requests in week 1;
  - a third change that makes these two quieter.
- **Accepting a known violation before blocking** (PR 24475). It does
  not matter while the pilot only comments. Before the switch to
  blocking, a team needs either a way to accept a finding or the rule
  that such a pull request is merged with an override.

Decided by Ervin on 2026-11-17:
- **The complexity fix comes first,** since it removes the only false
  Error. The duplication fix follows.
- **The comment rate is judged on the partner's pull requests.** No
  third change now: week 1 of the pilot measures the rate on the
  partner's last 50 pull requests, and only a rate above about 1 in 3
  there leads to quieter rules.
- **Accepting a known violation goes into the pilot plan (D6)** as a
  condition of the switch to blocking, not into a change now.

This change waited, as Gate 1 agreed: after both fixes, the second
measurement below, then tasks 3.2 and 3.3.

### Second measurement: 0.10.0

Run on 2026-11-18 with the 0.10.0 release binary
(`aarch64-unknown-linux-musl`, checked against its `SHA256SUMS`). The
same 150 pull requests, clones and frozen configurations were used,
with `run.py` and `summary.py` unchanged. The fixes it carries are
`complexity-of-moved-code` with `limit-move-evidence`, and
`duplication-of-new-code`.

`remeasure.py` compares the run with the first one finding by finding.
It finds no finding that the first run did not have. So the findings
left are a subset of the first run's, every Error is still judged, and
the judged Warnings that are left remain a random sample of the
Warnings left in each repository. The script prints 22 judged findings
as gone and SV28 as moved, since it overlaps SV23. By hand, SV28's own
block is gone, so 23 of the 78 judged findings are gone:
- 22 of the 24 noise findings: the duplication noise except SV23 and
  SV27, plus SV19 (the moved function) and CL11 (the false Error);
- 1 defensible finding: SV30, the known price of
  `duplication-of-new-code`.

| Criterion | 0.9.0 | 0.10.0 | Pass |
|---|---|---|---|
| False Errors | 1 of 17 | 0 of 16 | yes |
| Commented pull requests | see below | see below | `bitwarden/server` yes; `bitwarden/clients` no, left to week 1 (Ervin's decision above) |
| Useful or defensible | 54 of 78 (69.2%) | 53 of 55 (96.4%) | yes |
| Run time, `bitwarden/clients` | slowest 10.9 s | median 5.3 s, slowest 5.6 s | yes |

| Repository | All | Human | Bot |
|---|---|---|---|
| `jasontaylordev/CleanArchitecture` | 1 of 50 (2%) | 1 of 9 | 0 of 41 |
| `bitwarden/server` | 14 of 50 (28%) | 14 of 43 (33%) | 0 of 7 |
| `bitwarden/clients` | 22 of 50 (44%) | 22 of 43 (51%) | 0 of 7 |

- **`bitwarden/server` drops from 17 to 14 commented pull requests.**
  Duplication findings fall from 181 to 59, and the pull requests they
  appear in from 15 to 10.
- **`bitwarden/clients` keeps 22.** Its duplication falls from 98 to 80
  findings, in 3 pull requests instead of 5, but those 3 pull requests
  have other findings too. Its rate is driven by `tests/unchanged-test`
  (now the only rule in 7 pull requests) and by complexity Warnings, as
  the first run found.
- **Pull requests that would fail:** 3 in `bitwarden/server` (8781, 8812,
  8838) and 4 in `bitwarden/clients` (24324, 24427, 24475, 24690). PR
  24645 no longer fails, since its only Error was CL11.

### Verdict after the fixes: go for a comment-only pilot

Three of the four criteria pass. The fourth, the comment rate of
`bitwarden/clients`, is what Ervin decided to judge on the partner's own
pull requests in week 1 of the pilot. The only false Error is gone, and
96% of what qgate still reports is useful or defensible. The pilot plan
(D6) and the recipe follow.

## Pilot plan (task 3.2)

The roles: **Ervin** for qgate, and on the partner's side a **tech
lead**, who speaks for the team, and a **pipeline owner**, who may edit
the pipeline and the repository's policies. One person may hold both
partner roles. The partner's code, reports and judgements stay with the
partner: this public repository gets only numbers without code, and the
fix changes they lead to.

1. **Configuration.**
   - *Who:* Ervin with the tech lead, before the start.
   - *What:* the recipe's `qgate.toml` (README, "Angular + .NET")
     adapted to the partner's projects. The layer rules come from the
     project references and any import lint rules, as in D2, and every
     rule names its source. The exclusions are `**/Migrations/**` and the
     folders of generated code.
   - *Produces:* `qgate.toml` in the partner's repository, merged by a
     pull request of its own. `qgate check --base HEAD --worktree` runs
     without a configuration error.
2. **Comment-only start.**
   - *Who:* the pipeline owner.
   - *What:* the recipe's job, with `--fail-on never` on both `qgate
     check` lines, added to the pipeline that builds the pull requests,
     on the agent the partner builds on (Linux or Windows), and the build
     service's permission to comment. That pipeline's policy already
     gates the build, and the qgate job still fails it when qgate cannot
     run (exit code `2`) or cannot comment. So the job is tried first on
     one test pull request with a finding.
   - *Produces:* the test pull request with its qgate comment; then one
     qgate comment on each pull request with findings, and a pipeline
     that qgate does not turn red.
3. **Week 1: the same measurement on the partner's code.**
   - *Who:* Ervin runs the scripts and judges, with read access to the
     repository. If the code may not leave the partner, a developer the
     tech lead names does both, with the classes and reasons of D3.
     Whoever did not judge spot-checks: the tech lead, or Ervin in a
     session where the code stays on the partner's screen.
   - *What:* the steps of D1–D4 on the partner's last 50 pull requests,
     with the partner's `qgate.toml` and the release the job installs:
     `list.py` (which also reads Azure Repos' `Merged PR N:` subjects),
     `run.py`, `summary.py`, `sample.py`, the judgements and `judge.py`.
     `list.py` marks bots by the commit's author. In Azure Repos, the
     author of that commit is whoever completed the pull request, so the
     bot pull requests are marked by their creator instead, from Azure
     DevOps. The spot-check takes 10 judgements drawn by
     `spotcheck.py`; more than 2 disagreements send the judging round
     again (D3). The run time of the qgate job is read from the week's
     pull request builds, on the partner's own agents.
   - *Produces:* the criteria table of D4 for the partner's code, a
     judged sample in the format of `findings.md` (kept by the
     partner), and the job's run times.
   - *Then:*
     - a false Error is a fix change, and the pilot stays comment-only;
     - a comment rate above about 1 in 3 is the third change
       postponed above: `tests/unchanged-test` and small complexity
       Warnings made quieter (Ervin's decision of 2026-11-17);
     - other noise with a nameable mechanism is a fix change, as
       `duplication-of-new-code` was.
4. **Feedback during the pilot.**
   - *Who:* a developer who disagrees with a finding, and Ervin with the
     tech lead.
   - *What:* the developer answers in the qgate comment's thread with
     the finding's rule and line and why it is wrong. The thread stays
     when later builds update the comment. Once a week, Ervin and the
     tech lead judge, as in D3, each answered finding and every Error
     that a qgate comment showed that week, answered or not.
     A finding judged noise with a nameable mechanism becomes a fix
     change in this repository, with fixtures that describe the
     mechanism, not the partner's code. A release follows, and the
     pipeline owner updates the version in the job.
   - *Produces:* a feedback list with the pull request, finding, reason,
     judgement and fix change, kept by the partner; the count of the
     week's Errors and how many were wrong; fix changes and releases
     here.
5. **Switch to blocking.**
   - *Who:* the tech lead and Ervin decide together; the pipeline owner
     makes the switch.
   - *When:* all of these hold:
     - at least four weeks of comments;
     - 0 false Errors among every Error judged so far: the week-1
       sample's and those of the pilot's own pull requests (step 4);
     - an agreed way through a known violation. qgate has no way to
       accept a finding today (PR 24475 above). Either such a feature
       is built as a change of its own first, or the team agrees that
       such a pull request is completed with a policy override by a
       named person, and the override is noted on the pull request;
     - the tech lead agrees.
   - *What:* both `--fail-on never` options are removed, so the default
     `fail-on` (Errors) gates through the pipeline's policy, which must
     be *Required*.
   - *Produces:* a pipeline that blocks a pull request adding an Error,
     and the date of the switch in the feedback list. Warnings still
     only comment.

## Recipe (task 3.3)

The README's "Recipe: Angular + .NET in Azure DevOps" has these parts:
- a `qgate.toml` with the layer rules of the Clean Architecture
  template, one Angular rule, and the two exclusions the evaluation
  used: `**/Migrations/**` (`bitwarden/server`) and `**/generated/**`
  (`bitwarden/clients`);
- the Linux job in comment-only mode, a note for Windows agents, and
  a test pull request first (pilot plan, step 2);
- "Then a gate": the switch to blocking of step 5, in short. The other
  steps involve the partner and stay here.

Checked on 2026-11-18:
- **The `qgate.toml` loads.** The recipe's `qgate.toml`, placed in the
  `jasontaylordev/CleanArchitecture` clone at its head, gives "No
  regressions found." and exit code `0` with `--base HEAD --worktree`.
  With an unknown key added, it fails with exit code `2`, so the file
  is read.
- **The job is the README's Linux Azure job,** apart from `--fail-on
  never` on both `qgate check` lines. A script compared the two blocks
  of the README.

## Risks / Trade-offs

- **[The stand-ins are not the partner's code]** → they share the stack
  and the kind of rules. Week 1 of the pilot repeats the measurement on
  the partner's own code before anything blocks.
- **[A layer rule may never fire in 50 pull requests]** → this is
  reported as "no signal", not as a pass. A rule that is never broken
  says nothing about false Errors. Reverting a known violation by hand
  is not done: it would measure the fixture, not the stream.
- **[Bias in judging]** → reasons are written per finding, and Ervin
  spot-checks (D3).
- **[Large repositories make duplication slow]** → the 0.7.0 evaluation
  saw duplication over the whole head tree dominate the run time in the
  large repositories. The run-time criterion exists for this. If it
  fails, the fix is a change of its own.

## Migration Plan

None. Nothing ships. The README recipe is documentation of features
0.10.0 already has.
