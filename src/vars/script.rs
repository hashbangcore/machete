//! Reads variable declarations from a component data block.
//!
//! The body of a `<script type="data">` block holds one declaration per line
//! written as `clave = valor`. This is not JavaScript and is never executed as
//! such; the block lives inside a `<script>` only so the editor highlights it
//! and recognises its literals.

use super::scope::Bindings;
use crate::error::ScriptError;

/// Parses declarations of the form `name = value` into variable bindings.
///
/// Lines that hold only a comment are ignored. A `const`, `let` or `var` in
/// front of the name is accepted and ignored, since a block written that way is
/// still valid JavaScript for the editor. Anything that is not a well-formed
/// declaration is reported with its line number rather than skipped, so a typo
/// fails the build instead of silently losing a value.
pub fn parse_bindings(src: &str) -> Result<Bindings, ScriptError> {
    let mut bindings = Bindings::new();

    for (index, raw) in src.lines().enumerate() {
        let line = index + 1;
        let statement = strip_statement(raw);

        if statement.is_empty() {
            continue;
        }

        let Some((name, value)) = statement.split_once('=') else {
            return Err(ScriptError::MissingEquals {
                text: statement.to_string(),
                line,
            });
        };

        let name = strip_keyword(name.trim());
        if !is_valid_name(name) {
            return Err(ScriptError::InvalidName {
                text: name.to_string(),
                line,
            });
        }

        let value = read_value(value.trim(), name, line)?;
        bindings.insert(name.to_string(), value);
    }

    Ok(bindings)
}

/// Strips a comment line and the trailing semicolon from a declaration.
fn strip_statement(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return "";
    }
    trimmed.trim_end_matches(';').trim()
}

/// Removes any leading `const`, `let` or `var` keyword.
fn strip_keyword(input: &str) -> &str {
    let mut rest = input;

    loop {
        let trimmed = rest.trim_start();
        let Some(keyword) = ["const ", "let ", "var "]
            .into_iter()
            .find(|prefix| trimmed.starts_with(*prefix))
        else {
            return trimmed;
        };
        rest = &trimmed[keyword.len()..];
    }
}

/// Reports whether a name may be used as a variable identifier.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Reads a value, which may be quoted or bare.
///
/// A quoted value keeps its interior verbatim and resolves backslash escapes.
/// A bare value runs to the end of the line, so `flag = true` reads as the text
/// `true`. Machete infers no types, so nothing is converted.
fn read_value(value: &str, name: &str, line: usize) -> Result<String, ScriptError> {
    if value.is_empty() {
        return Err(ScriptError::MissingValue {
            name: name.to_string(),
            line,
        });
    }

    let quote = value.chars().next().expect("value is not empty");
    if quote != '"' && quote != '\'' {
        return Ok(value.to_string());
    }

    let Some(body) = value
        .strip_prefix(quote)
        .and_then(|rest| rest.strip_suffix(quote))
    else {
        return Err(ScriptError::UnterminatedString {
            name: name.to_string(),
            line,
        });
    };

    Ok(unescape(body))
}

/// Resolves the escape sequences a quoted value may contain.
///
/// An unknown escape keeps the backslash, so a Windows path such as `C:\temp`
/// survives unchanged.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
}
