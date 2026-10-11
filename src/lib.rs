//! Machete composes HTML pages from components, layouts and variables.
//!
//! The pipeline is a sequence of pure functions, each taking one step's output
//! as the next step's input:
//!
//! 1. [`html::tokenize`] splits source into tokens.
//! 2. [`html::build`] assembles those tokens into a document tree.
//! 3. [`compose::expand`] replaces component invocations with their definitions.
//! 4. [`vars::substitute`] resolves `{variable}` references.
//! 5. [`html::render`] writes the tree back out as HTML.
//!
//! No stage reads from disk or writes to it, so each one is testable on its own.
//! [`html::parse`] combines the first two for callers that do not need the
//! intermediate tokens.
//!
//! # Layout
//!
//! - [`node`] and [`error`] sit at the top level because every layer uses them:
//!   they are the shared vocabulary rather than part of any one layer.
//! - [`html`] parses and renders documents.
//! - [`vars`] resolves variable values.
//! - [`compose`] combines components.
//! - [`cli`] defines the command line interface.

pub mod cli;
pub mod compose;
pub mod error;
pub mod html;
pub mod node;
pub mod vars;

pub use cli::{Cli, CliError, Command, run};

pub use compose::{Defs, expand};
pub use error::{
    ExpandError, ParseError, ScopeError, ScriptError, SubstError, TokenizeError, TreeError,
};
pub use html::{parse, render, tokenize};
pub use node::{Attr, Element, Node};
pub use vars::{Bindings, Scope, substitute};
