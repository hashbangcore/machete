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
machete build -i ./scaffold -o ./public
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

Paths are given either as bare arguments or as flags, never both. Mixing the
two is an error rather than a silent preference, because neither reading is
obviously the one that was meant. Whatever is left unset falls back to
`config.toml`.

```
machete build                 both paths from config.toml
machete build public          output only
machete build . public        input and output
machete build -i . -o public  the flags
```

| Command | What it does |
|---|---|
| `machete build -i <site> -o <out>` | Expands a site into finished pages |
| `machete build -i <site> -o <out> -f` | Empties the output first, so stale pages cannot survive |
| `machete init <dir>` | Writes a new site |
| `machete init -o <dir> -f` | Overwrites files that already exist |
| `machete completions <shell>` | Prints a completion script for bash, zsh, fish, elvish or powershell |

## Site layout

```
.
├── config.toml
└── templates/
    ├── layout.html
    ├── index.html
    ├── layouts/          optional, for named layouts
    └── partials/         component definitions, one per file
```

`templates/partials/header.html` defines the component `<header>`.

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
not reach the output.

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
definition with no marker is an error, and so is declaring it twice, because
the first would silently lose content and the second has no single destination.

## Layouts

A page asks for a layout with `<html type="layout"/>`. Without a `name` it gets
the default one:

```html
<html type="layout"/>

<main>
  <h1>My site</h1>
</main>
```

With a name it gets one from `templates/layouts/`:

```html
<html type="layout" name="article"/>
```

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

Global variables are declared in `config.toml`. The keys at the top level are
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

## Development

```
cargo test          # 126 tests
cargo clippy --all-targets
cargo fmt
just build          # build the scaffold into ./outputs/build-example
just init           # write a new site into ./outputs/init-example
```

## Not in scope

Loops, conditionals, data processing, running JavaScript. Machete composes
HTML; it does not try to replace esbuild or Deno.