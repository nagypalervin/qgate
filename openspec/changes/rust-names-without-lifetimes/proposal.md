# Proposal: rust-names-without-lifetimes

## Why

A Rust unit's name holds its `impl`'s type and trait as written, lifetime
arguments included. `impl<'s> Walker<'s>` and `impl Walker<'_>` declare
methods of the same type, yet their units are named `Walker<'s>.record_token`
and `Walker<'_>.record_token`. While `share-adapter-walker` was being
written, splitting one `impl` block into two made `record_token` a "new
function" in all three adapters, and its old version vanished: the change
of a name nobody reads as a name. Lifetimes are bookkeeping for the borrow
checker, and elision lets the same type be written with `'_`, `'a`, `'s` or
no lifetime at all. They should not decide whether two versions of a
method are the same unit. The journal of `share-adapter-walker` lists this
as a follow-up.

## What Changes

- **Lifetimes leave a unit's name.** In the type and the trait of an
  `impl`, every lifetime is dropped (`'static` and `'_` too), together
  with the `,` or `+` that set it apart. Type arguments left empty lose
  their `<>`, and `for<'a>` lists are dropped:
  - `impl<'s> Walker<'s>` and `impl Walker<'_>` both give `Walker.record_token`;
  - `impl<'a> From<&'a Finding> for JsonFinding<'a>` gives `<JsonFinding as From<&Finding>>.from`;
  - `impl<'a, T> Stack<'a, T>` gives `Stack<T>.push`.
- **Comments leave it too.** The name is rendered from the syntax tree
  instead of the text (D2), so a comment inside an `impl`'s type no
  longer ends up in the name.
- **Everything else stays.** Type arguments, `&`, `mut`, `dyn` and paths
  stay in the name. Fingerprints, tokens and decision points are
  unchanged. The rule applies to Rust only, since TypeScript and C#
  have no lifetimes.
- **Finding messages change once.** Unit names appear in finding
  messages, so after an upgrade a report or PR comment names such units
  without lifetimes. Base and head are always analyzed by the same
  binary, so the upgrade itself creates no finding.

### Out of scope

- **Type arguments** (`Stack<T>`, `From<u8>`): they tell implementations
  apart. `impl From<u8> for Id` and `impl From<u16> for Id` must stay
  two units (design D1).
- **Generic parameters of a function** (`fn parse<'a>`): they are not
  part of a name today and do not become part of one.
- **The release**: the change ships with the next release.

### For Gate 1

1. **Lifetimes only, type arguments stay** (D1). *Recommended: yes.*
2. **Every lifetime goes**, `'static` included, wherever it stands in
   the `impl`'s type or trait: type arguments, references, bounds,
   `for<…>` (D2). A name that then collides with another unit's name
   gets the existing `#2` suffix. *Recommended: yes.*
3. **One pull request, no release.** The change touches `rust.rs`, its
   tests, a fixture and the spec. *Recommended: yes.*

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `rust-analysis`: the requirement "Rust qualified names" drops lifetimes
  from the names of `impl` types and traits.

## Impact

- **Code:** `crates/qgate-lang/src/rust.rs` (how an `impl` segment is
  rendered), its tests, and `fixtures/rust/units.rs`, whose
  `Parser<'a>.parse` becomes `Parser.parse`.
- **No other crate, dependency or configuration changes.**
- **Docs:** the README's Rust section, if it describes names with
  lifetimes; a journal entry.
- **Users:** after the next release, Rust unit names in finding messages
  have no lifetimes. qgate stores no names between runs, so nothing
  needs migrating.
