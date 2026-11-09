function simplified(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  if (c[3]) { return 3; }
  return -1;
}

function removed(c: boolean[]): number {
  if (c[0]) { return 0; }
  if (c[1]) { return 1; }
  if (c[2]) { return 2; }
  return -1;
}
