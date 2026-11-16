# Proposal: share-adapter-walker

## Why

The replay of `add-rust-analysis` ran qgate over its own history and
found 19 of its 20 source-code duplicates in the language adapters: the
C# adapter copied the TypeScript adapter's walker skeleton in 9 places,
and the Rust adapter, written by the same agent a few days later, copied
the C# one in 9 more. Every new language would copy it again, and a fix
in one walker (stack safety, frame bookkeeping, fingerprints) would have
to be found and made three times. It is the follow-up the journal of
`add-rust-analysis` ranks first.

## What Changes

- **A shared walker skeleton in `crates/qgate-lang/src/common.rs`:**
  - the non-recursive depth-first walk that tracks a node's ancestors
    and lets each node push state that leaving it pops;
  - the bookkeeping of function units: name segments, unique qualified
    names, the scope that collects decision points, and a fingerprint
    hasher per unit;
  - the token sink: normalized tokens with their lines, and the muted
    state of subtrees that produce no tokens.
- **The C# and Rust adapters use it** and keep only what is theirs:
  which nodes are units and how they are named, decision points, token
  rules, C# usings.
- **The TypeScript adapter uses the walk and the token sink.** Its unit
  bookkeeping (a `Scope` enum with class bodies and opaque scopes, calls
  bound to a name, decorators) differs enough that it keeps its own,
  unless sharing it turns out to need no change of behavior (design D3).
- **No behavior change.** Every finding, unit name, line, complexity,
  fingerprint and token stays as it is: no spec changes, no version
  bump, no README change.

### Out of scope

- **The `Repo` test helper** that `qgate-git` and the CLI's tests both
  have: a separate follow-up, since sharing test code across crates
  needs a feature or a crate of its own.
- **Test helpers inside the adapters' test modules** (`names`, `pairs`,
  `tokens`, `texts`): short and local to each test module.
- **Language-specific logic:** naming, decision points and token rules
  stay in each adapter, even where they look alike.

### For Gate 1

1. **A refactor with no spec change** (`skip_specs`), verified three ways:
   the existing tests pass unchanged; the replay over the merges of
   `main` gives byte-identical JSON with the binaries before and after;
   qgate's own duplication check, run over the change, reports the
   walker copies as gone and no new copy. *Recommended: yes.*
2. **TypeScript only as far as it fits** (D3): the walk and the token
   sink for sure, the unit bookkeeping only if it needs no behavior
   change. *Recommended: yes.*
3. **One pull request.** The change touches only `qgate-lang`, and its
   check is mechanical. *Recommended: one PR, no release; it ships with
   the next feature release.*

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None: behavior does not change (`skip_specs: true`).

## Impact

- **Code:** `crates/qgate-lang/src/common.rs`, `csharp.rs`, `rust.rs`,
  `typescript.rs`. No public API change: everything shared is
  `pub(crate)`.
- **No dependency, no version bump.** The binary's behavior is the same;
  the next release carries it.
- **Docs:** the module docs of the adapters and of `common.rs`; a
  journal entry.
