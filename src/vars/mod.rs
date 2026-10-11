//! Variable values and the rules for resolving them.
//!
//! [`Scope`] holds the values and decides which one wins,
//! [`parse_bindings`] reads declarations out of a component script, and
//! [`substitute`] replaces `{variable}` references in a document.

mod scope;
mod script;
mod substitute;

pub use scope::{Bindings, Scope};
pub use script::parse_bindings;
pub use substitute::{substitute, substitute_text};
