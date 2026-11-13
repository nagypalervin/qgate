# architecture-analysis Specification

## Purpose

Architecture analysis keeps the boundaries of a codebase from eroding:
it reports the imports that a change newly introduces across a boundary
the repository forbids in `qgate.toml`.

## Requirements

### Requirement: Forbid rules

Each `[[architecture.forbid]]` table in `qgate.toml` SHALL define a rule
with a `name`, a list of `from` patterns, and a list of `to` patterns, a
list of `to_namespaces` patterns, or both. A file whose path matches a
`from` pattern SHALL NOT import a target that matches a `to` pattern, nor
use a namespace that matches a `to_namespaces` pattern.

#### Scenario: Domain importing the UI

- **GIVEN** the rule `name = "domain-is-pure"`, `from = ["src/domain/**"]`, `to = ["src/ui/**"]`
- **AND** the head version of `src/domain/order.ts` adds `import { Button } from "../ui/Button";` on line 3
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output contains `error   src/domain/order.ts:3  imports src/ui/Button ("../ui/Button"), forbidden by "domain-is-pure"  [architecture/forbidden-import]`
- **AND** the exit code is 1

#### Scenario: File outside from

- **GIVEN** the same rule
- **AND** `src/app/page.ts` adds `import { Button } from "../ui/Button";`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported

#### Scenario: Domain using the infrastructure

- **GIVEN** the rule `name = "domain-is-pure"`, `from = ["src/Shop.Domain/**"]`, `to_namespaces = ["Shop.Infrastructure"]`
- **AND** the head version of `src/Shop.Domain/Order.cs` adds `using Shop.Infrastructure.Data;` on line 2
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output contains `error   src/Shop.Domain/Order.cs:2  uses Shop.Infrastructure.Data, forbidden by "domain-is-pure"  [architecture/forbidden-import]`
- **AND** the exit code is 1

#### Scenario: Each list applies to its language

- **GIVEN** a rule `imports` with `from = ["src/**"]` and `to = ["Shop.Infrastructure"]`, and a rule `usings` with `from = ["src/**"]` and `to_namespaces = ["Shop.Infrastructure"]`
- **WHEN** `src/a.ts` adds `import "Shop.Infrastructure";` and `src/B.cs` adds `using Shop.Infrastructure;`
- **THEN** two findings are reported: the import of `src/a.ts`, forbidden by `imports`, and the using of `src/B.cs`, forbidden by `usings`

### Requirement: Imports that count

The system SHALL treat as imports of a file: `import` declarations,
including type-only and side-effect imports; `export ... from`
declarations; `import x = require(...)`; calls of `import(...)` whose
first argument is a string literal; and calls of `require(...)` whose only
argument is one; wherever they appear, type positions included. Other
forms, such as a template literal or an identifier argument, SHALL be
ignored.

#### Scenario: Every static form

- **GIVEN** a rule that forbids `src/domain/**` to import `src/ui/**`
- **WHEN** `src/domain/a.ts` adds `import type { T } from "../ui/t";`, `import "../ui/style";`, `export * from "../ui/all";`, `import m = require("../ui/m");`, `const d = await import("../ui/d");` and `const r = require("../ui/r");`
- **THEN** six `architecture/forbidden-import` findings are reported, for `src/ui/t`, `src/ui/style`, `src/ui/all`, `src/ui/m`, `src/ui/d` and `src/ui/r`

#### Scenario: Type position and import attributes

- **GIVEN** a rule that forbids `src/domain/**` to import `src/ui/**`
- **WHEN** `src/domain/a.ts` adds `let b: import("../ui/b").B;` and `const j = await import("../ui/j.json", { with: { type: "json" } });`
- **THEN** findings are reported for `src/ui/b` and `src/ui/j.json`

#### Scenario: Non-literal dynamic import

- **WHEN** `src/domain/a.ts` adds ``const l = await import(`../ui/${name}`);``
- **THEN** no `architecture/forbidden-import` finding is reported

### Requirement: Import targets

The target of a specifier that starts with `./` or `../` SHALL be the path
it names, resolved against the importing file's directory, normalized and
relative to the repository root; a specifier that leaves the repository
root SHALL be ignored. An empty specifier SHALL have no target; any other
specifier SHALL be its own target, as written. Extensions and `index`
files SHALL NOT be added.

#### Scenario: Relative specifier

- **WHEN** `src/domain/order.ts` imports `"./../ui/./Button"`
- **THEN** the target is `src/ui/Button`

#### Scenario: Leaving the repository

- **WHEN** `src/a.ts` imports `"../../outside"`
- **THEN** the import has no target and is never reported

#### Scenario: Package and alias

- **WHEN** a file imports `"react/jsx-runtime"` and `"@/ui/Button"`
- **THEN** the targets are `react/jsx-runtime` and `@/ui/Button`

### Requirement: Patterns

A pattern SHALL be a glob over `/`-separated paths, relative to the
repository root: `*` and `?` match within one segment, `**` matches any
number of segments, and `[...]` is a character class. A pattern without
glob characters SHALL also match everything below it, and a pattern ending
in `/**` SHALL also match the directory itself.

#### Scenario: Package name

- **GIVEN** `to = ["react"]`
- **WHEN** files import `"react"`, `"react/jsx-runtime"` and `"react-dom"`
- **THEN** the first two match and `react-dom` does not

#### Scenario: Index import of a directory

- **GIVEN** `to = ["src/ui/**"]`
- **WHEN** `src/domain/a.ts` imports `"../ui"`
- **THEN** the target `src/ui` matches

#### Scenario: Single segment wildcard

- **GIVEN** `from = ["src/*/model.ts"]`
- **THEN** `src/order/model.ts` matches and `src/order/sub/model.ts` does not

### Requirement: Pattern spelling

Once leading `./` are removed, a pattern that starts with `/` or has a
`..` segment SHALL be invalid. Empty and `.` segments and a trailing `/`
SHALL be ignored, except in a pattern with braces or a `\` escape, which
keeps its glob meaning and is not rewritten further.

#### Scenario: Equivalent spellings

- **GIVEN** a rule with `from = ["./src//domain/"]` and `to = ["src/ui/**"]`
- **WHEN** `src/domain/order.ts` adds an import of `"../ui/Button"`
- **THEN** the import is reported, as with `from = ["src/domain"]`

#### Scenario: Pattern from the file-system root

- **GIVEN** a rule with `to = ["/src/ui/**"]` in `qgate.toml`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names the rule and the pattern `/src/ui/**`
- **AND** the exit code is 2

### Requirement: Only new violations are reported

A violation is a file, a rule and a target. The system SHALL report a
violation of the head version of a changed file only if the base version
of that file did not have the same violation under the same rules. For a
renamed file the base version is checked at its old path; for a file
renamed from JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`) only the imports
of that version count; if it cannot be read, no import of the file is new,
wherever it moved. An added file has no base version.

#### Scenario: Existing violation

- **GIVEN** the base and head versions of `src/domain/order.ts` both import `"../ui/Button"`, and the head version changes something else
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported

#### Scenario: Moved within the domain

- **GIVEN** `src/domain/order.ts`, which imports `"../ui/Button"`, is renamed to `src/domain/sales/order.ts` and its import becomes `"../../ui/Button"`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported

#### Scenario: Moved into the domain

- **GIVEN** `src/app/order.ts`, which imports `"../ui/Button"`, is renamed to `src/domain/order.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** the import of `src/ui/Button` is reported

#### Scenario: Renamed from JavaScript

- **GIVEN** `src/domain/order.js`, which imports `"../ui/Button"`, is renamed to `src/domain/order.ts`, which also adds an import of `"../ui/Modal"`
- **WHEN** the user runs `qgate check --base main`
- **THEN** only the import of `src/ui/Modal` is reported

#### Scenario: Rule added in the same change

- **GIVEN** the base version of `src/domain/order.ts` imports `"../ui/A"`, and the head adds the forbid rule to `qgate.toml` and an import of `"../ui/B"` to the file
- **WHEN** the user runs `qgate check --base main`
- **THEN** only the import of `src/ui/B` is reported

#### Scenario: Existing using, written another way

- **GIVEN** a rule with `to_namespaces = ["Shop.Infrastructure"]`, and the base version of `src/Shop.Domain/Order.cs` has `using Shop.Infrastructure.Data;`
- **WHEN** the head version writes it as `global using global::Shop.Infrastructure.Data;` and changes something else
- **THEN** no `architecture/forbidden-import` finding is reported

### Requirement: Forbidden import finding

Each new violation SHALL be one finding with rule
`architecture/forbidden-import` and severity Error, at the head path of
the file and the line of its first import of the target. The message SHALL
be `imports <target>, forbidden by "<name>"`, with ` ("<specifier>")` after
the target when the specifier differs from it; for a C# file it SHALL be
`uses <target>, forbidden by "<name>"`.

#### Scenario: Package import

- **GIVEN** a rule named `no-db-in-ui` with `from = ["src/ui/**"]` and `to = ["@prisma/client"]`
- **WHEN** `src/ui/List.tsx` adds `import { db } from "@prisma/client";` on line 2
- **THEN** standard output contains `error   src/ui/List.tsx:2  imports @prisma/client, forbidden by "no-db-in-ui"  [architecture/forbidden-import]`

#### Scenario: Same target imported twice

- **WHEN** the head version of `src/domain/a.ts` imports `"../ui/Button"` on line 2 and again on line 9
- **THEN** one finding is reported, on line 2

#### Scenario: Two rules forbid the same import

- **GIVEN** two rules whose `from` and `to` both match the import of `src/ui/Button` by `src/domain/a.ts`
- **THEN** one finding is reported for each rule

#### Scenario: Same namespace used twice

- **GIVEN** a rule with `to_namespaces = ["Microsoft.EntityFrameworkCore"]`
- **WHEN** the head version of `src/Shop.Domain/Order.cs` adds `using Microsoft.EntityFrameworkCore;` on line 1 and `using M = Microsoft.EntityFrameworkCore;` on line 4
- **THEN** one finding is reported, `uses Microsoft.EntityFrameworkCore`, on line 1

### Requirement: Unparsable files have no architecture findings

A changed file that cannot be parsed in either version SHALL produce no
`architecture/forbidden-import` finding; it is reported as
`input/unparsable` (see `revision-input`).

#### Scenario: Syntax error in a file with a forbidden import

- **GIVEN** the head version of `src/domain/a.ts` imports `"../ui/Button"` and has a syntax error
- **WHEN** the user runs `qgate check --base main`
- **THEN** the only finding for `src/domain/a.ts` is `input/unparsable`

### Requirement: Usings that count

The system SHALL treat as uses of a C# file its `using` directives,
`global`, `static` and alias directives included, wherever they appear in
the file: at the top, inside a namespace, or in any branch of an `#if`.
`using` statements and declarations SHALL NOT count.

#### Scenario: Every directive form

- **GIVEN** a rule that forbids `src/Shop.Domain/**` to use `Shop.Infrastructure`
- **WHEN** `src/Shop.Domain/A.cs` adds `using Shop.Infrastructure.A;`, `global using Shop.Infrastructure.B;`, `using static Shop.Infrastructure.C;` and `using D = Shop.Infrastructure.D;`
- **THEN** four `architecture/forbidden-import` findings are reported, for `Shop.Infrastructure.A`, `Shop.Infrastructure.B`, `Shop.Infrastructure.C` and `Shop.Infrastructure.D`

#### Scenario: Inside a namespace and an `#if`

- **GIVEN** the same rule
- **WHEN** `src/Shop.Domain/A.cs` adds `using Shop.Infrastructure.Debug;` between `#if DEBUG` and `#endif`, and `using Shop.Infrastructure.Data;` inside the body of `namespace Shop.Domain { ... }`
- **THEN** findings are reported for `Shop.Infrastructure.Debug` and `Shop.Infrastructure.Data`

#### Scenario: Using statements

- **GIVEN** the same rule
- **WHEN** a method in `src/Shop.Domain/A.cs` adds `using var db = new Shop.Infrastructure.Db();` and `using (db) { }`
- **THEN** no `architecture/forbidden-import` finding is reported

### Requirement: Using targets

The target of a `using` directive SHALL be the namespace or type it
names as written, without `global::`, without the `@` of verbatim
identifiers, without whitespace or comments, and without type arguments.
It SHALL NOT be resolved against an enclosing namespace. A directive
whose alias names a tuple, array, pointer, nullable or keyword type, or
whose name is qualified by an alias other than `global` (`X::`), SHALL
have no target.

#### Scenario: Normalized names

- **WHEN** a C# file has `using global::Shop.Data;`, `using @class.Foo;` and `using L = System.Collections.Generic.List<Shop.Data.Row>;`
- **THEN** the targets are `Shop.Data`, `class.Foo` and `System.Collections.Generic.List`

#### Scenario: No target

- **WHEN** a C# file has `using P = (int X, int Y);`, `using unsafe Ptr = int*;`, `using N = Shop.Data.Row?;` and `using Legacy::Shop.Data;`
- **THEN** none of them has a target

#### Scenario: As written inside a namespace

- **GIVEN** a rule with `to_namespaces = ["Shop.Infrastructure"]`
- **WHEN** `src/Shop.Domain/A.cs` adds `using Infrastructure;` inside `namespace Shop { ... }`
- **THEN** the target is `Infrastructure` and no finding is reported

### Requirement: Namespace patterns

A `to_namespaces` pattern SHALL be a glob over `.`-separated names: `*`
and `?` match within one segment, `**` matches any number of segments,
and `[...]` is a character class. A pattern without glob characters SHALL
also match every name below it, and a pattern ending in `.**` SHALL also
match the name itself. A pattern that is empty, has an empty segment, or
contains `/`, whitespace or `@` SHALL be invalid.

#### Scenario: A namespace and the names below it

- **GIVEN** `to_namespaces = ["Shop.Infrastructure"]`
- **WHEN** files use `Shop.Infrastructure`, `Shop.Infrastructure.Data.Db` and `Shop.InfrastructureTests`
- **THEN** the first two match and `Shop.InfrastructureTests` does not

#### Scenario: Any namespace ending in a segment

- **GIVEN** `to_namespaces = ["**.Data"]`
- **THEN** `Shop.Data` and `Shop.Orders.Data` match, and `Shop.Data.Rows` and `Shop.DataAccess` do not

#### Scenario: Empty segment

- **GIVEN** a rule in `qgate.toml` with `to_namespaces = ["Shop..Infrastructure"]`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard error names the rule and the pattern `Shop..Infrastructure`
- **AND** the exit code is 2
