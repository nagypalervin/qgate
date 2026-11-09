// Fixture: duplicate names.

const o = {
  "x#2"() {},
  x() {},
  x() {},
};

class A {
  static f() {
    function g() {}
  }
  f() {
    function g() {}
  }
}
