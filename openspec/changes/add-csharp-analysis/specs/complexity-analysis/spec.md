# Spec Delta

## MODIFIED Requirements

### Requirement: Cyclomatic complexity of a function

The system SHALL measure the cyclomatic complexity of a function unit as 1
plus the number of decision points in its body, following ESLint's
`complexity` rule except where this spec says otherwise. The requirements
on units, names and decision points in this spec apply to TypeScript; for
C#, the `csharp-analysis` capability defines them. Matching, the rules
and the findings apply to both.

#### Scenario: Straight-line function

- **WHEN** a function contains no decision points
- **THEN** its complexity is 1

#### Scenario: Compound condition

- **WHEN** a function body is `if (a && b) { x(); } else if (c) { y(); } else { z(); }`
- **THEN** its complexity is 4 (two `if`, one `&&`; the plain `else` does not count)

#### Scenario: A C# method that got more complex

- **GIVEN** the before version of `src/Cart.cs` has `Cart.Total(int)` with complexity 3, and the after version adds two `if` statements to it
- **WHEN** the two versions are compared
- **THEN** a Warning `complexity/cyclomatic-increase` finding reports `Cart.Total(int): cyclomatic complexity 3 -> 5 (+2)`
