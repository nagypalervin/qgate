# agent-hooks Specification

## Purpose

Defines how qgate runs from an AI coding agent's hooks, so the agent
learns what its uncommitted work made worse before it hands the work
back, starting with `qgate hook claude-code` for Claude Code's `Stop`
hook.

## Requirements

### Requirement: Hook command

`qgate hook claude-code [--base <rev>] [--fail-on <level>]` SHALL read
one JSON object, the hook input, from standard input. It SHALL answer in
Claude Code's hook protocol: exit code 0, and either nothing or one JSON
object on standard output. `--base` defaults to `HEAD` and `--fail-on`
to `error`, with the levels of `qgate check`. `qgate --help` SHALL list
the `hook` command.

#### Scenario: Nothing to report

- **GIVEN** a repository whose working tree has no change since `HEAD`
- **WHEN** Claude Code runs `qgate hook claude-code` with `{"hook_event_name":"Stop","stop_hook_active":false}` on standard input
- **THEN** standard output is empty
- **AND** the exit code is 0

#### Scenario: Help lists the hook command

- **WHEN** the user runs `qgate --help`
- **THEN** the output mentions the `hook` command

### Requirement: What the hook checks

The hook SHALL report the findings that `qgate check --base <base>
--worktree --fail-on <level>` reports in the project directory, in the
same order and with the same severities. The project directory SHALL be
`CLAUDE_PROJECT_DIR` when that variable is set, else the input's `cwd`,
else the current directory.

#### Scenario: Uncommitted work only

- **GIVEN** a committed change that made `f` in `src/a.ts` go from 2 to 9, and an uncommitted change that makes `g` in `src/b.ts` go from 2 to 9
- **WHEN** Claude Code runs `qgate hook claude-code`
- **THEN** the hook reports the finding for `g` and not the one for `f`

#### Scenario: Another base

- **GIVEN** the repository of the previous scenario, on a branch made from `main` before both changes
- **WHEN** Claude Code runs `qgate hook claude-code --base main`
- **THEN** the hook reports the findings for both `f` and `g`

#### Scenario: Project directory

- **GIVEN** `CLAUDE_PROJECT_DIR` names a repository with an uncommitted finding, and the input's `cwd` and the current directory are outside any repository
- **WHEN** Claude Code runs `qgate hook claude-code`
- **THEN** the hook reports that finding

### Requirement: Sending the agent back

When findings are at or above the `--fail-on` level and the input's
`stop_hook_active` is not `true`, the hook SHALL block the stop: its
output SHALL have `"decision":"block"` and a `reason` for the model that
lists those findings in the text format and asks the model to fix them
or to tell the user why a change is intended.

#### Scenario: One failing finding

- **GIVEN** an uncommitted change that makes `f` in `src/a.ts` go from 2 to 9
- **WHEN** Claude Code runs `qgate hook claude-code` with `{"hook_event_name":"Stop","stop_hook_active":false}`
- **THEN** standard output is one JSON object with `"decision":"block"`
- **AND** its `reason` is `qgate: your changes made code worse (compared with HEAD):\nerror   src/a.ts:1  f: cyclomatic complexity 2 -> 9 (+7)  [complexity/cyclomatic-increase]\nFix these, or if a change is intended, tell the user why and stop.`
- **AND** the exit code is 0

#### Scenario: Failing on warnings

- **GIVEN** an uncommitted change that adds a copy of a block that exists in another file
- **WHEN** Claude Code runs `qgate hook claude-code --fail-on warning`
- **THEN** the output blocks the stop, and its `reason` lists the `duplication/duplicate-block` finding

### Requirement: One round of fixing

When the input's `stop_hook_active` is `true`, the hook SHALL NOT block
the stop, whatever it finds. Findings that remain SHALL be shown to the
user in a `systemMessage`, so the user decides about them.

#### Scenario: Findings remain after the round

- **GIVEN** an uncommitted change that makes `f` in `src/a.ts` go from 2 to 9
- **WHEN** Claude Code runs `qgate hook claude-code` with `{"hook_event_name":"Stop","stop_hook_active":true}`
- **THEN** standard output is `{"systemMessage":"qgate: these findings remain after one round of fixing:\nerror   src/a.ts:1  f: cyclomatic complexity 2 -> 9 (+7)  [complexity/cyclomatic-increase]"}` followed by a newline
- **AND** the exit code is 0

#### Scenario: Fixed in the round

- **GIVEN** the agent removed every finding in its round of fixing
- **WHEN** Claude Code runs `qgate hook claude-code` with `"stop_hook_active":true`
- **THEN** standard output is empty

### Requirement: Findings below the level

Findings below the `--fail-on` level SHALL NOT block the stop and SHALL
NOT be sent to the model. The hook SHALL show them to the user in a
`systemMessage`, next to a block when there is one.

#### Scenario: Only a warning

- **GIVEN** an uncommitted change that makes `f` in `src/a.ts` go from 2 to 4
- **WHEN** Claude Code runs `qgate hook claude-code` with `"stop_hook_active":false`
- **THEN** standard output is `{"systemMessage":"qgate: below the --fail-on level \"error\":\nwarning src/a.ts:1  f: cyclomatic complexity 2 -> 4 (+2)  [complexity/cyclomatic-increase]"}` followed by a newline

#### Scenario: A warning next to an error

- **GIVEN** an uncommitted change that makes `f` go from 2 to 9 and `g` from 2 to 4
- **WHEN** Claude Code runs `qgate hook claude-code` with `"stop_hook_active":false`
- **THEN** the `reason` lists only the finding for `f`
- **AND** the `systemMessage` lists only the finding for `g`

#### Scenario: Never failing

- **GIVEN** an uncommitted change that makes `f` go from 2 to 9
- **WHEN** Claude Code runs `qgate hook claude-code --fail-on never`
- **THEN** the output does not block the stop, and its `systemMessage` lists the finding for `f`

### Requirement: Long lists

Each list in a `reason` or `systemMessage` SHALL show at most 20
findings. When there are more, its last line SHALL be `... and <k>
more; run qgate check --base <base> --worktree to see all`.

#### Scenario: Twenty-five failing findings

- **GIVEN** an uncommitted change with 25 findings at or above the level
- **WHEN** Claude Code runs `qgate hook claude-code` with `"stop_hook_active":false`
- **THEN** the `reason` lists the first 20 findings in the text format's order
- **AND** then `... and 5 more; run qgate check --base HEAD --worktree to see all`

### Requirement: Errors go to the user

When the check cannot run (no git, no repository, an unknown base, an
invalid `qgate.toml`), or the input is not a JSON object for the `Stop`
event, the hook SHALL NOT block the stop: its output SHALL be a
`systemMessage` starting with `qgate: ` and naming the problem, with
exit code 0. An invalid command line SHALL exit with code 1, never 2.

#### Scenario: Invalid configuration

- **GIVEN** a repository whose working tree has an invalid `qgate.toml`
- **WHEN** Claude Code runs `qgate hook claude-code`
- **THEN** standard output is one JSON object with a `systemMessage` that names `qgate.toml`, and no `decision`
- **AND** the exit code is 0

#### Scenario: Outside a repository

- **GIVEN** a project directory that is not in a git repository
- **WHEN** Claude Code runs `qgate hook claude-code`
- **THEN** the output is a `systemMessage` and does not block the stop

#### Scenario: Input that is not hook input

- **WHEN** `qgate hook claude-code` gets `not json` on standard input
- **THEN** the output is a `systemMessage` that says the input is not valid hook input
- **AND** the exit code is 0

#### Scenario: Another hook event

- **WHEN** `qgate hook claude-code` gets `{"hook_event_name":"PostToolUse"}` on standard input
- **THEN** the output is a `systemMessage` that says only the `Stop` event is supported

#### Scenario: Unknown fail-on level

- **WHEN** Claude Code runs `qgate hook claude-code --fail-on info`
- **THEN** standard error names the allowed levels
- **AND** standard output is empty
- **AND** the exit code is 1

### Requirement: The hook changes nothing

The hook SHALL leave the repository as `qgate check --worktree` does
(see `revision-input`): the index, `HEAD`, branches and files stay as
they are.

#### Scenario: Staged work stays staged

- **GIVEN** a staged change and an unstaged change
- **WHEN** Claude Code runs `qgate hook claude-code`
- **THEN** `git status --porcelain` shows the same output before and after
