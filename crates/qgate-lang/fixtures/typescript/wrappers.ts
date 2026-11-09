// Fixture: type wrappers between a definition and its binding.

const config = {
  load() {},
} satisfies Config;

const other = {
  load() {},
} as const;

const h = (async () => {
  if (a) {
    run();
  }
}) satisfies Handler;

const nonNull = (() => 1)!;

const Widget = (class {
  render() {}
}) as typeof Base;
