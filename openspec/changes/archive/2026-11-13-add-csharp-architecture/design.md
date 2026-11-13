# Design: add-csharp-architecture

## Context

- **The architecture analyzer** reads `FileModel::imports`, which only
  the TypeScript adapter fills. For each rule whose `from` matches the
  file's path, it resolves each specifier to a target. It then reports
  the (rule, target) pairs that the base version (at the old path) did
  not have. C# files already reach it, since the CLI runs every
  analyzer on every supported, not excluded, changed file. Their model
  has no imports, so it never reports.
- **Path patterns** are `PathPatterns` in `qgate-analyzers/src/patterns.rs`
  (from `add-path-exclusion`): globs over `/`-separated names, where a
  plain name also matches below it and `x/**` also matches `x`.
- **`qgate-config`** reads `[[architecture.forbid]]` into `ForbidRule
  { name, from, to }` with `deny_unknown_fields`. It checks names and
  non-empty lists; the CLI copies the rules into the analyzer's type
  (ADR 0003).
- **The C# adapter** mutes `using_directive` subtrees for tokens and
  reads nothing else from them.
- **Spike (tree-sitter-c-sharp 0.23.5, 2026-11-13).** Every form in the
  proposal parses without error.
  - A `using_directive` has an optional `name` field (the alias).
  - The used name is the other named child:
    - an `identifier`;
    - a `qualified_name`, with `qualifier` and `name` fields, nested;
    - an `alias_qualified_name` (`global::Shop`, with `alias` and
      `name`);
    - a `generic_name` with a `type_argument_list` as the last segment;
    - for aliases, also `tuple_type`, `pointer_type`, `array_type` or
      `predefined_type`.
  - Where a using can sit:
    - after a file-scoped namespace, as a sibling;
    - inside a block namespace's `declaration_list`;
    - inside `preproc_if`.
  - `using var x = ...` is a `local_declaration_statement`, and
    `using (x) {}` is a `using_statement`; neither is a directive.
  - Fully qualified names in code (`new Shop.Infrastructure.Db()`) are
    `qualified_name` nodes too, which a later change could use (see
    Non-Goals).

## Goals / Non-Goals

**Goals:** C# `using` directives checked against forbid rules with the
same reporting rules as TypeScript imports. The analyzer reports a new
violation only once, and the CLI needs no new branch.

**Non-Goals:**
- fully qualified names in code;
- usings that MSBuild adds;
- project references;
- resolving relative to the enclosing namespace;
- `from` by declared namespace.

## Decisions

### D1. The model gets `usings`, separate from `imports`

- **Shape:** `FileModel` gets `usings: Vec<Using>`, where `Using { name:
  String, line: u32 }`. `name` is the normalized target (D2), and
  `line` is the directive's first line. TypeScript leaves it empty;
  C# leaves `imports` empty.
- **Why not reuse `Import`:** an import's specifier is resolved by the
  analyzer (relative paths), while a using's name is already final.
  Keeping them apart stops a C# name from ever meeting a path pattern,
  and a TypeScript specifier from meeting a namespace pattern. The
  proposal's "each list applies to its language" follows from the
  model, not from a language check in the analyzer.

### D2. The adapter normalizes the target

- **Normalization is syntax, so it belongs in `qgate-lang`.** Walking
  the used-name node:
  - `identifier`: its text without a leading `@`;
  - `qualified_name`: qualifier, `.`, name;
  - `alias_qualified_name`: the name, if the alias is `global`;
    otherwise no target;
  - `generic_name`: its identifier, with the type arguments dropped;
  - any other node: no target.
- **Whitespace and comments** between segments disappear by
  construction.
- **Type arguments are not uses.** `using L = List<Shop.Data.Row>;`
  uses only `System.Collections.Generic.List`. A namespace inside type
  arguments is rare in a using alias, and counting it would need a
  second walk; this is recorded as a limit.
- **Where usings are collected:** every `using_directive` in the tree,
  in source order, wherever it sits (top level, namespace body,
  `#if`). A directive cannot appear inside a type or a method, so
  nothing else needs excluding.

### D3. `to_namespaces` in the existing rule (Gate 1, question 1)

- **Config:** `ForbidRule` gets `to_namespaces: Vec<String>`, and `to`
  becomes optional too, so that either list may be missing (serde
  default).
- **Validation in `qgate-config`:**
  - a list that is given but empty is an error, as now;
  - a rule with neither list is an error that names `to` and
    `to_namespaces`.
  - Telling "missing" from "empty" needs `Option<Vec<String>>`.
    **Decided in task 1.2:** the public fields are `Option<Vec<String>>`
    as well. The alternative, plain `Vec`s filled from a private serde
    struct through `try_from`, would put the TOML position in front of
    the list errors and change their messages; `None` also says plainly
    "not given".
- **Why one table:**
  - a layer rule for a full-stack repository is one name with both
    lists ("the domain must not depend on the infrastructure");
  - `from` is a path pattern in both languages;
  - one table keeps one rule name space, one place for duplicate
    names, and one finding format.
- **Rejected:**
  - a separate `[[architecture.forbid-namespace]]` table (two places
    to look, and duplicate names across them);
  - reusing `to` with dots: a namespace pattern would be checked
    against TypeScript specifiers, and `Shop.Data` would not match
    `Shop.Data.Rows`, because a plain path pattern matches below it
    only at `/`.

### D4. Namespace patterns reuse `PathPatterns`

- **`NamespacePatterns::new`:**
  - checks the namespace spelling: not empty, no empty segment (which
    also catches a leading or trailing `.`), no `/`;
  - replaces every `.` with `/`;
  - compiles the result as `PathPatterns`.
- **`matches(name)`** does the same replacement on the name. So
  `Shop.Infrastructure` matches below itself, `**.Data` is `**/Data`,
  and `Shop.Infrastructure.**` also matches `Shop.Infrastructure`, all
  by the path rules.
- **Errors:** an `InvalidPattern` with the pattern as written. The
  architecture analyzer wraps it with the rule's name, as for `from`
  and `to`, so the message is `qgate.toml: rule "domain-is-pure":
  invalid pattern "Shop..Infrastructure": ...`.
- **Why the check comes first:** path normalization drops empty
  segments, which would turn `Shop..X` into `Shop/X` silently.

### D5. The analyzer, one key space

- **Shape:** `CompiledRule` gets `to_namespaces: NamespacePatterns`;
  an absent list is an empty set that matches nothing.
- **Matching:** `violations` adds, for each rule whose `from` matches,
  the `usings` whose name matches `to_namespaces`. The key stays
  (rule index, target), and the value stays the first line and its
  written form.
- **Why the key is unambiguous:** C# names and TypeScript targets
  never share a file.
- **The message:** a C# violation shows `uses <name>`; a TypeScript one
  keeps `imports <target>` and its specifier. The value records which
  kind it is.
- **Rule id (Gate 1, question 2):** it stays
  `architecture/forbidden-import`. Existing `--fail-on` behavior, the
  JSON schema and the PR comments need no change.

### D6. What the CLI and the other analyzers notice

- **The CLI:** `architecture_analyzer` copies `to_namespaces`. Nothing
  else changes:
  - generated and excluded files never reach the analyzer;
  - a C# base version is read like any other;
  - an unparsable version yields no architecture findings.
- **Duplication:** usings still produce no tokens, so duplication is
  unchanged.
- **The two-file form** stays without rules.

### D7. Nested usings as written (Gate 1, question 3)

- **The C# rule:** inside `namespace Shop.Domain { using
  Infrastructure; }`, C# looks the name up in `Shop.Domain`, then in
  `Shop`, then globally.
- **Why not resolve it:** qgate cannot know which of these exists
  without reading the whole compilation. Checking every reading would
  report `Shop.Domain.Infrastructure` for a rule on
  `Shop.Domain.Infrastructure`, which may not exist. The README states
  the limit.
- **Why the limit is acceptable:** idiomatic code puts usings at the
  top with full names, and the evaluation (D8) measures how often this
  happens.

### D8. Evaluation and rehearsal before the pilot (Gate 1, questions 4 and 5)

- **Evaluation:**
  - **Rules:** layer rules a team would write:
    - `jasontaylordev/CleanArchitecture`: Domain must not use
      Application, Infrastructure, Web or `Microsoft.EntityFrameworkCore`;
      Application must not use Infrastructure or Web;
    - `dotnet/eShop`: the Ordering domain must not use its
      infrastructure, and similar.
  - **Run:** `qgate check` over each of their last 30 commits that
    touch C# (as the C# evaluation did), with `exclude =
    ["**/Migrations/**"]`.
  - **Record:**
    - each finding, checked against the diff;
    - the time per commit;
    - how many usings are nested in a namespace block, or are global
      usings in a separate file.
  - **A false positive or a miss** that the spec does not explain
    stops the release until Ervin decides.
- **Azure rehearsal:**
  - **Setup:** the README's Linux job at 0.6.0, in the rehearsal
    project, on a pull request that adds `using
    Shop.Infrastructure.Data;` to a domain file, with a rule in
    `qgate.toml`.
  - **Expected:**
    - an Error row in the comment;
    - a build issue;
    - the check step fails.

### D9. Two pull requests and 0.6.0 (Gate 1, question 6)

1. **PR 1, `change/add-csharp-architecture`:**
   - D1 to D6, with adapter, config, pattern, analyzer and CLI tests;
   - the README's architecture and C# sections;
   - version 0.6.0 in `Cargo.toml` and `action.yml`;
   - the evaluation (D8), recorded here before the tag.
2. **Release:** Ervin agrees, the agent pushes `v0.6.0`, Ervin
   publishes.
3. **PR 2, `change/add-csharp-architecture-2`:**
   - the README versions;
   - the roadmap ("Languages" names the C# rules; "Later" loses them);
   - the release record and the Azure rehearsal;
   - the journal entry and the archive.

## Evaluation (task 1.7, 2026-11-13)

**Repositories** (partial clones, nothing of theirs was built or run):
- `jasontaylordev/CleanArchitecture` at e1ab4cc (2026-11-09): 110 C#
  files, 193 using directives;
- `dotnet/eShop` at 913acea (2026-10-15): 547 C# files, 762 using
  directives.

**Binary:** `cargo build --release` of this branch (`qgate 0.6.0`,
aarch64 Linux), against the published 0.5.0 binary for comparison.

**Rules** a team would write, with `exclude = ["**/Migrations/**"]`
(the exact files are below):
- **CleanArchitecture:**
  - the domain must not use `CleanArchitecture.Application`, `.Infrastructure`,
    `.Web` or `Microsoft.EntityFrameworkCore`;
  - the application must not use the infrastructure, the web project
    or EF Core.

  The template allows EF Core in the application on purpose, so the
  last rule is stricter than its authors' and is there to see a
  finding.
- **eShop:**
  - the Ordering domain must not use `eShop.Ordering.Infrastructure`,
    `eShop.Ordering.API` or EF Core;
  - the Ordering infrastructure must not use the API;
  - `Catalog.API` must not use `eShop.Ordering`.

**(A) The last 30 non-merge commits that touch C#** (CleanArchitecture
2026-02-18 to 2026-10-12, eShop 2024-10-24 to 2026-10-15), each with
`check --base <commit>^ --worktree`:
- 172 and 173 changed C# files;
- no architecture finding. No commit adds a using these rules forbid,
  and the grep below finds none in either tree;
- every other finding is identical to qgate 0.5.0's on the same
  commits (an exclude-only configuration, JSON compared for all 60):
  - CleanArchitecture: 6 complexity increases, 14 duplicates and 4
    `input/unparsable`:
    - two `dotnet new` template files, whose `//#if` comments leave two
      declaration heads;
    - an `#if` whose branches close a lambda in a test factory, in two
      commits;
  - eShop: 19 complexity increases, 8 duplicates and 1
    `input/unparsable`, an `#if` inside a method chain in
    `MauiProgram.cs`.

  These are the known limits of 0.4.0's C# reading, unchanged.
- **Time per commit:** CleanArchitecture mean 0.08 s (max 0.12 s), eShop
  mean 0.16 s (max 0.28 s). 0.5.0 took 0.06 s and 0.17 s on the same
  commits. The first 0.6.0 pass took about 3 s a commit because the
  partial clone fetched blobs, and the numbers above are from the second
  pass.

**(B) Every using at HEAD,** with the HEAD tree as a child of an empty
commit:
- **CleanArchitecture:** exactly one finding,
  `src/Application/GlobalUsings.cs:4 uses Microsoft.EntityFrameworkCore`.
  `grep` for the forbidden names in `src/Domain` and `src/Application`
  finds that line and nothing else.
- **eShop:** no finding. `grep` finds no forbidden name in the three
  `from` directories, as text either.
- **Time:** 0.13 s and 0.59 s for the whole tree.

**What the trees say about the limits (D7, Risks):**
- **Global usings are where the dependencies live:** 25 of
  CleanArchitecture's 193 directives are global, as are 237 of eShop's
  762, nearly all in `GlobalUsings.cs` files. Nine application files
  of CleanArchitecture call EF Core methods with no using of their
  own. A pull request adding a tenth reports nothing, because the
  dependency entered once, in `GlobalUsings.cs`, as the README says.
- **Usings nested in a namespace block:** none in CleanArchitecture,
  and one in eShop (`src/OrderProcessor/.../GracePeriodConfirmedIntegrationEvent.cs:3`,
  `using eShop.EventBus.Events;`). That one is a full name, so it is
  read correctly. The limit of D7 does not show in either tree.

**A violation on real code** (`--worktree`, CleanArchitecture):
- `using CleanArchitecture.Infrastructure.Data;` added to
  `src/Domain/Entities/TodoItem.cs` gives `error
  src/Domain/Entities/TodoItem.cs:1  uses
  CleanArchitecture.Infrastructure.Data, forbidden by
  "domain-is-pure"` and exit code 1.
- A new domain file that writes `new
  CleanArchitecture.Infrastructure.Data.ApplicationDbContext(...)`
  without a using gives nothing, the documented miss.

**No finding the spec does not explain,** so nothing stops the release.

**To reproduce,** with the two repositories cloned (a partial clone,
`--filter=blob:none`, is enough):

`qgate.toml` for CleanArchitecture:

```toml
[files]
exclude = ["**/Migrations/**"]

[[architecture.forbid]]
name = "domain-is-pure"
from = ["src/Domain/**"]
to_namespaces = ["CleanArchitecture.Application", "CleanArchitecture.Infrastructure", "CleanArchitecture.Web", "Microsoft.EntityFrameworkCore"]

[[architecture.forbid]]
name = "application-has-no-infrastructure"
from = ["src/Application/**"]
to_namespaces = ["CleanArchitecture.Infrastructure", "CleanArchitecture.Web", "Microsoft.EntityFrameworkCore"]
```

`qgate.toml` for eShop:

```toml
[files]
exclude = ["**/Migrations/**"]

[[architecture.forbid]]
name = "ordering-domain-is-pure"
from = ["src/Ordering.Domain/**"]
to_namespaces = ["eShop.Ordering.Infrastructure", "eShop.Ordering.API", "Microsoft.EntityFrameworkCore"]

[[architecture.forbid]]
name = "ordering-infrastructure-not-api"
from = ["src/Ordering.Infrastructure/**"]
to_namespaces = ["eShop.Ordering.API"]

[[architecture.forbid]]
name = "catalog-not-ordering"
from = ["src/Catalog.API/**"]
to_namespaces = ["eShop.Ordering"]
```

(A) runs this loop with each file (and, for 0.5.0, with only the
`[files]` table):

```bash
#!/usr/bin/env bash
# Usage: per-commit.sh <repo> <qgate.toml> <qgate binary> <out dir>
set -u
repo=$1 config=$2 qgate=$3 out=$4
mkdir -p "$out"
wt="$out/wt"
git -C "$repo" worktree remove --force "$wt" 2>/dev/null
git -C "$repo" worktree add -q --detach "$wt" HEAD
for c in $(git -C "$repo" log -n 30 --no-merges --format=%H -- '*.cs'); do
  git -C "$wt" checkout -q --detach "$c"
  cp "$config" "$wt/qgate.toml"
  start=$(date +%s.%N)
  (cd "$wt" && "$qgate" check --base "$c^" --worktree --fail-on never --format json > "$out/$c.json" 2> "$out/$c.err")
  end=$(date +%s.%N)
  rm -f "$wt/qgate.toml"
  files=$(git -C "$repo" diff --name-only "$c^" "$c" -- '*.cs' | wc -l)
  printf '%s %s %s\n' "$c" "$files" "$(echo "$end - $start" | bc)" >> "$out/times.txt"
done
git -C "$repo" worktree remove --force "$wt"
```

(B) commits the HEAD tree onto an empty commit and checks it against
that commit, with the configuration in the tree:

```bash
empty=$(git commit-tree "$(git hash-object -t tree -w /dev/null)" -m empty)
# in a worktree of HEAD with qgate.toml added and committed:
whole=$(git commit-tree "HEAD^{tree}" -p "$empty" -m whole)
qgate check --base "$empty" --head "$whole" --fail-on never --format json
```

## Review of PR 1

The `reviewer` subagent mutated the adapter, the patterns, the
analyzer, the configuration and the CLI; every mutation but one was
killed. It also reproduced the evaluation numbers from the files above.
Fixed:
- **should-fix:** the evaluation could not be reproduced from this file;
  the configurations, the loop and the binary are now here;
- the error mapping of a list of namespace patterns was untested (the
  surviving mutation); a test with two patterns now names the bad one;
- a namespace pattern with whitespace or an `@` could never match and
  silently turned its rule off, against ADR 0003; both are now
  invalid (spec "Namespace patterns" amended);
- any alias qualifier other than `global::`, not only an extern alias,
  has no target, and so has an alias of a nullable type; the spec, the
  README and the Risks say so, with tests;
- a negated character class can match the separator (Risks);
- the proposal placed the evaluation record in PR 2, unlike D9;
- the TypeScript message is built only for the first import of a
  target again.

## Release 0.6.0 (task 2.1)

- PR #26 merged as fdcbadd. As Ervin agreed, the agent pushed the
  annotated tag `v0.6.0` ("qgate 0.6.0") after `check-version.sh` and
  `check-action-version.sh` accepted it on `main`.
- Release run 43523029663: all nine jobs green.
- The draft held six archives and `SHA256SUMS`; `sha256sum --check`
  passed for all six. The Linux arm64 archive holds the binary,
  `qgate-pr-comment.ps1` (byte for byte the one on `main`), `README.md`
  and both licenses. Its binary (11,211,840 bytes, the same size as in
  0.5.0) printed `qgate 0.6.0`. In a throwaway repository, on a domain
  file gaining `using Microsoft.EntityFrameworkCore;` under a rule with
  `to_namespaces`, it reported `src/Shop.Domain/Order.cs:2 uses
  Microsoft.EntityFrameworkCore, forbidden by "domain-is-pure"` and
  exited with 1.
- Archive sizes in bytes against 0.5.0; the change is the C# walk and
  the pattern code:

  | Archive | 0.5.0 | 0.6.0 |
  |---|---|---|
  | `x86_64-unknown-linux-musl.tar.gz` | 2,140,993 | 2,149,926 |
  | `aarch64-unknown-linux-musl.tar.gz` | 1,947,700 | 1,954,472 |
  | `aarch64-apple-darwin.tar.gz` | 1,945,945 | 1,953,299 |
  | `x86_64-apple-darwin.tar.gz` | 1,974,853 | 1,983,095 |
  | `x86_64-pc-windows-msvc.zip` | 1,933,168 | 1,939,601 |
  | `aarch64-pc-windows-msvc.zip` | 1,828,044 | 1,836,382 |

- Ervin published the draft on 2026-11-13 (23:01 UTC); `releases/latest`
  is `v0.6.0`.

## README at 0.6.0 (task 3.1)

Every install block, both `uses:` lines and the pinned commit
(`fdcbadd4bec9d6904c1ec4d857a4bc24398f40b0`, `v0.6.0^{commit}`) name
0.6.0, and `grep` finds no `0.5.0`. The roadmap's "Languages" row names
the C# architecture rules (0.6.0); the "Later" row no longer lists them.
The Linux install block, run as written against the published release
(with the arm64 target and a scratch directory instead of
`~/.local/bin`), passed its checksum and installed `qgate 0.6.0`.

## Azure Pipelines rehearsal (task 3.2, 2026-11-13)

- **Setup:** in the rehearsal project `qgate-rehearsal`, pipeline 36, a
  branch from `main` whose `azure-pipelines.yml` is the line `jobs:` and
  the README's Linux job at 0.6.0, cut out unchanged (`diff`:
  `0a1 > jobs:`). The branch adds:
  - `qgate.toml` with the README's rule: `from =
    ["src/Shop.Domain/**"]`, `to_namespaces = ["Shop.Infrastructure",
    "Microsoft.EntityFrameworkCore"]`;
  - `src/Shop.Domain/Order.cs`, with `using System;` and
    `using Microsoft.EntityFrameworkCore;`;
  - `src/Shop.Domain/GlobalUsings.cs`, with
    `global using System.Collections.Generic;` and
    `global using Shop.Infrastructure.Data;`;
  - `src/Shop.Infrastructure/ShopDb.cs`, which uses EF Core and the
    domain: outside `from`, so it must not be reported.

  Pull request 1359, abandoned afterwards, unmerged. The rehearsal
  `main` holds no C#, so the domain files are new; a using added to an
  existing file is covered by the CLI tests and the evaluation.
- **Deviation from D8:** D8 names one form, `using
  Shop.Infrastructure.Data;`. The rehearsal used two others, a
  `global using` of that namespace and a plain `using` of EF Core, so
  that one run covers both entries of `to_namespaces` and a second
  directive form. Task 3.2 asks for "a forbidden `using`", which both
  are.
- **Build 1340** on `ubuntu-latest` (image `ubuntu-26.04`, version
  20261111.196.1), reason `pullRequest`:
  - Install qgate (checksum OK), qgate report and qgate comment
    succeeded;
  - `qgate comment`: "created the qgate comment (2 findings, 2 at or
    above error)";
  - qgate check failed, as it should, with two build issues of code
    `architecture/forbidden-import`:
    - error at `src/Shop.Domain/GlobalUsings.cs` line 2, `uses
      Shop.Infrastructure.Data, forbidden by "domain-is-pure"`;
    - error at `src/Shop.Domain/Order.cs` line 2, `uses
      Microsoft.EntityFrameworkCore, forbidden by "domain-is-pure"`.
  - The job carried the hosted pool's warning that `ubuntu-latest`
    moves to Ubuntu 26 from 2026-10-19; this run already had it.
- **Thread 4561,** active, by "qgate-rehearsal Build Service
  (<organization>)", read through the REST API: "2 findings, 2 at or
  above `error`." and a table with exactly these two rows. `ShopDb.cs`
  and the `System` usings do not appear.

## Review of PR 2

The `reviewer` subagent checked the README versions and the pinned
commit against the tag, each archive size of both releases and the
release run with `gh`, the archive's script byte for byte, and the
rehearsal record for consistency with the README's job and rule (the
Azure facts need the private token). It found no blocker; its one nit,
the rehearsal using other forms than D8's example, is recorded above.

## Risks / Trade-offs

- **[A rule can be bypassed]** A fully qualified name in code, or a
  global using added by MSBuild, uses a namespace without a `using`
  line in the file. → qgate catches the common, accidental
  dependency, not a deliberate bypass. The README says so, and a
  later change can add qualified names in code (the spike shows the
  nodes are there).
- **[Global usings live in one file]** A `global using
  Shop.Infrastructure;` in `src/Shop.Domain/GlobalUsings.cs` affects
  the whole project but is reported only in that file. → That is the
  file that introduced the dependency, and it is inside the rule's
  `from`.
- **[A character class can match the separator]** `[.]` becomes `[/]`,
  and a negated class such as `[!x]` also matches the separator, so
  `Shop[!x]Data` matches `Shop.Data`, as the same class does in a path
  pattern. → Classes are rare in layer rules; `*` and `?` keep to one
  segment, which is what the README promises.
- **[More ways around a rule]** An alias of an array or nullable type
  (`using Rows = Shop.Infrastructure.Row[];`) and a name qualified by a
  using alias (`using S = Shop;` then `using S::Infrastructure.Data;`)
  have no target. → Syntax cannot tell a namespace alias from an extern
  alias, and both forms are rare; the README lists them with the other
  misses.
- **[Larger model]** Every C# file now carries its usings. → A few
  strings per file; the token list is far larger.
