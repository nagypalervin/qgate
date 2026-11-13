# Proposal: add-csharp-architecture

## Why

The partner pilot runs on an Angular frontend and a .NET backend. In a
layered .NET backend the rule that matters most is the dependency
direction between layers: the domain must not use the infrastructure
or Entity Framework Core. qgate checks architecture rules only for
TypeScript imports today, and the README says C# `using` directives
are not checked. Ervin decided on 2026-11-13 that the pilot needs
these rules, before M5, so they leave the roadmap's "Later" row.

## What Changes

- **C# `using` directives count as uses of a namespace.** These forms
  name a target:
  - `using A.B;` and `global using A.B;`;
  - `using static A.B.C;`;
  - `using X = A.B.C;`.

  The target is the name as written, normalized:
  - without `global::`, `@` prefixes and type arguments;
  - so `using L = global::Shop.Data.List<int>;` names `Shop.Data.List`.

  These have no target:
  - an alias of a tuple, array, pointer or built-in type;
  - an extern-alias name such as `Legacy::Shop.Data`.

  `using` statements and declarations (`using var x = ...`) are not
  directives and do not count.
- **A forbid rule gets `to_namespaces`,** a list of namespace patterns
  next to `to`:

  ```toml
  [[architecture.forbid]]
  name = "domain-is-pure"
  from = ["src/Shop.Domain/**"]
  to_namespaces = ["Shop.Infrastructure", "Microsoft.EntityFrameworkCore"]
  ```

  - `from` stays a path pattern and matches the C# file's path.
  - `to` keeps matching TypeScript import targets; `to_namespaces`
    matches C# `using` targets.
  - A rule needs at least one of the two.
- **Namespace patterns** are globs over `.`-separated names, mirroring
  path patterns:
  - `*` stays within one segment, and `**` spans segments;
  - a name without glob characters also matches every namespace below
    it, so `Shop.Infrastructure` matches `Shop.Infrastructure.Data`
    but not `Shop.InfrastructureTests`.

  An empty segment, a leading or trailing `.`, or a `/` makes a
  pattern invalid.
- **Findings and their timing work as for TypeScript:**
  - only new violations are reported, judged against the file's base
    version at its old path;
  - each finding is an Error with the same rule id,
    `architecture/forbidden-import`, and the message `uses <namespace>,
    forbidden by "<name>"`;
  - generated and excluded C# files take no part, as for every
    analyzer.
- **README:** the architecture section and the C# section show C#
  rules. The roadmap's "Later" row loses the item, and the "Languages"
  row names the C# rules.
- **Version 0.6.0,** including `action.yml`.

### Out of scope

- **Fully qualified names in code** (`new Shop.Infrastructure.Db()`).
  They are written without a `using`, and telling a namespace from a
  type or member needs semantic analysis.
- **Usings that MSBuild adds:**
  - `ImplicitUsings`;
  - `<Using Include="..."/>` in a `.csproj`.
- **Project references between `.csproj` files.**
- **Resolving a `using` relative to its enclosing namespace.**
  Inside `namespace Shop { using Infrastructure; }`, C# can resolve
  the name to `Shop.Infrastructure`, but qgate takes it as written:
  `Infrastructure`.
- **Matching by the namespace a file declares** rather than its path.
- **Rules for Razor, VB.NET or F#.**

### For Gate 1

Each question has a recommendation; design.md has the reasons.

1. **Where namespace patterns go.** Options:
   - (a) a `to_namespaces` key in the existing `[[architecture.forbid]]`
     table;
   - (b) a separate table, `[[architecture.forbid-namespace]]`;
   - (c) the existing `to`, with dots.

   *Recommended: (a).*
2. **The rule id.** Options:
   - keep `architecture/forbidden-import` for C#, with the message
     `uses <namespace>, ...`;
   - add a new id, `architecture/forbidden-using`.

   *Recommended: keep the id.* A `using` is C#'s import, and existing
   filters and severities keep working.
3. **Nested usings are taken as written** (see "Out of scope"), and
   the README states the limit. *Recommended: yes.* The alternative,
   checking every enclosing-namespace reading, would report namespaces
   that may not exist.
4. **Evaluation before the release.** Run realistic layer rules over
   the last 30 C# commits of `dotnet/eShop` and
   `jasontaylordev/CleanArchitecture`, as for C# analysis, and check
   every finding by hand. *Recommended: yes.*
5. **The rehearsal: Azure Pipelines,** the partner's CI. Use the
   README's Linux job at `0.6.0` in the rehearsal project, on a pull
   request whose domain file adds a forbidden `using`. *Recommended:
   yes.*
6. **Two pull requests with 0.6.0 between them,** as before. PR 1 is
   the rules, the README section and the version. PR 2 is the README
   versions, the roadmap, the evaluation record, the rehearsal and the
   archive. *Recommended: yes.*

**Decided at Gate 1 (2026-11-13):** Ervin approved the proposal with
every recommendation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `architecture-analysis`: forbid rules gain `to_namespaces`. New
  requirements cover the C# usings that count, their targets and
  namespace patterns. The finding's message gets the C# form.
- `configuration`: a rule needs `to` or `to_namespaces`, and invalid
  namespace patterns are configuration errors.

## Impact

- **Code:**
  - `crates/qgate-lang`: the C# adapter lists `using` targets in the
    file model;
  - `crates/qgate-config`: the `to_namespaces` key;
  - `crates/qgate-analyzers`: namespace patterns, built on
    `PathPatterns`, and C# targets in the architecture analyzer;
  - `crates/qgate-cli`: copying `to_namespaces` into the analyzer's
    rules, and end-to-end tests. The analyzer already runs on every
    changed file that is supported and not excluded, C# included.
- **No new dependency.**
- **Docs:** the README's architecture, C# and roadmap sections. The
  JSON schema is unchanged: same rule id, same finding shape.
