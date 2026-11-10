# 2026-11-10: add-mcp-server

**Went well:**
- **A spike before the proposal.** A 60-line logging server in Python
  showed what Claude Code really sends: `server/discover` first, then a
  fallback to `initialize`. The proposal could therefore commit to
  serving both protocol eras on evidence rather than on reading the
  specification.
- **The hand check settled the open question.** VS Code 1.145 with
  Copilot speaks the old era, and Claude Code the new one. The two
  target clients really do differ, so serving both was needed, not just
  cautious.
- **No SDK, no new dependency.** The protocol is one module of about
  440 lines plus its tests, written with `serde_json`. The price showed
  in review: an SDK would have checked ids and the `jsonrpc` field for
  us. Each of those fixes was a few lines.
- **New tests found a bug in a claim.** An end-to-end test for input
  that is not UTF-8 showed that the lossy conversion turned a bad byte
  inside a JSON string into a valid character. The design note had
  claimed the opposite.

**Had to step in:** These were the human's calls:
- the scope (client-independent, VS Code with Copilot as the second
  client);
- Gate 1 and Gate 2;
- the hand check in VS Code, since the agent cannot drive Copilot Chat;
- the merge.

**Surprise:**
- **MCP changed under the plan.** The 2026-07-28 revision, four months
  old, removed the handshake and `ping`, deprecated `roots` and added
  `server/discover`. Without a web check during explore, the design
  would have targeted the old revision only.
- **Claude Code ignores `cwd` in `.mcp.json`.** The README advised it
  anyway, untested. The reviewer found this by reading the client's
  schema.
- **A slip of the agent's own:** a mutation loop restored the file with
  `git checkout` and wiped the uncommitted review fixes. They were
  applied again from the same script, and the loop now restores from a
  copy.

**Effort:** one session, 1 PR, 1 review round and a verification round,
two hand checks.

**Blog idea:** "Test the protocol your clients speak, not the one the
spec describes." A logging stub server of a few dozen lines answered in a
minute what reading a changelog could not: which revision each real
client speaks today.
