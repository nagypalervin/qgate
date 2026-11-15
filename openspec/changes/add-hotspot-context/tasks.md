# Tasks

Three pull requests, with the 0.8.0 release between the second and the
third (design D7). Tests come first in every task; every commit passes
the pre-commit checks; commit messages and PR bodies are written to
files first.

## 1. History and the finding field (PR 1, `change/add-hotspot-context`)

- [x] 1.1 `qgate-git`: failing tests on temporary repositories with set committer dates (`GIT_COMMITTER_DATE`), then the history method of D1 and `is_shallow()`. Cover: a commit inside and one outside the window, a merge commit not counted, a rename followed to the head path, a file deleted before the head kept under its own path for the caller to drop (as built), a modification after a rename counted once per commit; after the review of PR 1 also renames on the merged branch (followed) and on another branch (not followed, design D1), the window's exact start, and `parse_log` on synthetic input, a shallow clone (`git clone --depth`) answering `true` and a full one `false`. Verified by `cargo test -p qgate-git`.
- [x] 1.2 `qgate-core`: `Hotspot` and `Finding::hotspot` (D4) with docs; `hotspot: None` at every struct literal in the workspace. No output changes: every existing test passes unchanged, verified by `cargo test --workspace`.

## 2. Hotspot context (PR 2, `change/add-hotspot-context-2`)

- [x] 2.1 `qgate-analyzers::hotspots`: failing unit tests for every scenario of "Hotspot files" (both conditions, simple but busy, complex but quiet, ties, the minimum of 5, complexity 0, 3-file repository) and for `annotate` (the note's text, `input/unparsable` left alone, findings outside hotspots untouched), then the ranking and the annotation (D3). Verified by `cargo test -p qgate-analyzers`.
- [x] 2.2 `qgate-report`: failing tests for the `hotspot` JSON key (present, absent) and the schema (`docs/schema/report-v1.json` validates a finding with it and rejects a non-integer `commits`), then the renderer and the schema. Verified by `cargo test -p qgate-report -p qgate-cli --test schema`.
- [x] 2.3 `qgate-cli`: failing end-to-end tests on repositories with dated commits for the scenarios of `hotspot-analysis` that run `qgate check` (a complexity increase in a hotspot in text, Markdown, Azure and JSON; a hotspot nothing made worse; an unparsable hotspot; the two-file form; the window's edge; merge commits; a rename in the window; excluded migrations; families apart; a shallow clone's standard error and unchanged exit code; `--worktree` with the snapshot not counted), and one MCP call whose finding carries `hotspot`. Then the wiring of D2 and D5. Verified by `cargo test --workspace`.
- [ ] 2.4 README: a "Hotspots" part in the analyzer sections (what a hotspot is, the note, annotation only, the window anchored at the head commit, families, excluded and generated files, shallow clones and the `fetch-depth: 0` / `fetchDepth: 0` they need, squash merges) and the JSON section's `hotspot` key; check each sentence against a test. Version 0.8.0 in `Cargo.toml` (with `Cargo.lock`) and every `QGATE_VERSION` of `action.yml`; both release checks pass locally for `v0.8.0`. Release notes drafted in `design.md`, naming the new JSON key.
- [ ] 2.5 Evaluation (D6) on the four repositories of 0.7.0, as full clones with all history and blobs (design D6, review of PR 1): hotspots per family, every annotated finding sorted by hand, the count option 2 would have raised to Error, time per commit and the `git log` share against 0.7.0; record it in `design.md` with the per-finding list and scripts under `evaluation/`. A `git log` that makes a check notably slower, or thresholds that look wrong, stop the release until Ervin decides.

## 3. Release 0.8.0 (between PR 2 and PR 3)

- [ ] 3.1 After PR 2 merges and Ervin agrees, push `v0.8.0` once `main`'s CI is green, and check the draft: six archives, `SHA256SUMS`, `sha256sum --check`, the Linux arm64 binary printing `qgate 0.8.0` and adding a hotspot note in a throwaway repository with dated commits. Ervin publishes, with the drafted release notes. Record it in `design.md` on `change/add-hotspot-context-3`.

## 4. README, roadmap and rehearsal (PR 3, `change/add-hotspot-context-3`)

- [ ] 4.1 Every README install block, `uses:` and the pinned commit at 0.8.0; `grep` finds `0.7.0` only where the roadmap dates the tests analyzer. The M5 roadmap row is done, with both halves dated.
- [ ] 4.2 Azure rehearsal (D6): the README's Linux job at 0.8.0 in `teamcollabeu/qgate-rehearsal`, on a pull request whose branch has six commits on one TypeScript file and then raises a unit's complexity there. Read the comment thread and the build issue through the REST API and record them in `design.md`: the Warning row and the issue end with the hotspot note, and the step passes. Abandon the pull request afterwards.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- On PR 3: the `docs/journal/` entry, then `/opsx:archive`.
- Gate 3: Ervin merges each pull request.
