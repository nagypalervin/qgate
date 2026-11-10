# Proposal: add-agent-hooks

## Why

M3 is about checking while coding. `qgate check --base HEAD --worktree
--format json` can already tell an AI coding agent what its uncommitted
work made worse, but nothing runs it at the right moment, and its exit
codes do not fit an agent's hook protocol. Claude Code, the agent qgate's
users and partner teams work with, runs hook commands at fixed points
and treats exit code 2 as "send this back to the model". In qgate, 2
means "the check could not run", and a failing check exits with 1,
which Claude Code ignores. A hook that calls `qgate check` directly
would therefore send setup errors to the model and let real regressions
through.

## What Changes

- **New command `qgate hook claude-code`**, run by a Claude Code `Stop`
  hook when the agent finishes a turn:
  - it reads the hook's input from standard input and checks the
    project's working tree against `HEAD`. So it sees only work that is
    not committed yet; committing counts as accepting a change;
  - **findings at or above `--fail-on`** (default `error`) send the agent
    back to work once. Claude Code gives the model the findings in the
    text format, and asks it to fix them or to tell the user why a
    change is intended;
  - **no second round:** if findings remain after that one round, the
    agent may stop, and the user sees what remains. The loop always
    ends;
  - **findings below the level** (Warnings, by default) are shown to the
    user, not the model;
  - **errors** (no git repository, an invalid `qgate.toml`, invalid
    input) are shown to the user and never sent to the model;
  - **options:** `--base <rev>` checks against another revision, such
    as `main`; `--fail-on` works as for `qgate check`;
  - it speaks Claude Code's hook protocol: JSON on standard output and
    exit code 0.
- **README:** a "With Claude Code" section with the `.claude/settings.json`
  entry a team commits to its repository, and a warning that `claude -p`
  and SDK sessions run a repository's hooks without asking.

### Decisions taken during exploration

- **Claude Code only.** It is what qgate's users run. Other agents get
  the MCP server (`add-mcp-server`), which works with any of them.
- **A qgate command, not a shell recipe around `qgate check`.** The
  protocol mapping, the exit codes and the loop guard are then tested
  in qgate, and teams need no `jq` or `sh` (which matters on Windows).
- **At the end of a turn (`Stop`), not after every edit.** It checks
  finished work once, instead of half-done refactors after every file
  write.
- **`HEAD` as the default base.** With `main`, a finding the user had
  already committed and accepted would send the agent back at the end of
  every turn, even one that only answered a question. The whole branch
  is checked in the pull request (CI, M4) and through the MCP server.
- **One round of fixing, then the user decides.** The agent may disagree
  with a finding, for instance about a function that has to be complex.
  It gets one chance to fix or explain, and the human decides about
  what remains.
- **`--fail-on error` by default, as for `qgate check`.** With `warning`,
  almost every coding turn would get an extra round (one `if` is +1
  complexity), and the hook would soon be switched off. Teams that want
  duplication and small complexity increases fixed by the agent use
  `--fail-on warning`.

### Out of scope

- A Claude Code plugin to install the hook. It can be added later on
  top of this command.
- A git pre-commit hook. That needs a check of the staged changes
  (`--staged`), a separate `revision-input` change.
- Other agents and other Claude Code hook events (`PostToolUse`,
  `SubagentStop`).
- Remembering findings the user accepted without committing.

## Capabilities

### New Capabilities

- `agent-hooks`: running qgate from a coding agent's hooks. It starts
  with `qgate hook claude-code`: its input, the base it checks against,
  when it sends the agent back, what the model and the user see, and how
  the loop ends.

### Modified Capabilities

None. The hook reuses the working-tree check (`revision-input`) and the
findings and levels of `check-command` without changing them.

## Impact

- **`qgate-cli`:**
  - a `hook` command with a `claude-code` subcommand;
  - the protocol mapping in its own module;
  - `serde` and `serde_json` become normal dependencies (to read the
    hook input and write its output). Both are already in the workspace
    and in `Cargo.lock`.
- **No change** to `qgate check`, the analyzers, the report formats or
  the other crates.
- **Docs:** README.
- **Users:** none affected until a team adds the hook to its
  `.claude/settings.json`.
