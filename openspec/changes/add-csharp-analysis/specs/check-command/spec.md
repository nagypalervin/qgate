# Spec Delta

## MODIFIED Requirements

### Requirement: Supported input files

In the two-file mode the system SHALL accept files with the extensions
`.ts`, `.mts`, `.cts`, `.tsx` and `.cs`, and SHALL reject any other file
with an error naming the file and exit code 2. The two files SHALL be
accepted even if their extensions differ, as long as both are in this
set and both are C# or neither is; a C# file compared with a TypeScript
file SHALL be rejected with an error naming both files and exit code 2.

#### Scenario: TSX input

- **WHEN** the user runs `qgate check before.tsx after.tsx` on valid TSX files that produce no Error finding
- **THEN** the analysis runs and the exit code is 0

#### Scenario: C# input

- **WHEN** the user runs `qgate check Before.cs After.cs` on valid C# files that produce no Error finding
- **THEN** the analysis runs and the exit code is 0

#### Scenario: C# compared with TypeScript

- **WHEN** the user runs `qgate check before.ts after.cs`
- **THEN** standard error contains an error that names `before.ts` and `after.cs`
- **AND** standard output is empty
- **AND** the exit code is 2

#### Scenario: Unsupported extension

- **WHEN** the user runs `qgate check before.js after.js`
- **THEN** standard error contains an error that names `before.js` as an unsupported file type
- **AND** standard output is empty
- **AND** the exit code is 2
