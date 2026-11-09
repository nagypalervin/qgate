// Fixture small_increase/after: existing functions grow by 1 and by 4.

// Two extra lines move every function down,
// so the finding line must come from this file.

export function applyDiscount(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  if (c[3]) { return 3; }
  return -1;
}

export function shipping(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  if (c[3]) { return 3; }
  if (c[4]) { return 4; }
  return -1;
}
