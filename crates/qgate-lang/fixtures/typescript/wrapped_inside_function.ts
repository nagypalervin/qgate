// Fixture: inside a function unit, calls still hide nothing new.

function C() {
  const h = useCallback(() => {
    if (a) {
      run();
    }
  }, []);
  return h;
}

function outer(items: number[]) {
  const total = items.reduce((s, i) => s + (i > 0 ? i : 0), 0);
  return total;
}

// A class field initializer collects nothing, even inside a function, so
// the wrapped function is named after the field.
function make() {
  return class {
    h = debounce(() => {
      if (a) {
        run();
      }
    });
  };
}
