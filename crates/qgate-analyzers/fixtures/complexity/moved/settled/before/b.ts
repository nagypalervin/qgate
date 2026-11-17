export class B {
  load(x: number): number {
    if (x > 1) { x--; }
    if (x > 2) { x--; }
    return x;
  }
}
