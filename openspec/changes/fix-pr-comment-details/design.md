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
`<path>:<line>` at its start or after whitespace, followed by a
character that is not an ASCII digit, or at the end. Plain substring
search, no regular expression, so a path's characters need no
escaping. A location without a line is always listed (the message
cannot name it by the same form).

- **Why text, not the rule id:** the report crate stays free of
  analyzer knowledge, and any later finding that names a location in
  its message gets the same treatment.
- **Why not change the message:** the text format shows no related
  locations, so the message is the only place it names the copy.
- **Why the digit check:** `src/a.ts:1` must not match
  `src/a.ts:10-19`.
- **Why the check before it** (added during task 1.1; the proposal had
  only the digit check): `src/a.ts:1` must not match `lib/src/a.ts:1-6`
  either, since a duplicate's copies can sit in such paths. Whitespace
  or the start is the only boundary accepted, so a path quoted any
  other way, such as `(src/a.ts:1)`, is listed again, and the row
  repeats a location as before this change. One case still loses a
  location: see Risks.
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

Decided during task 1.2:
- **The names come from `Get-PipelineContext`,** with the ids. The
  ids are optional there: a missing one only leads to the fallback.
- **`<organization>`** is the first path segment of a
  `https://dev.azure.com/<organization>/` collection URI, or the host's
  first label of `https://<organization>.visualstudio.com/`. Any other
  URI (Azure DevOps Server) leaves the parenthesis out:
  `<project> Build Service`.
- **Without the variables** (the function called on its own) the names
  are the README's placeholders,
  `<project> Build Service (<organization>)`.

### D3. Release 0.3.1 and two pull requests

- **PR 1,** `change/fix-pr-comment-details`: D1, D2, tests, README
  sentence, version 0.3.1 in `Cargo.toml` and every `QGATE_VERSION` of
  `action.yml`. The release workflow's checks hold both.
- Ervin (or the agent, if asked) pushes `v0.3.1`; Ervin publishes.
- **PR 2,** `change/fix-pr-comment-details-2`: every install block and
  `uses:` in the README at 0.3.1 (the pinned commit too), the release
  record, one GitHub rehearsal run with a duplicate (the comment row
  without "also at"), the journal entry and the archive.

## Release 0.3.1 (task 2.1)

- PR #18 merged as f03ae2c. As Ervin agreed, the agent pushed the
  annotated tag `v0.3.1` ("qgate 0.3.1") after `check-version.sh` and
  `check-action-version.sh` accepted it on `main`.
- Release run 43400095097: all nine jobs green.
- The draft held six archives and `SHA256SUMS`; `sha256sum --check`
  passed for all six. The Linux arm64 archive holds the binary,
  `qgate-pr-comment.ps1`, `README.md` and both licenses; its script is
  byte for byte the one on `main` (it names the Project Collection
  Build Service), and both Windows zips hold the script. The Linux
  arm64 binary printed `qgate 0.3.1`.
- Ervin published the draft on 2026-11-13 (09:33 UTC); `releases/latest`
  is `v0.3.1` and its files download.

## Rehearsal of a duplicate (task 3.2, 2026-11-13)

- In `nagypalervin/qgate-rehearsal`, the branch `linux/duplicate-0.3.1`
  holds the README's workflow, cut out of this branch's README
  unchanged (`uses: nagypalervin/qgate@v0.3.1`), and adds three copies
  of a date helper: `src/utils/date.ts`, `src/orders/format.ts` and
  `src/invoices/format.ts`. Pull request #14, closed unmerged.
- Run 43403022675 succeeded: "installed qgate 0.3.1
  (x86_64-unknown-linux-musl)", two Warning annotations, and "created
  the qgate comment (2 findings, 0 at or above error)".
- The comment, read through the API, has two rows such as
  `| warning | src/orders/format.ts:4 | duplicates src/invoices/format.ts:2-7 (lines 4-9, 68 tokens) | duplication/duplicate-block |`:
  the copy the message names is not repeated, and no row has "also
  at". With all three files new, each finding has one related location,
  so the case of a further copy listed after "also at" did not come up
  here; the unit tests and the CLI test cover it.

## Risks / Trade-offs

- **[The collection identity's form is not observed]** It is taken from
  Azure DevOps' identity naming, not from a run. → The fallback names
  both identities; a wrong guess at worst shows both, as the README
  does today.
- **[A message that names a location by chance]** A related location
  is dropped if the message happens to contain its `path:line`. → Then
  the location is named in the message anyway; nothing is lost.
- **[A path with a space]** A message naming `x y/a.ts:1-6` also names
  `y/a.ts:1` by this rule, so a copy at `y/a.ts` line 1 is left out of
  the row although the message does not name it (found in the review of
  PR 1). → Accepted: it needs two files whose paths differ only by a
  prefix ending in a space, and the JSON report still lists every
  related location.
