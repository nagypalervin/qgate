# Design: share-adapter-walker

## Context

See proposal.md for why. Each adapter in `crates/qgate-lang` has its own
`Walker`. What the three share today, by copy:

| Part | TypeScript | C# | Rust |
|---|---|---|---|
| Non-recursive walk with an ancestor stack | yes, and it does not enter type-level subtrees | yes, passes all ancestors | yes, passes the parent |
| Frame undone on leave | flags (`segment`, `scope`, `bound`, `muted`, `not_code`) | counts (`segments`, `scopes`) and `muted` | counts and `muted`, `comment` |
| Scope that collects decision points | `Scope` enum: unit, class body, opaque | `Option<usize>` | `Option<usize>` |
| Opening a unit: unique segment, qualified name, `Function`, hasher | split in two (`open_unit` takes the qualified name; the bound-call path names it first) | one function | one function |
| Fingerprint per unit, finished at the end | yes | yes | yes |
| Tokens with lines, `muted`, text runs | yes, with text runs | yes, with text runs | yes, without |

The replay of `add-rust-analysis` reported 19 duplicated blocks among
these parts. The specs say nothing about the walkers, so no requirement
changes.

## Goals / Non-Goals

**Goals:**

- One copy of the walk, of the unit bookkeeping and of the token sink.
- Byte-identical output for every input: same units, names, lines,
  complexities, fingerprints, tokens, imports and usings.
- A fourth adapter that writes only its language's rules.

**Non-Goals:**

- A visitor framework or macros. The shared parts are plain structs and
  one generic function, `pub(crate)`.
- Sharing naming, decision points or token rules; they only look alike.
- Faster parsing. The walk stays one pass.

## Decisions

### D1. One walk, driven by a trait

`common.rs` gets `walk(root, visitor)` and a trait the adapters
implement:

```rust
pub(crate) trait Visit {
    type Frame;
    /// Called on entering `node`; `ancestors` end with its parent.
    fn enter(&mut self, node: Node<'_>, ancestors: &[Node<'_>]) -> Self::Frame;
    /// Called after the node's subtree, with what `enter` returned.
    fn leave(&mut self, frame: Self::Frame);
    /// Whether to walk into the node's children (TypeScript skips types).
    fn descend(&mut self, node: Node<'_>) -> bool { true }
}
```

Every adapter gets all ancestors; the Rust adapter, which only needs the
parent, takes the last. `descend` returns `false` for TypeScript's type
subtrees, and TypeScript records their `import(...)` there, as its walk
does today. *Alternative:* a closure-based walk. A trait keeps the
adapter's state and its frame type together and reads like the code it
replaces.

### D2. Unit bookkeeping, generic over the scope

`Units<S>` holds the segments, the taken names, the scope stack (`S`),
the functions and their hashers:

- `open(name, line) -> usize` makes the segment unique, pushes it,
  records the function and its hasher, and returns its index; `add`
  records a unit named elsewhere, and `qualify` gives the name a segment
  would have (TypeScript's bound calls, D3);
- `push_segment` and `push_scope`;
- `unit()` for decision points (`count`) and fingerprints (`hasher`),
  through the trait `UnitScope`, which says which unit a scope collects
  for;
- `finish() -> Vec<Function>` writes the fingerprints.

**Closing what a node opened** (decided during apply): a frame takes a
`Mark` of the stack lengths when the node is entered, and leaving it
truncates to that mark (`restore`). The old frames counted their pushes
and popped as many. The two are the same because every push of a
segment or scope happens in the node's own `enter`, and nothing in
`enter` pops: by the time a node is left, its children have restored
their marks. The tests of `common.rs` and the replay check it.

C# and Rust use `S = Option<usize>`, as before. *Alternative:* a fixed
scope type for all; TypeScript's class bodies and opaque scopes would
leak into the other two.

### D3. TypeScript as far as it fits

TypeScript uses the walk (D1) and the token sink (D4) for sure. It uses
`Units<Scope>` only if its bound calls fit `open` without a change of
behavior: today a bound call names the unit (`unique_segment` and the
qualified name) before anything nested is walked, so units stay in
source order. If `Units` needs a second entry point for that
(`open_named(qualified, line)`), it gets one; if the order of units or a
`#2` suffix would change, TypeScript keeps its own bookkeeping and the
decision is recorded here. The test of D5 decides, not taste.

**Outcome (task 2.4):** TypeScript fits `Units<Scope>` fully. A named
function is `open`; a bound call takes `unique_segment`, `qualify` and
`add`, in the same order as before, so units stay in source order. Its
frame keeps a flag for the bound-call stack and the TypeScript-only
`not_code` state, and the `Mark` for segments and scopes; the
`struct_excessive_bools` exception it needed is gone. `descend` skips
type-level subtrees and records their imports. The tests pass
unchanged; the replay (task 3.1) is the final word.

### D4. The token sink

`Tokens` holds the tokens, the muted state and the text run:

- `push(text, node, text_part)` records a token at the node's line and
  whether a text run continues;
- `mute(&mut frame.muted)` mutes and notes in the frame that it did;
  leaving that frame calls `unmute()`.

Rust never starts a text run; it passes `false`, as its walker
effectively does today.

### D5. Proof of no change

Three checks, recorded in "Verification" below:

1. **Tests:** the suites of `qgate-lang`, `qgate-analyzers` and
   `qgate-cli` pass without one changed line in a test or fixture.
2. **Replay:** the replay script of `add-rust-analysis` runs over all
   merges of `main` with a binary built from `main` and one built from
   this branch; the reports, without run times, must be byte-identical.
   The repository's history covers TypeScript (fixtures), C# and Rust.
3. **Copies gone:** in a scratch repository whose first commit is empty
   and whose second adds `crates/qgate-lang/src`, `qgate check --base
   HEAD~1` lists the duplicates among the adapters. On `main` that is
   the walker copies; on this branch none of them, and no new one.

### D7. Generic node helpers move too

Check 3 on `main` (task 1.1) also lists small helpers the C# adapter
copied from the TypeScript one: whether a node has a child of some
kinds, and whether a node's operator is one of some. They hold no
language rule, so they move to `common.rs` with the walker. Found
during apply; within the change's aim, recorded here.

### D6. One pull request, no release

Everything is in `qgate-lang` and checked mechanically, so one pull
request. No version bump: users see no difference; the next feature
release carries it.

## Risks / Trade-offs

- [A subtle order change, e.g. a hasher opened after a nested unit's] →
  The fingerprint tests and the byte-identical replay catch it.
- [Generic code is harder to read than three copies] → Keep the shared
  parts small and named after what they hold; no generic beyond the
  scope type and the frame.
- [TypeScript's walker is the oldest and most special] → D3 lets it take
  only what fits; partial sharing is still most of the 19 copies.
- [The replay needs the script from the `add-rust-analysis` scratchpad]
  → Its loop is in that change's design (D8); rewriting it is minutes.

## Migration Plan

None: an internal refactor of one crate, no public API, no
configuration.

## Verification

**Baseline (task 1.1, `main` at `59f5ea2`):**

- **Replay:** 54 merges of `main` (#35 and #36 merged since the replay
  of `add-rust-analysis`), 229 findings; the reports without run times
  are the baseline.
- **Check 3:** 30 duplicates among the adapters. 12 are between test
  modules (out of scope). 18 are in source code: 15 copies of the walker
  skeleton (the parse entry, `new`, `walk`, `open_unit`, `leave`, the
  token push) and 3 of generic node helpers (D7).

**After the refactor (task 3.1, the branch's release binary):**

- **Replay: byte-identical.** The 54 reports without run times equal the
  baseline byte for byte (SHA-256 `8e9eba0562e9a7fe...` both): the same
  229 findings, with their names, lines, complexities, copies, test
  findings and hotspot context, over TypeScript, C# and Rust code.
- **Check 3: 30 duplicates down to 16.** The 12 between test modules are
  unchanged (out of scope). In source code, 18 went down to 4:
  - **1 language rule, kept on purpose:** the start of TypeScript's and
    C#'s `is_decision_point` (a list check, then a `match` with
    `operator_in`). It is each language's rule and was in the baseline.
  - **3 short idioms of the skeleton, accepted:** the `parse` entry with
    the head of its `From<Walker>` (4 lines plus the struct literal),
    the start of `enter` that builds the adapter's own frame and records
    the token and the code, and TypeScript's and C#'s handling of
    literal and text-part tokens. Each adapter has its own frame type
    and fields; sharing these would need a further abstraction only to
    move tokens below the threshold, which would make the code harder
    to read, not easier to change. This differs from D5's "none of
    them"; it is put to Ervin in the pull request, and task 3.1 stays
    open until he decides.
- **Found while closing the gap:** the three `parse` functions had
  become the same after task 2.4 (a new copy); `run` now parses, walks
  and builds the model for all three. `Tokens::mute` records in the
  frame that it muted, so the adapters' own `mute` methods went too.
- **qgate on its own refactor** (`qgate check --base main`, 0.9.0): a
  first version split each walker's inherent `impl<'s> Walker<'s>` into
  `impl<'s> Walker<'s>` for `new` and `impl Walker<'_>` for the rest, and
  qgate reported `Walker<'_>.record_token` as a new function in all three
  adapters: a Rust name holds the `impl`'s type as written, lifetime
  included (D2 of `add-rust-analysis`), so renaming a lifetime renames
  the units. One inherent `impl<'s> Walker<'s>` again, as before, keeps
  the names. What remains are 5 Warnings: `<Walker<'_> as Visit>.enter`
  as a new function in C# and TypeScript (the method really moved into
  the trait; in Rust it stays below 11) and the three idioms above as
  new copies. The lifetime in names is a follow-up for the Rust naming
  rule.
