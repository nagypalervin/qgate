# qgate

**Detects where your codebase starts to break under fast, AI-assisted development.**

> Status: early. `qgate check` compares TypeScript and [C#](#c) code
> between two git revisions (or two files) for cyclomatic complexity,
> newly duplicated code and, in TypeScript with a
> [`qgate.toml`](#architecture-rules-qgatetoml), forbidden imports. It
> reports on pull requests in GitHub Actions and Azure Pipelines; more
> analyzers follow the [roadmap](#roadmap) below.

## Try it

Download the archive for your platform from the
[latest release](https://github.com/nagypalervin/qgate/releases/latest),
check it against `SHA256SUMS`, unpack it, and put `qgate` on your
`PATH`:

| Platform | Target in the archive name |
|---|---|
| Linux x64 | `x86_64-unknown-linux-musl` |
| Linux arm64 | `aarch64-unknown-linux-musl` |
| macOS, Apple Silicon | `aarch64-apple-darwin` |
| macOS, Intel | `x86_64-apple-darwin` |
| Windows x64 | `x86_64-pc-windows-msvc` |
| Windows arm64 | `aarch64-pc-windows-msvc` |

The Linux binaries are statically linked and run on any distribution,
Alpine included (with `curl` installed). Each block below installs only
when the checksum matches. On Linux, with version 0.4.0 as the example:

```sh
v=0.4.0 t=x86_64-unknown-linux-musl
base=https://github.com/nagypalervin/qgate/releases/download/v$v
curl -fL --fail-early -O "$base/qgate-$v-$t.tar.gz" -O "$base/SHA256SUMS" &&
  grep " qgate-$v-$t.tar.gz\$" SHA256SUMS | sha256sum -c &&
  tar xzf "qgate-$v-$t.tar.gz" &&
  install -D "qgate-$v-$t/qgate" ~/.local/bin/qgate
```

On macOS, the check uses `shasum`:

```sh
v=0.4.0 t=aarch64-apple-darwin
base=https://github.com/nagypalervin/qgate/releases/download/v$v
curl -fL --fail-early -O "$base/qgate-$v-$t.tar.gz" -O "$base/SHA256SUMS" &&
  grep " qgate-$v-$t.tar.gz\$" SHA256SUMS | shasum -a 256 --check &&
  tar xzf "qgate-$v-$t.tar.gz" &&
  sudo install -d /usr/local/bin &&
  sudo install "qgate-$v-$t/qgate" /usr/local/bin/qgate
```

On Windows, in PowerShell, then add the unpacked folder to your `PATH`:

```powershell
$ErrorActionPreference = "Stop"
$v = "0.4.0"; $t = "x86_64-pc-windows-msvc"
$base = "https://github.com/nagypalervin/qgate/releases/download/v$v"
Invoke-WebRequest "$base/qgate-$v-$t.zip" -OutFile "qgate-$v-$t.zip"
Invoke-WebRequest "$base/SHA256SUMS" -OutFile SHA256SUMS
$line = Select-String -Path SHA256SUMS -SimpleMatch " qgate-$v-$t.zip"
if ((Get-FileHash "qgate-$v-$t.zip" -Algorithm SHA256).Hash -ne $line.Line.Split(" ")[0]) { throw "checksum mismatch" } else { Expand-Archive "qgate-$v-$t.zip" -DestinationPath . }
```

**The binaries are not signed yet.**
- **macOS:** a file downloaded with a browser is quarantined, and
  Gatekeeper refuses to run it. Run `xattr -d com.apple.quarantine qgate`
  once. Files downloaded with `curl` are not quarantined.
- **Windows:** SmartScreen or Defender may warn about a `qgate.exe`
  downloaded with a browser. Run `Unblock-File .\qgate.exe` once.

Or build it from a clone of this repository, with Rust 1.85 or later:

```sh
cargo install --locked --path crates/qgate-cli
```

Then, in a git repository with TypeScript or C# code, check what your
branch changed compared with `main`:

```sh
qgate check --base main
```

qgate compares the merge base of `main` and `HEAD` with `HEAD` (what a
pull request shows) and analyzes every changed `.ts`, `.mts`, `.cts`,
`.tsx` and `.cs` file, except generated C# files (see [C#](#c)).
`--head <rev>` checks another revision instead of `HEAD`, and
`--worktree` checks your uncommitted work (see
[below](#checking-uncommitted-work)). Paths in the output are relative to
the repository root.

Or compare two files directly, here two fixtures from this repository
(without installing):

```sh
cargo run -q -p qgate-cli -- check \
  crates/qgate-cli/tests/fixtures/old/orders.ts \
  crates/qgate-cli/tests/fixtures/src/orders.ts
```

```
error   crates/qgate-cli/tests/fixtures/src/orders.ts:42  OrderService.process: cyclomatic complexity 3 -> 9 (+6)  [complexity/cyclomatic-increase]
```

qgate reports:

| Rule | Reported when | Warning | Error |
|---|---|---|---|
| `complexity/cyclomatic-increase` | an existing function got more complex | +1 to +4 | +5 or more |
| `complexity/cyclomatic-new` | a new function is already complex (new files count as new) | 11 to 20 | above 20 |
| `architecture/forbidden-import` | `--base` mode only: a change adds an import that a rule in `qgate.toml` forbids (see below) | - | always |
| `duplication/duplicate-block` | `--base` mode only: a change adds a copy of a block of at least 50 tokens that exists elsewhere in the repository (see below) | always | - |
| `input/unparsable` | `--base` mode only: a changed file has a syntax error, syntax the parser does not support yet, or is not UTF-8; it is skipped | always | - |

Exit codes:

- `0`: the analysis ran and no finding reached the `--fail-on` level;
- `1`: at least one finding did. The report is printed as usual, and one
  line on standard error says why the run failed, for example
  `qgate: failing: 1 finding at or above "error" (see --fail-on)`;
- `2`: the check could not run, for example: an invalid invocation, an
  unsupported or unparsable file in the two-file form, a C# file compared
  with a TypeScript file, an unknown revision,
  revisions without a merge base, a directory outside a git repository, or
  no `git` on the `PATH`. This wins over `1`.

`--fail-on` takes `error` (the default: only Errors fail the run),
`warning` (Warnings fail it too, including `input/unparsable`) or `never`
(findings never fail it; only a check that could not run does). The
example above has an Error finding, so it exits with `1`; add
`--fail-on never` to only see the report.

`--format json` writes the report as JSON for programs, such as coding
agents, hooks and CI scripts, in every form (see
[JSON output](#json-output)); `--format text`, the default, is the
output shown here. Three formats are for CI: `--format markdown` writes
the report as a pull request comment, `--format azure` writes each
Error and Warning as an Azure Pipelines build issue (see
[In Azure Pipelines](#in-azure-pipelines)), and `--format github` as a
GitHub Actions annotation (see [In GitHub Actions](#in-github-actions)).

### Checking uncommitted work

To check what you (or your coding agent) changed before committing it:

```sh
qgate check --base main --worktree
```

The working tree takes the place of `HEAD`, so the check sees everything
a commit of all your changes would contain:

- **Tracked files as they are on disk**, staged or not; a file deleted
  from disk counts as deleted, and a file moved with a plain `mv` is
  still a rename.
- **Untracked files**, unless git ignores them (`.gitignore`,
  `.git/info/exclude` or `core.excludesFile`).
- **`qgate.toml` as it is on disk.**
- Files marked `skip-worktree` or `assume-unchanged` count as unchanged,
  as git sees them.

The before side is still the merge base of `main` and `HEAD`, so this
reports your committed and uncommitted changes together. For the
uncommitted ones only, compare with `HEAD`:

```sh
qgate check --base HEAD --worktree
```

With Claude Code, let a hook run this check whenever the agent finishes
(see [With Claude Code](#with-claude-code)). Other tools get the same
check as JSON, which they can read without parsing text (see
[JSON output](#json-output)):

```sh
qgate check --base main --worktree --format json
```

**Nothing in the repository changes:** the index (what you staged),
`HEAD`, branches, the stash and your files stay as they are, and no git
hook runs (a configured file-system monitor, `core.fsmonitor`, may still
be asked what changed). qgate writes a snapshot of the working tree into
git's object store as objects that no branch refers to; `git gc` prunes
them once they are old enough (two weeks by default). Under a clean
filter such as Git LFS, the filter stores its copy as usual. That
snapshot includes every untracked file that is not ignored, so list large
generated files (build output, bundles) in `.gitignore`.

A check that is killed midway (by a timeout, say) can leave a copy of
the index named `qgate-snapshot-index-*` (`qgate-index-*` from earlier
versions) in the git directory; the next `--worktree` check removes
copies more than a day old.

### With Claude Code

Add this to `.claude/settings.json` in your repository and commit it, so
everyone on the team gets it:

```json
{
  "hooks": {
    "Stop": [
      { "hooks": [{ "type": "command", "command": "qgate hook claude-code" }] }
    ]
  }
}
```

Whenever Claude Code finishes a turn, qgate checks the work that is not
committed yet, as `qgate check --base HEAD --worktree` would:

- **A finding at or above `--fail-on`** (an Error, by default) sends the
  agent back once, with the findings and the request to fix them or to
  tell you why a change is intended. Claude Code may show this as a
  "Stop hook error"; it is the block, not a failure of qgate.
- **One round only.** Whatever remains after that round is shown to you,
  and the agent may stop: you decide.
- **Findings below the level** (Warnings, by default) are shown to you,
  not to the agent.
- **Errors** (an invalid `qgate.toml`, say) are shown to you and never
  sent to the agent.
- **Committing accepts a change:** committed work is not checked again in
  the next turns. The whole branch is checked in the pull request (see
  [In GitHub Actions](#in-github-actions) and
  [In Azure Pipelines](#in-azure-pipelines)).

Keep in mind:

- **If the agent commits in its turn,** the hook sees nothing of that
  commit. Use `--base main` if you let the agent commit.
- **Your own uncommitted work counts too.** An Error in it sends the
  agent back even after a turn that only answered a question. Commit or
  stash your work before you hand over to the agent.
- **Another Stop hook uses up the round too.** Once any Stop hook has
  sent the agent back in a turn, qgate shows its findings to you instead
  of sending the agent back again.

Options go into the command:

- `qgate hook claude-code --fail-on warning` also sends the agent back
  for Warnings, such as a duplicated block or a small complexity
  increase. Expect an extra round in many turns.
- `qgate hook claude-code --base main` checks everything the branch
  changed since `main`. Then findings you have already committed send
  the agent back at the end of every turn.
- `--fail-on never` never sends the agent back; you see every finding.

`qgate` must be on the `PATH` of the shell Claude Code runs hooks in. The
hook protocol is described in the
[Claude Code hooks documentation](https://code.claude.com/docs/en/hooks).

**A note on trust:** in an interactive session, Claude Code runs a
repository's hooks only after you trust the folder. `claude -p` and SDK
sessions run them without asking, so do not run those on repositories
you do not trust; a repository can configure any command as a hook.

### With an MCP client

`qgate mcp` is an [MCP](https://modelcontextprotocol.io) server with one
tool, `check`. Any coding agent with an MCP client can call it whenever
the agent decides to: in the middle of a task, before it commits, or for
the whole branch before it opens a pull request.

For **Claude Code**, add this to `.mcp.json` in your repository and
commit it. `claude mcp add --scope project qgate -- qgate mcp` writes
an equivalent entry.

```json
{
  "mcpServers": {
    "qgate": { "command": "qgate", "args": ["mcp"] }
  }
}
```

For **VS Code with GitHub Copilot**, add this to `.vscode/mcp.json`:

```json
{
  "servers": {
    "qgate": { "type": "stdio", "command": "qgate", "args": ["mcp"] }
  }
}
```

For **other clients**, start `qgate mcp` as a stdio server in the
repository.

The server checks the repository it was started in. Claude Code always
starts it in the project directory, and VS Code starts it in the
workspace folder unless the entry sets `"cwd"`. If the repository is
elsewhere, for example in a subfolder of the project, the agent can
pass `directory`.

`check` takes three optional arguments:

| Argument | Default | Meaning |
|---|---|---|
| `base` | `HEAD` | The revision to compare against. `HEAD` checks the uncommitted work. `main` checks the whole branch. |
| `head` | the working tree, uncommitted and untracked files included | A revision to check instead, such as `HEAD` for the last commit. |
| `directory` | where the server started | The repository to check, absolute or relative to that directory. |

It returns the [JSON report](#json-output), the same one
`qgate check --base <base> --worktree --format json` prints (or
`--head <head>` instead of `--worktree`). It always
holds every finding, at every severity. `summary.failing` counts the
findings at the server's `--fail-on` level, `error` by default. A team
sets the level once, in the entry's arguments, for example
`"args": ["mcp", "--fail-on", "warning"]`.

If the check cannot run, the tool returns an error that the agent
reads, and the server keeps running. This happens for a directory
outside a repository, an unknown revision or an invalid `qgate.toml`.
The server changes nothing in the repository.

**The hook or the server?** They work well together:
- **The [hook](#with-claude-code)** runs at the end of every Claude Code
  turn, whether the agent thinks of it or not.
- **The server** works with any MCP client, and lets the agent check
  when it wants and against what it wants.

The server speaks MCP revision 2026-07-28 and, for clients that start
with a handshake, 2025-11-25 and 2025-06-18.

**A note on trust:** a repository's `.mcp.json` or `.vscode/mcp.json`
names a command that runs on your machine. Claude Code asks before it
starts a project's MCP server. VS Code starts a workspace's servers only
once you trust the workspace. Check what an entry runs before you
approve it.

### JSON output

`--format json` writes one JSON object on a single line to standard
output. Pretty-printed with `jq .`, it looks like this:

```sh
qgate check --base main --format json | jq .
```

```json
{
  "schema_version": 1,
  "findings": [
    {
      "rule": "duplication/duplicate-block",
      "severity": "warning",
      "message": "duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)",
      "path": "src/orders/format.ts",
      "line": 4,
      "end_line": 9,
      "related": [
        { "path": "src/utils/date.ts", "line": 1, "end_line": 6 }
      ]
    },
    {
      "rule": "complexity/cyclomatic-increase",
      "severity": "warning",
      "message": "total: cyclomatic complexity 3 -> 6 (+3)",
      "path": "src/orders/total.ts",
      "line": 1,
      "metric": { "before": 3, "after": 6 }
    }
  ],
  "summary": { "count": 2, "failing": 0, "fail_on": "error" }
}
```

- **`schema_version`:** `1`, the version of this format.
- **`findings`:** in the order the text format prints them: by path, then
  line, then rule. Each finding has:
  - `rule`, `severity` (`error`, `warning` or `info`), `message` and
    `path` (relative to the repository root, or as given in the two-file
    form), as the text format shows them;
  - `line`, 1-based, unless the finding is about a whole file;
  - `end_line`, when the finding spans lines: the last line of the range
    that starts at `line` (it may equal `line`);
  - `metric`, for a measured value: `before` and `after`, for example a
    function's complexity. `before` is absent for a new function, and
    `after` would be absent for a removed item;
  - `related`, when the finding refers to other places: for a duplicate
    block, the existing copies, one per file by path, each with `path`,
    `line` and `end_line`. The first is the one the message names.
- **`summary`:** `count` (the number of findings), `failing` (how many
  are at or above the `--fail-on` level, so the run exits with `1` when
  it is not `0`) and `fail_on` (that level).

A key without a value is left out, never `null`. Line numbers and
whole metric values are written as integers.

**What stays stable:** within `schema_version` 1, qgate may add keys and
new rule ids, but never removes or renames a key, changes its type or
meaning, or adds a severity; that needs version 2. So ignore keys you do
not know, and check `schema_version`. `message` is for people and may be
reworded in any release; read the other fields instead of parsing it.
The JSON Schema for version 1 is
[`docs/schema/report-v1.json`](docs/schema/report-v1.json).

Exit codes and the summary line on standard error are the same as for
text. When the check cannot run, standard output stays empty and the
error is text on standard error, with exit code `2`.

With `jq`, for example, list where the findings are:

```sh
qgate check --base main --format json | jq -r '.findings[] | "\(.path):\(.line // "-") \(.rule)"'
```

```
src/orders/format.ts:4 duplication/duplicate-block
src/orders/total.ts:1 complexity/cyclomatic-increase
```

### Duplicated code

In `--base` mode qgate compares the code a change adds with every
TypeScript file of the checked revision, or for C# with every C# file,
and reports a new copy:

```
warning src/orders/format.ts:4  duplicates src/utils/date.ts:1-6 (lines 4-9, 68 tokens)  [duplication/duplicate-block]
```

- **Renamed copies count:**
  - variable names and literal values are ignored, so a helper copied
    under other names is still a copy;
  - member names (`.getFullYear`), keywords and operators are compared;
  - comments and semicolons are ignored, and so are imports and type
    annotations in TypeScript, and `using` directives, namespace
    declarations and preprocessor directives in C#. C# types are
    compared like other names.
- **Copies across files only.** A block counts when it occurs in two or
  more files; repetition within one file (a data table, similar test
  cases one after another) does not.
- **Only new copies are reported.** Duplication the code base already has
  is not, nor code written next to it; nor is code that only moved, also
  out of a deleted file; nor a file renamed or moved from `.js` to `.ts`.
  When a change adds the same block to two new files, the first by path
  counts as the original.
- **A block must span at least 50 tokens**, about 5 to 8 lines.
- It is a Warning, so it fails a run only with `--fail-on warning`. The
  two-file form does not check duplication.

### Architecture rules (`qgate.toml`)

Put a `qgate.toml` at the repository root to forbid imports between parts
of your TypeScript code (C# `using` directives are not checked yet):

```toml
[[architecture.forbid]]
name = "domain-is-pure"
from = ["src/domain/**"]
to   = ["src/ui/**", "src/infra/**", "react"]
```

A change that adds `import { Button } from "../ui/Button";` to
`src/domain/order.ts` then fails with:

```
error   src/domain/order.ts:3  imports src/ui/Button ("../ui/Button"), forbidden by "domain-is-pure"  [architecture/forbidden-import]
```

- **Only new imports are reported.** An import the file already had in the
  base revision does not fail the check, so you can add a rule to a
  codebase that still breaks it. A move that git detects as a rename is
  checked against the file's old path: moving a file within `from`
  reports nothing, moving it into `from` reports its forbidden imports. A
  file renamed from `.js` to `.ts` keeps the imports it already had (if the
  old JavaScript cannot be read, none of its imports counts as new,
  wherever the file moved).
- **Patterns** are globs relative to the repository root: `*` and `?` stay
  within one directory, `**` spans directories. A pattern without glob
  characters also matches everything below it (`react` matches
  `react/jsx-runtime`, not `react-dom`), and `dir/**` also matches `dir`.
  A leading `./`, `//` inside and a trailing `/` are ignored
  (`./src//ui/` is `src/ui`); a pattern that starts with `/` (after any
  `./`) or has a `..` segment is an error. A trailing `/` does not make a
  glob match below it: write `**/ui/**`, not `**/ui/`. Braces (`{a,b}`)
  and `\` escapes are glob syntax; a pattern using them is not rewritten
  beyond a leading `./`.
- **`from`** matches the importing file's path. **`to`** matches the import
  target: a relative import (`./`, `../`) resolved to a path from the root,
  without extension (`src/ui/Button`); anything else as written
  (`react`, `@/ui/Button`). So prefer directory patterns, and list path
  aliases too: `to = ["src/ui/**", "@/ui/**"]`. Suffixes and prefixes
  stay part of the target: `"../ui/B?raw"` is `src/ui/B?raw` (matched by
  `src/ui/**`, not by `src/ui/B`), and `node:fs` is not `fs`.
- **Every import counts**, `import type` and `import("...")` in a type
  included; dynamic imports and `require` count with a string literal.
- **The file is read from the revision being checked** (`--head`, by default
  `HEAD`), not from the working tree, unless you check the working tree
  itself with `--worktree`. A missing `qgate.toml` means no
  rules and no excluded files; an invalid one (unknown keys included)
  fails with exit code `2`.
  The two-file form ignores it.
- **Not yet supported:** tsconfig `paths`, root-absolute imports
  (`import "/src/ui/Button"`, which no pattern can name), and files the
  TypeScript grammar
  cannot parse yet (`export type * from`, `import("./x").Foo<T>`), which
  are reported as `input/unparsable` instead.

### Leaving files out (`[files] exclude`)

To keep files out of every check, list path patterns under `[files]` in
`qgate.toml`:

```toml
[files]
exclude = ["**/Migrations/**", "vendor"]
```

- **An excluded file is left out everywhere** in `--base` and
  `--worktree` mode, as if qgate did not read its language: it is not
  analyzed, a copy of it is not reported, and its old code does not
  count as code that existed before. A file moved out of an excluded
  directory counts as new.
- **Patterns** are those of [architecture rules](#architecture-rules-qgatetoml):
  `vendor` also covers everything below it, `**/Migrations/**` any
  `Migrations` directory.
- **The list is read from the revision being checked**, like the rules,
  so a pull request that excludes its own files shows it in the diff of
  its `qgate.toml`.
- **An invalid pattern** fails with exit code `2`, naming `files.exclude`
  and the pattern. The two-file form ignores the setting.
- **An excluded file is still an import target:** a rule's `to` matches
  it as before.

### C#

`.cs` files get the complexity check in every form and the duplication
check in `--base` and `--worktree` mode, and so in the Claude Code hook
and the MCP server, which run the same check. Architecture rules do not
apply to C# yet: a `using` names a namespace, not a file, so it needs
rules of its own.

- **Units and names.** Methods, constructors, finalizers, operators,
  local functions and property, indexer and event accessors with a body
  are measured; lambdas count towards the unit they are in. A unit is
  named by its types and its signature, without the namespace:
  `Cart.Total(int)`, `Cart.Owner.get`, `Result<T>.Ok()`,
  `Cart.Total(int).Twice(int)` for a local function. So an overload
  added above another keeps both names, and moving a file to another
  namespace changes nothing. Changing a parameter's type makes the
  method new, like a rename.
- **Decision points:** `if`, `for`, `foreach`, `while`, `do`, each `case`
  label, each switch expression arm except `_`, `when`, `catch`, `?:`,
  `&&`, `||`, `??`, `??=`, `?.`, `?[]`, and the patterns `and` and `or`.
  Unlike TypeScript, optional parameter values do not count: the caller
  passes them.
- **Generated files are skipped** in `--base` and `--worktree` mode, by
  Roslyn's convention: names ending in `.Designer.cs`, `.generated.cs`,
  `.g.cs` or `.g.i.cs`, names starting with `TemporaryGeneratedFile_`,
  and files with an `<auto-generated` or `<autogenerated` comment
  before their first code (such as EF Core migration designers and
  model snapshots). They are neither analyzed nor compared, and a
  generated base version counts as no base: a file that stops being
  generated is new. The two-file form analyzes the files it is given.
- **EF Core migrations themselves are not generated** by this
  convention: they have no header, so they are analyzed, and a new
  migration that repeats an older one's table definitions is reported
  as a copy (a Warning). To leave them out, add
  `exclude = ["**/Migrations/**"]` under `[files]` (see
  [Leaving files out](#leaving-files-out-files-exclude)).
- **Preprocessor directives are not evaluated.** The code of every `#if`
  branch counts, and a directive's condition does not. A file is
  reported as `input/unparsable` where `#if` splits code the parser
  cannot read: branches that each open or close a block, or an `#if`
  inside an expression, an argument list, an initializer or a base
  list, or around a modifier or an `else if`. In the last 30 C# commits
  of `dotnet/eShop` and of `jasontaylordev/CleanArchitecture`, which
  changed 314 C# files, this hit 2 files.
- **Not read:** C# script (`.csx`), Razor (`.cshtml`, `.razor`), VB.NET
  and F#, and files that are not UTF-8 (`input/unparsable`).

### In GitHub Actions

Add the qgate action to a workflow for pull requests, for example
`.github/workflows/qgate.yml`:

```yaml
name: qgate
on: pull_request

permissions:
  contents: read
  pull-requests: write

jobs:
  qgate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: nagypalervin/qgate@v0.4.0
```

The action installs qgate 0.4.0 for the runner from the release and
checks it against `SHA256SUMS`. It compares the pull request with its
base branch (`origin/<base branch>`), so the checkout needs the full
history (`fetch-depth: 0`). It runs on GitHub-hosted Linux, macOS and
Windows runners, x64 and arm64.

#### What the pull request shows

- **Annotations** on the changed files, one per Error and Warning
  finding. GitHub shows at most 10 errors and 10 warnings of a step;
  the step's log lists every finding.
- **The job summary** of the run: the Markdown report, a table of up to
  50 findings.
- **One qgate comment** with the same report, posted when a run first
  finds something. Later runs update it, and once the findings are
  gone it says "No regressions found." A pull request that never had a
  finding gets no comment.
- **The check fails** when the pull request adds an Error finding. To
  block the merge, make the `qgate` check required in the branch's
  protection rules or ruleset.

#### Inputs

| Input | Default | Meaning |
|---|---|---|
| `fail-on` | `error` | `warning` for a stricter gate, `never` to report without failing |
| `comment` | `true` | `false` writes no comment; the workflow then needs only `contents: read` |
| `base` | the pull request's base branch | the revision to compare with, for example `origin/main`; needed outside `pull_request` events |

```yaml
      - uses: nagypalervin/qgate@v0.4.0
        with:
          fail-on: warning
```

#### Keep in mind

- **The comment needs `pull-requests: write`.** `issues: write` is not
  enough for a pull request. Without the permission the action fails
  with a message that names it, before qgate's gate runs, so there are
  no annotations.
- **Pull requests from forks** usually get a token that cannot write, so
  the action skips the comment for every fork with a notice. The
  annotations, the job summary and the result work as usual.
- **Pinning.** qgate never moves a release tag. To pin the action's
  code itself, use the commit instead:
  `nagypalervin/qgate@cb474ba36d9e6c0b877cdf87183de8de6409a54f # v0.4.0`.
  The binary still comes from the release, checked against its
  `SHA256SUMS`.
- **Self-hosted runners** need PowerShell 7.4 or later (`pwsh`), `git`,
  `tar` on Linux and macOS, and access to `github.com` and
  `api.github.com`.

#### Without the action

To run qgate in your own steps, install it from a release, outside the
checkout, so `actions/checkout` does not remove it and qgate does not
analyze it. On `ubuntu-latest`:

```yaml
- name: Install qgate
  working-directory: ${{ runner.temp }}
  run: |
    v=0.4.0 t=x86_64-unknown-linux-musl
    base=https://github.com/nagypalervin/qgate/releases/download/v$v
    curl -fsSL --fail-early -O "$base/qgate-$v-$t.tar.gz" -O "$base/SHA256SUMS"
    grep " qgate-$v-$t.tar.gz\$" SHA256SUMS | sha256sum -c
    tar xzf "qgate-$v-$t.tar.gz"
    echo "$PWD/qgate-$v-$t" >> "$GITHUB_PATH"
```

A `run` step stops at its first failing command, so a bad download
installs nothing. Then, in a workflow triggered `on: pull_request`
(`github.base_ref` is empty for other events), with the full history
checked out:

```yaml
- run: qgate check --base "origin/$BASE_REF" --format github
  env:
    BASE_REF: ${{ github.base_ref }}
```

For a later step or another tool, write the report as JSON to a file
instead. The exit code is the same, so the step still fails the job, and
GitHub Actions then skips the later steps: give the ones that read the
report `if: ${{ !cancelled() }}`, or they run only when there is nothing
to report.

```yaml
- run: qgate check --base "origin/$BASE_REF" --format json > qgate-report.json
  env:
    BASE_REF: ${{ github.base_ref }}
- uses: actions/upload-artifact@v7
  if: ${{ !cancelled() }}
  with:
    name: qgate-report
    path: qgate-report.json
```

### In Azure Pipelines

For a repository in Azure Repos, add a `qgate` job to the `jobs:` of the
pipeline that builds your pull requests, and a branch policy that runs
the pipeline for them ([below](#azure-branch-policy)). The job runs only
in pull request builds, next to your other jobs. It keeps one qgate
comment on the pull request up to date, lists the findings in the build
summary, and fails when the pull request adds an Error. It needs no
service connection: the comment is written with the pipeline's own
token, which needs one permission ([below](#azure-comment-permission)).
On a Microsoft-hosted Linux agent:

```yaml
- job: qgate
  condition: eq(variables['Build.Reason'], 'PullRequest')
  pool:
    vmImage: ubuntu-latest
  steps:
    - checkout: self
      fetchDepth: 0
    - bash: |
        set -euo pipefail
        v=0.4.0 t=x86_64-unknown-linux-musl
        base=https://github.com/nagypalervin/qgate/releases/download/v$v
        curl -fsSL --fail-early -O "$base/qgate-$v-$t.tar.gz" -O "$base/SHA256SUMS"
        grep " qgate-$v-$t.tar.gz\$" SHA256SUMS | sha256sum -c
        tar xzf "qgate-$v-$t.tar.gz"
        echo "##vso[task.prependpath]$PWD/qgate-$v-$t"
        echo "##vso[task.setvariable variable=QGATE_DIR]$PWD/qgate-$v-$t"
      displayName: Install qgate
      workingDirectory: $(Agent.TempDirectory)
    - bash: |
        qgate check --base "origin/${TARGET_BRANCH#refs/heads/}" --format markdown > "$AGENT_TEMPDIRECTORY/qgate.md" || [ $? -eq 1 ]
      displayName: qgate report
      env:
        TARGET_BRANCH: $(System.PullRequest.TargetBranch)
    - pwsh: |
        & "$env:QGATE_DIR/qgate-pr-comment.ps1" -Report "$env:AGENT_TEMPDIRECTORY/qgate.md"
      displayName: qgate comment
      env:
        SYSTEM_ACCESSTOKEN: $(System.AccessToken)
    - bash: |
        qgate check --base "origin/${TARGET_BRANCH#refs/heads/}" --format azure
      displayName: qgate check
      env:
        TARGET_BRANCH: $(System.PullRequest.TargetBranch)
```

On a Windows agent, use this job instead. qgate reads the sources from
git, so the job's agent need not be the one your build runs on.

```yaml
- job: qgate
  condition: eq(variables['Build.Reason'], 'PullRequest')
  pool:
    vmImage: windows-latest
  steps:
    - checkout: self
      fetchDepth: 0
    - pwsh: |
        $ErrorActionPreference = "Stop"
        $PSNativeCommandUseErrorActionPreference = $true
        $v = "0.4.0"; $t = "x86_64-pc-windows-msvc"
        $base = "https://github.com/nagypalervin/qgate/releases/download/v$v"
        Invoke-WebRequest "$base/qgate-$v-$t.zip" -OutFile "qgate-$v-$t.zip"
        Invoke-WebRequest "$base/SHA256SUMS" -OutFile SHA256SUMS
        $line = Select-String -Path SHA256SUMS -SimpleMatch " qgate-$v-$t.zip"
        if ((Get-FileHash "qgate-$v-$t.zip" -Algorithm SHA256).Hash -ne $line.Line.Split(" ")[0]) { throw "checksum mismatch" } else { Expand-Archive "qgate-$v-$t.zip" -DestinationPath . }
        Write-Host "##vso[task.prependpath]$PWD\qgate-$v-$t"
        Write-Host "##vso[task.setvariable variable=QGATE_DIR]$PWD\qgate-$v-$t"
      displayName: Install qgate
      workingDirectory: $(Agent.TempDirectory)
    - pwsh: |
        qgate check --base "origin/$($env:TARGET_BRANCH -replace '^refs/heads/', '')" --format markdown > "$env:AGENT_TEMPDIRECTORY\qgate.md"
        if ($LASTEXITCODE -eq 1) { exit 0 }
      displayName: qgate report
      env:
        TARGET_BRANCH: $(System.PullRequest.TargetBranch)
    - pwsh: |
        & "$env:QGATE_DIR\qgate-pr-comment.ps1" -Report "$env:AGENT_TEMPDIRECTORY\qgate.md"
      displayName: qgate comment
      env:
        SYSTEM_ACCESSTOKEN: $(System.AccessToken)
    - pwsh: |
        qgate check --base "origin/$($env:TARGET_BRANCH -replace '^refs/heads/', '')" --format azure
      displayName: qgate check
      env:
        TARGET_BRANCH: $(System.PullRequest.TargetBranch)
```

If your pipeline has `steps:` at the top level and no `jobs:`, move your
`pool` and `steps` into a job of their own first (`jobs:` with
`- job: build`), then add the `qgate` job beside it.

<a id="azure-branch-policy"></a>**The branch policy is required.** In Azure
Repos, the YAML `pr:` trigger does not apply: only a build validation
policy makes a pull request build. Without one, the `qgate` job is
skipped in every build and the pipeline stays green, so it looks as if
qgate found nothing. To add the policy:

1. In *Repos → Branches*, open *Branch policies* from the menu of `main`.
2. Under *Build validation*, select *+*.
3. Choose the pipeline and keep the trigger *Automatic*.
4. Set *Policy requirement* to *Required* and *Build expiration* to
   *Immediately when main is updated*.
5. Save.

*Required* is what makes it a merge gate: a pull request whose `qgate`
job fails can then not be completed.

<a id="azure-comment-permission"></a>**The comment needs one
permission.** The `qgate comment` step writes with the pipeline's token,
`System.AccessToken`, which belongs to the project's build service. By
default it may read the repository but not comment on pull requests. To
allow it:

1. In *Project settings → Repositories*, select the repository.
2. Open the *Security* tab.
3. Under *Users*, select *&lt;project&gt; Build Service (&lt;organization&gt;)*.
4. Set *Contribute to pull requests* to *Allow*.

If your organization or project does not limit the job authorization
scope to the current project, the token belongs to *Project Collection
Build Service (&lt;organization&gt;)* instead; give that identity the
permission. Without it, the `qgate comment` step fails the job for every
pull request with findings, Warnings included; only pull requests
without findings pass. The error names the permission and the build
identity by its id, such as `Build\<id>`, and tells which of the two
Build Service identities that is; when it cannot tell, it names both.

**What the pull request shows:**

- **One qgate comment.** The first build with findings creates it as an
  active thread with a table of the findings. Later builds update the
  same comment: with findings, it is set active again; once they are
  gone, it says "No regressions found." and is closed. A pull request
  that never had a finding gets no comment.
- **The build summary.** Each Error and Warning is listed as an issue
  of the build, with its file, line and rule.
- **The build result.** The `qgate check` step fails the job, and with
  the policy the pull request, when the pull request adds an Error.

Keep in mind:

- **`fetchDepth: 0` is needed.** By default Azure Pipelines fetches only
  the commit it builds, without the target branch. The pull request
  build checks out the merge of the pull request into its target, so
  `--base origin/<target>` checks exactly what the merge would change.
- **The first lines of each install step are needed.** A bash step in
  Azure Pipelines goes on after a failing command: without
  `set -euo pipefail`, a failed download or checksum check would not stop
  the install. A PowerShell step goes on after a failing native command,
  such as `git` or `curl.exe`; the second line stops it. The Windows
  install step calls none, but a command you add later would slip
  through without it. Start any script step you add the same way. Do
  not add the second line to the Windows `qgate report` step: it would
  stop the step at exit code `1`, before the step accepts it.
- **qgate runs twice.** The `qgate report` step writes the comment's
  Markdown and accepts exit code `1`, which only means findings; it
  still fails on `2`, when the check cannot run. The `qgate check` step
  is the gate. It comes last, so that the comment is written even when
  the gate fails.
- **`--fail-on`** works as in GitHub Actions. Add the same option, for
  example `--fail-on warning`, to both `qgate check` lines: the comment
  then names the level that gates the pull request.
- **Self-hosted agents** need access to github.com, PowerShell 7.4 or
  later as `pwsh`, and on Linux `curl`, `sha256sum` and `tar`. On
  arm64, use the target `aarch64-unknown-linux-musl` or
  `aarch64-pc-windows-msvc`.

## Why

AI coding assistants make it cheap to write a lot of code quickly. Speed
moves the bottleneck: architecture boundaries erode, functions quietly grow
more complex, near-duplicate blocks multiply, and tests fall behind the code
they are supposed to cover. Each pull request looks fine on its own; the
damage shows up months later.

qgate makes that drift visible **per change**, while it is still cheap
to fix, and is meant to record it over time (M6 on the
[roadmap](#roadmap)) so you can answer a simple question with data
instead of gut feeling:

*When we develop faster, where does the system break first?*

## How it works

qgate compares two revisions (`base..head`) and reports what got
**worse**. It measures deltas, not absolute values, so you can adopt it on
an existing codebase without first fixing every legacy problem.

```
git diff base..head ─► parser (tree-sitter) ─► analyzers ─► findings ─► reporters
                                              ├ architecture    ├ terminal
                                              ├ complexity      ├ JSON / SARIF
                                              ├ duplication     ├ PR comment
                                              └ tests           └ MCP server
```

The `tests` analyzer (M5) and SARIF output (Later) are planned; the rest
works today.

Design principles:

1. **Deterministic core.** The same input always gives the same result. An
   LLM may summarise findings for a PR comment, but it never decides what a
   finding is, and it is always optional.
2. **Deltas, not scores.** "This function went from 8 to 23" is actionable;
   "your codebase scores 61/100" is not.
3. **Close the loop with agents.** An MCP server lets AI coding agents ask
   qgate what they just broke *before* they open a pull request.

## Roadmap

| Milestone | Scope | Status |
|---|---|---|
| **M1** | `qgate check`, TypeScript parsing, complexity delta, git revisions, `--fail-on` | done |
| **M2** | Architecture rules (`qgate.toml`), duplication detection | done |
| **M3** | Checking while coding: the working tree as head, JSON output, a Claude Code hook, an MCP server, so agents check their own changes before opening a pull request | done |
| **M4** | CI integration: release binaries, GitHub Action, Azure DevOps Pipelines, pull request comments | done |
| **Languages** | C# next to TypeScript: complexity and duplication (0.4.0) | done |
| **M5** | Change risk from git history: hotspots (code that is both complex and changed often), code changed without its tests | planned |
| **M6** | Baseline and trends over time, optional AI summary | planned |
| **Later** | A git pre-commit hook, which needs a check of the staged changes (`--staged`); SARIF output, once a consumer fits a tool that reports only what got worse; architecture rules for C# `using` directives, which name namespaces rather than files | not planned yet |

The hotspot analysis in M5 builds on published work on behavioral code
analysis, notably Adam Tornhill's *Your Code as a Crime Scene*.

## Repository layout

```
crates/
  qgate-core/       shared types: Finding, Metric, Severity
  qgate-lang/       language adapters (tree-sitter)
  qgate-analyzers/  architecture, complexity, duplication, tests
  qgate-report/     text, JSON, Markdown, Azure Pipelines and GitHub Actions annotations (SARIF later)
  qgate-git/        changed files and blobs, via the `git` command
  qgate-config/     reads qgate.toml
  qgate-cli/        the `qgate` binary
ci/azure/               the Azure Pipelines comment script, shipped in every release
ci/github/              the GitHub Action's script
action.yml              the GitHub Action
docs/adr/               architecture decision records
docs/journal/           notes on how AI-assisted development went
openspec/               specs and changes (OpenSpec workflow, see CLAUDE.md)
.claude/agents/         independent reviewer agent
```

## Building

Requires Rust 1.85 or newer and a C compiler (tree-sitter compiles its
grammars from C at build time). `qgate check --base` needs `git` on the
`PATH`; the tests need git 2.32 or newer, because they create temporary
repositories isolated from your git configuration.

```sh
cargo build
cargo test --workspace
cargo run -p qgate-cli -- --version
```

To run the CI checks (format, clippy, tests) before every commit, enable
the repository's hook once per clone:

```sh
git config core.hooksPath .githooks
```

`git commit --no-verify` skips it for a work-in-progress commit.

## About

A personal open source project by [Ervin Nagypál](https://nagypalervin.hu/),
built in the open as an experiment in AI-assisted development. Lessons
learned along the way are written up on the blog.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
