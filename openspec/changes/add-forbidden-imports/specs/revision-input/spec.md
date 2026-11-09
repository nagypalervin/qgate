# Spec Delta

## MODIFIED Requirements

### Requirement: Renamed files

A file that git detects as renamed SHALL be compared with its version under
the old path, and findings SHALL use the new path. If the old path has no
supported extension, the file SHALL be treated as added, except that a
JavaScript version (`.js`, `.jsx`, `.mjs`, `.cjs`) supplies its imports
for the architecture rules; if that version cannot be read, no import of
the file counts as new, wherever it moved.

#### Scenario: Renamed and modified

- **GIVEN** the head renames `src/old.ts` to `src/new.ts` and makes `f` in it more complex
- **WHEN** the user runs `qgate check --base main`
- **THEN** a `complexity/cyclomatic-increase` finding is reported for `f` in `src/new.ts`

#### Scenario: Renamed from an unsupported extension

- **GIVEN** the head renames `src/util.coffee` or `src/util.js` to `src/util.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** every function in `src/util.ts` is analyzed as new

#### Scenario: Renamed from JavaScript that cannot be read

- **GIVEN** a forbid rule, and the head renames `src/domain/order.js`, which is not valid UTF-8 and imports a forbidden target, to `src/domain/order.ts`
- **WHEN** the user runs `qgate check --base main`
- **THEN** no `architecture/forbidden-import` finding is reported for `src/domain/order.ts`
- **AND** its functions are analyzed as new
