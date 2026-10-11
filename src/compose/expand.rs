//! Expands component invocations into real markup.
//!
//! An invocation is any tag carrying `type="component"`. The tag name selects a
//! definition, and the invocation's attributes plus its
//! `<script type="data">` block supply the variable values that the definition
//! refers to with `{name}`.

use super::{find_slots, replace_marker, strip_markers};
use crate::error::{ExpandError, ScopeError, ScriptError, SubstError};
use crate::node::{Element, Node};
use crate::vars::{Bindings, Scope, parse_bindings, substitute, substitute_text};
use std::collections::HashMap;

/// Component definitions, keyed by the tag name that invokes them.
pub type Defs = HashMap<String, Node>;

/// Expands every invocation found anywhere in `node`.
///
/// Recurses into definitions, so a component may invoke other components. The
/// returned tree carries no invocations and no content markers: everything left
/// is plain markup ready to be rendered.
pub fn expand(node: &Node, defs: &Defs, globals: &Scope) -> Result<Node, ExpandError> {
    expand_node(node, defs, globals)
}

/// Expands a single node and, in turn, its children.
fn expand_node(node: &Node, defs: &Defs, outer: &Scope) -> Result<Node, ExpandError> {
    match node {
        Node::Element(element) if element.is_invocation() => {
            expand_invocation(element, defs, outer)
        }

        // A data block is only meaningful inside an invocation. Reaching one
        // here means it was written somewhere it cannot be consumed, and it
        // would otherwise pass straight through into the output.
        Node::Element(element) if element.is_data_script() => {
            Err(ExpandError::LooseDataScript { offset: 0 })
        }

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

/// Expands one invocation into a copy of its definition.
///
/// The order matters. Attribute values are resolved against the enclosing
/// scope first, so a value written as a variable reference is passed on
/// resolved. Then the definition's own invocations are expanded, the content is
/// placed, and only then are variables substituted, so that inserted content is
/// rendered inside the component's scope.
fn expand_invocation(element: &Element, defs: &Defs, outer: &Scope) -> Result<Node, ExpandError> {
    let name = element.name.clone();

    let Some(definition) = defs.get(&name) else {
        return Err(ExpandError::UnknownComponent { name });
    };

    let (from_attrs, from_script, content) = read_invocation(element)?;

    let mut from_attrs = from_attrs;
    for (name, value) in std::mem::take(&mut from_attrs) {
        let resolved = substitute_text(&value, outer)
            .map_err(|e| nested(&element.name, ExpandError::Subst(e)))?;
        from_attrs.insert(name, resolved);
    }

    let locals = Scope::merge(&[from_attrs, from_script]).map_err(|e| nested(&name, e))?;
    let scope = outer.child(locals);

    let mut expanded = expand_node(definition, defs, &scope).map_err(|e| nested(&name, e))?;

    if content.is_empty() {
        strip_markers(&mut expanded);
    } else {
        insert_content(&mut expanded, content, &name)?;
    }

    substitute(&mut expanded, &scope).map_err(|e| nested(&name, e))?;

    Ok(expanded)
}

/// Splits an invocation's contents into three groups: values from attributes,
/// values from the data block, and the HTML to insert at `<content/>`.
///
/// Every attribute other than `type` becomes a variable and is consumed, so it
/// does not reach the output. At most one data block is allowed, since a
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
            Node::Element(inner) if inner.is_data_script() => {
                if from_script.is_some() {
                    return Err(ExpandError::DuplicateDataScript {
                        name: element.name.clone(),
                    });
                }
                let body = inner
                    .children
                    .iter()
                    .find_map(|c| match c {
                        Node::RawText { content, .. } => Some(content.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                from_script = Some(parse_bindings(&body).map_err(|e| nested(&element.name, e))?);
            }

            other => {
                // Indentation between the invocation and its data block is
                // formatting, not content. Counting it would make a component
                // look like it received something when it received nothing.
                let is_blank = matches!(other, Node::Text(text) if text.trim().is_empty());
                if !is_blank {
                    content.push(other.clone());
                }
            }
        }
    }

    Ok((from_attrs, from_script.unwrap_or_default(), content))
}

/// Places `content` at the definition's `<content/>` marker and removes the
/// marker, so it never reaches the output.
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
            replace_marker(node, &slots[0], content);
            Ok(())
        }

        _ => Err(ExpandError::DuplicateContentSlot {
            name: component.to_string(),
        }),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::{parse, render};

    fn defs(pairs: &[(&str, &str)]) -> Defs {
        pairs
            .iter()
            .map(|(name, src)| (name.to_string(), parse(src).expect("valid definition")))
            .collect()
    }

    fn scope() -> Scope {
        Scope::new(
            [("title".to_string(), "machete".to_string())]
                .into_iter()
                .collect(),
        )
    }

    fn run(src: &str, defs: &Defs) -> Result<String, ExpandError> {
        expand(&parse(src).expect("valid source"), defs, &scope()).map(|t| render(&t))
    }

    #[test]
    fn expands_a_component() {
        let defs = defs(&[("aside", "<aside>{title}</aside>")]);
        assert_eq!(
            run(r#"<aside type="component" title="hola"></aside>"#, &defs).unwrap(),
            "<aside>hola</aside>"
        );
    }

    #[test]
    fn the_type_attribute_never_reaches_the_output() {
        let defs = defs(&[("aside", "<aside>x</aside>")]);
        let out = run(r#"<aside type="component"></aside>"#, &defs).unwrap();
        assert!(!out.contains("type="), "output was {out}");
    }

    #[test]
    fn an_attribute_is_consumed_as_a_variable() {
        let defs = defs(&[("aside", "<aside>{url}</aside>")]);
        assert_eq!(
            run(r#"<aside type="component" url="/a"></aside>"#, &defs).unwrap(),
            "<aside>/a</aside>"
        );
    }

    #[test]
    fn reads_variables_from_a_data_block() {
        let defs = defs(&[("aside", "<aside>{title}</aside>")]);
        let src = r#"<aside type="component"><script type="data">title = "hola";</script></aside>"#;
        assert_eq!(run(src, &defs).unwrap(), "<aside>hola</aside>");
    }

    #[test]
    fn a_data_block_never_reaches_the_output() {
        let defs = defs(&[("aside", "<aside>x</aside>")]);
        let src = r#"<aside type="component"><script type="data">a = "1";</script></aside>"#;
        let out = run(src, &defs).unwrap();
        assert!(!out.contains("script"), "output was {out}");
    }

    #[test]
    fn an_attribute_value_is_resolved_before_it_becomes_a_local() {
        let defs = defs(&[("head", "<head><title>{title}</title></head>")]);
        assert_eq!(
            run(r#"<head type="component" title="{title}"/>"#, &defs).unwrap(),
            "<head><title>machete</title></head>"
        );
    }

    #[test]
    fn content_replaces_the_marker() {
        let defs = defs(&[("figure", "<figure><content/></figure>")]);
        assert_eq!(
            run(r#"<figure type="component"><p>x</p></figure>"#, &defs).unwrap(),
            "<figure><p>x</p></figure>"
        );
    }

    #[test]
    fn content_inside_the_marker_keeps_its_own_markup() {
        let defs = defs(&[("figure", "<figure><content/></figure>")]);
        assert_eq!(
            run(
                r#"<figure type="component"><div><b>x</b></div></figure>"#,
                &defs
            )
            .unwrap(),
            "<figure><div><b>x</b></div></figure>"
        );
    }

    #[test]
    fn content_is_rendered_inside_the_component_scope() {
        let defs = defs(&[("figure", "<figure><content/></figure>")]);
        let src = r#"<figure type="component" title="propio"><p>{title}</p></figure>"#;
        assert_eq!(run(src, &defs).unwrap(), "<figure><p>propio</p></figure>");
    }

    #[test]
    fn an_empty_marker_is_removed() {
        let defs = defs(&[("figure", "<figure><content/></figure>")]);
        assert_eq!(
            run(r#"<figure type="component"/>"#, &defs).unwrap(),
            "<figure></figure>"
        );
    }

    #[test]
    fn a_definition_without_a_marker_rejects_content() {
        let defs = defs(&[("aside", "<aside>x</aside>")]);
        assert!(matches!(
            run(r#"<aside type="component"><p>x</p></aside>"#, &defs),
            Err(ExpandError::MissingContentSlot { .. })
        ));
    }

    #[test]
    fn a_definition_without_a_marker_accepts_no_content() {
        let defs = defs(&[("aside", "<aside>x</aside>")]);
        let src = r#"<aside type="component"><script type="data">a = "1";</script></aside>"#;
        assert_eq!(run(src, &defs).unwrap(), "<aside>x</aside>");
    }

    #[test]
    fn two_markers_are_ambiguous() {
        let defs = defs(&[("figure", "<figure><content/><content/></figure>")]);
        assert!(matches!(
            run(r#"<figure type="component"><p>x</p></figure>"#, &defs),
            Err(ExpandError::DuplicateContentSlot { .. })
        ));
    }

    #[test]
    fn an_unknown_component_is_an_error() {
        assert!(matches!(
            run(r#"<nope type="component"/>"#, &Defs::new()),
            Err(ExpandError::UnknownComponent { .. })
        ));
    }

    #[test]
    fn a_name_that_is_not_a_standard_tag_still_works() {
        let defs = defs(&[("002-mmenu", "<nav>menu</nav>")]);
        assert_eq!(
            run(r#"<002-mmenu type="component"/>"#, &defs).unwrap(),
            "<nav>menu</nav>"
        );
    }

    #[test]
    fn components_nest() {
        let defs = defs(&[
            ("outer", "<div><inner type=\"component\"/></div>"),
            ("inner", "<span>in</span>"),
        ]);
        assert_eq!(
            run(r#"<outer type="component"/>"#, &defs).unwrap(),
            "<div><span>in</span></div>"
        );
    }

    #[test]
    fn a_name_supplied_twice_is_an_error() {
        let defs = defs(&[("aside", "<aside>x</aside>")]);
        let src = r#"<aside type="component" a="1"><script type="data">a = "2";</script></aside>"#;
        assert!(matches!(run(src, &defs), Err(ExpandError::Nested { .. })));
    }

    #[test]
    fn two_data_blocks_are_an_error() {
        let defs = defs(&[("aside", "<aside>x</aside>")]);
        let src = r#"<aside type="component"><script type="data">a="1";</script><script type="data">b="2";</script></aside>"#;
        assert!(matches!(
            run(src, &defs),
            Err(ExpandError::DuplicateDataScript { .. })
        ));
    }

    #[test]
    fn a_data_block_outside_a_component_is_an_error() {
        assert!(matches!(
            run(
                r#"<main><script type="data">a = "1";</script></main>"#,
                &Defs::new()
            ),
            Err(ExpandError::LooseDataScript { .. })
        ));
    }

    #[test]
    fn an_undefined_variable_is_an_error() {
        let defs = defs(&[("aside", "<aside>{nope}</aside>")]);
        assert!(matches!(
            run(r#"<aside type="component"/>"#, &defs),
            Err(ExpandError::Nested { .. })
        ));
    }

    #[test]
    fn whitespace_around_a_data_block_is_not_content() {
        let defs = defs(&[("aside", "<aside>x</aside>")]);
        let src = "<aside type=\"component\">\n  <script type=\"data\">\n    a = \"1\";\n  </script>\n</aside>";
        assert_eq!(run(src, &defs).unwrap(), "<aside>x</aside>");
    }

    #[test]
    fn an_ordinary_script_is_content_and_survives_verbatim() {
        let defs = defs(&[("aside", "<aside><content/></aside>")]);
        let src = r#"<aside type="component"><script>if (a<b) { x = "{y}"; }</script></aside>"#;
        assert_eq!(
            run(src, &defs).unwrap(),
            r#"<aside><script>if (a<b) { x = "{y}"; }</script></aside>"#
        );
    }

    #[test]
    fn an_ordinary_script_is_not_read_as_variables() {
        let defs = defs(&[("aside", "<aside><content/></aside>")]);
        // If the block were read as declarations, `a` would be undefined.
        let src = r#"<aside type="component"><script>var a = 1;</script></aside>"#;
        assert!(run(src, &defs).is_ok());
    }
}
