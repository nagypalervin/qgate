# Proposal: add-forbidden-imports

## Why

qgate promises to catch architecture erosion (README, "Why"), but today it
only measures complexity. The most common erosion is a new import across a
boundary: domain code that starts importing the UI, or a database client
showing up where it should not. This is the first M2 change: it adds the
first architecture rule and, with it, `qgate.toml`.

## What Changes

- **New configuration file `qgate.toml`** at the repository root. It has
  one section for now, `[architecture]`. Revision mode reads it from the
  head revision (`<head>:qgate.toml`), through git, not from the working
  tree.
  - A missing file means no architecture rules; everything else works as
    today.
  - An invalid file exits with 2 and names the problem. Invalid means a
    TOML syntax error, an unknown key, a wrong type, an empty or duplicate
    rule name, an empty pattern list, or an invalid pattern.
  - The two-file form does not read it.
- **Forbid rules:**

  ```toml
  [[architecture.forbid]]
  name = "domain-is-pure"
  from = ["src/domain/**"]
  to   = ["src/ui/**", "src/infra/**", "react"]
  ```

  - **`from`** matches the path of the importing file.
  - **`to`** matches the import target.
  - **Patterns** are globs relative to the repository root. A pattern
    without glob characters matches itself and everything below it
    (`react` matches `react/jsx-runtime`), and `dir/**` also matches `dir`.
- **Imports that count:**
  - `import`, including `import type` and side-effect imports;
  - `export ... from`;
  - `import x = require(...)`;
  - `import(...)` and `require(...)` with a string literal.
- **Import targets:**
  - a relative specifier (`./`, `../`) is resolved against the importing
    file's directory to a path from the repository root;
  - any other specifier is matched as written (`react`, `@/ui/Button`).
- **New rule `architecture/forbidden-import`, always Error.** It reports a
  forbidden import only if the file's base version did not already import
  the same target against the same rule. For a renamed file, the base
  version is checked at its old path. Message:

  ```
  error   src/domain/order.ts:3  imports src/ui/Button ("../ui/Button"), forbidden by "domain-is-pure"  [architecture/forbidden-import]
  ```

### Decisions taken during exploration

- **The architecture rule comes before general configuration.** The file
  format grows from a real need. Thresholds and the `--fail-on` default can
  move into `qgate.toml` later.
- **Deny lists, not layers with allowed directions.** A repository can
  start with one line, and packages fit the same form. Named layers can
  later generate the same deny rules.
- **Type-only imports count.** Domain code depending on UI types is still
  coupled to the UI.
- **No extension probing and no tsconfig `paths`.** Directory patterns do
  not need probing. Users who import through aliases list the alias form
  too (`"@/ui/**"`). This keeps the check deterministic and free of
  file-system and tsconfig resolution.
- **The head revision's `qgate.toml`.** A pull request can loosen its own
  rules, but it can equally edit the CI workflow. qgate is a quality gate,
  not a security boundary, and the change to `qgate.toml` is visible in
  review. In return, new rules can be tried before merging, and the rules
  always describe the revision under analysis.
- **Unknown keys are errors**, so a typo such as `[[architecture.forbd]]`
  cannot silently switch a rule off.
- **"New" is per file, rule and target.** Moving a file within a layer
  reports nothing, and moving a file that imports the UI into the domain
  reports the imports.

### Out of scope

- Thresholds, `--fail-on` and other settings in `qgate.toml`.
- Named layers, allow lists, and per-rule severity.
- Import cycles, which need the whole dependency graph, not only changed
  files.
- tsconfig `paths` and `baseUrl`, extension and `index` probing, and
  `package.json` `imports`/`exports` maps.
- Non-literal dynamic imports (`` import(`./${x}`) ``).
- A `--config` option, or searching for `qgate.toml` outside the
  repository root.
- Architecture rules in the two-file form.
- Duplication detection, the other half of M2.

## Capabilities

### New Capabilities

- `configuration`: `qgate.toml`. Where it is read from and from which
  revision, what a missing or invalid file does, and that the two-file form
  ignores it.
- `architecture-analysis`: which imports count, how targets are resolved,
  how forbid patterns match, what makes a violation new, and the
  `architecture/forbidden-import` finding.

### Modified Capabilities

None. `check-command` and `revision-input` keep their requirements. The
new exit-2 case belongs to `configuration`.

## Impact

- **`qgate-lang`:** `FileModel` gains the file's imports (specifier and
  line).
- **New crate `qgate-config`:** reads `qgate.toml` into plain data. It is
  a leaf crate, like `qgate-git`. This needs ADR 0003 and an update of the
  dependency rules in `CLAUDE.md` and `.claude/agents/reviewer.md`
  (`cli -> config`).
- **`qgate-analyzers`:** a new `architecture` analyzer with its own rule
  types and pattern matching. `FileChange` gains the old path of a renamed
  file.
- **`qgate-git`:** reads one file at a revision (`qgate.toml` at the
  head).
- **`qgate-cli`:** reads the configuration in revision mode and runs the
  architecture analyzer when rules exist.
- **New dependencies, measured in a throwaway crate:**
  - `toml` 1.x (features `parse`, `serde`, `std`) adds 5 built crates
    (`toml`, `toml_parser`, `toml_datetime`, `serde_spanned`, `winnow`);
    `serde` with `derive` was already in the lock (corrected at the PR 2
    review; the first count, about 11, came from a standalone crate);
  - `globset` adds 2 (`globset`, `bstr`); its regex crates are already in
    the tree through tree-sitter.
- **Docs:** README (rules table, a `qgate.toml` section), the crate docs.
