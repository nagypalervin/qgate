# Design: analyze-wrapped-functions

## Context

See proposal.md for why. The TypeScript adapter
(`crates/qgate-lang/src/typescript.rs`) makes one depth-first, non-recursive
walk over the tree-sitter tree. `Walker::enter` decides about each node when
it is first reached, and pushes a `Frame` that `leave` pops. The frame
records two things: a name segment (`segments`) and a scope (`scopes`).
`Some(i)` collects decision points into `functions[i]`; `None` is a class
body barrier.

- **Naming today:** a function, object or class is named by `name_of`.
  This calls `binding_name`, which walks up from the value through
  `WRAPPER_KINDS` to a `variable_declarator`, `public_field_definition`,
  `pair` or `export_statement`. A call stops that walk, so a wrapped
  function is anonymous.
- **Where its decision points go:** a wrapped function at module level, or
  in a class field initializer, has no scope that collects. The `if` in it
  goes nowhere.
- **The grammar:**
  - `call_expression` has the fields `function` and `arguments`;
  - `new_expression` has `constructor` and `arguments`;
  - `await_expression` and `arguments` have only unnamed children;
  - type arguments are a separate field, skipped by the walk (`TYPE_KINDS`).

## Goals / Non-Goals

**Goals:**

- Implement the two new requirements and the modified one in the
  `complexity-analysis` delta, inside the existing single walk.
- Leave every unit that exists today unchanged inside function units, in
  name and in complexity.

**Non-Goals:**

- No change to `FileModel`, `Function`, the analyzer, the report or the
  CLI.
- No second pass over the tree and no recursion: the walk stays
  stack-safe.

## Decisions

### D1: The bound call is recognized when the walk enters it

**What is a bound call?** One that sits directly at the binding. Formally:
a `call_expression`, `new_expression` or `await_expression` for which
`binding_name` finds a binding, while no scope collects, i.e.
`self.scopes.last()` is not `Some(Some(_))`.

- **Only the outermost call qualifies.** Inside `memo(forwardRef(fn))`,
  `forwardRef(...)` is an argument, not a bound value, so `binding_name`
  returns `None` for it.
- **`await` works without new code:** in `const data = await retry(fn)` the
  `await` is the bound value.
- **Duplicate names:** `binding_name` already looks through type wrappers,
  and already gives `default` for an unnamed default export. The segment
  is made unique with `unique_segment`, like any other name.

The walker keeps a stack of active bound calls:

```
struct Bound {
    node: usize,                 // tree-sitter node id of the bound call
    segment: String,             // the binding's name, made unique
    unit: Option<usize>,         // index in `functions`, if a function passes through
}
```

Bound calls can nest. In `wrap({ a: retry(() => {...}) })`, the object
passes through and pushes no scope, so `retry(...)` is a bound call of its
own, named `a`. Its unit is `X.a`. The frame of the bound call pops it.

**Alternative considered:** start from each function and walk up to the
binding, as `binding_name` does today. This finds the name, but not whether
another function in the same call already created the unit. That needs
shared state per call, which the bound-call stack is.

### D2: "Passes through" is one upward walk

`pass_through_top(node)` walks up from a value and returns the topmost node
it reaches. The walk continues while one of these holds:

- the parent is a type wrapper (`WRAPPER_KINDS`) or an `await_expression`;
- the parent is `arguments` (the walk continues from that node's parent,
  the call or `new`);
- the parent is a `call_expression` and the node is its `function` field
  (an immediately invoked function, or the inner call of a curried call).

A function, object or class passes through the innermost active bound call
exactly when `pass_through_top(node)` is that call. This covers:

| Form | Path upward from the value |
|---|---|
| `wrap(fn)` | fn, arguments, call |
| `memo(forwardRef(fn))` | fn, arguments, call, arguments, call |
| `connect(m)(fn)` | fn, arguments, outer call |
| `new Promise(fn)` | fn, arguments, `new` |
| `await retry(fn)` | fn, arguments, call, `await` |
| `(() => {...})()` | arrow, parenthesized, call (`function` field) |

**Excluded (as the proposal states):** the object of a member call
(`wrap(fn).then(...)`), array elements and conditional branches. The upward
walk stops at them.

### D3: The unit is created when the call is entered, if a function passes through

When a bound call is entered, a bounded scan answers one question: does a
function pass through it? The scan descends only along pass-through edges:
`arguments`, the `function` field of a call, `new`, `await` and type
wrappers. It never enters a function body, so its cost is the size of the
call's argument list, not of the code inside.

- **A function passes through:** the unit is pushed now, with the
  binding's name, line and complexity 1. Units therefore stay in source
  order, before the units nested in its arguments.
- **No function passes through:** no unit is created (`const x = foo()`,
  or `defineStore(...)` with only an object). This follows the
  requirement "only if there is at least one such function".

**Alternative considered:** create the unit lazily, at the first function
that passes through. In `wrap({ m() {} }, () => {...})` that would push
`X.m` before `X`, which breaks the source order that `FileModel` documents.

### D4: Entering a value that passes through

| Value | Segment pushed | Scope pushed | New unit |
|---|---|---|---|
| function expression, arrow, generator (named or not) | the bound call's segment | `Some(unit)` | no |
| object literal or class | the bound call's segment | none (as for any container) | no |

- **A function value ignores its own name.** This keeps "binding wins over
  the own name", and several functions merge into one unit.
- **Anonymous functions nested inside it** count towards that unit by the
  existing rule. Named ones become `X.inner`.
- **Decision points in the call's other arguments** (`opts ?? {}`) stay
  uncounted without extra code: the bound call itself pushes no scope, and
  no scope collects there.

### D5: Fixtures

- **The `GET` case leaves `module_level.ts`.** Its expected result changes;
  `module_level.ts` keeps what is still not measured and gains the
  `app.get` and `module.exports` cases.
- **New fixtures, one topic each**, as the review of the first change
  asked:
  - `wrapped_calls.ts`: the pass-through forms;
  - `bound_call_units.ts`: summing, other arguments, the named function
    expression, objects and classes, function plus object, class field and
    object property;
  - `wrapped_inside_function.ts`: nothing changes inside a function.

## Risks / Trade-offs

- **[Names that are not functions]** `const p = new Promise(fn)` gives a
  unit `p`, and `export const sorted = list.sort(cmp)` a unit `sorted`.
  → Accepted: at module level the alternative is not measuring the code
  at all, and the name still points to the right line.
- **[Method names change]** Methods of objects in bound calls get a
  prefix (`actions.load` becomes `useCart.actions.load`). → No false
  findings, because both revisions are analyzed by the same binary and
  there is no baseline. Stated in the proposal's Impact.
- **[Scan cost]** The D3 scan runs for every module-level bound call,
  including plain ones like `const x = foo(a, b)`. → It visits only
  pass-through edges, never function bodies, so its total work is linear
  in the size of the argument lists.
- **[Fewer forms than users write]** A wrapper reached through a member
  (`wrap(fn).then(...)`), an array (`compose([f, g])`) or an assignment
  (`module.exports = ...`) is still not measured. → Named in Out of scope;
  each is a small follow-up if real code needs it.

## Migration Plan

None. There is no stored state; the next run of `qgate check` uses the new
rule on both revisions.

## Implementation Notes

- **`Bound` stores the end of the call's own pass-through chain**, not the
  call node. In `const typed = wrap(fn as H) satisfies W` the function's
  chain ends at the `satisfies` expression around the call, so D1's "node
  id of the bound call" would never match. The walker compares
  `pass_through_top(value)` with `pass_through_top(call)`, which also
  covers parentheses and `as` around the call.
- **The two directions use the same edges:**
  - type wrappers, `await`, the `arguments` of a call or `new`, and the
    `function` field of a call;
  - the `constructor` of `new` and the template string of a tagged
    template are not edges in either direction, so `pass_through_top`
    (upward) and the D3 scan (downward) cannot disagree.
- **Fixtures beyond D5:**
  - `default_wrapped_object.ts` holds `export default defineComponent(...)`,
    because a fixture already has a default export;
  - `wrapped_inside_function.ts` also pins a class field inside a function
    (`make.h`). The requirement applies there, because a class field
    initializer collects nothing. So task 1.2's "already passes" held for
    `C` and `outer` only.
