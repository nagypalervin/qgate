# Design: add-csharp-analysis

## Context

- `qgate-lang` turns one file's text into a `FileModel` (units with
  their complexity, imports, normalized tokens). `typescript.rs` walks
  the tree-sitter tree once, with a stack of named frames, and the
  analyzers never see syntax. ADR 0001 expects each new language to be
  "an adapter and fixtures, but no analyzer changes".
- `qgate-cli` picks files by extension (`language_of`), parses both
  versions of a changed file (`file_change`, `parse_blob`), and feeds one
  `DuplicationIndex` with the changed files and every other supported
  file of the head tree.
- The spike (2026-11-13, `tree-sitter` 0.26 with `tree-sitter-c-sharp`
  0.23.5) parsed C# 12 code without errors: file-scoped namespaces,
  primary constructors, records, `required` and `init`, raw strings,
  collection expressions, list and relational patterns, `file` classes,
  local functions, operators, indexers and finalizers. A UTF-8 byte order
  mark, CRLF endings and simple `#if`, `#region` and `#pragma` lines
  parsed too. An `#if`/`#else` whose branches each open a block was a
  syntax error. The grammar depends only on `tree-sitter-language`, as
  the TypeScript grammar does.

## Goals / Non-Goals

**Goals:**
- C# through the same `FileModel`, with no change to the complexity or
  duplication analyzers.
- Names that stay stable across the edits C# code sees most: overloads
  added, members reordered, files moved between namespaces.

**Non-Goals:**
- Imports for C#: `FileModel::imports` stays empty for a `.cs` file.
- Semantic analysis: no symbol resolution and no project files, so qgate
  stays a per-file parser that needs no build.

## Decisions

### D1. One adapter module, the TypeScript walker's shape

`crates/qgate-lang/src/csharp.rs` with `Language::CSharp`
(`from_extension("cs")`). Like `typescript.rs`, it is one walk with a
frame stack:
- **Types push their name:** class, struct, record, interface (default
  interface methods have bodies).
- **Units push a frame:**
  - `method_declaration`, `constructor_declaration`,
    `destructor_declaration`, `operator_declaration`,
    `conversion_operator_declaration` and `local_function_statement`
    with a `block` or `arrow_expression_clause`;
  - an `accessor_declaration` with a body;
  - a property or indexer whose own `arrow_expression_clause` makes it
    a getter (`.get`).
- **Not units:** namespaces push nothing; lambdas and anonymous methods
  push no frame.
- **Code outside units:** field initializers and top-level statements
  count towards no unit.
- **Syntax errors:** the first error line comes from the same
  `ERROR`/missing-node search as for TypeScript.

Shared helpers (`line_of`, the error search, name de-duplication with
`#2`) move to a small module that both adapters use, instead of being
copied.

*Alternative:* a generic, table-driven adapter for both languages. The
TypeScript rules (bound calls, type wrappers) do not fit a table, and
the gain for two languages is small.

### D2. Names: types, then the member with its signature

- **Path:** enclosing types and units joined by `.`; namespaces left
  out, so moving a file to another namespace keeps every name.
- **Generic types** keep their type parameter list as written:
  `Result<T>.Ok()` next to `Result.Ok()`. Without it the two would get
  `#2` by source order (review of PR 1). Renaming `T` renames the
  type's units, a rare edit accepted like a parameter type change.
- **Method-like units:** the type parameters and parameter list are
  part of their own segment: `Map<T>(ref T, params int[])`.
- **Parameter list:** each parameter's modifiers (`ref`, `out`, `in`,
  `params`, `this`, `scoped`) and type as written, joined by `, `.
  Whitespace is removed except one space between two words, so
  `Dictionary<string, int>` becomes `Dictionary<string,int>` and
  `scoped Span<int>` keeps its space. (Task 1.4: the grammar parses
  `scoped` as part of the type, so "one space after each modifier", the
  first wording, would have glued it to the type. The spec says "between
  two words" now.)
- **`params`:** the grammar puts a `params` parameter's keyword and type
  directly in the parameter list, not in a `parameter` node; the name
  builder reads them from there.
- **Special members:**
  - constructors `Cart(IClock)`; finalizers `~Cart()`;
  - operators `operator +(Inner, Inner)`, and `operator checked +` for a
    checked one; conversions `implicit operator int(K)`;
  - indexers `this[int]`;
  - an explicit interface implementation of any member keeps its
    interface before its name: `IDisposable.Dispose()`, `I.P.get`,
    `I.this[int].get`, `IAdd<C>.operator +(C, C)` (review of PR 1: first
    only methods did).
- **Accessors:** `.get`, `.set`, `.init`, `.add`, `.remove` after the
  property, indexer or event.
- **Collisions:** the existing `#2` rule handles any that remain, such
  as a static and an instance parameterless constructor, or two partial
  declarations of one class with the same member in one file.

*Why signatures (Gate 1, question 1):*
- **Overloads are common in C#.** TypeScript's `#2` by source order
  would rename an overload whenever one is added above it. qgate would
  then compare unrelated bodies and report increases and decreases that
  are not there.
- **The cost:** changing a parameter's type is a rename (the unit is
  "new"), the same trade-off as renaming a method today.

*Alternative:* signatures only when a name collides. Adding a second
overload would then rename the first, which is the instability this
avoids.

### D3. Decision points

Counted node kinds:
- **Statements:** `if_statement`, `for_statement`, `foreach_statement`,
  `while_statement`, `do_statement`, `catch_clause`.
- **Switch:**
  - each `case` label of a `switch_section`, counted as `case` tokens,
    so `case 1: case 2:` counts twice;
  - a `switch_expression_arm` unless its pattern is the discard `_`;
  - `when_clause` (a case guard) and `catch_filter_clause` (a catch's
    `when`).
- **Expressions:** `conditional_expression`; `binary_expression` with
  `&&`, `||` or `??`; `assignment_expression` with `??=`;
  `conditional_access_expression` (both `?.` and `?[]`).
- **Patterns:** `and_pattern` and `or_pattern`.

*Choices (Gate 1, question 3):*
- **No optional parameter values.** TypeScript counts them because
  ESLint does: a JavaScript default is evaluated in the function. A C#
  default is a constant the caller passes, so it is not a branch of the
  method.
- **`and` and `or` patterns count** as the `&&` and `||` they replace:
  `x is > 0 and < 50` and `x > 0 && x < 50` measure the same.
- **Not counted:** `default` labels and `_` arms, as `else` and
  TypeScript's `default` are not; and `goto`, `break` and `continue`,
  which ESLint does not count either.

### D4. Tokens

- **Placeholders:**
  - an `identifier` becomes `$id`, except the name of a
    `member_access_expression` or `member_binding_expression`, which
    keeps its text (as a TypeScript member name does), also inside a
    `generic_name` (`x.Cast<int>()`; review of PR 1);
  - a literal node becomes one `$lit` without descending: string,
    verbatim and raw string, character, integer and real;
  - in an interpolated string, a run of text parts and escapes becomes
    one `$lit`, as a template's run does in TypeScript; a format
    (`{x:D2}`) becomes one `$lit`; the interpolations are tokens as
    usual.
- **Kept text:** keywords, `predefined_type` (`int`, `string`), operators
  and punctuation.
- **No tokens:** `using_directive`, the namespace's keyword and name
  (a file-scoped declaration as a whole), preprocessor directives,
  comments and `;`. (Task 1.6: the spec first named only the
  namespace's name, which left a lone `namespace` token in every
  file-scoped file.)
- **`#if` branches hold code.** The grammar nests the code of an `#if`,
  `#elif` or `#else` branch inside the directive's node. Only the
  directive tokens and the `condition` are muted, and decision points in
  muted code do not count, so `#if A && B` adds nothing (review of PR 1:
  first every `preproc_` subtree was muted, which hid the code).
- **Contextual keywords:** the grammar parses `when`, `value` and the
  like as an `identifier` with a keyword inside, so an `identifier` node
  is one token and its children are not entered.
- **Types are code in C#.** Removing them as TypeScript does would remove
  half of every declaration; generic arguments become `$id` tokens like
  any name, so a renamed copy with renamed types still matches.

### D5. Generated files, in `qgate-lang`

- **By name:** `qgate_lang::csharp_generated_name(file_name)`. A name
  ending (case-insensitive) in `.designer.cs`, `.generated.cs`, `.g.cs`
  or `.g.i.cs`, or starting with `TemporaryGeneratedFile_`.
- **By header:** `qgate_lang::csharp_generated_header(source)`, a text
  scan, not a parse, so a generated file that does not parse is still
  recognized:
  - skip a byte order mark, whitespace and preprocessor lines (any line
    starting with `#`);
  - read `//` and `/* */` comments until the first other text;
  - true if one contains `<auto-generated` or `<autogenerated`, compared
    case-sensitively.
- **Source:** this is Roslyn's convention for generated code
  (`GeneratedCodeUtilities`).
- **Checked against Roslyn (task 1.1):** `GeneratedCodeUtilities.cs` in
  `dotnet/roslyn`, `src/Compilers/Core/Portable/SourceGeneration/`, last
  changed in fa098b0471 (2024-06-28).
  - **By name:** a file name starting with `TemporaryGeneratedFile_`, or
    whose name without its extension ends in `.designer`, `.generated`,
    `.g` or `.g.i`, all ignoring case. For a `.cs` file that is the
    list in the spec.
  - **By header:** a comment in the leading trivia of the file's first
    token containing `<autogenerated` or `<auto-generated` (ordinal, so
    case-sensitive). Leading trivia includes preprocessor directives,
    so a comment after `#nullable disable` counts; hence "any line
    starting with `#`" above rather than only `#pragma` and `#nullable`.
  - **Not adopted:** Roslyn also honors `generated_code = true` in
    `.editorconfig` and the `[GeneratedCode]` attribute on a symbol.
    qgate reads neither (no editorconfig parsing, no symbols); a
    follow-up could add the first.
- **CLI, by name:** a file is skipped before it is read: changed files,
  the corpus and the base side.
- **CLI, by header:** checked on the head version of a changed file and
  on each corpus file; a skipped file is neither analyzed nor indexed.
- **Two-file form:** analyzes what it is given; skipping is a
  revision-mode rule.
- **Why in `qgate-lang`:** it is knowledge about C#, and the CLI stays
  thin.

### D6. One duplication index per language family

- **Families:** TypeScript (with TSX and JavaScript renamed or deleted)
  and C#.
- **CLI:** builds one `DuplicationIndex` per family that has a changed
  file, feeds each with its own corpus files, and concatenates the
  findings, which the report sorts as before. The analyzer is unchanged.
- **Why:** tokens of two languages can coincide (`$id ( $id ) { return
  $id . ...`), and a TypeScript file can never reuse a C# copy anyway.

### D7. The two-file form

`Language` gets a family check. Two `.cs` files are compared like two
TypeScript files; a `.cs` file with a TypeScript one is an error naming
both, exit code 2, since unit names from two languages can never match.

### D8. Evaluation before the release (Gate 1, question 5)

- **Command:** a scratch script runs the release-candidate binary over
  the last 30 merge commits of two public C# repositories:
  `qgate check --base <merge>^1 --head <merge>`.
- **Recorded in this design:**
  - files analyzed and skipped as generated;
  - `input/unparsable` findings and their causes;
  - findings by rule, with a hand check of ten of them;
  - run time.
- **Outcome:** a cause that affects many files (for example `#if`) is
  fixed or turned into a follow-up before 0.4.0, Ervin decides which.

### D9. Three pull requests and 0.4.0

1. **PR 1, `change/add-csharp-analysis`:** the adapter, its fixtures and
   unit tests, the shared helpers, the dependency.
2. **PR 2, `change/add-csharp-analysis-2`:**
   - the CLI (extensions, generated files, index per family, two-file
     check), the analyzer fixture pairs and CLI tests;
   - the evaluation (D8), the README;
   - version 0.4.0 in `Cargo.toml` and `action.yml`.
3. **Release:** Ervin agrees, and the agent pushes `v0.4.0`; Ervin
   publishes.
4. **PR 3, `change/add-csharp-analysis-3`:**
   - every README version at 0.4.0;
   - the release record with the binary sizes;
   - one Azure Pipelines run with a C# pull request on a Windows agent
     (Gate 1, question 6);
   - the journal entry and the archive.

## Findings of PR 1 (tasks 1.4 to 1.8)

- **Binary size (task 1.8).** A release build (`strip = true`, aarch64
  Linux, rustc 1.99.1) of `qgate-cli`: 6,035,032 bytes before the
  grammar (0cc7f0c), 11,408,984 bytes with it (+89%). `gzip -9` gives
  1,559,258 and 1,884,075 bytes (+21%), which is closer to what a
  release archive adds to a download. Accepted; feature-gating languages
  (ADR 0001) stays the fallback if size starts to matter.
- **A syntax error the shared error search missed (task 1.8).** For
  `x = ;` the C# grammar inserts a zero-width `identifier` whose
  `has_error` is true but whose `is_missing` and `is_error` are false,
  so the file passed as valid. The search in `common.rs` now also
  counts a node that has an error but no child with one. TypeScript's
  tests pass unchanged; the change can only turn a file that wrongly
  passed into `input/unparsable`.
- **The line of an unbalanced block.** When an `#if`/`#else` leaves a
  block open, the error node covers the whole enclosing namespace, so
  the reported line is the namespace's (line 3 of
  `fixtures/csharp/syntax_error.cs`), not the `#if`'s. The message
  already says "or syntax the parser does not support yet"; the
  evaluation (D8) shows whether a better line is worth a change.

## Review of PR 1

The `reviewer` subagent compared TypeScript's `parse()` before and after
the refactor on 3,976 inputs (every fixture, mutations and broken
snippets) without a difference, and asked for:
- **Blocker:** code in `#if` branches had no tokens → D4.
- **`#if` conditions counted** `&&` and `||` → D4.
- **Generic member names** (`x.Cast<int>()`) became `$id` → D4.
- **Explicit implementations of properties, indexers, events and
  operators** lost their interface → D2.
- **`Result` and `Result<T>`** in one file collided → D2.
- **The "Conditional compilation" requirement promised too much:** the
  grammar also fails on an `#if` inside an expression, an argument
  list, a collection initializer or a base list, around a modifier, and
  around an `else if`. The spec now says that a file the parser cannot
  read with its directives in place is unparsable, and Ervin was told
  that Gate 1 question 4 covers more than blocks; the evaluation counts
  these files.
- **Token wording:** "each run" of interpolated text and formats
  recorded in the spec and D4.
- **Nits:** a dead `record_struct_declaration`, `operator checked`,
  anonymous methods untested, crate docs naming TypeScript only, no test
  for the CLI guard, a non-exhaustive grammar choice, a parent call the
  comments said was avoided, and the dependency claim: besides
  `tree-sitter-language`, the grammar has the build dependency `cc`,
  which `tree-sitter-typescript` already brings.

## Risks / Trade-offs

- **[`#if` the grammar cannot place]** Branches that each open a block,
  and an `#if` inside an expression, an argument list, an initializer
  or a base list, around a modifier or around an `else if`: such files
  get `input/unparsable`. → The evaluation counts them; tree-sitter
  cannot pick a branch, and a preprocessor pass is a change of its own.
- **[Generated files outside the convention]** For example EF Core
  migrations themselves, which have no header and may repeat
  `CreateTable` blocks. → The evaluation shows whether they report
  duplicates; an exclude list in `qgate.toml` would be the follow-up.
- **[Grammar lag]** C# 13 and 14 syntax newer than the grammar (0.23.5,
  2025) is a syntax error, reported as "syntax the parser does not
  support yet". → The message already says so; the grammar is updated
  like the TypeScript one.
- **[Binary size]** The C# parse tables add to every binary. → Measured
  in PR 1 and recorded; ADR 0001 already names feature-gating languages
  as the fallback.
- **[UTF-16 files]** Older Visual Studio projects can hold UTF-16 files,
  which are "not valid UTF-8" (`input/unparsable`). → Accepted; the
  evaluation shows how rare they are.
- **[Signature names in messages]** `Outer.Inner.Map<T>(ref T, params
  int[])` is long in a PR comment row. → Accepted for exactness; the
  Markdown cell escapes `<`, `>` and `[` as it does today.
