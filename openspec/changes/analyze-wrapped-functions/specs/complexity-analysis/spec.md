# Spec Delta

## MODIFIED Requirements

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

## ADDED Requirements

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
