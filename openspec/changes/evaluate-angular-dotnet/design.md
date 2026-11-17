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
0.9.0 already has.
