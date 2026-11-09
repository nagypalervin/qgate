//! The `qgate` command.
//!
//! Thin by design: it parses arguments, reads files and picks exit codes;
//! parsing, analysis and rendering live in the library crates.
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
use qgate_lang::{FileModel, Language, ParseError};
use qgate_report::render_text;

/// Exit code for invalid invocations and inputs.
const INVALID: u8 = 2;

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
        if args.contains_id("base") {
            eprintln!("error: revision mode is not implemented yet");
            return ExitCode::from(INVALID);
        }
        return match check(args) {
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
    // Sorted across analyzers so the same input always gives the same bytes.
    findings.sort_by(|a, b| (a.location.line, &a.rule).cmp(&(b.location.line, &b.rule)));
    Ok(render_text(&findings))
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
