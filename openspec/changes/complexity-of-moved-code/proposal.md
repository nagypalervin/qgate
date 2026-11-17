# Proposal: complexity-of-moved-code

## Why

The Angular + .NET evaluation (`evaluate-angular-dotnet`, on its branch
until it merges) ended in "no-go until two fixes". This is the first fix,
the one Ervin put first: it removes the only false Error of the sample.

The complexity analyzer compares the two versions of one file and
matches functions by qualified name. Code that a change only moves
therefore looks new or grown:

- **CL11, an inlined function** (`bitwarden/clients` PR 24645). Removing
  a feature flag moved the body of `decryptManyV2` into `decryptMany` and
  deleted the V1 path. `decryptMany` went from 4 to 12, an Error that
  would fail a pull request which only removed code.
- **SV19, a moved function** (`bitwarden/server` PR 8856).
  `RunSuccessTestAsync(...)` moved from `IdentityServerSsoTests` into a
  new `SsoLoginTestHelpers` class in another file. Its qualified name
  changed, so it was reported as a new function of complexity 12.

A pilot can only switch to blocking with 0 false Errors, so this has to
be fixed first.

## What Changes

- **Complexity is analyzed per change, not per file.** The analyzer gets
  every changed file at once, together with the functions of files the
  change deleted, so it can see where a function came from. The
  existing `Analyzer::analyze` of one file keeps working as a change of
  one file (D1).
- **A moved function is matched across classes and files.** A function
  in the head that has no match by qualified name is compared with a
  function that the change removed somewhere else, when both have the
  same *own name* (the last segment of the qualified name; for C# it
  includes the parameter types). If several removed functions have that
  name, the one with the same fingerprint wins; if that does not settle
  it, nothing is matched and the function counts as new, as today (D2).
  A moved function that got more complex is reported under
  `complexity/cyclomatic-increase`, and its message names where it came
  from:
  `SsoLoginTestHelpers.RunSuccessTestAsync(...): cyclomatic complexity 12 -> 14 (+2), moved from IdentityServerSsoTests.RunSuccessTestAsync(...)`.
- **An inlined function is credited.** A removed function whose code
  now stands, for the most part, inside a function that grew (D3) adds
  its decision points to that function's before value. Only the growth
  beyond them counts towards the severity, and the message says so:
  `decryptMany: cyclomatic complexity 4 -> 15 (+11, +2 beyond the inlined decryptManyV2)`.
  When nothing is left beyond them, there is no finding.
- **A token knows its unit.** `qgate-lang`'s `Token` records which unit's
  own code it is: the same code that unit's fingerprint covers, in all
  three adapters. That way the analyzer can compare a removed unit's code
  with a grown one's (D3). Token text, and so duplication, does not change.

### Out of scope

- **Duplication in moved and edited code** (P1, P2, SV10 of the
  evaluation): the second fix, its own change.
- **A function renamed in place** (`calc` to `calculateTotal`, same
  class): its own name changes, so it stays "removed plus new", as the
  spec's "Renamed function" scenario says. Matching renames by
  fingerprint is a separate question.
- **A parameter added to a C# method** (SV6): the own name includes the
  parameter types, so it stays a new function. The evaluation judged
  that verdict right.
- **C# 14 `extension` members**: a separate follow-up.
- **The release**: the change ships with the next release.

### For Gate 1

Approved by Ervin on 2026-11-17 with all four recommendations.

1. **Matching by own name, unique or settled by fingerprint** (D2), in
   the same language family. *Recommended: yes.* The alternative,
   matching by fingerprint alone, misses a moved function that was also
   edited.
2. **Inlining needs evidence in the code** (D3): at least half of the
   removed function's tokens appear, in order, in the grown function's
   head version. *Recommended: yes.* The alternative, crediting every
   function removed from the same file, would also hide real growth
   when a pull request deletes dead code and adds logic elsewhere.
3. **The messages say "moved from" and "beyond the inlined"** (D4);
   the metric keeps the real before and after values. *Recommended:
   yes.*
4. **Two pull requests, no release.** PR 1: per-change analysis and
   moved functions. PR 2: the token's unit and inlining, the re-measurement
   of the 150 pull requests, and the archive. A release, 0.10.0, follows
   once the duplication fix is merged too, at your call. *Recommended:
   yes.*

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `complexity-analysis`: "Matching functions between versions" matches
  moved functions across classes and files; a new requirement credits
  inlined functions; "Finding content" adds the two message forms.

## Impact

- **Code:**
  - `crates/qgate-analyzers/src/complexity.rs`: the per-change analysis,
    moved and inlined functions, and its fixtures;
  - `crates/qgate-lang`: `Token::unit`, recorded by the shared token
    list (`common.rs`) from the unit each adapter already passes to the
    fingerprint;
  - `crates/qgate-cli/src/main.rs`: complexity runs once per change; the
    base versions of deleted files are parsed once, for both complexity
    and duplication.
- **No dependency or configuration change.** Rule ids stay.
- **Docs:** the README's complexity section, and a journal entry.
- **Users:** fewer `cyclomatic-new` findings for moved code, and fewer
  or milder `cyclomatic-increase` findings for inlined code. The message
  of a moved or credited finding is longer.
