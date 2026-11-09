// Fixture: decision points.
// Each function isolates one kind of decision point. The expected
// cyclomatic complexity is asserted in src/typescript.rs.

function straight(a: number): number {
  const b = a + 1;
  return b * 2;
}

function compoundCondition(a: boolean, b: boolean, c: boolean): void {
  if (a && b) {
    run();
  } else if (c) {
    run();
  } else {
    run();
  }
}

function forLoops(items: number[], o: object, n: number): void {
  for (let i = 0; i < n; i++) {
    run();
  }
  for (const key in o) {
    run();
  }
  for (const item of items) {
    run();
  }
}

function whileLoops(n: number): void {
  while (n > 0) {
    n--;
  }
  do {
    n++;
  } while (n < 10);
}

function loopsAndSwitch(items: number[], kind: string, n: number): void {
  for (const item of items) {
    run();
  }
  while (n > 0) {
    n--;
  }
  switch (kind) {
    case "a":
      run();
      break;
    case "b":
      run();
      break;
    default:
      run();
  }
}

function onlyDefault(kind: string): void {
  switch (kind) {
    default:
      run();
  }
}

function errorHandling(a: boolean, b?: string): string {
  try {
    run();
  } catch (e) {
    run();
  } finally {
    run();
  }
  const c = a ? "yes" : "no";
  return b ?? c;
}

function catchWithoutBinding(): void {
  try {
    run();
  } catch {
    run();
  }
}

function logicalOperators(a: boolean, b: boolean, c?: boolean, d?: boolean): boolean | undefined {
  return (a || b) && (c ?? d);
}

function logicalAssignment(o: { x?: number; y?: number; z?: number }): void {
  o.x ||= 1;
  o.y &&= 2;
  o.z ??= 3;
}

function optionalAndDefaults(x = 1): unknown {
  return a?.b?.c;
}

function optionalCall(f?: () => void): void {
  f?.();
}

function destructuringDefaults(o: any): void {
  const { a = 1, b: [c = 2] = [] } = o;
}

function parameterDestructuringDefault({ a = 1 } = {}): void {
  run();
}

function typeLevel<T>(x?: T): T extends string ? 1 : 2 {
  return g(x!);
}

function inlineTypes(o: { a?: number; b: string | undefined }): asserts o is { a: number } {
  const t: Array<string | number> = [];
}
