// Fixture: code with no named ancestor is not measured.

describe("module level", () => {
  it("is not measured", () => {
    if (a) {
      run();
    }
  });
});

export const GET = withAuth(async (req: Request) => {
  if (a) {
    run();
  }
});

if (topLevel) {
  run();
}
