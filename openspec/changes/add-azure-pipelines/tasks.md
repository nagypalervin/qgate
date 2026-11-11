# Tasks

This change is one pull request, `change/add-azure-pipelines` (design D8).
It has no code.
- **The pre-commit checks still run on every commit:** `cargo fmt`,
  `cargo clippy -D warnings` and `cargo test --workspace`.
- **Commit messages and PR bodies** are written to files first.
- **Azure DevOps** is reached only through the token file
  `~/.config/qgate-rehearsal/pat`, and only in the project
  `teamcollabeu/qgate-rehearsal`. The token is never printed.

## 1. README

- [ ] 1.1 Rename the README section "In CI" to "In GitHub Actions" and change the hook section's link from `#in-ci` to `#in-github-actions` (design D6). Verify with `grep` that no `#in-ci` link is left.
- [ ] 1.2 Add the section "In Azure Pipelines" after it (design D2 to D6):
  - the paragraph on what the job does;
  - the Linux job (bash) and the Windows job (`pwsh`), each with:
    - `fetchDepth: 0`;
    - the install step in `$(Agent.TempDirectory)` with the D4 preamble;
    - the one-command check step;
  - the branch policy steps;
  - the notes.

  Verify that the commands in the install steps match the "Try it" blocks, apart from the D4 preamble and the `##vso[task.prependpath]` line.

## 2. Rehearsal (design D7)

- [ ] 2.1 Prepare the rehearsal project:
  - abandon the spike's pull requests 1339 and 1340;
  - write a script in the scratchpad that extracts the two jobs from `README.md` into a pipeline file per OS. Verify that `diff` against the README's YAML blocks shows no difference other than the `jobs:` line.
- [ ] 2.2 Linux round. Push the extracted Linux pipeline with rehearsal branches and run scenarios 1 to 4 of D7. Verify each expected result from the build logs and the policy evaluations through the REST API.
- [ ] 2.3 Windows round: the same four scenarios with the extracted Windows pipeline. Verify in the same way.
- [ ] 2.4 Record the rehearsal in `design.md`:
  - the build ids and each scenario's result;
  - the policy statuses;
  - the line that scenario 4 adds;
  - the extraction `diff`.

  If any scenario fails, stop and fix the README first. Then run the round again.

## Workflow follow-up

- Open the pull request after task 2.4, then run the `reviewer` subagent on it and fix its findings.
- Write the `docs/journal/` entry and archive the change (`/opsx:archive`) on the same branch.
- Gate 3: Ervin merges.
- Update the M4 memory after the merge. Ask Ervin whether to keep the rehearsal project for `add-pr-comments`, which is the plan, and remind him that the token expires on 2026-11-17.
