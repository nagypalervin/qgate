# Design: add-duplicate-detection

## Context

See proposal.md for why and what.

- **Analyzers today are per file.** `Analyzer::analyze(&FileChange)` sees
  one file's two versions. Duplication needs every file of the head
  revision at once.
- **`qgate-git` reads only what changed.** It reads the changed files
  (`changed_files`, `read_blob`) and one file at a revision (`file_at`).
  Each read is one `git` process. ADR 0002 names `git cat-file --batch`
  as the next step when per-call cost matters.
- **The TypeScript adapter** walks each tree once and skips type-level
  subtrees (`TYPE_KINDS`). It records functions and imports, but no tokens.
- **Size:** a large TypeScript repository has a few thousand files and
  some million tokens. Keeping every token of every file in memory would
  cost hundreds of megabytes, so the head tree is streamed.

## Goals / Non-Goals

**Goals:**

- Implement the `duplication-analysis` spec in three pull requests.
- **Read the head tree once,** in one git process, and keep only what the
  findings need: counts and positions of the windows that the changed
  files contain.
- **Be deterministic:** the same input gives the same bytes. Hashes are
  fixed (no `RandomState`), maps are ordered or sorted before output.

**Non-Goals:**

- Caching parsed files between runs.
- Parallel parsing. It can come later without changing the interfaces.
- Type-3 clones, i.e. copies with gaps.

## Decisions

### D1: Normalized tokens in `FileModel`

```
FileModel { functions, imports, tokens: Vec<Token> }
Token { text: String, line: u32 }   // text already normalized
```

The walk emits a token for each leaf node (`child_count() == 0`) and
skips comments. Normalization:

| Leaf | `text` |
|---|---|
| `identifier`, `shorthand_property_identifier`, `statement_identifier` | `$id` |
| `property_identifier`, `private_property_identifier` (member names, object keys) | as written |
| `string` (the whole node, not entered), `number`, `regex` | `$lit` |
| `string_fragment` inside a template string | `$lit` (backticks, `${` and `}` stay) |
| keywords, operators, punctuation, `this`, `true`, `null`, ... | as written |

- **No tokens:** these subtrees produce none:
  - `import_statement`, and `export_statement` with a `source`;
  - the type-level subtrees: `TYPE_KINDS`, plus `implements_clause`, the
    type operand of `as`/`satisfies`, and `type_identifier` or
    `predefined_type` leaves anywhere.
- **Recorded in `enter`, while the walk runs,** so token order is source
  order.
- **The cost:** one `String` per token while the model lives. That is
  fine for one file; the head tree is streamed (D3), so no more than one
  file's tokens are alive at a time, plus the changed files.

### D2: Reading the head tree in `qgate-git`

- **`Repository::files_at(revision) -> Result<Vec<TreeFile>, GitError>`**
  - `TreeFile { path: String, id: BlobId }`, sorted by path.
  - Runs `git ls-tree -r -z --full-tree <commit>`, after the revision is
    resolved with the existing `resolve`, so option-like revisions are
    rejected.
  - Keeps regular files only (modes `100644`, `100755`), like
    `changed_files`.
- **`Repository::blob_reader() -> Result<BlobReader, GitError>`**
  - Starts `git cat-file --batch` once, at the work-tree top, with the
    same environment as `run`.
  - `BlobReader::read(&mut self, id) -> Result<Vec<u8>, GitError>`
    writes `<id>\n`, then reads `<id> <type> <size>\n`, then `<size>`
    bytes and the trailing `\n`.
  - A `missing` answer, a non-blob object or a blob above 64 MiB is an
    error that leaves the reader in step. An answer out of step closes
    it. When git ends, the error carries its exit status and the end of
    its stderr.
  - Git's stderr is drained on a thread while git runs, keeping the last
    8 KiB.
  - Dropping the reader closes stdin and stdout and waits for the
    process. The drain is not joined: a process git started may still
    hold stderr open, and the thread ends on its own.

The git crate stays language-free: the CLI picks the supported paths.

### D3: A streaming duplication index in `qgate-analyzers`

Duplication is a repository-level analysis, not an `Analyzer`. The
module `duplication` exposes an index that is fed file by file:

```
let mut index = DuplicationIndex::new(&changes);     // changed files: path, before tokens, after tokens
for file in head_files { index.add(path, &tokens) }  // every compared head file except the changed ones
let findings = index.findings();
```

1. **Windows.** A window is `K = 50` consecutive tokens. Its hash
   combines the token hashes polynomially, in wrapping `u64` arithmetic
   (FNV-1a for the token text, a fixed odd base for the window). A
   rolling update makes each window O(1).
2. **`new`.** It hashes every window of the changed files' head versions
   (the candidates), counts each candidate's occurrences there, and
   counts its occurrences in their base versions. That gives
   `delta[h] = after - before`.
3. **`add`.** For a head file, it hashes every window and, for each
   candidate hash, records the occurrence `(path, token index, first
   line, last line)`. Other windows are dropped, so memory grows with the
   changed code, not with the repository.
4. **`findings`.**
   - A candidate is a new duplicate if it occurs at least twice in the
     head and `delta > 0`.
   - Its occurrences in the changed files are ordered by path and
     position. In each file, the first `before` occurrences count as
     old; the rest are new.
   - If no old occurrence exists anywhere, the first new one is the
     original and is not reported.
   - At most `delta` new occurrences are reported.

### D4: Blocks and their counterparts

- **Blocks.** In each changed file, the reported window starts are merged
  when consecutive (`i`, `i+1`, ...). A block covers tokens `first ..
  last + K`, so its token count is `last - first + K`, and its lines run
  from the first token's line to the last token's line.
- **The counterpart** is the first occurrence, by path and line, of the
  block's first window that is not itself reported.
  - Its last line is the last line of that file's occurrence of the
    block's last window at the matching offset. That window is a
    candidate too, so its occurrence was recorded.
  - "and `k` more" counts the other occurrences of the first window that
    are not reported.
- **Hash collisions** are possible in principle. *Superseded in PR 3 (see
  Implementation Notes): two independent hashes per window, no re-read.*
  Before a block is reported, the tokens of its first window are compared
  with the counterpart's. The counterpart's tokens come from the head
  file, which is read again only for this check; that happens rarely and
  only for reported blocks.

### D5: CLI wiring

In revision mode, after the per-file analyzers, and only when at least one
changed file could be analyzed:

1. `files_at(head)`, kept to supported extensions.
2. For each path that is not a changed file: `BlobReader::read`, then
   `parse`, then `index.add`, dropping the model at once. A file that
   cannot be parsed, or is not UTF-8, is skipped silently.
3. The changed files' head versions are added from their models, which
   are already parsed. *Superseded in PR 3: `DuplicationIndex::new`
   counts them itself; deleted files and files renamed to another
   language are base-only inputs.*
4. `index.findings()` joins the other findings before sorting.

The two-file form never builds an index.

### D6: No new ADR

- The dependency direction does not change.
- `cat-file --batch` is the step ADR 0002 already describes.
- The repository-level index is an analyzer-crate interface, recorded
  here.

## Risks / Trade-offs

- **[Run time]** Revision mode now parses the whole head tree on every
  run. → Measured (task 3.4), release build, aarch64 Linux VM:
  - repository: 2,000 generated TypeScript files, 224,240 lines;
  - change: 20 files, each adding a copy of a function from another file;
  - result: 1.4 s wall time and 21 MB peak memory (three runs, all within
    0.02 s); all 20 copies found.

  - a 400-file change on the same repository: 2.4 s and 317 MB peak.

  Memory does not depend on the repository size, because only the
  changed files' windows are kept (D3), but it is high per changed token:
  every token is a `String`, and every window a map entry. Interning
  token texts and compacting the window maps are the follow-ups, with
  caching and parallel parsing, before qgate meets very large pull
  requests.
- **[Which copy is new within one file]** Old and new are told apart by
  counts, not by a diff. Within one file, the first occurrences by
  position count as old, so a copy inserted *above* the block it copies is
  reported at the old block and names the new one. The duplication is
  real; only the side shown is wrong. → Accepted (decided at PR 3 review)
  and stated in the spec and the README; a token diff per changed file
  is the fix if it matters.
- **[False positives of type-2 normalization]** Structurally identical
  but unrelated code (switch arms, test tables, configuration objects)
  can match. → Kept member names, the exclusions and the 50-token
  threshold limit this; it is a Warning that fails nothing by default;
  thresholds and excludes in `qgate.toml` are the next step.
- **[Hash collisions]** Checked against the tokens before reporting (D4).
- **[Grammar lag]** A head file the grammar cannot parse takes no part,
  silently. A copy of code that sits in such a file is not found.
- **[Changed-file order]** Which new occurrence counts as the original
  when all are new depends on path order. This is deterministic, but
  arbitrary from the user's point of view; the message names the other
  copy either way.

## Migration Plan

None. The two-file form is unchanged; revision mode gains a Warning-level
finding, which does not fail a run by default.

## Implementation Notes

- **PR 1, tokens:**
  - `type_identifier` leaves become `$id`. Type positions are never
    entered, so the ones the walk reaches name classes (`class K`); D1's
    "no token" for them would have dropped class names.
  - The `as` and `satisfies` keywords stay; only their named type operand
    is muted.
  - A file renamed from JavaScript keeps the tokens of its JavaScript
    version as its base, like its imports (`add-forbidden-imports`). A
    pure JS-to-TS rename therefore adds no occurrences. If the JavaScript
    cannot be read, the head's own tokens stand in.
  - `Frame` has four flags now; clippy's `struct_excessive_bools` is
    allowed with a comment, because each flag is an independent undo step
    for one stack, not a state.
- **PR 1, review round 1:**
  - Type-level code leaked tokens. Muted now:
    - `export interface/type`, `export type {}` and `export as namespace`;
    - type-only specifiers, `import X = N.Y`;
    - overload, abstract and index signatures, `declare` declarations and
      fields;
    - the `as`/`satisfies` keyword with its type, `x!`, `let x!`, optional
      `?`, and a `this` parameter.

    Without this, declaration files and overload sets, identical after
    normalization, would be reported as duplicates. The spec has a
    separate requirement, "Code without tokens", with a scenario for
    declaration files.
  - A literal is one `$lit` whatever its content. Anonymous keyword leaves
    named `string`/`number` (in `predefined_type`) are no longer taken for
    literals, and a run of text parts (`\n` escapes, `&amp;` in JSX text)
    is one token.
  - Semicolons are dropped with comments and the shebang line: automatic
    semicolon insertion makes them layout, so `a = 1` and `a = 1;` match.
  - Lowercase JSX element names (`div`) and namespaced JSX names
    (`xlink:href`) keep their text, like attribute names; components
    (`Button`) are variables. The spec lists the kept names.
  - The walk passes each node's parent along, instead of calling
    `Node::parent()`, which walks down from the root.
  - The leftover comma of a muted `this` parameter (`( , $id )`) is
    accepted: it is the same in every copy.
  - Task 3.2 now includes the JS-to-TS rename, also with unreadable
    JavaScript, and the spec states that rule.
- **PR 1, review round 2:**
  - Class method and constructor overloads (`method_signature`) are
    muted; they were the common form of overloads the first round missed.
  - The definite-assignment `!` on class fields and the optional `?` on
    class methods are muted too.
  - Tests now pin abstract fields, optional static fields and the
    shebang line. The `is_named` guard before literals has a comment: the
    `string`/`number` keywords it guards against only occur in type
    positions, which are muted or never entered.
  - The spec wording now matches the code:
    - `true`, `null` and the like are kept keywords;
    - a template or JSX text part between substitutions is one
      placeholder, so `` `${a}` `` and `` `-${a}` `` differ by that part;
    - namespaced JSX names are kept.
  - Known exception: a namespace that holds only types keeps its skeleton
    (`export namespace $id { }`, five tokens). It is far below the
    50-token threshold.
- **PR 2, reading the head tree:**
  - `run` and the batch process share one `git_command` helper (same
    environment, test isolation and spawn errors).
  - The batch process's stderr is drained on a thread and its end kept
    for error messages (see "PR 2, review round 2").
  - An id that names a tree or commit is an error, but its contents are
    read and discarded first, so the next answer starts cleanly; a test
    reads a blob after a tree.
  - `files_at` sorts by path explicitly, although git's tree order already
    agrees with byte order for full paths.
- **PR 2, review round 1:**
  - Dropping a reader could hang if an answer was left unread: `wait()`
    ran while stdout was still open and git was blocked writing. `close`
    now drops stdin and stdout before waiting, so git exits on end of
    input or on the closed output.
  - When the process ends or a pipe fails, `read` waits for it and
    reports its real exit status and stderr. Before, every case was
    "failed with a signal: Broken pipe", and git's message was lost.
  - An answer out of step (one that does not start with the requested id,
    has a bad size, or lacks the trailing newline) closes the reader, and
    every later `read` fails at once instead of parsing leftover bytes.
    `<id> missing` and a non-blob object keep it in step.
  - Tests: a 1.5 MB blob with NULs and newlines, a commit id, a repository
    without `.git` (exit 128 and git's message), a killed process, an
    answer out of step, and a drop with an unread 2 MB answer (with a
    timeout, so it fails rather than hangs).
  - `TreeFile.path` documents its lossy UTF-8, like `ChangedFile.path`.
- **PR 2, review round 2:**
  - Reading stderr only after git ended brought back a deadlock: with a
    `GIT_TRACE*` setting git can write more than a pipe holds while it
    is still answering, and then blocks. A thread now drains stderr while
    git runs and keeps its last 8 KiB. A test reads 1,500 packed blobs
    with `GIT_TRACE_PACK_ACCESS` and a timeout.
  - A blob above 64 MiB is skipped (read and discarded) with an error
    that keeps the reader in step, instead of a size-sized allocation;
    tests use a 2 MiB limit to exceed it cheaply. PR 3 treats a read
    error for an unchanged head file like an unparsable file: skipped.
  - A header that is not UTF-8 is a protocol error, not an I/O error;
    stderr is decoded lossily like `stderr_of`; `exchange` takes the open
    pipes, so the duplicate "closed" guard is gone.
- **PR 2, review round 3:**
  - `Drop` no longer joins the stderr drain: a process git started (a
    lazy fetch, an ssh helper) can inherit stderr and keep it open, which
    made every drop wait for that process. `fail` still joins it, after
    git has exited, because it needs the text.
  - The drain retries on `Interrupted`; a thread that cannot be started
    is a `GitError`, not a panic.
  - Object sizes are parsed as `u64`, so a blob too large for `usize`
    is skipped like any oversized one; a test reads a blob of exactly
    the limit.
- **PR 3, the analysis:**
  - `DuplicationIndex::new` counts the changed files' head versions as
    head files itself; the CLI adds only the unchanged files. A changed
    file can then never be added twice or forgotten.
  - Hash collisions: each window has two independent 64-bit hashes (FNV-1a
    with two seeds, two polynomial bases) instead of D4's re-reading of
    the counterpart. This is deterministic and needs no second read; a
    test forces the first hash to collide everywhere.
  - Overlapping occurrences in one file are one copy, not two. In
    repetitive code (eleven `if (c[i]) { return i; }` lines) a window
    equals itself shifted by one statement, which reported the function
    as a duplicate of itself. Repetition far enough apart not to overlap
    still counts, as the spec says ("in the same file").
  - What "new" means in counts: the number reported for a block is at
    most the number of occurrences the changed files gained in total, so
    code that moved from one changed file to another adds nothing.
  - The e2e helper `function()` writes `if (c.k0)`, `if (c.k1)`, ... so
    its generated decisions are not repeated code.
  - Task 3.3: breaking the old/new split per file, the original rule,
    block merging, the overlap rule, the before subtraction, the per-block
    budget and the second hash each fails a test. The member-name and
    import rules live in `qgate-lang` and were checked the same way in
    PR 1. A one-place fast path (`places.len() < 2`) is an equivalent
    mutant: the original rule covers it.
- **PR 3, review round 1:**
  - Code moved out of a deleted file, or out of a file renamed to another
    language, was reported as a new copy: only changed files counted as
    "before". `Repository::deleted_files` lists regular files the head no
    longer has (deleted, or replaced by a symlink), and the CLI feeds them
    and the renamed-away files as base-only inputs. E2e tests cover both.
  - A block now extends only as far as one copy continues it, and the
    finding names a copy of the whole block (the first by path that
    covers it), with "and k more" counting the copies of the whole block
    that do not overlap each other. Before, a run could join windows with
    different counterparts, name a partial copy, or count shifted views.
  - The before totals per key are summed once in `new`. Before, each key
    looped over every changed file, which took 31 s for 800 changed files
    in the reviewer's measurement.
  - A read error that closed the batch reader is fatal (`is_closed`),
    like a git process that ended; only a single unreadable object is
    skipped.
  - Known limitation recorded (human decision): within one file, the
    first occurrences count as old (see Risks).
  - Not changed: a changed file whose base version cannot be parsed is
    `input/unparsable` and takes no part in duplication either, as
    "Compared files" says for files that cannot be parsed.
