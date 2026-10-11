//! Argument parsing and path resolution for the command line interface.
//!
//! These go through clap rather than the handlers, so they check what the
//! justfile passes and the shorter forms a user types by hand.
//!
//! Paths are given either as bare arguments or as flags, never both. A bare
//! path beats `config.toml`, and `config.toml` beats the built-in default.

use clap::Parser;
use machete::cli::{BuildArgs, Cli, Command, InitArgs};
use machete::error::BuildError;
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
fn a_flag_with_a_bare_path_is_rejected() {
    let error = build_args(&["-i", "flag", "site", "out"]).check();
    assert!(matches!(error, Err(BuildError::MixedArguments { .. })));
}

#[test]
fn the_mixed_message_names_the_flag_and_the_command() {
    let Err(BuildError::MixedArguments { command, flags }) = build_args(&["-i", "a", "b"]).check()
    else {
        panic!("expected a mixed-arguments error");
    };
    assert_eq!(command, "build");
    assert_eq!(flags, "-i");
}

#[test]
fn both_flags_at_once_are_fine_without_bare_paths() {
    assert!(build_args(&["-i", "a", "-o", "b"]).check().is_ok());
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

fn init_args(args: &[&str]) -> InitArgs {
    let mut full = vec!["machete", "init"];
    full.extend_from_slice(args);
    let Command::Init(parsed) = Cli::try_parse_from(&full)
        .expect("the arguments are accepted")
        .command
    else {
        panic!("expected the init command");
    };
    parsed
}

#[test]
fn init_takes_a_flag_and_force() {
    let args = init_args(&["-f", "-o", "demo"]);
    assert_eq!(args.target(), PathBuf::from("demo"));
    assert!(args.force);
}

#[test]
fn init_takes_a_bare_path() {
    assert_eq!(init_args(&["demo-01"]).target(), PathBuf::from("demo-01"));
}

#[test]
fn init_defaults_to_the_current_directory() {
    assert_eq!(init_args(&[]).target(), PathBuf::from("."));
}

#[test]
fn an_init_flag_with_a_bare_path_is_rejected() {
    let error = init_args(&["bare", "-o", "flag"]).check();
    assert!(matches!(error, Err(BuildError::MixedArguments { .. })));
}

#[test]
fn init_rejects_the_mix_even_with_a_third_path() {
    // `init dir5 -o dir6 dir7` reports the mix, which is the better message:
    // naming the directory twice in two ways is the real problem.
    let error = init_args(&["dir5", "-o", "dir6", "dir7"]).check();
    assert!(matches!(error, Err(BuildError::MixedArguments { .. })));
}

#[test]
fn init_rejects_two_bare_paths() {
    assert!(init_args(&["a", "b"]).check().is_err());
    assert!(init_args(&["a"]).check().is_ok());
    assert!(init_args(&[]).check().is_ok());
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
