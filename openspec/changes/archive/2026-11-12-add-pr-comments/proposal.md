# Proposal: add-pr-comments

## Why

Since `add-azure-pipelines`, a qgate finding blocks the pull request in
Azure Pipelines. The developer then sees only a failed build that
"exited with code 1". The findings themselves are in the job log, which
takes three clicks to reach. A gate that does not say why it closed gets
ignored or worked around. The README's repository layout and
`qgate-report`'s module doc already promise Markdown output for pull
request comments.

## What Changes

- **`qgate check --format markdown`:** the report as one Markdown
  document, meant as the body of a pull request comment:
  - a summary sentence;
  - a table of findings, at most 50 rows;
  - a hidden first line with the counts, for scripts.

  It works in every `check` form, with the same exit codes as the other
  formats. It is not tied to Azure DevOps, so a GitHub comment can use
  the same output later.
- **`qgate check --format azure`:** one Azure Pipelines logging command
  (`##vso[task.logissue …]`) per Error or Warning finding. The agent
  shows each one in the build summary with its file and line. The exit
  code still decides whether the build fails.
- **`qgate-pr-comment.ps1`,** a PowerShell 7 script. It posts the
  Markdown report to an Azure Repos pull request as **one** comment
  thread. A later build updates that thread instead of adding another:
  - it creates the thread when there are findings;
  - it updates the thread and reopens it on the next build;
  - it closes the thread with "No regressions found." once the findings
    are gone;
  - it writes nothing on a clean pull request that never had a
    comment.

  When the build identity lacks the permission, the script fails with a
  message that names the missing permission. The script ships in
  **every release archive**, so `SHA256SUMS` covers it and it comes with
  the matching qgate version. It is tested in CI.
- **Version 0.2.0.** The README job can only use the new formats once
  they are released.
- **README, "In Azure Pipelines":** the job gains:
  - the build summary annotations;
  - the comment;
  - the one permission the comment needs.

  It is rehearsed again with 0.2.0 on both agents. The other install
  blocks move to 0.2.0.

### Decisions taken during exploration

- **One summary thread, not a comment per finding** (Ervin, on the
  agent's recommendation).
  - It is updated in place, so it stays one comment however many builds
    run.
  - The same Markdown fits GitHub.
  - Comments on the diff lines can follow if partners ask for them. The
    build summary already links findings to files and lines.
- **The posting logic is a PowerShell script in the release archive**
  (Ervin, on the agent's recommendation). The alternatives were:
  - a README snippet in bash and in PowerShell, which cannot be tested
    and needs `jq`;
  - posting from qgate itself, which needs HTTP and TLS in the binary,
    on six targets, for one REST call, and goes against design
    principle 1's split between the core and optional layers.

  PowerShell 7 is preinstalled on the Microsoft-hosted Linux agents as
  well, so one script serves both operating systems.
- **qgate renders, the pipeline posts.** qgate stays deterministic and
  offline; the token lives only in the pipeline step that needs it.
- **Spike, 2026-11-11,** in the rehearsal project `qgate-rehearsal`,
  builds 1309 and 1310:
  - **A logging-command issue does not change a step's result.** A
    `warning` issue left the step succeeded, and so did an `error`
    issue with exit code 0. The gate stays with qgate's exit code.
  - **Issues show file and line,** as in `src/grade.ts(1,): error
    complexity/cyclomatic-increase: …`, with the rule as `code`.
  - **Escaping.** A raw `;` in `sourcepath` cut the path at the `;`.
    `%3B`, `%5D`, `%AZP25` and `%0D%0A` decoded as expected. A raw `]`
    or `;` in the message did no harm.
  - **PowerShell 7.6.9 runs on `ubuntu-latest` too.**
  - **Without the permission,** creating a thread failed with HTTP 403
    and `TF401027: You need the Git 'PullRequestContribute' permission
    … identity 'Build\<project id>'`. Listing threads worked. The
    identity is the project's "Build Service" user.
  - **With "Contribute to pull requests" allowed for that user,** the
    pipeline's `System.AccessToken` did all of the following on both
    agents:
    - created a thread with a custom property;
    - found it again by that property among the system's own threads;
    - updated its comment;
    - closed it.
  - **PowerShell trap:** `"$url?api-version"` reads a variable named
    `url?api`. The script needs `"${url}?…"`, and tests.

### Out of scope

- Comments on diff lines.
- GitHub: a `--format github` for annotations, posting with `gh`, the
  GitHub Action. These are the next M4 change, and the Markdown format
  is built to be reused there.
- SARIF.
- Azure DevOps Server, and Azure Pipelines for GitHub-hosted
  repositories.
- Showing qgate's results as a pull request status ("checks" entry)
  separate from the build.

### Resolved during review of the proposal

- **PowerShell on the development machine** (Ervin, 2026-11-11: yes).
  This machine had no `pwsh`. PowerShell 7 is installed locally from
  its official release archive, after checking its checksum, into the
  user's home and without `sudo` (design D5, task 1.4). The script's
  tests then run locally as well as in CI.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `report-formats`:
  - `--format` gains `markdown` and `azure`;
  - the exit-code requirement covers every format;
  - new requirements describe the Markdown report and the Azure
    Pipelines annotations.
- `check-command`: the usage error for an unknown `--format` lists the
  four values.
- `releases`: every archive also holds `qgate-pr-comment.ps1`.
- `ci-integration`:
  - the documented job also annotates the build summary and keeps one
    pull request comment up to date;
  - the findings appear as build issues and in the comment, no longer
    as text-format lines in the job log;
  - the job needs one permission for the comment, where it needed none
    before.

## Impact

- **Rust code:**
  - `crates/qgate-report` gets two renderers;
  - `crates/qgate-cli` gets two `Format` values.

  No new crate dependencies.
- **New files:**
  - `ci/azure/qgate-pr-comment.ps1` and its Pester tests;
  - a CI job that runs those tests on Linux and Windows. It becomes a
    required check, with Ervin's approval.
- **Release:**
  - `release.yml` packs the script;
  - version `0.2.0`;
  - Ervin pushes `v0.2.0` and publishes the draft.
- **Docs:**
  - README "In Azure Pipelines", "JSON output" or a new "Output
    formats" note, and the version in the install blocks;
  - `CLAUDE.md` and README "Repository layout" name the `ci/`
    directory.
- **Two pull requests:**
  1. the formats, the script, the release packaging and the version;
  2. after `v0.2.0` is published: the README, the rehearsal, the
     journal entry and the archive.
