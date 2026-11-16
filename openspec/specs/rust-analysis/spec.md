# rust-analysis Specification

## Purpose
Rust analysis defines how qgate reads Rust source: which files it reads,
which units it measures and how it names them, what counts as a decision
point, and which tokens it compares to find copies.

## Requirements

### Requirement: Rust files

A file whose name ends in `.rs` SHALL be read as Rust. A UTF-8 byte
order mark at its start SHALL be ignored.

#### Scenario: A plain source file

- **GIVEN** `src/cart.rs` holds `fn total(n: u32) -> u32 { if n > 0 { n } else { 0 } }`
- **WHEN** the file is analyzed
- **THEN** it has one unit `total` with complexity 2

#### Scenario: A file with a byte order mark

- **GIVEN** `src/cart.rs` starts with a UTF-8 byte order mark and holds a function with one `if`
- **WHEN** the file is analyzed
- **THEN** the function has complexity 2 and no `input/unparsable` finding is reported

### Requirement: Rust function units

Every `fn` item with a body SHALL be a function unit: free functions,
associated functions and methods in `impl` blocks, default methods in
traits, and `fn` items nested in another function or in an inline
module. A `fn` without a body (a trait method declaration, a function
in an `extern` block) SHALL NOT be a unit.

#### Scenario: The items of a file

- **WHEN** a file has a free function, an `impl` block with `new` and one method, a trait with one declared and one default method, and a `fn` item inside the free function
- **THEN** each of them except the declared trait method is measured separately: five units

#### Scenario: Declarations without a body

- **WHEN** a file holds a trait with two declared methods and an `extern "C"` block with one function
- **THEN** no function unit is found

### Requirement: Rust closures count towards their named ancestor

A closure SHALL NOT be a unit of its own; its decision points SHALL
count towards the nearest enclosing unit. Code outside units, such as
the initializers of `const` and `static` items outside functions, SHALL
NOT be measured; a `const` or `static` inside a function SHALL count
towards it.

#### Scenario: An iterator chain in a function

- **GIVEN** `fn paid(o: &[Order]) -> usize { o.iter().filter(|x| x.paid && !x.void).count() }`
- **WHEN** the file is analyzed
- **THEN** the only unit is `paid` with complexity 2

#### Scenario: A static initializer

- **GIVEN** a file whose only code is `static LIMIT: LazyLock<u32> = LazyLock::new(|| if cfg!(test) { 1 } else { 9 });`
- **WHEN** the file is analyzed
- **THEN** no function unit is found

#### Scenario: A constant inside a function

- **GIVEN** `fn limit() -> u32 { const L: u32 = if cfg!(test) { 1 } else { 9 }; L }`
- **WHEN** the file is analyzed
- **THEN** the only unit is `limit` with complexity 2, and changing `9` to `8` changes the unit

### Requirement: Rust qualified names

A unit's name SHALL be the names of its enclosing inline modules, its
`impl` or trait, and enclosing units, joined by `.`, followed by its own
name. An inherent `impl` SHALL be named by its type, a trait `impl` by
`<Type as Trait>`, a trait by its name; types and traits as written,
with type arguments and without whitespace. File modules SHALL NOT be
part of the name.

#### Scenario: Inherent and trait implementations

- **GIVEN** `mod shop { pub struct Cart; impl Cart { fn total(&self) -> u32 { 0 } } impl fmt::Display for Cart { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) } } impl fmt::Debug for Cart { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { Ok(()) } } }`
- **WHEN** the file is analyzed
- **THEN** the units are named `shop.Cart.total`, `shop.<Cart as fmt::Display>.fmt` and `shop.<Cart as fmt::Debug>.fmt`

#### Scenario: Generic implementations

- **GIVEN** `impl<T: Clone> Stack<T> { fn push(&mut self, x: T) {} }`, `impl From<u8> for Id { fn from(v: u8) -> Self { Id } }` and `impl From<HashMap<String, u32>> for Id { fn from(v: HashMap<String, u32>) -> Self { Id } }`
- **WHEN** the file is analyzed
- **THEN** the units are named `Stack<T>.push`, `<Id as From<u8>>.from` and `<Id as From<HashMap<String,u32>>>.from`

#### Scenario: Traits, nested functions and test modules

- **GIVEN** `trait Shape { fn area(&self) -> f64 { 0.0 } } fn outer() { fn inner() {} } #[cfg(test)] mod tests { #[test] fn adds_two() {} }`
- **WHEN** the file is analyzed
- **THEN** the units are named `Shape.area`, `outer`, `outer.inner` and `tests.adds_two`

#### Scenario: One name twice

- **GIVEN** `#[cfg(unix)] fn home() -> String { a() }` followed by `#[cfg(windows)] fn home() -> String { b() }`
- **WHEN** the file is analyzed
- **THEN** the units are named `home` and `home#2`

### Requirement: Rust decision points

The system SHALL count one decision point for each: `if` expression,
with `if let`; `while` loop, with `while let`; `for` loop; `let` with
an `else` block; `match` arm whose pattern is not the wildcard `_`;
match guard; `|` between alternatives of a pattern; operator `&&` and
`||`. The `?` operator, `loop`, `break`, `continue` and `return` SHALL
NOT count.

#### Scenario: Match

- **WHEN** a function is `fn f(d: u32) -> u32 { match d { 0 => 1, n if n < 50 => 2, _ => 3 } }`
- **THEN** its complexity is 4 (two arms and one guard; the `_` arm does not count)

#### Scenario: Alternatives in a pattern

- **WHEN** a function is `fn f(c: char) -> bool { match c { 'a' | 'e' => true, _ => false } }`
- **THEN** its complexity is 3 (one arm and one `|`)

#### Scenario: Let else and loops

- **WHEN** a function contains `let Some(x) = o else { return };`, one `while let` loop, one `for` loop and one `loop` with a `break`
- **THEN** its complexity is 4

#### Scenario: The question mark does not count

- **WHEN** a function is `fn read(p: &Path) -> io::Result<u32> { let s = fs::read_to_string(p)?; let n = s.trim().parse().map_err(io::Error::other)?; Ok(n) }`
- **THEN** its complexity is 1

### Requirement: Macros in Rust

The arguments of a macro call and the body of a `macro_rules!`
definition SHALL NOT be read as code: their decision points SHALL NOT
count and a `macro_rules!` definition SHALL NOT be a unit. Their tokens
SHALL still be compared, normalized like other code.

#### Scenario: A condition inside an assertion

- **WHEN** a function is `fn check(a: bool, b: bool) { assert!(a && b, "both"); }`
- **THEN** its complexity is 1

#### Scenario: Copies inside macro calls

- **GIVEN** two functions whose bodies differ only in names and values, each a `println!` call with the same format and arguments of the same shape
- **THEN** both have the same normalized tokens

### Requirement: Rust normalized tokens

In Rust, these rules SHALL replace the TypeScript ones of
`duplication-analysis`. Identifiers and lifetimes SHALL become one
placeholder, except a field or method name after `.` (a tuple field
such as `.0` too) and a path segment after `::`, which keep their text. Each string, raw string, byte string,
character and number literal SHALL become another. Keywords, including
primitive types, `self`, `Self`, `true` and `false`, operators and other
punctuation SHALL keep their text.

#### Scenario: Renamed copy

- **GIVEN** `fn fmt_date(d: &Date) -> String { let y = d.year; format!("{y}-") }` and `fn format_order_date(when: &Date) -> String { let year = when.year; format!("{year}/") }`
- **THEN** both have the same normalized tokens

#### Scenario: Different members and paths

- **GIVEN** two functions that differ only in reading `.year` and `.month`, or in calling `Vec::new()` and `Vec::default()`
- **THEN** their normalized tokens differ

#### Scenario: Tuple fields

- **GIVEN** two functions that differ only in reading `self.0` and `self.1`
- **THEN** their normalized tokens differ

### Requirement: Rust code without tokens

`use` declarations, `extern crate` items, module declarations without a
body (`mod name;`), the keyword and name of an inline module, outer and
inner attributes, comments (doc comments included) and semicolons
SHALL produce no tokens.

#### Scenario: Imports and attributes are not compared

- **GIVEN** two files that share only a long list of `use` declarations, `mod` declarations and `#![allow(...)]` and `#[derive(...)]` attributes
- **THEN** no tokens are compared for them and no duplicate is found

#### Scenario: Doc comments

- **GIVEN** two functions that differ only in their `///` doc comments
- **THEN** their normalized tokens are the same
