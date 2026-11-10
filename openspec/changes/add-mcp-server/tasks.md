# Tasks

This change is one pull request, `change/add-mcp-server`, with these
planning artifacts.
- **Decisions made during apply** go into `design.md`.
- **Every commit passes these checks**, which the pre-commit hook
  enforces:
  - `cargo fmt`;
  - `cargo clippy -D warnings`;
  - `cargo test --workspace`.

  CI also runs the tests on Rust 1.85.
- **Commit messages and PR bodies** are written to files first.

## 1. Protocol module

- [x] 1.1 Write failing unit tests in `crates/qgate-cli/src/mcp.rs`. They use a fake check callback that records its arguments (design D1 to D5, D8). They cover:
  - **`server/discover`:**
    - `resultType`;
    - `supportedVersions`;
    - `capabilities`;
    - the server info in `_meta`, with qgate's version;
    - `instructions`.
  - **`initialize`:**
    - `2025-11-25` and `2025-06-18` are answered as requested;
    - `2024-11-05` and a missing version get `2025-11-25`;
    - the same capabilities, `serverInfo` and `instructions` as discovery.
  - **Eras:**
    - a request without a handshake;
    - `_meta` declaring `2026-07-28`;
    - `_meta` declaring an unknown version, which gets `-32022` with `data.supported` and `data.requested`;
    - after `initialize`, results without `resultType`, `ttlMs` or `cacheScope`;
    - without `initialize`, results with them.
  - **`ping`** gets `{}`. **Notifications** get no output, including `notifications/initialized`, `notifications/cancelled` and an unknown one.
  - **`tools/list`:**
    - one tool `check`, with `title`, `description` and `annotations`;
    - an input schema with exactly `base`, `head` and `directory`, all optional strings;
    - `outputSchema` equal to `docs/schema/report-v1.json`.
  - **`tools/call`:**
    - no arguments, or `null`, gives `HEAD`, the working tree and the server's directory;
    - `base`, `head` and a relative or absolute `directory` reach the callback as given or joined;
    - findings become `structuredContent` plus a text block with the same JSON;
    - the report's summary uses the server's level;
    - a callback error gives `isError: true` with `qgate: <message>` and no `structuredContent`;
    - an unknown argument, a value that is not a string, or arguments that are not an object give a tool error that names the problem;
    - an unknown tool, or a missing `name`, gives `-32602`.
  - **Protocol errors:**
    - not JSON gives `-32700` with `id` null;
    - an array, or an object without `method`, gives `-32600`;
    - an unknown method gives `-32601`;
    - an empty line gives no output.
  - **Every response** is one line with `"jsonrpc":"2.0"` and the request's `id`, as a number or as a string.

  Verify that they fail.
- [x] 1.2 Implement `mcp.rs` (design D1 to D5, D7, D8). Verify that the 1.1 tests pass.

## 2. `qgate mcp`

- [x] 2.1 Write failing end-to-end tests in `crates/qgate-cli/tests/mcp.rs`, one per scenario of the `mcp-server` spec.
  - **How each test runs:** it builds a temporary repository with the helpers in `tests/support`, starts `qgate mcp` in it, writes the messages to standard input, closes it, and reads the response lines.
  - **Extra checks:**
    - `structuredContent` validates against the schema with `tests/support/schema.rs`;
    - it equals what `qgate check --base HEAD --worktree --format json` prints in the same repository.

  Verify that they fail.
- [x] 2.2 Add `qgate mcp` to the CLI (design D1, D6, D8):
  - `--fail-on`, with help text;
  - the loop over standard input;
  - the working directory, read once;
  - `check_revisions` as the callback;
  - the exit codes, with a closed output pipe ending the server with 0.

  Verify that the 2.1 tests pass and every existing test passes unchanged.
- [ ] 2.3 Check that the tests bite. Break each of these, one at a time, and confirm that a test fails for each:
  - era selection (always add `resultType`);
  - version check (accept any `_meta` version);
  - the result (drop the text block, or report only failing findings);
  - the error path (return a protocol error instead of `isError`);
  - the directory (ignore the `directory` argument);
  - the level (ignore `--fail-on`);
  - notifications (answer them).

## 3. Docs and verification

- [ ] 3.1 Update `README.md` with a "With an MCP client" section next to "With Claude Code". It covers:
  - the `.mcp.json` entry for Claude Code and the `.vscode/mcp.json` entry for VS Code with GitHub Copilot, with `--fail-on` in the command;
  - the command for other clients;
  - what the tool takes and returns;
  - when to use the hook and when to use the server;
  - the trust note for a repository's MCP configuration.

  Run every README command in a temporary repository and verify that it behaves as documented.
- [ ] 3.2 Check the server by hand with Claude Code, in a temporary repository with one finding (design D9). Use `claude -p` with an `--mcp-config` that names the built binary. Confirm that the model calls `check` and reports the finding, and that the era is 2026-07-28. Record the result in `design.md`.
- [ ] 3.3 Ask the human to check the server by hand in VS Code with GitHub Copilot, with the README's `.vscode/mcp.json` entry and the `tee` wrapper from design D9:
  - Copilot calls `check` and reports the finding;
  - an invalid `qgate.toml` reaches the model as a tool error.

  Record the result, including which era VS Code speaks, in `design.md`, and answer the open question.

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- Write the `docs/journal/` entry, then archive the change on the branch (`/opsx:archive`).
- Gate 3: the human merges the pull request.
- Update the README roadmap, since M3 then lacks only the git hooks, and update the M3 memory.
