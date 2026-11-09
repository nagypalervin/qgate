//! The `qgate` command.
//!
//! Thin by design: it parses arguments, reads files and picks exit codes;
//! parsing, analysis and rendering live in the library crates.
//!
//! Exit codes: `0` when the analysis ran (with or without findings), `2`
//! for an invalid invocation or input. `1` is reserved for a future "check
//! failed" mode.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Arg, Command, value_parser};

/// Exit code for invalid invocations and inputs.
const INVALID: u8 = 2;

fn command() -> Command {
    Command::new("qgate")
        .about("Detects where your codebase starts to break under fast, AI-assisted development")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::new("check")
                .about("Compare two versions of a TypeScript file and report what got worse")
                .arg(
                    Arg::new("before")
                        .help("The file before the change (.ts, .mts, .cts or .tsx)")
                        .required(true)
                        .value_parser(value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new("after")
                        .help("The file after the change; findings report this path")
                        .required(true)
                        .value_parser(value_parser!(PathBuf)),
                ),
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

    if let Some(("check", _)) = matches.subcommand() {
        eprintln!("error: `qgate check` is not implemented yet");
        return ExitCode::from(INVALID);
    }
    let _ = command.print_help();
    ExitCode::SUCCESS
}
