# Proposal: add-github-action

## Why

Azure Pipelines users get findings as build issues and as one pull
request comment since `add-pr-comments`. On GitHub, the README still
shows a hand-written job: it installs qgate, runs the text format and
fails the job. The findings stay in the log, and every team has to copy
and keep up a dozen lines of YAML. The roadmap's M4 promises a GitHub
Action, and `add-pr-comments` built the Markdown report so that GitHub
could reuse it.

## What Changes

- **`qgate check --format github`:** one GitHub Actions workflow command
  (`::error` or `::warning`) per Error or Warning finding, with file,
  line and the rule as title. GitHub shows each one as an annotation on
  the pull request's changed files. The exit code still decides whether
  the job fails.
- **A composite action in this repository** (`action.yml` at the root),
  used as `uses: nagypalervin/qgate@v0.3.0`. On a `pull_request` event
  it:
  1. installs the qgate release that matches the action's tag, for the
     runner's operating system and architecture, and checks it against
     `SHA256SUMS`;
  2. writes the Markdown report to the job summary;
  3. keeps **one** qgate comment on the pull request up to date:
     - it creates the comment when there are findings;
     - it updates the comment on later runs;
     - it rewrites it to "No regressions found." once the findings are
       gone;
     - it writes nothing on a clean pull request that never had one;
  4. runs the gate with `--format github`, so the annotations appear and
     the exit code decides the job's result.

  Inputs: `base` (default: the pull request's base branch), `fail-on`
  (default `error`) and `comment` (default `true`). The token is the
  job's `GITHUB_TOKEN`; the workflow grants it the permission to
  comment.
- **Pull requests whose token cannot write:** from a fork,
  `GITHUB_TOKEN` is read-only. The action then skips the comment with a
  notice and still writes the summary, the annotations and the gate.
  (Changed after Gate 1, in the review of PR 1: Dependabot pull requests
  are no longer skipped; their workflows get the permissions the
  workflow grants.) A same-repository pull request whose token lacks the
  permission fails the comment step with a message that names it.
- **The logic is PowerShell 7,** in `ci/github/`, tested with Pester in
  the existing `pester` CI jobs. `pwsh` runs on every GitHub-hosted
  runner, so one script serves Linux, macOS and Windows, as the Azure
  script does.
- **Version 0.3.0.** The action's tag and the qgate version are the same
  number. The release workflow refuses a tag whose version differs from
  the one `action.yml` installs.
- **README, "In GitHub Actions":** the action replaces the hand-written
  job, with the permissions it needs and what the pull request shows.
  It is rehearsed in a private GitHub repository on Linux and Windows
  runners.

### Decisions taken during exploration (the agent's recommendations)

- **The action lives in this repository,** not in a separate one. Action
  and binaries share one tag and one review; no second repository to
  release and keep in step.
- **A composite action in PowerShell,** not a JavaScript action. A
  JavaScript action needs Node, a bundler and a committed `dist/`; the
  composite action reuses the approach, the test setup and the CI jobs
  of the Azure script.
- **One summary comment, as on Azure.** GitHub comments have no closed
  state, so a fixed pull request gets the comment rewritten instead of
  closed.
- **qgate renders, the action posts.** qgate stays offline; only the
  action's comment step sees the token.

### To check in a spike, before the code

The docs leave these open, and the design depends on them:
- which `permissions:` let `GITHUB_TOKEN` create and edit a pull request
  comment (`pull-requests: write`, `issues: write`, or either);
- how many annotations a step and a job show, and how `%`, `:`, `,` and
  line breaks are escaped in workflow commands;
- the `pwsh` version on `ubuntu-latest`, `windows-latest` and
  `macos-latest`, and whether `>` keeps qgate's UTF-8 output unchanged.

### Out of scope

- Publishing the action on the GitHub Marketplace. Its listing rules may
  require a repository without workflow files; a later change can split
  the action out if that is wanted.
- A moving major tag (`v0`, `v1`); users pin `@v0.3.0` or a commit.
- Comments on diff lines and pull request reviews.
- SARIF and code scanning.
- `pull_request_target`, which would give fork pull requests a write
  token but runs untrusted code with it.
- Action outputs (counts) and a `working-directory` input.
- The two follow-ups recorded by `add-pr-comments` (the Azure script's
  hint for the Project Collection Build Service; a duplicate's first
  copy named twice in the Markdown row).

### For Gate 1

- **A private rehearsal repository on GitHub,** for example
  `nagypalervin/qgate-rehearsal`, created by Ervin (or by the agent with
  `gh`, if Ervin says so). The spike and the README rehearsal run there,
  so the public repository gets no test pull requests.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `report-formats`: `--format` gains `github`; new requirements
  describe the GitHub Actions annotations and their escaping.
- `check-command`: the usage error for an unknown `--format` lists the
  five values.
- `ci-integration`: new requirements for the GitHub Action: install,
  checking a pull request, the job summary, the one comment, read-only
  tokens, the missing permission, and events other than `pull_request`.
- `releases`: a tag whose version differs from the one `action.yml`
  installs fails the release workflow.

## Impact

- **Rust code:** `crates/qgate-report` gets a renderer and
  `crates/qgate-cli` a `Format` value. No new crate dependencies.
- **New files:** `action.yml`; `ci/github/` with the PowerShell script
  and its Pester tests. The `pester` CI jobs run `ci/` instead of
  `ci/azure/`; their check names stay the same.
- **Release:** the version check covers `action.yml`; version `0.3.0`;
  Ervin pushes `v0.3.0` and publishes the draft.
- **Docs:** README "In GitHub Actions", the format paragraph and the
  install blocks' version; README "Repository layout" and `CLAUDE.md`
  name `action.yml`.
- **Two pull requests:**
  1. the spike record, the format, the action, its tests, the release
     check and the version;
  2. after `v0.3.0` is published: the README, the rehearsal with
     `@v0.3.0`, the journal entry and the archive.
