//! Replaces `{variable}` references with their values.
//!
//! Substitution runs over text nodes, attribute values and raw text prose.
//! It skips `<script>` and `<style>`, where braces belong to another language.

use super::scope::Scope;
use crate::error::SubstError;
use crate::node::{Node, is_code};

/// Replaces every `{name}` in the tree with the value found in `scope`.
///
/// Fails on the first reference that has no value, rather than leaving it in
/// the output, so a page is never emitted half-rendered.
pub fn substitute(node: &mut Node, scope: &Scope) -> Result<(), SubstError> {
    match node {
        Node::Text(text) => {
            *text = substitute_text(text, scope)?;
        }

        Node::Element(element) => {
            for attr in element.attrs.iter_mut() {
                if let Some(value) = attr.value.as_mut() {
                    *value = substitute_text(value, scope)?;
                }
            }
            for child in element.children.iter_mut() {
                substitute(child, scope)?;
            }
        }

        Node::Document(children) => {
            for child in children.iter_mut() {
                substitute(child, scope)?;
            }
        }

        Node::RawText { tag, content } => {
            // A `{` inside a script or a style belongs to that language.
            // `textarea` and `title` are also raw text but hold prose, so
            // their braces are Machete's and get substituted.
            if !is_code(tag) {
                *content = substitute_text(content, scope)?;
            }
        }

        Node::Comment(_) | Node::Doctype(_) => {}
    }

    Ok(())
}

/// Replaces `{name}` references in a single string.
///
/// An empty pair `{}` is left as it is, and a `{` with no closing brace is left
/// as literal text. Any other unresolved name is an error.
pub fn substitute_text(text: &str, scope: &Scope) -> Result<String, SubstError> {
    if !text.contains('{') {
        return Ok(text.to_string());
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start + 1..].find('}') else {
            break;
        };

        let name = &rest[start + 1..start + 1 + end];
        out.push_str(&rest[..start]);

        if name.is_empty() {
            out.push_str("{}");
        } else {
            let Some(value) = scope.get(name) else {
                return Err(SubstError::UndefinedVariable {
                    name: name.to_string(),
                });
            };
            out.push_str(value);
        }

        rest = &rest[start + end + 2..];
    }

    out.push_str(rest);
    Ok(out)
}
