export class Cart {
  total(a: number): number {
    if (a > 1) { a++; }
    if (a > 2) { a++; }
    if (a > 3) { a++; }
    if (a > 4) { a++; }
    if (a > 5) { a++; }
    if (a > 6) { a++; }
    if (a > 7) { a++; }
    if (a > 8) { a++; }
    if (a > 9) { a++; }
    if (a > 10) { a++; }
    return a;
  }

  size(): number {
    return 0;
  }
}
