# Proposal: fix-pr-comment-details

## Why

`add-pr-comments` left two flaws for a follow-up change. Both ship in
0.2.0 and 0.3.0, and both reach partners through every PR comment:

- **A duplicate names its first copy twice.** The Markdown row reads
  `duplicates src/grade.ts:1-19 (lines 1-19, 85 tokens); also at
  src/grade.ts:1`: the message names the copy, and the related
  locations repeat it. GitHub and Azure DevOps comments both show this.
- **The Azure script's hint can name the wrong identity.** Without the
  permission, the comment step's error always tells the user to give
  the *project's* Build Service the permission. When the pipeline's job
  authorization scope is the whole collection, the token belongs to the
  *Project Collection* Build Service, and following the hint changes
  nothing. The README even warns about it ("its hint names the
  project's Build Service in either case").

## What Changes

- **Markdown report:** a related location whose `<path>:<line>` the
  message already names is left out of the Finding cell's "also at"
  list. The rule is about text, not about one analyzer, so it holds for
  any future finding that names a location in its message.
- **`ci/azure/qgate-pr-comment.ps1`:** the hint names the identity the
  token belongs to. Azure DevOps names it in the error as
  `Build\<id>`:
  - the id of the project (`SYSTEM_TEAMPROJECTID`): the project's
    Build Service, as now;
  - the id of the collection (`SYSTEM_COLLECTIONID`): the Project
    Collection Build Service;
  - anything else, or no id: both, as the README does.
- **README "In Azure Pipelines":** the sentence that the hint names the
  project's Build Service in either case goes.
- **Version 0.3.1,** so the binary, the script in the archives and the
  GitHub Action carry the fixes; the action installs 0.3.1.

### Out of scope

- The text format, which shows no related locations, and the JSON
  report, which keeps every related location: programs read them as
  data.
- The duplication finding's message.
- SARIF (next in M4).

### For Gate 1

- **Rehearse the collection case in Azure DevOps?** It needs the
  `qgate-rehearsal` project's "Limit job authorization scope to current
  project for non-release pipelines" turned off and the collection
  identity's permission removed, both by Ervin in the UI, and a PAT
  valid after 2026-11-17. Without it, the collection case rests on
  Pester tests built from the error text of task 1.8 of
  `add-pr-comments` (the project case) and on the documented form of the
  identity. The recommendation is to skip it: the fallback names both
  identities when the id matches neither.
  **Decided at Gate 1 (2026-11-13):** Ervin approved the proposal with
  the recommendation; the collection case is not rehearsed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `report-formats`: "Markdown table cells" leaves out a related
  location the message already names.
- `ci-integration`: "Missing comment permission" names the identity the
  token belongs to, the project's or the collection's Build Service.

## Impact

- **Code:** `crates/qgate-report/src/markdown.rs`;
  `ci/azure/qgate-pr-comment.ps1` and its Pester tests.
- **Release:** version `0.3.1` in `Cargo.toml` and in `action.yml`;
  Ervin publishes the draft after the tag.
- **Docs:** README "In Azure Pipelines", and every install block and
  `uses:` at 0.3.1.
- **Two small pull requests:** the fixes and the version; then, after
  0.3.1 is published, the README versions, the release record, a
  rehearsal of a duplicate in the GitHub rehearsal repository, the
  journal entry and the archive.
