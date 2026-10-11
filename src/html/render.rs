//! Serializes a document tree back into HTML source.

use crate::node::{Element, Node, VOID_ELEMENTS};

/// Renders a tree to an HTML string.
///
/// Output is stable: rendering a tree and parsing it again yields an equivalent
/// tree, so this is safe to use as a round-trip check.
pub fn render(node: &Node) -> String {
    let mut out = String::new();
    render_into(node, &mut out);
    out
}

/// Appends the source form of `node` to `out`.
///
/// Recurses over children. Void elements and the content marker are written
/// without a closing tag; everything else gets an explicit one.
fn render_into(node: &Node, out: &mut String) {
    match node {
        Node::Doctype(text) => {
            out.push_str("<!DOCTYPE ");
            out.push_str(text);
            out.push('>');
        }

        Node::Comment(text) => {
            out.push_str("<!--");
            out.push_str(text);
            out.push_str("-->");
        }

        Node::Text(text) => out.push_str(text),

        // The contents of a raw text element are already verbatim, so they are
        // appended as they are, without escaping.
        Node::RawText { content, .. } => out.push_str(content),

        Node::Element(element) => {
            out.push('<');
            out.push_str(&element.name);
            render_attrs(element, out);

            if element.is_content_marker() {
                out.push_str("/>");
                return;
            }

            if element.children.is_empty() && VOID_ELEMENTS.contains(&element.name.as_str()) {
                out.push('>');
                return;
            }

            out.push('>');
            for child in &element.children {
                render_into(child, out);
            }
            out.push_str("</");
            out.push_str(&element.name);
            out.push('>');
        }

        Node::Document(children) => {
            for child in children {
                render_into(child, out);
            }
        }
    }
}

/// Writes an element's attributes, quoting every value.
fn render_attrs(element: &Element, out: &mut String) {
    for attr in &element.attrs {
        out.push(' ');
        out.push_str(&attr.name);
        if let Some(value) = &attr.value {
            out.push_str("=\"");
            out.push_str(value);
            out.push('"');
        }
    }
}
