// Fixture small_increase/before: existing functions grow by 1 and by 4.

export function applyDiscount(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  return -1;
}

export function shipping(c: boolean[]): number {
  if (c[0]) { return 0; }
  return -1;
}
