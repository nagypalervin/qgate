# Proposal: duplication-of-new-code

## Why

The Angular + .NET evaluation (`evaluate-angular-dotnet`) ended in "no-go
until two fixes". The first, `complexity-of-moved-code`, is merged. This
is the second: duplication noise in code that a change moved or edited.
Duplication is 279 of the 386 findings in the replay of the 150 pull
requests, and 22 of the 41 judged duplication findings are noise. They
come from three patterns:

- **P1, a split file.** `bitwarden/server` PRs 8854, 8856 and 8859 split
  large test files into many: 114 of that repository's 181 duplication
  Warnings. Repetition inside the old file was not duplication ("copies
  across files only"). In the new files it is, although the code only
  moved.
- **P2, an edited copy.** `bitwarden/clients` PR 24403 adds
  `mock<LogService>()` to each importer constructor in 15 specs that were
  already alike (CL21, CL23, CL24). One changed line in each block makes
  the whole block new, because the edits lie less than 50 tokens apart.
- **Moved out of a deleted file** (SV10): the README says such code is not
  reported, but it was, from the split of PR 8854.

The rule measures copies by windows of 50 normalized tokens, so code
that moved and changed shape on the way (repetition spread over several
files, a small edit in every copy) looks new. A pilot needs the noise
low enough that a reviewer reads the Warnings.

## What Changes

- **Only code the change wrote counts.** A token counts towards a
  reported block only if its line is one the change wrote. A line the
  change wrote is an added line that did not come from removed code.
  Code that only moved does not count, even when repetition within one
  file becomes repetition across files (D1).
- **Moved lines are found by their text, in runs of three.** Lines are
  compared by their text without the leading and trailing white space,
  not by normalized tokens. Three consecutive added lines that match
  three consecutive removed lines moved. Each removed run is used once:
  - first by the file it was removed from;
  - then by other files, in path and line order.

  So a helper that a split copies into two files counts once as moved
  and once as written (D2, D3).
- **A file the block only moved to is no new copy.** When a new holder
  of a block has none of its tokens on written lines, it is not
  reported, and it is the original (design D1, decided during the
  apply).
- **The threshold stays 50.** At least 50 of a block's tokens must be new
  code. Today's other conditions stay (more files hold the block, at most
  as many as the change added; not code the file already shared).
- **`FileModel` gets line hashes.** `qgate-lang` records a hash of each
  line's text, so the analyzer can compare lines without the source
  (D4).
- **A line diff of each changed file.** Added and removed lines come from
  a diff of the file's two versions. It cuts the common start and end
  first, then runs Myers on the rest, with a limit. Beyond the limit,
  the rest counts as removed and added (D5). The common start and end
  cut (`middles`) moves from the complexity analyzer into the diff
  module and stays the same.

### Out of scope

- **Complexity's move evidence with Myers.** It is the follow-up of
  `limit-move-evidence`. The diff module could serve it later; this
  change does not touch complexity behavior.
- **Moves detected by normalized tokens.** A block moved and renamed at
  the same time counts as written. The exact text keeps three judged
  defensible findings (SV20, SV22, SV36) that normalized lines would lose.
- **A configurable threshold or run length.**
- **The release.** 0.10.0 follows at Ervin's call.

### For Gate 1

Decided by Ervin in the exploration on 2026-11-17:

1. Only code the change wrote counts. Moved code does not, also when it
   becomes repetition across files.
2. Runs of three lines with their exact text (trimmed). A single line is
   too weak: in a large refactoring (PR 24684) short lines such as `);`
   find a removed partner everywhere, and the useful CL14 would be lost.
3. One to one: a removed run is used by one added run only.
4. The threshold stays 50 new tokens. SV30 (defensible) is the known
   price. Its block begins in the tail of an existing test, so fewer than
   50 of its 73 tokens are new.

Approved by Ervin at Gate 1 on 2026-11-17, with both recommendations
below:

5. **Two pull requests, no release.** *Recommended: yes.*
   - PR 1: line hashes in `FileModel` and the diff module, with their
     tests. No behavior changes.
   - PR 2: the rule in the duplication analyzer, the spec, the README,
     the replay and the archive.
6. **Replay expectation** (from the spike, which used `git diff`).
   *Recommended: accept these as the bar.*
   - The judged duplication findings: 6 of 6 useful and 12 of 13
     defensible stay (SV30 goes), and 20 of 22 noise go (SV8 and SV27
     stay).
   - Duplication findings drop from 279 to about 136.
   - Every other finding stays identical.
   - Any judged finding that differs from this needs an explanation
     before merge.

Decided by Ervin during the apply, on 2026-11-17:

7. **A file the block only moved to is no new copy** (design D1). The
   written-line condition alone would let a moved copy, first by path,
   hide a written one when an unchanged file also holds the block.
8. **The line diff slides changes down, like git's diff** (design D5). Without
   it, a test appended after a similar test lost its written tokens, and
   the replay lost SV20.
9. **SV23 stays and SV8 goes, the reverse of item 6** (design
   "Verification"). Both have the spike's written lines; the spike
   estimated their tokens, and qgate counts 40 for SV8 and 55 for SV23.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `duplication-analysis`:
  - "Only new duplication is reported" counts only tokens on lines the
    change wrote;
  - two new requirements, "Lines the change wrote" and "Moved lines",
    define added, moved and written lines, with scenarios for P1, P2
    and a helper copied into two files.

## Impact

- **Code:**
  - `crates/qgate-lang`: `FileModel::lines`, set by `parse` for every
    language;
  - `crates/qgate-analyzers`:
    - a new `diff` module (`middles`, Myers with a limit);
    - `complexity.rs` uses the moved `middles`;
    - `duplication.rs` gets the line pairing and the written-line
      condition;
  - `crates/qgate-cli/src/main.rs`: `ChangedSource` carries the line
    hashes of both versions.
- **No dependency or configuration change.** Rule ids stay.
- **Docs:** the README's duplication section, and a journal entry.
- **Users:**
  - far fewer duplication Warnings for split files and repeated small
    edits;
  - a block that is half moved and half written is reported only when
    the written half has 50 tokens.
