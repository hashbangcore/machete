//! Machete composes HTML pages from components, layouts and variables.
//!
//! The pipeline is a sequence of pure functions, each taking one step's output
//! as the next step's input:
//!
//! 1. [`html::tokenize`] splits source into tokens.
//! 2. [`html::build`] assembles those tokens into a document tree.
//! 3. [`compose::apply_layout`] inserts the page into the layout it asked for.
//! 4. [`compose::expand`] replaces component invocations with their definitions.
//! 5. [`vars::substitute`] resolves `{variable}` references.
//! 6. [`html::render`] writes the tree back out as HTML.
//!
//! Only [`build`] and [`config`] touch the filesystem. Everything else is a
//! function from input to output, so each stage is testable on its own.
//!
//! # Output
//!
//! The HTML written to `public/` is plain markup. None of Machete's own
//! constructs reach it: no `<content/>`, no `type="component"`, no
//! `type="layout"`, no `<script type="data">`.
//!
//! # Layout of the crate
//!
//! - [`node`] and [`error`] sit at the top level because every layer uses them:
//!   they are the shared vocabulary rather than part of any one layer.
//! - [`html`] parses and renders documents.
//! - [`vars`] resolves variable values.
//! - [`compose`] combines components and applies layouts.

pub mod build;
pub mod cli;
pub mod compose;
pub mod config;
pub mod error;
pub mod html;
pub mod node;
pub mod scaffold;
pub mod vars;

pub use error::BuildError;
