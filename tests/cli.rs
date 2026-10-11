//! Argument parsing for the command line interface.
//!
//! `build` follows the Sass shape, `input:output`, where either side may be
//! left empty and is then filled in from `config.toml`. `init` takes at most
//! one bare path.

use clap::Parser;
use machete::cli::{BuildArgs, Cli, Command, InitArgs, Target};
use machete::error::BuildError;
use std::path::PathBuf;

fn build_args(target: Option<&str>, extra: &[&str]) -> BuildArgs {
    let mut full = vec!["machete", "build"];
    if let Some(target) = target {
        full.push(target);
    }
    full.extend_from_slice(extra);
    let Command::Build(args) = Cli::try_parse_from(&full)
        .expect("the arguments are accepted")
        .command
    else {
        panic!("expected the build command");
    };
    args
}

fn target(args: &str) -> Target {
    build_args(Some(args), &[]).parse().expect("a valid target")
}

fn init_args(args: &[&str]) -> InitArgs {
    let mut full = vec!["machete", "init"];
    full.extend_from_slice(args);
    let Command::Init(args) = Cli::try_parse_from(&full)
        .expect("the arguments are accepted")
        .command
    else {
        panic!("expected the init command");
    };
    args
}

#[test]
fn no_target_means_the_configuration_decides() {
    let parsed = build_args(None, &[]).parse().expect("valid");
    assert_eq!(parsed, Target::default());
    assert_eq!(parsed.input(), PathBuf::from("."));
    assert_eq!(parsed.output(), None);
}

#[test]
fn both_sides_given() {
    let parsed = target("scaffold:public");
    assert_eq!(parsed.input, Some(PathBuf::from("scaffold")));
    assert_eq!(parsed.output, Some(PathBuf::from("public")));
}

#[test]
fn an_empty_input_falls_back_to_the_current_directory() {
    let parsed = target(":public");
    assert_eq!(parsed.input, None);
    assert_eq!(parsed.input(), PathBuf::from("."));
    assert_eq!(parsed.output, Some(PathBuf::from("public")));
}

#[test]
fn an_empty_output_falls_back_to_the_configuration() {
    let parsed = target("scaffold:");
    assert_eq!(parsed.input, Some(PathBuf::from("scaffold")));
    assert_eq!(parsed.output, None);
}

#[test]
fn a_bare_path_without_a_colon_is_rejected() {
    // It would not say whether it is the source or the destination.
    let error = build_args(Some("public"), &[]).parse();
    assert!(matches!(error, Err(BuildError::InvalidTarget { .. })));
}

#[test]
fn two_colons_are_rejected() {
    let error = build_args(Some("a:b:c"), &[]).parse();
    assert!(matches!(error, Err(BuildError::InvalidTarget { .. })));
}

#[test]
fn the_invalid_target_message_names_what_it_wanted() {
    let error = build_args(Some("a:b:c"), &[])
        .parse()
        .expect_err("two colons");
    let message = error.to_string();
    assert!(message.contains("input:output"), "message was {message}");
    assert!(message.contains("a:b:c"), "message was {message}");
}

#[test]
fn force_travels_alongside_a_target() {
    assert!(build_args(Some("a:b"), &["-f"]).force);
    assert!(!build_args(Some("a:b"), &[]).force);
}

#[test]
fn init_defaults_to_the_current_directory() {
    assert_eq!(init_args(&[]).target(), PathBuf::from("."));
}

#[test]
fn init_takes_one_bare_path() {
    assert_eq!(init_args(&["new-site"]).target(), PathBuf::from("new-site"));
}

#[test]
fn init_takes_a_nested_path() {
    assert_eq!(
        init_args(&["output/dir/new-site"]).target(),
        PathBuf::from("output/dir/new-site")
    );
}

#[test]
fn init_takes_force() {
    assert!(init_args(&["-f"]).force);
    assert!(init_args(&["new-site", "-f"]).force);
}

#[test]
fn init_rejects_two_paths() {
    assert!(Cli::try_parse_from(["machete", "init", "a", "b"]).is_err());
}

#[test]
fn the_path_flags_are_gone() {
    assert!(Cli::try_parse_from(["machete", "build", "-i", "a"]).is_err());
    assert!(Cli::try_parse_from(["machete", "build", "-o", "a"]).is_err());
    assert!(Cli::try_parse_from(["machete", "init", "-o", "a"]).is_err());
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
