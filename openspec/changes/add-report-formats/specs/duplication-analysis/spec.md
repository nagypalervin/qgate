## MODIFIED Requirements

### Requirement: Duplicate block finding

Each reported block SHALL be one finding with rule
`duplication/duplicate-block` and severity Warning, at the path and first
line of the new occurrence. The message SHALL be `duplicates
<path>:<first>-<last> (lines <first>-<last>, <n> tokens)`, naming the
first file by path whose copy covers the whole block and is not itself
reported as a new copy of it; with `k` further such files, `, and <k>
more` SHALL end the parentheses. The finding SHALL span the new
occurrence's lines and name, as related locations in path order, the
first copy of the whole block by line in each such file, with the lines
it covers.

#### Scenario: Message

- **GIVEN** `src/utils/date.ts` lines 3-9 hold a 54-token block, and the change adds a renamed copy at lines 12-18 of `src/orders/format.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** standard output contains `warning src/orders/format.ts:12  duplicates src/utils/date.ts:3-9 (lines 12-18, 54 tokens)  [duplication/duplicate-block]`
- **AND** the exit code is 0

#### Scenario: Several existing copies

- **GIVEN** the block of the previous scenario also exists in `src/a.ts` and `src/b.ts`
- **WHEN** the change adds another copy to `src/orders/format.ts`
- **THEN** the message names `src/a.ts` and ends with `54 tokens, and 2 more)`

#### Scenario: Failing on duplication

- **WHEN** the user runs `qgate check --base main --fail-on warning` on a change that adds a duplicate block
- **THEN** the exit code is 1

#### Scenario: Range and related locations

- **GIVEN** the block of the "Several existing copies" scenario
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** the finding has `"line":12,"end_line":18`
- **AND** its `related` array names `src/a.ts`, then `src/b.ts`, then `src/utils/date.ts`, each with the first and last line of its copy
