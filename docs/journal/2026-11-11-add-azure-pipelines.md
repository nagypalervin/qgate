# 2026-11-11: add-azure-pipelines

**Went well:**
- **Measure the CI system before writing the docs.** The spike ran
  probes in a throwaway Azure DevOps project before the proposal was
  written. They found three facts a copied GitHub Actions setup would
  have hidden:
  - the checkout is a single commit;
  - the target branch reads `refs/heads/main`;
  - neither bash nor PowerShell steps stop at a failing command.

  Each became one line of the README job, with a reason.
- **The README is what ran.** The rehearsal pipelines were cut out of
  the README by a script and compared with `diff`. After the review
  changed the prose, a byte compare showed that the jobs had not moved.
- **A hybrid split with a real organization.** Ervin made the admin
  settings in the web UI: the project, the pipeline, the branch policy.
  The agent pushed branches, opened pull requests and read logs through
  the REST API. It used a narrow seven-day token that it never printed.

**Had to step in:** These were the human's calls:
- Azure DevOps Services, and both Linux and Windows agents;
- the hybrid rehearsal and the token;
- Gate 1 and Gate 2;
- what the public record may say about the organization.

**Surprise:**
- **The branch policy is not optional.** The first README presented
  the build validation policy as the step that turns the job into a
  gate. The review pointed out that in Azure Repos it is the only thing
  that starts a pull request build at all. Without it, the job is
  skipped and the pipeline stays green: a gate that silently never
  closes.
- **The README overstated the PowerShell risk.** The review caught that
  the Windows install block's `throw` always stops it. Only native
  commands slip through, and that is what the spike had measured.
- **`main` refused the agent's push.** The branch policy that the
  rehearsal needed blocked direct pushes too. Pull request builds take
  their YAML from the merge commit, so every rehearsal round ran from
  pull request branches.

**Effort:** one session, 1 PR, 1 review round, 16 rehearsal builds.

**Blog idea:** "The gate that never closes." A CI check that is skipped
looks exactly like a check that passed. When you document a merge
gate, rehearse the case where it does not run, not only the cases where
it fails and passes.
