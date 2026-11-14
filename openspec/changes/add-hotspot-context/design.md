# Design: add-hotspot-context

**Approved at Gate 2 (2026-11-14):** with the proposal, as written.

## Context

See proposal.md for why and for what the explore session decided; the
specs `hotspot-analysis` and `report-formats` say what. What the design
works with:

- **`qgate-git`** reads two revisions only: `changes`, `files_at`,
  blobs (ADR 0002, subprocess). History is new to it.
- **The revision check** (`check_revisions` in `qgate-cli`) already
  reads the head file list once (0.7.0, D8 of `add-test-gap-analysis`).
  Duplication parses every supported, non-excluded, non-generated head
  file of each family that has an analyzed change, so the head models
  are at hand.
- **`Finding`** in `qgate-core` has `rule`, `severity`, `message`,
  `location`, `metric` and `related`. Findings are built as struct
  literals in about 30 places across six crates.
- **The JSON schema** (`docs/schema/report-v1.json`) lets version 1 add
  keys; the `message` is documented as "do not parse".
- **git 2.43 here; CI runners are newer.**
  - `git rev-parse --is-shallow-repository` answers `true` on a
    `--depth` clone.
  - `git log --name-status --find-renames` lists a rename as
    `R<score>\t<old>\t<new>`.

## Goals / Non-Goals

**Goals:**

- One extra `git log` process per check, no extra blob reads: the
  complexity comes from the models already parsed.
- The ranking is a pure function of numbers per path, testable without
  git.

**Non-Goals:**

- History of functions, authors, coupling.
- Caching history between runs; the MCP server pays the `git log` per
  call, and the evaluation measures what that costs.

## Decisions

### D1. History in `qgate-git`: commit count per head path

A new `Repository` method returns, for a head revision and a start
time, the number of non-merge commits that touched each path, keyed by
the path in the head revision.

- **Command:** `git log --no-merges --find-renames --name-status
  --format=commit%x00%ct <head> --since=@<start>`. Each commit's
  committer time is checked again against `start` in Rust, because
  `--since` stops early only on well-ordered dates. That makes the
  window exact even with clock skew.
- **Renames:** the log walks from new to old. A map from "path at this
  point in history" to "head path" starts as the identity. An
  `R<n> old new` line adds `old -> head(new)`. `A`, `M` and `R` lines
  count one for the mapped head path. A path that is gone at the head
  maps to nothing and is dropped.
- **Which paths:** all of them. The CLI filters to ranked files, so the
  git crate stays ignorant of languages (ADR 0002).
- **Shallow:** a second method, `is_shallow()`, runs `git rev-parse
  --is-shallow-repository`.
- **Errors:** as other git calls: exit codes, not localized messages.
- **Alternatives:**
  - `git log --follow` per file: one process per file, too slow;
  - a git library: rejected by ADR 0002.

### D2. The window (Gate 1, question 1)

- **Start:** 182 days before the committer date of the head commit
  (`git log -1 --format=%ct`). With `--worktree`, the head is the
  snapshot commit, which is dated now. So the window ends at `HEAD`'s
  committer date, and the snapshot itself never counts. Uncommitted work
  is one edit, not history.
- **The note says "6 months":** 182 days is 26 weeks, and the note is
  for people. The spec fixes the number of days.

### D3. Ranking in `qgate_analyzers::hotspots`

A pure function takes, per language family, the ranked files with their
churn and complexity, and returns the hotspot set.

- **Population** (Gate 1, question 3): the head files duplication
  reads, that is supported, not excluded and not generated, per family.
  It is computed only for families with an analyzed change, and only
  those can have findings.
- **Thresholds** (fixed in 0.8.0, Gate 1, question 6):
  - churn at least 5, with fewer than 10% of the family's files above
    it;
  - complexity at least 1, with fewer than 25% of the family's files
    above it.

  "Fewer than k% above it" includes ties at the top and needs no
  rounding rule: with 20 files, a file is in the top 10% when at most
  one file has a higher churn.
- **Complexity:** the sum of `Function::cyclomatic` over the file's head
  model. Changed files use their analyzed head model, unchanged ones
  the model duplication parsed.
- **Annotation:** `annotate(findings, hotspots)` sets
  `Finding::hotspot` and appends `; hotspot: <n> commits in 6 months,
  complexity <c>` to the message of every finding at a hotspot path,
  except `input/unparsable`. It is a separate step at the end of the
  check, so no analyzer knows about history.

### D4. `Finding::hotspot` in `qgate-core` (Gate 1, question 7)

`pub hotspot: Option<Hotspot>`, with `Hotspot { commits: u32,
complexity: u32 }`. Every struct literal gets `hotspot: None`, a
mechanical edit in PR 1 that changes no output. The JSON renderer
writes the key when set; the other renderers need nothing, because
the note is in the message.

- **Alternative:** the note only in the message, with no field. That
  would leave out the core change, but JSON consumers (the pilot
  evaluation, an MCP agent) would have to parse a message the schema
  tells them not to parse.
- **Alternative:** `Finding` with `#[non_exhaustive]` and a
  constructor. That is a larger refactor than this change needs.

### D5. CLI wiring

- **Order:**
  - after the analyzers and duplication, and before the sort;
  - only when the check has an analyzed change, as for duplication and
    tests;
  - shallow check first. If shallow: no history read, and a note
    returned to `qgate check`, which prints it once on standard error.
    The hook and the MCP server drop it.
- **Models:** `duplication()` today parses unchanged head files and
  drops their models after indexing. It will also record each file's
  complexity sum, a `HashMap<String, u32>` per family, so the models
  are parsed once.
- **Two-file form:** untouched.

### D6. Evaluation and rehearsal (Gate 1, questions 8 and 9)

- **Evaluation:**
  - **Repositories:** the four of 0.7.0, re-cloned with full history
    as partial clones (`--filter=blob:none`). These are not shallow,
    unlike a `--depth` or `--shallow-since` clone, which qgate would
    rightly refuse to rank.
  - **Run:** the last 30 code commits again, with the 0.7.0 scripts
    extended.
  - **Record:**
    - hotspots per family and repository;
    - every annotated finding, sorted by hand: does it sit in code a
      reviewer would call fragile?
    - how many Warnings option 2 (severity raising) would have made
      Errors;
    - the time per commit against 0.7.0, and the `git log` share of it.
  - **Thresholds:** they may change before the release; Ervin decides.
- **Azure rehearsal:**
  - **Setup:** a branch with six commits that each touch one TypeScript
    file holding the complex units, and a pull request that raises one
    unit's complexity. Hotspot status comes from history reachable from
    the pull request's merge commit, so the rehearsal `main` stays
    untouched.
  - **Expected:** the Warning row and the build issue end with the
    hotspot note.

### D7. Three pull requests (Gate 1, question 10)

- **PR 1** (`change/add-hotspot-context`): D1 in `qgate-git` with tests
  on dated temporary repositories, and D4's field with `hotspot: None`
  everywhere. No output changes.
- **PR 2** (`change/add-hotspot-context-2`): D2, D3, D5, the JSON key and
  schema, the README, 0.8.0 and the evaluation.
- **Release 0.8.0.**
- **PR 3** (`change/add-hotspot-context-3`): the README versions, the
  roadmap, the rehearsal, the journal and the archive.

## Risks / Trade-offs

- **[`git log --find-renames` is slow on large windows]** It is O(changed
  files) per commit, with rename detection on top. Mitigation: the
  evaluation times it on `bitwarden/clients` (thousands of commits in 6
  months). If it is too slow, drop `--find-renames` and accept that a
  renamed file starts over, decided with Ervin before the release.
- **[Relative thresholds in tiny repositories]** With 3 files the most
  changed one is always in the top 10%; the minimum of 5 commits
  guards against noise. Accepted.
- **[Bulk commits]** A formatting commit that touches every file adds
  one to every file. It shifts no ranking, since all move together.
  Accepted.
- **[Branches that rewrite history]** Only what is reachable from the
  head counts, so a squash-merge workflow counts one commit per pull
  request. That is consistent and arguably right; the README says so.
- **[The MCP server runs `git log` on every call]** Acceptable if the
  evaluation's timing holds. Caching is a later change.
- **[A new optional JSON key]** Consumers that reject unknown keys
  break. The version 1 contract allows it (`report-formats`), and the
  release notes name it.
- **[The note makes messages longer]** Azure and GitHub annotations
  and the Markdown table get longer lines. Only findings in hotspots
  get the note, a minority by construction.

## Migration Plan

Nothing to migrate. Rolling back means pinning 0.7.0.
