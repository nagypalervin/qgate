# Spec Delta

## Purpose

Complexity analysis tells a user which functions became harder to test and
understand in a change, by comparing per-function cyclomatic complexity
between a base and a head version and reporting only what got worse.

## ADDED Requirements

### Requirement: Cyclomatic complexity of a function

The system SHALL measure the cyclomatic complexity of a function unit as 1
plus the number of decision points in its body, following ESLint's
`complexity` rule except where this spec says otherwise.

#### Scenario: Straight-line function

- **WHEN** a function contains no decision points
- **THEN** its complexity is 1

#### Scenario: Compound condition

- **WHEN** a function body is `if (a && b) { x(); } else if (c) { y(); } else { z(); }`
- **THEN** its complexity is 4 (two `if`, one `&&`; the plain `else` does not count)

### Requirement: Decision points

The system SHALL count one decision point for each: `if`; `for`, `for...in`,
`for...of`, `while` and `do...while` loop; `case` clause with a test;
`catch` clause; conditional expression `? :`; logical operator `&&`, `||`
and `??`; logical assignment `&&=`, `||=` and `??=`; optional chaining
`?.`; and default value in a parameter or destructuring pattern.

#### Scenario: Loops and switch

- **WHEN** a function contains one `for...of` loop, one `while` loop and a `switch` with two `case` clauses and a `default`
- **THEN** its complexity is 5

#### Scenario: Error handling and expressions

- **WHEN** a function contains a `try` with a `catch` and a `finally`, one conditional expression and one `??`
- **THEN** its complexity is 4

#### Scenario: Optional chaining and default values

- **WHEN** a function is `function f(x = 1) { return a?.b?.c; }`
- **THEN** its complexity is 4

### Requirement: Type-level syntax is not counted

The system SHALL NOT count TypeScript type-level constructs as decision
points, including conditional types, optional properties and parameters
marked with `?`, and non-null assertions.

#### Scenario: Conditional type and optional parameter

- **WHEN** a function is `function f<T>(x?: T): T extends string ? 1 : 2 { return g(x!); }`
- **THEN** its complexity is 1

### Requirement: Function units

The system SHALL treat as a function unit every function declaration,
function expression, arrow function, generator, class method, constructor,
getter and setter that has a body, and every object literal method.

#### Scenario: Class members

- **WHEN** a class has a constructor, a method, a getter and a setter
- **THEN** each of the four is measured separately

### Requirement: Named nested functions are separate units

A named function inside another function SHALL be measured on its own, and
its decision points SHALL NOT count towards the enclosing function.

#### Scenario: Inner helper

- **GIVEN** `function outer() { if (a) {} function inner() { if (b) {} if (c) {} } }`
- **WHEN** the file is analyzed
- **THEN** `outer` has complexity 2 and `outer.inner` has complexity 3

### Requirement: Anonymous functions count towards their named ancestor

An anonymous function (one that receives no name under the naming rules)
SHALL NOT be a unit of its own; its decision points SHALL count towards the
nearest enclosing named function unit. This deliberately deviates from
ESLint, which reports each anonymous function separately.

#### Scenario: Callback in a method

- **GIVEN** `function process(orders) { return orders.filter(o => o.paid && o.shipped); }`
- **WHEN** the file is analyzed
- **THEN** `process` has complexity 2 and no other unit is reported

### Requirement: Code outside named functions is not measured

The system SHALL NOT measure module-level statements, class field
initializers that are not functions, or anonymous functions that have no
named ancestor.

#### Scenario: Test file

- **GIVEN** a file whose only code is `describe("x", () => { it("y", () => { if (a) {} }); });`
- **WHEN** the file is analyzed
- **THEN** no function unit is found

### Requirement: Qualified function names

Each function unit SHALL have a qualified name made of the names of its
named ancestors and its own name, joined by `.`. A function is named by its
declaration, by the variable, class field or object property it is assigned
to in its definition, or as `default` when it is an unnamed default export.

#### Scenario: Names in a class and an object

- **GIVEN** `class Cart { total() {} handle = () => {}; get size() { return 0; } }` and `const api = { load() {} };`
- **WHEN** the file is analyzed
- **THEN** the units are named `Cart.total`, `Cart.handle`, `Cart.get size` and `api.load`

#### Scenario: Unnamed default export

- **WHEN** the file contains `export default function () { if (a) {} }`
- **THEN** the unit is named `default`

### Requirement: Duplicate names

When several units in one file would get the same qualified name, the
system SHALL keep the first one in source order as is and suffix the later
ones with `#2`, `#3` and so on.

#### Scenario: Static and instance method with the same name

- **GIVEN** a class `A` with `static create() {}` followed by `create() {}`
- **WHEN** the file is analyzed
- **THEN** the units are named `A.create` and `A.create#2`

### Requirement: Overload signatures are ignored

Function and method overload signatures without a body SHALL NOT be units;
only the implementation is measured.

#### Scenario: Overloaded function

- **GIVEN** two overload signatures of `parse` followed by one implementation with one `if`
- **WHEN** the file is analyzed
- **THEN** exactly one unit `parse` with complexity 2 is found

### Requirement: Matching functions between versions

The system SHALL match a unit in the before file with the unit of the same
qualified name in the after file. A unit with no match in the before file is
new; a unit with no match in the after file is removed.

#### Scenario: Renamed function

- **GIVEN** `calc` in the before file is renamed to `calculateTotal` in the after file, with the same body
- **WHEN** the two files are compared
- **THEN** `calc` is treated as removed and `calculateTotal` as new

### Requirement: Complexity increase rule

For a matched unit whose complexity increased, the system SHALL report a
finding with rule `complexity/cyclomatic-increase`, severity Warning for an
increase of 1 to 4 and Error for an increase of 5 or more.

#### Scenario: Small increase

- **WHEN** a matched function goes from 4 to 5
- **THEN** a Warning `complexity/cyclomatic-increase` finding is reported

#### Scenario: Large increase

- **WHEN** a matched function goes from 3 to 8
- **THEN** an Error `complexity/cyclomatic-increase` finding is reported

#### Scenario: Large legacy function grows a little

- **WHEN** a matched function goes from 40 to 41
- **THEN** a Warning finding is reported, not an Error

### Requirement: New complex function rule

For a new unit with complexity above 10, the system SHALL report a finding
with rule `complexity/cyclomatic-new`, severity Warning for 11 to 20 and
Error above 20. New units with complexity 10 or less SHALL NOT be reported.

#### Scenario: Simple new function

- **WHEN** a new function has complexity 10
- **THEN** no finding is reported for it

#### Scenario: Complex new function

- **WHEN** a new function has complexity 11
- **THEN** a Warning `complexity/cyclomatic-new` finding is reported

#### Scenario: Very complex new function

- **WHEN** a new function has complexity 21
- **THEN** an Error `complexity/cyclomatic-new` finding is reported

### Requirement: No findings for improvements

The system SHALL NOT report a finding for a removed unit or for a matched
unit whose complexity stayed the same or decreased.

#### Scenario: Simplified and removed functions

- **GIVEN** one function went from 6 to 3 and another was removed
- **WHEN** the two files are compared
- **THEN** no finding is reported

### Requirement: Finding content

Every complexity finding SHALL carry the before and after values, point to
the line of the unit's name in the after file (or of the function itself for
an unnamed default export), and use the message
`<name>: cyclomatic complexity <before> -> <after> (+<delta>)` for an
increase and `<name>: new function, cyclomatic complexity <after>` for a new
unit.

#### Scenario: Increase message

- **WHEN** `applyDiscount`, named on line 88 of the after file, goes from 4 to 5
- **THEN** the finding points to line 88 with message `applyDiscount: cyclomatic complexity 4 -> 5 (+1)`

#### Scenario: New function message

- **WHEN** a new function `buildInvoice` with complexity 14 is named on line 120
- **THEN** the finding points to line 120 with message `buildInvoice: new function, cyclomatic complexity 14`
