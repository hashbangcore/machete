//! Command line interface.
//!
//! [`Cli`] describes the accepted commands and [`run`] dispatches one to its
//! handler. Both live here rather than in `main.rs` so the argument parsing can
//! be exercised from tests without spawning a process.

use crate::build::build;
use crate::config::Config;
use crate::error::BuildError;
use crate::scaffold::write;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
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
    /// Expand a site's templates into finished HTML pages.
    Build(BuildArgs),

    /// Create the directory layout and config file for a new site.
    Init(InitArgs),

    /// Print a shell completion script to stdout.
    Completions(CompletionsArgs),
}

/// Arguments for `machete build`.
#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Site directory holding `config.toml` and `templates/`.
    #[arg(short, long, value_name = "DIR")]
    pub input: PathBuf,

    /// Directory to write the generated pages into.
    #[arg(short, long, value_name = "DIR")]
    pub output: PathBuf,

    /// Empty the output directory before writing.
    #[arg(short, long)]
    pub force: bool,
}

/// Arguments for `machete init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Directory to create the new site in.
    #[arg(short, long, value_name = "DIR")]
    pub output: PathBuf,

    /// Overwrite files that already exist instead of leaving them untouched.
    #[arg(short, long)]
    pub force: bool,
}

/// Arguments for `machete completions`.
#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// The shell to generate completions for.
    #[arg(value_name = "SHELL")]
    pub shell: Shell,
}

/// Parses the arguments and runs the command they select.
pub fn run(cli: Cli) -> Result<(), BuildError> {
    match cli.command {
        Command::Build(args) => {
            let config = Config::load(&args.input)?;
            build(&config, &args.output, args.force)
        }
        Command::Init(args) => write(&args.output, args.force),
        Command::Completions(args) => completions(args),
    }
}

/// Writes a completion script for the requested shell to stdout.
fn completions(args: CompletionsArgs) -> Result<(), BuildError> {
    let mut command = Cli::command();
    clap_complete::generate(args.shell, &mut command, "machete", &mut std::io::stdout());
    Ok(())
}
