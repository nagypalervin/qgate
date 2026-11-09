// Fixture: the `{` of the if on line 5 is never closed.
function ok(): void {
}

function broken(a: number): number {
  if (a) {
    return a;
