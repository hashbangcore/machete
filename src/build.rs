//! Driving a build: reading a site, expanding it and writing the pages.
//!
//! Everything below the pipeline proper lives here, because it touches the
//! filesystem. The stages themselves are pure and live in [`crate::html`],
//! [`crate::vars`] and [`crate::compose`].
//!
//! [`Config`] is here for the same reason: reading `machete.toml` is I/O.

use crate::compose::{self, Defs, apply_layout, extract_layout};
use crate::error::{BuildError, ExpandError, IoError, LayoutError};
use crate::html::{parse, render};
use crate::node::Node;
use crate::vars::{Bindings, Scope, substitute};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// How deep to walk when looking for pages.
const MAX_DEPTH: usize = 16;

/// Reads every page from `config` and writes the finished HTML into `output`.
///
/// With `force` set, `output` is emptied first so pages from an earlier build
/// cannot survive alongside the new ones.
pub fn build(config: &Config, output: &Path, force: bool) -> Result<(), BuildError> {
    if force && output.exists() {
        fs::remove_dir_all(output)
            .map_err(|source| BuildError::Io(IoError::new("remove", output, source)))?;
    }

    fs::create_dir_all(output)
        .map_err(|source| BuildError::Io(IoError::new("create directory", output, source)))?;

    let defs = load_definitions(config)?;
    let pages = collect_pages(&config.templates)?;

    if pages.is_empty() {
        return Err(BuildError::NoPages {
            templates: config.templates.clone(),
        });
    }

    for path in pages {
        let page = build_page(config, &defs, &path)?;
        write_page(output, &path, &config.templates, &page)?;
    }

    Ok(())
}

/// Builds one page and returns the HTML to write.
fn build_page(config: &Config, defs: &Defs, path: &Path) -> Result<String, BuildError> {
    let parsed = read_page(path)?;

    let extracted = extract_layout(parsed).map_err(|source| BuildError::Layout {
        path: path.to_path_buf(),
        source,
    })?;

    // After extraction, because the layout a page asked for decides which
    // variables it sees.
    let scope = config.scope_for(extracted.layout.as_deref());

    let layout_path = config.layout_path(extracted.layout.as_deref());

    if !layout_path.exists() {
        return Err(BuildError::Layout {
            path: path.to_path_buf(),
            source: LayoutError::NotFound {
                name: extracted.layout.unwrap_or_else(|| "layout".to_string()),
            },
        });
    }

    let layout = read_page(&layout_path)?;

    let merged = apply_layout(layout, extracted.page).map_err(|source| BuildError::Layout {
        path: path.to_path_buf(),
        source,
    })?;

    let mut page = compose::expand(&merged, defs, &scope).map_err(|source| BuildError::Expand {
        path: path.to_path_buf(),
        source,
    })?;

    substitute(&mut page, &scope).map_err(|source| BuildError::Expand {
        path: path.to_path_buf(),
        source: ExpandError::Subst(source),
    })?;

    ensure_clean(&page).map_err(|source| BuildError::Expand {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(render(&page))
}

/// Fails when any construction of Machete is still present.
///
/// The pipeline is meant to consume every one of them. This is the backstop: if
/// a new feature ever leaves a marker behind, the build stops rather than
/// writing a page that still contains `<content/>` or `type="component"`.
fn ensure_clean(node: &Node) -> Result<(), ExpandError> {
    let mut found = None;
    walk(node, &mut |current| {
        if found.is_some() {
            return;
        }
        if let Some(element) = current.as_element() {
            if element.is_content_marker() {
                found = Some("content marker".to_string());
            } else if element.is_invocation() {
                found = Some(format!("{} invocation", element.name));
            } else if element.is_data_script() {
                found = Some("script data block".to_string());
            } else if element.is_layout_directive() {
                found = Some("layout directive".to_string());
            }
        }
    });

    match found {
        Some(found) => Err(ExpandError::LeftoverMarkup { found }),
        None => Ok(()),
    }
}

/// Visits every node in the tree, depth first.
fn walk(node: &Node, visit: &mut impl FnMut(&Node)) {
    visit(node);
    if let Some(children) = node.child_list() {
        for child in children {
            walk(child, visit);
        }
    }
}

/// Reads a template file and parses it.
fn read_page(path: &Path) -> Result<Node, BuildError> {
    let text = fs::read_to_string(path)
        .map_err(|source| BuildError::Io(IoError::new("read", path, source)))?;

    parse(&text).map_err(|source| BuildError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes one page, creating the directories its relative path needs.
fn write_page(
    output: &Path,
    source: &Path,
    templates: &Path,
    html: &str,
) -> Result<(), BuildError> {
    // Keep the directory a page sits in, so `templates/blog/post.html` lands in
    // `public/blog/post.html` rather than colliding with `post.html` at the root.
    let relative = source.strip_prefix(templates).unwrap_or(source);

    let target = if relative.as_os_str().is_empty() {
        output.join("index.html")
    } else {
        output.join(relative)
    };

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| BuildError::Io(IoError::new("create directory", parent, source)))?;
    }

    fs::write(&target, html)
        .map_err(|source| BuildError::Io(IoError::new("write", &target, source)))
}

/// Loads every component definition, keyed by the file it came from.
fn load_definitions(config: &Config) -> Result<Defs, BuildError> {
    let mut defs = Defs::new();

    if !config.partials.exists() {
        return Ok(defs);
    }

    let entries = fs::read_dir(&config.partials)
        .map_err(|source| BuildError::Io(IoError::new("read", &config.partials, source)))?;

    for entry in entries {
        let entry = entry
            .map_err(|source| BuildError::Io(IoError::new("read", &config.partials, source)))?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("html") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let text = fs::read_to_string(&path)
            .map_err(|source| BuildError::Io(IoError::new("read", &path, source)))?;

        let tree = parse(&text).map_err(|source| BuildError::Parse {
            path: path.clone(),
            source,
        })?;

        defs.insert(stem.to_string(), tree);
    }

    Ok(defs)
}

/// Lists the pages to build: every `.html` under `templates/`, except the
/// layout files and the `partials/` and `layouts/` directories.
fn collect_pages(templates: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let mut pages = Vec::new();

    if !templates.exists() {
        return Ok(pages);
    }

    let mut pending = vec![(templates.to_path_buf(), 0usize)];

    while let Some((dir, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            continue;
        }

        let entries = fs::read_dir(&dir)
            .map_err(|source| BuildError::Io(IoError::new("read", &dir, source)))?;

        for entry in entries {
            let entry =
                entry.map_err(|source| BuildError::Io(IoError::new("read", &dir, source)))?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if path.is_dir() {
                // These hold definitions, not pages.
                if name == "partials" || name == "layouts" {
                    continue;
                }
                pending.push((path, depth + 1));
                continue;
            }

            if path.extension().and_then(|e| e.to_str()) != Some("html") {
                continue;
            }

            // The default layout is applied to pages, never built on its own.
            if dir == templates && name == "layout.html" {
                continue;
            }

            if dir.parent() == Some(templates) && name.starts_with("layout.") {
                continue;
            }

            pages.push(path);
        }
    }

    pages.sort();
    Ok(pages)
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// The directories a site is made of.
#[derive(Debug, Clone)]
pub struct Config {
    /// The site root, the directory holding `machete.toml`.
    pub root: PathBuf,
    /// Where pages and layouts live.
    pub templates: PathBuf,
    /// Where component definitions live.
    pub partials: PathBuf,
    /// Where additional layouts live.
    pub layouts: PathBuf,
    /// Where the built pages go, relative to the root.
    pub output: PathBuf,
    /// Variables from `[data]`, available to every page, layout and component.
    pub data: Bindings,
    /// Variables from `[data.<layout>]`, each merged over `[data]`.
    pub layout_data: HashMap<String, Bindings>,
    /// Top-level keys such as `author` or `version`.
    ///
    /// Machete never injects these. They are kept so a user can define whatever
    /// else they want in the file, for a pipeline or for themselves.
    pub metadata: Bindings,
}

/// The default directory names, used when a site is laid out conventionally.
const TEMPLATES: &str = "templates";
const PARTIALS: &str = "partials";
const LAYOUTS: &str = "layouts";
const CONFIG_FILE: &str = "machete.toml";
const DEFAULT_OUTPUT: &str = "public";

/// The configuration table that configures paths rather than variables.
const PATHS: &str = "paths";

/// The configuration table holding the variables injected into the site.
const DATA: &str = "data";

impl Config {
    /// Reads `machete.toml` from `root` and resolves the site directories.
    ///
    /// The file is what identifies the directory as a machete project, so a
    /// missing one is an error rather than an empty set of globals. Without
    /// that, a forgotten `title` would be reported as an undefined variable and
    /// the real cause, a missing config file, would stay hidden.
    pub fn load(root: &Path) -> Result<Self, BuildError> {
        let path = root.join(CONFIG_FILE);
        let (parsed, output) = match std::fs::read_to_string(&path) {
            Ok(text) => parse_config(&text, &path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(BuildError::NoConfig {
                    root: root.to_path_buf(),
                });
            }
            Err(source) => return Err(BuildError::Io(IoError::new("read", path, source))),
        };

        let templates = root.join(TEMPLATES);

        if !templates.is_dir() {
            return Err(BuildError::NotASite {
                root: root.to_path_buf(),
                expected: templates.clone(),
            });
        }

        Ok(Self {
            root: root.to_path_buf(),
            partials: templates.join(PARTIALS),
            layouts: templates.join(LAYOUTS),
            templates,
            output: output.unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT)),
            data: parsed.data,
            layout_data: parsed.layout_data,
            metadata: parsed.metadata,
        })
    }

    /// Returns the output directory resolved against the site root.
    pub fn output_dir(&self, override_path: Option<&Path>) -> PathBuf {
        match override_path {
            Some(path) => path.to_path_buf(),
            None => self.root.join(&self.output),
        }
    }

    /// Returns a scope holding only the global variables.
    /// Parses a configuration file held in memory.
    #[cfg(test)]
    pub(crate) fn parse_for_test(text: &str) -> Self {
        let (parsed, output) = parse_config(text, Path::new("machete.toml")).expect("valid config");
        Self {
            root: PathBuf::from("."),
            templates: PathBuf::from(TEMPLATES),
            partials: PathBuf::from(TEMPLATES).join(PARTIALS),
            layouts: PathBuf::from(TEMPLATES).join(LAYOUTS),
            output: output.unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT)),
            data: parsed.data,
            layout_data: parsed.layout_data,
            metadata: parsed.metadata,
        }
    }

    /// Returns the variables a page using `layout` should see.
    ///
    /// `[data.<layout>]` was already merged over `[data]` when the config was
    /// read, so this is a lookup. A layout without a section of its own sees
    /// the base on its own.
    pub fn scope_for(&self, layout: Option<&str>) -> Scope {
        let merged = layout
            .and_then(|name| self.layout_data.get(name))
            .unwrap_or(&self.data);
        Scope::new(merged.clone())
    }

    /// Reads the layout with the given name, or the default one when unnamed.
    pub fn layout_path(&self, name: Option<&str>) -> PathBuf {
        match name {
            Some(name) => self.layouts.join(format!("{name}.html")),
            None => self.templates.join("layout.html"),
        }
    }
}

/// Reads a configuration file into its globals and its output path.
///
/// The keys at the top level are the global variables. The `paths` table is
/// reserved: it configures where things live, so its keys never become
/// variables.
fn parse_config(text: &str, path: &Path) -> Result<(Parsed, Option<PathBuf>), BuildError> {
    let table: toml::Table = toml::from_str(text).map_err(|source| BuildError::Config {
        path: path.to_path_buf(),
        source,
    })?;

    let mut parsed = Parsed::default();
    let mut output = None;

    for (key, value) in table {
        match key.as_str() {
            PATHS => {
                output = value
                    .get("output")
                    .map(|value| scalar(value.clone()))
                    .filter(|text| !text.is_empty())
                    .map(PathBuf::from);
            }

            DATA => read_data(&value, &mut parsed)?,

            // Anything else belongs to the user. Machete neither reads nor
            // injects it, so a site may keep whatever it likes alongside.
            _ => {
                parsed.metadata.insert(key, scalar(value));
            }
        }
    }

    Ok((parsed, output))
}

/// The variable sections read out of a configuration file.
#[derive(Default)]
struct Parsed {
    data: Bindings,
    layout_data: HashMap<String, Bindings>,
    metadata: Bindings,
}

/// Reads `[data]` and the `[data.<layout>]` sections below it.
///
/// A scalar belongs to `[data]`. A table names a layout, and its values are
/// merged over the base so the layout inherits whatever it does not repeat.
/// A table inside one of those is a level too deep and is reported.
fn read_data(value: &toml::Value, parsed: &mut Parsed) -> Result<(), BuildError> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };

    // Two passes. The first collects the base, the second merges each layout
    // over it. Doing both in one pass would merge over a base that is still
    // being filled, so a layout would miss keys written after it in the file.
    let mut base = Bindings::new();
    let mut layouts: Vec<(String, &toml::Table)> = Vec::new();

    for (key, value) in table {
        match value.as_table() {
            Some(layout) => layouts.push((key.clone(), layout)),
            None => {
                base.insert(key.clone(), scalar(value.clone()));
            }
        }
    }

    for (name, layout) in layouts {
        let mut merged = base.clone();

        for (key, value) in layout {
            if value.is_table() {
                return Err(BuildError::NestedData {
                    name: format!("{name}.{key}"),
                });
            }
            merged.insert(key.clone(), scalar(value.clone()));
        }

        parsed.layout_data.insert(name, merged);
    }

    parsed.data = base;

    Ok(())
}

/// Renders a TOML scalar as the text a variable should hold.
fn scalar(value: toml::Value) -> String {
    match value {
        toml::Value::String(text) => text,
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Scaffold
// ---------------------------------------------------------------------------

/// Writes the scaffold for a new site into `root`.
///
/// The files come from the `scaffold/` directory, embedded at compile time, so
/// `init` produces the same site wherever it runs.
///
/// Without `force`, a file that already exists is left alone, so running `init`
/// twice does not overwrite work in progress.
pub fn write_scaffold(root: &Path, force: bool) -> Result<(), BuildError> {
    for entry in scaffold_files() {
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

/// A file belonging to the scaffold, with its path relative to the site root.
struct ScaffoldFile {
    path: &'static str,
    contents: &'static str,
}

/// The scaffold itself, in the order it is written.
fn scaffold_files() -> Vec<ScaffoldFile> {
    vec![
        ScaffoldFile {
            path: "machete.toml",
            contents: include_str!("../scaffold/machete.toml"),
        },
        ScaffoldFile {
            path: "templates/layout.html",
            contents: include_str!("../scaffold/templates/layout.html"),
        },
        ScaffoldFile {
            path: "templates/index.html",
            contents: include_str!("../scaffold/templates/index.html"),
        },
        ScaffoldFile {
            path: "templates/partials/head.html",
            contents: include_str!("../scaffold/templates/partials/head.html"),
        },
        ScaffoldFile {
            path: "templates/partials/aside-component.html",
            contents: include_str!("../scaffold/templates/partials/aside-component.html"),
        },
        ScaffoldFile {
            path: "templates/partials/figure.html",
            contents: include_str!("../scaffold/templates/partials/figure.html"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Config {
        Config::parse_for_test(text)
    }

    #[test]
    fn data_becomes_the_base() {
        let config = parse("[data]\ntitle = \"t\"\n");
        assert_eq!(config.data.get("title").map(String::as_str), Some("t"));
        assert!(config.layout_data.is_empty());
    }

    #[test]
    fn layout_data_merges_over_the_base() {
        let config =
            parse("[data]\ntitle = \"t\"\nlang = \"es\"\n\n[data.about]\ntitle = \"about\"\n");
        assert_eq!(config.scope_for(Some("about")).get("title"), Some("about"));
        assert_eq!(config.scope_for(Some("about")).get("lang"), Some("es"));
    }

    #[test]
    fn a_layout_without_a_section_sees_only_the_base() {
        let config = parse("[data]\ntitle = \"t\"\n\n[data.about]\ntitle = \"about\"\n");
        assert_eq!(config.scope_for(Some("other")).get("title"), Some("t"));
    }

    #[test]
    fn loose_keys_land_in_metadata() {
        let config = parse("author = \"hash\"\n\n[data]\ntitle = \"t\"\n");
        assert_eq!(
            config.metadata.get("author").map(String::as_str),
            Some("hash")
        );
        assert!(!config.data.contains_key("author"));
    }

    #[test]
    fn nesting_two_deep_is_rejected() {
        let error = match parse_config("[data.blog.post]\ntitle = \"x\"\n", Path::new("x")) {
            Err(error) => error,
            Ok(_) => panic!("expected a nested-data error"),
        };
        assert!(matches!(error, BuildError::NestedData { .. }));
    }

    #[test]
    fn a_foreign_structure_does_not_stop_the_parse() {
        let config = parse("[ci]\nrun = \"x\"\n\n[data]\ntitle = \"t\"\n");
        assert_eq!(config.data.get("title").map(String::as_str), Some("t"));
    }
}
