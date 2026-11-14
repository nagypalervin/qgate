// Fixture: one kind of edit per unit; after.ts makes them.
// Asserted in src/typescript.rs.

export function layout(a: number, b: number): number {
  return a + b;
}

export function comments(a: number): number {
  return a * 2;
}

export function semicolons(a: number) {
  const b = a + 1;
  return b;
}

export function typed(a: number): number {
  const items: Array<number> = [a];
  return (items as number[]).length!;
}

export function literal(): number {
  return 1;
}

export function text(): string {
  return "a";
}

export function template(x: number): string {
  return `total: ${x}`;
}

export function renamed(a: number): number {
  const local = a;
  return local;
}

export const GET = withAuth(async () => {
  return 200;
});

export const POST = withAuth(async () => {
  return 201;
});

export class Cart {
  limit = 1;

  refresh = debounce(() => this.total([]), 100);

  total(items: number[]): number {
    return items.reduce((s, x) => s + x, 0);
  }

  outer(): number {
    function inner(): number {
      return 1;
    }
    return inner() + 1;
  }

  @HostListener("click")
  onClick(): number {
    return 1;
  }
}
