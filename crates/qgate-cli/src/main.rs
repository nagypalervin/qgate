//! The `qgate` command.
//!
//! Argument parsing is hand-rolled for now to keep the skeleton free of
//! dependencies; it moves to `clap` once the first real subcommand
//! (`qgate check`) arrives in M1.

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
qgate - detects where your codebase starts to break under fast, AI-assisted development

USAGE:
    qgate [OPTIONS]

OPTIONS:
    -h, --help       Print this help
    -V, --version    Print version

Nothing to check yet: `qgate check` arrives in milestone M1.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("qgate {VERSION}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown argument '{other}'\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}
