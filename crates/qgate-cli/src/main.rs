//! The `qgate` command.
//!
//! Thin by design: it parses arguments, reads files or asks `qgate-git`
//! for changed files and `qgate.toml`, wires the configuration into the
//! analyzers, and picks exit codes; parsing, analysis and rendering live in
//! the library crates.
//!
//! Exit codes: `2` when the check could not run (an invalid invocation or
//! input), else `1` when a finding is at or above the `--fail-on` level,
//! else `0`.

use std::fs;
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::builder::PossibleValue;
use clap::error::ErrorKind as ClapErrorKind;
use clap::{Arg, ArgMatches, Command, ValueEnum, value_parser};
use qgate_analyzers::architecture::{ArchitectureAnalyzer, ForbidRule};
use qgate_analyzers::complexity::ComplexityAnalyzer;
use qgate_analyzers::{Analyzer, FileChange};
use qgate_config::Config;
use qgate_core::{Finding, Location, Severity};
use qgate_git::{ChangedFile, GitError, Repository};
use qgate_lang::{FileModel, Language, ParseError};
use qgate_report::render_text;

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
}

fn check_command() -> Command {
    Command::new("check")
        .about("Report what got worse between two versions of TypeScript code")
        .override_usage(
            "qgate check <before> <after> [--fail-on <level>]\n       \
             qgate check --base <rev> [--head <rev>] [--fail-on <level>]",
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
                .requires("base"),
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
            return ExitCode::from(u8::try_from(err.exit_code()).unwrap_or(INVALID));
        }
    };

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
                let head = args
                    .get_one::<String>("head")
                    .map_or("HEAD", String::as_str);
                check_revisions(base, head)
            }
            None => check(args),
        };
        let fail_on = args
            .get_one::<FailOn>("fail-on")
            .copied()
            .unwrap_or(DEFAULT_FAIL_ON);
        return match result.and_then(|findings| {
            write_stdout(&render_text(&findings))?;
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

/// Run `qgate check --base <base> --head <head>` in the repository that
/// contains the current directory, and return the sorted findings.
fn check_revisions(base: &str, head: &str) -> Result<Vec<Finding>, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    let repo = Repository::discover(&cwd).map_err(|e| git_message(&e, base))?;
    // The diff first: it checks both revisions and their merge base, so an
    // unknown base (a shallow clone) is reported as such, never hidden
    // behind a broken qgate.toml.
    let files = repo
        .changed_files(base, head)
        .map_err(|e| git_message(&e, base))?;
    // Checked before any file is analyzed.
    let architecture = architecture_analyzer(&load_config(&repo, base, head)?)?;
    let mut analyzers: Vec<&dyn Analyzer> = vec![&ComplexityAnalyzer];
    if !architecture.is_empty() {
        analyzers.push(&architecture);
    }
    let mut findings = Vec::new();
    for file in files {
        let Some(language) = language_of(&file.path) else {
            continue;
        };
        match file_change(&repo, &file, language)? {
            Ok(change) => {
                for analyzer in &analyzers {
                    findings.extend(analyzer.analyze(&change));
                }
            }
            Err(not_analyzed) => findings.push(not_analyzed),
        }
    }
    sort(&mut findings);
    Ok(findings)
}

/// The configuration as the head revision has it (ADR 0003): uncommitted
/// changes do not count, and a missing file is an empty configuration.
fn load_config(repo: &Repository, base: &str, head: &str) -> Result<Config, String> {
    match repo
        .file_at(head, CONFIG_FILE)
        .map_err(|e| git_message(&e, base))?
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
        },
        metric: None,
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
    let old_language = match &file.old_path {
        Some(old) => language_of(old),
        None => Some(language),
    };
    let before = match (&file.before, old_language) {
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
        old_path: file.old_path.as_ref().map(PathBuf::from),
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

/// The language of a path, from its extension.
fn language_of(path: &str) -> Option<Language> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(Language::from_extension)
}

/// The message for a git error. An unknown `base` gets the shallow-clone
/// hint; an unknown head or an option-like value would be misled by it.
fn git_message(error: &GitError, base: &str) -> String {
    match error {
        GitError::UnknownRevision { revision }
            if revision == base && !revision.starts_with('-') =>
        {
            format!("{error}; {SHALLOW_CLONE_HINT}")
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
