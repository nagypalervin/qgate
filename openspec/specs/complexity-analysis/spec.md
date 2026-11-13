# complexity-analysis Specification

## Purpose

Complexity analysis tells a user which functions became harder to test and
understand in a change, by comparing per-function cyclomatic complexity
between a base and a head version and reporting only what got worse.

## Requirements

### Requirement: Cyclomatic complexity of a function

The system SHALL measure the cyclomatic complexity of a function unit as 1
plus the number of decision points in its body, following ESLint's
`complexity` rule except where this spec says otherwise. The requirements
on units, names and decision points in this spec apply to TypeScript; for
C#, the `csharp-analysis` capability defines them. Matching, the rules
and the findings apply to both.

#### Scenario: Straight-line function

- **WHEN** a function contains no decision points
- **THEN** its complexity is 1

#### Scenario: Compound condition

- **WHEN** a function body is `if (a && b) { x(); } else if (c) { y(); } else { z(); }`
- **THEN** its complexity is 4 (two `if`, one `&&`; the plain `else` does not count)

#### Scenario: A C# method that got more complex

- **GIVEN** the before version of `src/Cart.cs` has `Cart.Total(int)` with complexity 3, and the after version adds two `if` statements to it
- **WHEN** the two versions are compared
- **THEN** a Warning `complexity/cyclomatic-increase` finding reports `Cart.Total(int): cyclomatic complexity 3 -> 5 (+2)`

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

#### Scenario: Generators

- **WHEN** the file contains `function* gen(a) { if (a) { yield 1; } }`, `const g = function* () {};` and `class C { *items() {} }`
- **THEN** the units are `gen` with complexity 2, `g` and `C.items`

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
named ancestor, except for functions that a call passes to a binding (see
"Calls at module level do not hide the binding").

#### Scenario: Test file

- **GIVEN** a file whose only code is `describe("x", () => { it("y", () => { if (a) {} }); });`
- **WHEN** the file is analyzed
- **THEN** no function unit is found

#### Scenario: Class field initializer and static block

- **GIVEN** `function make() { return class { n = a ?? 0; static { if (b) {} } }; }`
- **WHEN** the file is analyzed
- **THEN** `make` has complexity 1

#### Scenario: Function wrapped in a call at module level

- **GIVEN** a file whose only code is `export const GET = withAuth(async (req) => { if (a) {} });`
- **WHEN** the file is analyzed
- **THEN** the only unit is `GET` with complexity 2

#### Scenario: Call without a binding

- **GIVEN** a file whose only code is `app.get("/", (req, res) => { if (a) {} });` and `module.exports = wrap(() => { if (b) {} });`
- **WHEN** the file is analyzed
- **THEN** no function unit is found

### Requirement: Calls at module level do not hide the binding

At module level and in class field initializers, a call between a
function, object literal or class and its binding SHALL NOT hide the
binding; elsewhere it still does. A value passes through when it is,
possibly behind type wrappers, an argument of a call or `new`, the
function of a call, or the operand of `await`, and that expression in turn
passes through or is the bound value.

#### Scenario: Nested and member calls

- **WHEN** the file contains `export const Button = React.memo(forwardRef<H, P>((props, ref) => { if (a) {} }));`
- **THEN** the only unit is `Button` with complexity 2

#### Scenario: Curried call, new, await and immediately invoked function

- **WHEN** the file contains `export const Comp = connect(map)((props) => { if (a) {} });`, `const p = new Promise((resolve) => { if (b) {} });`, `const data = await retry(async () => { if (c) {} });` and `const config = (() => { if (d) {} return 1; })();`
- **THEN** the units are `Comp`, `p`, `data` and `config`, each with complexity 2

#### Scenario: Anonymous callbacks and static blocks

- **WHEN** the file contains `describe("x", () => { it("y", async () => { const res = await act(async () => { if (a) {} }); }); });` and `class R { static { const h = wrap(() => { if (b) {} }); } }`
- **THEN** no function unit is found

#### Scenario: Class field in a test callback or a static block

- **WHEN** the file contains `describe("x", () => { class Fake { h = wrap(() => { if (a) {} }); } });` and `class Outer { static { class Inner { h = wrap(() => {}); } } }`
- **THEN** the units are `Fake.h` with complexity 2 and `Outer.Inner.h` with complexity 1

#### Scenario: Default export

- **WHEN** the file contains `export default withRouter(() => { if (a) {} });`
- **THEN** the only unit is `default` with complexity 2

### Requirement: A bound call is one unit

All functions that pass through one bound call SHALL form one unit, named
by the binding, on the line of the binding's name, and only if there is at
least one such function. Its complexity is 1 plus their decision points;
decision points elsewhere in the call SHALL NOT count. Object literals and
classes that pass through SHALL take the binding's name as the first
segment of their members' qualified names.

#### Scenario: Several functions in one call

- **WHEN** the file contains `export const sel = createSelector((s) => s.items, (s) => s.d ?? 1, (items, d) => (items.length > 0 ? d : 0));`
- **THEN** the only unit is `sel` with complexity 3

#### Scenario: Other arguments of the call do not count

- **WHEN** the file contains `export const h = withRetry(async () => { if (a) {} }, { tries: n ?? 3 });`
- **THEN** the only unit is `h` with complexity 2

#### Scenario: Named function expression in a call

- **WHEN** the file contains `export const h = wrap(function onDone() { if (a) {} });`
- **THEN** the only unit is `h` with complexity 2

#### Scenario: Objects and classes in a call

- **WHEN** the file contains `export const useCart = defineStore("cart", { actions: { load() { if (a) {} } } });` and `export default defineComponent({ setup() {} });`
- **THEN** the units are `useCart.actions.load` with complexity 2 and `default.setup` with complexity 1

#### Scenario: Names inside a function that passes through

- **WHEN** the file contains `export const Panel = memo((props) => { const handleClick = () => { if (a) {} }; function inner() {} const o = { m() {} }; return props.items.map((i) => (i ? 1 : 0)); });`
- **THEN** the units are `Panel` with complexity 2, `Panel.handleClick` with complexity 2, `Panel.inner` and `Panel.o.m`

#### Scenario: Function and object in one call

- **WHEN** the file contains `export const X = wrap(() => { if (a) {} }, { m() {} });`
- **THEN** the units are `X` with complexity 2 and `X.m` with complexity 1

#### Scenario: Class field and object property

- **WHEN** the file contains `class A { onScroll = debounce(() => { if (a) {} }); }` and `const api = { load: withRetry(async () => { if (b) {} }) };`
- **THEN** the units are `A.onScroll` and `api.load`, each with complexity 2

#### Scenario: Inside a function nothing changes

- **GIVEN** `function C() { const h = useCallback(() => { if (a) {} }, []); }`
- **WHEN** the file is analyzed
- **THEN** the only unit is `C` with complexity 2

### Requirement: Qualified function names

Each function unit SHALL have a qualified name made of the names of its
named ancestors and its own name, joined by `.`. A function, class or
object literal is named by the variable, class field or object property it
is assigned to in its definition, otherwise by its own declared name, or as
`default` when it is an unnamed default export.

#### Scenario: Names in a class and an object

- **GIVEN** `class Cart { total() {} handle = () => {}; get size() { return 0; } }` and `const api = { load() {} };`
- **WHEN** the file is analyzed
- **THEN** the units are named `Cart.total`, `Cart.handle`, `Cart.get size` and `api.load`

#### Scenario: Unnamed default export

- **WHEN** the file contains `export default function () { if (a) {} }`
- **THEN** the unit is named `default`

#### Scenario: Unnamed default-exported object and class

- **WHEN** one file contains `export default { load() {} };` and another `export default class { m() {} }`
- **THEN** the units are named `default.load` and `default.m`

#### Scenario: Named function expression passed as an argument

- **WHEN** the file contains `call(function onDone() { if (a) {} });` inside no other named function
- **THEN** the unit is named `onDone`

#### Scenario: Binding wins over the own name

- **WHEN** the file contains `const bound = function ownName() {};`
- **THEN** the unit is named `bound`

### Requirement: Type wrappers do not hide the binding

Parentheses, `as` and `satisfies` expressions and non-null assertions
between a function, class or object literal and the variable, class field
or object property it is assigned to SHALL NOT change its name.

#### Scenario: Object with satisfies and as const

- **WHEN** the file contains `const config = { load() {} } satisfies Config;` and `const other = { load() {} } as const;`
- **THEN** the units are named `config.load` and `other.load`

#### Scenario: Parenthesized arrow function with satisfies

- **WHEN** the file contains `const h = (async () => { if (a) {} }) satisfies Handler;`
- **THEN** the unit is named `h` with complexity 2

### Requirement: Duplicate names

When several units in one file would get the same qualified name, the
system SHALL keep the first one in source order as is and suffix the later
ones with `#2`, `#3` and so on, skipping any suffixed name that is already
taken. The suffix is part of the unit's name for its nested units.

#### Scenario: Static and instance method with the same name

- **GIVEN** a class `A` with `static create() {}` followed by `create() {}`
- **WHEN** the file is analyzed
- **THEN** the units are named `A.create` and `A.create#2`

#### Scenario: Suffix already taken

- **WHEN** the file contains `const o = { "x#2"() {}, x() {}, x() {} };`
- **THEN** the units are named `o.x#2`, `o.x` and `o.x#3`

#### Scenario: Nested units of a duplicate

- **GIVEN** `class A { static f() { function g() {} } f() { function g() {} } }`
- **WHEN** the file is analyzed
- **THEN** the units are named `A.f`, `A.f.g`, `A.f#2` and `A.f#2.g`

### Requirement: Property names

A property name SHALL be used as written in the source, except that the
quotes around a string key are removed; escape sequences inside it are kept
as written. Computed names keep their brackets.

#### Scenario: Quoted and computed keys

- **WHEN** the file contains `const o = { "load-data"() {}, 'it\'s'() {}, [key]() {} };`
- **THEN** the units are named `o.load-data`, `o.it\'s` and `o.[key]`

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
