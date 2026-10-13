//! What a run says about itself.
//!
//! Everything that identifies the run is worked out once, when the program
//! starts, and kept in a single place. The clock is the clearest case: read it
//! on every event and two events in the same second could disagree about when
//! they happened, or worse, about the date if one crossed midnight.
//!
//! The shape of a report follows what other tools do. Warnings and errors are
//! prefixed and coloured, as rustc does, and a finished run ends with one line
//! saying what it produced, as cargo does with its count and its time. The
//! colours are only used when standard error is a terminal; a pipe gets the
//! words alone.
//!
//! JSON carries the same facts in fields rather than in a sentence, so both
//! forms say the same thing in the way their reader expects.

use clap::ValueEnum;
use std::io::IsTerminal;
use std::path::{Path, absolute};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// How a run writes down what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Plain,
    Json,
}

/// The run's identity, resolved once at startup.
struct Run {
    format: Format,
    quiet: bool,
    /// Only turned on when someone is there to see it.
    color: bool,
    /// Resolved here and nowhere else, so every event reads the same one.
    timestamp: String,
    /// When the program started, to time the run against.
    started: Instant,
    command: &'static str,
    input: String,
}

/// Set once, when the program starts, and read by everything that reports.
static RUN: OnceLock<Mutex<Run>> = OnceLock::new();

/// Records how this run should report, and when it happened.
///
/// Called once, before anything else runs. The clock is read here rather than
/// at each event, and so is the decision of whether anyone is watching.
pub fn start(format: Format, quiet: bool) {
    let _ = RUN.set(Mutex::new(Run {
        format,
        quiet,
        color: std::io::stderr().is_terminal(),
        timestamp: iso8601(SystemTime::now()),
        started: Instant::now(),
        command: "",
        input: String::new(),
    }));
}

/// Names the command and the site it runs on.
///
/// A second resolution, because the arguments are only known once they are
/// parsed. It happens once too, so the value does not change halfway through.
pub fn identify(command: &'static str, input: &Path) {
    let Some(run) = RUN.get() else {
        return;
    };
    let Ok(mut run) = run.lock() else {
        return;
    };

    run.command = command;
    run.input = input.display().to_string();
}

/// The format this run reports in.
pub fn format() -> Format {
    with_run(|run| run.format).unwrap_or(Format::Plain)
}

/// Whether this run says nothing at all.
///
/// `--quiet` wins over `--format`, so nothing is written whichever format was
/// named.
pub fn is_quiet() -> bool {
    with_run(|run| run.quiet).unwrap_or(false)
}

/// Whether the run may use colour, because there is a terminal to show it on.
///
/// Only plain text is coloured. JSON carries its own structure, and escape
/// codes inside it would break anything trying to read it.
pub fn use_color() -> bool {
    with_run(|run| run.color && run.format == Format::Plain).unwrap_or(false)
}

/// Reads the run under its lock, or `None` when nobody has started one.
///
/// Tests call the reporting functions without a run, and a missing report is
/// simply nothing to say rather than a failure.
fn with_run<T>(leer: impl FnOnce(&Run) -> T) -> Option<T> {
    let run = RUN.get()?;
    let run = run.lock().ok()?;

    Some(leer(&run))
}

/// Paints a line, or leaves it alone when nobody is watching.
fn paint(color: bool, code: &str, text: &str) -> String {
    if color {
        format!("{code}{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

const VERDE: &str = "\x1b[32m";
const AMARILLO: &str = "\x1b[33m";
const ROJO: &str = "\x1b[31m";

/// Reports that a command finished the work it was asked to do.
///
/// Says what it produced rather than the word success on its own: a person
/// wants to know there were four pages and not that there were pages, and a
/// script wants the four.
pub fn success(output: Option<&Path>, pages: usize, assets: usize) {
    if is_quiet() {
        return;
    }

    let Some((format, command, input, timestamp, duration)) = with_run(|run| {
        (
            run.format,
            run.command,
            run.input.clone(),
            run.timestamp.clone(),
            run.started.elapsed().as_millis() as u64,
        )
    }) else {
        return;
    };

    let destino = output
        .map(|path| path.display().to_string())
        .unwrap_or_default();

    match format {
        Format::Plain => {
            let counts = match (pages, assets) {
                (0, 0) => "nothing to write".to_string(),
                (pages, 0) => plural(pages, "page", "pages"),
                (0, assets) => plural(assets, "asset", "assets"),
                (pages, assets) => format!(
                    "{} and {}",
                    plural(pages, "page", "pages"),
                    plural(assets, "asset", "assets")
                ),
            };

            let donde = if destino.is_empty() {
                String::new()
            } else {
                format!(" in {destino}")
            };

            let _ = (command, input, timestamp);

            tracing::info!(
                "{}",
                paint(
                    use_color(),
                    VERDE,
                    &format!("success: {counts}{donde} ({duration}ms)")
                )
            );
        }

        Format::Json => tracing::info!(
            status = "success",
            command = command,
            input = %input,
            output = %destino,
            pages = pages,
            assets = assets,
            duration_ms = duration,
            timestamp = %timestamp,
        ),
    }
}

/// Reports that a scaffold was written, or that it was not.
///
/// Says where the site landed, in an absolute path, because a relative one
/// only means something to whoever typed it and a caller automating this has no
/// other way of learning it. The count of files is left out: it says nothing a
/// reader can act on, and so does a time that is always nearly nothing next to
/// writing them.
pub fn scaffolded(path: &Path, files: usize) {
    if is_quiet() {
        return;
    }

    let site = absolute(path).unwrap_or_else(|_| path.to_path_buf());

    if files == 0 {
        warning(&format!(
            "{} already exists, nothing written",
            site.display()
        ));
        return;
    }

    match format() {
        Format::Plain => {
            let text = format!("success: site created at {}", site.display());

            tracing::info!("{}", paint(use_color(), VERDE, &text));
        }

        Format::Json => {
            let Some((command, input, timestamp)) = identity() else {
                return;
            };

            tracing::info!(
                status = "success",
                command = command,
                input = %input,
                output = %site.display(),
                timestamp = %timestamp,
            );
        }
    }
}

/// The command, the input and the timestamp, all resolved once.
fn identity() -> Option<(&'static str, String, String)> {
    with_run(|run| (run.command, run.input.clone(), run.timestamp.clone()))
}

/// Reports something the reader should know that is not a failure.
///
/// Warnings do not stop the run and do not change the exit code. They are the
/// middle voice between the plain success and the error, and they are the only
/// thing here that `warn!` is used for.
pub fn warning(reason: &str) {
    if is_quiet() {
        return;
    }

    let Some((command, input, timestamp)) = identity() else {
        return;
    };

    match format() {
        Format::Plain => tracing::warn!(
            "{}",
            paint(use_color(), AMARILLO, &format!("warning: {reason}"))
        ),

        Format::Json => tracing::warn!(
            status = "warning",
            command = command,
            input = %input,
            reason = reason,
            timestamp = %timestamp,
        ),
    }
}

/// Reports that a command could not finish.
///
/// Quiet does not reach here. A silent failure is worse than any amount of
/// noise: the exit code says something went wrong, and saying nothing about it
/// leaves the reader with nothing to act on. `--quiet` silences the report of
/// work done, never the report of work that failed.
pub fn failure(reason: &str) {
    let Some((command, input, timestamp)) = identity() else {
        return;
    };

    match format() {
        Format::Plain => {
            tracing::error!("{}", paint(use_color(), ROJO, &format!("error: {reason}")))
        }

        Format::Json => tracing::error!(
            status = "error",
            command = command,
            input = %input,
            reason = reason,
            timestamp = %timestamp,
        ),
    }
}

/// One and many, without a grammar of its own.
fn plural(cuenta: usize, uno: &str, muchos: &str) -> String {
    if cuenta == 1 {
        format!("{cuenta} {uno}")
    } else {
        format!("{cuenta} {muchos}")
    }
}

/// Formats a moment as UTC, the way `sitemaps.org` and everyone else writes it.
///
/// The civil date comes from the days-since-the-epoch count, which is an exact
/// integer calculation. It needs no date library, and there is one place here
/// that could ever be wrong.
fn iso8601(momento: SystemTime) -> String {
    let segundos = momento
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let dias = segundos.div_euclid(86_400);
    let hora = segundos.rem_euclid(86_400);

    let (anyo, mes, dia) = civil_desde_dias(dias);

    format!(
        "{anyo:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}Z",
        hora / 3_600,
        (hora % 3_600) / 60,
        hora % 60
    )
}

/// Turns a count of days since 1970-01-01 into a year, a month and a day.
///
/// Shifts the epoch to 0000-03-01 so leap days land at the end of the year and
/// the months come out in order.
fn civil_desde_dias(dias: i64) -> (i64, u32, u32) {
    let deslocado = dias + 719_468;
    let era = deslocado.div_euclid(146_097);
    let dia_de_era = deslocado.rem_euclid(146_097);
    let anyo_de_era =
        (dia_de_era - dia_de_era / 1_460 + dia_de_era / 36_524 - dia_de_era / 146_096) / 365;
    let anyo = anyo_de_era + era * 400;
    let dia_del_anyo = dia_de_era - (365 * anyo_de_era + anyo_de_era / 4 - anyo_de_era / 100);
    let mes_desplazado = (5 * dia_del_anyo + 2) / 153;

    let dia = (dia_del_anyo - (153 * mes_desplazado + 2) / 5 + 1) as u32;
    let mes = if mes_desplazado < 10 {
        mes_desplazado + 3
    } else {
        mes_desplazado - 9
    } as u32;

    (if mes <= 2 { anyo + 1 } else { anyo }, mes, dia)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn momento(segundos: u64) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(segundos)
    }

    #[test]
    fn el_epoch_sale_como_se_espera() {
        assert_eq!(iso8601(momento(0)), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn el_comienzo_del_siglo_veinte() {
        assert_eq!(iso8601(momento(946_684_800)), "2000-01-01T00:00:00Z");
    }

    #[test]
    fn despues_de_un_bisiesto() {
        assert_eq!(iso8601(momento(978_220_800)), "2000-12-31T00:00:00Z");
        assert_eq!(iso8601(momento(1_709_164_800)), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn una_fecha_cualquiera() {
        assert_eq!(iso8601(momento(1_791_849_600)), "2026-10-13T00:00:00Z");
    }

    #[test]
    fn la_hora_se_descompone_bien() {
        assert_eq!(
            iso8601(momento(1_791_849_600 + 3_661)),
            "2026-10-13T01:01:01Z"
        );
    }

    #[test]
    fn el_plural_acepta_uno_y_varios() {
        assert_eq!(plural(1, "page", "pages"), "1 page");
        assert_eq!(plural(4, "page", "pages"), "4 pages");
        assert_eq!(plural(0, "asset", "assets"), "0 assets");
    }
}
