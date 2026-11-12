# 2026-11-12: add-pr-comments

**Went well:**
- **The script ran against the real API before the README did.** PR 1
  ran the comment script in the rehearsal project through every state
  of the thread, and once with the permission removed. That run found
  the one bug unit tests could not: Azure DevOps escapes the quotes in
  its JSON error, so the message lost the identity's name.
- **A test that passed for the wrong reason.** The Pester test for that
  bug held plain quotes, because the agent's tool input decoded the
  `'` it typed. The test now builds the escape at runtime, and
  the trap is in the agent's memory.
- **Two PRs with a release between them.** The README job downloads
  the script from the 0.2.0 archive, so the docs could only be
  rehearsed once 0.2.0 was out. Splitting at the release kept each PR
  reviewable and made the rehearsal use what partners will use.
- **The README is still what ran.** As in add-azure-pipelines, the
  jobs were cut out of the README and compared with `diff`; five
  scenarios on two agents, plus a non-ASCII path to check the Windows
  redirection.

**Had to step in:** These were the human's calls:
- Gate 1 and Gate 2, and installing PowerShell locally;
- removing and restoring the Build Service permission;
- the required checks in the ruleset;
- reverting a runner pin built on a wrong diagnosis, and waiting out
  the outage instead;
- pushing the tag through the agent this time, and publishing;
- looking at the comment in the web UI.

**Surprise:**
- **A GitHub runner DNS outage looked like an image change.** The
  Linux release builds could not resolve the Ubuntu mirrors on the day
  `ubuntu-latest` was moving to 26.04. The agent pinned 24.04, and the
  next run failed the same way on 24.04. The pin was reverted as a new
  commit; the outage cleared hours later.
- **The review caught a living spec that would have gone stale.** PR 2
  changed what the job prints, but the delta did not modify the
  requirement that promised text-format log lines. Archiving would have
  merged a scenario that the documented job no longer meets.
- **`#Requires` above the help hides the help.** PowerShell's
  `Get-Help` found nothing until the line moved below the comment block.

**Effort:** two days, 2 PRs, 2 review rounds, about 30 rehearsal builds
(the script in PR 1, the README jobs in PR 2), one release.

**Blog idea:** "Rehearse the failure path through the docs." A unit
test proves the script fails well; only a run of the copied job proves
that the failure reaches the person reading the build. The review asked
for exactly that run, and it took ten minutes.
