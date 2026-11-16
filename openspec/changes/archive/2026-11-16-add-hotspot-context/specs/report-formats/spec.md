# Spec Delta

## ADDED Requirements

### Requirement: Hotspot in the JSON report

A finding with hotspot context (see `hotspot-analysis`) SHALL have a
`hotspot` key in the JSON report: an object with the integers `commits`
and `complexity`. Any other finding SHALL have no `hotspot` key, never
`null`. The key is new within `schema_version` 1.

#### Scenario: A finding in a hotspot

- **GIVEN** the change of "A complexity increase in a hotspot" in `hotspot-analysis`
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding is `{"rule":"complexity/cyclomatic-increase","severity":"warning","message":"Cart.Total(): cyclomatic complexity 4 -> 6 (+2); hotspot: 31 commits in 6 months, complexity 87","path":"src/Cart.cs","line":12,"metric":{"before":4,"after":6},"hotspot":{"commits":31,"complexity":87}}`

#### Scenario: A finding outside hotspots

- **GIVEN** a complexity increase in a file that is not a hotspot
- **WHEN** the user runs `qgate check --base main --format json`
- **THEN** its finding has no `hotspot` key
