// Fixture: the first syntax error is on line 7.
// Everything before it is valid.

function valid(): void {
}

function broken(a: number {
  return a;
}
