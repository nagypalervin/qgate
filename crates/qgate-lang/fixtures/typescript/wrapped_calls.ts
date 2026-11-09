// Fixture: calls at module level do not hide the binding.

export const GET = withAuth(async (req: Request) => {
  if (a) {
    run();
  }
});

// Nested calls, a member call and type arguments.
export const Button = React.memo(
  forwardRef<HTMLButtonElement, Props>((props, ref) => {
    if (a) {
      run();
    }
  }),
);

// A curried call.
export const Comp = connect(mapState)((props: Props) => {
  if (a) {
    run();
  }
});

const p = new Promise((resolve) => {
  if (b) {
    resolve(1);
  }
});

const data = await retry(async () => {
  if (c) {
    run();
  }
});

// An immediately invoked function.
const config = (() => {
  if (d) {
    return 1;
  }
  return 2;
})();

// Type wrappers around the call and around the function.
const typed = wrap((() => {
  if (e) {
    run();
  }
}) as Handler) satisfies Wrapped;

export default withRouter(() => {
  if (a) {
    run();
  }
});
