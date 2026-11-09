# Spec Delta

## ADDED Requirements

### Requirement: Forbid rules

Each `[[architecture.forbid]]` table in `qgate.toml` SHALL define a rule
with a `name`, a list of `from` patterns and a list of `to` patterns. A
file whose path matches a `from` pattern SHALL NOT import a target that
matches a `to` pattern.

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
root SHALL be ignored. Any other specifier SHALL be its own target, as
written. Extensions and `index` files SHALL NOT be added.

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
in `/**` SHALL also match the directory itself. A leading `./` and a
trailing `/` SHALL be ignored; a pattern starting with `/` is invalid.

#### Scenario: Package name

- **GIVEN** `to = ["react"]`
- **WHEN** files import `"react"`, `"react/jsx-runtime"` and `"react-dom"`
- **THEN** the first two match and `react-dom` does not

#### Scenario: Index import of a directory

- **GIVEN** `to = ["src/ui/**"]`
- **WHEN** `src/domain/a.ts` imports `"../ui"`
- **THEN** the target `src/ui` matches

#### Scenario: Equivalent spellings

- **GIVEN** `from = ["./src/domain/"]`
- **THEN** it matches `src/domain/order.ts`, like `from = ["src/domain"]`

#### Scenario: Single segment wildcard

- **GIVEN** `from = ["src/*/model.ts"]`
- **THEN** `src/order/model.ts` matches and `src/order/sub/model.ts` does not

### Requirement: Only new violations are reported

A violation is a file, a rule and a target. The system SHALL report a
violation of the head version of a changed file only if the base version
of that file did not have the same violation under the same rules. For a
renamed file the base version is checked at its old path; for a file
renamed from JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`) only the imports
of that version count. An added file has no base version.

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

### Requirement: Forbidden import finding

Each new violation SHALL be one finding with rule
`architecture/forbidden-import` and severity Error, at the head path of
the file and the line of its first import of the target. The message SHALL
be `imports <target>, forbidden by "<name>"`, with ` ("<specifier>")` after
the target when the specifier differs from it.

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

### Requirement: Unparsable files have no architecture findings

A changed file that cannot be parsed in either version SHALL produce no
`architecture/forbidden-import` finding; it is reported as
`input/unparsable` (see `revision-input`).

#### Scenario: Syntax error in a file with a forbidden import

- **GIVEN** the head version of `src/domain/a.ts` imports `"../ui/Button"` and has a syntax error
- **WHEN** the user runs `qgate check --base main`
- **THEN** the only finding for `src/domain/a.ts` is `input/unparsable`
