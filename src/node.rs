//! Core document types.
//!
//! A parsed document is a tree of [`Node`] values. An [`Element`] carries a tag
//! name, its attributes and its children; the remaining variants cover the leaf
//! kinds that appear in HTML source.

/// Tag name used for the content marker, `<content/>`.
pub const CONTENT: &str = "content";

/// `type` value marking a tag as a component invocation.
pub const COMPONENT_TYPE: &str = "component";

/// `type` value on a `<script>` that declares variables instead of code.
pub const DATA_TYPE: &str = "data";

/// `type` value on an `<html>` tag that requests a layout.
pub const LAYOUT_TYPE: &str = "layout";

/// Reports whether a `type` value is one of Machete's directives.
///
/// These three constructs are self-closing. Every other tag follows the HTML
/// spec, where a trailing slash on a non-void element is ignored and the error
/// is that the element stays open.
pub fn is_directive(value: Option<&str>) -> bool {
    matches!(value, Some(COMPONENT_TYPE) | Some(LAYOUT_TYPE))
}

/// Elements whose content is parsed as raw text.
///
/// Inside these, a `<` does not start a tag, so the tokenizer captures their
/// contents verbatim. This is about markup only: whether their braces belong to
/// Machete is decided separately by [`is_code`].
pub const RAWTEXT_ELEMENTS: [&str; 4] = ["script", "style", "textarea", "title"];

/// Elements whose braces belong to another language.
///
/// A `{` inside a `<script>` or a `<style>` is JavaScript or CSS, not a
/// variable reference, so substitution skips them. `textarea` and `title` are
/// also raw text, but they hold plain prose, so their braces are Machete's.
pub const CODE_ELEMENTS: [&str; 2] = ["script", "style"];

/// Reports whether a tag's contents are code rather than prose.
pub fn is_code(tag: &str) -> bool {
    CODE_ELEMENTS.contains(&tag)
}

/// The fixed set of HTML void elements.
///
/// Void elements have no closing tag and can never have children. A trailing
/// slash on any of them is redundant but valid, e.g. `<br/>`.
pub const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// A single HTML attribute.
///
/// `value` is `None` for a valueless attribute such as `hidden`, which is
/// written without an equals sign in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub value: Option<String>,
}

impl Attr {
    /// Builds an attribute that carries a value, e.g. `class="a"`.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Some(value.into()),
        }
    }

    /// Builds an attribute with no value, e.g. `hidden`.
    pub fn bare(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
        }
    }
}

/// An HTML element with a tag name, attributes and children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
}

impl Element {
    /// Builds an empty element with the given tag name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Returns the value of an attribute, or `None` when it is absent or
    /// carries no value.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .and_then(|a| a.value.as_deref())
    }

    /// Reports whether this element is one of the HTML void elements.
    pub fn is_void(&self) -> bool {
        VOID_ELEMENTS.contains(&self.name.as_str())
    }

    /// Reports whether a self-closing slash is allowed on this element.
    ///
    /// True for the void elements and for Machete's own constructions: the
    /// `<content/>` marker, a `type="component"` invocation and a
    /// `type="layout"` directive. The tag name is irrelevant, so an invocation
    /// of `<002-mmenu/>` self-closes the same way `<header/>` does.
    pub fn accepts_self_closing(&self) -> bool {
        self.is_void() || self.is_content_marker() || is_directive(self.attr("type"))
    }

    /// Reports whether this element is a `<script type="data">` block, the
    /// construct that declares variables rather than carrying code.
    pub fn is_data_script(&self) -> bool {
        self.name == "script" && self.attr("type") == Some(DATA_TYPE)
    }

    /// Reports whether this element is an `<html type="layout">` directive.
    pub fn is_layout_directive(&self) -> bool {
        self.name == "html" && self.attr("type") == Some(LAYOUT_TYPE)
    }

    /// Reports whether this element is a `type="component"` invocation.
    ///
    /// A `<script type="data">` is not one: it declares variables for the
    /// invocation that contains it.
    pub fn is_invocation(&self) -> bool {
        !self.is_data_script() && self.attr("type") == Some(COMPONENT_TYPE)
    }

    /// Reports whether this element is the content marker, i.e. a bare
    /// `<content/>` with no attributes and no children.
    ///
    /// The marker is written with a trailing slash, but HTML ignores that slash
    /// on non-void elements. Machete treats it as self-closing, so it must be
    /// recognized by name rather than by syntax.
    pub fn is_content_marker(&self) -> bool {
        self.name == CONTENT && self.children.is_empty() && self.attrs.is_empty()
    }
}

/// A node in a parsed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// The root of a parsed fragment, holding its top-level nodes.
    Document(Vec<Node>),
    /// An element with attributes and children.
    Element(Element),
    /// Character data.
    Text(String),
    /// An HTML comment, stored without the `<!--` and `-->` delimiters.
    Comment(String),
    /// A doctype declaration, stored without the `<!DOCTYPE` and `>` markers.
    Doctype(String),
    /// Verbatim character data inside a raw text element.
    ///
    /// Used for the contents of `<script>`, `<style>`, `<textarea>` and
    /// `<title>`, where `<` and `{` are literal characters rather than markup.
    RawText { tag: String, content: String },
}

impl Node {
    /// Returns the inner element, or `None` for any other node kind.
    pub fn as_element(&self) -> Option<&Element> {
        match self {
            Node::Element(element) => Some(element),
            _ => None,
        }
    }

    /// Returns the inner element mutably, or `None` for any other node kind.
    pub fn as_element_mut(&mut self) -> Option<&mut Element> {
        match self {
            Node::Element(element) => Some(element),
            _ => None,
        }
    }

    /// Returns a handle to this node's children.
    ///
    /// Yields `None` for leaf nodes, which have no children to hand out.
    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Node::Document(children) | Node::Element(Element { children, .. }) => Some(children),
            _ => None,
        }
    }

    /// Returns this node's children, or `None` for a leaf node.
    pub fn child_list(&self) -> Option<&Vec<Node>> {
        match self {
            Node::Document(children) | Node::Element(Element { children, .. }) => Some(children),
            _ => None,
        }
    }
}
