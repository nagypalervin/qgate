# Design: rust-names-without-lifetimes

## Context

See proposal.md for why. Today the Rust adapter names an `impl` segment
from the source text of the `impl`'s `type` and `trait` fields, passed
through `compact` (whitespace removed except between two words). So the
name is the text as written, and lifetimes and any comment inside the
type come with it.

In tree-sitter-rust 0.24, lifetimes appear in a type as `lifetime` nodes
in four places: in `type_arguments` (`Walker<'s>`), in a `reference_type`
(`&'a str`), in a `bounded_type` (`dyn Fn() + 'a`), and in a
`for_lifetimes` list (`for<'a> fn(&'a str)`).

In qgate's own source, 11 `impl` blocks carry a lifetime: the three
adapters' `Walker`s, `Conventions<'a>` and `JsonFinding<'a>`.

## Goals / Non-Goals

**Goals:**

- No lifetime in any unit name, wherever it stands in the `impl`'s type
  or trait.
- Everything else in the name unchanged.

**Non-Goals:**

- Normalizing other spellings of the same type (`Self`, type aliases,
  `crate::` paths). They are real differences in the source, and none
  has shown up as noise.

## Decisions

### D1. Lifetimes only, type arguments stay

**Chosen:** drop lifetimes and nothing else.

**Alternatives:**
- *Drop all generic arguments* (`Stack<T>` → `Stack`). This would also
  make a rename of `T` harmless. But `impl From<u8> for Id` and
  `impl From<u16> for Id` would both become `<Id as From>.from`, told apart
  only by `#2`. That suffix follows source order, so reordering the two
  `impl`s would swap their identities and report false changes. Renaming
  a type parameter is rare. Two trait implementations with different
  arguments are common.
- *Write every lifetime as `'_`* (`Walker<'_>`). This would be stable too,
  but noisier, and it would still differ between `&str` and `&'a str`.

A lifetime never tells two `impl` blocks of one type apart: the compiler
rejects two inherent methods with the same name for `Walker<'a>` and
`Walker<'b>` as duplicate definitions. So dropping lifetimes loses no
distinction that a name needs.

### D2. Render the type from its syntax tree, not from its text

**Chosen:** build the segment from the leaf tokens of the `type` and
`trait` subtrees, joined with the same spacing rule `compact` applies.
The rendering skips:
- `lifetime` and `for_lifetimes` nodes;
- the `,` or `+` that separated a skipped lifetime from a sibling that
  remains;
- the `<` and `>` of a `type_arguments` node whose only arguments were
  lifetimes;
- comments.

**Alternative:** strip `'ident` with a pattern on the text. A
const-generic char argument (`Code<'x'>`) and a comment holding `'`
would break it, and fixing up `<,` and `+>` afterwards turns into a
parser of its own.

**Consequence:** a comment inside an `impl`'s type no longer ends up in
the name. Today `impl Stack</* item */ T>` is named `Stack</*item*/T>`.
The requirement text says so ("without lifetimes, comments and
whitespace").

### D3. Collisions keep the existing suffix

After dropping lifetimes, two `impl` blocks may give the same segment
(`impl Foo for &'a str` and `impl Foo for &'static str` are both
`<&str as Foo>`). Unit names already get `#2` when they repeat, so no
new rule is needed. Such a pair is not valid Rust when both are written
for the same type and trait, so this only matters for code under
`#[cfg]`.

## Risks / Trade-offs

- **[Names in existing reports change once]** → base and head are
  analyzed by the same binary, so no finding comes from the upgrade
  itself. Only the text of messages about such units changes between
  0.9.0 and the next release.
- **[A grammar node kind we did not list holds a lifetime]** →
  verification: the replay over the merges of `main`, run with the
  binary before and after, may differ only in unit names that contained
  `'`. A test also scans every unit name of qgate's own source for `'`.

## Migration Plan

None. qgate keeps no state between runs. The change ships with the next
release.
