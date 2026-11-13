# Spec Delta

## MODIFIED Requirements

### Requirement: Markdown table cells

Each table cell SHALL render as the plain text it holds, whatever the
finding's path, message or rule contains:
- a character that is Markdown syntax, a table delimiter or HTML SHALL
  be escaped, so it neither starts markup nor ends the cell;
- a line break SHALL become a space.

The Location cell SHALL be `<path>:<line>`, or `<path>` for a finding
without a line. A finding's related locations SHALL be listed in its
Finding cell after the message, except one whose `<path>:<line>` the
message already names: at the start of the message or after
whitespace, and not followed by a digit.

#### Scenario: A message with table and HTML characters

- **GIVEN** a finding whose message is `a | b <script> *c*`
- **WHEN** the Markdown report is rendered
- **THEN** its row still has four cells, and the Finding cell renders as the text `a | b <script> *c*`

#### Scenario: A duplicate with its copies

- **GIVEN** a duplicate-block finding with two related locations
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell holds the message followed by both related locations as `<path>:<line>`

#### Scenario: A copy the message names

- **GIVEN** a duplicate-block finding whose message is `duplicates src/grade.ts:1-19 (lines 1-19, 85 tokens)` and whose only related location is `src/grade.ts` line 1
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell holds the message alone, without "also at"

#### Scenario: A similar location the message does not name

- **GIVEN** a finding whose message names `src/a.ts:10-19` and whose related locations are `src/a.ts` line 1 and `src/a.ts` line 10
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell lists `src/a.ts:1` after the message, and not `src/a.ts:10`

#### Scenario: A path the message names only as part of a longer path

- **GIVEN** a finding whose message names `lib/src/a.ts:1-6` and whose related locations are `lib/src/a.ts` line 1 and `src/a.ts` line 1
- **WHEN** the Markdown report is rendered
- **THEN** the Finding cell lists `src/a.ts:1` after the message, and not `lib/src/a.ts:1`
