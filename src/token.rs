//! Turns HTML source into a flat sequence of tokens.
//!
//! This is the first stage of parsing. It recognises tags, attributes, text and
//! comments, but performs no nesting checks: matching start tags to end tags is
//! the job of [`crate::tree::build`].

use crate::error::TokenizeError;
use crate::node::Attr;

/// Elements whose content is raw text.
///
/// Inside these elements a `<` does not start a tag and a `{` does not start a
/// variable reference. The contents are captured verbatim up to the matching
/// closing tag, so JavaScript and CSS survive parsing unchanged.
pub const RAWTEXT_ELEMENTS: [&str; 4] = ["script", "style", "textarea", "title"];

/// A single lexical unit of an HTML document.
///
/// `offset` records the byte position where the token starts, so errors can
/// point back at the original source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A doctype declaration, without the `<!DOCTYPE` and `>` markers.
    Doctype(String),
    /// A comment body, without the `<!--` and `-->` delimiters.
    Comment(String),
    /// An opening tag, plus a closing tag `</name>`.
    StartTag {
        name: String,
        attrs: Vec<Attr>,
        /// True when the source wrote a trailing slash, e.g. `<br/>`.
        self_closing: bool,
        offset: usize,
    },
    /// A closing tag, e.g. `</div>`.
    EndTag { name: String, offset: usize },
    /// Character data between tags.
    Text(String),
    /// A raw text element: its opening tag, its verbatim contents and nothing
    /// else. The matching closing tag is consumed and not emitted separately.
    RawText {
        tag: String,
        attrs: Vec<Attr>,
        content: String,
        offset: usize,
    },
}

/// Splits `src` into tokens in document order.
///
/// Runs in a single pass, skipping over characters that cannot begin a token and
/// emitting the text between them. Fails only when a tag, comment or quoted
/// attribute value is left unterminated.
pub fn tokenize(src: &str) -> Result<Vec<Token>, TokenizeError> {
    let bytes = src.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut text_start = 0;

    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }

        if src[index..].starts_with("<!--") {
            let offset = index;
            push_text(&mut tokens, &src[text_start..offset]);
            let end = src[index + 4..]
                .find("-->")
                .map(|rel| index + 4 + rel)
                .ok_or(TokenizeError::UnterminatedComment { offset })?;
            tokens.push(Token::Comment(src[index + 4..end].to_string()));
            index = end + 3;
            text_start = index;
            continue;
        }

        if src[index..].to_ascii_lowercase().starts_with("<!doctype") {
            let offset = index;
            push_text(&mut tokens, &src[text_start..offset]);
            let end = src[index..]
                .find('>')
                .ok_or(TokenizeError::UnterminatedTag { offset })?;
            // Store only what follows `<!DOCTYPE`, since render adds it back.
            let body = src[index + 9..end].trim();
            tokens.push(Token::Doctype(body.to_string()));
            index = end + 1;
            text_start = index;
            continue;
        }

        if src[index..].starts_with("</") {
            let offset = index;
            push_text(&mut tokens, &src[text_start..offset]);
            let (name, end) = read_tag_name(src, index + 2, offset)?;
            index = skip_to_tag_end(src, end, offset)?;
            tokens.push(Token::EndTag { name, offset });
            text_start = index;
            continue;
        }

        // A `<` that is not followed by a name character is literal text,
        // for example the `a < b` inside a code sample.
        let Some((name, mut cursor)) = read_optional_tag_name(src, index + 1) else {
            index += 1;
            continue;
        };

        let offset = index;
        push_text(&mut tokens, &src[text_start..offset]);

        let (attrs, self_closing) = read_attrs(src, &mut cursor)?;
        index = skip_to_tag_end(src, cursor, offset)?;

        if !self_closing && RAWTEXT_ELEMENTS.contains(&name.as_str()) {
            let content_start = index;
            let Some(closing) = find_rawtext_end(src, index, &name) else {
                return Err(TokenizeError::UnclosedRawText { tag: name });
            };
            tokens.push(Token::RawText {
                tag: name.clone(),
                attrs,
                content: src[content_start..closing].to_string(),
                offset,
            });
            index = src[closing..]
                .find('>')
                .map(|rel| closing + rel + 1)
                .ok_or(TokenizeError::UnclosedRawText { tag: name })?;
            text_start = index;
            continue;
        }

        tokens.push(Token::StartTag {
            name,
            attrs,
            self_closing,
            offset,
        });
        text_start = index;
    }

    push_text(&mut tokens, &src[text_start..]);
    Ok(tokens)
}

/// Emits a text token, skipping empty runs so blank gaps between tags are not
/// recorded as zero-length nodes.
fn push_text(tokens: &mut Vec<Token>, text: &str) {
    if !text.is_empty() {
        tokens.push(Token::Text(text.to_string()));
    }
}

/// Reads the name of a closing tag, which must not be empty.
fn read_tag_name(src: &str, start: usize, offset: usize) -> Result<(String, usize), TokenizeError> {
    read_optional_tag_name(src, start).ok_or(TokenizeError::EmptyTagName { offset })
}

/// Reads a lowercased tag name, returning `None` when no name character
/// follows at `start`.
fn read_optional_tag_name(src: &str, start: usize) -> Option<(String, usize)> {
    let bytes = src.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() && is_name_byte(bytes[cursor]) {
        cursor += 1;
    }
    if cursor == start {
        return None;
    }
    Some((src[start..cursor].to_ascii_lowercase(), cursor))
}

/// Reports whether a byte may appear in a tag or attribute name.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
}

/// Reads attributes up to the closing `>` of a tag.
///
/// Accepts double-quoted, single-quoted and unquoted values, and attributes
/// written without a value. A `/` before the closing `>` sets `self_closing`
/// rather than producing an attribute.
fn read_attrs(src: &str, cursor: &mut usize) -> Result<(Vec<Attr>, bool), TokenizeError> {
    let bytes = src.as_bytes();
    let mut attrs = Vec::new();
    let mut self_closing = false;

    loop {
        while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }

        if *cursor >= bytes.len() || bytes[*cursor] == b'>' {
            break;
        }

        if bytes[*cursor] == b'/' {
            self_closing = true;
            *cursor += 1;
            continue;
        }

        if !is_name_byte(bytes[*cursor]) {
            *cursor += 1;
            continue;
        }

        let start = *cursor;
        while *cursor < bytes.len() && is_name_byte(bytes[*cursor]) {
            *cursor += 1;
        }
        let name = src[start..*cursor].to_ascii_lowercase();

        while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }

        // No `=` follows, so this is a valueless attribute.
        if *cursor >= bytes.len() || bytes[*cursor] != b'=' {
            attrs.push(Attr::bare(name));
            continue;
        }

        *cursor += 1;
        while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }

        if *cursor >= bytes.len() {
            attrs.push(Attr::new(name, ""));
            break;
        }

        let quote = bytes[*cursor];
        if quote == b'"' || quote == b'\'' {
            *cursor += 1;
            let value_start = *cursor;
            let Some(rel) = src[value_start..].find(quote as char) else {
                return Err(TokenizeError::UnterminatedAttribute { name });
            };
            attrs.push(Attr::new(name, &src[value_start..value_start + rel]));
            *cursor = value_start + rel + 1;
        } else {
            let value_start = *cursor;
            while *cursor < bytes.len()
                && !bytes[*cursor].is_ascii_whitespace()
                && bytes[*cursor] != b'>'
            {
                *cursor += 1;
            }
            attrs.push(Attr::new(name, &src[value_start..*cursor]));
        }
    }

    Ok((attrs, self_closing))
}

/// Advances past the closing `>` of a tag, skipping any `>` that falls inside a
/// quoted attribute value.
fn skip_to_tag_end(src: &str, cursor: usize, offset: usize) -> Result<usize, TokenizeError> {
    let bytes = src.as_bytes();
    let mut index = cursor;

    while index < bytes.len() {
        if bytes[index] == b'"' || bytes[index] == b'\'' {
            let quote = bytes[index];
            index += 1;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
        } else if bytes[index] == b'>' {
            return Ok(index + 1);
        }
        index += 1;
    }

    Err(TokenizeError::UnterminatedTag { offset })
}

/// Finds the `</tag` that closes a raw text element.
///
/// A match only counts when the text after the tag name is whitespace, a slash
/// or `>`, so `</scripted>` does not close a `<script>`.
fn find_rawtext_end(src: &str, from: usize, tag: &str) -> Option<usize> {
    let needle = format!("</{tag}");
    let bytes = src.as_bytes();
    let mut index = from;

    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }

        let remaining = &src[index..];
        if remaining.len() < needle.len() {
            return None;
        }

        if remaining.to_ascii_lowercase().starts_with(&needle) {
            let after = bytes.get(index + needle.len()).copied();
            if after.is_none_or(|b| b == b'>' || b.is_ascii_whitespace() || b == b'/') {
                return Some(index);
            }
        }

        index += 1;
    }

    None
}
