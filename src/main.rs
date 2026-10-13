use clap::Parser;
use machete::cli::{Cli, run};
use machete::error::BuildError;
use machete::report::{self, Format};
use std::process::ExitCode;

/// Runs the command selected by the process arguments.
fn main() -> ExitCode {
    let cli = Cli::parse();

    // The clock is read here, once, and everything else reads it from there.
    report::start(cli.format.unwrap_or(Format::Plain), cli.quiet);
    init_tracing();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // An output that would hold the site is not a crash: the build
            // simply did not run. It is said as a warning, and the exit code is
            // still a failure, because nothing was built and a script must be
            // able to tell.
            match &error {
                BuildError::OutputInsideSite { .. } | BuildError::InputMissing { .. } => {
                    report::warning(&error.to_string())
                }
                _ => report::failure(&error.to_string()),
            }

            ExitCode::FAILURE
        }
    }
}

/// Installs the global subscriber that collects what the program reports.
///
/// Which formatter is installed follows `--format`. Plain is written by hand so
/// that a person sees one line and nothing else: the timestamp, the level and
/// the target are all turned off, and a plain event carries no fields, so only
/// the message is left. JSON keeps the fields, flattened to the top level so a
/// reader finds `status` where it expects it rather than nested under
/// `fields`.
///
/// Everything goes to standard error. Standard output carries the data a
/// command was asked to produce, as `machete completions` does, and a log line
/// in the middle of a shell script would break it.
///
/// The level filter from `RUST_LOG` is not wired up yet.
fn init_tracing() {
    match report::format() {
        Format::Plain => {
            tracing_subscriber::fmt()
                .without_time()
                .with_level(false)
                .with_target(false)
                .with_file(false)
                .with_line_number(false)
                .with_writer(std::io::stderr)
                .init();
        }

        Format::Json | Format::Jsonl => {
            tracing_subscriber::fmt()
                .json()
                .flatten_event(true)
                .with_current_span(false)
                .with_span_list(false)
                .without_time()
                .with_level(false)
                .with_target(false)
                .with_writer(std::io::stderr)
                .init();
        }
    }
}
