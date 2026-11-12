# 2026-11-12: add-github-action

**Went well:**
- **A spike before the code, against the real API.** The docs name
  `issues: write` for adding a comment; on a pull request it got 403,
  and only `pull-requests: write` worked. The spike also measured the
  annotation limit (10 per kind and step), which the docs leave out.
  Both went into the design before a line of PowerShell.
- **A run before the release.** The action downloads the release that
  matches its tag, so it cannot really run until the tag is published.
  A copy pinned to the previous release ran as a local action and
  exercised install, summary and comment on real runners; only the
  gate had to wait for 0.3.0.
- **Reusing the Azure pattern.** One PowerShell script with Pester
  tests, the same CI jobs, the same rehearsal by cutting the workflow
  out of the README and comparing it with `diff`. The change took one
  day instead of two.

**Had to step in:** These were the human's calls:
- Gate 1, and letting the agent create the private rehearsal
  repository and push the tag;
- adding the macOS Pester check to the ruleset;
- publishing the release;
- looking at the comment, the job summary and the annotation in the
  web UI.

**Surprise:**
- **The plan assumed Dependabot gets a read-only token.** GitHub's docs
  say otherwise: since 2021, its workflows get what `permissions:`
  grants (not rehearsed; it needs a Dependabot pull request). The
  review caught it after the human had approved the proposal, so the
  spec changed after Gate 1.
- **"Look at this pull request" read as "merge it".** The human merged
  a test pull request in the rehearsal repository. Harmless there, but
  a request to look at something needs to say "do not merge".
- **The README promised more than the rehearsal showed.** Six
  platforms when two had run, every finding in a job summary that
  stops at 50 rows. The second review compared each sentence with the
  record; four more runner images and three more scenarios closed the
  gap in half an hour.
- **The job summary has no API.** The check run's output fields are
  empty, so only a person looking at the page can confirm it.
- **The runner masks the `::stop-commands::` token** in the log, so the
  wrapping shows up as `***`.

**Effort:** one day, 2 PRs, 2 review rounds, one spike, one pre-release
run, 18 rehearsal runs, one release.

**Blog idea:** "The docs said `issues: write`." Ten minutes of spike
against the real API saved a README that would have told every user the
wrong permission.
