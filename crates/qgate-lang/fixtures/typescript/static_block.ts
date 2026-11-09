// Fixture: class field initializers and static blocks are not measured,
// even inside a function.
function make() {
  return class {
    n = a ?? 0;
    static {
      if (b) {
        run();
      }
    }
  };
}
