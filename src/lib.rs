//! Machete composes HTML pages from components, layouts and variables.
//!
//! The pipeline is a sequence of pure functions, each taking one step's output
//! as the next step's input:
//!
//! 1. [`token::tokenize`] splits source into tokens.
//! 2. [`tree::build`] assembles those tokens into a document tree.
//! 3. [`expand::expand`] replaces component invocations with their definitions.
//! 4. [`substitute::substitute`] resolves `{variable}` references.
//! 5. [`render::render`] writes the tree back out as HTML.
//!
//! No stage reads from disk or writes to it, so each one is testable on its own.
//! [`tree::parse`] combines the first two for callers that do not need the
//! intermediate tokens.

pub mod error;
pub mod expand;
pub mod node;
pub mod render;
pub mod scope;
pub mod script;
pub mod substitute;
pub mod token;
pub mod tree;

pub use error::{
    ExpandError, ParseError, ScopeError, ScriptError, SubstError, TokenizeError, TreeError,
};
pub use node::{Attr, Element, Node};
pub use scope::{Bindings, Scope};
