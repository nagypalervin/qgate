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
  README now holds the version in six install blocks:
  - three in "Try it";
  - one in GitHub Actions;
  - two in Azure Pipelines.

  Updating them is part of a release and stays manual (proposal, out of
  scope). The design first said four; the review counted six.
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

## Decisions during apply

- **The Linux install step follows the GitHub Actions block, not the
  "Try it" block (task 1.2).** "Try it" chains its commands with `&&`
  and installs into `~/.local/bin`, which suits a shell, not a CI step.
  The GitHub Actions block is the CI form of the same download and
  check. Compared with `diff`:
  - **Linux step vs the GitHub Actions install block:** it adds
    `set -euo pipefail`, and `##vso[task.prependpath]` takes the place
    of `>> "$GITHUB_PATH"`.
  - **Windows step vs the "Try it" PowerShell block:** it adds
    `$PSNativeCommandUseErrorActionPreference = $true` and the
    `##vso[task.prependpath]` line.
- **The check step is a block scalar (`bash: |`, `pwsh: |`)** even
  though it is one line, so that its quotes, `#` and `$(` reach the
  shell without YAML quoting rules in between.
- **The branch policy is five steps.** Build expiration is set to
  "Immediately when main is updated" because the result depends on the
  target branch.

## Rehearsal record (tasks 2.1 to 2.4)

Run on 2026-11-11 in `teamcollabeu/qgate-rehearsal`, pipeline 36, with
qgate 0.1.0, on `ubuntu-latest` and `windows-latest`. The spike's pull
requests 1339 and 1340 were abandoned first.

**Extraction.** A script took the first (Linux) and the second (Windows)
YAML block of the README section "In Azure Pipelines" and wrote each as
`azure-pipelines.yml`: the line `jobs:` and the block, unchanged.
`diff` of each block against its pipeline file:

```
0a1
> jobs:
```

**Scenario 5** changes one value: the pinned version becomes `0.0.0`,
which has no release (`v=0.0.0` on Linux, `$v = "0.0.0"` on Windows).

**Scenario 4's added line** is the only other change to the README's
text:

- **Linux:** this line goes after the `curl` line:
  ```
          printf 'x' >> "qgate-$v-$t.tar.gz"
  ```
- **Windows:** this line goes after the `SHA256SUMS` download:
  ```
          Add-Content -Path "qgate-$v-$t.zip" -Value "x" -NoNewline
  ```

**Results.** Every scenario gave the expected result:

| # | Scenario | Linux | Windows |
|---|---|---|---|
| 1 | Complexity 2 → 8 | PR 1341, build 1293. `qgate check` failed. Log: `error   src/grade.ts:1  grade: cyclomatic complexity 2 -> 8 (+6)  [complexity/cyclomatic-increase]`. Policy **rejected**. | PR 1344, build 1299. Same finding, the step failed with "PowerShell exited with code '1'". Policy **rejected**. |
| 2 | README only | PR 1342, build 1294. `No regressions found.`, job succeeded. Policy **approved**. | PR 1345, build 1301. Same. Policy **approved**. |
| 3 | Manual run on `r-<os>/clean` | Build 1302: job `qgate` **skipped**, build succeeded. | Build 1304: same. |
| 4 | Corrupted archive | PR 1343, build 1297. `qgate-0.1.0-x86_64-unknown-linux-musl.tar.gz: FAILED`, install step failed, check step skipped (`SucceededNode()` false). Policy rejected. | PR 1346, build 1303. `checksum mismatch` thrown, install step failed, check step skipped. Policy rejected. |
| 5 | Failed download (added in review) | PR 1347, build 1306. `curl: (22) The requested URL returned error: 404`, install step failed, check step skipped. Policy rejected. | PR 1348, build 1307. `Invoke-WebRequest : Not Found`, install step failed, check step skipped. Policy rejected. |

**Also observed:**
- **CI builds on every pushed branch.** The README job has no
  `trigger:`, so the rehearsal pipeline, which holds only that job,
  also ran a CI build for each pushed branch (builds 1291, 1292, 1295,
  1296, 1298, 1300; reason `individualCI`).
  - In each one, the `qgate` job was skipped and the build succeeded.
  - This is a second kind of non-PR build that passed scenario 3.
  - A partner's pipeline keeps its own `trigger:`, so the README adds
    none.
- **In the Windows log, qgate's summary line can come before the
  finding.** stderr and stdout reach the log separately. The text
  format's order on stdout is unchanged.

## Review of the pull request

The `reviewer` subagent found no blocker. It raised four should-fix
items and four nits:

- **Fixed: the PowerShell claim (should-fix).** The README said a
  PowerShell step goes on after a failing command, so that without the
  first lines a bad checksum would not stop the install. That is wrong
  for the Windows block, whose `throw` always stops it. The spike showed
  only that *native* commands slip through. The note now says this, and
  why the second line is there.
- **Fixed: the branch policy (should-fix).** In Azure Repos the policy
  is the only thing that makes a pull request build at all. Without it
  the job is skipped in every build, and the pipeline stays green. The
  README presented the policy as an extra that turns the job into a
  gate. It now says that the policy is required, and that *Required*
  makes it block. The intro links to it.
- **Fixed: the manual-run scenario (should-fix).** The spec said "by
  hand on `main`", but the rehearsal ran it on a rehearsal branch. The
  scenario now says "on a branch". The rehearsal's CI builds became a
  second scenario, "A CI build after a push".
- **Planned: the journal and the archive (should-fix).** They are not on
  the branch yet, and are the next step (D8).
- **Rehearsed: a failed download (nit).** The requirement named it, but
  no scenario covered it. A scenario was added and rehearsed as
  scenario 5 above.
- **Fixed: the log line (nit).** The spec quoted the finding line with
  one space after `error`, while the text format pads the severity
  column. The scenario now refers to the format instead of quoting it.
- **Fixed: the version count (nit).** The README holds the version in six
  install blocks, not four (D5).
- **For Ervin to decide: the organization (nit).** The planning
  artifacts name the Azure DevOps organization and the token's location
  and scopes, in a public repository.

After the fixes, both README jobs still extract byte for byte to the
rehearsed pipeline files. Only the prose changed.

