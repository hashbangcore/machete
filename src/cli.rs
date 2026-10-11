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
/// Paths may be given as flags or as bare arguments. A single bare path is the
/// output; two are the input and then the output. Whatever is left unset falls
/// back to `config.toml`, and finally to the current directory and `public/`.
/// Flags win over bare paths, and both win over the configuration.
#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Output path, or input and output when given twice.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Site directory holding `config.toml` and `templates/`.
    #[arg(short, long, value_name = "DIR")]
    pub input: Option<PathBuf>,

    /// Directory to write the generated pages into.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,

    /// Empty the output directory before writing.
    #[arg(short, long)]
    pub force: bool,
}

impl BuildArgs {
    /// Returns the site root to read, from the flag or the first bare path.
    ///
    /// A lone bare path is the output, so only a pair carries an input.
    pub fn input(&self) -> Option<&Path> {
        self.input.as_deref().or_else(|| {
            // A lone bare path is the output, so the input needs a pair.
            let pair = matches!(self.paths.as_slice(), [_, _]);
            pair.then(|| self.paths[0].as_path())
        })
    }

    /// Returns the output to write, from the flag or the last bare path.
    pub fn output(&self) -> Option<&Path> {
        self.output
            .as_deref()
            .or_else(|| self.paths.last().map(PathBuf::as_path))
    }

    /// Fails when more than two bare paths are given.
    pub fn check(&self) -> Result<(), BuildError> {
        if self.paths.len() > 2 {
            return Err(BuildError::TooManyPaths {
                command: "build",
                given: self.paths.len(),
            });
        }
        Ok(())
    }
}

/// Arguments for `machete init`.
///
/// Takes the directory as a bare path or as a flag, the same way `build` does.
/// The flag wins. With neither, the site is written to the current directory.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Directory to create the new site in.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Directory to create the new site in.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,

    /// Overwrite files that already exist instead of leaving them untouched.
    #[arg(short, long)]
    pub force: bool,
}

impl InitArgs {
    /// Returns the directory to write the site into.
    pub fn target(&self) -> PathBuf {
        self.output
            .clone()
            .or_else(|| self.paths.first().cloned())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Fails when more than one bare path is given.
    pub fn check(&self) -> Result<(), BuildError> {
        if self.paths.len() > 1 {
            return Err(BuildError::TooManyPaths {
                command: "init",
                given: self.paths.len(),
            });
        }
        Ok(())
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
            args.check()?;
            let root = args
                .input()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let config = Config::load(&root)?;
            build(&config, &config.output_dir(args.output()), args.force)
        }
        Command::Init(args) => {
            args.check()?;
            write_scaffold(&args.target(), args.force)
        }
        Command::Completions(args) => completions(args),
    }
}

/// Writes a completion script for the requested shell to stdout.
fn completions(args: CompletionsArgs) -> Result<(), BuildError> {
    let mut command = Cli::command();
    clap_complete::generate(args.shell, &mut command, "machete", &mut std::io::stdout());
    Ok(())
}
