// Fixture: generators.

function* gen(a: boolean) {
  if (a) {
    yield 1;
  }
}

const g = function* () {};

class C {
  *items() {}
}
