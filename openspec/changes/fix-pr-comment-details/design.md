# Design: fix-pr-comment-details

## Context

- `crates/qgate-report/src/markdown.rs`, `finding_text`: the message,
  then `; also at ` and every related location as `<path>:<line>`
  (`location_text`), or the path alone without a line.
- The duplication analyzer's message names the first copy as
  `<path>:<first>-<last>` and counts the rest ("and 2 more"); its
  `related` list starts with that copy (`duplication.rs`, the
  `duplication-analysis` spec).
- `ci/azure/qgate-pr-comment.ps1`, `Get-AzureErrorMessage`: on HTTP 403
  with `TF401027` it reads the identity from `identity '...'` and
  always advises the project's Build Service. Task 1.8 of
  `add-pr-comments` recorded the error for the project scope:
  `identity 'Build\<project id>'`.

## Goals / Non-Goals

**Goals:** no location twice in a Markdown row; a hint that names the
identity the error names.

**Non-Goals:** changing what the analyzers report, the text or JSON
formats, or the Azure script's other messages.

## Decisions

### D1. Leave out a related location the message names

A related location with a line is left out when the message contains
`<path>:<line>` followed by a character that is not an ASCII digit, or
at the end. Plain substring search, no regular expression, so a path's
characters need no escaping. A location without a line is always
listed (the message cannot name it by the same form).

- **Why text, not the rule id:** the report crate stays free of
  analyzer knowledge, and any later finding that names a location in
  its message gets the same treatment.
- **Why not change the message:** the text format shows no related
  locations, so the message is the only place it names the copy.
- **Why the digit check:** `src/a.ts:1` must not match
  `src/a.ts:10-19`.
- **Only Markdown.** JSON keeps every related location, as data.

### D2. The identity in the Azure hint

`Get-AzureErrorMessage` takes the identity from the error, as now. If
it is `Build\<id>`:
- `<id>` equals `SYSTEM_TEAMPROJECTID` (case-insensitive): advise
  "<project> Build Service (<organization>)", as now;
- `<id>` equals `SYSTEM_COLLECTIONID`: advise "Project Collection Build
  Service (<organization>)", and say that the pipeline's job
  authorization scope is the collection;
- otherwise, or with no identity or no variables: advise both, in that
  order.

The function gets the two ids as parameters, so the tests need no
environment; `Invoke-Azure` passes them from the pipeline variables.

### D3. Release 0.3.1 and two pull requests

- **PR 1,** `change/fix-pr-comment-details`: D1, D2, tests, README
  sentence, version 0.3.1 in `Cargo.toml` and every `QGATE_VERSION` of
  `action.yml`. The release workflow's checks hold both.
- Ervin (or the agent, if asked) pushes `v0.3.1`; Ervin publishes.
- **PR 2,** `change/fix-pr-comment-details-2`: every install block and
  `uses:` in the README at 0.3.1 (the pinned commit too), the release
  record, one GitHub rehearsal run with a duplicate (the comment row
  without "also at"), the journal entry and the archive.

## Risks / Trade-offs

- **[The collection identity's form is not observed]** It is taken from
  Azure DevOps' identity naming, not from a run. → The fallback names
  both identities; a wrong guess at worst shows both, as the README
  does today.
- **[A message that names a location by chance]** A related location
  is dropped if the message happens to contain its `path:line`. → Then
  the location is named in the message anyway; nothing is lost.
