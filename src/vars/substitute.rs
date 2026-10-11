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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::{parse, render};
    use crate::node::Node;

    fn scope() -> Scope {
        Scope::new(
            [("title".to_string(), "machete".to_string())]
                .into_iter()
                .collect(),
        )
    }

    fn applied(src: &str) -> Result<String, SubstError> {
        let mut tree = parse(src).expect("valid source");
        substitute(&mut tree, &scope())?;
        Ok(render(&tree))
    }

    #[test]
    fn replaces_in_text() {
        assert_eq!(applied("<p>{title}</p>").unwrap(), "<p>machete</p>");
    }

    #[test]
    fn replaces_inside_an_attribute() {
        assert_eq!(
            applied(r#"<meta content="{title}">"#).unwrap(),
            r#"<meta content="machete">"#
        );
    }

    #[test]
    fn replaces_several_in_one_node() {
        assert_eq!(
            applied("<p>{title} y {title}</p>").unwrap(),
            "<p>machete y machete</p>"
        );
    }

    #[test]
    fn resolves_a_reference_inside_title_text() {
        assert_eq!(
            applied("<title>{title}</title>").unwrap(),
            "<title>machete</title>"
        );
    }

    #[test]
    fn leaves_javascript_alone() {
        let src = "<script>if (a<b) { x = \"{title}\"; }</script>";
        assert_eq!(applied(src).unwrap(), src);
    }

    #[test]
    fn leaves_css_alone() {
        let src = "<style>a { color: red; }</style>";
        assert_eq!(applied(src).unwrap(), src);
    }

    #[test]
    fn an_unknown_name_is_an_error() {
        assert!(matches!(
            applied("<p>{nope}</p>"),
            Err(SubstError::UndefinedVariable { .. })
        ));
    }

    #[test]
    fn an_unclosed_brace_is_literal_text() {
        assert_eq!(applied("<p>{title</p>").unwrap(), "<p>{title</p>");
    }

    #[test]
    fn an_empty_pair_is_left_alone() {
        assert_eq!(applied("<p>{}</p>").unwrap(), "<p>{}</p>");
    }

    #[test]
    fn touches_only_text_and_attributes() {
        let mut tree = parse("<!-- {title} --><p>{title}</p>").expect("valid");
        substitute(&mut tree, &scope()).expect("resolves");
        let comment = &tree.child_list().unwrap()[0];
        assert!(matches!(comment, Node::Comment(text) if text == " {title} "));
    }
}
