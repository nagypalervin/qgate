# Design: add-azure-pipelines

## Context

See `proposal.md` for the why and for the spike results.

- **README "In CI"** covers GitHub Actions only:
  - an install step on `ubuntu-latest`;
  - `checkout` with `fetch-depth: 0`;
  - `qgate check --base "origin/$BASE_REF"`;
  - a JSON-report variant.

  The Claude Code hook section links to it as `#in-ci`.
- **README "Try it"** has a PowerShell install block. It throws on a
  checksum mismatch and unpacks with `Expand-Archive`. The spike ran it
  on `windows-latest`, its first run on Windows.
- **qgate's `--base`** compares the merge base of `--base` and the head
  with the head (the `revision-input` spec). Its exit codes are 0, 1
  (findings at or above `--fail-on`) and 2 (could not run).
- **The rehearsal project** is `teamcollabeu/qgate-rehearsal`:
  - pipeline 36 runs `/azure-pipelines.yml`;
  - a required build validation policy is on `main`, so `main` takes no
    direct pushes (TF402455);
  - the spike's pull requests 1339 (a complexity increase) and 1340
    (README only) are still open.

## Goals / Non-Goals

**Goals:**
- A partner copies one job into their pipeline, adds a branch policy,
  and has a qgate merge gate. They need no token, service connection
  or extra permission.
- Every line the README shows ran in the rehearsal, on the agent it is
  documented for.
- A failed download, a bad checksum or a failing command stops the job.
  None of them can pass as success.

**Non-Goals:**
- Findings outside the job log (`add-pr-comments`).
- Sharing the job as a template or task (D1).
- macOS and arm64 agents, Azure DevOps Server, GitHub-hosted repositories
  built in Azure Pipelines.

## Decisions

### D1. README snippets, not a template or an extension

The job is documented in the README, for users to copy.

- **A YAML template in this repository.** A consuming pipeline would
  reference it through `resources.repositories` with `type: github`.
  That needs a GitHub service connection in every consuming project,
  even though the repository is public. Partners would also take on
  this repository's update cadence with every `ref` bump.
- **A Marketplace extension with a task** (`- task: qgate@0`). This is
  the nicest to use. It is also a product of its own: a publisher
  account, a Node.js task, packaging with `tfx`, and versions separate
  from qgate's.

The snippet can be rehearsed in full today, and a partner can read
every line of it. Either alternative can wrap the same steps later if
partners ask for one.

### D2. A separate job, gated on the build reason

The snippet is a whole job, not steps to insert into an existing job:

```yaml
- job: qgate
  condition: eq(variables['Build.Reason'], 'PullRequest')
  pool:
    vmImage: ubuntu-latest
  steps: ...
```

- **It runs next to the existing build and test jobs.** A failing
  build does not hide qgate's findings, and qgate does not hide
  failing tests.
- **The job condition skips the whole job, checkout included,** on
  anything other than a pull request build. Its target branch would be
  the literal text `$(System.PullRequest.TargetBranch)` there (spike).
  A skipped job does not fail the build. The rehearsal checks this
  (D7).
- **The job's agent can differ from the build's agent.** qgate reads
  the sources from git, not from a build output. A team that builds on
  Windows can run the Linux job, and the other way round. The README
  says so, and shows both variants because partners asked for both.

A pipeline that has only top-level `steps:` must be turned into
`jobs:` first. The README says how in one sentence.

### D3. Checkout and base

- **`checkout: self` with `fetchDepth: 0`.** The default fetched one
  commit and no branches (spike), so `origin/main` and the merge base
  did not exist.
- **The base is `origin/<target>`.**
  - `<target>` is `System.PullRequest.TargetBranch` without its
    `refs/heads/` prefix.
  - The variable reaches the script through `env:`, not through `$(…)`
    macro syntax inside the script.
  - The pull request build checks out `refs/pull/<id>/merge`, whose
    first parent is the target's tip (spike). So the merge base is that
    tip, and qgate sees exactly what the merge would change.
- **Stripping only a leading `refs/heads/`** leaves a plain branch name
  unchanged. This is how GitHub-hosted repositories report it, untested
  (proposal, out of scope).

### D4. Failing commands stop the step, in both shells

The spike measured that neither shell stops at a failing command by
default (proposal).

- **bash, install step:** starts with `set -euo pipefail`.
  - The pipeline `grep … | sha256sum -c` fails when the line is missing
    or the sum differs.
  - `curl -fsSL --fail-early` fails on an HTTP error.
- **PowerShell, install step:** a `pwsh:` step (PowerShell 7), not
  `powershell:` (Windows PowerShell 5.1). It starts with:
  ```powershell
  $ErrorActionPreference = "Stop"
  $PSNativeCommandUseErrorActionPreference = $true
  ```
  - The first line stops on a failing cmdlet, such as `Invoke-WebRequest`
    on an HTTP error, or on the `throw` for a checksum mismatch.
  - The second line stops on a failing native command. It needs
    PowerShell 7.4 or later; the agent has 7.6.9.
  - The README's "Try it" block calls no native command. The second line
    is there so that a command a user adds later cannot slip through.
- **The check step is the single `qgate check` command,** in both
  shells. The step's exit code is the command's, so exit code 1 or 2
  fails the job (spike: "Bash exited with code '1'", "PowerShell exited
  with code '1'").
  - No preference lines are needed there.
  - In PowerShell, `$PSNativeCommandUseErrorActionPreference` would turn
    exit code 1 into an error record with a stack trace around qgate's
    own message, which only adds noise.

### D5. Install location and version

- **The release is unpacked in `$(Agent.TempDirectory)`.** This is
  outside the checkout, so qgate never analyzes itself. The agent
  empties the directory after every job.
- **`##vso[task.prependpath]`** adds the unpacked directory to `PATH`
  for the later steps.
- **The version is pinned (`v=0.1.0`), as in the rest of the README.**
  A CI gate must not change behaviour because of a new release. The
  README now holds the version in four install blocks. Updating them is
  part of a release and stays manual (proposal, out of scope).
- **The target is the x64 one on both hosted agents.** The README names
  the arm64 targets for self-hosted arm64 agents.

### D6. README structure

- **"In CI" becomes "In GitHub Actions",** and the hook section's link
  changes from `#in-ci` to `#in-github-actions`. No other file links to
  it.
- **A new section "In Azure Pipelines" follows it:**
  1. One paragraph: what the job does, and that it is a separate job
     for pull request builds.
  2. The Linux job. Then the Windows job, with one sentence saying that
     the agent need not match the build's.
  3. **Making it a merge gate:** the build validation policy, as five
     steps in the web UI (*Repos → Branches → main → Branch policies →
     Build validation*). The YAML `pr:` trigger does not apply to Azure
     Repos.
  4. **Notes:**
     - findings appear in the job log; the pull request shows the
       failed build;
     - `--fail-on` works as in GitHub Actions;
     - self-hosted agents need `curl`, `sha256sum` and `tar` (Linux) or
       PowerShell 7.4+ as `pwsh` (Windows), and access to github.com;
     - the arm64 target names.

### D7. The rehearsal

**Where:** the rehearsal project `teamcollabeu/qgate-rehearsal` (pipeline
36).

**What runs:** the README's jobs, extracted mechanically from the README
and placed under `jobs:` in `/azure-pipelines.yml`, one operating system
per round. Both jobs are named `qgate`, so they cannot share one
pipeline file. The extracted text is compared with `diff` against the
README before every push.

**How it is driven:**
- `main` accepts no direct pushes. Every round therefore uses new pull
  request branches, and pull request builds take the YAML from the
  merge commit.
- The non-PR run is a manual run queued through the REST API on a
  rehearsal branch.

**Scenarios, per round (Linux, then Windows):**

| # | Input | Expected |
|---|---|---|
| 1 | A PR that raises `grade`'s complexity from 2 to 8 | job fails; log shows the `error … [complexity/cyclomatic-increase]` line; policy status `rejected` |
| 2 | A PR that changes only `README.md` | job succeeds; `No regressions found.`; policy `approved` |
| 3 | A manual run on a rehearsal branch | `qgate` job skipped; build not failed by it |
| 4 | A PR whose install step has one added line, after the download, that appends a byte to the archive | install step fails before unpacking; check step does not run |

Scenario 4 is the only one that changes the README's text. The added
line is recorded with the result.

The old spike pull requests 1339 and 1340 are abandoned before the first
round. The project stays for `add-pr-comments`.

**What is recorded in this file:**
- the build ids;
- each scenario's result;
- the policy statuses;
- the `diff` of the README against the rehearsed YAML.

### D8. One pull request

The change has no code. One pull request, `change/add-azure-pipelines`,
carries:
- the README;
- the rehearsal record;
- the review fixes;
- the journal entry;
- the archive.

The rehearsal runs before the review, so the reviewer sees the record.

## Risks / Trade-offs

- **[Hosted images move]** `ubuntu-latest` moves to Ubuntu 26 from
  2026-10-19 (a notice in the spike's builds). `windows-latest` moves
  too. → The Linux binary is static, so the move should not affect it.
  A new rehearsal run is cheap, since the project and the extraction
  stay.
- **[Copied snippets age]** Partners' copies keep their pinned version
  and miss later fixes to the snippet. → This is the price of D1. The
  README section shows the version in one place per job.
- **[The rehearsal runs in a real organization]** `teamcollabeu` holds
  client projects. → The token is scoped to Code and Build and expires
  after seven days. It lives outside the repository and is never
  printed. Only the `qgate-rehearsal` project is touched. Admin
  settings stay with Ervin.
- **[Findings only in the log]** A developer has to open the build to
  see why it failed. → Accepted for this change; `add-pr-comments`
  addresses it.
- **[Branch names with special characters]** `origin/<target>` is
  passed as one argument, and qgate treats revisions only as revisions
  (`revision-input`). → No quoting issue in either shell. Not rehearsed
  beyond `main`.
