# Proposal: add-fail-on

## Why

qgate is meant to be a quality *gate*, but today `qgate check` exits with 0
whenever the analysis ran, so a CI job only shows findings and never stops a
pull request. The `check-command` spec already marks this as temporary ("In
this version ..."), and design D7 of `add-cyclomatic-complexity-check`
reserved exit code 1 for "check failed". With no released version and no
users yet, now is the cheapest moment to change the default.

## What Changes

- **BREAKING (default exit code):** `qgate check` exits with **1** when the
  analysis ran and at least one finding has severity Error. Before, it
  exited with 0.
- **New option `--fail-on <level>`** with the values:
  - `error` (default): fail on Error findings;
  - `warning`: fail on Warning or Error;
  - `never`: always exit with 0 when the analysis ran.

  An unknown value is a usage error (exit 2).
- **`input/unparsable` is a Warning.** It fails the run only with
  `--fail-on warning`.
- **Exit code precedence:** 2 when the check could not run (as before),
  else 1 when a finding meets the level, else 0.
- **Output:** standard output is unchanged. When the run fails because of
  findings, one summary line goes to standard error, so a CI log shows why
  the job failed.
- **Both modes:** the two-file form and `--base`.
- **Repository updates:**
  - the README's exit codes and CI section;
  - the CI smoke step, which runs the README example (an Error finding) and
    now passes `--fail-on never`;
  - the end-to-end test that pinned exit code 0 for an Error finding.

### Decisions taken during exploration

- **Fail by default on Error, opt out with `never`.** This is the convention
  of ESLint, `tsc` and `clippy -D warnings`. qgate reports only deltas, so a
  legacy codebase does not fail on what it already contains, only on what a
  change made worse. Core already documents Error as "got worse beyond the
  threshold".
- **One option with a level**, not ESLint's `--max-warnings <n>`: a count of
  new warnings in a pull request has no stable meaning, and a count cannot
  express `never`.
- **No `info` level** for now: no rule emits Info. Adding the value later
  breaks nothing.
- **Summary on standard error, not standard output**, so scripts and the
  later JSON output (M3) keep a clean stdout. A passing run prints no extra
  line.

### Out of scope

- Configuring the level in `qgate.toml` (M2).
- Per-rule levels, a baseline file, or counting thresholds.
- A summary line for passing runs or other output formats (M3).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `check-command`:
  - "Findings do not change the exit code" is renamed to "Failing on
    findings", and its content is replaced by the `--fail-on` behaviour;
  - "Invalid invocation" adds an unknown `--fail-on` value.

## Impact

- **`qgate-cli`:** the new option, the exit-code decision, and the stderr
  summary.
- **Libraries:** none. `Severity` is already ordered
  (`Info < Warning < Error`), which is what the decision needs.
- **Tests:** `error_finding_exits_zero` in `crates/qgate-cli/tests/check.rs`
  changes. All other end-to-end tests that expect 0 produce only Warnings,
  so they keep passing under the new default.
- **CI:** `.github/workflows/ci.yml`, step "Smoke test qgate check".
- **Docs:** README ("Try it" exit codes, "In CI").
- **Users:** none yet, so the breaking default needs no migration. It is
  stated in the README.

## Implementation Notes

- **Singular in the summary:** one failing finding reads `1 finding at or
  above "error"`, more read `N findings`. The spec scenario shows the plural
  form.
- **Rendering moved into `main`:** `check` and `check_revisions` return the
  sorted findings instead of the rendered text, so the exit code is decided
  from the same findings that were printed.
- **Order of checks:** the report is written first; a write error other
  than a closed pipe is still exit 2 and wins over 1. A closed pipe keeps
  the exit code findings give (1 with failing findings), tested with
  thousands of findings and `--fail-on warning`.
- **The summary is written with `writeln!` and its error ignored**, not
  `eprintln!`, so a closed standard error cannot turn exit 1 into a panic.
- **`FailOn` implements clap's `ValueEnum` by hand** (the `derive` feature
  is off). clap then rejects unknown values with exit 2 and lists the
  allowed ones, and `--help` shows the default and the possible values.
- **Fixture `fail/`:** two functions grow by +6 (Error) and one by +1
  (Warning), for the summary scenario. Revision-mode tests build their
  inputs in temporary repositories as before.
