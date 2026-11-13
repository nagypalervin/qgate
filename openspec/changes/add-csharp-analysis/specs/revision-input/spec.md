# Spec Delta

## MODIFIED Requirements

### Requirement: Only changed, supported files are analyzed

The system SHALL analyze only files that differ between the two compared
revisions and whose head path has the extension `.ts`, `.mts`, `.cts`,
`.tsx` or `.cs`, except generated C# files (see `csharp-analysis`).
Other files SHALL be ignored without a message. All changed files in
the repository count, regardless of the current directory.

#### Scenario: Unchanged and unsupported files

- **GIVEN** the head changes `src/a.ts` and `README.md`, and the repository also contains an unchanged, complex `src/b.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** only `src/a.ts` is analyzed

#### Scenario: C# files

- **GIVEN** the head changes `src/Cart.cs`, `src/Cart.Designer.cs` and `src/Shop.csproj`
- **WHEN** the user runs `qgate check --base main`
- **THEN** only `src/Cart.cs` is analyzed

#### Scenario: No changes

- **WHEN** the user runs `qgate check --base HEAD --head HEAD`
- **THEN** standard output is `No regressions found.`
- **AND** the exit code is 0
