//! Core document types.
//!
//! A parsed document is a tree of [`Node`] values. An [`Element`] carries a tag
//! name, its attributes and its children; the remaining variants cover the leaf
//! kinds that appear in HTML source.

/// Tag name used for the content marker, `<content/>`.
pub const CONTENT: &str = "content";

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

    /// Returns a mutable handle to this node's children.
    ///
    /// Yields `None` for leaf nodes, which have no children to hand out.
    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Node::Document(children) | Node::Element(Element { children, .. }) => Some(children),
            _ => None,
        }
    }
}
