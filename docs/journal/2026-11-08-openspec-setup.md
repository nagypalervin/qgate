# 2026-11-08: OpenSpec setup

**Went well:** `openspec init --tools claude` ran non-interactively and
installed six `/opsx:*` commands plus matching skills; nothing else in the
repo was touched.

**Had to step in:** The machine had no Node.js (apt only offers 18; OpenSpec
needs 20.19+) and no passwordless sudo. The agent tried to install nvm by
piping a remote script into bash; Claude Code's auto mode blocked it as
external code. The human installed Node instead, which is the right split.

**Surprise:** OpenSpec telemetry is opt-out and has no per-project switch,
only an environment variable or a per-machine config. We set
`OPENSPEC_TELEMETRY=0` in the project's `.claude/settings.json`, so it is off
for every Claude Code session here; plain terminal runs still need the
variable.

**Effort:** short session, no review rounds.

**Blog idea:** "The guardrail that stopped my agent from `curl | bash`."
