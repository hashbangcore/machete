//! Turning component invocations into markup.
//!
//! [`expand`] walks a document and replaces every `type="component"` tag with
//! the definition it names, resolving variables and inserting content along the
//! way.

mod expand;

pub use expand::{COMPONENT_TYPE, Defs, expand};
