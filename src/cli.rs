//! Command line interface.
//!
//! [`Cli`] describes the accepted commands and [`run`] dispatches one to its
//! handler. Both live here rather than in `main.rs` so the argument parsing can
//! be exercised from tests without spawning a process.

use crate::build::{build, write_scaffold, Config, Overrides};
use crate::error::BuildError;
use crate::report::{self, Format};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::path::{Path, PathBuf};

/// Static site generator.
#[derive(Debug, Parser)]
#[command(name = "machete", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output format.
    #[arg(long, value_enum, value_name = "FORMAT", global = true)]
    pub format: Option<Format>,

    /// Quiet mode
    #[arg(long, global = true)]
    pub quiet: bool,
}

/// The commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate static pages
    Build(BuildArgs),

    /// Scaffold a project
    Init(InitArgs),

    /// Print a completion script.
    Completions(CompletionsArgs),
}

/// Arguments for `machete build`.
#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Source and destination, separated by a colon.
    #[arg(value_name = "INPUT:OUTPUT")]
    pub target: Option<String>,

    /// Templates directory.
    #[arg(short, long, value_name = "DIR")]
    pub templates: Option<PathBuf>,

    /// Assets directory.
    #[arg(short = 'a', long, value_name = "DIR")]
    pub assets: Option<PathBuf>,
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

    /// Returns the directories the flags set, leaving the rest to the file.
    pub fn overrides(&self) -> Overrides {
        Overrides {
            output: None,
            templates: self.templates.clone(),
            assets: self.assets.clone(),
        }
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
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Target directory for the new project
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
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
            report::identify("build", &target.input());
            let config = Config::load_with(&target.input(), &args.overrides())?;
            let output = config.output_dir(target.output());
            let built = build(&config, &output)?;
            if !built.sitemap {
                report::warning("no sitemap written: [build] has no url, or sitemap is turned off");
            }
            report::success(Some(&output), built.pages, built.assets);
            Ok(())
        }
        Command::Init(args) => {
            let root = args.target();
            report::identify("init", &root);
            let files = write_scaffold(&root)?;
            report::scaffolded(&root, files);
            Ok(())
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
