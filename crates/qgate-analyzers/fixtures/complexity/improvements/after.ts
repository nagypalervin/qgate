// Fixture improvements/after: one simplified, one removed, one unchanged.

export function simplified(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  return -1;
}

export function unchanged(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  return -1;
}
