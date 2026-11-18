# Proposal: evaluate-angular-dotnet

## Why

The partner pilot is next: an Angular frontend and a .NET backend on
Azure DevOps. Each analyzer was evaluated alone when it was added:
complexity, duplication, `using` rules, missing tests, hotspots. Each
time the question was whether that rule is right. Nobody has yet looked
at qgate the way a team will see it in the pilot:
- all analyzers in one comment per pull request;
- over a stream of real pull requests;
- with a `qgate.toml` a team would write.

Before the pilot we need to know three things: whether that experience
is worth having, which configuration to hand the partner, and how to
roll it out.

## What Changes

- **An evaluation on public stand-ins for the partner's stack:**
  - `bitwarden/clients` (Angular);
  - `bitwarden/server` (.NET, with a Core and Infrastructure split);
  - `jasontaylordev/CleanArchitecture` (.NET layers and an Angular client).

  All three merge pull requests by squash, so each commit of `main` is
  one pull request. Each repository gets:
  - the last 50 pull requests;
  - full history, for hotspots;
  - the published 0.9.0 release binary, the one a pilot would install;
  - every analyzer, with one `qgate.toml` written before the first run.
- **Go criteria fixed in advance,** decided by Ervin on 2026-11-17:

  | Criterion | Threshold |
  |---|---|
  | Errors that would fail a pull request without reason | 0 in the sample |
  | Pull requests that get a comment | at most about 1 in 3 |
  | Findings judged useful or defensible in the sample | at least 70% |
  | Run time per pull request, largest repository, Linux | at most 30 s |

  A failed criterion is not an automatic no-go. It leads to a targeted
  fix in a separate small change, and the evaluation is measured again.
- **Three deliverables:**
  - the evaluation report: numbers, per-finding judgements, scripts, and
    go or no-go;
  - an "Angular + .NET" recipe in the README: a `qgate.toml` with layer
    rules and exclusions, and the Azure Pipelines job in comment-only
    mode (`--fail-on never`);
  - the pilot plan: what the pilot measures and when it may start
    blocking pull requests.
- **The pilot starts comment-only,** decided by Ervin. The switch to
  blocking needs 0 false Errors on the pilot's own data. In the first
  week of the pilot, the same measurement runs on the partner's code.
- **No behavior change** (`skip_specs`): no code, spec or release.

### Out of scope

- **Fixes the evaluation may call for.** They become their own changes,
  as `add-path-exclusion` did after the C# evaluation.
- **The partner's own code.** It is measured in the pilot's first week,
  not now.
- **Windows run time on a large repository.** The Windows setup is
  already rehearsed in Azure. Timing it on the partner's code is part of
  the pilot.
- **A comparison with CodeScene,** which `bitwarden/clients` uses: a
  different kind of tool, and a possible article, not a criterion.

### For Gate 1

Approved by Ervin on 2026-11-17 with all five recommendations.

1. **Repositories and size:** the three above, 50 pull requests each.
   Dependency-bot pull requests count, because they are part of the
   stream a team sees, but they are reported separately as well.
   *Recommended: yes.*
2. **Configuration frozen before the first run** (design D2). If the
   numbers then suggest a better configuration, the change is recorded
   as a second run, so the recipe shows both the before and after.
   *Recommended: yes.*
3. **Who judges the findings** (D3): the agent classifies the sample
   (every Error, and up to 30 Warnings per repository, drawn with a fixed
   seed) as useful, defensible or noise, with a written reason for each.
   Ervin spot-checks 10 at random before the go/no-go.
   *Recommended: yes.*
4. **Where the pilot plan lives:** in `design.md` of this change,
   archived next to the evidence it rests on. The README gets only the
   recipe. *Recommended: yes.* (As built, the recipe ends with the switch
   to blocking in a few lines, because a reader of the README needs to
   know when comments may become a gate.)
5. **One pull request,** with the report, the recipe and the archive. If
   a criterion fails, this change waits for the fix change and measures
   again before its pull request. *Recommended: yes.*

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. qgate's behavior does not change (`skip_specs: true`).

## Impact

- **Repository:**
  - this change's folder, with `evaluation/` for the scripts and the
    per-finding list;
  - the README, with a new "Angular + .NET" recipe;
  - a journal entry.
- **No code, dependency or release.**
- **The pilot:** its configuration and its rollout come from this
  change.
