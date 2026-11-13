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
  - Telling "missing" from "empty" needs `Option<Vec<String>>` in the
    serde layer. The public field can stay a plain `Vec`, filled after
    validation.
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
- **[A `.` in a character class]** `[.]` is translated to `[/]`.
  → A namespace segment cannot contain `.`, so such a class can only
  match a separator, which is what the translated class does.
- **[Larger model]** Every C# file now carries its usings. → A few
  strings per file; the token list is far larger.
