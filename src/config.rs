//! Reading a site's configuration.
//!
//! `config.toml` supplies the variables that are global to the whole site. The
//! keys at the top level are the globals; nothing else is required, since the
//! directory layout has defaults.

use crate::error::{BuildError, IoError};
use crate::vars::{Bindings, Scope};
use std::path::{Path, PathBuf};

/// The directories a site is made of.
#[derive(Debug, Clone)]
pub struct Config {
    /// The site root, the directory holding `config.toml`.
    pub root: PathBuf,
    /// Where pages and layouts live.
    pub templates: PathBuf,
    /// Where component definitions live.
    pub partials: PathBuf,
    /// Where additional layouts live.
    pub layouts: PathBuf,
    /// Variables available to every page, layout and component.
    pub globals: Bindings,
}

/// The default directory names, used when a site is laid out conventionally.
const TEMPLATES: &str = "templates";
const PARTIALS: &str = "partials";
const LAYOUTS: &str = "layouts";
const CONFIG_FILE: &str = "config.toml";

impl Config {
    /// Reads `config.toml` from `root` and resolves the site directories.
    ///
    /// A missing `config.toml` is not an error: a site may define everything in
    /// its pages, and the globals are simply empty.
    pub fn load(root: &Path) -> Result<Self, BuildError> {
        let path = root.join(CONFIG_FILE);
        let globals = match std::fs::read_to_string(&path) {
            Ok(text) => parse_globals(&text, &path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Bindings::new(),
            Err(source) => return Err(BuildError::Io(IoError::new("read", path, source))),
        };

        let templates = root.join(TEMPLATES);

        Ok(Self {
            root: root.to_path_buf(),
            partials: templates.join(PARTIALS),
            layouts: templates.join(LAYOUTS),
            templates,
            globals,
        })
    }

    /// Returns a scope holding only the global variables.
    pub fn scope(&self) -> Scope {
        Scope::new(self.globals.clone())
    }

    /// Reads the layout with the given name, or the default one when unnamed.
    pub fn layout_path(&self, name: Option<&str>) -> PathBuf {
        match name {
            Some(name) => self.layouts.join(format!("{name}.html")),
            None => self.templates.join("layout.html"),
        }
    }
}

/// Reads the top-level keys of a configuration file as variable bindings.
fn parse_globals(text: &str, path: &Path) -> Result<Bindings, BuildError> {
    let globals: toml::Table = toml::from_str(text).map_err(|source| BuildError::Config {
        path: path.to_path_buf(),
        source,
    })?;

    let mut bindings = Bindings::new();

    for (key, value) in globals {
        let text = match value {
            toml::Value::String(text) => text,
            other => other.to_string(),
        };
        bindings.insert(key, text);
    }

    Ok(bindings)
}
