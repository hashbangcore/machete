//! The record of what a build wrote into the output directory.
//!
//! An output directory is often shared. A bundler puts `bundle.js` there, a
//! fingerprinting step rewrites it, somebody drops a `robots.txt` in by hand.
//! Emptying the directory took their files with it, along with the stale ones
//! the build was there to clear.
//!
//! This module keeps the other half of that promise. Every build writes down
//! the paths it produced, in `.manifest.toml` inside the output, and the next
//! build removes exactly those before writing again. A page deleted from
//! `templates/` still disappears from `public/`; the file another tool wrote
//! next to it stays where it is.
//!
//! With no record there is nothing to tell a stale page from a file that was
//! always meant to be there, so a build that finds none empties the output the
//! way it did before. That happens once, on the first build after this change,
//! and the build that empties it writes the record that keeps it from
//! happening again.

use crate::error::{BuildError, IoError};
use crate::report;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// The file the record is kept in, inside the output directory.
///
/// Hidden, and beside what it describes rather than in a place that has to be
/// configured: the output can be moved, renamed or pointed somewhere else on
/// the command line, and the record has to follow it on its own.
///
/// The record does not carry the name of the tool that wrote it. The output
/// belongs to whoever publishes it, and a file in there naming a build tool is
/// one more thing to answer for; the contents say what it is anyway.
pub const FILE: &str = ".manifest.toml";

/// What the output directory says about a build that came before.
///
/// Reading the record is not allowed to fail the build: a missing one means the
/// output is not a machete output yet, and a damaged one is a reason to leave
/// the directory alone, never a reason to take it down.
#[derive(Debug)]
enum Previous {
    /// No record: the output was never written by machete, or was cleared by hand.
    None,
    /// A record that is there and cannot be used, with the reason why.
    Unreadable(String),
    /// The paths a previous build wrote.
    Files(Vec<PathBuf>),
}

/// Takes the output directory to the state this build writes into.
///
/// The files a previous build wrote are removed, so a page or an asset deleted
/// from the sources does not survive from an earlier build. Anything machete
/// never wrote is left alone: the directory is shared with whatever else the
/// site is built with, and emptying it would take their work along with the
/// stale files.
pub fn prepare(output: &Path) -> Result<(), BuildError> {
    match read(output) {
        Previous::Files(files) => clean(output, &files),
        Previous::Unreadable(reason) => {
            report::warning(&format!(
                "{} could not be read, so nothing was removed ({reason})",
                path_to(output).display()
            ));
        }

        // Nothing has ever said what belongs here, so everything is stale.
        // The record written at the end of this build ends the question.
        Previous::None => {
            if output.exists() {
                fs::remove_dir_all(output)
                    .map_err(|source| BuildError::Io(IoError::new("remove", output, source)))?;
            }
        }
    }

    fs::create_dir_all(output)
        .map_err(|source| BuildError::Io(IoError::new("create directory", output, source)))?;

    Ok(())
}

/// Writes down the paths this build puts in the output.
///
/// Written before the files rather than after, so that a build interrupted
/// halfway is still described by its record. A path that is left behind but was
/// never created is not a problem: removing a file that is not there is not an
/// error.
pub fn write(output: &Path, files: &[PathBuf]) -> Result<(), BuildError> {
    let mut table = toml::Table::new();

    let mut paths: Vec<String> = Vec::with_capacity(files.len());

    for relative in files {
        // Only paths inside the output, the same rule `clean` applies when it
        // reads the record back.
        if !is_inside(relative) {
            continue;
        }

        // A name that is not text cannot be written down. The file is still
        // written; it is only left unrecorded, and said out loud so the gap is
        // not discovered later as a page that never goes away.
        let Some(path) = relative.to_str() else {
            report::warning(&format!(
                "{} is not valid text and is not recorded in the manifest",
                relative.display()
            ));
            continue;
        };

        // Sorted and without repeats: an asset the sitemap overwrites, or a
        // path reached twice, is one file, and the record should say so.
        if !paths.iter().any(|held| held == path) {
            paths.push(path.to_string());
        }
    }

    paths.sort();

    table.insert(
        "files".into(),
        toml::Value::Array(paths.into_iter().map(toml::Value::String).collect()),
    );

    let target = path_to(output);

    let text = toml::to_string(&table).map_err(|source| {
        BuildError::Io(IoError::new(
            "write manifest",
            &target,
            io::Error::other(source),
        ))
    })?;

    fs::write(&target, text)
        .map_err(|source| BuildError::Io(IoError::new("write manifest", target, source)))
}

/// Where the record lives inside an output directory.
fn path_to(output: &Path) -> PathBuf {
    output.join(FILE)
}

/// Reads the record out of an output directory.
fn read(output: &Path) -> Previous {
    let path = path_to(output);

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Previous::None,
        Err(source) => {
            return Previous::Unreadable(source.to_string());
        }
    };

    let table: toml::Table = match toml::from_str(&text) {
        Ok(table) => table,
        Err(source) => return Previous::Unreadable(source.message().to_string()),
    };

    let Some(files) = table.get("files").and_then(toml::Value::as_array) else {
        return Previous::Unreadable("no list of files".to_string());
    };

    let paths = files
        .iter()
        .filter_map(toml::Value::as_str)
        .map(PathBuf::from)
        .collect();

    Previous::Files(paths)
}

/// Removes the files a previous build wrote.
///
/// A file that is already gone is not a failure. It is what a build that
/// stopped halfway leaves behind, and the point of the record is to converge
/// rather than to complain.
fn clean(output: &Path, files: &[PathBuf]) {
    for relative in files {
        if !is_inside(relative) {
            continue;
        }

        let path = output.join(relative);

        // `symlink_metadata` rather than `is_dir`, because a link to a
        // directory is not one: it is a link, and it is a file machete wrote
        // that something else replaced.
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {
                report::warning(&format!(
                    "{} is a directory and was left alone",
                    path.display()
                ));
                continue;
            }

            Ok(_) => {
                if let Err(source) = fs::remove_file(&path) {
                    report::warning(&format!("could not remove {}: {source}", path.display()));
                }
            }

            // Already gone.
            Err(_) => continue,
        }

        prune(output, relative);
    }
}

/// Removes the directories a written file leaves empty once it is gone.
///
/// Only empty ones go. A directory still holding a file from another tool
/// belongs to that tool, and taking it because machete emptied its neighbour
/// would be the same mistake as deleting the file.
fn prune(output: &Path, relative: &Path) {
    let Some(parent) = relative.parent() else {
        return;
    };

    if parent.as_os_str().is_empty() {
        return;
    }

    // Deepest first, so `blog/2024/post.html` empties `2024` and then `blog`.
    let mut current = output.to_path_buf();

    for component in parent.components() {
        current.push(component);

        let empty = fs::read_dir(&current)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);

        if !empty || fs::remove_dir(&current).is_err() {
            return;
        }
    }
}

/// Reports whether a recorded path stays inside the output directory.
///
/// The record is written by machete, but it is a file on disk that anyone can
/// edit, and a name like `../robots.txt` in it must not turn a build into
/// something that walks out of the directory it was given.
fn is_inside(relative: &Path) -> bool {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return false;
    }

    relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_out_of_the_output_is_not_one_of_ours() {
        assert!(is_inside(Path::new("index.html")));
        assert!(is_inside(Path::new("blog/post.html")));

        assert!(!is_inside(Path::new("")));
        assert!(!is_inside(Path::new("..")));
        assert!(!is_inside(Path::new("../robots.txt")));
        assert!(!is_inside(Path::new("blog/../../robots.txt")));
        assert!(!is_inside(Path::new("/etc/passwd")));
    }

    #[test]
    fn a_written_record_holds_the_paths() {
        let dir = std::env::temp_dir().join(format!("machete-manifest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("can create the directory");

        write(
            &dir,
            &[
                PathBuf::from("index.html"),
                PathBuf::from("sitemap.xml"),
                PathBuf::from("index.html"),
                PathBuf::from("../fuera.txt"),
            ],
        )
        .expect("the record is written");

        let text = fs::read_to_string(path_to(&dir)).expect("can read it back");

        // One line, sorted, without the repeat and without the path that
        // climbs out. A record is read by people when something has gone wrong.
        assert_eq!(text.trim(), "files = [\"index.html\", \"sitemap.xml\"]");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_absent_record_says_so() {
        let dir =
            std::env::temp_dir().join(format!("machete-manifest-none-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("can create the directory");

        assert!(matches!(read(&dir), Previous::None));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_record_that_is_not_ours_is_not_guessed_at() {
        let dir = std::env::temp_dir().join(format!("machete-manifest-old-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("can create the directory");

        // Broken, and a file of something else that happens to sit there.
        fs::write(path_to(&dir), "esto no es toml {{\n").expect("can write it");
        assert!(matches!(read(&dir), Previous::Unreadable(_)));

        fs::write(path_to(&dir), "otro = 1\n").expect("can write it");
        assert!(matches!(read(&dir), Previous::Unreadable(_)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_record_reads_back_the_paths_it_was_written_with() {
        let dir =
            std::env::temp_dir().join(format!("machete-manifest-read-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("can create the directory");

        write(&dir, &[PathBuf::from("blog/post.html")]).expect("the record is written");

        match read(&dir) {
            Previous::Files(files) => assert_eq!(files, vec![PathBuf::from("blog/post.html")]),
            other => panic!("expected the files, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
