# Proposal: add-duplicate-detection

## Why

"Near-duplicate blocks multiply" is one of the four kinds of drift qgate
promises to make visible (README, "Why"). Under AI-assisted development
the typical case is not a copy inside one pull request. It is new code
that re-creates something the repository already has, often with renamed
variables, instead of calling it. This is the second half of M2.

## What Changes

- **New rule `duplication/duplicate-block`, always Warning**, in revision
  mode (`qgate check --base`). It reports a block of code that a change
  newly duplicates, at the new occurrence, and names an existing copy:

  ```
  warning src/orders/format.ts:12  duplicates src/utils/date.ts:3-9 (lines 12-18, 54 tokens)  [duplication/duplicate-block]
  ```

- **Compared against the whole head revision.** The new code of the
  changed files is compared with every supported file of the head
  revision, not only with other changed files. qgate now reads the head
  revision's whole tree, through a single long-lived `git cat-file
  --batch` process.
- **Only new duplication:** a block is reported when it occurs at least
  twice in the head revision and the change increased its number of
  occurrences. Unchanged files are identical in both revisions, so the
  increase can be computed from the changed files alone; the base
  revision's other files are never read.
- **Renamed copies count (type-2 clones):**
  - local identifiers and literals are replaced by placeholders;
  - keywords, operators, punctuation and member names (`.getFullYear`)
    are kept;
  - imports, `export ... from` and type-level code are left out.
- **Size threshold:** a block must span at least 50 normalized tokens.
- **Not in the two-file form**, which has no repository to compare with.

### Decisions taken during exploration

- **Whole head revision, not only the changed files**:
  - Re-creating an existing helper is the case that matters most, and
    comparing only the changed files would miss it.
  - The delta formula keeps the cost to one read of the head tree.
- **Type-2 normalization**, so renamed copies are caught. Three things
  limit false matches: kept member names, the exclusions, and the 50-token
  threshold.
- **Token windows merged into blocks**, as PMD CPD and jscpd do, rather
  than comparing whole functions, so partial copies are found.
- **Always Warning, fixed constants.** Duplication is a weaker signal
  than a forbidden import: by default it does not fail a run; with
  `--fail-on warning` it does. Thresholds in `qgate.toml` come later.
- **One finding per new copy.** When every occurrence is new (a copy made
  inside the change), the first by path and position counts as the
  original and is not reported.

### Out of scope

- **Configuration:** thresholds or exclusion patterns in `qgate.toml`, and
  special treatment of test files or generated code.
- **Other clone types:** type-3 clones (copies with inserted or deleted
  statements) and semantic clones.
- **Refactoring advice:** suggesting the shared function a duplicate
  should call.
- **The two-file form:** duplication is not checked there.
- **The base revision's tree:** reading it is not needed (see above).
- **Other languages:** duplication across languages, or in languages
  other than TypeScript and TSX.

## Capabilities

### New Capabilities

- `duplication-analysis`: what counts as a duplicate, which files are
  compared, what makes a duplicate new, the size threshold and the
  `duplication/duplicate-block` finding.

### Modified Capabilities

None. Revision mode gains a finding but no changed requirement; the
two-file form is unchanged.

## Impact

- **`qgate-lang`:** `FileModel` gains a normalized token stream with line
  numbers.
- **`qgate-git`:**
  - listing the regular files of a revision (`git ls-tree -r`);
  - reading many blobs through one `git cat-file --batch` process, as ADR
    0002 anticipated under "Consequences".
- **`qgate-analyzers`:** a `duplication` module. It is a repository-level
  analysis, because it sees every file of the head revision. The
  per-file `Analyzer` trait stays as it is.
- **`qgate-cli`:** in revision mode, reads and parses the head tree's
  supported files and runs the duplication analysis.
- **Dependencies:** none expected.
- **Performance:** revision mode now parses every supported file of the
  head revision, not only the changed ones. This is measured in a task;
  see design, Risks.
- **Docs:** README (rules table and a duplication section).
