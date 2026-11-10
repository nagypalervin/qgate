# mcp-server Specification

## Purpose

Defines qgate as a Model Context Protocol (MCP) server, so any AI coding
agent's MCP client can ask qgate, whenever the agent decides to, what
its work made worse.

## Requirements

### Requirement: Server command

`qgate mcp [--fail-on <level>]` SHALL serve MCP over stdio. It SHALL
read JSON-RPC 2.0 messages from standard input, one per line, and write
each response as one line on standard output. It SHALL write nothing
else to standard output. It SHALL exit with code 0 when standard input
ends or the client closes standard output, and with 1 when reading or
writing fails otherwise. `--fail-on` takes the levels of `qgate check`,
`error` by default. `qgate --help` SHALL list the `mcp` command.

#### Scenario: Input ends

- **WHEN** a client starts `qgate mcp` and closes its standard input without sending anything
- **THEN** standard output is empty
- **AND** the exit code is 0

#### Scenario: The client closes the output

- **WHEN** the client closes the server's standard output and sends a request
- **THEN** the exit code is 0

#### Scenario: Input that cannot be read

- **WHEN** reading the server's standard input fails
- **THEN** standard error says that the input cannot be read
- **AND** the exit code is 1

#### Scenario: Help lists the MCP command

- **WHEN** the user runs `qgate --help`
- **THEN** the output mentions the `mcp` command

#### Scenario: Unknown fail-on level

- **WHEN** a client starts `qgate mcp --fail-on info`
- **THEN** standard error names the allowed levels
- **AND** standard output is empty
- **AND** the exit code is 2

### Requirement: Current protocol revision

The server SHALL implement MCP revision `2026-07-28`. It SHALL answer
`server/discover` with `supportedVersions` holding `2026-07-28`, a `tools`
capability, `instructions` that say when to call `check`, and in `_meta`
under `io.modelcontextprotocol/serverInfo` the name `qgate` with qgate's
version. It SHALL serve every
request whose `_meta` declares `2026-07-28`, without a handshake.

#### Scenario: Discovery

- **WHEN** the client sends `{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}`
- **THEN** the response has the `id` 1 and a `result` with `"resultType":"complete"`
- **AND** its `supportedVersions` holds `2026-07-28`
- **AND** its `capabilities` has `tools`
- **AND** its `_meta` names the server `qgate` with qgate's version

#### Scenario: A call without a handshake

- **GIVEN** a client that has sent nothing else
- **WHEN** it sends `tools/list` with `_meta` declaring `2026-07-28`
- **THEN** the response lists the tools

### Requirement: Unsupported protocol revision

A request whose `_meta` declares a protocol revision the server does not
serve SHALL get the JSON-RPC error `-32022`. The error's `data` SHALL have
`supported`, the revisions the server serves, and `requested`, the one
the request declared.

#### Scenario: A future revision

- **WHEN** the client sends `tools/list` with `_meta` declaring `2099-01-01`
- **THEN** the response is an error with code `-32022`
- **AND** its `data.requested` is `2099-01-01` and its `data.supported` holds `2026-07-28`

### Requirement: Older protocol revisions

The server SHALL also serve clients that start with `initialize`, the
handshake of revisions up to `2025-11-25`. It SHALL answer with the
requested `protocolVersion` when it is `2025-11-25` or `2025-06-18`, and
with `2025-11-25` otherwise. The answer SHALL have the same capabilities,
`serverInfo` and `instructions` as the answer to `server/discover`. It
SHALL ignore `notifications/initialized` and answer `ping` with an empty
result.

#### Scenario: Handshake

- **WHEN** the client sends `initialize` with `"protocolVersion":"2025-11-25"`
- **THEN** the result has `"protocolVersion":"2025-11-25"`, a `tools` capability and `serverInfo` named `qgate`

#### Scenario: An older revision the server does not serve

- **WHEN** the client sends `initialize` with `"protocolVersion":"2024-11-05"`
- **THEN** the result has `"protocolVersion":"2025-11-25"`

#### Scenario: Ping

- **GIVEN** a client that has sent `initialize` and `notifications/initialized`
- **WHEN** it sends `{"jsonrpc":"2.0","id":7,"method":"ping"}`
- **THEN** the response is `{"jsonrpc":"2.0","id":7,"result":{}}`
- **AND** no response is written for the notification

### Requirement: The check tool

`tools/list` SHALL list one tool, `check`. Its input schema SHALL have
three optional string properties and no others: `base`, `head` and
`directory`. Its output schema SHALL be the JSON report's schema, version
1. Its annotations SHALL say that it is read-only, idempotent, and does
not reach outside the computer.

#### Scenario: Listing the tools

- **WHEN** the client sends `tools/list`
- **THEN** the result's `tools` has exactly one tool, named `check`
- **AND** its `inputSchema` has the properties `base`, `head` and `directory`, none required
- **AND** its `annotations` has `"readOnlyHint":true`

### Requirement: What the tool checks

A call of `check` SHALL report the findings that `qgate check --base
<base> --worktree --format json --fail-on <level>` reports, where `base`
defaults to `HEAD` and the level is the server's `--fail-on`. When `head`
is given, it SHALL report what `--head <head>` reports instead of
`--worktree`.

#### Scenario: Uncommitted work

- **GIVEN** a committed change that made `f` in `src/a.ts` go from 2 to 9, and an uncommitted change that makes `g` in `src/b.ts` go from 2 to 9
- **WHEN** the client calls `check` with no arguments
- **THEN** the report has the finding for `g` and not the one for `f`

#### Scenario: The whole branch

- **GIVEN** the repository of the previous scenario, on a branch made from `main` before both changes
- **WHEN** the client calls `check` with `{"base":"main"}`
- **THEN** the report has the findings for both `f` and `g`

#### Scenario: A revision instead of the working tree

- **GIVEN** the repository of the first scenario
- **WHEN** the client calls `check` with `{"base":"main","head":"HEAD"}`
- **THEN** the report has the finding for `f` and not the one for `g`

### Requirement: The tool's result

When the check runs, the result SHALL have `"isError":false`.
`structuredContent` SHALL be the JSON report, as `qgate check --format json`
writes it, and `content` SHALL be one text block with that same JSON. The
report SHALL hold every finding at every severity, and its `summary` SHALL
count failing findings at the server's `--fail-on` level.

#### Scenario: One failing finding

- **GIVEN** an uncommitted change that makes `f` in `src/a.ts` go from 2 to 9
- **WHEN** the client calls `check`
- **THEN** `structuredContent` has `"schema_version":1` and one finding with the rule `complexity/cyclomatic-increase`, the severity `error` and the path `src/a.ts`
- **AND** its `summary` is `{"count":1,"failing":1,"fail_on":"error"}`
- **AND** `content` is one text block whose text, parsed as JSON, equals `structuredContent`

#### Scenario: Nothing got worse

- **GIVEN** a repository whose working tree has no change since `HEAD`
- **WHEN** the client calls `check`
- **THEN** `structuredContent` has an empty `findings` array and `"isError"` is `false`

#### Scenario: The team's level

- **GIVEN** an uncommitted change that makes `f` in `src/a.ts` go from 2 to 4, a warning
- **WHEN** the client calls `check` on a server started as `qgate mcp --fail-on warning`
- **THEN** the report's `summary` is `{"count":1,"failing":1,"fail_on":"warning"}`

### Requirement: The directory checked

The tool SHALL check the repository that contains `directory`. A relative
`directory` SHALL be taken from the server's working directory. Without
`directory`, the tool SHALL check the repository that contains the server's
working directory.

#### Scenario: Default directory

- **GIVEN** a server started in a repository with an uncommitted finding
- **WHEN** the client calls `check` with no arguments
- **THEN** the report has that finding

#### Scenario: Another directory

- **GIVEN** a server started outside any repository, and a repository at `/work/app` with an uncommitted finding
- **WHEN** the client calls `check` with `{"directory":"/work/app"}`
- **THEN** the report has that finding

### Requirement: A check that cannot run

When the check cannot run, for example outside a repository, with an
unknown revision, an invalid `qgate.toml` or an internal error in qgate,
the result SHALL have
`"isError":true` and no `structuredContent`. Its `content` SHALL be one
text block starting with `qgate: ` and naming the problem, as `qgate check`
would on standard error. The server SHALL keep running.

#### Scenario: Unknown base

- **WHEN** the client calls `check` with `{"base":"no-such-branch"}`
- **THEN** the result has `"isError":true` and a text that names `no-such-branch`
- **AND** a following `tools/list` is answered

#### Scenario: Invalid configuration

- **GIVEN** a repository whose working tree has an invalid `qgate.toml`
- **WHEN** the client calls `check`
- **THEN** the result has `"isError":true` and a text that names `qgate.toml`

#### Scenario: Outside a repository

- **GIVEN** a server started outside any repository
- **WHEN** the client calls `check` with no arguments
- **THEN** the result has `"isError":true`

### Requirement: Invalid arguments

Arguments that do not match the input schema, such as an unknown property
or a value that is not a string, SHALL give a result with `"isError":true`
and a text naming the argument, so the model can correct its call. A
`null` value SHALL count as an absent argument. A call of a tool other
than `check` SHALL get the JSON-RPC error `-32602`.

#### Scenario: A value that is not a string

- **WHEN** the client calls `check` with `{"base":7}`
- **THEN** the result has `"isError":true` and a text that names `base`

#### Scenario: A null argument

- **GIVEN** the repository of "Uncommitted work"
- **WHEN** the client calls `check` with `{"base":"main","head":null}`
- **THEN** the report has the findings for both `f` and `g`

#### Scenario: Unknown tool

- **WHEN** the client sends `tools/call` with `"name":"explain"`
- **THEN** the response is an error with code `-32602`

### Requirement: Protocol errors

The server SHALL answer a line that is not JSON (or not UTF-8) with the
error `-32700` and the `id` `null`, a request for a method it does not
know with `-32601`, and SHALL write no response to a notification. It
SHALL keep running after each of them.

#### Scenario: Not JSON

- **WHEN** the client sends the line `not json`
- **THEN** the response is an error with code `-32700` and `"id":null`
- **AND** a following `tools/list` is answered

#### Scenario: Unknown method

- **WHEN** the client sends `{"jsonrpc":"2.0","id":3,"method":"resources/list"}`
- **THEN** the response is an error with the `id` 3 and code `-32601`

#### Scenario: A cancellation

- **WHEN** the client sends the notification `notifications/cancelled`
- **THEN** no response is written

#### Scenario: Input that is not UTF-8

- **WHEN** the client sends a line with a byte sequence that is not UTF-8
- **THEN** the response is an error with code `-32700` and `"id":null`

### Requirement: Invalid messages

A message whose `id` is not a string or an integer SHALL get the error
`-32600` with the `id` `null`. A message without `"jsonrpc":"2.0"`, or
without a method and not a response, SHALL get `-32600`. A response
from the client (a `result` or an `error`, without a method) SHALL get
no response, whatever its `id`, since the server sends no requests.

#### Scenario: A null id

- **WHEN** the client sends `{"jsonrpc":"2.0","id":null,"method":"tools/list"}`
- **THEN** the response is an error with code `-32600` and `"id":null`

#### Scenario: A response from the client

- **WHEN** the client sends `{"jsonrpc":"2.0","id":4,"result":{}}`
- **THEN** no response is written

### Requirement: The server changes nothing

A call of `check` SHALL leave the repository as `qgate check --worktree`
does (see `revision-input`): the index, `HEAD`, branches and files stay
as they are.

#### Scenario: Staged work stays staged

- **GIVEN** a staged change and an unstaged change
- **WHEN** the client calls `check`
- **THEN** `git status --porcelain` shows the same output before and after
