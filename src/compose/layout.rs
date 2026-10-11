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
