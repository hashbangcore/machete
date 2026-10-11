//! End-to-end tests for `machete build` and `machete init`.
//!
//! These exercise the whole pipeline against real files on disk, which is the
//! only way to cover the parts that touch the filesystem: reading a site,
//! writing pages, and the promise that no Machete markup survives into the
//! output.

use machete::build::build;
use machete::build::{Config, write_scaffold as write};
use machete::error::BuildError;
use std::path::{Path, PathBuf};

/// A temporary directory that removes itself when the test ends.
struct Workspace {
    path: PathBuf,
}

impl Workspace {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("machete-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("can create the workspace");
        Self { path }
    }

    fn join(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.join(relative);
        std::fs::create_dir_all(path.parent().expect("has a parent"))
            .expect("can create the directory");
        std::fs::write(path, contents).expect("can write the file");
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.join(relative)).expect("can read the file")
    }

    fn site(&self) -> Config {
        Config::load(&self.path).expect("the site loads")
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Writes a minimal site: one global variable, one component, one layout.
fn minimal_site(workspace: &Workspace) {
    workspace.write("machete.toml", "title = \"machete\"\n");
    workspace.write(
        "templates/layout.html",
        "<!DOCTYPE html>\n<html>\n<body>\n<content/>\n</body>\n</html>\n",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main><h1>{title}</h1></main>\n",
    );
}

#[test]
fn builds_a_page_into_the_output_directory() {
    let workspace = Workspace::new("build-basic");
    minimal_site(&workspace);

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    assert!(out.join("index.html").exists());
    let page = workspace.read("out/index.html");
    assert!(page.contains("<h1>machete</h1>"), "page was {page}");
}

#[test]
fn the_output_holds_no_machete_markup() {
    let workspace = Workspace::new("build-clean");
    write(&workspace.path, false).expect("init writes the scaffold");

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    let page = workspace.read("out/index.html");
    for marker in [
        "<content",
        "type=\"component\"",
        "type=\"data\"",
        "type=\"layout\"",
    ] {
        assert!(!page.contains(marker), "{marker} survived in {page}");
    }
}

#[test]
fn globals_reach_a_component_through_the_layout() {
    let workspace = Workspace::new("build-globals");
    minimal_site(&workspace);
    workspace.write(
        "templates/partials/head.html",
        "<head><title>{title}</title></head>\n",
    );
    workspace.write(
        "templates/layout.html",
        "<html>\n<head type=\"component\" title=\"{title}\"/>\n<body><content/></body>\n</html>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    let page = workspace.read("out/index.html");
    assert!(page.contains("<title>machete</title>"), "page was {page}");
}

#[test]
fn a_component_may_take_content() {
    let workspace = Workspace::new("build-content");
    minimal_site(&workspace);
    workspace.write(
        "templates/partials/figure.html",
        "<figure><content/></figure>\n",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<figure type=\"component\"><p>dentro</p></figure>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    assert!(
        workspace
            .read("out/index.html")
            .contains("<figure><p>dentro</p></figure>")
    );
}

#[test]
fn a_component_may_take_variables_from_a_data_block() {
    let workspace = Workspace::new("build-data");
    minimal_site(&workspace);
    workspace.write(
        "templates/partials/aside.html",
        "<aside><h2>{title}</h2></aside>\n",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<aside type=\"component\">\n<script type=\"data\">\n\
         title = \"hola\";\n</script>\n</aside>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    assert!(workspace.read("out/index.html").contains("<h2>hola</h2>"));
}

#[test]
fn a_component_named_from_a_non_standard_file_is_usable() {
    let workspace = Workspace::new("build-odd-name");
    minimal_site(&workspace);
    workspace.write("templates/partials/002-mmenu.html", "<nav>menu</nav>\n");
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<002-mmenu type=\"component\"/>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    assert!(workspace.read("out/index.html").contains("<nav>menu</nav>"));
}

#[test]
fn force_empties_the_output_first() {
    let workspace = Workspace::new("build-force");
    minimal_site(&workspace);

    let out = workspace.join("out");
    std::fs::create_dir_all(&out).expect("can create the output");
    std::fs::write(out.join("stale.html"), "viejo").expect("can write the stale file");

    build(&workspace.site(), &out, true).expect("the build succeeds");

    assert!(!out.join("stale.html").exists());
    assert!(out.join("index.html").exists());
}

#[test]
fn a_layout_is_never_built_as_a_page() {
    let workspace = Workspace::new("build-skips-layout");
    minimal_site(&workspace);

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    assert!(!out.join("layout.html").exists());
}

#[test]
fn partials_are_never_built_as_pages() {
    let workspace = Workspace::new("build-skips-partials");
    minimal_site(&workspace);
    workspace.write("templates/partials/card.html", "<div>carta</div>\n");

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    assert!(!out.join("card.html").exists());
}

#[test]
fn a_missing_layout_is_reported() {
    let workspace = Workspace::new("build-no-layout");
    workspace.write("machete.toml", "title = \"x\"\n");
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>y</main>\n",
    );

    let out = workspace.join("out");
    let error = build(&workspace.site(), &out, false).expect_err("the build fails");
    assert!(format!("{error}").contains("layout"), "error was {error}");
}

#[test]
fn an_unknown_component_is_reported() {
    let workspace = Workspace::new("build-unknown");
    minimal_site(&workspace);
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<nope type=\"component\"/>\n",
    );

    let out = workspace.join("out");
    let error = build(&workspace.site(), &out, false).expect_err("the build fails");
    assert!(format!("{error}").contains("nope"), "error was {error}");
}

#[test]
fn an_unresolved_variable_is_reported() {
    let workspace = Workspace::new("build-undefined");
    minimal_site(&workspace);
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main><h1>{nope}</h1></main>\n",
    );

    let out = workspace.join("out");
    let error = build(&workspace.site(), &out, false).expect_err("the build fails");
    assert!(format!("{error}").contains("nope"), "error was {error}");
}

#[test]
fn a_data_block_outside_a_component_is_reported() {
    let workspace = Workspace::new("build-loose-data");
    minimal_site(&workspace);
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main><script type=\"data\">a = \"1\";</script></main>\n",
    );

    let out = workspace.join("out");
    let error = build(&workspace.site(), &out, false).expect_err("the build fails");
    assert!(
        format!("{error}").contains("inside a component"),
        "error was {error}"
    );
}

#[test]
fn a_site_without_config_still_builds() {
    let workspace = Workspace::new("build-no-config");
    workspace.write(
        "templates/layout.html",
        "<html><body><content/></body></html>\n",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>x</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");

    assert!(workspace.read("out/index.html").contains("<main>x</main>"));
}

#[test]
fn init_writes_every_scaffold_file() {
    let workspace = Workspace::new("init-files");

    write(&workspace.path, true).expect("init writes the scaffold");

    for relative in [
        "machete.toml",
        "templates/layout.html",
        "templates/index.html",
        "templates/partials/head.html",
    ] {
        assert!(workspace.join(relative).exists(), "{relative} is missing");
    }
}

#[test]
fn init_leaves_existing_files_alone_without_force() {
    let workspace = Workspace::new("init-no-force");
    workspace.write("machete.toml", "title = \"mio\"\n");

    write(&workspace.path, false).expect("init runs");

    assert_eq!(workspace.read("machete.toml"), "title = \"mio\"\n");
}

#[test]
fn init_overwrites_with_force() {
    let workspace = Workspace::new("init-force");
    workspace.write("machete.toml", "title = \"mio\"\n");

    write(&workspace.path, true).expect("init runs");

    assert_ne!(workspace.read("machete.toml"), "title = \"mio\"\n");
}

#[test]
fn the_generated_site_builds_to_the_same_page() {
    // The round trip matters: the scaffold embedded in the binary and the one
    // in the repository must not drift apart.
    let from_init = Workspace::new("roundtrip-init");
    let from_repo = Workspace::new("roundtrip-repo");

    write(&from_init.path, true).expect("init writes the scaffold");
    copy_scaffold(Path::new("scaffold"), &from_repo.path);

    let a = from_init.join("out");
    let b = from_repo.join("out");
    build(&from_init.site(), &a, false).expect("the generated site builds");
    build(&from_repo.site(), &b, false).expect("the repository site builds");

    assert_eq!(
        from_init.read("out/index.html"),
        from_repo.read("out/index.html")
    );
}

/// Copies a scaffold directory into a workspace, to stand in for `init`.
fn copy_scaffold(from: &Path, to: &Path) {
    let partials = from.join("templates/partials");
    let entries = std::fs::read_dir(&partials).expect("the scaffold has partials");

    std::fs::copy(from.join("machete.toml"), to.join("machete.toml")).expect("copies config");
    std::fs::create_dir_all(to.join("templates")).expect("creates templates");
    for name in ["layout.html", "index.html"] {
        std::fs::copy(
            from.join("templates").join(name),
            to.join("templates").join(name),
        )
        .expect("copies a template");
    }

    let target = to.join("templates/partials");
    std::fs::create_dir_all(&target).expect("creates partials");
    for entry in entries {
        let entry = entry.expect("reads an entry");
        std::fs::copy(entry.path(), target.join(entry.file_name())).expect("copies a partial");
    }
}

#[test]
fn an_input_without_templates_is_reported() {
    // Pointing -i at templates/ instead of the site root is the easy mistake
    // to make, and it used to produce an empty output and exit successfully.
    let workspace = Workspace::new("build-wrong-input");
    workspace.write("machete.toml", "title = \"x\"\n");
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>y</main>\n",
    );

    let templates = workspace.join("templates");
    let error = Config::load(&templates).expect_err("not a site");
    assert!(matches!(error, BuildError::NotASite { .. }));
    let message = format!("{error}");
    assert!(message.contains("not a site"), "message was {message}");
    assert!(message.contains("templates"), "message was {message}");
}

#[test]
fn a_site_with_templates_but_no_pages_is_reported() {
    let workspace = Workspace::new("build-no-pages");
    workspace.write("machete.toml", "title = \"x\"\n");
    std::fs::create_dir_all(workspace.join("templates")).expect("creates templates");

    let out = workspace.join("out");
    let error = build(&workspace.site(), &out, false).expect_err("nothing to build");
    assert!(matches!(error, BuildError::NoPages { .. }));
    assert!(!out.join("index.html").exists());
}

#[test]
fn the_site_root_is_the_directory_holding_config_toml() {
    let workspace = Workspace::new("build-root");
    minimal_site(&workspace);

    // Building from the site root works and produces the page.
    let out = workspace.join("out");
    build(&workspace.site(), &out, false).expect("the build succeeds");
    assert!(out.join("index.html").exists());
}

#[test]
fn the_output_comes_from_the_configuration_by_default() {
    let workspace = Workspace::new("paths-config");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "title = \"machete\"\n\n[paths]\noutput = \"sitio\"\n",
    );

    let config = workspace.site();
    let out = config.output_dir(None);

    assert_eq!(out, workspace.join("sitio"));
    build(&config, &out, false).expect("the build succeeds");
    assert!(workspace.join("sitio/index.html").exists());
}

#[test]
fn a_command_line_output_beats_the_configuration() {
    let workspace = Workspace::new("paths-override");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "title = \"machete\"\n\n[paths]\noutput = \"sitio\"\n",
    );

    let config = workspace.site();
    // An override is taken as written, relative to wherever machete was run,
    // so it is not resolved against the site root.
    let out = config.output_dir(Some(Path::new("otro")));

    assert_eq!(out, Path::new("otro"));
}

#[test]
fn paths_is_reserved_and_never_becomes_a_variable() {
    let workspace = Workspace::new("paths-reserved");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "title = \"x\"\n\n[paths]\noutput = \"public\"\n",
    );
    assert_eq!(workspace.site().globals.get("paths"), None);
    assert_eq!(
        workspace.site().globals.get("title").map(String::as_str),
        Some("x")
    );
}

#[test]
fn the_output_defaults_to_public() {
    let workspace = Workspace::new("paths-default");
    minimal_site(&workspace);
    assert_eq!(workspace.site().output_dir(None), workspace.join("public"));
}

#[test]
fn a_site_without_paths_still_builds_to_public() {
    let workspace = Workspace::new("paths-absent");
    minimal_site(&workspace);
    workspace.write("machete.toml", "title = \"x\"\n");

    let config = workspace.site();
    let out = config.output_dir(None);
    build(&config, &out, false).expect("the build succeeds");
    assert!(out.join("index.html").exists());
}
