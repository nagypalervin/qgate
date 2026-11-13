# Tasks

Two pull requests with a patch release between them (design D3). Every
commit passes the pre-commit checks; commit messages and PR bodies are
written to files first.

## 1. Fixes and version (PR 1, `change/fix-pr-comment-details`)

- [ ] 1.1 Write failing unit tests in `crates/qgate-report/src/markdown.rs` (design D1):
  - a duplicate whose only related location the message names: no "also at";
  - a duplicate with "and 2 more": the named copy left out, the other two listed;
  - `src/a.ts:1` kept when the message names `src/a.ts:10-19`, and `src/a.ts:10` left out;
  - a location named at the very end of the message left out;
  - a related location without a line always listed.

  Then change `finding_text` until they pass. Check that the existing CLI test of the Markdown report still holds.
- [ ] 1.2 Write failing Pester tests in `ci/azure/qgate-pr-comment.Tests.ps1` (design D2): the project id, the collection id (any case), an unknown id, no identity, and no variables. Then change `Get-AzureErrorMessage` and pass the ids from `Get-PipelineContext`.
- [ ] 1.3 README "In Azure Pipelines": drop "and its hint names the project's Build Service in either case"; say that the hint names the identity the error names.
- [ ] 1.4 Version 0.3.1 in `Cargo.toml` (with `Cargo.lock`) and in every `QGATE_VERSION` of `action.yml`; run both release checks locally for `v0.3.1`.

## 2. Release 0.3.1 (between the pull requests)

- [ ] 2.1 After PR 1 merges, push `v0.3.1` (if Ervin asks the agent to) and check the draft: six archives, `SHA256SUMS`, `sha256sum --check`, the script's new hint in the archive, the Linux arm64 binary printing `qgate 0.3.1`. Ervin publishes. Record it in `design.md` on `change/fix-pr-comment-details-2`.

## 3. README and rehearsal (PR 2, `change/fix-pr-comment-details-2`)

- [ ] 3.1 Every install block, `uses:` and the pinned commit in the README at 0.3.1; check with `grep` that no `0.3.0` is left.
- [ ] 3.2 In the GitHub rehearsal repository, run the README workflow (`@v0.3.1`) on a pull request that adds a duplicate block; check through the API that the comment's row has no "also at" for the named copy. Record it in `design.md`.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- On PR 2: the `docs/journal/` entry, then `/opsx:archive`.
- Gate 3: Ervin merges each pull request.
