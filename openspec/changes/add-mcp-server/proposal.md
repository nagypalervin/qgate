# Proposal: add-mcp-server

## Why

M3 is about checking while coding. The Claude Code `Stop` hook
(`add-agent-hooks`) covers one agent at one moment: the end of a turn,
against `HEAD`. Other agents cannot use it. Partner teams work in VS Code
with GitHub Copilot, for example. No agent can ask qgate on its own
initiative either: in the middle of a task, before it commits, or about
the whole branch against `main`.

The Model Context Protocol (MCP) solves both. Any MCP client can call a
qgate tool when the agent decides to. The README has promised this
since the start (design principle 3, roadmap M3).

## What Changes

- **New command `qgate mcp [--fail-on <level>]`:** an MCP server on
  standard input and output (stdio). An MCP client starts it as a child
  process. It runs until the client closes its input.
  - Standard output carries only protocol messages. Diagnostics go to
    standard error.
  - `--fail-on` (default `error`) sets the level that the report's
    `summary.failing` counts, as for `qgate check`. A team sets it once,
    in the server entry it commits; the model does not choose it.
- **One tool, `check`:** it reports what got worse, as
  `qgate check --format json` does.
  - **Parameters, all optional:**
    - `base`: the revision to compare against, default `HEAD`;
    - `head`: a revision to check; without it the working tree is
      checked, including uncommitted and untracked files;
    - `directory`: the repository to check; without it, the directory
      the client started the server in.
  - **When the check runs,** the result is the JSON report (schema
    version 1): as structured content, and as the same JSON in a text
    block. It holds every finding, at every severity.
  - **When the check cannot run** (no repository, an unknown revision,
    an invalid `qgate.toml`), the result is a tool error with the
    message the CLI would print. The model sees it and can correct its
    call.
  - **It changes nothing the user sees:** not the working tree, the index
    or a ref. Clients are told it is read-only.
- **Both protocol eras.** The current MCP revision, 2026-07-28, has no
  handshake and adds `server/discover`. Older clients still send
  `initialize`, and the 2025-11-25 revision is the latest of that era.
  The server answers both, so a client works with it whichever one it
  speaks.
- **README:** a "With an MCP client" section with the entry a team
  commits:
  - for Claude Code, `.mcp.json`;
  - for VS Code with GitHub Copilot, `.vscode/mcp.json`;
  - for other clients, the command to run.

### Decisions taken during exploration

- **Client-independent; tested with Claude Code and VS Code with GitHub
  Copilot.** VS Code with Copilot is the client partner teams are most
  likely to use, with Azure DevOps as their CI.
- **No `roots`.** The 2026-07-28 revision deprecates them and recommends
  tool parameters instead. Both target clients start the server in the
  project directory: Claude Code always, VS Code by default (the
  workspace folder). `directory` covers everything else.
- **The server speaks MCP itself, with `serde_json`.** It needs no SDK.
  A tools-only stdio server needs a handful of methods, and the official
  Rust SDK would bring an async runtime and many transitive
  dependencies. `design.md` records the choice.
- **Spike, 2026-11-10:** a logging stdio server checked what Claude Code
  2.1.319 does.
  - Claude Code starts the server in its project directory.
  - It first sends `server/discover` (2026-07-28).
  - When a server rejects that request, it falls back to `initialize`
    with `2025-11-25`.

  Both eras therefore appear in real use.
- **The model chooses the base.** Unlike the hook, the tool is called
  when the agent decides to. It may check its uncommitted work (`HEAD`,
  the default) or the whole branch (`main`). The tool description and
  the server's instructions say when to use each.

### Out of scope

- Any transport other than stdio, such as Streamable HTTP. qgate stays
  local and needs no network.
- Resources, prompts, `roots`, elicitation, the tasks extension and
  progress notifications.
- Cancelling a running check. Checks are short and run one at a time;
  a cancellation is accepted and ignored.
- More tools, such as explaining a rule or checking a single file.
- An install command, a Claude Code plugin, or a VS Code extension.
- Setup instructions for clients other than the two targets.

## Capabilities

### New Capabilities

- `mcp-server`: qgate as an MCP server. It covers:
  - the `qgate mcp` command and the protocol eras it answers;
  - the `check` tool: its parameters, its result and its errors;
  - which directory it checks.

### Modified Capabilities

None. The tool reuses:
- the revision and working-tree checks (`revision-input`);
- the findings and levels (`check-command`);
- the JSON report (`report-formats`).

None of them change.

## Impact

- **`qgate-cli`:**
  - an `mcp` command;
  - the protocol in its own module, next to the hook's;
  - no new dependency, since `serde` and `serde_json` are already
    normal dependencies.
- **No change** to `qgate check`, `qgate hook`, the analyzers, the
  report formats or the other crates.
- **Docs:** README.
- **Users:** none are affected until a team adds the server to its
  client's configuration.
