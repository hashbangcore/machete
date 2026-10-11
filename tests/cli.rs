//! Argument parsing for the command line interface.
//!
//! These go through clap rather than the handlers, so they check what the
//! justfile actually passes: `build -i <site> -f -o <out>` and
//! `init -f -o <dir>`.

use clap::Parser;
use machete::cli::{Cli, Command};

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("machete").chain(args.iter().copied()))
        .expect("the arguments are accepted")
}

#[test]
fn build_takes_input_force_and_output() {
    let cli = parse(&["build", "-i", "./scaffold", "-f", "-o", "./outputs/"]);
    let Command::Build(args) = cli.command else {
        panic!("expected the build command");
    };
    assert_eq!(args.input.to_str(), Some("./scaffold"));
    assert_eq!(args.output.to_str(), Some("./outputs/"));
    assert!(args.force);
}

#[test]
fn build_accepts_long_flags() {
    let cli = parse(&["build", "--input", "site", "--output", "out", "--force"]);
    let Command::Build(args) = cli.command else {
        panic!("expected the build command");
    };
    assert!(args.force);
}

#[test]
fn init_takes_force_and_output() {
    let cli = parse(&["init", "-f", "-o", "./outputs/init-example"]);
    let Command::Init(args) = cli.command else {
        panic!("expected the init command");
    };
    assert_eq!(args.output.to_str(), Some("./outputs/init-example"));
    assert!(args.force);
}

#[test]
fn build_requires_an_input() {
    assert!(Cli::try_parse_from(["machete", "build", "-o", "out"]).is_err());
}

#[test]
fn build_requires_an_output() {
    assert!(Cli::try_parse_from(["machete", "build", "-i", "site"]).is_err());
}

#[test]
fn init_requires_an_output() {
    assert!(Cli::try_parse_from(["machete", "init"]).is_err());
}

#[test]
fn force_defaults_to_false() {
    let Command::Build(args) = parse(&["build", "-i", "s", "-o", "o"]).command else {
        panic!("expected the build command");
    };
    assert!(!args.force);
}

#[test]
fn completions_takes_a_shell() {
    assert!(matches!(
        parse(&["completions", "bash"]).command,
        Command::Completions(_)
    ));
}

#[test]
fn an_unknown_command_is_rejected() {
    assert!(Cli::try_parse_from(["machete", "nope"]).is_err());
}

#[test]
fn no_command_shows_help() {
    assert!(Cli::try_parse_from(["machete"]).is_err());
}
