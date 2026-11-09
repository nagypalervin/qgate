# Proposal: analyze-wrapped-functions

## Why

qgate does not measure a function wrapped in a call at module level, such as
`export const GET = withAuth(async (req) => {...})` or
`export const Button = memo(() => {...})`. In Next.js route handlers, React
components and similar code, these are often the functions that matter
most. The first complexity change left this gap on purpose and named it as
a follow-up (`add-cyclomatic-complexity-check`, proposal "Out of scope").
Closing it now finishes the M1 complexity check before M2 adds
configuration.

## What Changes

- **One new naming rule:** where code is not measured today because no
  enclosing function unit collects its decision points (module level, or a
  class field initializer), a call between a value and its binding does
  not hide the binding. Type wrappers already behave this way: `as`,
  `satisfies`, parentheses and `!`. The binding is a variable, class field,
  object property or default export, as today.
- **Pass-through positions:** the name passes through:
  - a call argument;
  - nested calls (`memo(forwardRef(fn))`);
  - member calls with type arguments (`React.forwardRef<H, P>(fn)`);
  - curried calls (`connect(map)(fn)`);
  - `new` (`new Promise(fn)`);
  - `await` (`await retry(fn)`);
  - an immediately invoked function (`(() => {...})()`);
  - `export default wrap(fn)`.
- **Functions in a bound call form one unit**, named by the binding:
  - its complexity is 1 plus the decision points of every function in a
    pass-through position;
  - several functions, as in `createSelector(f, g, h)`, are summed. This
    follows "anonymous functions count towards their named ancestor";
  - decision points in the call's other arguments (`wrap(fn, opts ?? {})`)
    do not count, as for any other module-level code;
  - a named function expression in a pass-through position is part of the
    unit too, as in "binding wins over the own name".
- **Objects and classes in a bound call are named after the binding.**
  `defineStore("cart", { actions: { load() {} } })` bound to `useCart`
  gives `useCart.actions.load`. Before this change it gave `actions.load`.
  This mirrors `const api = { load() {} }`, which gives `api.load`.
- **Inside function units nothing changes.** The callback of a
  `useCallback`, `.map` or `.reduce` still counts towards the enclosing
  function, and no existing complexity value changes.

### Decisions taken during exploration

- **Module level only** (option A). Only code that is measured nowhere
  today changes, so existing results keep their numbers. Applying the rule
  everywhere would have split `const total = items.reduce(cb)` out of its
  function as a unit named after a number.
- **The eight pass-through forms above.** Assignments
  (`module.exports = wrap(fn)`) are excluded; see Out of scope.
- **One unit per bound call, summed** (option a), not `X`, `X#2`, `X#3`.
  Duplicate suffixes shift when an argument is added or removed.
- **Objects and classes get the binding as prefix** (option b), so a file
  with two `defineStore` calls gives two distinct names instead of
  `actions.load` and `actions.load#2`.

### Out of scope

- Assignments as bindings: `module.exports = ...`,
  `exports.handler = ...` and `obj.prop = ...`. They are unnamed even
  without a call today; that is a separate gap.
- Calls without a binding, such as `describe(...)` / `it(...)` and
  `app.get("/", (req, res) => {...})`. Nothing gives them a name.
- Values that are not in a pass-through position:
  - the object of a member call (`wrap(fn).then(...)`);
  - array elements;
  - a branch of a conditional (`wrap(a ? f : g)`).
- Changing anything inside function units (option B of the exploration).
- Rule ids, thresholds and output. `complexity/cyclomatic-new` simply finds
  more functions.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `complexity-analysis`:
  - "Code outside named functions is not measured" makes an exception for
    calls with a binding. Its scenario "Function wrapped in a call at
    module level" is reversed, from no unit to unit `GET`;
  - a new requirement, "Calls at module level do not hide the binding",
    describes where the rule applies and the pass-through forms;
  - a new requirement, "A bound call is one unit", describes the summed
    unit and the naming of objects and classes.

## Impact

- **`qgate-lang`:** naming and unit boundaries in the TypeScript adapter
  (`crates/qgate-lang/src/typescript.rs`), its fixtures and tests, and the
  crate and `Function` docs that describe unit boundaries.
- **Other crates:** none. The analyzer, report and CLI see more units, or
  different names for methods of objects in bound calls; no interface
  changes.
- **Users:** more findings in code that uses wrappers at module level,
  because more functions are measured. The renamed methods cause no false
  findings: both revisions are analyzed by the same qgate version, and no
  baseline is stored.
- **Dependencies:** none.
