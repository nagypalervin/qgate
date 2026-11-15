# Spec Delta

## MODIFIED Requirements

### Requirement: Finding fields

Each finding SHALL have `rule`, `severity` (`error`, `warning` or
`info`), `message` and `path`, as the text format shows them, and when it
has them: `line`; `end_line`, the last line of a range (may equal `line`);
`metric`, with `before` (absent for a new item) and `after` (absent for a
removed one); `related`, a non-empty array of objects with `path`, `line`
and, for a range, `end_line`; `hotspot`, with the integers `commits` and
`complexity` (see `hotspot-analysis`). A key without a value SHALL be
left out, never `null`.

#### Scenario: New function

- **GIVEN** a new function `big` with complexity 12 on line 1 of the added `src/new.ts`
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding is `{"rule":"complexity/cyclomatic-new","severity":"warning","message":"big: new function, cyclomatic complexity 12","path":"src/new.ts","line":1,"metric":{"after":12}}`

#### Scenario: Finding without a line

- **GIVEN** `src/a.ts` is not valid UTF-8 in the head revision
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding has `"path":"src/a.ts"` and no `line`, `end_line`, `metric` or `related` key

#### Scenario: Duplicate block

- **GIVEN** `src/utils/date.ts` lines 1-6 hold a 68-token block, and the change adds a renamed copy at lines 4-9 of `src/orders/format.ts`
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding is `{"rule":"duplication/duplicate-block","severity":"warning","message":"duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)","path":"src/orders/format.ts","line":4,"end_line":9,"related":[{"path":"src/utils/date.ts","line":1,"end_line":6}]}`

#### Scenario: Characters JSON must escape

- **GIVEN** a forbidden import whose message contains `"` characters
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** the message is written with each `"` escaped as `\"`, and the output parses as JSON

#### Scenario: A finding in a hotspot

- **GIVEN** the change of "A complexity increase in a hotspot" in `hotspot-analysis`
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding is `{"rule":"complexity/cyclomatic-increase","severity":"warning","message":"Cart.Total(): cyclomatic complexity 4 -> 6 (+2); hotspot: 31 commits in 6 months, complexity 87","path":"src/Cart.cs","line":12,"metric":{"before":4,"after":6},"hotspot":{"commits":31,"complexity":87}}`
