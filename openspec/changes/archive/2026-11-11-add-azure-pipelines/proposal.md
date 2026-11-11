# Proposal: add-azure-pipelines

## Why

The partner teams build their pull requests in Azure DevOps Services,
on Linux and Windows agents. Since qgate 0.1.0, a CI agent can install
qgate from a release, but the README only shows GitHub Actions. Azure
Pipelines differs in exactly the places where a copied GitHub setup
would silently stop working:
- the default checkout is a single commit;
- the target branch reads `refs/heads/main`;
- a failing command does not stop a script step.

## What Changes

- **README, a new section "In Azure Pipelines":**
  - a `qgate` job for a Microsoft-hosted **Linux** agent (`ubuntu-latest`, bash);
  - the same job for a **Windows** agent (`windows-latest`, PowerShell 7).

  Each job:
  - checks out the full history;
  - installs a pinned qgate release into the agent's temp directory, with the checksum check;
  - runs `qgate check` against the pull request's target branch.

  It runs only for pull request builds. The section also covers:
  - turning the pipeline into a merge gate with a branch policy;
  - what self-hosted and arm64 agents need.
- **README, the section "In CI" becomes "In GitHub Actions",** because
  that is all it covers. The one link to it is updated.
- **A rehearsal.** The final README jobs, copied verbatim, run in a
  rehearsal project in Ervin's Azure DevOps organization, on both
  operating systems:
  - a pull request with a finding fails and is blocked;
  - a clean one passes;
  - a non-PR build skips the job;
  - a corrupted download stops the install.

  The results are recorded in `design.md`.

No qgate code changes.

### Decisions taken during exploration

- **Azure DevOps Services, Linux and Windows agents** (Ervin). The
  partners do not use the on-prem Azure DevOps Server, so hosted agents
  can reach GitHub, and installing from the release works.
- **README snippets, not a shared template or a Marketplace extension.**
  - A YAML template in this repository needs a GitHub service connection in every consuming project.
  - An extension needs a publisher account, a Node task and its own release channel.

  Snippets can be tested in full now. A template or a task can follow if partners ask for one.
- **The PR gate first, PR comments next.** In this change, findings
  appear only in the job log; the pull request shows a failed build
  with "exited with code 1". Showing findings in the build summary
  (`##vso[task.logissue]`) and as a PR comment needs new output formats
  in qgate. That is the next change, `add-pr-comments`.
- **A hybrid rehearsal** (Ervin). Ervin makes the admin settings in
  the web UI: the project, the pipeline, the branch policy. The agent
  pushes branches, opens pull requests and reads build logs through the
  REST API, with a narrow, short-lived token kept outside the
  repository.
- **Spike, 2026-11-10,** in the rehearsal project `qgate-rehearsal`,
  pipeline builds 1286 to 1290:
  - **The default checkout fetches one commit and no branches.**
    `fetchDepth: 0` brings the history and `origin/main`.
  - **The pull request build checks out `refs/pull/<id>/merge`.** Its
    first parent is the tip of the target branch. So
    `--base origin/main` measures exactly what the pull request changes.
  - **A bash step does not stop at a failing command.** The probe went
    on after `false`. With `set -euo pipefail`, a corrupted archive
    stopped the install before unpacking.
  - **PowerShell has the same trap.** A failing native command does not
    stop a `pwsh` step, even with `$ErrorActionPreference = "Stop"`.
    `$PSNativeCommandUseErrorActionPreference = $true` (PowerShell 7.4
    and later; the agent has 7.6.9) does stop it. The README's
    PowerShell install block stopped on a corrupted archive. It had not
    been tried on Windows before.
  - **`System.PullRequest.TargetBranch` is `refs/heads/main`.** Outside
    a pull request build it is not empty: it stays as the literal text
    `$(System.PullRequest.TargetBranch)`.
  - **The YAML `pr:` trigger does not apply to Azure Repos.** The
    branch policy's build validation queued the pull request builds.
  - **qgate 0.1.0 worked on both agents.** A complexity increase
    failed the build and the policy rejected the pull request. A
    README-only pull request passed.

### Out of scope

- Findings in the build summary or as PR comments (`add-pr-comments`).
- A YAML template in this repository, a Marketplace extension or task.
- Azure DevOps Server (on-prem) and agents without access to GitHub.
- Repositories hosted on GitHub but built in Azure Pipelines. The
  documented job may work there, but it is not rehearsed.
- Rehearsing macOS and arm64 agents. The README says what changes for
  them.
- Keeping the README's pinned version up to date automatically.

## Capabilities

### New Capabilities

- `ci-integration`: the CI setups qgate documents and rehearses, and
  what each one guarantees:
  - it installs a verified release;
  - it checks what the pull request changes;
  - it fails the build at the `--fail-on` level;
  - it does nothing outside pull request builds.

  This change adds Azure Pipelines. The GitHub Actions setup already in
  the README can be specified there later.

### Modified Capabilities

None. qgate itself does not change.

## Impact

- **Docs:** `README.md` only.
- **Code, CI, workflows:** none in this repository.
- **Outside the repository:**
  - the rehearsal project `qgate-rehearsal` in Ervin's Azure DevOps organization;
  - a personal access token in `~/.config/qgate-rehearsal/pat`. Its
    scopes are Code (read and write) and Build (read and execute), and
    it expires seven days after 2026-11-10.
- **One pull request** (`change/add-azure-pipelines`): the README, the
  rehearsal record, the journal entry and the archive.
