//! Argument parsing and path resolution for the command line interface.
//!
//! These go through clap rather than the handlers, so they check what the
//! justfile passes and the shorter forms a user types by hand.
//!
//! Precedence runs from strongest to weakest: a flag beats a bare path, a bare
//! path beats `config.toml`, and `config.toml` beats the built-in default.

use clap::Parser;
use machete::cli::{BuildArgs, Cli, Command};
use std::path::{Path, PathBuf};

fn build_args(args: &[&str]) -> BuildArgs {
    let mut full = vec!["machete", "build"];
    full.extend_from_slice(args);
    let Command::Build(parsed) = Cli::try_parse_from(&full)
        .expect("the arguments are accepted")
        .command
    else {
        panic!("expected the build command");
    };
    parsed
}

fn input(args: &[&str]) -> Option<PathBuf> {
    build_args(args).input().map(PathBuf::from)
}

fn output(args: &[&str]) -> Option<PathBuf> {
    build_args(args).output().map(PathBuf::from)
}

#[test]
fn flags_set_both_paths() {
    let args = build_args(&["-i", "./scaffold", "-f", "-o", "./outputs/"]);
    assert_eq!(args.input(), Some(Path::new("./scaffold")));
    assert_eq!(args.output(), Some(Path::new("./outputs/")));
    assert!(args.force);
}

#[test]
fn flags_also_have_long_forms() {
    let args = build_args(&["--input", "site", "--output", "out", "--force"]);
    assert_eq!(args.input(), Some(Path::new("site")));
    assert_eq!(args.output(), Some(Path::new("out")));
    assert!(args.force);
}

#[test]
fn no_paths_fall_back_to_the_configuration() {
    assert_eq!(input(&[]), None);
    assert_eq!(output(&[]), None);
}

#[test]
fn one_bare_path_is_the_output() {
    assert_eq!(input(&["out"]), None);
    assert_eq!(output(&["out"]), Some(PathBuf::from("out")));
}

#[test]
fn two_bare_paths_are_the_input_then_the_output() {
    assert_eq!(input(&["site", "out"]), Some(PathBuf::from("site")));
    assert_eq!(output(&["site", "out"]), Some(PathBuf::from("out")));
}

#[test]
fn a_flag_beats_a_bare_path() {
    assert_eq!(
        input(&["-i", "flag", "site", "out"]),
        Some(PathBuf::from("flag"))
    );
    assert_eq!(
        output(&["-i", "site", "out", "-o", "flag"]),
        Some(PathBuf::from("flag"))
    );
}

#[test]
fn three_bare_paths_are_rejected() {
    let args = build_args(&["a", "b", "c"]);
    assert!(args.check().is_err());
}

#[test]
fn one_or_two_paths_are_accepted() {
    assert!(build_args(&[]).check().is_ok());
    assert!(build_args(&["a"]).check().is_ok());
    assert!(build_args(&["a", "b"]).check().is_ok());
}

#[test]
fn force_defaults_to_false() {
    assert!(!build_args(&[]).force);
}

#[test]
fn init_takes_force_and_output() {
    let cli = Cli::try_parse_from(["machete", "init", "-f", "-o", "./outputs/init-example"])
        .expect("the arguments are accepted");
    let Command::Init(args) = cli.command else {
        panic!("expected the init command");
    };
    assert_eq!(args.output.to_str(), Some("./outputs/init-example"));
    assert!(args.force);
}

#[test]
fn init_requires_an_output() {
    assert!(Cli::try_parse_from(["machete", "init"]).is_err());
}

#[test]
fn completions_takes_a_shell() {
    assert!(matches!(
        Cli::try_parse_from(["machete", "completions", "bash"])
            .expect("the arguments are accepted")
            .command,
        Command::Completions(_)
    ));
}

#[test]
fn an_unknown_command_is_rejected() {
    assert!(Cli::try_parse_from(["machete", "nope"]).is_err());
}
