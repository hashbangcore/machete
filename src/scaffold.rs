//! The files `machete init` writes for a new site.
//!
//! They are embedded at compile time from the `scaffold/` directory, so `init`
//! produces the same site no matter where it is run from.

use crate::error::{BuildError, IoError};
use std::fs;
use std::path::Path;

/// A file belonging to the scaffold, with its path relative to the site root.
struct Entry {
    path: &'static str,
    contents: &'static str,
}

/// The scaffold itself, in the order it is written.
fn entries() -> Vec<Entry> {
    vec![
        Entry {
            path: "config.toml",
            contents: include_str!("../scaffold/config.toml"),
        },
        Entry {
            path: "templates/layout.html",
            contents: include_str!("../scaffold/templates/layout.html"),
        },
        Entry {
            path: "templates/index.html",
            contents: include_str!("../scaffold/templates/index.html"),
        },
        Entry {
            path: "templates/partials/head.html",
            contents: include_str!("../scaffold/templates/partials/head.html"),
        },
        Entry {
            path: "templates/partials/aside-component.html",
            contents: include_str!("../scaffold/templates/partials/aside-component.html"),
        },
        Entry {
            path: "templates/partials/figure.html",
            contents: include_str!("../scaffold/templates/partials/figure.html"),
        },
    ]
}

/// Writes the scaffold into `root`.
///
/// Without `force`, a file that already exists is left alone, so running `init`
/// twice does not overwrite work in progress.
pub fn write(root: &Path, force: bool) -> Result<(), BuildError> {
    for entry in entries() {
        let path = root.join(entry.path);

        if path.exists() && !force {
            continue;
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                BuildError::Io(IoError::new("create directory", parent, source))
            })?;
        }

        fs::write(&path, entry.contents)
            .map_err(|source| BuildError::Io(IoError::new("write", &path, source)))?;
    }

    Ok(())
}
