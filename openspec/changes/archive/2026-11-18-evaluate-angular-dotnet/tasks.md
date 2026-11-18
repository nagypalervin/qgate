# Tasks

One pull request (`change/evaluate-angular-dotnet`). Clones and run
output stay in the scratchpad; scripts, configurations and the
per-finding list go to `evaluation/`. Commit messages and the PR body are
written to files first.

## 1. Corpus and configuration

- [x] 1.1 Full clones of `bitwarden/clients`, `bitwarden/server` and `jasontaylordev/CleanArchitecture` outside the repository, and the 0.9.0 Linux arm64 release archive checked against its `SHA256SUMS`. `evaluation/list.py` lists the last 50 first-parent commits of each `main`, marks bot pull requests by author (renovate, dependabot) and records each repository's head commit. Verified by `qgate --version` printing `qgate 0.9.0` and 50 rows per repository in `evaluation/commits.tsv`.
- [x] 1.2 One `qgate.toml` per repository in `evaluation/config/`, written from the rules the repository already enforces (D2): ESLint `no-restricted-imports`, .NET architecture tests, project references. Each rule carries a comment naming its source. Committed before any run. Verified by a check with an empty diff (`--base HEAD --worktree`) in each clone exiting 0 without a configuration error.

## 2. Run and judge

- [x] 2.1 `evaluation/run.py`: for each listed commit, check it out, place the frozen `qgate.toml` and run `qgate check --base C^ --worktree --format json --fail-on never` (D1), saving the JSON and the wall time. Verified by 150 JSON reports that all parse, and a second run of one commit giving the same findings.
- [x] 2.2 `evaluation/summary.py`: per repository, commented and would-fail pull requests, with and without bots; findings per rule and severity; the slowest and median run time (D4). Verified by the script printing the same numbers twice from the saved reports.
- [x] 2.3 The sample of D3 (every Error, up to 30 Warnings per repository, `random.Random(20261117)`), drawn by `evaluation/sample.py`, and each sampled finding judged useful, defensible or noise with one sentence of reason in `evaluation/findings.md`. Verified by every sampled finding appearing in `findings.md` exactly once with a class and a reason.
- [x] 2.4 Ervin's spot-check: 10 findings drawn at random from the sample, shown to him with the code they point at. His verdicts are recorded in `findings.md`. Verified by the record; more than 2 disagreements send 2.3 round again (D3).

## 3. Verdict, recipe and pilot plan

- [x] 3.1 An "Evaluation" section in `design.md` with the criteria table of D4, per-rule counts, every "no signal" rule, and go or no-go. A failed criterion names the fix change to propose, and the work stops here until Ervin decides. Verified by each number in the table matching `summary.py` output.
- [x] 3.2 The pilot plan (D6) in `design.md`: the comment-only start, the week-1 measurement with these scripts, the condition and the step to blocking, the feedback route. Verified by every step naming who does it and what it produces.
- [x] 3.3 README "Angular + .NET" recipe: a `qgate.toml` with layer rules and the exclusions the evaluation supports, and the Azure Pipelines job of the README's Azure section with `--fail-on never` on both check lines. Verified by the recipe's `qgate.toml` loading in the `CleanArchitecture` clone without a configuration error, and the job differing from the README's Azure job only in `--fail-on`.

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- The `docs/journal/` entry, then `/opsx:archive` on this branch.
- Gate 3: Ervin merges the pull request.
