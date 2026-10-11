//! Parsing HTML source into a tree, and rendering a tree back into HTML.
//!
//! [`tokenize`] splits source into a flat token stream, [`build`] nests those
//! tokens into a document, and [`render`] writes a document out again. Parsing
//! and rendering are separate steps because the component pipeline inserts
//! tree manipulation between them.

mod render;
mod token;
mod tree;

pub use render::render;
pub use token::{RAWTEXT_ELEMENTS, Token, tokenize};
pub use tree::{build, parse};
