// Fixture: functions passed through one bound call form one unit.

// Several functions: 1 + ?? + ternary.
export const sel = createSelector(
  (s: State) => s.items,
  (s: State) => s.d ?? 1,
  (items: Item[], d: number) => (items.length > 0 ? d : 0),
);

// The ?? in the options object does not count.
export const h = withRetry(
  async () => {
    if (a) {
      run();
    }
  },
  { tries: n ?? 3 },
);

// The binding wins over the function's own name.
export const named = wrap(function onDone() {
  if (a) {
    run();
  }
});

// Objects and classes take the binding as prefix; no unit of their own.
export const useCart = defineStore("cart", {
  actions: {
    load() {
      if (a) {
        run();
      }
    },
  },
});

export const Mixed = mixin(class {
  m() {
    if (a) {
      run();
    }
  }
});

// A function and an object in one call.
export const X = wrap(
  () => {
    if (a) {
      run();
    }
  },
  { m() {} },
);

// A bound call inside an object that passes through.
export const nested = wrap({
  a: retry(() => {
    if (a) {
      run();
    }
  }),
});

// A class field and an object property as bindings.
class A {
  onScroll = debounce(() => {
    if (a) {
      run();
    }
  });
}

const api = {
  load: withRetry(async () => {
    if (b) {
      run();
    }
  }),
};

// A call with no function, object or class: no unit.
export const plain = compute(a ?? b);
