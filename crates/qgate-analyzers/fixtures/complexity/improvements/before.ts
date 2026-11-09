// Fixture improvements/before: one simplified, one removed, one unchanged.

export function simplified(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  if (c[3]) { return 3; }
  if (c[4]) { return 4; }
  return -1;
}

export function removed(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  return -1;
}

export function unchanged(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  return -1;
}
