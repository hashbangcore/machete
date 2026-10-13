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
use crate::sitemap;
use crate::vars::{Bindings, Scope, substitute};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// How deep to walk when looking for pages.
const MAX_DEPTH: usize = 16;

/// The page that stands for the root of the output directory.
const INDEX: &str = "index.html";

/// The file a sitemap is written to.
const SITEMAP: &str = "sitemap.xml";

/// Reads every page from `config` and writes the finished HTML into `output`,
/// returning how many pages were written.
///
/// The output directory is emptied first, so what ends up in it is exactly what
/// this build produced and nothing else. A page deleted from the templates is
/// gone from the output too, instead of lingering from an earlier build.
///
/// Static files are copied from `config.assets` into `output` untouched, keeping
/// whatever directories they sit in, so `assets/css/main.css` lands at
/// `output/css/main.css`.
pub fn build(config: &Config, output: &Path) -> Result<usize, BuildError> {
    let defs = load_definitions(config)?;
    let pages = collect_pages(&config.templates)?;

    if pages.is_empty() {
        return Err(BuildError::NoPages {
            templates: config.templates.clone(),
        });
    }

    // Emptied once the site is known to be buildable. Checking first means a
    // site with no pages fails without taking its output down with it.
    if output.exists() {
        fs::remove_dir_all(output)
            .map_err(|source| BuildError::Io(IoError::new("remove", output, source)))?;
    }

    fs::create_dir_all(output)
        .map_err(|source| BuildError::Io(IoError::new("create directory", output, source)))?;

    // The paths are recorded as the pages are written rather than worked out a
    // second time for the sitemap, so the two cannot disagree about what exists.
    let mut written = Vec::new();

    for path in pages {
        let relative = relative_output_path(&path, &config.templates);
        let page = build_page(config, &defs, &path)?;
        write_page(output, &relative, &page.html)?;

        if page.indexable {
            written.push(relative);
        }
    }

    copy_assets(config, output)?;
    write_sitemap(config, output, &written)?;

    Ok(written.len())
}

/// Writes `sitemap.xml` when the site says where it is published.
///
/// A site without a URL gets nothing. There would be no absolute address to
/// write, and a sitemap of relative paths would be worse than no sitemap: it
/// would be rejected. This runs after the assets so that it wins over an
/// `assets/sitemap.xml` the site may carry of its own.
fn write_sitemap(config: &Config, output: &Path, pages: &[PathBuf]) -> Result<(), BuildError> {
    // Checked before the URL: turning the sitemap off is an explicit choice,
    // so there is nothing to look for once it has been made.
    if !config.sitemap {
        return Ok(());
    }

    let Some(base) = config.url.as_deref() else {
        return Ok(());
    };

    let target = output.join(SITEMAP);

    fs::write(&target, sitemap::render(base, pages))
        .map_err(|source| BuildError::Io(IoError::new("write", &target, source)))
}

/// Copies every file under `config.assets` into `output`, unchanged.
///
/// The layout of the source directory is kept, so a file inside `css/` stays
/// inside `css/`. A site with no assets directory simply has nothing to copy.
fn copy_assets(config: &Config, output: &Path) -> Result<(), BuildError> {
    if !config.assets.is_dir() {
        return Ok(());
    }

    let mut pending = vec![config.assets.clone()];

    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|source| BuildError::Io(IoError::new("read", &dir, source)))?;

        for entry in entries {
            let entry =
                entry.map_err(|source| BuildError::Io(IoError::new("read", &dir, source)))?;
            let path = entry.path();

            if path.is_dir() {
                pending.push(path);
                continue;
            }

            let relative = path
                .strip_prefix(&config.assets)
                .expect("path came from walking the assets directory");
            let target = output.join(relative);

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    BuildError::Io(IoError::new("create directory", parent, source))
                })?;
            }

            fs::copy(&path, &target)
                .map_err(|source| BuildError::Io(IoError::new("copy", &path, source)))?;
        }
    }

    Ok(())
}

/// One finished page: the HTML to write, and whether a search engine should be
/// told about it.
struct Page {
    html: String,
    indexable: bool,
}

/// Builds one page and returns what to write for it.
fn build_page(config: &Config, defs: &Defs, path: &Path) -> Result<Page, BuildError> {
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

    Ok(Page {
        html: render(&page),
        indexable: is_indexable(&page),
    })
}

/// Reports whether a page asks to be indexed.
///
/// A page carrying `<meta name="robots" content="noindex">` is written but left
/// out of the sitemap. The tag is looked for in the finished tree, after
/// components have been expanded and variables resolved, so one added by a
/// component or produced by a variable counts as much as one written by hand.
fn is_indexable(node: &Node) -> bool {
    let mut indexable = true;

    walk(node, &mut |current| {
        if !indexable {
            return;
        }

        let Some(element) = current.as_element() else {
            return;
        };

        if element.name != "meta" {
            return;
        }

        let robots = element
            .attr("name")
            .is_some_and(|name| name.eq_ignore_ascii_case("robots"));
        let noindex = element.attr("content").is_some_and(|content| {
            content
                .split([',', ' ', '\t', '\n', '\r'])
                .any(|token| token.eq_ignore_ascii_case("noindex"))
        });

        if robots && noindex {
            indexable = false;
        }
    });

    indexable
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

/// The path a page takes inside the output directory, relative to it.
///
/// Keeps the directory a page sits in, so `templates/blog/post.html` lands in
/// `public/blog/post.html` rather than colliding with `post.html` at the root.
fn relative_output_path(source: &Path, templates: &Path) -> PathBuf {
    let relative = source.strip_prefix(templates).unwrap_or(source);

    if relative.as_os_str().is_empty() {
        PathBuf::from(INDEX)
    } else {
        relative.to_path_buf()
    }
}

/// Writes one page, creating the directories its relative path needs.
fn write_page(output: &Path, relative: &Path, html: &str) -> Result<(), BuildError> {
    let target = output.join(relative);

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

/// Directories given on the command line.
///
/// Each one beats the same name in `[build]`, which in turn beats the default.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// Where the pages are written.
    pub output: Option<PathBuf>,
    /// Where the templates live.
    pub templates: Option<PathBuf>,
    /// Where the static files live.
    pub assets: Option<PathBuf>,
}

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
    /// Static files copied into the output untouched.
    pub assets: PathBuf,
    /// Where the built pages go, relative to the root.
    pub output: PathBuf,
    /// Where the site is published, as `https://example.com`.
    ///
    /// A sitemap is only meaningful with it, because the protocol asks for
    /// absolute URLs. Without this the build writes no sitemap.
    pub url: Option<String>,
    /// Whether to write a sitemap, on unless the site turns it off.
    ///
    /// Kept separate from [`Self::url`] because the two answer different
    /// questions: this says whether to write one, that says where the site
    /// lives. Turning the sitemap off works either way; publishing it needs
    /// both.
    pub sitemap: bool,
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
const DEFAULT_ASSETS: &str = "assets";

/// The configuration table that holds the directory names.
const BUILD: &str = "build";

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
        Self::load_with(root, &Overrides::default())
    }

    /// Reads `machete.toml` from `root`, applying any directory a flag set.
    ///
    /// A flag beats the file, and the file beats the default.
    pub fn load_with(root: &Path, overrides: &Overrides) -> Result<Self, BuildError> {
        let path = root.join(CONFIG_FILE);
        let parsed = match std::fs::read_to_string(&path) {
            Ok(text) => parse_config(&text, &path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(BuildError::NoConfig {
                    root: root.to_path_buf(),
                });
            }
            Err(source) => return Err(BuildError::Io(IoError::new("read", path, source))),
        };

        // Flag, then the file, then the default.
        let templates = root.join(
            overrides
                .templates
                .as_deref()
                .or(parsed.dirs.templates.as_deref())
                .unwrap_or(Path::new(TEMPLATES)),
        );

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
            assets: root.join(
                overrides
                    .assets
                    .as_deref()
                    .or(parsed.dirs.assets.as_deref())
                    .unwrap_or(Path::new(DEFAULT_ASSETS)),
            ),
            output: overrides
                .output
                .as_deref()
                .or(parsed.dirs.public.as_deref())
                .unwrap_or(Path::new(DEFAULT_OUTPUT))
                .to_path_buf(),
            url: parsed.dirs.url,
            sitemap: parsed.dirs.sitemap.unwrap_or(true),
            data: parsed.data,
            layout_data: parsed.layout_data,
            metadata: parsed.metadata,
        })
    }

    /// Returns the output directory, resolved against the site root.
    ///
    /// A path given on the command line is used as written, so it stays
    /// relative to the working directory rather than jumping to the root.
    pub fn output_dir(&self, override_path: Option<&Path>) -> PathBuf {
        match override_path {
            Some(path) => path.to_path_buf(),
            None => self.root.join(&self.output),
        }
    }

    /// Parses a configuration file held in memory.
    #[cfg(test)]
    pub(crate) fn parse_for_test(text: &str) -> Self {
        let parsed = parse_config(text, Path::new(CONFIG_FILE)).expect("valid config");
        Self {
            root: PathBuf::from("."),
            templates: PathBuf::from(TEMPLATES),
            partials: PathBuf::from(TEMPLATES).join(PARTIALS),
            layouts: PathBuf::from(TEMPLATES).join(LAYOUTS),
            assets: PathBuf::from(DEFAULT_ASSETS),
            output: parsed
                .dirs
                .public
                .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT)),
            url: parsed.dirs.url,
            sitemap: parsed.dirs.sitemap.unwrap_or(true),
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
fn parse_config(text: &str, path: &Path) -> Result<Parsed, BuildError> {
    let table: toml::Table = toml::from_str(text).map_err(|source| BuildError::Config {
        path: path.to_path_buf(),
        source,
    })?;

    let mut parsed = Parsed::default();

    for (key, value) in table {
        match key.as_str() {
            BUILD => read_build(&value, &mut parsed.dirs)?,

            DATA => read_data(&value, &mut parsed)?,

            // Anything else belongs to the user. Machete neither reads nor
            // injects it, so a site may keep whatever it likes alongside.
            _ => {
                parsed.metadata.insert(key, scalar(value));
            }
        }
    }

    Ok(parsed)
}

/// The variable sections read out of a configuration file.
#[derive(Default)]
struct Parsed {
    data: Bindings,
    layout_data: HashMap<String, Bindings>,
    metadata: Bindings,
    dirs: Dirs,
}

/// The directory names read out of `[build]`.
#[derive(Default)]
struct Dirs {
    public: Option<PathBuf>,
    templates: Option<PathBuf>,
    assets: Option<PathBuf>,
    url: Option<String>,
    sitemap: Option<bool>,
}

/// Reads the paths out of `[build]`.
///
/// Each one is optional and falls back to its default, so a site only writes
/// what it wants to change. `[build]` itself may be absent entirely.
fn read_build(value: &toml::Value, dirs: &mut Dirs) -> Result<(), BuildError> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };

    for (key, value) in table {
        let name = scalar(value.clone());

        match key.as_str() {
            "public" => set_dir(&mut dirs.public, &name),
            "templates" => set_dir(&mut dirs.templates, &name),
            "assets" => set_dir(&mut dirs.assets, &name),

            // The one thing in `[build]` that is not a directory.
            "url" => {
                if !name.is_empty() {
                    dirs.url = Some(name);
                }
            }

            // The one thing in `[build]` that is not a path or a string.
            "sitemap" => dirs.sitemap = Some(read_bool("sitemap", value)?),

            _ => continue,
        }
    }

    Ok(())
}

/// Reads a boolean out of `[build]`, rejecting anything else.
///
/// Turning the sitemap off is the whole point of the key, so a value that is
/// not a boolean is a mistake worth reporting rather than guessing at: `0` and
/// `"no"` mean nothing here, and reading either as "on" would be the opposite
/// of what the author wrote.
fn read_bool(key: &'static str, value: &toml::Value) -> Result<bool, BuildError> {
    value
        .as_bool()
        .ok_or_else(|| BuildError::InvalidBuildValue {
            key,
            found: value.to_string(),
        })
}

/// Records a directory name, treating an empty one as unset.
fn set_dir(slot: &mut Option<PathBuf>, name: &str) {
    if !name.is_empty() {
        *slot = Some(PathBuf::from(name));
    }
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
/// The files come from the `docs/scaffold/` directory, embedded at compile
/// time, so `init` produces the same site wherever it runs.
///
/// A directory that already exists is left completely alone. An existing site
/// belongs to whoever is working on it, and a scaffold is a starting point
/// rather than something worth overwriting.
pub fn write_scaffold(root: &Path) -> Result<(), BuildError> {
    if root.exists() {
        return Ok(());
    }

    for entry in scaffold_files() {
        let path = root.join(entry.path);

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
            contents: include_str!("../docs/scaffold/machete.toml"),
        },
        ScaffoldFile {
            path: "templates/layout.html",
            contents: include_str!("../docs/scaffold/templates/layout.html"),
        },
        ScaffoldFile {
            path: "templates/index.html",
            contents: include_str!("../docs/scaffold/templates/index.html"),
        },
        ScaffoldFile {
            path: "templates/partials/head.html",
            contents: include_str!("../docs/scaffold/templates/partials/head.html"),
        },
        ScaffoldFile {
            path: "templates/partials/header.html",
            contents: include_str!("../docs/scaffold/templates/partials/header.html"),
        },
        ScaffoldFile {
            path: "assets/css/style.css",
            contents: include_str!("../docs/scaffold/assets/css/style.css"),
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

    #[test]
    fn the_sitemap_is_on_unless_the_site_says_otherwise() {
        let config = parse("[build]\nurl = \"https://ejemplo.com\"\n");
        assert!(config.sitemap);
    }

    #[test]
    fn the_sitemap_can_be_turned_off() {
        let config = parse("[build]\nurl = \"https://ejemplo.com\"\nsitemap = false\n");
        assert!(!config.sitemap);
    }

    #[test]
    fn turning_the_sitemap_off_leaves_the_url_alone() {
        let config = parse("[build]\nurl = \"https://ejemplo.com\"\nsitemap = false\n");
        assert_eq!(config.url.as_deref(), Some("https://ejemplo.com"));
    }

    #[test]
    fn a_sitemap_value_that_is_not_a_boolean_is_reported() {
        for text in [
            "[build]\nsitemap = \"no\"\n",
            "[build]\nsitemap = 0\n",
            "[build]\nsitemap = \"false\"\n",
        ] {
            let error = match parse_config(text, Path::new("x")) {
                Err(error) => error,
                Ok(_) => panic!("expected a bad-build-value error for {text}"),
            };
            assert!(matches!(
                error,
                BuildError::InvalidBuildValue { key: "sitemap", .. }
            ));
        }
    }
}
