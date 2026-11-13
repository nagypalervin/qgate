# Tasks

Two pull requests with the 0.6.0 release between them (design D9).
Tests come first in every task; every commit passes the pre-commit
checks; commit messages and PR bodies are written to files first.

## 1. C# rules (PR 1, `change/add-csharp-architecture`)

- [x] 1.1 `qgate-lang`: failing adapter tests for each form of "Usings that count" and "Using targets" (the four directive forms, `#if`, block namespace, file-scoped namespace, using statements, `global::`, `@`, type arguments, tuple/pointer/array/built-in aliases, an extern alias, comments between segments), then `FileModel::usings` and the C# walk (D1, D2). TypeScript models keep an empty list.
- [x] 1.2 `qgate-config`: failing tests for `to_namespaces` (only `to`, only `to_namespaces`, both, neither, an empty list of each), then the field and the "neither" error naming the rule, `to` and `to_namespaces` (D3).
- [x] 1.3 `qgate-analyzers`: failing tests for `NamespacePatterns` (below a name, not a prefix, `*`, `**.Data`, `.**` matching the name itself, braces, empty, empty segment, leading and trailing `.`, `/`), then the type (D4); then analyzer tests with C# fixtures for a new using, an existing one written another way, a rename into and within `from`, two usings of one target, the C# message, and `to` and `to_namespaces` not crossing languages, then the analyzer change (D5).
- [x] 1.4 `qgate-cli`: failing end-to-end tests for the scenarios of "Forbid rules" ("Domain using the infrastructure", "Each list applies to its language"), "Existing using, written another way", "Same namespace used twice", "Usings that count", "As written inside a namespace", the invalid namespace pattern and the two configuration scenarios, plus a generated and an excluded C# file with a forbidden using, then the copy of `to_namespaces` (D6).
- [x] 1.5 README: the architecture section's C# rules (`to_namespaces`, namespace patterns, which usings count, the limits of D7 and the risks); the C# section no longer says rules do not apply; check each sentence against a test.
- [ ] 1.6 Version 0.6.0 in `Cargo.toml` (with `Cargo.lock`) and every `QGATE_VERSION` of `action.yml`; both release checks pass locally for `v0.6.0`.
- [ ] 1.7 Evaluation (D8) on `jasontaylordev/CleanArchitecture` and `dotnet/eShop` with layer rules and `exclude = ["**/Migrations/**"]`: findings checked by hand, time per commit, nested and global usings counted; record it in `design.md`. A finding the spec does not explain stops the release until Ervin decides.

## 2. Release 0.6.0 (between the pull requests)

- [ ] 2.1 After PR 1 merges and Ervin agrees, push `v0.6.0` and check the draft: six archives, `SHA256SUMS`, `sha256sum --check`, the Linux arm64 binary printing `qgate 0.6.0` and reporting a forbidden `using`. Ervin publishes. Record it in `design.md` on `change/add-csharp-architecture-2`.

## 3. README, roadmap and rehearsal (PR 2, `change/add-csharp-architecture-2`)

- [ ] 3.1 Every README install block, `uses:` and the pinned commit at 0.6.0; `grep` finds no `0.5.0`. The roadmap's "Languages" row names the C# rules, and the "Later" row no longer lists them.
- [ ] 3.2 Azure rehearsal (D8): the README's Linux job at 0.6.0 in the rehearsal project, on a pull request that adds a forbidden `using` to a domain file; read the comment thread and the build issues through the REST API and record them in `design.md`. Abandon the pull request afterwards.

## Workflow follow-up

- Run the `reviewer` subagent on each pull request and fix its findings.
- On PR 2: the `docs/journal/` entry, then `/opsx:archive`.
- Gate 3: Ervin merges each pull request.
