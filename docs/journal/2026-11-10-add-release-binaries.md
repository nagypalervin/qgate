# 2026-11-10: add-release-binaries

**Went well:**
- **A spike before the proposal, again.** Running the suite on all six
  runners from a throwaway branch took ten minutes. It turned "Windows
  will be a lot of work" into two test assumptions, and showed that
  qgate's behaviour needed no change. The proposal was small because
  of it.
- **The release was rehearsed before the tag.** The workflow ran on its
  own pull request, publishing aside, and the agent checked the
  rehearsal's archives. The real `v0.1.0` run then passed on its first
  attempt. Its arm64 binary has the same BuildID as the rehearsal's.
- **Checks that would notice their own failure.** The Windows import
  check fails unless it sees `kernel32.dll`, so an empty listing cannot
  pass as "no VCRUNTIME". The version script is tested with a wrong tag
  on every rehearsal.

**Had to step in:** These were the human's calls:
- "every platform", including Intel Macs and Windows arm64;
- deferring code signing;
- Gate 1 and Gate 2, and the Gate 2 amendment for the static C runtime
  on Windows;
- pushing the tag and publishing the draft, the release's two human
  gates;
- the three merges.

**Surprise:**
- **The first review of the plan found two defects that would have
  broken or weakened every release:**
  - `file` calls the x64 musl binary `static-pie linked`, which a check
    for "statically linked" would have rejected;
  - MSVC binaries need the Visual C++ Redistributable, which GitHub's
    runners have and many Windows machines lack.

  Both were in the approved design, and neither would have shown up in
  CI. Reviewing the plan when the first pull request landed, not only
  the code, paid off here.
- **The new platform checks were not required.** CI ran them, but the
  repository's ruleset only required the two old checks. A test job
  is not a gate until the ruleset says so.
- **Pasted install commands do not stop on failure.** The README's
  first version would have unpacked and installed an archive whose
  checksum had just failed.

**Effort:** one session, 3 PRs, 2 review rounds, 1 release.

**Blog idea:** "Rehearse the release." A release workflow is code that
runs once per version, with write access, and is usually tested in
production. Running everything but the last step on the pull request
that changes it made the first real tag uneventful.
