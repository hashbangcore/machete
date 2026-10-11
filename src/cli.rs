//! Command line interface.
//!
//! [`Cli`] describes the accepted commands and [`run`] dispatches one to its
//! handler. Both live here rather than in `main.rs` so the argument parsing can
//! be exercised from tests without spawning a process.

use crate::build::{Config, build, write_scaffold};
use crate::error::BuildError;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::path::{Path, PathBuf};

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
///
/// Paths follow the Sass shape, `input:output`. Either side may be left empty,
/// and an empty side is filled in from `machete.toml` rather than defaulted here,
/// so `machete build :dist` writes to `dist` only when the config says so.
#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Source and destination, separated by a colon.
    #[arg(value_name = "INPUT:OUTPUT")]
    pub target: Option<String>,

    /// Empty the output directory before writing.
    #[arg(short, long)]
    pub force: bool,
}

impl BuildArgs {
    /// Splits `input:output` into its two halves.
    ///
    /// A bare path without a colon is rejected: it would not say whether it is
    /// the source or the destination, and guessing would quietly build the
    /// wrong thing.
    pub fn parse(&self) -> Result<Target, BuildError> {
        let Some(text) = self.target.as_deref() else {
            return Ok(Target::default());
        };

        let Some((input, output)) = text.split_once(':') else {
            return Err(BuildError::InvalidTarget {
                found: text.to_string(),
                reason: "missing ':' between input and output",
            });
        };

        if output.contains(':') {
            return Err(BuildError::InvalidTarget {
                found: text.to_string(),
                reason: "only one ':' is allowed",
            });
        }

        Ok(Target {
            input: non_empty(input),
            output: non_empty(output),
        })
    }
}

/// The two sides of a build, either of which may be left to `machete.toml`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Target {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
}

impl Target {
    /// Returns the site root to read.
    pub fn input(&self) -> PathBuf {
        self.input.clone().unwrap_or_else(|| PathBuf::from("."))
    }

    /// Returns the output to write, or `None` when the config decides.
    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }
}

/// Reads a path as given, treating an empty half as unset.
fn non_empty(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// Arguments for `machete init`.
///
/// Takes at most one bare path: the directory to create. With none, the site
/// lands in the current directory.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Directory to create the new site in.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Overwrite files that already exist instead of leaving them untouched.
    #[arg(short, long)]
    pub force: bool,
}

impl InitArgs {
    /// Returns the directory to write the site into.
    pub fn target(&self) -> PathBuf {
        self.path.clone().unwrap_or_else(|| PathBuf::from("."))
    }
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
            let target = args.parse()?;
            let config = Config::load(&target.input())?;
            build(&config, &config.output_dir(target.output()), args.force)
        }
        Command::Init(args) => write_scaffold(&args.target(), args.force),
        Command::Completions(args) => completions(args),
    }
}

/// Writes a completion script for the requested shell to stdout.
fn completions(args: CompletionsArgs) -> Result<(), BuildError> {
    let mut command = Cli::command();
    clap_complete::generate(args.shell, &mut command, "machete", &mut std::io::stdout());
    Ok(())
}
