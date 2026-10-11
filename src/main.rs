use machete::render::render;
use machete::tree::parse;
use std::process::ExitCode;

/// Reads HTML from an argument, parses it and prints the result.
///
/// This is a placeholder for the real command line interface, which will read
/// `config.toml`, expand a directory of templates and write pages to disk.
fn main() -> ExitCode {
    let Some(source) = std::env::args().nth(1) else {
        eprintln!("usage: machete <html>");
        return ExitCode::FAILURE;
    };

    match parse(&source) {
        Ok(tree) => {
            println!("{}", render(&tree));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
