# Tasks

One pull request, `change/add-agent-hooks`, with these planning
artifacts. Decisions made during apply go into `design.md`. Every commit
passes `cargo fmt`, `cargo clippy -D warnings` and `cargo test
--workspace` (the pre-commit hook enforces it), and CI also runs the
tests on Rust 1.85. Commit messages and PR bodies are written to files
first.

## 1. Groundwork

- [x] 1.1 Pass the directory to discover the repository from into `check_revisions` (design D2). `qgate check` passes the current directory. Verify that every existing test passes unchanged.
- [x] 1.2 Make `serde` and `serde_json` normal dependencies of `qgate-cli` (design D3), saying why in the commit message. Verify that `Cargo.lock` gains no new package.

## 2. `qgate hook claude-code`

- [x] 2.1 Write failing unit tests in `crates/qgate-cli/src/hook.rs` for the decision function (design D1, D5):
  - nothing to report: no output;
  - a failing finding: a block with the exact `reason`;
  - `stop_hook_active`: no block, and the remaining findings in the `systemMessage`;
  - findings below the level only: a `systemMessage`, no block;
  - both: a `reason` with the failing findings and a `systemMessage` with the others;
  - `--fail-on never`: never a block;
  - 21 and 25 findings: 20 lines, then the `... and <k> more` line;
  - a check error, invalid JSON, a missing or another `hook_event_name`: a `systemMessage`, never a block;
  - the output is one line, without `null`.

  Verify that they fail.
- [x] 2.2 Implement the input, the decision function and the output (design D1, D3, D5). Verify that the 2.1 tests pass.
- [x] 2.3 Write failing end-to-end tests in `crates/qgate-cli/tests/hook.rs`, one per scenario of the `agent-hooks` spec. Each builds a temporary repository with the helpers in `tests/support`, feeds the input on standard input, and removes `CLAUDE_PROJECT_DIR` from the environment unless the scenario sets it. Verify that they fail.
- [x] 2.4 Add `qgate hook claude-code` to the CLI (design D2, D4):
  - `--base` and `--fail-on`, with help text;
  - reading standard input;
  - the project directory;
  - exit code 1 instead of 2 for an invalid `qgate hook` command line.

  Verify that the 2.3 tests pass and every existing test passes unchanged.
- [x] 2.5 Check that the tests bite. Break each of these, one at a time, and confirm that a test fails for each:
  - the loop guard (block even when `stop_hook_active` is `true`);
  - the level split (send every finding to the model);
  - the error path (block with the error as `reason`);
  - the list limit (show every finding);
  - the project directory (ignore `CLAUDE_PROJECT_DIR`);
  - the exit code mapping (keep 2 for a usage error).

## 3. Docs and verification

- [x] 3.1 Update `README.md`:
  - a "With Claude Code" section: the `.claude/settings.json` entry, what the agent and the user see, `--base` and `--fail-on`, the one-round rule, and that committing accepts a change;
  - the warning that `claude -p` and SDK sessions run a repository's hooks without asking;
  - a link to the Claude Code hook documentation.

  Run every README command in a temporary repository and verify that it behaves as documented.
- [x] 3.2 Measure the hook in a generated repository of about 2,000 TypeScript files: a turn with no change, and a turn with one changed file (design D6). Record the times in `design.md`. If the no-change turn takes more than 2 seconds, stop and revisit the design before going on.
- [ ] 3.3 With the human's agreement, because it runs a real Claude Code session, check the hook by hand in a temporary repository with the README's settings entry:
  - a regression that blocks the stop, and the model's fix;
  - a Warning shown to the user;
  - an invalid `qgate.toml` that does not reach the model.

  Record the result in `design.md`.

## Workflow follow-up

- Run the `reviewer` subagent on the pull request and fix its findings.
- Write the `docs/journal/` entry, then archive the change on the branch (`/opsx:archive`).
- Gate 3: the human merges the pull request.
- Update the README roadmap and the M3 memory once the hook is merged; `add-mcp-server` comes next.
