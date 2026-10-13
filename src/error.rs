//! Error types for every stage of the pipeline.
//!
//! Each type maps to one stage so a caller can tell where a failure came from:
//! [`TokenizeError`] and [`TreeError`] while parsing, [`ScriptError`] while
//! reading variable declarations, [`SubstError`] while substituting variables,
//! [`ExpandError`] and [`LayoutError`] while composing, and [`IoError`] while
//! touching the filesystem. [`BuildError`] is the single type a caller at the
//! top of the pipeline has to handle.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// The directory a site keeps its pages in, named in the messages above.
const TEMPLATES: &str = "templates";

/// The file that identifies a directory as a machete project.
const CONFIG_FILE: &str = "machete.toml";

/// Raised when the source cannot be split into tokens.
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
pub enum ParseError {
    Tokenize(TokenizeError),
    Tree(TreeError),
}

/// Raised when a `<script type="data">` body cannot be read.
#[derive(Debug)]
pub enum ScriptError {
    /// A declaration had no `=`.
    MissingEquals { text: String, line: usize },
    /// A variable name contained characters that are not allowed.
    InvalidName { text: String, line: usize },
    /// A quoted value was never closed.
    UnterminatedString { name: String, line: usize },
    /// A declaration had `=` but no value after it.
    MissingValue { name: String, line: usize },
}

/// Raised when variable bindings are built or combined.
#[derive(Debug)]
pub enum ScopeError {
    /// The same variable name was supplied more than once.
    DuplicateVariable { name: String },
}

/// Raised when a `{variable}` reference cannot be resolved.
#[derive(Debug)]
pub enum SubstError {
    /// The variable has no value in any scope.
    UndefinedVariable { name: String },
}

/// Raised while expanding components.
#[derive(Debug)]
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
    /// A component carries more than one `<script type="data">`.
    DuplicateDataScript {
        name: String,
    },
    /// A `<script type="data">` appears outside any component invocation.
    LooseDataScript {
        offset: usize,
    },
    /// Machete markup survived into the tree about to be written.
    LeftoverMarkup {
        found: String,
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

/// Raised while applying a layout to a page.
#[derive(Debug)]
pub enum LayoutError {
    /// The layout named by the page does not exist.
    NotFound { name: String },
    /// The layout has no `<content/>` to insert the page into.
    NoContentSlot,
    /// The layout declares `<content/>` more than once.
    DuplicateContentSlot,
}

/// Raised by a filesystem operation.
#[derive(Debug)]
pub struct IoError {
    /// What was being attempted, e.g. "read" or "create directory".
    pub action: &'static str,
    /// The path the operation targeted.
    pub path: PathBuf,
    /// The underlying operating system error.
    pub source: io::Error,
}

/// Raised when a filesystem operation cannot be completed.
#[derive(Debug)]
pub enum BuildError {
    /// The directory has no `machete.toml`, so it is not a machete project.
    NoConfig { root: PathBuf },
    /// The input directory is not a site: it holds no `templates/`.
    NotASite { root: PathBuf, expected: PathBuf },
    /// The site has templates but no pages to build.
    NoPages { templates: PathBuf },
    /// The `input:output` argument could not be read.
    InvalidTarget { found: String, reason: &'static str },
    /// `[data]` nests further than one level, as in `[data.blog.post]`.
    NestedData { name: String },
    /// A `[build]` key was given a value of the wrong kind.
    InvalidBuildValue { key: &'static str, found: String },
    /// The output directory would hold the site's own sources.
    OutputInsideSite { output: PathBuf, templates: PathBuf },
    /// A filesystem operation failed.
    Io(IoError),
    /// `machete.toml` could not be parsed.
    Config {
        path: PathBuf,
        source: toml::de::Error,
    },
    /// A template could not be parsed.
    Parse { path: PathBuf, source: ParseError },
    /// A template could not be expanded.
    Expand { path: PathBuf, source: ExpandError },
    /// A layout could not be applied.
    Layout { path: PathBuf, source: LayoutError },
}

impl fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedTag { offset } => write!(f, "unclosed tag at offset {offset}"),
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
            Self::MissingEquals { text, line } => {
                write!(f, "missing '=' in '{text}' on line {line}")
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
        }
    }
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateVariable { name } => {
                write!(f, "variable '{name}' is defined more than once")
            }
        }
    }
}

impl fmt::Display for SubstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedVariable { name } => write!(
                f,
                "variable '{name}' is not defined. Set it in {CONFIG_FILE}, \
                 or pass it to the component as an attribute"
            ),
        }
    }
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownComponent { name } => write!(f, "no definition for component '{name}'"),
            Self::MissingContentSlot { name } => write!(
                f,
                "component '{name}' received content but its definition has no <content/>"
            ),
            Self::DuplicateContentSlot { name } => write!(
                f,
                "definition of '{name}' declares <content/> more than once"
            ),
            Self::DuplicateDataScript { name } => {
                write!(
                    f,
                    "component '{name}' has more than one <script type=\"data\">"
                )
            }
            Self::LooseDataScript { .. } => {
                write!(f, "<script type=\"data\"> must appear inside a component")
            }
            Self::LeftoverMarkup { found } => {
                write!(f, "machete markup survived into the output: {found}")
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

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { name } => write!(f, "no layout named '{name}'"),
            Self::NoContentSlot => write!(f, "the layout has no <content/>"),
            Self::DuplicateContentSlot => {
                write!(f, "the layout declares <content/> more than once")
            }
        }
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to {} {}: {}",
            self.action,
            self.path.display(),
            self.source
        )
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfig { root } => write!(
                f,
                "{} has no {CONFIG_FILE}, so it is not a machete project. \
                 Run `machete init` here to create one, or point the input at \
                 a directory that has one",
                root.display()
            ),
            Self::NotASite { root, expected } => write!(
                f,
                "{} is not a site: it has no {}. Build from the directory holding {}",
                root.display(),
                expected
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default(),
                TEMPLATES
            ),
            Self::NoPages { templates } => {
                write!(f, "no pages to build in {}", templates.display())
            }
            Self::NestedData { name } => write!(
                f,
                "[data.{name}] nests a level too deep. \
                 Use [data] or [data.<layout>], where <layout> is one name"
            ),
            Self::InvalidBuildValue { key, found } => {
                write!(f, "'{key}' in [build] expects true or false, found {found}")
            }
            Self::OutputInsideSite { output, templates } => write!(
                f,
                "{} would hold {} inside it. The output cannot be the site \
                 directory or anything above it, or the build would delete the \
                 templates it is reading from",
                output.display(),
                templates.display()
            ),
            Self::InvalidTarget { found, reason } => write!(
                f,
                "cannot read '{found}': {reason}. Expected input:output, \
                 for example scaffold:public"
            ),
            Self::Io(error) => write!(f, "{error}"),
            Self::Config { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            }
            Self::Parse { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Expand { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Layout { path, source } => write!(f, "{}: {source}", path.display()),
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
impl std::error::Error for LayoutError {}
impl std::error::Error for BuildError {}

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

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl IoError {
    /// Wraps an I/O failure together with what was being attempted and where.
    pub fn new(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self {
            action,
            path: path.into(),
            source,
        }
    }
}
