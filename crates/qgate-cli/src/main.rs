//! The `qgate` command.
//!
//! Thin by design: it parses arguments, reads files or asks `qgate-git`
//! for changed files and `qgate.toml` (of a revision, or of a snapshot of
//! the working tree), wires the configuration into the analyzers, and
//! picks exit codes; parsing, analysis and rendering live in the library
//! crates.
//!
//! Exit codes of `qgate check`: `2` when the check could not run (an
//! invalid invocation or input), else `1` when a finding is at or above the
//! `--fail-on` level, else `0`. `qgate hook` speaks an agent's hook
//! protocol instead (see the `hook` module): `0`, or `1` for an invalid
//! invocation, never `2`. `qgate mcp` serves MCP until its input ends (see
//! the `mcp` module): `0`, `1` when its input or output fails, `2` for an
//! invalid invocation.

mod hook;
mod mcp;

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead as _, ErrorKind, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::builder::PossibleValue;
use clap::error::ErrorKind as ClapErrorKind;
use clap::{Arg, ArgAction, ArgMatches, Command, ValueEnum, value_parser};
use qgate_analyzers::architecture::{ArchitectureAnalyzer, ForbidRule};
use qgate_analyzers::complexity::ComplexityAnalyzer;
use qgate_analyzers::duplication::{ChangedSource, DuplicationIndex};
use qgate_analyzers::{Analyzer, FileChange};
use qgate_config::Config;
use qgate_core::{Finding, Location, Severity};
use qgate_git::{BlobId, BlobReader, ChangedFile, Changes, GitError, Repository};
use qgate_lang::{FileModel, Language, ParseError, Token};
use qgate_report::{ReportSummary, render_json, render_text};

/// Exit code for a check that ran and found findings at or above the
/// `--fail-on` level.
const FAILED: u8 = 1;

/// Exit code for invalid invocations and inputs.
const INVALID: u8 = 2;

/// The configuration file, at the repository root.
const CONFIG_FILE: &str = "qgate.toml";

/// Rule id for a file that revision mode could not analyze. A public API
/// like every rule id; only the CLI creates it (design D4).
const UNPARSABLE_RULE: &str = "input/unparsable";

/// Added to an unknown-revision error, because a shallow CI clone is by far
/// the most common cause.
const SHALLOW_CLONE_HINT: &str = "if this is a shallow clone, fetch the base first \
     (in GitHub Actions: actions/checkout with fetch-depth: 0)";

/// The `--fail-on` level when the option is not given.
const DEFAULT_FAIL_ON: FailOn = FailOn::Error;

/// Which findings fail the run: the value of `--fail-on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailOn {
    /// Findings with severity Error.
    Error,
    /// Findings with severity Warning or Error.
    Warning,
    /// No finding; only a check that could not run fails.
    Never,
}

impl FailOn {
    /// The lowest severity that fails the run, if any does.
    fn threshold(self) -> Option<Severity> {
        match self {
            Self::Error => Some(Severity::Error),
            Self::Warning => Some(Severity::Warning),
            Self::Never => None,
        }
    }

    /// The value as the user writes it.
    fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Never => "never",
        }
    }

    /// How many of `findings` fail the run.
    fn failing(self, findings: &[Finding]) -> usize {
        self.threshold().map_or(0, |threshold| {
            findings.iter().filter(|f| f.severity >= threshold).count()
        })
    }
}

/// How the report is written: the value of `--format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// Lines for people (the `check-command` spec).
    Text,
    /// The versioned JSON report (the `report-formats` spec).
    Json,
}

impl Format {
    /// The value as the user writes it.
    fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }

    /// The report for `findings`, which fail the run at the `fail_on` level.
    fn render(self, findings: &[Finding], fail_on: FailOn) -> String {
        match self {
            Self::Text => render_text(findings),
            Self::Json => render_json(
                findings,
                ReportSummary {
                    failing: fail_on.failing(findings),
                    fail_on: fail_on.name(),
                },
            ),
        }
    }
}

impl ValueEnum for Format {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Text, Self::Json]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.name()))
    }
}

/// The `--format` value when the option is not given.
const DEFAULT_FORMAT: Format = Format::Text;

impl ValueEnum for FailOn {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Error, Self::Warning, Self::Never]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.name()))
    }
}

fn command() -> Command {
    Command::new("qgate")
        .about("Detects where your codebase starts to break under fast, AI-assisted development")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(check_command())
        .subcommand(hook_command())
        .subcommand(mcp_command())
}

/// `qgate mcp`: an MCP server on standard input and output (the
/// `mcp-server` spec).
fn mcp_command() -> Command {
    Command::new("mcp")
        .about(
            "Serve the Model Context Protocol on standard input and output, with a check \
             tool that AI coding agents call",
        )
        .arg(
            Arg::new("fail-on")
                .long("fail-on")
                .value_name("level")
                .help("The level the report's summary.failing counts")
                .value_parser(value_parser!(FailOn))
                .default_value(DEFAULT_FAIL_ON.name()),
        )
}

/// `qgate hook <agent>`: run from a coding agent's hooks (the
/// `agent-hooks` spec).
fn hook_command() -> Command {
    Command::new("hook")
        .about("Run from a coding agent's hooks: check its uncommitted work")
        .subcommand_required(true)
        .subcommand(
            Command::new("claude-code")
                .about(
                    "Answer a Claude Code Stop hook: send the agent back once with \
                     what its uncommitted work made worse",
                )
                .arg(
                    Arg::new("base")
                        .long("base")
                        .value_name("rev")
                        .help("Check the working tree against this revision")
                        .default_value("HEAD"),
                )
                .arg(
                    Arg::new("fail-on")
                        .long("fail-on")
                        .value_name("level")
                        .help(
                            "Send the agent back on a finding at or above this level; \
                             the user sees the others",
                        )
                        .value_parser(value_parser!(FailOn))
                        .default_value(DEFAULT_FAIL_ON.name()),
                ),
        )
}

fn check_command() -> Command {
    Command::new("check")
        .about("Report what got worse between two versions of TypeScript code")
        .override_usage(
            "qgate check <before> <after> [--format <format>] [--fail-on <level>]\n       \
             qgate check --base <rev> [--head <rev>] [--format <format>] [--fail-on <level>]\n       \
             qgate check --base <rev> --worktree [--format <format>] [--fail-on <level>]",
        )
        .arg(
            Arg::new("before")
                .help("The file before the change (.ts, .mts, .cts or .tsx)")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("after")
                .help("The file after the change; findings report this path")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("base")
                .long("base")
                .value_name("rev")
                .help(
                    "Compare git revisions instead of files: the merge base of \
                             <rev> and --head against --head",
                )
                .conflicts_with_all(["before", "after"]),
        )
        .arg(
            Arg::new("head")
                .long("head")
                .value_name("rev")
                .help("The revision to check (default: HEAD); needs --base")
                .requires("base")
                // clap drops `requires` when the required argument conflicts
                // with one given, as --base does with two files.
                .conflicts_with_all(["before", "after"]),
        )
        .arg(
            Arg::new("worktree")
                .long("worktree")
                .action(ArgAction::SetTrue)
                .help("Check the working tree (uncommitted and untracked files) instead of --head")
                .requires("base")
                .conflicts_with_all(["head", "before", "after"]),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .value_name("format")
                .help("How to write the report: text for people, json for programs")
                .value_parser(value_parser!(Format))
                .default_value(DEFAULT_FORMAT.name()),
        )
        .arg(
            Arg::new("fail-on")
                .long("fail-on")
                .value_name("level")
                .help(
                    "Exit with 1 on a finding at or above this level \
                     (never: only a check that cannot run fails)",
                )
                .value_parser(value_parser!(FailOn))
                .default_value(DEFAULT_FAIL_ON.name()),
        )
}

fn main() -> ExitCode {
    let mut command = command();
    let matches = match command.try_get_matches_from_mut(std::env::args_os()) {
        Ok(matches) => matches,
        // Help and version also arrive here, with exit code 0.
        Err(err) => {
            let _ = err.print();
            let code = u8::try_from(err.exit_code()).unwrap_or(INVALID);
            // Claude Code sends a hook's standard error to the model on exit
            // code 2; a broken settings entry is for the user (design D4 of
            // `add-agent-hooks`). The top-level command has no options other
            // than `--help` and `--version`, so a hook command line always
            // starts with `hook`; a global option would have to change this.
            let in_hook = std::env::args_os().nth(1).is_some_and(|arg| arg == "hook");
            return ExitCode::from(if in_hook && code == INVALID {
                FAILED
            } else {
                code
            });
        }
    };

    if let Some(("hook", hook_args)) = matches.subcommand() {
        if let Some(("claude-code", args)) = hook_args.subcommand() {
            return claude_code_hook(args);
        }
    }

    if let Some(("mcp", args)) = matches.subcommand() {
        return mcp_server(args);
    }

    if let Some(("check", args)) = matches.subcommand() {
        let two_files = args.contains_id("before") && args.contains_id("after");
        if !two_files && !args.contains_id("base") {
            let err = check_command().error(
                ClapErrorKind::MissingRequiredArgument,
                "give two files, or --base <rev> to compare git revisions",
            );
            let _ = err.print();
            return ExitCode::from(INVALID);
        }
        let result = match args.get_one::<String>("base") {
            Some(base) => {
                let head = if args.get_flag("worktree") {
                    HeadArg::WorkTree
                } else {
                    HeadArg::Revision(
                        args.get_one::<String>("head")
                            .map_or("HEAD", String::as_str),
                    )
                };
                std::env::current_dir()
                    .map_err(|e| format!("cannot read the current directory: {e}"))
                    .and_then(|cwd| check_revisions(&cwd, base, head))
            }
            None => check(args),
        };
        let fail_on = args
            .get_one::<FailOn>("fail-on")
            .copied()
            .unwrap_or(DEFAULT_FAIL_ON);
        let format = args
            .get_one::<Format>("format")
            .copied()
            .unwrap_or(DEFAULT_FORMAT);
        return match result.and_then(|findings| {
            write_stdout(&format.render(&findings, fail_on))?;
            Ok(findings)
        }) {
            Ok(findings) => exit_code(fail_on, &findings),
            Err(message) => {
                eprintln!("error: {message}");
                ExitCode::from(INVALID)
            }
        };
    }
    let _ = command.print_help();
    ExitCode::SUCCESS
}

/// `qgate hook claude-code`: read the hook input, check the working tree
/// of the project, and answer in Claude Code's hook protocol. Every
/// outcome exits with 0; errors go to the user in the response.
fn claude_code_hook(args: &ArgMatches) -> ExitCode {
    let base = args
        .get_one::<String>("base")
        .map_or("HEAD", String::as_str);
    let fail_on = args
        .get_one::<FailOn>("fail-on")
        .copied()
        .unwrap_or(DEFAULT_FAIL_ON);
    let mut text = String::new();
    let response = match std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("cannot read the hook input: {e}"))
        .and_then(|_| hook::parse_input(&text))
    {
        Ok(input) => {
            let result =
                project_dir(&input).and_then(|dir| check_revisions(&dir, base, HeadArg::WorkTree));
            hook::respond(result, input.stop_hook_active(), base, fail_on)
        }
        Err(message) => hook::Response::error(&message),
    };
    match write_stdout(&response.output()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // Not 2: Claude Code would send it to the model.
            let _ = writeln!(std::io::stderr(), "error: {message}");
            ExitCode::from(FAILED)
        }
    }
}

/// `qgate mcp`: answer each line of standard input until it ends (design
/// D1, D6 and D8 of `add-mcp-server`).
fn mcp_server(args: &ArgMatches) -> ExitCode {
    let fail_on = args
        .get_one::<FailOn>("fail-on")
        .copied()
        .unwrap_or(DEFAULT_FAIL_ON);
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"));
    let mut server = mcp::Server::new(fail_on, cwd, check_revisions);
    let mut input = std::io::stdin().lock();
    let mut line = Vec::new();
    loop {
        line.clear();
        match input.read_until(b'\n', &mut line) {
            Ok(0) => return ExitCode::SUCCESS,
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = writeln!(std::io::stderr(), "error: cannot read the input: {e}");
                return ExitCode::from(FAILED);
            }
        }
        let Some(mut response) = server.handle_bytes(&line) else {
            continue;
        };
        response.push('\n');
        let mut stdout = std::io::stdout().lock();
        match stdout
            .write_all(response.as_bytes())
            .and_then(|()| stdout.flush())
        {
            Ok(()) => {}
            // The client is gone.
            Err(e) if e.kind() == ErrorKind::BrokenPipe => return ExitCode::SUCCESS,
            Err(e) => {
                let _ = writeln!(std::io::stderr(), "error: cannot write a response: {e}");
                return ExitCode::from(FAILED);
            }
        }
    }
}

/// The directory the hook checks (design D2 of `add-agent-hooks`):
/// `CLAUDE_PROJECT_DIR`, else the input's `cwd`, else the current
/// directory.
fn project_dir(input: &hook::Input) -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("CLAUDE_PROJECT_DIR").filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    if let Some(cwd) = input.cwd() {
        return Ok(cwd.clone());
    }
    std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))
}

/// Write the report to standard output. A reader that stops early (as in
/// `qgate check ... | head`) is not an error: `print!` would panic there.
fn write_stdout(report: &str) -> Result<(), String> {
    let mut stdout = std::io::stdout().lock();
    match stdout
        .write_all(report.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Err(e) if e.kind() != ErrorKind::BrokenPipe => Err(format!("cannot write the report: {e}")),
        _ => Ok(()),
    }
}

/// The exit code of a check that ran: `1` with a summary line on standard
/// error when findings fail it at the `fail_on` level, else `0`.
fn exit_code(fail_on: FailOn, findings: &[Finding]) -> ExitCode {
    let failing = fail_on.failing(findings);
    if failing == 0 {
        return ExitCode::SUCCESS;
    }
    let noun = if failing == 1 { "finding" } else { "findings" };
    // Not `eprintln!`: a closed standard error must not turn 1 into a panic.
    let _ = writeln!(
        std::io::stderr(),
        "qgate: failing: {failing} {noun} at or above \"{}\" (see --fail-on)",
        fail_on.name()
    );
    ExitCode::from(FAILED)
}

/// Run `qgate check` and return the sorted findings, or an error message
/// for standard error. Nothing is printed here, so a failure leaves standard
/// output empty.
fn check(args: &ArgMatches) -> Result<Vec<Finding>, String> {
    let path = |name: &str| {
        args.get_one::<PathBuf>(name)
            .ok_or_else(|| format!("missing <{name}>"))
    };
    let (before_path, after_path) = (path("before")?, path("after")?);
    let change = FileChange {
        path: after_path.clone(),
        // Two files are compared as one file; the architecture rules,
        // which would care about a rename, do not run in this form.
        old_path: None,
        before: load(before_path)?,
        after: load(after_path)?,
    };

    let mut findings = ComplexityAnalyzer.analyze(&change);
    sort(&mut findings);
    Ok(findings)
}

/// Sort across files and analyzers so the same input always gives the same
/// bytes: by path as a byte string (the order git uses), then line (no line
/// first), then rule id.
fn sort(findings: &mut [Finding]) {
    fn key(f: &Finding) -> (&[u8], Option<u32>, &str) {
        (
            f.location.path.as_os_str().as_encoded_bytes(),
            f.location.line,
            &f.rule,
        )
    }
    findings.sort_by(|a, b| key(a).cmp(&key(b)));
}

/// The head of a revision check as the user gave it.
#[derive(Debug, Clone, Copy)]
enum HeadArg<'a> {
    /// `--head <rev>`, or `HEAD` by default.
    Revision(&'a str),
    /// `--worktree`.
    WorkTree,
}

/// The head a revision check reads, and how its messages name it (design
/// D3 of `add-worktree-input`).
struct Head {
    /// The revision git reads: the user's, or the id of the working
    /// tree's snapshot.
    revision: String,
    /// Whether `revision` is a snapshot of the working tree, which
    /// messages call `HEAD` or "the working tree", never by its id.
    worktree: bool,
}

/// Run `qgate check --base <base>` with `--head <rev>` or `--worktree` in
/// the repository that contains `dir`, and return the sorted findings.
fn check_revisions(dir: &Path, base: &str, head: HeadArg<'_>) -> Result<Vec<Finding>, String> {
    let repo = Repository::discover(dir).map_err(|e| e.to_string())?;
    let head = match head {
        HeadArg::Revision(revision) => Head {
            revision: revision.to_owned(),
            worktree: false,
        },
        // Before the diff, which then reads the snapshot like any commit.
        // Its errors name `HEAD` or the working tree, and an unknown
        // `HEAD` here is no shallow clone, even with `--base HEAD`.
        HeadArg::WorkTree => Head {
            revision: repo.worktree_snapshot().map_err(|e| e.to_string())?,
            worktree: true,
        },
    };
    // The diff first: it checks both revisions and their merge base, so an
    // unknown base (a shallow clone) is reported as such, never hidden
    // behind a broken qgate.toml.
    let Changes {
        changed: files,
        deleted,
    } = repo
        .changes(base, &head.revision)
        .map_err(|e| git_message(&e, base, &head))?;
    // Checked before any file is analyzed.
    let architecture = architecture_analyzer(&load_config(&repo, base, &head)?)?;
    let mut analyzers: Vec<&dyn Analyzer> = vec![&ComplexityAnalyzer];
    if !architecture.is_empty() {
        analyzers.push(&architecture);
    }
    let mut findings = Vec::new();
    let mut changed_paths = HashSet::new();
    let mut analyzed = Vec::new();
    // Code that left the supported files: still a base version for
    // duplication, so moving it is not a new copy.
    let mut left = Vec::new();
    for file in files {
        let Some(language) = language_of(&file.path) else {
            // Renamed from TypeScript to a language qgate does not read:
            // its old code left. (JavaScript renamed to JavaScript is
            // neither compared nor gone.)
            if let (Some(old), Some(id)) = (&file.old_path, &file.before) {
                if language_of(old).is_some() {
                    left.push((old.clone(), id.clone()));
                }
            }
            continue;
        };
        changed_paths.insert(file.path.clone());
        match file_change(&repo, &file, language)? {
            Ok(change) => {
                for analyzer in &analyzers {
                    findings.extend(analyzer.analyze(&change));
                }
                analyzed.push(change);
            }
            Err(not_analyzed) => findings.push(not_analyzed),
        }
    }
    if !analyzed.is_empty() {
        left.extend(deleted.into_iter().map(|file| (file.path, file.id)));
        findings.extend(duplication(
            &repo,
            base,
            &head,
            &analyzed,
            &left,
            &changed_paths,
        )?);
    }
    sort(&mut findings);
    Ok(findings)
}

/// The `duplication/duplicate-block` findings: the analyzed changed files
/// against every other supported file of the head revision, read through
/// one git process and parsed one at a time (design D5 of
/// `add-duplicate-detection`). A changed file that could not be analyzed
/// takes no part; an unchanged one that cannot be read or parsed is
/// skipped silently.
fn duplication(
    repo: &Repository,
    base: &str,
    head: &Head,
    changes: &[FileChange],
    left: &[(String, BlobId)],
    changed_paths: &HashSet<String>,
) -> Result<Vec<Finding>, String> {
    let mut reader = repo
        .blob_reader()
        .map_err(|e| git_message(&e, base, head))?;
    // The base tokens of supported files that are gone (deleted, or
    // renamed to another language); one that cannot be read or parsed
    // counts as empty.
    let mut gone: Vec<(String, Vec<Token>)> = Vec::new();
    for (path, id) in left {
        // A deleted JavaScript file's code counts too, like a renamed one's.
        let language = match language_of(path) {
            Some(language) => language,
            None if is_javascript(path) => Language::Tsx,
            None => continue,
        };
        let tokens = match read_source(&mut reader, id)? {
            Some(bytes) => match parse_blob(language, bytes)? {
                Ok(model) => model.tokens,
                Err(_) => Vec::new(),
            },
            None => Vec::new(),
        };
        gone.push((path.clone(), tokens));
    }
    let paths: Vec<String> = changes
        .iter()
        .map(|change| change.path.to_string_lossy().into_owned())
        .collect();
    let mut sources: Vec<ChangedSource<'_>> = changes
        .iter()
        .zip(&paths)
        .map(|(change, path)| ChangedSource {
            path,
            before: &change.before.tokens,
            after: &change.after.tokens,
        })
        .collect();
    sources.extend(gone.iter().map(|(path, tokens)| ChangedSource {
        path,
        before: tokens,
        after: &[],
    }));
    let mut index = DuplicationIndex::new(&sources);
    let tree = repo
        .files_at(&head.revision)
        .map_err(|e| git_message(&e, base, head))?;
    for file in tree {
        if changed_paths.contains(&file.path) {
            continue;
        }
        let Some(language) = language_of(&file.path) else {
            continue;
        };
        let Some(bytes) = read_source(&mut reader, &file.id)? else {
            continue;
        };
        if let Ok(model) = parse_blob(language, bytes)? {
            index.add(&file.path, &model.tokens);
        }
    }
    Ok(index.findings())
}

/// One blob through the batch reader: `None` if git cannot give it as a
/// source file (too large, say), an error if the reader can read no more
/// (git ended, or its answers went out of step).
fn read_source(reader: &mut BlobReader, id: &BlobId) -> Result<Option<Vec<u8>>, String> {
    match reader.read(id) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if reader.is_closed() || matches!(e, GitError::Command { .. }) => Err(e.to_string()),
        Err(_) => Ok(None),
    }
}

/// The configuration as the head revision has it (ADR 0003): uncommitted
/// changes count only when the head is the working tree, and a missing
/// file is an empty configuration.
fn load_config(repo: &Repository, base: &str, head: &Head) -> Result<Config, String> {
    match repo
        .file_at(&head.revision, CONFIG_FILE)
        .map_err(|e| git_message(&e, base, head))?
    {
        Some(bytes) => qgate_config::parse(&bytes).map_err(|e| format!("{CONFIG_FILE}: {e}")),
        None => Ok(Config::default()),
    }
}

/// The architecture analyzer for the configuration's forbid rules. Copies
/// the plain values into the analyzer's own rule type (ADR 0003); an
/// invalid pattern is a configuration error.
fn architecture_analyzer(config: &Config) -> Result<ArchitectureAnalyzer, String> {
    let rules: Vec<ForbidRule> = config
        .architecture
        .forbid
        .iter()
        .map(|rule| ForbidRule {
            name: rule.name.clone(),
            from: rule.from.clone(),
            to: rule.to.clone(),
        })
        .collect();
    ArchitectureAnalyzer::new(&rules).map_err(|e| format!("{CONFIG_FILE}: {e}"))
}

/// Both versions of one changed file as models, or an `input/unparsable`
/// finding if either cannot be parsed. Git failures are errors.
fn file_change(
    repo: &Repository,
    file: &ChangedFile,
    language: Language,
) -> Result<Result<FileChange, Finding>, String> {
    let not_analyzed = |reason: String, line: Option<u32>| Finding {
        rule: UNPARSABLE_RULE.to_owned(),
        severity: Severity::Warning,
        message: format!("file not analyzed: {reason}"),
        location: Location {
            path: PathBuf::from(&file.path),
            line,
            end_line: None,
        },
        metric: None,
        related: Vec::new(),
    };

    let after_bytes = repo.read_blob(&file.after).map_err(|e| e.to_string())?;
    let after = match parse_blob(language, after_bytes)? {
        Ok(model) => model,
        Err(Unparsable::NotUtf8) => {
            return Ok(Err(not_analyzed("not valid UTF-8".to_owned(), None)));
        }
        Err(Unparsable::Syntax(line)) => {
            let reason =
                format!("syntax error on line {line} (or syntax the parser does not support yet)");
            return Ok(Err(not_analyzed(reason, Some(line))));
        }
    };

    // An added file, or one renamed from a language qgate does not read,
    // has no earlier version of its code: every function in it is new.
    let mut old_path = file.old_path.as_ref().map(PathBuf::from);
    let old_language = match &file.old_path {
        Some(old) => language_of(old),
        None => Some(language),
    };
    let before = match (&file.before, old_language) {
        // Renamed from JavaScript (a JS-to-TS migration): its functions are
        // new, but the imports and the code it already had are not. If the
        // JavaScript cannot be read, the head's own imports and tokens
        // stand in, at the head's path, so neither an import nor a copy of
        // the file counts as new wherever it moved; complexity is as for
        // any added file.
        (Some(id), None) if file.old_path.as_deref().is_some_and(is_javascript) => {
            let bytes = repo.read_blob(id).map_err(|e| e.to_string())?;
            // The code it already had is not new either: its tokens count
            // as the base version for duplication.
            let (imports, tokens) = if let Ok(model) = parse_blob(Language::Tsx, bytes)? {
                (model.imports, model.tokens)
            } else {
                old_path = None;
                (after.imports.clone(), after.tokens.clone())
            };
            FileModel {
                functions: Vec::new(),
                imports,
                tokens,
            }
        }
        (Some(id), Some(old_language)) => {
            let bytes = repo.read_blob(id).map_err(|e| e.to_string())?;
            match parse_blob(old_language, bytes)? {
                Ok(model) => model,
                Err(Unparsable::NotUtf8) => {
                    let reason = "the base version is not valid UTF-8".to_owned();
                    return Ok(Err(not_analyzed(reason, None)));
                }
                Err(Unparsable::Syntax(line)) => {
                    let reason = format!(
                        "syntax error on line {line} of the base version \
                         (or syntax the parser does not support yet)"
                    );
                    return Ok(Err(not_analyzed(reason, None)));
                }
            }
        }
        _ => FileModel::default(),
    };

    Ok(Ok(FileChange {
        path: PathBuf::from(&file.path),
        old_path,
        before,
        after,
    }))
}

/// Why one version of a file could not be analyzed.
enum Unparsable {
    NotUtf8,
    Syntax(u32),
}

/// Decode and parse one blob. A parser failure that is not caused by the
/// input is an error for the whole run.
fn parse_blob(language: Language, bytes: Vec<u8>) -> Result<Result<FileModel, Unparsable>, String> {
    let Ok(source) = String::from_utf8(bytes) else {
        return Ok(Err(Unparsable::NotUtf8));
    };
    match qgate_lang::parse(language, &source) {
        Ok(model) => Ok(Ok(model)),
        Err(ParseError::Syntax { line }) => Ok(Err(Unparsable::Syntax(line))),
        Err(ParseError::Parser { message }) => Err(format!("parser failure: {message}")),
    }
}

/// Whether a path is a JavaScript file, whose imports qgate reads when it
/// is renamed to TypeScript (`add-forbidden-imports`, PR 3 review).
fn is_javascript(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext, "js" | "jsx" | "mjs" | "cjs"))
}

/// The language of a path, from its extension.
fn language_of(path: &str) -> Option<Language> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(Language::from_extension)
}

/// The message for a git error. An unknown `base` gets the shallow-clone
/// hint; an unknown head or an option-like value would be misled by it.
/// A snapshot of the working tree is named `HEAD` or "the working tree",
/// never by its id.
fn git_message(error: &GitError, base: &str, head: &Head) -> String {
    match error {
        GitError::UnknownRevision { revision }
            if revision == base && !revision.starts_with('-') =>
        {
            format!("{error}; {SHALLOW_CLONE_HINT}")
        }
        GitError::NoMergeBase {
            base,
            head: revision,
        } if head.worktree && *revision == head.revision => GitError::NoMergeBase {
            base: base.clone(),
            head: "HEAD".to_owned(),
        }
        .to_string(),
        GitError::NotARegularFile { path, revision }
            if head.worktree && *revision == head.revision =>
        {
            format!("`{path}` is not a regular file in the working tree")
        }
        _ => error.to_string(),
    }
}

/// Pick the language from the extension, read and parse one input file.
fn load(path: &Path) -> Result<FileModel, String> {
    let shown = path.display();
    let language = path
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(Language::from_extension)
        .ok_or_else(|| {
            format!("{shown}: unsupported file type (expected .ts, .mts, .cts or .tsx)")
        })?;
    let source = fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))?;
    qgate_lang::parse(language, &source).map_err(|e| match e {
        ParseError::Syntax { line } => {
            format!("{shown}:{line}: syntax error (or syntax the parser does not support yet)")
        }
        ParseError::Parser { message } => format!("{shown}: parser failure: {message}"),
    })
}
