use machete::compose::{Defs, expand};
use machete::html::{parse, render};
use machete::vars::Scope;
use std::collections::BTreeMap;
use std::fs;

fn defs() -> Defs {
    let mut d = Defs::new();
    for name in ["head", "aside", "figure"] {
        let path = format!("scaffold/templates/partials/{name}.html");
        let src = fs::read_to_string(&path).unwrap();
        d.insert(name.to_string(), parse(&src).unwrap());
    }
    d
}

fn globals() -> Scope {
    let mut g = BTreeMap::new();
    g.insert("title".to_string(), "machete".to_string());
    Scope::new(g)
}

fn run(label: &str, html: &str) {
    print!("{label:<28}");
    match parse(html) {
        Ok(tree) => match expand(&tree, &defs(), &globals()) {
            Ok(out) => println!("{}", render(&out)),
            Err(e) => println!("EXPAND ERROR: {e}"),
        },
        Err(e) => println!("PARSE ERROR: {e}"),
    }
}

fn main() {
    run(
        "aside (script)",
        r#"<aside type="component"><script type="component">const title = "t"; const link="/x"; const label="y"; const text="z";</script></aside>"#,
    );
    run(
        "figure (content)",
        r#"<figure type="component"><p>hola</p></figure>"#,
    );
    run(
        "layout head (auto-cerrado)",
        r#"<head type="component" title="{title}"/>"#,
    );
    run("layout directive", r#"<html type="layout"/>"#);
}
