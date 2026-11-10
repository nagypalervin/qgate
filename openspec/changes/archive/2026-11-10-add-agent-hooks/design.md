# Design: add-agent-hooks

## Context

- **`qgate-cli`** (about 735 lines in `main.rs`):
  - parses the command line with clap;
  - runs `check_revisions(base, head)`, which discovers the repository
    from the current directory, takes a snapshot of the working tree
    for `--worktree`, and returns sorted findings or an error string;
  - maps `FailOn` to the failing count and the exit code, and renders
    text or JSON through `qgate-report`.
- **The Claude Code hook protocol** (checked against
  https://code.claude.com/docs/en/hooks during exploration):
  - **Input.** A `Stop` hook gets a JSON object on standard input,
    including `hook_event_name`, `cwd` and `stop_hook_active`. The last
    is `true` when Claude is already continuing because a stop hook
    blocked.
  - **Output.** With exit code 0, the hook may print
    `{"decision":"block","reason":"..."}` to send the model back with
    `reason`, and/or `systemMessage`, which is shown to the user.
  - **Exit codes.** 2 also blocks, with standard error going to the
    model. Any other code is a non-blocking error shown to the user.
  - **Loop cap.** After 8 consecutive blocks, Claude Code ends the turn
    itself.
  - **Timeout.** 600 seconds by default.
  - **Project directory.** `CLAUDE_PROJECT_DIR` is the project root;
    `cwd` follows the agent.

See `proposal.md` for the motivation, and the `agent-hooks` spec for
the required behaviour.

## Goals / Non-Goals

**Goals:**
- The protocol decision (block, tell the user, or stay silent) is a pure
  function of the input, the check's result and the options, so unit
  tests cover every branch without git.
- `qgate check` does not change at all.

**Non-Goals:**
- A general hook framework for several agents. A second agent would get
  its own subcommand next to `claude-code`; shared parts are extracted
  then, not now.
- Checking that Claude Code actually honours the output. That is
  verified once by hand (task 3.3), not in CI.

## Decisions

### D1. The protocol lives in a `hook` module of `qgate-cli`

- **Where:** `crates/qgate-cli/src/hook.rs`. It holds:
  - the input type;
  - the decision function `respond(input, result, options) -> Response`;
  - the output type;
  - the message texts.

  `main.rs` only reads standard input, runs the check and writes the
  response.
- **Why not `qgate-report`:** the protocol is more than rendering. It
  parses input, and it decides whether to block from `stop_hook_active`.
  `qgate-report` must stay free of that logic (CLAUDE.md, "No analysis
  logic here"). The hook does use `render_text` from `qgate-report` for
  the finding lines.
- **Why not a new crate:** nothing else uses the protocol, and the plugin
  that may come later runs the same binary. A new crate would also need
  an ADR. If an MCP server or a second agent needs the protocol later,
  it can move then.

### D2. The check is reused, with the directory passed in

- **The change:** `check_revisions` gets the directory to discover the
  repository from as a parameter. Today it reads the current directory
  itself.
  - `qgate check` passes the current directory, so its behaviour does
    not change.
  - The hook passes the project directory: `CLAUDE_PROJECT_DIR`, else
    the input's `cwd`, else the current directory.
- **Why the project directory first:** `cwd` follows the agent, which may
  `cd` into a subdirectory or elsewhere. The project root is where the
  session and `.claude/settings.json` belong.
- **Alternative:** a `cd "$CLAUDE_PROJECT_DIR" &&` in the settings
  command. That is shell-specific (Windows), and easy to get wrong in
  each team's copy.

### D3. Input and output with serde

- **Input:** a struct with:
  - `hook_event_name: Option<String>`;
  - `stop_hook_active: Option<bool>` (absent counts as `false`);
  - `cwd: Option<PathBuf>`.

  Unknown fields are ignored, because Claude Code adds fields over
  time. Input that is not a JSON object, or whose `hook_event_name` is
  not `Stop`, is an error for the user.
- **Output:** a struct with `decision`, `reason` and `systemMessage`,
  each optional and left out when empty, written as one line followed
  by a newline. With nothing to say, nothing is written.
- **Dependencies:** `serde` and `serde_json` become normal dependencies of
  `qgate-cli`. Both are workspace dependencies already, and locked.

### D4. Every outcome exits with 0, except a broken command line

- **The rule:** all check errors and input errors become a
  `systemMessage` with exit code 0. The user sees them, the model does
  not, and the turn ends.
- **A broken command line** (an unknown `--fail-on` level, an unknown
  option) is the one exception. It happens before any input is read,
  and clap prints the usage message. For `qgate hook`, the CLI maps
  clap's usage exit code 2 to 1, which Claude Code shows to the user as
  a non-blocking hook error. `--help` and `--version` keep exit code 0.
- **Why not exit 1 for every error:** a non-blocking hook error is less
  visible in Claude Code than a `systemMessage`. That is fine for a
  mistyped settings entry, which shows on the first run, but not for an
  invalid `qgate.toml` that appears in the middle of a session.

### D5. Messages

- **The finding lines:** each list is the text format's lines from
  `render_text`, in the check's order, without the trailing newline.
- **At most 20 findings per list.** Then a line
  `... and <k> more; run qgate check --base <base> --worktree to see all`.
  - **Why 20:** it keeps the model's context small. Claude Code may
    move hook output above 10,000 characters into a file, and 20 lines
    of about 150 characters stay well below that.
- **The texts:**
  - **reason:** `qgate: your changes made code worse (compared with
    <base>):`, the lines, then `Fix these, or if a change is intended,
    tell the user why and stop.`;
  - **below the level:** `qgate: below the --fail-on level "<level>":`
    and the lines;
  - **after the round:** `qgate: these findings remain after one round
    of fixing:` and all the findings;
  - **errors:** `qgate: ` and the same message `qgate check` prints after
    `error: `.

  When a block also has findings below the level, both are written: the
  `reason` for the model and the `systemMessage` for the user.
- **After the round,** the list holds every finding, not only the
  failing ones. The user decides about all of them at once.
- **These texts are not a versioned format.** They are read by a model
  and a person, and may be reworded.

### D6. The working tree, every turn

- **What runs:** the hook runs the same snapshot as `--worktree` (a
  private index, `add -A`, `write-tree`, `commit-tree`) at the end of
  every turn, including turns that changed nothing.
  - With no change, the diff against `HEAD` is empty, and no file is
    parsed.
  - Unchanged files are not written again: git stores objects by
    content.
  - The unreferenced snapshot commits are pruned by `git gc`, as for
    `--worktree`.
- **Measurement (task 3.2):** the time for a turn with no change and
  for a turn with one changed file, in a generated repository of about
  2,000 TypeScript files.
  - **Budget:** 2 seconds for the no-change turn.
  - **Over budget:** the design is revisited before the PR, for
    instance with a `git status` shortcut. It is not shipped silently.

### D7. Tests

- **Unit tests** in `hook.rs` cover `respond` with findings built by
  hand: every branch of the spec, the 20-finding limit, and the exact
  texts.
- **End-to-end tests** in a new `crates/qgate-cli/tests/hook.rs` cover
  every scenario. They feed input on standard input to the built binary
  in temporary repositories, using the shared helpers in `tests/support`.
  `CLAUDE_PROJECT_DIR` is removed from the environment of every test, so
  the developer's own session cannot leak into it.
- **A manual check in a real Claude Code session (task 3.3):**
  - one blocked stop and the model's fix;
  - a `systemMessage` the user sees;
  - an error that does not reach the model.

## Implementation Notes

- **The hook (tasks 1.2-2.4).**
  - `Cargo.lock` gains only the `serde` and `serde_json` edges of
    `qgate-cli`, no new package.
  - Input that is valid JSON but not an object (`[]`, a string) is
    rejected before serde reads it, because a derived struct would also
    accept a JSON array.
  - Missing `hook_event_name` counts as invalid hook input; another event
    name gets its own message naming it.
  - An empty `CLAUDE_PROJECT_DIR` counts as unset.
  - `qgate hook` without `claude-code` is a usage error too, so it also
    exits with 1. A failure to write standard output exits with 1 and a
    message on standard error, since there is no response to give.
  - `qgate hook claude-code --base` defaults to `HEAD` in clap, so
    `--help` shows the default.
  - "The same order as `qgate check`" has its own end-to-end test. It
    compares the hook's lines with `qgate check`'s for five mixed Errors
    and Warnings in three files, and it fails when the hook reverses its
    list.
- **Mutations (task 2.5).** Each of these fails unit and end-to-end tests:
  - blocking although `stop_hook_active` is `true`;
  - sending every finding to the model;
  - blocking with a check error as the `reason`;
  - listing every finding;
  - ignoring `CLAUDE_PROJECT_DIR`;
  - keeping exit code 2 for a usage error.
- **README (task 3.1).** Every statement was run in a temporary
  repository, calling the hook as Claude Code does:
  - the settings entry parses;
  - an Error blocks, and the second round goes to the user;
  - a Warning goes to the user;
  - committed work is not checked again, but `--base main` sees it;
  - `--fail-on never` and `--fail-on warning` behave as documented;
  - an invalid `qgate.toml` goes to the user.
- **Measurement (task 3.2), within the budget of D6.** In a generated
  repository of 2,000 TypeScript files (about 100,000 lines, varied
  structure), with the release build on this development machine (an
  ARM64 Linux VM with 4 cores):
  - **a turn with no change:** 0.01 s. The whole snapshot runs (an
    `strace` showed `add -A`, `write-tree`, `commit-tree`, `diff` and
    `ls-tree`); git is that fast on a clean index;
  - **a turn with one changed file:** 0.60 s, almost all of it the
    duplication index over the other 1,999 files;
  - **a turn with one new file** (a copy of another): 0.60 s.

  No shortcut is needed.
- **Manual check (task 3.3).** Four `claude -p` runs in a temporary
  repository with the README's settings entry (Claude Code 2.1.319, Sonnet,
  `--include-hook-events`, the release build on the `PATH`):
  - **A block and the model's fix.** Asked to add discounts to a
    function, the model raised its complexity by 5. The hook blocked, and
    the model got the `reason` as "Stop hook feedback". It moved the logic
    into helpers and lookup tables, and the second run of the hook wrote
    nothing. To get there, the model dropped an early return, and it told
    the user.
  - **An explanation instead of a fix.** When the prompt required
    everything in one function, the model answered the block by
    explaining why the increase was intended, and stopped. The second run
    saw `stop_hook_active` and sent the remaining finding to the user.
  - **A Warning.** A +2 increase reached the user as a notice, not the
    model, and the turn ended.
  - **An invalid `qgate.toml`.** The parse error reached the user as a
    notice. The model's transcript does not contain it.

  Two things Claude Code does that qgate does not control:
  - **A deliberate block is labelled as an error.** Claude Code also
    raised a "Stop hook error occurred" notification, even though qgate
    exited with 0 and Claude Code recorded the outcome as a success.
  - **Notices are prefixed.** Every line of a `systemMessage` was shown
    with a "Stop says: " prefix.

  The interactive interface was not checked; `-p` has none.

## Risks / Trade-offs

- **[Another tool's Stop hook has already blocked.]** Once any Stop hook
  has sent the agent back in a turn, `stop_hook_active` is `true` at the
  next stop, and qgate does not block again in that turn, even for a
  finding it has not reported yet.
  - **Mitigation:** the user still sees the findings in the
    `systemMessage`, and the README says why qgate did not send the agent
    back.
- **[The agent commits its own work.]** When the agent runs `git commit`
  in its turn, the working tree is clean against `HEAD` at `Stop`, and the
  hook reports nothing about that commit.
  - **Mitigation:** the README says so, and suggests `--base main` for
    sessions in which the agent commits. The pull request still checks
    the whole branch.
- **[The user's own uncommitted work.]** The hook cannot tell the agent's
  changes from the user's. A turn that only answered a question still
  sends the agent back for an Error in work the user has not committed,
  and the agent may then edit it.
  - **Mitigation:** the README says so and suggests committing or
    stashing first. (`--fail-on never` is no quick switch: it lives in
    the team's committed settings.) Telling the changes apart would
    need the hook to remember the tree at the start of the turn, which
    is out of scope here.
- **[A repository with no commits.]** `HEAD` does not exist yet, so
  every turn shows the user `qgate: unknown revision`, as `qgate check
  --base HEAD --worktree` would. This is kept: it is an error for the
  user, as the spec requires, and it ends with the first commit.
- **[Claude Code changes its protocol.]**
  - **Mitigation:** the end-to-end tests fix the bytes qgate writes, the
    manual check (task 3.3) confirms them against Claude Code, and the
    README links to the hook documentation.
- **[Repository hooks run without asking in `claude -p`.]** A team that
  runs Claude non-interactively on untrusted repositories also runs their
  hooks.
  - **Mitigation:** a README warning. qgate itself only reads the
    repository and writes unreferenced git objects.
- **[The agent explains instead of fixing.]** A model may answer "this is
  intended" too readily.
  - **Mitigation:** the user sees its explanation, and the finding stays
    visible in the pull request. The instruction asks for the reason to
    be given to the user, not only stated.
- **[The model might treat the finding text as instructions.]** A
  message can quote an import specifier from the code.
  - **Mitigation:** the text format quotes it, as in the terminal. The
    `reason` is qgate's own text around those lines.

## Migration Plan

None. The command is new, and nothing runs it until a team adds it to
its settings.

## Open Questions

None. Behaviour is decided in the spec, and the measurement in D6 has a
fixed budget and a fallback.
