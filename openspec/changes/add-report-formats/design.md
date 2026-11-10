# Design: add-report-formats

## Context

- **`qgate-core`:**
  - `Finding` has `rule`, `severity`, `message`, `location` and an optional
    `metric`;
  - `Location` has `path` and an optional `line`;
  - `Metric` holds `before` and `after` as `Option<f64>`.

  The crate has no dependencies and no I/O.
- **`qgate-report`:** only `render_text` exists.
- **`qgate-cli`:** sorts the findings, renders them, writes them through
  `write_stdout`, which tolerates a closed pipe, and prints the failing
  summary line on standard error. `FailOn` lives there.
- **Duplication analyzer:** builds one finding per reported block, and
  computes the new occurrence's last line and the named counterpart's
  lines for the message. The other counterparts are only counted
  (`, and <k> more`).
- **Workspace dependencies:** `serde` is already one (for `qgate-config`).
  `serde_json` is not.

For the motivation see `proposal.md`, and for the required behaviour
the specs.

## Goals / Non-Goals

**Goals:**
- A JSON report that is easy to keep stable: its field names and shapes
  are defined in one place, separate from the core types.
- No change at all to the text output.

**Non-Goals:**
- Serde support on the core types.
- Parsing the JSON report back into findings. The MCP server will
  produce the report, not read it.

## Decisions

### D1. Two new fields in the core types

- `Location` gets `end_line: Option<u32>`: the last line of a finding
  that spans lines. It is `None` for a single-line finding.
- `Finding` gets `related: Vec<Location>`: other places the finding
  refers to, in a meaningful order. It is empty for most findings.

**Why generic fields, not a duplication-specific one:** SARIF (M4) has
the same concepts (`region.endLine`, `relatedLocations`), so the M4
renderer maps them directly. Architecture findings can use `related`
later if their targets ever become locations.

**Alternative:** a `details` enum per rule. It is more expressive, but every
consumer would need a case per rule, and nothing needs it yet. It can
be added later, compatibly.

**What changes in the code:** every struct literal of `Location` and
`Finding` in the workspace changes, about 10 of them. This is
mechanical, and the compiler finds them all.

### D2. The duplication analyzer fills them in

- **The finding's range:** `end_line` is the last line of the new
  occurrence. It is the same number the message prints after `lines
  <first>-`.
- **Related locations:**
  - each file that counts for the message (the named one and the
    `k` more) gets one related location;
  - in a file with several copies, the location is the first copy by
    line, as the copies are already sorted;
  - each location's `end_line` is computed as for the named counterpart
    today: the last line of the copy, over the length of the whole block;
  - the order is by path, so the first related location is always the
    file the message names.
- **The message does not change.**

### D3. The JSON shape is defined in `qgate-report`, with serde

- **Where the shape lives:** private structs in `qgate-report` mirror the
  report: envelope, finding, location, metric and summary. Each derives
  `serde::Serialize`, and `render_json` copies the core values into
  them.
- **Field order:** the order the spec shows comes from the order of the
  struct fields.
- **Absent fields:** `#[serde(skip_serializing_if = ...)]` leaves out
  empty options and an empty `related`, so there is never a `null`.
- **Why separate structs, not derives on the core types:**
  - the JSON field names are a public API. Keeping them next to the
    renderer means a refactor in `qgate-core` cannot rename a JSON key by
    accident;
  - `qgate-core` stays free of dependencies;
  - the M4 formats (Markdown, SARIF) get their own mapping in the same
    crate.
- **Why `serde_json`, not hand-written JSON:**
  - it escapes strings correctly: control characters, quotes and
    backslashes, which appear in messages that quote import specifiers;
  - it formats numbers correctly;
  - it is maintained with `serde`, and adds only `itoa`, `ryu` and
    `memchr`.

  A hand-written writer would be about 60 lines, but escaping is
  exactly where a public format must not have bugs.
- **Output:** compact, one line, followed by a newline. An agent hook puts
  the report into the model's context, where whitespace costs tokens.
  Line-oriented tools get one record per run, and `jq .` pretty-prints it
  for people.

### D4. Whole numbers are written as integers

- **The problem:** `Metric` values are `f64`, so serde would write
  complexity 4 as `4.0`.
- **The fix:** the mirror struct holds a small number type. A finite
  value with no fraction that fits in an `i64` serializes as an integer;
  any other value as a float.
- **Why not change `Metric` to integers:** later metrics, such as a
  duplication ratio or churn, may not be whole numbers. The JSON Schema
  declares the fields as `number`, so both forms stay valid.

### D5. The summary comes from the CLI as plain values

- `render_json(findings, summary)` takes a `ReportSummary { failing:
  usize, fail_on: &str }` that `qgate-report` defines. The CLI fills it
  in from `FailOn`, which stays in the CLI.
- The number of findings is computed by the renderer.
- `qgate-report` gains no knowledge of exit codes.

### D6. `--format` in the CLI

- **The option:** a `Format` value enum (`text`, `json`), parsed the same
  way as `--fail-on`, with the default `text`.
- **The output:** the rendered string goes through the existing
  `write_stdout`. A failing run still prints its summary line on standard
  error after the report, in both formats.
- **Errors:** the error path does not change. Errors are returned as
  strings before anything is rendered, so standard output stays empty.
- **The two-file form** renders through the same function, so it gets
  JSON with no extra code.

### D7. Paths are written lossily

- Paths are written as `Path::to_string_lossy`, the same text that
  `display()` shows in the text format.
- **Why this is enough:** revision mode gets its paths from git as
  `String` already, so only the two-file form can meet a path that is not
  UTF-8.

### D8. The JSON Schema and how it is checked

- **The schema:**
  - it lives at `docs/schema/report-v1.json`, JSON Schema draft 2020-12;
  - it describes the envelope, findings, related locations and the
    summary;
  - it allows additional properties, because new keys may be added
    within version 1.
- **How tests check reports against it:** a validator of about 80 lines
  in the CLI's tests (a test helper, not shipped). It walks the schema
  and supports only the keywords the schema uses:
  - `type`, `properties`, `required`, `items`, `enum`, `const`, `minimum`
    and `$ref` to `$defs`.

  A second test fails if the schema ever uses another keyword, so the
  helper cannot silently skip a rule.
- **Why not the `jsonschema` crate as a dev-dependency:** it pulls in
  dozens of crates (regex engines, URL handling, optional HTTP) for a
  schema that uses eight keywords. A dev-dependency does not reach users,
  but it does slow down every CI run and MSRV check.
- **The test reports** cover a complexity finding, an architecture
  finding with escaped quotes, a duplicate block with related locations,
  an `input/unparsable` finding without a line, and an empty report.
  They are run end to end, so they also check the real output.
- **`serde_json` in the tests:** the CLI tests parse the output with
  `serde_json`, added as a dev-dependency of `qgate-cli`. It is the same
  crate the report uses.

## Risks / Trade-offs

- **[Version 1 is frozen once published.]** A wrong name or shape stays
  until version 2.
  - **Mitigation:**
    - the spec scenarios fix the exact bytes of each kind of finding;
    - the reviewer checks the shape before the first release;
    - nothing has been released yet (binaries come with M4), so the
      format can still change in this change's reviews.
- **[Consumers might parse `message`.]**
  - The architecture target, for one, is only in the message.
  - **Mitigation:** the README says that messages are for people and may
    be reworded; structured fields are the contract.
  - **Remaining gap:** structured architecture details would close it,
    and are deferred (see the proposal's Out of scope).
- **[`end_line` and `related` appear only in JSON.]** The text format and
  its tests do not show them.
  - **Mitigation:** analyzer unit tests check the fields directly, and
    the JSON end-to-end tests check them again.
- **[The hand-written schema validator could be wrong.]**
  - **Mitigation:**
    - its own tests feed it invalid reports (a missing key, a wrong
      type, a `null`) and expect a rejection;
    - the keyword guard keeps it in step with the schema.

## Migration Plan

None. The option is new, the default output is unchanged, and the new
core fields are internal to the workspace (no crate is published).

## Open Questions

None. Behaviour is decided in the specs, and implementation choices are
recorded above.
