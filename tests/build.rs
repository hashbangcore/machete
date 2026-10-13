//! End-to-end tests for `machete build` and `machete init`.
//!
//! These exercise the whole pipeline against real files on disk, which is the
//! only way to cover the parts that touch the filesystem: reading a site,
//! writing pages, and the promise that no Machete markup survives into the
//! output.

use machete::build::build;
use machete::build::{Config, Overrides, write_scaffold as write};
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
    workspace.write("machete.toml", "[data]\ntitle = \"machete\"\n");
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
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("index.html").exists());
    let page = workspace.read("out/index.html");
    assert!(page.contains("<h1>machete</h1>"), "page was {page}");
}

#[test]
fn the_output_holds_no_machete_markup() {
    let workspace = Workspace::new("build-clean");
    let site = workspace.join("sitio");
    write(&site).expect("init writes the scaffold");

    let config = Config::load(&site).expect("the site loads");
    let out = workspace.join("out");
    build(&config, &out).expect("the build succeeds");

    let page = workspace.read("out/index.html");

    assert!(!page.contains("<content"), "<content survived in {page}");

    for attribute in ["type=\"component\"", "type=\"data\"", "type=\"layout\""] {
        assert!(
            !inside_a_tag(&page, attribute),
            "{attribute} survived in {page}"
        );
    }
}

/// Reports whether any real element in `page` carries `attribute`.
///
/// A `<` written in prose comes out escaped, so the scaffold can talk about
/// `type="data"` in plain text and it will arrive as `&lt;script type="data"&gt;`,
/// which is inert. Only an unescaped `<` opens a tag, so the attribute is
/// looked for between a `<` and the `>` that closes it.
fn inside_a_tag(page: &str, attribute: &str) -> bool {
    let mut start = 0;

    while let Some(offset) = page[start..].find('<') {
        let open = start + offset;

        let Some(length) = page[open..].find('>') else {
            return false;
        };

        if page[open..open + length].contains(attribute) {
            return true;
        }

        start = open + length + 1;
    }

    false
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
    build(&workspace.site(), &out).expect("the build succeeds");

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
    build(&workspace.site(), &out).expect("the build succeeds");

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
    build(&workspace.site(), &out).expect("the build succeeds");

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
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(workspace.read("out/index.html").contains("<nav>menu</nav>"));
}

#[test]
fn the_output_is_emptied_before_anything_is_written() {
    let workspace = Workspace::new("build-fresh");
    minimal_site(&workspace);

    let out = workspace.join("out");
    std::fs::create_dir_all(out.join("viejo/deep")).expect("can create the output");
    std::fs::write(out.join("stale.html"), "viejo").expect("can write a stale file");
    std::fs::write(out.join("viejo/deep/otro.html"), "mas viejo").expect("can write it");

    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("index.html").exists());
    // The output holds exactly what this build produced: a page removed from
    // the templates does not survive from an earlier build.
    assert!(!out.join("stale.html").exists(), "a stale file survives");
    assert!(!out.join("viejo").exists(), "an old directory survives");
}

#[test]
fn a_site_with_no_pages_keeps_its_output() {
    let workspace = Workspace::new("build-empty-keeps-output");
    minimal_site(&workspace);
    std::fs::remove_file(workspace.join("templates/index.html")).expect("can remove the page");

    let out = workspace.join("out");
    std::fs::create_dir_all(&out).expect("can create the output");
    std::fs::write(out.join("stale.html"), "viejo").expect("can write a stale file");

    let error = match build(&workspace.site(), &out) {
        Err(error) => error,
        Ok(pages) => panic!("expected a no-pages error, built {pages}"),
    };
    assert!(matches!(error, BuildError::NoPages { .. }));

    assert!(
        out.join("stale.html").exists(),
        "the output was taken down even though there was nothing to build"
    );
}

#[test]
fn a_layout_is_never_built_as_a_page() {
    let workspace = Workspace::new("build-skips-layout");
    minimal_site(&workspace);

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(!out.join("layout.html").exists());
}

#[test]
fn partials_are_never_built_as_pages() {
    let workspace = Workspace::new("build-skips-partials");
    minimal_site(&workspace);
    workspace.write("templates/partials/card.html", "<div>carta</div>\n");

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(!out.join("card.html").exists());
}

#[test]
fn a_missing_layout_is_reported() {
    let workspace = Workspace::new("build-no-layout");
    workspace.write("machete.toml", "[data]\ntitle = \"x\"\n");
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>y</main>\n",
    );

    let out = workspace.join("out");
    let error = build(&workspace.site(), &out).expect_err("the build fails");
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
    let error = build(&workspace.site(), &out).expect_err("the build fails");
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
    let error = build(&workspace.site(), &out).expect_err("the build fails");
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
    let error = build(&workspace.site(), &out).expect_err("the build fails");
    assert!(
        format!("{error}").contains("inside a component"),
        "error was {error}"
    );
}

#[test]
fn a_directory_without_a_config_is_not_a_project() {
    // machete.toml is what identifies a directory as a machete project.
    // Without it, a forgotten variable would be blamed instead of the missing
    // file that caused it.
    let workspace = Workspace::new("build-no-config");
    workspace.write(
        "templates/layout.html",
        "<html><body><content/></body></html>\n",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>{title}</main>\n",
    );

    let out = workspace.join("out");
    let error = Config::load(&workspace.path).expect_err("no config");
    assert!(matches!(error, BuildError::NoConfig { .. }));

    let message = error.to_string();
    assert!(message.contains("machete.toml"), "message was {message}");
    assert!(message.contains("init"), "message was {message}");

    assert!(
        !out.join("index.html").exists(),
        "nothing should be written"
    );
}

#[test]
fn init_writes_every_scaffold_file() {
    let workspace = Workspace::new("init-files");
    let site = workspace.join("sitio");

    write(&site).expect("init writes the scaffold");

    for relative in [
        "machete.toml",
        "templates/layout.html",
        "templates/index.html",
        "templates/partials/head.html",
        "templates/partials/header.html",
        "assets/css/style.css",
    ] {
        assert!(site.join(relative).exists(), "{relative} is missing");
    }
}

#[test]
fn init_does_nothing_when_the_directory_exists() {
    let workspace = Workspace::new("init-existing");
    workspace.write("machete.toml", "[data]\ntitle = \"mio\"\n");

    write(&workspace.path).expect("init runs");

    assert_eq!(workspace.read("machete.toml"), "[data]\ntitle = \"mio\"\n");
    assert!(
        !workspace.join("templates/layout.html").exists(),
        "nothing from the scaffold was written"
    );
}

#[test]
fn the_generated_site_builds_to_the_same_page() {
    // The round trip matters: the scaffold embedded in the binary and the one
    // in the repository must not drift apart.
    let from_init = Workspace::new("roundtrip-init");
    let from_repo = Workspace::new("roundtrip-repo");
    let generado = from_init.join("sitio");

    write(&generado).expect("init writes the scaffold");
    copy_scaffold(Path::new("docs/scaffold"), &from_repo.path);

    let a = from_init.join("out");
    let b = from_repo.join("out");
    let config = Config::load(&generado).expect("the generated site loads");
    build(&config, &a).expect("the generated site builds");
    build(&from_repo.site(), &b).expect("the repository site builds");

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

    let css = to.join("assets/css");
    std::fs::create_dir_all(&css).expect("creates the assets directory");
    std::fs::copy(from.join("assets/css/style.css"), css.join("style.css"))
        .expect("copies the stylesheet");
}

#[test]
fn an_input_without_templates_is_reported() {
    // Pointing at templates/ instead of the site root is the easy mistake to
    // make, and it used to produce an empty output and exit successfully.
    // Here the inner directory holds the config, so what is missing is the
    // templates/ below it.
    let workspace = Workspace::new("build-wrong-input");
    workspace.write("templates/machete.toml", "title = \"x\"\n");

    let inner = workspace.join("templates");
    let error = Config::load(&inner).expect_err("not a site");
    assert!(matches!(error, BuildError::NotASite { .. }));
    let message = error.to_string();
    assert!(message.contains("not a site"), "message was {message}");
    assert!(message.contains("templates"), "message was {message}");
}

#[test]
fn pointing_at_the_templates_directory_is_reported_as_no_config() {
    // The likelier mistake is naming templates/ as the input, which now fails
    // on the missing config rather than on the missing templates.
    let workspace = Workspace::new("build-wrong-input-2");
    workspace.write("machete.toml", "[data]\ntitle = \"x\"\n");
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>y</main>\n",
    );

    let error = Config::load(&workspace.join("templates")).expect_err("no config");
    assert!(matches!(error, BuildError::NoConfig { .. }));
}

#[test]
fn a_site_with_templates_but_no_pages_is_reported() {
    let workspace = Workspace::new("build-no-pages");
    workspace.write("machete.toml", "[data]\ntitle = \"x\"\n");
    std::fs::create_dir_all(workspace.join("templates")).expect("creates templates");

    let out = workspace.join("out");
    let error = build(&workspace.site(), &out).expect_err("nothing to build");
    assert!(matches!(error, BuildError::NoPages { .. }));
    assert!(!out.join("index.html").exists());
}

#[test]
fn the_site_root_is_the_directory_holding_config_toml() {
    let workspace = Workspace::new("build-root");
    minimal_site(&workspace);

    // Building from the site root works and produces the page.
    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");
    assert!(out.join("index.html").exists());
}

#[test]
fn the_output_comes_from_the_configuration_by_default() {
    let workspace = Workspace::new("paths-config");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"machete\"\n\n[build]\npublic = \"sitio\"\n",
    );

    let config = workspace.site();
    let out = config.output_dir(None);

    assert_eq!(out, workspace.join("sitio"));
    build(&config, &out).expect("the build succeeds");
    assert!(workspace.join("sitio/index.html").exists());
}

#[test]
fn a_command_line_output_beats_the_configuration() {
    let workspace = Workspace::new("paths-override");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"machete\"\n\n[build]\npublic = \"sitio\"\n",
    );

    let config = workspace.site();
    // An override is taken as written, relative to wherever machete was run,
    // so it is not resolved against the site root.
    let out = config.output_dir(Some(Path::new("otro")));

    assert_eq!(out, Path::new("otro"));
}

#[test]
fn paths_and_data_are_reserved_and_never_become_variables() {
    let workspace = Workspace::new("reserved-sections");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"x\"\n\n[build]\noutput = \"public\"\n",
    );

    let config = workspace.site();
    assert_eq!(config.data.get("paths"), None);
    assert_eq!(config.data.get("data"), None);
    assert_eq!(config.data.get("title").map(String::as_str), Some("x"));
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
    workspace.write("machete.toml", "[data]\ntitle = \"x\"\n");

    let config = workspace.site();
    let out = config.output_dir(None);
    build(&config, &out).expect("the build succeeds");
    assert!(out.join("index.html").exists());
}

#[test]
fn a_page_keeps_the_directory_it_sits_in() {
    // `templates/blog/post.html` has to land in `public/blog/post.html`, or it
    // would overwrite `post.html` written from the root of templates/.
    let workspace = Workspace::new("paths-nested-page");
    minimal_site(&workspace);
    workspace.write(
        "templates/post.html",
        "<html type=\"layout\"/>\n<main>raiz</main>\n",
    );
    workspace.write(
        "templates/blog/post.html",
        "<html type=\"layout\"/>\n<main>blog</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("post.html").exists(), "root page missing");
    assert!(out.join("blog/post.html").exists(), "nested page missing");
    assert!(workspace.read("out/blog/post.html").contains("blog"));
}

/// Writes a site with a layout per name, so layout-specific data has
/// somewhere to land.
fn site_with_layouts(workspace: &Workspace, names: &[&str]) {
    workspace.write("machete.toml", "[data]\ntitle = \"base\"\n");
    workspace.write(
        "templates/layout.html",
        "<!DOCTYPE html>\n<html><body><content/></body></html>\n",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>{title}|{lang}</main>\n",
    );

    for name in names {
        workspace.write(
            &format!("templates/layouts/{name}.html"),
            &format!("<!DOCTYPE html>\n<html><body data=\"{name}\"><content/></body></html>\n"),
        );
        workspace.write(
            &format!("templates/{name}.html"),
            &format!("<html type=\"layout\" name=\"{name}\"/>\n<main>{{title}}|{{lang}}</main>\n"),
        );
    }
}

#[test]
fn layout_data_overrides_the_base() {
    let workspace = Workspace::new("data-override");
    site_with_layouts(&workspace, &["about"]);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"base\"\nlang = \"es\"\n\n[data.about]\ntitle = \"about\"\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    let about = workspace.read("out/about.html");
    assert!(about.contains("<main>about|es</main>"), "page was {about}");
    assert!(
        workspace
            .read("out/index.html")
            .contains("<main>base|es</main>")
    );
}

#[test]
fn layout_data_inherits_from_the_base() {
    let workspace = Workspace::new("data-inherit");
    site_with_layouts(&workspace, &["about"]);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"base\"\nlang = \"es\"\n\n[data.about]\ntitle = \"about\"\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    // about overrides title but keeps lang from [data]
    assert!(
        workspace
            .read("out/about.html")
            .contains("<main>about|es</main>"),
        "page was {}",
        workspace.read("out/about.html")
    );
}

#[test]
fn layouts_do_not_inherit_from_each_other() {
    let workspace = Workspace::new("data-siblings");
    site_with_layouts(&workspace, &["about", "gallery"]);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"base\"\nlang = \"es\"\n\n\
         [data.about]\ntitle = \"about\"\n\n[data.gallery]\ntitle = \"gallery\"\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(
        workspace
            .read("out/about.html")
            .contains("<main>about|es</main>")
    );
    assert!(
        workspace
            .read("out/gallery.html")
            .contains("<main>gallery|es</main>")
    );
}

#[test]
fn a_layout_with_no_section_of_its_own_sees_the_base() {
    let workspace = Workspace::new("data-no-section");
    site_with_layouts(&workspace, &["about"]);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"base\"\n\n[data.about]\ntitle = \"about\"\n",
    );

    let config = workspace.site();
    assert_eq!(config.scope_for(Some("gallery")).get("title"), Some("base"));
    assert_eq!(config.scope_for(Some("about")).get("title"), Some("about"));
    assert_eq!(config.scope_for(None).get("title"), Some("base"));
}

#[test]
fn a_loose_key_is_metadata_and_is_never_injected() {
    let workspace = Workspace::new("data-metadata");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "author = \"hash\"\nversion = \"1.2.3\"\n\n[data]\ntitle = \"machete\"\n",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<main>{title}</main>\n",
    );

    let config = workspace.site();

    // Kept, so the user can read them back.
    assert_eq!(
        config.metadata.get("author").map(String::as_str),
        Some("hash")
    );
    assert_eq!(
        config.metadata.get("version").map(String::as_str),
        Some("1.2.3")
    );

    // Not injected: the page only ever saw {title}.
    assert!(!config.data.contains_key("author"));

    let out = workspace.join("out");
    build(&config, &out).expect("the build succeeds");

    let page = workspace.read("out/index.html");
    assert!(!page.contains("hash"), "author reached the page: {page}");
}

#[test]
fn a_loose_key_is_never_a_variable() {
    let workspace = Workspace::new("data-metadata-unused");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "author = \"hash\"\n\n[data]\ntitle = \"machete\"\n",
    );

    assert!(!workspace.site().data.contains_key("author"));
}

#[test]
fn data_nested_a_level_too_deep_is_an_error() {
    let workspace = Workspace::new("data-nested");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"base\"\n\n[data.blog.post]\ntitle = \"post\"\n",
    );

    let error = Config::load(&workspace.path).expect_err("too deep");
    assert!(matches!(error, BuildError::NestedData { .. }));

    let message = error.to_string();
    assert!(
        message.contains("[data.blog.post]"),
        "message was {message}"
    );
    assert!(message.contains("one name"), "message was {message}");
}

#[test]
fn any_structure_the_user_writes_beside_data_is_left_alone() {
    // Machete reads [data], [data.*] and [build]. Anything else belongs to the
    // user and must not stop a build.
    let workspace = Workspace::new("data-other-structure");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[ci]\nrun = \"cargo test\"\nretries = 3\n\n\
         [pipeline.steps.build]\nimage = \"rust:1.99\"\n\n\
         [data]\ntitle = \"machete\"\n\n[build]\noutput = \"public\"\n",
    );

    let config = workspace.site();
    assert_eq!(
        config.data.get("title").map(String::as_str),
        Some("machete")
    );

    let out = workspace.join("out");
    build(&config, &out).expect("a foreign structure does not stop a build");
    assert!(out.join("index.html").exists());
}

#[test]
fn assets_are_copied_untouched() {
    let workspace = Workspace::new("assets-copy");
    minimal_site(&workspace);
    workspace.write("assets/main.css", "body { color: red }");
    workspace.write("assets/css/main.css", ".a { color: blue }");
    workspace.write("assets/bundle.js", "console.log(1)");
    workspace.write("assets/bundles/one.js", "console.log(2)");

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert_eq!(workspace.read("out/main.css"), "body { color: red }");
    assert_eq!(workspace.read("out/css/main.css"), ".a { color: blue }");
    assert_eq!(workspace.read("out/bundle.js"), "console.log(1)");
    assert_eq!(workspace.read("out/bundles/one.js"), "console.log(2)");
}

#[test]
fn a_site_without_assets_builds_fine() {
    let workspace = Workspace::new("assets-absent");
    minimal_site(&workspace);

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("index.html").exists());
}

#[test]
fn the_assets_directory_comes_from_the_configuration() {
    let workspace = Workspace::new("assets-config");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nassets = \"recursos\"\n",
    );
    workspace.write("recursos/estilo.css", "h1 {}");

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("estilo.css").exists());
}

/// Returns the `<loc>` values a sitemap lists, in order.
fn locations(sitemap: &str) -> Vec<String> {
    sitemap
        .lines()
        .filter(|line| line.contains("<loc>"))
        .map(|line| {
            line.trim()
                .trim_start_matches("<loc>")
                .trim_end_matches("</loc>")
                .to_string()
        })
        .collect()
}

#[test]
fn a_url_in_the_configuration_writes_a_sitemap() {
    let workspace = Workspace::new("sitemap-basic");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    let sitemap = workspace.read("out/sitemap.xml");
    assert!(
        sitemap.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "was {sitemap}"
    );
    assert_eq!(locations(&sitemap), ["https://ejemplo.com/"]);
}

#[test]
fn a_site_without_a_url_writes_no_sitemap() {
    let workspace = Workspace::new("sitemap-absent");
    minimal_site(&workspace);

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("index.html").exists(), "the page is still written");
    assert!(!out.join("sitemap.xml").exists(), "no sitemap is written");
}

#[test]
fn the_sitemap_can_be_turned_off_in_the_configuration() {
    let workspace = Workspace::new("sitemap-off");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\nsitemap = false\n",
    );
    workspace.write(
        "templates/about.html",
        "<html type=\"layout\"/><main>acerca</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(
        out.join("index.html").exists(),
        "the pages are still written"
    );
    assert!(
        out.join("about.html").exists(),
        "every page is still written"
    );
    assert!(!out.join("sitemap.xml").exists(), "no sitemap is written");
}

#[test]
fn the_sitemap_lists_every_page_at_the_address_it_is_served_from() {
    let workspace = Workspace::new("sitemap-pages");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\n",
    );
    workspace.write(
        "templates/about.html",
        "<html type=\"layout\"/><main>acerca</main>\n",
    );
    workspace.write(
        "templates/blog/post.html",
        "<html type=\"layout\"/><main>entrada</main>\n",
    );
    workspace.write(
        "templates/blog/index.html",
        "<html type=\"layout\"/><main>blog</main>\n",
    );
    workspace.write("assets/estilo.css", "h1 {}");

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert_eq!(
        locations(&workspace.read("out/sitemap.xml")),
        [
            "https://ejemplo.com/about.html",
            "https://ejemplo.com/blog/",
            "https://ejemplo.com/blog/post.html",
            "https://ejemplo.com/",
        ]
    );
}

#[test]
fn the_sitemap_leaves_out_layouts_components_and_assets() {
    let workspace = Workspace::new("sitemap-only-pages");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\n",
    );
    workspace.write(
        "templates/layouts/otro.html",
        "<html><body><content/></body></html>\n",
    );
    workspace.write("templates/partials/pie.html", "<footer>pie</footer>");
    workspace.write("assets/estilo.css", "h1 {}");

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert_eq!(
        locations(&workspace.read("out/sitemap.xml")),
        ["https://ejemplo.com/"]
    );
}

#[test]
fn a_page_asking_not_to_be_indexed_is_left_out_of_the_sitemap() {
    let workspace = Workspace::new("sitemap-noindex");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\n",
    );
    workspace.write(
        "templates/404.html",
        "<html type=\"layout\"/>\n<head><meta name=\"robots\" content=\"noindex\"></head>\n<main>no</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("404.html").exists(), "the page is still written");
    assert_eq!(
        locations(&workspace.read("out/sitemap.xml")),
        ["https://ejemplo.com/"]
    );
}

#[test]
fn noindex_is_read_from_the_whole_content_attribute() {
    let workspace = Workspace::new("sitemap-noindex-list");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\n",
    );
    workspace.write(
        "templates/a.html",
        "<html type=\"layout\"/>\n<head><meta name=\"ROBOTS\" content=\"nofollow, NoIndex\"></head>\n<main>a</main>\n",
    );
    workspace.write(
        "templates/b.html",
        "<html type=\"layout\"/>\n<head><meta name=\"robots\" content=\"index, follow\"></head>\n<main>b</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert_eq!(
        locations(&workspace.read("out/sitemap.xml")),
        ["https://ejemplo.com/b.html", "https://ejemplo.com/"]
    );
}

#[test]
fn noindex_added_by_a_component_still_removes_the_page() {
    let workspace = Workspace::new("sitemap-noindex-component");
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\n",
    );
    workspace.write(
        "templates/layout.html",
        "<!DOCTYPE html>\n<html>\n<body>\n<content/>\n</body>\n</html>\n",
    );
    workspace.write(
        "templates/partials/cabeza.html",
        "<meta name=\"robots\" content=\"noindex\">",
    );
    workspace.write(
        "templates/index.html",
        "<html type=\"layout\"/>\n<head><cabeza type=\"component\"/></head>\n<main>hola</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("index.html").exists());
    assert_eq!(
        locations(&workspace.read("out/sitemap.xml")),
        Vec::<String>::new()
    );
}

#[test]
fn a_trailing_slash_in_the_url_does_not_produce_a_double_slash() {
    let workspace = Workspace::new("sitemap-slash");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com/\"\n",
    );
    workspace.write(
        "templates/about.html",
        "<html type=\"layout\"/><main>acerca</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert_eq!(
        locations(&workspace.read("out/sitemap.xml")),
        ["https://ejemplo.com/about.html", "https://ejemplo.com/"]
    );
}

#[test]
fn the_sitemap_the_build_writes_wins_over_one_shipped_as_an_asset() {
    let workspace = Workspace::new("sitemap-override");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\nurl = \"https://ejemplo.com\"\n",
    );
    workspace.write("assets/sitemap.xml", "<urlset>obsoleto</urlset>");

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    let sitemap = workspace.read("out/sitemap.xml");
    assert!(sitemap.contains("https://ejemplo.com/"), "was {sitemap}");
    assert!(!sitemap.contains("obsoleto"), "was {sitemap}");
}

#[test]
fn the_templates_directory_comes_from_the_configuration() {
    let workspace = Workspace::new("templates-config");
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\ntemplates = \"plantillas\"\n",
    );
    workspace.write(
        "plantillas/layout.html",
        "<html><body><content/></body></html>\n",
    );
    workspace.write(
        "plantillas/index.html",
        "<html type=\"layout\"/>\n<main>x</main>\n",
    );

    let out = workspace.join("out");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("index.html").exists());
}

#[test]
fn a_flag_beats_the_configuration() {
    let workspace = Workspace::new("assets-flag");
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\ntemplates = \"plantillas\"\n",
    );
    workspace.write(
        "plantillas/layout.html",
        "<html><body><content/></body></html>\n",
    );
    workspace.write(
        "plantillas/index.html",
        "<html type=\"layout\"/>\n<main>x</main>\n",
    );
    workspace.write("otro/layout.html", "<html><body><content/></body></html>\n");
    workspace.write(
        "otro/index.html",
        "<html type=\"layout\"/>\n<main>otro</main>\n",
    );

    let overrides = Overrides {
        templates: Some(PathBuf::from("otro")),
        ..Default::default()
    };
    let config = Config::load_with(&workspace.path, &overrides).expect("loads");

    assert_eq!(config.templates, workspace.join("otro"));

    let out = workspace.join("out");
    build(&config, &out).expect("the build succeeds");
    assert!(out.join("index.html").exists());
}

#[test]
fn an_output_of_dot_is_refused_before_anything_is_deleted() {
    let workspace = Workspace::new("build-output-dot");
    minimal_site(&workspace);
    workspace.write(
        "machete.toml",
        "[data]\ntitle = \"t\"\n\n[build]\npublic = \".\"\n",
    );

    let out = workspace.join(".");
    let error = match build(&workspace.site(), &out) {
        Err(error) => error,
        Ok(pages) => panic!("the build ran and wrote {pages} pages"),
    };

    assert!(matches!(error, BuildError::OutputInsideSite { .. }));
    // The whole point is that it stops before the templates are removed.
    assert!(
        workspace.join("templates/index.html").exists(),
        "the templates were touched"
    );
}

#[test]
fn an_output_over_the_templates_is_refused() {
    let workspace = Workspace::new("build-output-templates");
    minimal_site(&workspace);

    let out = workspace.join("templates");
    let error = match build(&workspace.site(), &out) {
        Err(error) => error,
        Ok(pages) => panic!("the build ran and wrote {pages} pages"),
    };

    assert!(matches!(error, BuildError::OutputInsideSite { .. }));
    assert!(workspace.join("templates/layout.html").exists());
}

#[test]
fn a_sibling_of_the_site_is_still_a_fine_output() {
    let workspace = Workspace::new("build-output-sibling");
    minimal_site(&workspace);

    let out = workspace.join("dist");
    build(&workspace.site(), &out).expect("the build succeeds");

    assert!(out.join("index.html").exists());
}

#[test]
fn init_reports_that_it_ran() {
    let workspace = Workspace::new("init-reports");
    let site = workspace.join("sitio");

    write(&site).expect("init runs");

    assert!(site.join("machete.toml").exists());
}
