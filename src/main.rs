use clap::Parser;
use machete::cli::{Cli, run};
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

/// Runs the command selected by the process arguments.
fn main() -> ExitCode {
    init_tracing();

    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Installs the global subscriber that collects what the program reports.
///
/// The subscriber is set once, before anything runs, so that every event is
/// collected for the rest of the process. A library should never install one:
/// whoever uses it decides.
///
/// Two choices are already settled. Events go to standard error, because
/// standard output carries the data a command was asked to produce, as
/// `machete completions bash` does; anything written there would land in the
/// middle of a completion script. And the level comes from `RUST_LOG`, falling
/// back to `info` when it is unset or cannot be read.
///
/// The JSON formatter and the `#[instrument]` attribute are available but not
/// used yet: there are no events to emit and no command takes a verbosity flag.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
