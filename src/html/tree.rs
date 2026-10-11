//! Builds a document tree from a token stream.
//!
//! Nesting is resolved with a stack of open elements. Every start tag must be
//! matched by an end tag, except void elements and the `<content/>` marker,
//! which are self-closing by definition.

use super::token::Token;
use crate::error::{ParseError, TreeError};
use crate::node::{Element, Node};

/// Accumulates nodes and tracks which element is currently open.
///
/// Top-level nodes go to `roots`; anything else is appended to the children of
/// the element at the top of `stack`.
struct Builder {
    roots: Vec<Node>,
    stack: Vec<Element>,
}

impl Builder {
    fn new() -> Self {
        Self {
            roots: Vec::new(),
            stack: Vec::new(),
        }
    }

    /// Adds a node to the innermost open element, or to the roots when none is
    /// open.
    fn attach(&mut self, node: Node) {
        match self.stack.last_mut() {
            Some(parent) => parent.children.push(node),
            None => self.roots.push(node),
        }
    }

    /// Pushes an element onto the open stack.
    fn open(&mut self, element: Element) {
        self.stack.push(element);
    }

    /// Closes the innermost open element with the given name and attaches it.
    ///
    /// Fails when that name is not open, or when it is open but other elements
    /// sit above it, since those would be left unclosed.
    fn close(&mut self, name: &str, offset: usize) -> Result<(), TreeError> {
        let Some(position) = self.stack.iter().rposition(|e| e.name == name) else {
            return Err(TreeError::MismatchedTag {
                expected: name.to_string(),
                found: name.to_string(),
                offset,
            });
        };

        if position + 1 != self.stack.len() {
            let unclosed = self.stack[position + 1].name.clone();
            return Err(TreeError::UnclosedTag {
                name: unclosed,
                offset,
            });
        }

        let element = self.stack.pop().expect("position is the last index");
        self.attach(Node::Element(element));
        Ok(())
    }

    /// Returns the finished tree, or fails if any element is still open.
    fn finish(self) -> Result<Node, TreeError> {
        if let Some(unclosed) = self.stack.first() {
            return Err(TreeError::UnclosedTag {
                name: unclosed.name.clone(),
                offset: 0,
            });
        }
        Ok(Node::Document(self.roots))
    }
}

/// Builds a document tree from `tokens`.
///
/// Applies Machete's self-closing rule: the void elements and the three
/// constructions of Machete (`<content/>`, `type="component"` and
/// `type="layout"`) close themselves. Every other element requires an explicit
/// closing tag, so `<div/>` is an error.
pub fn build(tokens: Vec<Token>) -> Result<Node, TreeError> {
    let mut builder = Builder::new();

    for token in tokens {
        match token {
            Token::Doctype(text) => builder.attach(Node::Doctype(text)),
            Token::Comment(text) => builder.attach(Node::Comment(text)),
            Token::Text(text) => builder.attach(Node::Text(text)),

            Token::RawText {
                tag,
                attrs,
                content,
                ..
            } => {
                builder.attach(Node::Element(Element {
                    name: tag.clone(),
                    attrs,
                    children: vec![Node::RawText { tag, content }],
                }));
            }

            Token::StartTag {
                name,
                attrs,
                self_closing,
                offset,
            } => {
                let element = Element {
                    name,
                    attrs,
                    children: Vec::new(),
                };

                if self_closing && !element.accepts_self_closing() {
                    return Err(TreeError::UnclosedTag {
                        name: element.name,
                        offset,
                    });
                }

                if self_closing || element.is_void() {
                    builder.attach(Node::Element(element));
                } else {
                    builder.open(element);
                }
            }

            Token::EndTag { name, offset } => builder.close(&name, offset)?,
        }
    }

    builder.finish()
}

/// Parses HTML source into a document tree.
///
/// Convenience wrapper over [`super::token::tokenize`] and [`build`], for
/// callers that have no use for the intermediate tokens.
pub fn parse(src: &str) -> Result<Node, ParseError> {
    let tokens = super::token::tokenize(src)?;
    Ok(build(tokens)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::render;

    fn roundtrip(src: &str) -> String {
        render(&parse(src).expect("valid source"))
    }

    #[test]
    fn nests_elements() {
        assert_eq!(
            roundtrip("<div><section><p>x</p></section></div>"),
            "<div><section><p>x</p></section></div>"
        );
    }

    #[test]
    fn void_elements_never_nest() {
        assert_eq!(
            roundtrip("<br><img src=\"a.png\">"),
            "<br><img src=\"a.png\">"
        );
    }

    #[test]
    fn content_marker_is_self_closing() {
        assert_eq!(roundtrip("<div><content/></div>"), "<div><content/></div>");
    }

    #[test]
    fn component_invocation_may_self_close() {
        let tree = parse(r#"<header type="component" title="x"/>"#).expect("valid");
        let element = tree.child_list().unwrap()[0].as_element().unwrap();
        assert_eq!(element.name, "header");
        assert!(element.children.is_empty());
    }

    #[test]
    fn layout_directive_may_self_close() {
        assert!(parse(r#"<html type="layout"/>"#).is_ok());
    }

    #[test]
    fn a_non_void_tag_does_not_self_close() {
        assert!(parse("<div/>").is_err());
        assert!(parse("<span></span/></div>").is_err());
    }

    #[test]
    fn rejects_a_tag_left_open() {
        assert!(parse("<div><p>a</div>").is_err());
    }

    #[test]
    fn rejects_an_unmatched_end_tag() {
        assert!(parse("<div>a</section>").is_err());
    }

    #[test]
    fn raw_text_becomes_an_element_with_one_text_child() {
        let tree = parse("<script>a<b</script>").expect("valid");
        let script = tree.child_list().unwrap()[0].as_element().unwrap();
        assert_eq!(script.name, "script");
        assert!(matches!(script.children[0], Node::RawText { .. }));
    }

    #[test]
    fn roundtrips_a_whole_document() {
        let src = "<!DOCTYPE html><html lang=\"es\"><head><title>T</title></head><body><p>ok</p></body></html>";
        assert_eq!(roundtrip(src), src);
    }
}
