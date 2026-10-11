//! Error types for every stage of the pipeline.
//!
//! Each type maps to one stage so a caller can tell where a failure came from:
//! [`TokenizeError`] and [`TreeError`] while parsing, [`ScriptError`] while
//! reading variable declarations, [`SubstError`] while substituting variables,
//! and [`ExpandError`] while composing components.

use std::fmt;

/// Raised when the source cannot be split into tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenizeError {
    /// A `<` was never followed by a closing `>`.
    UnterminatedTag { offset: usize },
    /// A `<!--` was never followed by `-->`.
    UnterminatedComment { offset: usize },
    /// A quoted attribute value was never closed.
    UnterminatedAttribute { name: String },
    /// A raw text element such as `<script>` was never closed.
    UnclosedRawText { tag: String },
    /// A closing tag `</>` had no name.
    EmptyTagName { offset: usize },
}

/// Raised when tokens cannot be assembled into a well-formed tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    /// An end tag did not match the innermost open element.
    MismatchedTag {
        expected: String,
        found: String,
        offset: usize,
    },
    /// Elements were left open when the input ran out.
    UnclosedTag { name: String, offset: usize },
}

/// Raised when parsing fails at either the tokenizer or the tree builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Tokenize(TokenizeError),
    Tree(TreeError),
}

/// Raised when a `<script type="component">` body cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// A declaration had no `=`.
    MissingEquals { name: String, line: usize },
    /// A variable name contained characters that are not allowed.
    InvalidName { text: String, line: usize },
    /// A quoted value was never closed.
    UnterminatedString { name: String, line: usize },
    /// A declaration had `=` but no value after it.
    MissingValue { name: String, line: usize },
    /// A value did not begin with a quote.
    UnexpectedToken { text: String, line: usize },
}

/// Raised when variable bindings are built or combined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeError {
    /// The same variable name was supplied more than once.
    DuplicateVariable { name: String },
    /// A variable name was not a valid identifier.
    InvalidName { name: String },
}

/// Raised when a `{variable}` reference cannot be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubstError {
    /// The variable has no value in any scope.
    UndefinedVariable { name: String },
}

/// Raised while composing components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandError {
    /// An invocation names a component with no definition.
    UnknownComponent {
        name: String,
    },
    /// A component received content but its definition has no `<content/>`.
    MissingContentSlot {
        name: String,
    },
    /// A definition declares `<content/>` more than once.
    DuplicateContentSlot {
        name: String,
    },
    /// A component carries more than one `<script type="component">`.
    DuplicateComponentScript {
        name: String,
    },
    /// An invocation had an empty tag name.
    EmptyComponentName {
        offset: usize,
    },
    /// A failure that happened while expanding a nested component.
    Nested {
        component: String,
        source: Box<ExpandError>,
    },
    Scope(ScopeError),
    Subst(SubstError),
    Script(ScriptError),
}

impl fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedTag { offset } => {
                write!(f, "unclosed tag at offset {offset}")
            }
            Self::UnterminatedComment { offset } => {
                write!(f, "unclosed comment at offset {offset}")
            }
            Self::UnterminatedAttribute { name } => {
                write!(f, "unclosed quote in attribute '{name}'")
            }
            Self::UnclosedRawText { tag } => write!(f, "unclosed <{tag}>"),
            Self::EmptyTagName { offset } => write!(f, "empty tag name at offset {offset}"),
        }
    }
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MismatchedTag {
                expected,
                found,
                offset,
            } => write!(
                f,
                "expected </{expected}> but found </{found}> at offset {offset}"
            ),
            Self::UnclosedTag { name, offset } => {
                write!(f, "<{name}> opened at offset {offset} was never closed")
            }
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tokenize(error) => write!(f, "{error}"),
            Self::Tree(error) => write!(f, "{error}"),
        }
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEquals { name, line } => {
                write!(f, "missing '=' in declaration of '{name}' on line {line}")
            }
            Self::InvalidName { text, line } => {
                write!(f, "invalid variable name '{text}' on line {line}")
            }
            Self::UnterminatedString { name, line } => {
                write!(f, "unclosed string for '{name}' on line {line}")
            }
            Self::MissingValue { name, line } => {
                write!(f, "missing value for '{name}' on line {line}")
            }
            Self::UnexpectedToken { text, line } => {
                write!(f, "unexpected token '{text}' on line {line}")
            }
        }
    }
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateVariable { name } => {
                write!(f, "variable '{name}' is defined more than once")
            }
            Self::InvalidName { name } => write!(f, "invalid variable name '{name}'"),
        }
    }
}

impl fmt::Display for SubstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedVariable { name } => write!(f, "variable '{name}' is not defined"),
        }
    }
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownComponent { name } => {
                write!(f, "no definition for component '{name}'")
            }
            Self::MissingContentSlot { name } => write!(
                f,
                "component '{name}' received content but its definition has no <content/>"
            ),
            Self::DuplicateContentSlot { name } => {
                write!(
                    f,
                    "definition of '{name}' declares <content/> more than once"
                )
            }
            Self::DuplicateComponentScript { name } => {
                write!(
                    f,
                    "component '{name}' has more than one <script type=\"component\">"
                )
            }
            Self::EmptyComponentName { offset } => {
                write!(f, "component with empty name at offset {offset}")
            }
            Self::Nested { component, source } => {
                write!(f, "in component '{component}': {source}")
            }
            Self::Scope(error) => write!(f, "{error}"),
            Self::Subst(error) => write!(f, "{error}"),
            Self::Script(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TokenizeError {}
impl std::error::Error for TreeError {}
impl std::error::Error for ParseError {}
impl std::error::Error for ScriptError {}
impl std::error::Error for ScopeError {}
impl std::error::Error for SubstError {}
impl std::error::Error for ExpandError {}

impl From<TokenizeError> for ParseError {
    fn from(error: TokenizeError) -> Self {
        Self::Tokenize(error)
    }
}

impl From<TreeError> for ParseError {
    fn from(error: TreeError) -> Self {
        Self::Tree(error)
    }
}
