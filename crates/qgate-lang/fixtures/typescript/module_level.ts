// Fixture: code with no named ancestor is not measured.

describe("module level", () => {
  it("is not measured", () => {
    if (a) {
      run();
    }
  });
});

// A call without a binding: nothing names the callback.
app.get("/", (req: Request, res: Response) => {
  if (a) {
    run();
  }
});

// An assignment is not a binding (out of scope).
module.exports = wrap(() => {
  if (b) {
    run();
  }
});

if (topLevel) {
  run();
}
