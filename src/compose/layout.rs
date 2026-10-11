//! Applying a layout to a page.
//!
//! A page declares the layout it wants with `<html type="layout"/>`, optionally
//! named. That directive is metadata, not markup: it is taken out of the page
//! and everything else is what gets handed to the layout.

use super::{find_slots, replace_marker};
use crate::error::LayoutError;
use crate::node::Node;

/// What a page asked for: the layout to use and the page body itself.
pub struct Extracted {
    /// The layout name, or `None` for the default one.
    pub layout: Option<String>,
    /// The page without the layout directive.
    pub page: Node,
}

/// Takes the layout directive out of a page and returns what remains.
///
/// A page without the directive uses the default layout. The directive itself
/// is removed, so it never reaches the output.
pub fn extract_layout(node: Node) -> Result<Extracted, LayoutError> {
    let mut page = Node::Document(vec![]);
    let mut layout = None;

    if let Node::Document(children) = node {
        let mut kept = Vec::with_capacity(children.len());

        for child in children {
            match child.as_element() {
                Some(element) if element.is_layout_directive() => {
                    if layout.is_some() {
                        return Err(LayoutError::DuplicateContentSlot);
                    }
                    layout = element.attr("name").map(str::to_string);
                }
                _ => kept.push(child),
            }
        }

        page = Node::Document(kept);
    }

    Ok(Extracted { layout, page })
}

/// Inserts a page into a layout's `<content/>` marker and removes the marker.
///
/// The page goes in first, before any component is expanded, because the marker
/// may sit inside a component invocation: expanding first would carry the marker
/// into the definition and leave nowhere to put the page.
pub fn apply_layout(layout: Node, page: Node) -> Result<Node, LayoutError> {
    let mut layout = layout;
    let slots = find_slots(&layout);

    match slots.len() {
        0 => Err(LayoutError::NoContentSlot),
        1 => {
            let children = match page {
                Node::Document(children) => children,
                other => vec![other],
            };
            replace_marker(&mut layout, &slots[0], children);
            Ok(layout)
        }
        _ => Err(LayoutError::DuplicateContentSlot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::render;

    fn parsed(src: &str) -> Node {
        crate::html::parse(src).expect("valid source")
    }

    #[test]
    fn takes_the_directive_out_of_the_page() {
        let extracted =
            extract_layout(parsed("<html type=\"layout\"/><main>x</main>")).expect("ok");
        assert_eq!(extracted.layout, None);
        assert_eq!(render(&extracted.page), "<main>x</main>");
    }

    #[test]
    fn reads_a_named_layout() {
        let extracted = extract_layout(parsed(
            "<html type=\"layout\" name=\"article\"/><main>x</main>",
        ))
        .expect("ok");
        assert_eq!(extracted.layout.as_deref(), Some("article"));
    }

    #[test]
    fn a_page_without_the_directive_is_left_alone() {
        let extracted = extract_layout(parsed("<main>x</main>")).expect("ok");
        assert_eq!(extracted.layout, None);
        assert_eq!(render(&extracted.page), "<main>x</main>");
    }

    #[test]
    fn a_plain_html_tag_is_not_a_directive() {
        let extracted =
            extract_layout(parsed(r#"<html lang="es"><body>x</body></html>"#)).expect("ok");
        assert_eq!(extracted.layout, None);
        assert!(render(&extracted.page).contains(r#"<html lang="es">"#));
    }

    #[test]
    fn inserts_the_page_where_the_marker_was() {
        let layout = parsed("<html><body><content/></body></html>");
        let merged = apply_layout(layout, parsed("<main>x</main>")).expect("ok");
        assert_eq!(render(&merged), "<html><body><main>x</main></body></html>");
    }

    #[test]
    fn the_marker_is_gone_afterwards() {
        let layout = parsed("<html><body><content/></body></html>");
        let merged = apply_layout(layout, parsed("<main>x</main>")).expect("ok");
        assert!(!render(&merged).contains("content"));
    }

    #[test]
    fn a_layout_without_a_marker_is_an_error() {
        let layout = parsed("<html><body></body></html>");
        assert!(matches!(
            apply_layout(layout, parsed("<main>x</main>")),
            Err(LayoutError::NoContentSlot)
        ));
    }

    #[test]
    fn two_markers_in_a_layout_are_ambiguous() {
        let layout = parsed("<html><body><content/><content/></body></html>");
        assert!(matches!(
            apply_layout(layout, parsed("<main>x</main>")),
            Err(LayoutError::DuplicateContentSlot)
        ));
    }

    #[test]
    fn a_marker_inside_an_invocation_still_works() {
        let layout = parsed(r#"<body><main type="component"><content/></main></body>"#);
        let merged = apply_layout(layout, parsed("<p>x</p>")).expect("ok");
        assert_eq!(
            render(&merged),
            r#"<body><main type="component"><p>x</p></main></body>"#
        );
    }
}
