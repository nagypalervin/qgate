// Fixture large_increase/after: an existing function grows by 5.

export function process(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  if (c[3]) { return 3; }
  if (c[4]) { return 4; }
  if (c[5]) { return 5; }
  if (c[6]) { return 6; }
  return -1;
}
