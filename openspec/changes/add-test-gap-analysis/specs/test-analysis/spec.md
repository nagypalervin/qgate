# Spec Delta

## Purpose

Test analysis tells a reviewer which changed code arrived without a
change to its tests, by pairing each changed source file with its test
files through naming conventions and reporting code that changed while
its tests did not.

## ADDED Requirements

### Requirement: Checked changes

In revision mode the system SHALL check every changed source file whose
head version is supported, not excluded by `[files] exclude`, not
generated C#, and parses. Deleted files SHALL NOT be checked. The
two-file form SHALL NOT check tests.

#### Scenario: Two-file form

- **WHEN** the user runs `qgate check before.ts after.ts` where `after.ts` adds a function and no `after.spec.ts` exists
- **THEN** no `tests/` finding is reported

#### Scenario: Unparsable source file

- **GIVEN** the change edits `src/app/cart.service.ts`, which has `cart.service.spec.ts` beside it, and leaves a syntax error in it
- **WHEN** the user runs `qgate check --base main`
- **THEN** an `input/unparsable` finding is reported for it, and no `tests/` finding

#### Scenario: Working tree

- **GIVEN** an uncommitted edit changes the body of `Cart.total` in `src/app/cart.service.ts`, and `src/app/cart.service.spec.ts` is unchanged
- **WHEN** the user runs `qgate check --base main --worktree`
- **THEN** a `tests/unchanged-test` finding is reported for `src/app/cart.service.ts`

### Requirement: Changed units

A changed unit SHALL be a function unit of the head version that has no
unit of the same qualified name in the base version (at its old path if
renamed), or whose code differs from it. Code SHALL be compared without
whitespace, comments, semicolons and TypeScript types, and without
nested named units, whose change is theirs only. A file with no
changed unit SHALL have no `tests/` finding.

#### Scenario: Body changed

- **GIVEN** the change replaces `return a + b;` with `return a + b + 1;` in `Cart.total`
- **THEN** `Cart.total` is a changed unit

#### Scenario: Only layout and comments

- **GIVEN** the change reformats `Cart.total` and adds a comment inside it
- **THEN** `Cart.total` is not a changed unit

#### Scenario: Only a TypeScript type annotation

- **GIVEN** the change replaces `(a: number)` with `(a: string)` in the parameters of `Cart.total`
- **THEN** `Cart.total` is not a changed unit

#### Scenario: Only imports and types

- **GIVEN** the change reorders the imports of `src/app/cart.service.ts` and adds an interface to it
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported for it

#### Scenario: Removed function

- **GIVEN** the change deletes the method `Cart.clear` and changes nothing else
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: Renamed without edits

- **GIVEN** the change renames `src/app/cart.service.ts` to `src/app/basket.service.ts` without other edits
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: A C# overload

- **GIVEN** the change adds `Total(int)` next to the existing `Cart.Total()` in `Cart.cs`
- **THEN** `Cart.Total(int)` is a changed unit and `Cart.Total()` is not

### Requirement: TypeScript tests

A TypeScript file whose name ends in `.spec.<e>` or `.test.<e>`, where
`<e>` is `ts`, `mts`, `cts` or `tsx`, SHALL be a test, and never a
source file. The tests of a source file `<dir>/<stem>.<ext>` SHALL be
the files `<dir>/<stem>.spec.<e>` and `<dir>/<stem>.test.<e>`, for any
such `<e>`, that exist in the head revision.

#### Scenario: Angular component

- **GIVEN** the head revision has `src/app/cart/cart.component.ts` and `src/app/cart/cart.component.spec.ts`
- **THEN** `cart.component.spec.ts` is the test of `cart.component.ts`

#### Scenario: A changed spec alone

- **GIVEN** the change edits only `src/app/cart/cart.component.spec.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: An ES module source

- **GIVEN** the head revision has `src/order.mts` and `src/order.spec.ts`
- **THEN** `order.spec.ts` is the test of `order.mts`, and `src/order.spec.mts` would be a test too, never a source

#### Scenario: A test in another directory

- **GIVEN** `src/app/cart.service.ts` has no test beside it, and `test/cart.service.spec.ts` exists
- **THEN** `src/app/cart.service.ts` has no test

### Requirement: C# projects

A C# file SHALL belong to the project of the nearest `.csproj` file in
its own directory or above it, named by that file's name without
`.csproj`. A project SHALL be a test project when the last
`.`-separated segment of its name ends in `Test` or `Tests`. Every `.cs`
file of a test project SHALL be a test, and never a source file. A C#
file that belongs to no project SHALL have no tests.

#### Scenario: Test project names

- **GIVEN** projects named `Core.Test`, `Shop.Domain.UnitTests`, `Application.FunctionalTests` and `Shop.Testing`
- **THEN** the first three are test projects and `Shop.Testing` is not

#### Scenario: A changed test project alone

- **GIVEN** the change edits only `tests/Domain.UnitTests/OrderTests.cs`, in the test project `Domain.UnitTests`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: A file outside any project

- **GIVEN** no `.csproj` exists in the directory of `scripts/Build.cs` or above it, and the change adds a method to it
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

### Requirement: C# tests

The test projects of a project `P` SHALL be the test projects named
`P.<X>`, where `<X>` is one segment. The tests of a source file
`<Stem>.cs` of project `P` SHALL be the files named `<Stem>Tests.cs` or
`<Stem>Test.cs`, in any directory of a test project of `P`, that exist
in the head revision.

#### Scenario: Clean Architecture layout

- **GIVEN** `src/Domain/Domain.csproj`, `tests/Domain.UnitTests/Domain.UnitTests.csproj`, `src/Domain/ValueObjects/Colour.cs` and `tests/Domain.UnitTests/ValueObjects/ColourTests.cs`
- **THEN** `ColourTests.cs` is the test of `Colour.cs`

#### Scenario: Another folder in the test project

- **GIVEN** the layout of the previous scenario, but the test is `tests/Domain.UnitTests/ColourTests.cs`
- **THEN** it is still the test of `Colour.cs`

#### Scenario: Several test projects

- **GIVEN** `Application.UnitTests` and `Application.FunctionalTests` each contain a `TodoItemsTests.cs`
- **THEN** both are tests of `TodoItems.cs` in the project `Application`

#### Scenario: A test project of another project

- **GIVEN** projects `Shop`, `Shop.Api` and `Shop.Api.Tests`, and `Shop.Api.Tests` contains `OrderTests.cs`
- **THEN** `OrderTests.cs` is no test of `Order.cs` in the project `Shop`

### Requirement: Test files regardless of exclusion

Whether a test exists, and whether it was added or changed, SHALL be
judged by the paths of the head revision and of the change, also for
files that `[files] exclude` names or that are generated. A test SHALL
count as changed when the change adds it, edits it or renames it.

#### Scenario: Excluded specs still count

- **GIVEN** `qgate.toml` has `[files]` with `exclude = ["**/*.spec.ts"]`, and the change edits `Cart.total` in `src/app/cart.service.ts` and edits `src/app/cart.service.spec.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

### Requirement: Sources that need no test

`qgate.toml` MAY list path patterns under `exclude` in a `[tests]`
table, with the syntax and spelling of `[files] exclude`. A source file
whose path matches one SHALL have no `tests/` finding; it is still
analyzed otherwise.

#### Scenario: Environment files

- **GIVEN** `qgate.toml` has `[tests]` with `exclude = ["src/environments/**"]`, and the change adds `src/environments/environment.prod.ts` with a function, in a project that has specs
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: Other analyzers still run

- **GIVEN** `[tests]` has `exclude = ["**/Program.cs"]`, and the change adds a method of complexity 15 to `src/Web/Program.cs`
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `complexity/cyclomatic-new` Warning is reported for it, and no `tests/` finding

### Requirement: Unchanged test finding

When a source file has changed units and at least one test, and the
change adds or changes none of its tests, the system SHALL report one
finding with rule `tests/unchanged-test` and severity Warning, at the
file's path and the line of its first changed unit in source order.

#### Scenario: Service changed, spec not

- **GIVEN** `src/app/cart.service.spec.ts` exists, and the change edits the bodies of `CartService.total`, `CartService.add` and `CartService.remove` in `src/app/cart.service.ts`, the first on line 12
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output contains `warning src/app/cart.service.ts:12  CartService.total and 2 more changed, but not its test src/app/cart.service.spec.ts  [tests/unchanged-test]`
- **AND** the exit code is 0

#### Scenario: One unit

- **GIVEN** the change edits only `Order.Ship()` in `src/Domain/Order.cs`, and `tests/Domain.UnitTests/OrderTests.cs` is unchanged
- **THEN** the message is `Order.Ship() changed, but not its test tests/Domain.UnitTests/OrderTests.cs`

#### Scenario: The spec changed too

- **GIVEN** the change edits `CartService.total` and `src/app/cart.service.spec.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: One of several tests changed

- **GIVEN** `Application.UnitTests` and `Application.FunctionalTests` each have a `TodoItemsTests.cs`, and the change edits `TodoItems.cs` and only the functional test
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: Several unchanged tests

- **GIVEN** `cart.service.spec.ts` and `cart.service.test.ts` both exist beside `cart.service.ts`, and the change edits `CartService.total` only
- **THEN** the message names `src/app/cart.service.spec.ts`, the first test by path, and ends with `, and 1 more`

#### Scenario: A new function in a tested file

- **GIVEN** `src/app/cart.service.spec.ts` exists, and the change adds the method `CartService.clear` to `src/app/cart.service.ts` without touching the spec
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `tests/unchanged-test` finding is reported

#### Scenario: Failing on warnings

- **WHEN** the user runs `qgate check --base main --fail-on warning` on the change of "Service changed, spec not"
- **THEN** the exit code is 1

### Requirement: Missing test finding

When a source file has changed units and no test, its base version had
no function unit (it is new, or gets its first code), and its project
has a test, the system SHALL report one finding with rule
`tests/missing-test` and severity Warning, at the file's path and the
line of its first unit.

#### Scenario: New Angular service

- **GIVEN** `package.json` is at the repository root and `src/app/cart/cart.component.spec.ts` exists, and the change adds `src/app/orders/order.service.ts` with the methods `OrderService.load` (line 8) and `OrderService.save`, and no spec
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output contains `warning src/app/orders/order.service.ts:8  OrderService.load and 1 more have no test: expected src/app/orders/order.service.spec.ts  [tests/missing-test]`

#### Scenario: New C# class

- **GIVEN** the project `Domain` has the test projects `Domain.IntegrationTests` and `Domain.UnitTests`, and the change adds `src/Domain/Invoice.cs` with one method `Invoice.Total()` and no `InvoiceTests.cs`
- **THEN** a `tests/missing-test` finding reports `Invoice.Total() has no test: expected InvoiceTests.cs in Domain.IntegrationTests`, naming the first test project by name

#### Scenario: A new file with its spec

- **GIVEN** the change adds `src/app/orders/order.service.ts` with a method and `src/app/orders/order.service.spec.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: A new file with only types

- **GIVEN** the change adds `src/app/orders/order.model.ts` with interfaces only
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: An existing file without a test

- **GIVEN** `src/app/legacy/report.ts` has functions and no test in the base revision, and the change edits one of them
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

### Requirement: Projects that test other code

For `tests/missing-test`, a C# file's project SHALL have a test when it
has a test project. A TypeScript file's project SHALL have a test when
the nearest directory at or above the file that contains a
`package.json`, or the repository root if none does, contains a test
file at any depth in the head revision.

#### Scenario: C# project without a test project

- **GIVEN** the project `Web` has no test project, and the change adds `src/Web/Endpoints/Orders.cs` with a method
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: Repository without specs

- **GIVEN** a TypeScript repository with no `.spec.ts` or `.test.ts` file, and the change adds `src/format.ts` with a function
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `tests/` finding is reported

#### Scenario: Monorepo

- **GIVEN** `apps/web/package.json` and `libs/util/package.json`, specs only below `apps/web`, and the change adds a new file with a function and no spec in each
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `tests/missing-test` finding is reported for the file below `apps/web` only
