// Fixture large_increase/before: an existing function grows by 5.

export function process(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  return -1;
}
