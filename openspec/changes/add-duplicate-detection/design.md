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
    bytes and the trailing `\n`. A `missing` answer is an error.
  - Dropping the reader closes stdin and waits for the process.

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
- **Hash collisions** are possible in principle. Before a block is
  reported, the tokens of its first window are compared with the
  counterpart's. The counterpart's tokens come from the head file, which
  is read again only for this check; that happens rarely and only for
  reported blocks.

### D5: CLI wiring

In revision mode, after the per-file analyzers, and only when at least one
changed file could be analyzed:

1. `files_at(head)`, kept to supported extensions.
2. For each path that is not a changed file: `BlobReader::read`, then
   `parse`, then `index.add`, dropping the model at once. A file that
   cannot be parsed, or is not UTF-8, is skipped silently.
3. The changed files' head versions are added from their models, which
   are already parsed.
4. `index.findings()` joins the other findings before sorting.

The two-file form never builds an index.

### D6: No new ADR

- The dependency direction does not change.
- `cat-file --batch` is the step ADR 0002 already describes.
- The repository-level index is an analyzer-crate interface, recorded
  here.

## Risks / Trade-offs

- **[Run time]** Revision mode now parses the whole head tree on every
  run. → Task 3.5 measures it on a generated repository of 2,000 files
  and records the result here. Caching and parallel parsing are the
  follow-ups if it is too slow.
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
