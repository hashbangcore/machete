//! Turning component invocations into markup.
//!
//! [`expand`] walks a document and replaces every `type="component"` tag with
//! the definition it names, resolving variables and inserting content along the
//! way. What it returns carries no Machete markup: the output is plain HTML.

mod expand;
mod layout;

pub use expand::{Defs, expand};
pub use layout::{Extracted, apply_layout, extract_layout};

use crate::node::{Element, Node};

/// Replaces the content marker at `path` with `replacement`.
///
/// The marker is removed rather than filled. A marker that reached the output
/// would put `<content/>` in the served HTML, which is not standard markup.
pub(crate) fn replace_marker(tree: &mut Node, path: &[usize], replacement: Vec<Node>) {
    let (parent_path, last) = path.split_at(path.len() - 1);
    let parent = node_at(tree, parent_path).expect("path came from this tree");
    let siblings = parent.children_mut().expect("parent has children");
    siblings.splice(last[0]..=last[0], replacement);
}

/// Removes every content marker from the tree.
///
/// Used when a definition carries a `<content/>` but nothing was inserted at it:
/// the marker has nothing to mark, so it is dropped instead of rendered.
pub(crate) fn strip_markers(node: &mut Node) {
    let Some(children) = node.children_mut() else {
        return;
    };

    children.retain(|child| !child.as_element().is_some_and(Element::is_content_marker));

    for child in children.iter_mut() {
        strip_markers(child);
    }
}

/// Collects the path to every `<content/>` marker in the tree.
///
/// Each path is a sequence of child indices from the root, so a marker can be
/// located again in a mutable tree.
pub(crate) fn find_slots(node: &Node) -> Vec<Vec<usize>> {
    let mut found = Vec::new();
    collect_slots(node, &mut Vec::new(), &mut found);
    found
}

/// Walks the tree recording the current path and storing it at each marker.
fn collect_slots(node: &Node, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    let children = match node {
        Node::Document(children) | Node::Element(Element { children, .. }) => children,
        _ => return,
    };

    for (index, child) in children.iter().enumerate() {
        path.push(index);
        if child.as_element().is_some_and(Element::is_content_marker) {
            out.push(path.clone());
        } else {
            collect_slots(child, path, out);
        }
        path.pop();
    }
}

/// Follows a path of child indices down to a node.
pub(crate) fn node_at<'a>(node: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let mut current = node;
    for index in path {
        current = current.children_mut()?.get_mut(*index)?;
    }
    Some(current)
}
