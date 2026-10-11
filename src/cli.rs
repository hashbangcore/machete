//! Command line interface.
//!
//! [`Cli`] describes the accepted commands and [`run`] dispatches one to its
//! handler. Both live here rather than in `main.rs` so the argument parsing can
//! be exercised from tests without spawning a process.

use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Static site generator that composes HTML from components, layouts and
/// variables.
#[derive(Debug, Parser)]
#[command(name = "machete", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// The commands `machete` accepts.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create the directory layout and config file for a new site.
    Init(InitArgs),

    /// Print a shell completion script to stdout.
    Completions(CompletionsArgs),
}

/// Arguments for `machete init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Directory to create the site in. Defaults to the current one.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Overwrite files that already exist instead of leaving them untouched.
    #[arg(long, short)]
    pub force: bool,
}

/// Arguments for `machete completions`.
#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// The shell to generate completions for.
    #[arg(value_name = "SHELL")]
    pub shell: Shell,
}

/// A failure reported by a command handler.
#[derive(Debug)]
pub enum CliError {
    /// The handler ran into a problem writing to a file or directory.
    Io {
        action: String,
        path: PathBuf,
        source: io::Error,
    },
    /// The handler is not written yet.
    NotImplemented { command: &'static str },
    /// A completion script could not be written.
    Completions(io::Error),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                source,
            } => {
                write!(f, "failed to {action} {}: {source}", path.display())
            }
            Self::Completions(source) => write!(f, "failed to write completions: {source}"),
            Self::NotImplemented { command } => {
                write!(f, "'{command}' is not implemented yet")
            }
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Completions(source) => Some(source),
            Self::NotImplemented { .. } => None,
        }
    }
}

impl CliError {
    /// Wraps an I/O failure together with what was being attempted and where.
    pub fn io(action: impl Into<String>, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action: action.into(),
            path: path.into(),
            source,
        }
    }
}

/// Parses the arguments and runs the command they select.
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Init(args) => init(args),
        Command::Completions(args) => completions(args),
    }
}

/// Writes the default project layout and config file for a new site.
fn init(_args: InitArgs) -> Result<(), CliError> {
    Err(CliError::NotImplemented { command: "init" })
}

/// Writes a completion script for the requested shell to stdout.
fn completions(args: CompletionsArgs) -> Result<(), CliError> {
    let mut command = Cli::command();
    clap_complete::generate(args.shell, &mut command, "machete", &mut io::stdout());
    Ok(())
}
