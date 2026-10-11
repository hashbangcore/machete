//! Driving a build: reading a site, expanding it and writing the pages.
//!
//! Everything below the pipeline proper lives here, because it touches the
//! filesystem. The stages themselves are pure and live in [`crate::html`],
//! [`crate::vars`] and [`crate::compose`].

use crate::compose::{Defs, apply_layout, extract_layout};
use crate::config::Config;
use crate::error::{BuildError, ExpandError, IoError, LayoutError};
use crate::html::{parse, render};
use crate::node::Node;
use crate::vars::substitute;
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

    for path in collect_pages(&config.templates)? {
        let page = build_page(config, &defs, &path, output)?;
        write_page(output, &path, &page)?;
    }

    Ok(())
}

/// Builds one page and returns the HTML to write.
fn build_page(
    config: &Config,
    defs: &Defs,
    path: &Path,
    _output: &Path,
) -> Result<String, BuildError> {
    let parsed = read_page(path)?;
    let extracted = extract_layout(parsed).map_err(|source| BuildError::Layout {
        path: path.to_path_buf(),
        source,
    })?;

    let layout_name = extracted.layout.as_deref();
    let layout_path = config.layout_path(layout_name);
    let layout = read_page(&layout_path).map_err(|error| match error {
        BuildError::Io(IoError { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            BuildError::Layout {
                path: layout_path.clone(),
                source: LayoutError::NotFound {
                    name: layout_name.unwrap_or("layout").to_string(),
                },
            }
        }
        other => other,
    })?;

    let merged = apply_layout(layout, extracted.page).map_err(|source| BuildError::Layout {
        path: layout_path,
        source,
    })?;

    let expanded = crate::compose::expand(&merged, defs, &config.scope()).map_err(|source| {
        BuildError::Expand {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let mut finished = expanded;
    substitute(&mut finished, &config.scope()).map_err(|source| BuildError::Expand {
        path: path.to_path_buf(),
        source: ExpandError::Subst(source),
    })?;

    ensure_clean(&finished).map_err(|source| BuildError::Expand {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(render(&finished))
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
fn write_page(output: &Path, source: &Path, html: &str) -> Result<(), BuildError> {
    let relative = source
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("index.html"));

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
