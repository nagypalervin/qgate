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
  records the function and its hasher, and returns its index;
- `push_segment`, `push_scope` and their pops, which the adapter's frame
  undoes;
- `current_unit()` for decision points and fingerprints, through a small
  trait on `S` that says which unit a scope collects for;
- `finish() -> Vec<Function>` writes the fingerprints.

C# and Rust use `S = Option<usize>`, as today. The frames keep counts
(`segments`, `scopes`), so leaving pops exactly what entering pushed.
*Alternative:* a fixed scope type for all; TypeScript's class bodies and
opaque scopes would leak into the other two.

### D3. TypeScript as far as it fits

TypeScript uses the walk (D1) and the token sink (D4) for sure. It uses
`Units<Scope>` only if its bound calls fit `open` without a change of
behavior: today a bound call names the unit (`unique_segment` and the
qualified name) before anything nested is walked, so units stay in
source order. If `Units` needs a second entry point for that
(`open_named(qualified, line)`), it gets one; if the order of units or a
`#2` suffix would change, TypeScript keeps its own bookkeeping and the
decision is recorded here. The test of D5 decides, not taste.

### D4. The token sink

`Tokens` holds the tokens, the muted state and the text run:

- `push(text, line, text_part)` records a token and whether a text run
  continues;
- `mute()` and `unmute()`, with the frame recording who muted.

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
