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
  they are, except that a holder the block only moved to is none (D1).
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
removes tokens from the count; it never adds one. So the condition
alone can only silence findings, never create one.

*Alternative:* replace the window coverage by written lines. That would
change which files are new holders and which copy is named, and give
up conditions that the spec scenarios ("Existing duplication", "Code
written next to shared code") rest on. Rejected.

**Moved holders are no new copies** (decided by Ervin during the apply,
2026-11-17). The written-line condition alone left a gap: `a.ts` is
deleted, its block goes to `b.ts` and `d.ts`, and the unchanged `c.ts`
keeps its copy. `counts()` reported `b.ts`, the first new holder by
path, while the pairing of D3, also in path order, made `b.ts`'s lines
moved, so the written copy in `d.ts` went unreported. Now `counts()`
looks at the new holders of a window: one where no place of the window
has a token on a written line is moved. Moved holders are sorted first
and skipped. If there is one, it is the original, and no other holder
is skipped; otherwise the first by path is, as before. So `d.ts` is
reported, naming `b.ts`.

This is no longer "only silences": a finding can pass from a moved
holder to a written one in the same change. Per window, the number of
reported holders still never grows, since at least as many are skipped
as before.

*Alternative:* the spec as it was, with this case as a known loss. Then
an unchanged copy elsewhere would hide a written copy that the "helper
copied into two files" scenario reports without it. Rejected.

### D2: A line is its trimmed text, hashed

`qgate_lang::parse` sets a new field, `FileModel::lines: Vec<u64>`.
Element `i` holds the 64-bit FNV-1a hash of line `i + 1` without leading
and trailing white space (and a byte order mark, which editors add and
drop), and 0 for an empty line, so the pairing of
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
    as moved. Lines next to an edit become written. Per key, the first
    pass pairs as many runs as a real diff would keep, but in line order
    it may pair another occurrence of the same run, so which of the
    file's lines count as written can shift. The replay is the check
    for both.
  - Each run of changed elements then slides down as far as equal
    elements allow, as git's diff does in the common case (decided by
    Ervin during the apply, 2026-11-17). Git also slides up to join
    runs, and its indent heuristic may stop earlier; neither is done
    here. Without the slide, a test appended after a similar test that
    ends in the same lines counted as the old test's tail plus a new
    test's head, so the tail of the copy looked kept and the copy lost
    its written tokens (SV20 in the replay). A run that slides into the
    next one joins it.

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
  - the slide: a copy added after the code it copies is added last, in
    both directions, and a run that slides into the next joins it;
  - `middles` behaves as before (complexity's tests).
- **`qgate-lang`:** `lines` for CRLF, white space, an empty last line
  and a file without a trailing newline, in all three languages through
  `parse`.
- **`duplication`:** one fixture test per new spec scenario, and the
  existing tests unchanged, except
  `only_as_many_files_as_the_change_added_are_reported`, which now
  expects the finding for `d.ts` (D1, moved holders).
- **Mutants:** each must fail a test:
  - no own-file pass;
  - no consumption;
  - runs of one line;
  - empty runs allowed;
  - untrimmed hashes;
  - no written-line condition;
  - moved holders not skipped;
  - moved holders not sorted first.

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
  count as added in other places than in the spike. → The slide of D5
  follows git's convention, and the replay checks the rest. The bar is
  the judged findings of the proposal, and any difference is explained
  before merge.
- **Known noise stays:** SV23 and SV27, moved blocks with enough edited
  lines. The spike named SV8 instead of SV23; see Verification.
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
- **Review.** The `reviewer` subagent ran one round. It found no
  blocker and confirmed the trace arithmetic of Myers with its own
  mutants. Fixed after it:
  - the limit tests now pin it exactly: `MAX_EDITS + 1` and one more
    step each fail a test;
  - the line hashes are pinned to FNV-1a, to lines ending at `\n` only,
    and to every token's line; a byte order mark is trimmed too;
  - `ChangedSource` documents the line hashes, and the test helper fills
    them;
  - the design and the spec delta describe the fallback and the base
    version more precisely.

  The end-to-end test of the CLI's wiring (task 4.4) comes in PR 2,
  where the analyzer reads the lines. The trace still stores both
  parities of diagonals; halving it is left out.
- **qgate on itself.** It reported two Warnings: `myers` as a new
  function of complexity 18, and the test oracle of the diff as a copy
  of complexity's `common_in_order`. The search is now split in two,
  and `common_in_order` moved into `diff`, generic, for both. After
  that, no finding.

**PR 2** (the rule):

- **Tests first.** Of the seven new fixture tests, the three the rule
  changes failed on their result ("The same small edit in existing
  copies", "A copy completed by a few written lines", "A split file");
  the other four passed already.
- **A decision during the apply.** With the rule in place,
  `only_as_many_files_as_the_change_added_are_reported` reported nothing:
  the moved copy in `b.ts` hid the written one in `d.ts`. Ervin chose
  that a moved holder is no new copy (D1); the test now expects the
  finding for `d.ts`, and the spec delta has the scenario "Moved and
  copied next to an existing copy".
- **Mutants of D6**, each by hand and reverted, each failing a test:
  - no own-file pass: the pairing test, "Moved within a file and copied";
  - no consumption: the pairing test, "A helper copied into two files"
    and three more;
  - runs of one line: the pairing test;
  - empty runs allowed: the pairing test;
  - untrimmed hashes: "A split file", in the analyzer and end to end;
  - no written-line condition: "The same small edit in existing
    copies", "A copy completed by a few written lines", "A split file";
  - moved holders not skipped:
    `only_as_many_files_as_the_change_added_are_reported`;
  - moved holders not sorted first (only by path): "Moved and renamed",
    which runs with the moved copy first and last by path (added after
    the review).
- **The CLI's wiring** (task 4.4). The two end-to-end tests fail when the
  CLI swaps `before_lines` and `after_lines`, when it leaves them empty,
  or when the deleted file's or the JavaScript base's lines alone are
  empty.
- **A second decision: the slide** (D5). The first replay lost SV20,
  a defensible finding, which the spike kept. Our diff aligned the
  appended test's last lines with the old test's last lines, where git
  slides the insertion down. Ervin chose the slide. Three mutants each
  fail one of its tests: no slide of the added side, no slide of the
  removed side, and runs that do not join.
- **Replay of the 150 pull requests against `main` after PR 43:**
  - duplication falls from 279 to 139 findings, and no finding appears
    that did not overlap one before;
  - every finding of another rule is identical;
  - useful: 6 of 6 stay;
  - defensible: 12 of 13 stay, only SV30 goes, as the proposal expected;
  - noise: 20 of 22 go. SV28 overlaps the SV23 that stays and counts as
    gone, checked by hand.
- **SV23 instead of SV8** (accepted by Ervin). SV23 stays and SV8 goes,
  where the spike predicted the reverse. The written lines of both are
  the same as the spike's; the spike only estimated tokens with a
  regular expression. qgate's tokenizer counts 40 written tokens for SV8
  and 55 for SV23, against the threshold of 50.
- **Cost.** In the batch the slowest run, server PR 8812, took 7.5 s
  against 7.0 s. Timed alone, three times each and interleaved, the
  medians are 7.17 s before and 7.29 s after
  on PR 8812, and 6.68 s and 6.85 s on PR 8826: within 3 %.
- **qgate on itself.** It reported four Warnings, none failing.
  `written_lines` was a new function of complexity 11; building the pool
  of removed runs moved into `removed_runs`, and the finding went. The
  other three are +1 each, and each is one condition of the rule: a
  token's written flag in `with_token_hash`, the moved holders in
  `counts`, and the written-line test in `blocks`. They stay.
- **Review.** The `reviewer` subagent ran one round and found no
  blocker. Fixed after it:
  - no test pinned that a moved holder sorts before a written one; "Moved
    and renamed" now runs both path orders, and the mutant fails it;
  - the README said each removed line moves once (it is each run of
    three) and that the first new file by path is the original (a file
    the block only moved to is);
  - a comment still said the written flags can only silence a finding;
  - the slide is git's in the common case, not in every case;
  - the proposal named one new requirement instead of the two of the
    spec delta, and task 5.4 named four decisions instead of nine.
