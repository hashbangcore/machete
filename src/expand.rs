//! Expands component invocations into real markup.
//!
//! An invocation is any tag carrying `type="component"`. The tag name selects a
//! definition, and the invocation's attributes plus its
//! `<script type="component">` block supply the variable values that the
//! definition refers to with `{name}`.

use crate::error::{ExpandError, ScopeError, ScriptError, SubstError};
use crate::node::{Element, Node};
use crate::scope::{Bindings, Scope};
use crate::script::parse_bindings;
use crate::substitute::substitute;
use std::collections::HashMap;

/// The attribute value that marks a tag as a component invocation.
pub const COMPONENT_TYPE: &str = "component";

/// Component definitions, keyed by the tag name that invokes them.
pub type Defs = HashMap<String, Node>;

/// Expands every invocation found anywhere in `node`.
///
/// Recurses into definitions, so a component may invoke other components. The
/// returned tree has no invocations left in it.
pub fn expand(node: &Node, defs: &Defs, globals: &Scope) -> Result<Node, ExpandError> {
    expand_node(node, defs, globals)
}

/// Expands a single node and, in turn, its children.
fn expand_node(node: &Node, defs: &Defs, outer: &Scope) -> Result<Node, ExpandError> {
    match node {
        Node::Element(element) if is_invocation(element) => expand_invocation(element, defs, outer),

        Node::Element(element) => {
            let mut element = element.clone();
            element.children = expand_children(&element.children, defs, outer)?;
            Ok(Node::Element(element))
        }

        Node::Document(children) => Ok(Node::Document(expand_children(children, defs, outer)?)),

        other => Ok(other.clone()),
    }
}

/// Expands each child in turn.
fn expand_children(
    children: &[Node],
    defs: &Defs,
    outer: &Scope,
) -> Result<Vec<Node>, ExpandError> {
    children
        .iter()
        .map(|child| expand_node(child, defs, outer))
        .collect()
}

/// Reports whether a tag is a component invocation.
///
/// The `<script type="component">` block is not an invocation: it is a variable
/// declaration, and it is handled while reading the invocation's values.
fn is_invocation(element: &Element) -> bool {
    element.name != "script" && element.attr("type") == Some(COMPONENT_TYPE)
}

/// Expands one invocation into a copy of its definition.
///
/// The order matters: read the values, expand the definition's own invocations
/// first, then substitute variables, and only then insert the content, so that
/// the content is rendered inside the component's scope.
fn expand_invocation(element: &Element, defs: &Defs, outer: &Scope) -> Result<Node, ExpandError> {
    let name = element.name.clone();

    if name.is_empty() {
        return Err(ExpandError::EmptyComponentName { offset: 0 });
    }

    let Some(definition) = defs.get(&name) else {
        return Err(ExpandError::UnknownComponent { name });
    };

    let (from_attrs, from_script, content) = read_invocation(element)?;

    let locals = Scope::merge(&[from_attrs, from_script]).map_err(|error| nested(&name, error))?;
    let scope = outer.child(locals);

    let mut expanded =
        expand_node(definition, defs, &scope).map_err(|error| nested(&name, error))?;

    substitute(&mut expanded, &scope).map_err(|error| nested(&name, error))?;

    if content.is_empty() {
        return Ok(expanded);
    }

    insert_content(&mut expanded, content, &name)?;

    Ok(expanded)
}

/// Splits an invocation's contents into three groups: values from attributes,
/// values from the script block, and the HTML to insert at `<content/>`.
///
/// Every attribute other than `type` becomes a variable and is consumed, so it
/// does not reach the output. At most one component script is allowed, since a
/// second one would silently discard the first.
fn read_invocation(element: &Element) -> Result<(Bindings, Bindings, Vec<Node>), ExpandError> {
    let mut from_attrs = Bindings::new();
    let mut from_script: Option<Bindings> = None;
    let mut content = Vec::new();

    for attr in &element.attrs {
        if attr.name == "type" {
            continue;
        }
        let Some(value) = attr.value.as_deref() else {
            continue;
        };
        from_attrs.insert(attr.name.clone(), value.to_string());
    }

    for child in &element.children {
        match child {
            Node::Element(Element { name, children, .. })
                if name == "script" && children.iter().any(is_component_script_body) =>
            {
                if from_script.is_some() {
                    return Err(ExpandError::DuplicateComponentScript { name: name.clone() });
                }
                let body = children.iter().find_map(|c| match c {
                    Node::RawText { content, .. } => Some(content.clone()),
                    _ => None,
                });
                let Some(body) = body else {
                    return Err(ExpandError::DuplicateComponentScript { name: name.clone() });
                };
                from_script = Some(parse_bindings(&body).map_err(|e| nested(&element.name, e))?);
            }

            other => content.push(other.clone()),
        }
    }

    Ok((from_attrs, from_script.unwrap_or_default(), content))
}

/// Reports whether a script body is the component declaration form.
///
/// A plain `<script>` carries real JavaScript and must be preserved verbatim,
/// so only `<script type="component">` is read as variable declarations.
fn is_component_script_body(node: &Node) -> bool {
    node.as_element()
        .is_some_and(|e| e.attr("type") == Some(COMPONENT_TYPE))
}

/// Places `content` into the definition's `<content/>` marker.
///
/// Fails when the definition has no marker, because the content would be lost,
/// and when it has more than one, because the target would be ambiguous.
fn insert_content(node: &mut Node, content: Vec<Node>, component: &str) -> Result<(), ExpandError> {
    let slots = find_slots(node);

    match slots.len() {
        0 => Err(ExpandError::MissingContentSlot {
            name: component.to_string(),
        }),

        1 => {
            let target = node_at(node, &slots[0]).expect("path came from this tree");
            if let Some(element) = target.as_element_mut() {
                element.children = content;
            }
            Ok(())
        }

        _ => Err(ExpandError::DuplicateContentSlot {
            name: component.to_string(),
        }),
    }
}

/// Collects the path to every `<content/>` marker in the tree.
///
/// Each path is a sequence of child indices from the root, so a marker can be
/// located again in a mutable tree.
fn find_slots(node: &Node) -> Vec<Vec<usize>> {
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
fn node_at<'a>(node: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let mut current = node;
    for index in path {
        current = current.children_mut()?.get_mut(*index)?;
    }
    Some(current)
}

/// Wraps an error with the component it came from.
fn nested(component: &str, source: impl Into<ExpandError>) -> ExpandError {
    ExpandError::Nested {
        component: component.to_string(),
        source: Box::new(source.into()),
    }
}

impl From<ScopeError> for ExpandError {
    fn from(error: ScopeError) -> Self {
        ExpandError::Scope(error)
    }
}

impl From<SubstError> for ExpandError {
    fn from(error: SubstError) -> Self {
        ExpandError::Subst(error)
    }
}

impl From<ScriptError> for ExpandError {
    fn from(error: ScriptError) -> Self {
        ExpandError::Script(error)
    }
}
