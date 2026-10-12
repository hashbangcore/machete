//! Rendering a sitemap.
//!
//! The protocol asks for absolute URLs, so a sitemap is only possible once the
//! site says what it is published at. Everything here is a function from input
//! to output: the caller has already written the pages to disk and passes the
//! paths it wrote, which is what keeps the sitemap from advertising a URL that
//! does not exist.
//!
//! Only pages are listed. Static files are part of the site but not things a
//! search engine indexes, and listing them adds noise without adding reach.
//!
//! No `lastmod` is written. The protocol treats it as optional and Machete has
//! no source of dates that would not mislead: a template's modification time
//! moves whenever the file is checked out, and says nothing about whether the
//! page it produces changed.

use std::path::{Component, Path, PathBuf};

/// The index file that stands for a directory rather than for itself.
const INDEX: &str = "index.html";

/// Renders a sitemap for `base`, listing the pages written under it.
///
/// `pages` are paths relative to the output directory, as they were written.
/// They are listed in the order given, so a caller that sorted its pages gets a
/// stable file.
pub fn render(base: &str, pages: &[PathBuf]) -> String {
    let base = base.trim().trim_end_matches('/');

    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );

    for page in pages {
        out.push_str("  <url>\n    <loc>");
        out.push_str(&escape(base));
        // Percent-encoding leaves only unreserved characters, so the path needs
        // no XML escaping of its own.
        out.push_str(&loc_path(page));
        out.push_str("</loc>\n  </url>\n");
    }

    out.push_str("</urlset>\n");
    out
}

/// Returns the URL path a written page is served at, starting with a slash.
///
/// An `index.html` stands for the directory holding it, because that is the
/// address a reader types and the one a server answers: `index.html` becomes
/// `/` and `blog/index.html` becomes `/blog/`.
fn loc_path(page: &Path) -> String {
    let segments: Vec<String> = page
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    let mut path = String::from("/");

    for (position, segment) in segments.iter().enumerate() {
        let is_last = position + 1 == segments.len();

        if is_last && segment == INDEX {
            if path.len() > 1 {
                path.push('/');
            }
            continue;
        }

        if path.len() > 1 {
            path.push('/');
        }

        path.push_str(&encode(segment));
    }

    path
}

/// Percent-encodes one path segment.
///
/// Each segment is encoded on its own, so the `/` that separates them survives.
/// Everything outside the unreserved set of RFC 3986 becomes `%XX`, which also
/// takes care of the characters that would otherwise be ambiguous in a URL or
/// illegal in XML.
fn encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());

    for byte in segment.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }

    out
}

/// Escapes the five characters XML reserves.
///
/// Only the base is escaped, because it comes from the configuration file and is
/// written through as it was given.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(character),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    fn locs(base: &str, names: &[&str]) -> Vec<String> {
        render(base, &pages(names))
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
    fn the_index_at_the_root_becomes_the_site_root() {
        assert_eq!(
            locs("https://ejemplo.com", &["index.html"]),
            ["https://ejemplo.com/"]
        );
    }

    #[test]
    fn an_index_below_the_root_becomes_its_directory() {
        assert_eq!(
            locs("https://ejemplo.com", &["blog/index.html"]),
            ["https://ejemplo.com/blog/"]
        );
    }

    #[test]
    fn an_ordinary_page_keeps_its_name() {
        assert_eq!(
            locs("https://ejemplo.com", &["acerca.html"]),
            ["https://ejemplo.com/acerca.html"]
        );
    }

    #[test]
    fn nested_pages_keep_their_directories() {
        assert_eq!(
            locs("https://ejemplo.com", &["blog/2026/uno.html"]),
            ["https://ejemplo.com/blog/2026/uno.html"]
        );
    }

    #[test]
    fn a_trailing_slash_in_the_base_does_not_double() {
        assert_eq!(
            locs("https://ejemplo.com/", &["acerca.html"]),
            ["https://ejemplo.com/acerca.html"]
        );
    }

    #[test]
    fn a_base_with_a_path_is_kept() {
        assert_eq!(
            locs("https://ejemplo.com/proyecto/", &["acerca.html"]),
            ["https://ejemplo.com/proyecto/acerca.html"]
        );
    }

    #[test]
    fn spaces_are_encoded() {
        assert_eq!(
            locs("https://ejemplo.com", &["acerca de machete.html"]),
            ["https://ejemplo.com/acerca%20de%20machete.html"]
        );
    }

    #[test]
    fn characters_that_are_special_in_urls_become_percent_sequences() {
        assert_eq!(
            locs("https://ejemplo.com", &["a&b.html"]),
            ["https://ejemplo.com/a%26b.html"]
        );
    }

    #[test]
    fn non_ascii_is_encoded_as_utf8() {
        assert_eq!(
            locs("https://ejemplo.com", &["ñ.html"]),
            ["https://ejemplo.com/%C3%B1.html"]
        );
    }

    #[test]
    fn the_base_is_escaped_because_it_comes_from_the_config_file() {
        assert_eq!(escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
    }

    #[test]
    fn a_site_with_no_pages_still_produces_a_valid_document() {
        let xml = render("https://ejemplo.com", &[]);
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(xml.contains("<urlset"));
        assert!(xml.trim_end().ends_with("</urlset>"));
        assert!(!xml.contains("<url>"));
    }

    #[test]
    fn pages_are_listed_in_the_order_they_are_given() {
        assert_eq!(
            locs("https://ejemplo.com", &["b.html", "a.html"]),
            ["https://ejemplo.com/b.html", "https://ejemplo.com/a.html"]
        );
    }
}
