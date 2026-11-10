# 2026-11-10: add-agent-hooks

**Went well:**
- **The protocol as a pure function.** Whether to block, tell the user or stay silent depends only on the input, the check's result and the options. Unit tests covered every branch without git, and all six deliberate mutations failed tests.
- **Measuring before optimising.** The design had a 2-second budget and a fallback (a `git status` shortcut). The no-change turn took 0.01 s in a 2,000-file repository, so the shortcut was never written.
- **Testing the hook with a real agent was cheap.** Four `claude -p` runs with `--include-hook-events` cost about 0.21 USD. They showed exactly what Claude Code did with each response, which the end-to-end tests cannot show.

**Had to step in:** These were the human's calls:
- Claude Code only;
- `HEAD` as the default base;
- the blocking policy (one round, Errors only);
- Gate 1 and Gate 2;
- agreeing to run a real Claude Code session;
- the merge.

**Surprise:**
- **The model behaved differently depending on the prompt.** Told to keep everything in one function, it explained the complexity and stopped. Without that constraint, it refactored into helpers and lookup tables. It also dropped an early return to bring the metric down, and told the user. Both are what the protocol asks for, but the second shows that an agent will reshape code to satisfy a metric.
- **Claude Code calls a deliberate block an error.** qgate exited with 0 and Claude Code recorded the outcome as a success. Still, it raised a "Stop hook error occurred" notification.
- **The review was about whose code it is.** The code was right. The findings were all consequences of checking against `HEAD` that nobody had written down:
  - a commit the agent makes in its turn is not seen;
  - the user's own uncommitted work sends the agent back;
  - a turn that another Stop hook has already blocked uses up the round.

**Effort:** one session, 1 PR, 1 review round and a verification round.

**Blog idea:** "The hook cannot tell whose code it is." A quality gate at the end of an agent's turn sees a working tree, not authorship. Committing, stashing and the choice of base turn out to be how a team tells qgate what the agent did.
