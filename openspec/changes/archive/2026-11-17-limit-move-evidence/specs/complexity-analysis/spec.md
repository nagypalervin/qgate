## ADDED Requirements

### Requirement: Limit of the move evidence

An after unit and a removed unit whose own tokens, without their common
start and end, form more than 2^26 pairs SHALL NOT be partners by the
tokens they share in order. They SHALL still be matched when they have
the same code. The time and memory of the move evidence stay bounded.

#### Scenario: A long moved function changed at both ends

- **GIVEN** the base revision has `Tests.Run(int)` with complexity 12, and the head revision removes it
- **WHEN** the head revision adds `Helpers.Run(int)` with complexity 12 in another file, whose own tokens share more than half of each unit's own tokens in order with those of `Tests.Run(int)`, but differ from them between the first changed token and the last over more than 8,192 tokens in each unit
- **THEN** `Helpers.Run(int)` is new, and a Warning `complexity/cyclomatic-new` finding is reported for it

#### Scenario: A long moved function changed in one place

- **GIVEN** the base revision has `Tests.Run(int)` with complexity 12 and more than 8,192 own tokens, and the head revision removes it
- **WHEN** the head revision adds `Helpers.Run(int)` in another file, with the same code except one changed token
- **THEN** `Helpers.Run(int)` is matched with `Tests.Run(int)`, and no finding is reported for it

#### Scenario: A long function moved unchanged

- **GIVEN** the base revision has `Tests.Run(int)` with complexity 12 and more than 8,192 own tokens, and the head revision removes it
- **WHEN** the head revision adds `Helpers.Run(int)` with the same code in another file
- **THEN** `Helpers.Run(int)` is matched with `Tests.Run(int)`, and no finding is reported for it
