// Fixture: naming and unit boundaries.
// The expected (name, line, cyclomatic) triples are asserted in
// src/typescript.rs.

class Cart {
  constructor(private items: number[]) {}

  total(): number {
    return this.items.reduce((sum, item) => sum + (item ?? 0), 0);
  }

  handle = () => {
    if (this.items.length > 0) {
      run();
    }
  };

  get size(): number {
    return this.items.length;
  }

  set size(value: number) {
    if (value < 0) {
      throw new Error("negative");
    }
  }

  static create(): Cart {
    return new Cart([]);
  }

  create(): void {}

  [Symbol.iterator]() {}

  #secret() {}

  count = this.items?.length ?? 0;
}

const api = {
  load() {},
  save: function () {
    return a || b;
  },
  "load-data": () => {},
  nested: {
    deep() {},
  },
};

function outer() {
  if (a) {
    run();
  }
  function inner() {
    if (b) {
      run();
    }
    if (c) {
      run();
    }
  }
}

function process(orders: Order[]) {
  return orders.filter((o) => o.paid && o.shipped);
}

const bound = function ownName() {
  return a ? 1 : 2;
};

call(function callbackWithName() {
  if (a) {
    run();
  }
});

function factory() {
  return class Widget {
    ready = flag ?? false;
    render() {
      return a && b;
    }
  };
}

function parse(x: string): string;
function parse(x: number): number;
function parse(x: any): any {
  if (typeof x === "string") {
    return x;
  }
  return x;
}

abstract class Reader {
  abstract next(): void;
  read(x: string): void;
  read(x: number): void;
  read(x: any): void {
    if (x) {
      run();
    }
  }
}

describe("module level", () => {
  it("is not measured", () => {
    if (a) {
      run();
    }
  });
});

if (topLevel) {
  run();
}

export default function () {
  if (a) {
    run();
  }
}
