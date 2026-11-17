# Design: duplication-of-new-code

## Context

See proposal.md for why. The duplication analyzer
(`crates/qgate-analyzers/src/duplication.rs`) works on normalized tokens
only:

- `ChangedSource` hands it the tokens of both versions of each changed
  file, and the base tokens of deleted files.
- `counts()` decides per 50-token window whether the change increased
  the number of files that hold it.
- `coverage()` marks each token of a changed file as covered by a
  reported window (`new`) or by duplicated code the file already had
  (`old`).
- `blocks()` reports a block when at least 50 of its tokens are `new`,
  not `old`, and not already used by an earlier block of the file.

A `Token` keeps only the normalized text, its line and its unit. The
source text is gone after `qgate_lang::parse`, so nothing today can tell
an added line from a kept or moved one.

The exploration measured the rule with a spike over the replay of the
150 evaluation pull requests (`git diff -U0` for added and removed
lines). The replay tools are on the branch `change/evaluate-angular-dotnet`.

## Goals / Non-Goals

**Goals:**

- Count only tokens on lines the change wrote, as the spec delta
  defines them.
- Keep `counts()`, the extent of a block and the message unchanged:
  which files are new holders, and which copy a finding names, stay as
  they are.
- No git call, dependency or configuration added. The analyzer stays
  testable with before/after fixtures.

**Non-Goals:**

- Using the diff in the complexity analyzer (the Myers follow-up of
  `limit-move-evidence`).
- Detecting a move that renamed names on the way.

## Decisions

### D1: The written-line condition is added to today's coverage

A token counts towards a block's 50 new tokens when it is `new`, not
`old`, not used by an earlier block, **and its line is written**.
`counts()` keeps deciding growth by holding files, so moved code still
keeps a window from growing where it does today. The new condition only
removes tokens from the count; it never adds one. So the change can only
silence findings, never create one.

*Alternative:* replace the window coverage by written lines. That would
change which files are new holders and which copy is named, and give
up conditions that the spec scenarios ("Existing duplication", "Code
written next to shared code") rest on. Rejected.

### D2: A line is its trimmed text, hashed

`qgate_lang::parse` sets a new field, `FileModel::lines: Vec<u64>`.
Element `i` holds the 64-bit FNV-1a hash of line `i + 1` without leading
and trailing white space, and 0 for an empty line, so the pairing of
D3 can tell empty lines without the text. Lines are split by
`str::lines`, so `\r\n` works too. This happens once for all three adapters, after the adapter
returns. The field covers every line, comment and empty lines included.

*Alternatives:*

- **Normalized tokens per line.** They lose SV20, SV22 and SV36
  (defensible): a written line with new names in a familiar shape would
  pass as moved.
- **The raw text in `Token`.** It costs a string per token and still
  misses lines without tokens.
- **Untrimmed text.** A block moved into a nested scope changes its
  indentation and would count as written.
- **Hashing in the CLI.** The analyzer's fixture tests parse sources
  through `qgate_lang::parse` and would not get the hashes.

### D3: Moved lines are paired in runs of three, one to one, own file first

In `DuplicationIndex::new`:

1. **Removed and added lines.** For each changed file, the diff of D5
   gives its removed base lines and added head lines. A deleted file
   adds all of its lines to the removed side.
2. **Runs.** A run is three added (or removed) lines with consecutive
   line numbers in one file, at least one of them not empty. There is
   one run per starting line, so the runs overlap. A run's key is its
   three hashes.
3. **Pool.** The removed runs form a pool that counts each key per file.
4. **First pass, own file.** In path order, then line order, each added
   run takes one occurrence of its key from its own file's removed runs.
5. **Second pass, other files.** Each added run that is still unpaired
   takes one occurrence from any file's remaining removed runs. The
   files are tried in path order.
6. **Result.** A line of a paired run is moved. An added line that is
   not moved is written: `written[file][line]`.

The first pass handles a block that moved within a file while another
file got a copy. In a global path order, the copy could take the
removed run first, and the moved block would count as written (spec
scenario "Moved within a file and copied").

*Alternatives, from the spike in the exploration:*

| Variant | Useful (6) kept | Defensible (13) kept | Noise (22) silenced |
|---|---|---|---|
| single lines | 5 (CL14 lost) | 12 | 21 |
| runs of 3, no consumption | 6 | 12 | 21 (SV8 kept) |
| **runs of 3, one to one** | **6** | **12** | **20** (SV8, SV27 kept) |

- **Single lines.** In the large refactoring of PR 24684, short lines
  such as `);` find a removed partner everywhere.
- **No consumption.** A helper that a split copies into two files would
  be silent. Ervin chose one to one.

### D4: The analyzer gets the line hashes through `ChangedSource`

`ChangedSource` gets `before_lines: &[u64]` and `after_lines: &[u64]`
next to its tokens. The CLI fills them from the `FileModel`s it already
holds; a deleted file has no `after_lines`. `DuplicationIndex::add`
(unchanged head files) does not need them.

### D5: A Myers line diff in a new `diff` module, with a limit

`crates/qgate-analyzers/src/diff.rs` gets two functions:

- **`middles`,** moved from `complexity.rs` and made generic over
  `T: PartialEq`. `complexity.rs` calls it as before, so complexity
  behavior does not change.
- **`kept`:** given two slices, it marks which elements of each are
  kept by a shortest edit script.
  - It cuts the common start and end with `middles`, then runs Myers'
    greedy O((N+M)·D) algorithm on the middles.
  - It stops at `MAX_EDITS` differences, 2^11 = 2048 inserted plus
    deleted lines. The trace of a run up to D costs about D² entries,
    so about 4 million `isize` (32 MB) at the limit, and the time is
    (N+M)·D.
  - Beyond the limit, every line of both middles counts as removed and
    added. The pairing of D3 then matches the file's unchanged runs with
    its own removed runs in the first pass, so the middle mostly counts
    as moved. Lines next to an edit become written, which can only
    report more, as today, never less than a real diff would.

*Alternatives:*

- **`git diff` through `qgate-git`.** It is what the spike used. But it
  adds a git call per check, and the analyzer could not be tested with
  fixtures alone.
- **No diff: every line of the base removed, every line of the head
  added, paired by D3.** It is simpler. But an unchanged line between
  two edits less than three lines apart would be written, and P2's
  edits are often that close.
- **LCS by dynamic programming.** It costs O(N·M) for every edit,
  however small. Myers' cost grows with the edits, and it is the named
  follow-up for complexity's move evidence.

### D6: Tests

Fixtures first, then the code. Mutation checks are done by hand.

- **`diff`:**
  - Myers against a brute-force LCS on many small generated inputs (a
    fixed seed, no new dependency);
  - the limit (one difference past it gives the fallback);
  - `middles` behaves as before (complexity's tests).
- **`qgate-lang`:** `lines` for CRLF, white space, an empty last line
  and a file without a trailing newline, in all three languages through
  `parse`.
- **`duplication`:** one fixture test per new spec scenario, and the
  existing tests unchanged.
- **Mutants:** each must fail a test:
  - no own-file pass;
  - no consumption;
  - runs of one line;
  - empty runs allowed;
  - untrimmed hashes;
  - no written-line condition.

### D7: Two pull requests

- **PR 1:** D2, D4's plumbing and D5. The analyzer does not use the lines
  yet; no finding changes, and the replay confirms it.
- **PR 2:** D1 and D3, the spec delta, the README, the replay and the
  archive.

## Risks / Trade-offs

- **Generic runs pair with written code** (three lines like `}`, `}`,
  `});` that a large refactoring removed elsewhere). The written tokens
  are undercounted and a real copy may be missed. → Runs of three kept
  CL14 in the spike. The replay checks it, and a minimum of tokens per
  run is the lever if it shows up.
- **Myers breaks ties differently from git.** Repeated lines (`}`) may
  count as added in other places than in the spike. → The replay. The
  bar is the judged findings of the proposal, and any difference is
  explained before merge.
- **Known noise stays:** SV8 and SV27, moved blocks with enough edited
  lines.
- **Known loss:** SV30 (defensible). Its block starts in the tail of an
  existing test.
- **Line hash collisions** make a written line moved. A run needs three
  64-bit hashes to collide in a row: negligible.
- **Cost.** The hashes and runs are linear in the lines of the changed
  files, and the diff is bounded by D5. The replay's slowest run (7.0 s
  after PR 43) must not grow noticeably.

## Migration Plan

None. The finding, its rule id and its message stay; only fewer of them
appear. No configuration changes.

## Verification

**PR 1** (line hashes, the diff module, the plumbing):

- **Checks.** `cargo fmt`, `clippy -D warnings` and `cargo test
  --workspace` pass.
- **Tests first.** The new tests of `qgate-lang` failed to compile, and
  those of `kept` failed on their result.
- **Diff mutants.** Without the cut of the common start and end, and
  without the limit, `kept` each fails
  `beyond_the_limit_only_the_common_start_and_end_are_kept`.
- **Replay of the 150 pull requests against `main` after PR 43.** Every
  report is identical (gone 0, new 0, changed 0). The slowest run, server
  PR 8812, took 9.1 s in the batch against 7.0 s before. Timed alone,
  three times each, both binaries take 7.2 to 7.4 s on it, so the batch
  difference was the machine's load.
