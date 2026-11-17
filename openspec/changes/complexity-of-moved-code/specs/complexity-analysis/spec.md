## MODIFIED Requirements

### Requirement: Matching functions between versions

The system SHALL match a unit in the after file with the unit of the same
qualified name in the before file. A unit with no such match SHALL be
matched as a moved unit where "Moved functions" allows it; otherwise it
is new. A unit in the before file that is matched by neither is removed.

#### Scenario: Renamed function

- **GIVEN** `calc` in the before file is renamed to `calculateTotal` in the after file, with the same body
- **WHEN** the two files are compared
- **THEN** `calc` is treated as removed and `calculateTotal` as new

### Requirement: Finding content

Every complexity finding SHALL carry the before and after values, point to
the line of the unit's name in the after file (or of the function itself for
an unnamed default export), and use the message
`<name>: cyclomatic complexity <before> -> <after> (+<delta>)` for an
increase and `<name>: new function, cyclomatic complexity <after>` for a new
unit. An increase of a moved unit SHALL append `, moved from <old name>`.
An increase with inlined units SHALL write the parenthesis as
`(+<delta>, +<growth> beyond the inlined <names>)`, the names joined by
`, ` in their order in the before version.

#### Scenario: Increase message

- **WHEN** `applyDiscount`, named on line 88 of the after file, goes from 4 to 5
- **THEN** the finding points to line 88 with message `applyDiscount: cyclomatic complexity 4 -> 5 (+1)`

#### Scenario: New function message

- **WHEN** a new function `buildInvoice` with complexity 14 is named on line 120
- **THEN** the finding points to line 120 with message `buildInvoice: new function, cyclomatic complexity 14`

#### Scenario: Moved function message

- **WHEN** `Tests.Run(int)` moves to `Helpers.Run(int)` and goes from 12 to 14
- **THEN** the message is `Helpers.Run(int): cyclomatic complexity 12 -> 14 (+2), moved from Tests.Run(int)`

#### Scenario: Inlined function message

- **WHEN** `decryptMany` goes from 4 to 15 and inlines `decryptManyV2`, which had complexity 10
- **THEN** the message is `decryptMany: cyclomatic complexity 4 -> 15 (+11, +2 beyond the inlined decryptManyV2)`, with before value 4 and after value 15

## ADDED Requirements

### Requirement: Moved functions

Units without a match by qualified name SHALL be grouped by own name,
the last segment of the qualified name (for C#, with its parameter
types), across one language family's changed and deleted files; units
of a deleted JavaScript file are left out. An after unit and a removed
unit of a group SHALL be matched when they have the same code, or else
when at least half of the removed unit's tokens appear in order in the
after unit, provided neither has another such partner.

#### Scenario: Method moved to a class in another file

- **GIVEN** `src/Tests.cs` has `Tests.Run(int)` with complexity 12 at the base revision
- **WHEN** the head revision moves it unchanged into a new class `Helpers` in a new file `src/Helpers.cs`
- **THEN** `Helpers.Run(int)` is matched with `Tests.Run(int)`, and no finding is reported for it

#### Scenario: Moved and grown

- **GIVEN** the same move, and the head revision adds two `if` statements to the method
- **WHEN** the revisions are compared
- **THEN** a Warning `complexity/cyclomatic-increase` finding is reported for `Helpers.Run(int)`, from 12 to 14

#### Scenario: Moved out of a deleted file

- **GIVEN** `src/legacy.ts` has `Legacy.load` with complexity 15 at the base revision
- **WHEN** the head revision deletes the file and has `Store.load` with the same code in `src/store.ts`
- **THEN** no finding is reported for `Store.load`

#### Scenario: Moved out of a deleted JavaScript file

- **GIVEN** `src/legacy.js` has a function `load` with complexity 15 at the base revision
- **WHEN** the head revision deletes the file and has `Store.load` with the same code in `src/store.ts`
- **THEN** `Store.load` is new, and a Warning `complexity/cyclomatic-new` finding with complexity 15 is reported for it

#### Scenario: Moved to another class in the same file

- **GIVEN** `Cart.total` with complexity 11 in `src/cart.ts`
- **WHEN** the head revision moves it unchanged into a class `Pricing` in the same file
- **THEN** no finding is reported for `Pricing.total`

#### Scenario: Same own name, different code

- **GIVEN** the base revision has `AComponent.ngOnInit` with complexity 15, and the head revision removes it
- **WHEN** the head revision adds `BComponent.ngOnInit` with complexity 14, in which less than half of the tokens of `AComponent.ngOnInit` appear in order
- **THEN** `BComponent.ngOnInit` is new, and a Warning `complexity/cyclomatic-new` finding is reported for it

#### Scenario: Ambiguous own name

- **GIVEN** the base revision has `A.load` and `B.load`, each with complexity 12, and the head revision removes both
- **WHEN** the head revision adds `C.load` with complexity 12, whose code differs from both by one operator
- **THEN** `C.load` is new, and a Warning `complexity/cyclomatic-new` finding is reported for it

#### Scenario: Same code settles an ambiguous own name

- **GIVEN** the base revision has `A.load` with complexity 12 and `B.load` with complexity 3, and the head revision removes both
- **WHEN** the head revision adds `C.load` with the code of `A.load`
- **THEN** `C.load` is matched with `A.load`, and no finding is reported for it

#### Scenario: Another language family

- **GIVEN** the base revision removes a TypeScript function `run` with complexity 12
- **WHEN** the head revision adds a Rust function `run` with complexity 12
- **THEN** the Rust function is new, and a Warning `complexity/cyclomatic-new` finding is reported for it

### Requirement: Inlined functions

A removed unit SHALL count as inlined into a matched unit that got more
complex when at least half of the removed unit's tokens appear, in
order, in the matched unit's after version. Its complexity minus 1 is
then added to the before value for the severity. When the after value
does not exceed that sum, no finding SHALL be reported. A removed unit
SHALL count as inlined into at most one unit.

#### Scenario: Inlined behind a feature flag

- **GIVEN** `decryptMany` with complexity 4 chooses between `decryptManyV1` and `decryptManyV2` (complexity 10)
- **WHEN** the head revision removes the choice and both functions, and `decryptMany` now holds the code of `decryptManyV2`, with complexity 12
- **THEN** no finding is reported for `decryptMany`

#### Scenario: Inlined and grown beyond it

- **GIVEN** the same base revision
- **WHEN** `decryptMany` holds the code of `decryptManyV2` plus three more `if` statements, with complexity 15
- **THEN** a Warning `complexity/cyclomatic-increase` finding is reported for `decryptMany`, for the growth of 2

#### Scenario: Removed but not inlined

- **GIVEN** `process` with complexity 3 and `legacy` with complexity 9 in the base revision
- **WHEN** the head revision removes `legacy` and adds five `if` statements of other code to `process`
- **THEN** an Error `complexity/cyclomatic-increase` finding is reported for `process`, from 3 to 8 (+5)
