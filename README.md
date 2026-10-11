# Machete

A static site generator for composing HTML out of components, layouts and
variables.

You write fragments. Machete wraps them into finished pages.

```html
<!-- templates/index.html -->
<html type="layout"/>

<main>
  <h1>{title}</h1>
  <figure type="component">
    <p>Everything inside goes where content sits.</p>
  </figure>
</main>
```

```bash
machete build
```

```html
<!-- public/index.html -->
<!DOCTYPE html>
<html lang="en">
<head><title>machete</title></head>
<body>
  <main>
    <h1>machete</h1>
    <figure class="note"><p>Everything inside goes where content sits.</p></figure>
  </main>
</body>
</html>
```

The output is plain HTML. None of Machete's constructs reach it.

## Install

```
cargo install --path .
```

## Commands

| Command | What it does |
|---|---|
| `machete build [input:output]` | Expands a site into finished pages |
| `machete build -f [input:output]` | Empties the output first, so stale pages cannot survive |
| `machete init [dir]` | Writes a new site |
| `machete init -f [dir]` | Overwrites files that already exist |
| `machete completions <shell>` | Prints a completion script for bash, zsh, fish, elvish or powershell |

### build takes `input:output`

The shape Sass uses. Either side may be left empty, and an empty side is filled
in from `machete.toml` rather than defaulted here:

```
machete build                 both from machete.toml
machete build :dist           output only
machete build scaffold:       input only
machete build scaffold:dist   both
```

A bare path without a colon is rejected: it would not say whether it is the
source or the destination.

Machete reads the directory it is given and nothing above it, so the root is
always the one you name, or the current directory.

### init takes one path

```
machete init                        the current directory
machete init new-site               new-site/
machete init output/dir/new-site    the whole path, parents included
```

## Site layout

```
.
├── machete.toml
└── templates/
    ├── layout.html
    ├── index.html
    ├── layouts/          optional, for named layouts
    └── partials/         component definitions, one per file
```

Every `.html` file under `templates/` becomes a page, except `layout.html`, the
ones in `layouts/`, and the component definitions in `partials/`. A page keeps
the directory it sits in, so `templates/blog/post.html` is written to
`public/blog/post.html`.

`machete.toml` is what identifies the directory as a machete project. Building a
directory without one is an error rather than a build with empty globals, so a
forgotten variable is never reported when the real cause is a missing file.
Machete ignores a `config.toml` that belongs to some other tool.

## Components

Any tag carrying `type="component"` invokes a component. The tag name selects
the file in `partials/`.

```html
<header type="component">
  <nav>
    <a href="/">Home</a>
  </nav>
</header>
```

A component that needs no content may be written self-closing:

```html
<head type="component" title="{title}"/>
```

### Defining one

```
<!-- templates/partials/header.html -->
<header>
  <h1>{title}</h1>
  <nav><content/></nav>
</header>
```

One component per file. The name is the file name.

### Variables

Values reach a component either through attributes or through a data block.
Attributes suit one or two values; a data block suits many.

```html
<nav type="component" url="/about.html" label="About"></nav>

<header type="component">
  <script type="data">
    title = "My site";
    url = "https://example.com";
  </script>
</header>
```

Every attribute other than `type` becomes a variable and is consumed, so it does
not reach the output. Naming the same variable twice, by attribute and by data
block, is an error.

A data block is not JavaScript and is never run as such. It lives inside a
`<script>` only so the editor highlights it and recognises its literals.
`const`, `let` and `var` are all accepted and all optional, which keeps the
block valid JavaScript either way.

```html
<script type="data">
  title = "My site";          // fine
  const url = "/a";           // also fine
</script>
```

Every value is a string. There is no type inference, so `flag = true` gives you
the text `true`.

A data block only makes sense inside a component. One written anywhere else is
an error.

### Content

`<content/>` marks where the HTML passed to the component goes. The marker
disappears from the output; the content takes its place.

```
<!-- templates/partials/figure.html -->
<figure class="note">
  <content/>
</figure>
```

```html
<figure type="component">
  <p>This text enters at the marker.</p>
</figure>
```

Content is rendered inside the component's scope, so it can use the values the
component was given.

A definition may declare `<content/>` once, or not at all. Passing content to a
definition with no marker is an error, and so is declaring it twice, because the
first would silently lose content and the second has no single destination.

## Layouts

A page asks for a layout with `<html type="layout"/>`. Without a `name` it gets
`templates/layout.html`:

```html
<html type="layout"/>

<main>
  <h1>My site</h1>
</main>
```

With a `name` it gets `templates/layouts/<name>.html`:

```html
<html type="layout" name="article"/>
```

Asking for a layout that does not exist is an error.

The layout is an ordinary document with a `<content/>` where the page goes.

```html
<!DOCTYPE html>
<html lang="en">
<head type="component" title="{title}"/>

<body>
  <content/>
</body>
</html>
```

The page is inserted before components are expanded, because the marker may sit
inside a component invocation. Expanding first would carry the marker into the
definition and leave nowhere to put the page.

## Variables

Global variables are declared in `machete.toml`. The keys at the top level are
the globals, and they reach every page, layout and component:

```toml
title = "machete"

[paths]
output = "public"
```

`paths` is reserved: its keys configure where things live and never become
variables. The output defaults to `public/` when it is not given.

Local variables belong to a single component invocation, through its attributes
or its data block. They do not change the globals.

When both carry the same name, the local one wins. The global keeps its value
for the rest of the document.

A variable with no value is an error. The build stops rather than leaving
`{title}` in the page.

## Naming

A component's name is its file name. Machete does not check that the name is a
valid HTML tag, because a linter can advise on that and the author decides.

```
templates/partials/002-mmenu.html   →   <002-mmenu type="component"/>
```

The only time Machete reports a naming problem is when the component does not
exist.

## Self-closing

`<content/>`, `type="component"` and `type="layout"` close themselves, like the
void elements. Every other tag follows the HTML standard, where a trailing slash
on a non-void element is ignored and the element stays open. So `<div/>` is an
error.

This is the one place Machete departs from the HTML spec, and it has to: in
standard HTML the slash is ignored on `<content/>`, which would leave an
element open and swallow the rest of the document.

## Output

The pages written to the output directory are standard HTML. They carry no
`<content/>`, no `type="component"`, no `type="layout"` and no
`<script type="data">`. They can be served as they are.

A validation pass fails the build if any of it survives, so a page can never be
written while still holding Machete markup.

Ordinary JavaScript and CSS are preserved untouched. A `{` inside a `<script>`
or a `<style>` belongs to that language, not to Machete. A `<title>` or a
`<textarea>` also holds raw text, but its braces are prose, so they resolve
like any other.

## What fails a build

Machete stops rather than writing a page half-rendered.

- A component invoked with no definition
- A layout asked for by name that does not exist
- A variable with no value
- The same variable given twice to one component
- Content passed to a definition with no `<content/>`
- A definition declaring `<content/>` more than once
- A `<script type="data">` outside a component
- A tag left open, or written self-closing when the HTML standard says it is not
- An `input:output` with no colon, or with more than one
- An input directory with no `machete.toml` in it
- An input directory with no `templates/` inside it

## Development

```
cargo test              # 125 tests
cargo clippy --all-targets
cargo fmt
just build              # build the scaffold into ./outputs/build-example
just init               # write a new site into ./outputs/init-example
just install            # cargo install --path .
```

## Not in scope

Loops, conditionals, data processing, running JavaScript. Machete composes
HTML; it does not try to replace esbuild or Deno.