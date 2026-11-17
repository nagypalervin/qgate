## MODIFIED Requirements

### Requirement: Rust qualified names

A unit's name SHALL be the names of its enclosing inline modules, its
`impl` or trait, and enclosing units, joined by `.`, followed by its own
name. An inherent `impl` SHALL be named by its type, a trait `impl` by
`<Type as Trait>`, a trait by its name; types and traits as written,
with type arguments, without lifetimes, comments and whitespace. File
modules SHALL NOT be part of the name.

#### Scenario: Inherent and trait implementations

- **GIVEN** `mod shop { pub struct Cart; impl Cart { fn total(&self) -> u32 { 0 } } impl fmt::Display for Cart { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) } } impl fmt::Debug for Cart { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) } } }`
- **WHEN** the file is analyzed
- **THEN** the units are named `shop.Cart.total`, `shop.<Cart as fmt::Display>.fmt` and `shop.<Cart as fmt::Debug>.fmt`

#### Scenario: Generic implementations

- **GIVEN** `impl<T: Clone> Stack<T> { fn push(&mut self, x: T) {} }`, `impl From<u8> for Id { fn from(v: u8) -> Self { Id } }` and `impl From<HashMap<String, u32>> for Id { fn from(v: HashMap<String, u32>) -> Self { Id } }`
- **WHEN** the file is analyzed
- **THEN** the units are named `Stack<T>.push`, `<Id as From<u8>>.from` and `<Id as From<HashMap<String,u32>>>.from`

#### Scenario: Lifetimes are not part of the name

- **GIVEN** `impl<'s> Walker<'s> { fn step(&mut self) {} }`, `impl Walker<'_> { fn stop(&mut self) {} }`, `impl<'a, T> Pair<'a, T> { fn first(&self) {} }`, `impl<'a> From<&'a str> for Id<'a> { fn from(v: &'a str) -> Self { Id(v) } }` and `impl Show for Box<dyn Fn() + 'static> { fn show(&self) {} }`
- **WHEN** the file is analyzed
- **THEN** the units are named `Walker.step`, `Walker.stop`, `Pair<T>.first`, `<Id as From<&str>>.from` and `<Box<dyn Fn()> as Show>.show`

#### Scenario: A comment inside a type

- **GIVEN** `impl Stack</* item */ T> { fn push(&mut self, x: T) {} }`
- **WHEN** the file is analyzed
- **THEN** the unit is named `Stack<T>.push`

#### Scenario: A lifetime renamed

- **GIVEN** a base revision with a method `step` of complexity 11 in `impl<'s> Walker<'s>`
- **WHEN** the head revision moves the method unchanged into `impl Walker<'_>`
- **THEN** `Walker.step` is the same unit in both revisions, and no finding is reported for it

#### Scenario: Traits, nested functions and test modules

- **GIVEN** `trait Shape { fn area(&self) -> f64 { 0.0 } } fn outer() { fn inner() {} } #[cfg(test)] mod tests { #[test] fn adds_two() {} }`
- **WHEN** the file is analyzed
- **THEN** the units are named `Shape.area`, `outer`, `outer.inner` and `tests.adds_two`

#### Scenario: One name twice

- **GIVEN** `#[cfg(unix)] fn home() -> String { a() }` followed by `#[cfg(windows)] fn home() -> String { b() }`
- **WHEN** the file is analyzed
- **THEN** the units are named `home` and `home#2`
