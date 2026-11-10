# Design: add-mcp-server

## Context

See `proposal.md` for the why.

- **`qgate-cli` already has one protocol adapter.** The `hook` module
  (`add-agent-hooks`) turns findings into Claude Code's hook protocol.
  It keeps the protocol pure, while `main` reads input, runs the check
  and writes output. `check_revisions(dir, base, head)` runs a revision
  or working-tree check in any directory and returns sorted findings,
  or a message.
- **The JSON report exists.** `render_json(findings, summary)` in
  `qgate-report` writes report version 1, and its schema is
  `docs/schema/report-v1.json`. The tests already validate against
  that schema with a small validator (`tests/support/schema.rs`).
- **MCP has two eras in use** (the spike on 2026-11-10, see the
  proposal):
  - revision `2026-07-28` has no handshake. Each request carries
    `_meta["io.modelcontextprotocol/protocolVersion"]`, and
    `server/discover` is required;
  - up to `2025-11-25`, a client sends `initialize`, then
    `notifications/initialized`, and may send `ping`.

  Claude Code 2.1.319 tries `server/discover` first and falls back to
  `initialize` if a server rejects it. A dual-era server is allowed,
  and an `initialize` "selects legacy semantics, scoped to the stdio
  process".
- **Stdio rules:**
  - messages are newline-delimited JSON-RPC 2.0;
  - the server writes only responses (and allowed notifications) to
    standard output;
  - in 2026-07-28 a stdio server MUST NOT send requests;
  - logs go to standard error.

## Goals / Non-Goals

**Goals:**
- A small, synchronous server with no new dependency, whose protocol
  logic can be tested without processes.
- Both eras behind the same tool code. The era decides only the
  envelope of a result, never which findings it reports.
- The tool's result is the JSON report, the same bytes
  `qgate check --format json` writes. A report has one format, not two.

**Non-Goals:**
- Concurrency. Requests are answered one at a time, in order.
- Covering the whole MCP specification. Methods the server does not
  offer get `-32601`.

## Decisions

### D1. The protocol lives in an `mcp` module of `qgate-cli`

The hook protocol is a module of `qgate-cli`, and the MCP protocol will
be one too: `crates/qgate-cli/src/mcp.rs`, next to `hook.rs`.

- **`mcp`** holds a `Server`. Its handler takes one input line and
  returns the response line, if there is one. The handler also gets a
  `check` callback, which turns a directory, a base and a head into
  findings or a message. The server's state is the era chosen for the
  process. Everything else is pure.
- **`main`** owns the I/O:
  - it reads standard input line by line and calls the handler;
  - it writes each response with one `write_all` and a flush;
  - it passes `check_revisions` as the callback.

*Alternatives:*
- **A `qgate-mcp` crate.** It would need the check, which lives in
  `qgate-cli`. So it would mean either moving the check into a new
  library crate, or a dependency on the binary crate. That is a
  structural change and needs an ADR. One stdio adapter does not need
  that.
- **Logic in `main.rs`.** That file has 836 lines already. The hook set
  the pattern of a separate module.

### D2. The server speaks MCP itself, with `serde_json`, not with an SDK

The server reads every message as a `serde_json::Value` and builds
every response with `serde_json::json!`. Its whole surface is these
methods:
- `server/discover`;
- `initialize`;
- `ping`;
- `tools/list`;
- `tools/call`;
- the notifications `notifications/initialized` and
  `notifications/cancelled`.

Every other method gets `-32601`. Every other notification is ignored.

*Alternative:* the official Rust SDK, `rmcp`.
- It brings an async runtime (`tokio`), a schema generator and a long
  tail of transitive dependencies. qgate needs none of them, since it
  answers one request at a time.
- Its support for 2026-07-28 would also tie our release timing to its
  releases.

CLAUDE.md asks for few dependencies. The cost of writing it ourselves
is that we follow the specification by hand. The specification
scenarios cover each method and error code for both eras. A later
revision is a small, local change in one module.

### D3. Eras: decided per request, `initialize` switches the process to the old one

- **`initialize`** puts the process in the old era, with the negotiated
  version:
  - the requested `protocolVersion`, when it is `2025-11-25` or
    `2025-06-18`;
  - otherwise `2025-11-25`. The specification then lets the client
    decide whether to go on.

  `2025-06-18` is the earliest revision with `structuredContent` and
  `outputSchema`, which the tool's result relies on. An older client
  receives a result with fields it does not know, which JSON-RPC
  clients ignore.
- **A request whose `_meta` declares a version** is checked against
  `["2026-07-28"]`:
  - if it matches, the request is served in the current era;
  - otherwise it gets `-32022` with `data.supported` and
    `data.requested`.
- **A request that declares no version** gets the process's era: the
  old one after `initialize`, otherwise the current one. A request
  that declares no version without a handshake is not valid MCP. It is
  served anyway: that costs nothing, and hand tests with `printf` stay
  short.
- **The envelope differs by era:**
  - **current era:** results get `"resultType":"complete"`, and
    `tools/list` also gets `"ttlMs"` and `"cacheScope":"public"`. The
    tool list is the same for every client and every call;
  - **old era:** results have no `resultType`, `ttlMs` or
    `cacheScope`. The spike showed that Claude Code accepts them in the
    old era, but other clients may validate strictly.

  `ttlMs` is 0, because a new qgate binary may list the tool
  differently. A cached list would not see that.
- **The same answer for discovery and handshake.** `server/discover`
  and `initialize` give the same `capabilities`
  (`{"tools":{"listChanged":false}}`), the same server info
  (`{"name":"qgate","version":<CARGO_PKG_VERSION>}`) and the same
  `instructions`. Discovery puts the server info in
  `_meta["io.modelcontextprotocol/serverInfo"]`. The handshake puts it
  in `serverInfo`.

*Alternative:* serve only 2026-07-28 and reject `initialize` with the
supported versions, as the specification allows. VS Code's era is not
known yet (see D9). Claude Code already shows that clients in the field
keep the fallback. Support for both is about 40 lines.

### D4. The tool's result is the JSON report, once as a value and once as text

- **A check that runs.** `render_json(findings, summary)` gives the
  report line, exactly what `qgate check --format json --fail-on
  <level>` prints. The result is:
  - `content`: one text block with that line, without the trailing
    newline;
  - `structuredContent`: the same line, parsed;
  - `isError: false`.

  The 2026-07-28 tools page says a tool returning structured content
  "SHOULD also return the serialized JSON in a TextContent block".
- **A check that cannot run.** The message `check_revisions` returns is
  the text `qgate check` prints after `error: `. The result is:
  - `content`: one text block, `qgate: <message>`;
  - `isError: true`;
  - no `structuredContent`.
- **`outputSchema`** is `docs/schema/report-v1.json`, embedded at build
  time with
  `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/schema/report-v1.json"))`.
  The schema then has one source, the published one. A test checks that
  the listed `outputSchema` equals the file.

*Alternatives:*
- **Text for the model plus JSON for programs.** The model reads JSON
  well, and the report's `message` fields are already the text lines.
  Two blocks would put every finding into the context twice.
- **Only the findings at or above `--fail-on`.** The model would never
  see the Warnings it could cheaply fix. `summary.failing` already
  says what the team counts as failing, and the server's instructions
  say to fix those first.

### D5. Arguments are checked by hand

`arguments` may be absent or `null`, which means no arguments, or an
object. It may hold:
- `base` and `head`, strings;
- `directory`, a string.

Anything else gives a tool error. Possible texts:
- `qgate: unknown argument "foo"; the arguments are base, head and directory`;
- `qgate: argument "base" must be a string`;
- `qgate: the arguments must be an object`.

The texts name the argument, so the model can retry. From
`2025-11-25` on, the specification wants input validation errors as
tool errors, not protocol errors. An empty `base` or `head` is passed
to git, which reports it as an unknown revision.

An unknown tool name gets the JSON-RPC error `-32602` (invalid params),
as both eras specify. So does a `tools/call` without a string `name`.

### D6. The directory: the server's working directory, or the argument

The directory is the `directory` argument when it is given, joined
onto the working directory when it is relative. Without it, it is the
process's working directory. Then `check_revisions(&dir, base, head)`
runs as usual. The working directory is read once, at start-up. If it
cannot be read, a call that needs it gets a tool error.

- **No `roots`.** Roots are deprecated in 2026-07-28. Getting them
  would mean an input-required round trip in the current era and a
  server-to-client request in the old one.
- **No `CLAUDE_PROJECT_DIR`.** The hook reads it, but Claude Code
  starts MCP servers in the project directory anyway (spike), and other
  clients do not set it.

`directory` can name any repository on the machine. The check only
reads, and the agent that calls it can usually run `qgate check` in a
shell anyway, so this gives it no new reach.

### D7. What the model reads: the tool description and the server instructions

The tool's `title` is "Check what got worse". The `description`:

> Report what the changes made worse: higher complexity, duplicated
> code, broken architecture rules. Without arguments it checks the
> uncommitted work: the working tree, untracked files included,
> against HEAD. Pass base "main" (or the branch this work will merge
> into) to check the whole branch. Pass head to check a commit instead
> of the working tree. Each finding is a delta that this change made
> worse. summary.failing counts the findings the team treats as
> failing.

The `instructions`, sent with discovery and the handshake:

> qgate reports what a change made worse. Call check before you hand
> work back or commit it. Fix the findings that summary.failing counts,
> or tell the user why such a change is intended. Fix the others when
> it is cheap.

The texts are in English, like all of qgate's output. The tests pin
their presence, not their wording.

### D8. Errors, notifications and the I/O loop

- **Errors the server answers:**
  - a line that is not JSON gets `-32700` with `"id": null`;
  - a JSON array (a batch, which was removed in `2025-06-18`) or an
    object without a string `method` gets `-32600`. The response uses
    its `id` if it has one, otherwise `null`.
- **Notifications.** A message without an `id` is a notification and
  never gets a response, even one with an unknown method.
- **Empty lines** are skipped.
- **Exit codes:**
  - 0 when standard input ends;
  - 1 when standard input cannot be read, or standard output fails
    with anything other than a closed pipe;
  - 2 for an invalid command line (clap), as for `qgate check`.

  A client that closes the pipe early (`BrokenPipe`) ends the server
  with 0: the client is gone.
- **Checks run one at a time.** A `notifications/cancelled` arrives
  after the response it names has been written, so the specification's
  "MUST NOT send any further messages" holds anyway.
- **No logging.** The server writes nothing to standard error, except
  the exit-1 cases above.

### D9. Tests and hand checks

- **Unit tests in `mcp`**, with a fake check callback. They cover:
  - every protocol scenario of the spec in both eras: discovery,
    handshake, version selection, `-32022`, `ping`;
  - the envelopes;
  - argument errors;
  - the protocol errors.
- **Integration tests in `crates/qgate-cli/tests/mcp.rs`.** They start
  the binary with piped standard input in a temporary repository,
  using the same repository helpers as `revisions.rs` and `hook.rs`.
  They cover:
  - the tool scenarios (the working tree, `base`, `head`, `directory`,
    the level, a check that cannot run, staged work staying staged);
  - standard input ending;
  - `--help`;
  - an invalid `--fail-on`.

  `structuredContent` is validated with `tests/support/schema.rs`. A
  test also asserts that it equals `qgate check --format json` in the
  same repository.
- **Hand checks, recorded in this file:**
  - **Claude Code** (the agent does it): `claude -p` with an
    `--mcp-config` that names the built binary, in a repository with
    one finding. The agent calls `check` and reports the finding.
    `--include-hook-events` is not needed. The spike's message log is
    the model for what to record.
  - **VS Code with GitHub Copilot** (Ervin does it, since the agent
    cannot drive Copilot Chat): `.vscode/mcp.json` from the README, in
    a repository with one finding. Ask Copilot in agent mode to check
    the work with qgate. To see which era VS Code 1.145 speaks, run the
    server once through a wrapper that copies its input to a file, such
    as `sh -c 'tee /tmp/qgate-mcp-in.log | qgate mcp'`. The server
    itself does not log (D8).

## Implementation Notes

Decisions made during apply:

- **Tasks 1.1 to 2.2 are one commit.** Until `main` uses the `mcp`
  module, it is dead code, which `clippy -D warnings` rejects. The
  pre-commit hook also runs the tests, so failing tests cannot be
  committed on their own.

  The tests were still written first and run to fail:
  - against a stub `handle` that answered nothing, 23 of the 25 unit
    tests failed. The two that passed check that a notification and an
    empty line get no answer, which a silent stub also does;
  - before `qgate mcp` existed, all 30 end-to-end tests failed.
- **Discovery is always marked as current.** A `server/discover` result
  carries `resultType`, even after an `initialize`. Discovery exists
  only in the current era.
- **Every request's version is checked, discovery included.** A
  request whose `_meta` declares a revision other than 2026-07-28 gets
  `-32022`, whatever its method. The error's `data.supported` tells the
  client the same as discovery would. `initialize` is exempt, because
  it carries its version in `protocolVersion`.
- **A version that is not a string** gets `-32022` too, with the value
  as `requested`.
- **`ping` is answered in both eras.** The current era removed it, but
  answering costs nothing. In the current era its result is
  `{"resultType":"complete"}`.
- **Key order.**
  - The response envelope is a struct, so a response line starts with
    `"jsonrpc"`, then `"id"`, as in the JSON-RPC examples and the spec's
    `ping` scenario.
  - Inside `result`, `serde_json` maps sort their keys. So
    `structuredContent` lists `findings`, `schema_version`, `summary`
    in that order.
  - The text block keeps the bytes of `qgate check --format json`
    exactly. Its order is the report's, and a test compares them.
- **Input that is not UTF-8.** `main` reads lines as bytes and passes
  them on with invalid sequences replaced. Such a line is not valid
  JSON-RPC either, so it gets `-32700` like any other unparsable line.
- **Arguments are checked in key order.** With several bad arguments,
  the first in alphabetical order is named, since `serde_json` maps
  sort their keys. The tests use one bad argument each.
- **The end-to-end tests' `found` helper looks only at complexity
  findings.** The `f` and `g` fixtures are copies of each other, so an
  uncommitted `g` also gives a `duplication/duplicate-block` finding.
  The scenarios are about which complexity change is reported.
  `the_report_is_the_check_commands_report` compares whole reports,
  duplication included.
- **The unit tests' fake check is boxed (`FakeCheck`),** because
  clippy's `type_complexity` rejects the `impl FnMut` return type.
- **Rust 1.85:** the `qgate-cli` tests pass on the MSRV toolchain
  locally too.

- **The tests bite (task 2.3).** Each mutation of `mcp.rs` was run
  with `cargo test -p qgate-cli --no-fail-fast`, then reverted. Every
  one fails unit tests and end-to-end tests:

  | Mutation | Unit tests failing | End-to-end tests failing |
  |---|---|---|
  | Always add `resultType` | 2 | `a_whole_old_era_session`, `ping` |
  | Accept any `_meta` version | 1 | `a_future_revision` |
  | Drop the text block | 1 | `one_failing_finding`, `the_report_is_the_check_commands_report` |
  | Report only failing findings | 1 | `the_report_is_the_check_commands_report` |
  | Protocol error instead of `isError` | 3 | `a_value_that_is_not_a_string`, `outside_a_repository`, `invalid_configuration`, `unknown_base` |
  | Ignore `directory` | 2 | `a_relative_directory`, `another_directory` |
  | Ignore `--fail-on` | 1 | `the_teams_level` |
  | Answer notifications | 2 | `a_cancellation`, `a_whole_old_era_session`, `ping` |

  The task names seven checks. "The result" covers two mutations: the
  dropped text block and the failing findings only.

- **The README was tried by hand (task 3.1).** In a temporary
  repository, the built binary was first on the `PATH`, with an
  uncommitted `f` going from 2 to 9.
  - `claude mcp add --scope project qgate -- qgate mcp` writes
    `.mcp.json` with `"type": "stdio"` and an empty `env` besides the
    README's keys. The README calls it "an equivalent entry".
  - A `tools/call` piped into `qgate mcp` returned the finding.
  - `--fail-on warning` with an unknown base returned the tool error
    that names `nope`.
  - `directory` worked from outside the repository.
  - The text block equals `qgate check --base HEAD --worktree --format json`.
- **VS Code trust.** VS Code's documentation says that a workspace's
  `.vscode/mcp.json` follows Workspace Trust: in Restricted Mode its
  servers do not start. The README's trust note says so.
- **Hand check with Claude Code (task 3.2), 2026-11-10.**
  - **Setup:** Claude Code 2.1.319, with `claude -p` and an
    `--mcp-config` whose command was
    `sh -c 'tee <log> | target/debug/qgate mcp'`. The temporary
    repository had one uncommitted finding.
  - **The client sent:** `server/discover`, `tools/list` and
    `tools/call`, each with `_meta` declaring 2026-07-28. So the current
    era was used.
  - **The model** called `check` with an absolute `directory` (the
    project), not with empty arguments. It answered: "qgate found one
    failing finding in your uncommitted work: in `src/a.ts:1`, the
    cyclomatic complexity of function `f` went from 2 to 9, and the team
    treats that as an error."
  - **The repository:** `git status --porcelain` was unchanged
    afterwards.

## Risks / Trade-offs

- **[The 2026-07-28 revision is four months old, and clients differ in
  how they implement it]** → Both eras are served. Each era has spec
  scenarios, and both target clients are checked by hand. The protocol
  is in one module, so a fix stays local.
- **[A client validates `structuredContent` strictly against
  `outputSchema`]** → The structured content is the report that the
  schema already describes, and a test validates it. Error results
  have no structured content. The specification asks for structured
  results only on success, and SDK clients skip validation when
  `isError` is true.
- **[`include_str!` reads a file outside the crate, which `cargo
  package` would not ship]** → No crate is published yet. Release
  binaries (M4) are built from the repository. If crates are ever
  published, the schema moves into `qgate-report`, and the docs file
  becomes the copy that a test compares.
- **[`readOnlyHint: true` while the working-tree snapshot writes
  objects into `.git`]** → The snapshot changes nothing the user sees:
  not the work tree, the index, a ref or the stash. The objects it
  adds are unreferenced, and `git gc` removes them (the
  `revision-input` spec, "the check changes nothing"). The hint
  describes the user's environment, and there it is true.
- **[A project's `.mcp.json` names a command that runs on the
  developer's machine]** → The same trust question as hooks. Claude
  Code asks before it uses a project's MCP server, and VS Code asks in
  an untrusted workspace. The README says so, as it does for the hook.
- **[A large repository makes a check take seconds, and the client
  shows a waiting tool call]** → Accepted. Checks are already fast on
  the repositories qgate targets, and progress notifications are out
  of scope.

## Migration Plan

Nothing to migrate. The command is new. Rollback is removing the server
entry from the client's configuration.

## Open Questions

- **Does VS Code 1.145 speak the current era or the old one?** It does
  not change the design, since both are served. The hand check in D9
  answers it, and the answer goes into this file.
