//! Reads variable declarations from a component script.
//!
//! The body of a `<script type="component">` block holds one declaration per
//! line. `const`, `let` and `var` are all accepted and optional, which keeps the
//! block valid JavaScript so syntax highlighting and linters still work.

use super::scope::Bindings;
use crate::error::ScriptError;

/// Parses declarations of the form `name = "value"` into variable bindings.
///
/// Lines that hold only a comment are ignored. Anything that is not a
/// well-formed declaration is reported with its line number rather than
/// skipped, so a typo fails the build instead of silently losing a value.
pub fn parse_bindings(src: &str) -> Result<Bindings, ScriptError> {
    let mut bindings = Bindings::new();

    for (index, raw) in src.lines().enumerate() {
        let line = index + 1;
        let statement = strip_statement(raw)?;

        if statement.is_empty() {
            continue;
        }

        let Some((name, value)) = statement.split_once('=') else {
            return Err(ScriptError::MissingEquals {
                name: statement.to_string(),
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

        let value = unquote(value.trim(), name, line)?;
        bindings.insert(name.to_string(), value);
    }

    Ok(bindings)
}

/// Strips comments and the trailing semicolon from a declaration line.
fn strip_statement(raw: &str) -> Result<&str, ScriptError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return Ok("");
    }
    Ok(trimmed.trim_end_matches(';').trim())
}

/// Removes any number of leading `const`, `let` or `var` keywords.
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

/// Removes the quotes from a value and resolves backslash escapes.
fn unquote(value: &str, name: &str, line: usize) -> Result<String, ScriptError> {
    if value.is_empty() {
        return Err(ScriptError::MissingValue {
            name: name.to_string(),
            line,
        });
    }

    let Some(quote) = value.chars().next().filter(|c| *c == '"' || *c == '\'') else {
        return Err(ScriptError::UnexpectedToken {
            text: value.to_string(),
            line,
        });
    };

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
