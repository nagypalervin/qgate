//! The `qgate` command.
//!
//! Thin by design: it parses arguments, reads files or asks `qgate-git`
//! for changed files, and picks exit codes; parsing, analysis and rendering
//! live in the library crates.
//!
//! Exit codes: `0` when the analysis ran (with or without findings), `2`
//! for an invalid invocation or input. `1` is reserved for a future "check
//! failed" mode.

use std::fs;
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::error::ErrorKind as ClapErrorKind;
use clap::{Arg, ArgMatches, Command, value_parser};
use qgate_analyzers::complexity::ComplexityAnalyzer;
use qgate_analyzers::{Analyzer, FileChange};
use qgate_core::{Finding, Location, Severity};
use qgate_git::{ChangedFile, GitError, Repository};
use qgate_lang::{FileModel, Language, ParseError};
use qgate_report::render_text;

/// Exit code for invalid invocations and inputs.
const INVALID: u8 = 2;

/// Rule id for a file that revision mode could not analyze. A public API
/// like every rule id; only the CLI creates it (design D4).
const UNPARSABLE_RULE: &str = "input/unparsable";

/// Added to an unknown-revision error, because a shallow CI clone is by far
/// the most common cause.
const SHALLOW_CLONE_HINT: &str = "if this is a shallow clone, fetch the base first \
     (in GitHub Actions: actions/checkout with fetch-depth: 0)";

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
            "qgate check <before> <after>\n       qgate check --base <rev> [--head <rev>]",
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
        return match result {
            Ok(report) => write_stdout(&report),
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
fn write_stdout(report: &str) -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    match stdout
        .write_all(report.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: cannot write the report: {e}");
            ExitCode::from(INVALID)
        }
    }
}

/// Run `qgate check` and return the rendered report, or an error message
/// for standard error. Nothing is printed here, so a failure leaves standard
/// output empty.
fn check(args: &ArgMatches) -> Result<String, String> {
    let path = |name: &str| {
        args.get_one::<PathBuf>(name)
            .ok_or_else(|| format!("missing <{name}>"))
    };
    let (before_path, after_path) = (path("before")?, path("after")?);
    let change = FileChange {
        path: after_path.clone(),
        before: load(before_path)?,
        after: load(after_path)?,
    };

    let mut findings = ComplexityAnalyzer.analyze(&change);
    sort(&mut findings);
    Ok(render_text(&findings))
}

/// Sort across files and analyzers so the same input always gives the same
/// bytes: by path, then line (no line first), then rule id.
fn sort(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        (&a.location.path, a.location.line, &a.rule).cmp(&(
            &b.location.path,
            b.location.line,
            &b.rule,
        ))
    });
}

/// Run `qgate check --base <base> --head <head>` in the repository that
/// contains the current directory.
fn check_revisions(base: &str, head: &str) -> Result<String, String> {
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    let repo = Repository::discover(&cwd).map_err(|e| git_message(&e))?;
    let mut findings = Vec::new();
    for file in repo
        .changed_files(base, head)
        .map_err(|e| git_message(&e))?
    {
        let Some(language) = language_of(&file.path) else {
            continue;
        };
        match file_change(&repo, &file, language)? {
            Ok(change) => findings.extend(ComplexityAnalyzer.analyze(&change)),
            Err(not_analyzed) => findings.push(not_analyzed),
        }
    }
    sort(&mut findings);
    Ok(render_text(&findings))
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

    let after_bytes = repo.read_blob(&file.after).map_err(|e| git_message(&e))?;
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
            let bytes = repo.read_blob(id).map_err(|e| git_message(&e))?;
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

/// The message for a git error, with a hint where one helps.
fn git_message(error: &GitError) -> String {
    match error {
        GitError::UnknownRevision { .. } => format!("{error}; {SHALLOW_CLONE_HINT}"),
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
