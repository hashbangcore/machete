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
/// Paths are given either as flags or as bare arguments, never both. A single
/// bare path is the output; two are the input and then the output. Whatever is
/// left unset falls back to `config.toml`, and finally to the current directory
/// and `public/`.
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
        self.input
            .as_deref()
            .or_else(|| (self.paths.len() > 1).then(|| self.paths[0].as_path()))
    }

    /// Returns the output to write, from the flag or the last bare path.
    pub fn output(&self) -> Option<&Path> {
        self.output
            .as_deref()
            .or_else(|| self.paths.last().map(PathBuf::as_path))
    }

    /// Rejects arguments that name the same thing twice or mix both forms.
    ///
    /// A bare path together with the matching flag is an error rather than a
    /// silent preference for one of them, since neither reading is obviously
    /// the one that was meant.
    pub fn check(&self) -> Result<(), BuildError> {
        let mut flags = Vec::new();
        if self.input.is_some() {
            flags.push("-i");
        }
        if self.output.is_some() {
            flags.push("-o");
        }

        if !flags.is_empty() && !self.paths.is_empty() {
            return Err(BuildError::MixedArguments {
                command: "build",
                flags: flags.join(" and "),
            });
        }

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
/// The directory is given either as a bare path or as a flag, never both. With
/// neither, the site is written to the current directory.
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

    /// Rejects arguments that name the directory twice or mix both forms.
    pub fn check(&self) -> Result<(), BuildError> {
        if self.output.is_some() && !self.paths.is_empty() {
            return Err(BuildError::MixedArguments {
                command: "init",
                flags: "-o".to_string(),
            });
        }

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
