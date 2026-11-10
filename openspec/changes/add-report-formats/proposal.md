# Proposal: add-report-formats

## Why

M3 puts qgate inside the coding loop: an agent hook or an MCP server runs
`qgate check --base main --worktree` and hands the result to an AI coding
agent, which then fixes what got worse. Today the only output is text for
people. An agent, a hook script or a CI step has to parse it with regular
expressions, and it carries no structured information about where a
duplicated block's original is. Agent hooks (`add-agent-hooks`) and the MCP
server (`add-mcp-server`) both need a machine-readable report first.

## What Changes

- **New option `--format <format>`** for `qgate check`, in every form (two
  files, revisions, working tree):
  - `text`, the default: the current output, byte for byte;
  - `json`: one JSON document on standard output.
- **The JSON report is a public, versioned format**, the same one the MCP
  server will return later:
  - an envelope with `schema_version: 1`, the findings and a summary;
  - every finding with its rule id, severity, message (the same text as in
    the text format), path, line, and, where they exist, an end line, the
    measured values before and after, and related locations;
  - fields are only ever added within a version. Removing or changing one
    needs a new `schema_version`.
- **Findings carry more structure.** This is visible only in the JSON
  report; the text format does not change:
  - a finding can span lines (an end line);
  - a finding can name related locations;
  - a duplicate block finding uses both: the range of the new copy, and
    every existing copy it duplicates, one per file.
- **Summary:** the number of findings, how many fail the run at the
  `--fail-on` level, and that level. An agent can tell why a run failed
  without reading the exit code.
- **Unchanged:**
  - the exit codes and the `--fail-on` behaviour;
  - errors still go to standard error as text, with empty standard
    output and exit code 2, in both formats;
  - the order of the findings.
- **Unknown format values are usage errors (exit 2),** like unknown
  `--fail-on` levels.
- **Docs:** a "JSON output" section in the README, and a JSON Schema file
  for the format in the repository, so hooks and scripts can validate
  against it.

### Decisions taken during exploration

- **One format for every consumer.** Hooks, the MCP server and scripts all
  get the same document. One format to version and test, instead of a CLI
  format plus a different MCP response.
- **`--format`, not `--json`.** Markdown (pull request comments) and SARIF
  follow in M4, and they fit the same option.
- **One JSON document with an envelope**, not a bare array or JSON Lines.
  The envelope can grow without breaking readers, and the summary has a
  place.
- **Related locations from version 1.** For duplication the original is
  what an agent needs most: it can extract a shared helper instead of
  deleting its own code. Adding the field later would be compatible, but
  hooks written in the meantime would not use it.
- **Architecture findings get no related location.** The forbidden target
  is a module name or an extensionless path, not a place in a file. A
  structured `details` field can come later, compatibly.
- **Errors are not JSON.** The check-command spec already defines errors
  as text on standard error with empty standard output. The MCP server
  will map errors itself.

### Out of scope

- Markdown and SARIF (M4).
- Agent hooks (`add-agent-hooks`) and the MCP server (`add-mcp-server`).
- Structured details for architecture and complexity findings beyond the
  metric (a later, compatible addition).
- Errors as JSON.

## Capabilities

### New Capabilities

- `report-formats`: the machine-readable report formats and the
  `--format` option that selects them. It starts with the JSON format: its
  envelope, finding fields, versioning rules and summary. Markdown and
  SARIF join it in M4. The text format stays in `check-command` ("Finding
  line format", "No regressions"), where it is specified today.

### Modified Capabilities

- `check-command`:
  - **"Invalid invocation":** an unknown `--format` value is a usage
    error.
  - **"Failing on findings":** standard output stays the same at every
    `--fail-on` level, except the JSON summary, which names the level and
    counts the failing findings.
- `duplication-analysis`:
  - **"Duplicate block finding":** the finding spans the new copy's lines
    and names every existing copy as a related location, one per file.

## Impact

- **`qgate-core`:**
  - `Location` gains an optional end line;
  - `Finding` gains a list of related locations.

  Every place that builds them changes (about 10, all in the workspace).
  Still no I/O and no dependency.
- **`qgate-analyzers`:** the duplication analyzer fills in the end line and
  the related locations. The other analyzers set neither.
- **`qgate-report`:** a JSON renderer. **New dependency `serde_json`**, for
  correct escaping and number output in a public format; it has few
  transitive dependencies, and `serde` is already in the workspace.
- **`qgate-cli`:** the `--format` option and its usage error.
- **Specs:** `report-formats` (new), `check-command`, `duplication-analysis`
  (deltas).
- **Docs:** README, and the JSON Schema file.
- **Users:** none affected. The text output and the exit codes do not
  change, and the option is new.
