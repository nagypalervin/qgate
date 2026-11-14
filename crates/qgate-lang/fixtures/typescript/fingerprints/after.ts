// Fixture: one kind of edit per unit, made to before.ts.
// Asserted in src/typescript.rs.

export function layout(a: number,
                       b: number): number { return a   +   b; }

export function comments(a: number): number {
  // doubled
  return a /* twice */ * 2;
}

export function semicolons(a: number) {
  const b = a + 1
  return b
}

export function typed(a: unknown): string {
  const items: number[] = [a];
  return (items as readonly number[]).length;
}

export function literal(): number {
  return 2;
}

export function text(): string {
  return "b";
}

export function template(x: number): string {
  return `sum: ${x}`;
}

export function renamed(a: number): number {
  const value = a;
  return value;
}

export const GET = withAuth(async () => {
  return 204;
});

export const POST = withAdmin(async () => {
  return 201;
});

export class Cart {
  limit = 2;

  refresh = debounce(() => this.total([]), 500);

  total(items: number[]): number {
    return items.reduce((s, x) => s + 2 * x, 0);
  }

  outer(): number {
    function inner(): number {
      return 3;
    }
    return inner() + 1;
  }

  @HostListener("keydown")
  onClick(): number {
    return 1;
  }
}
