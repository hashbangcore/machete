use clap::Parser;
use machete::cli::{Cli, run};
use std::process::ExitCode;

/// Runs the command selected by the process arguments.
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
