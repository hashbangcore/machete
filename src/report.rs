//! What a run says about itself.
//!
//! Everything that identifies the run is worked out once, when the program
//! starts, and kept in a single place. The timestamp is the clearest case: read
//! the clock on every event and two events in the same second could disagree
//! about when they happened, or worse, about the date if one crossed midnight.
//! Here it is resolved once and everybody reuses it.
//!
//! Adding a field to the report means adding it to [`Run`] and reading it in the
//! event that needs it. Nothing else has to change, and the value is still
//! resolved in one place.

use clap::ValueEnum;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// How a run writes down what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Words for a person: `success`, or `error: <motivo>`.
    Plain,
    /// A JSON object per record.
    Json,
    /// A JSON object per line, to append to a file of records.
    Jsonl,
}

/// The run's identity, resolved once at startup.
struct Run {
    format: Format,
    quiet: bool,
    /// Resolved here and nowhere else, so every event reads the same one.
    timestamp: String,
    command: &'static str,
    input: String,
}

/// Set once, when the program starts, and read by everything that reports.
static RUN: OnceLock<Mutex<Run>> = OnceLock::new();

/// Records how this run should report, and when it happened.
///
/// Called once, before anything else runs. The clock is read here rather than
/// at each event.
pub fn start(format: Format, quiet: bool) {
    let timestamp = iso8601(SystemTime::now());

    let _ = RUN.set(Mutex::new(Run {
        format,
        quiet,
        timestamp,
        command: "",
        input: String::new(),
    }));
}

/// Names the command and the site it runs on.
///
/// A second resolution, because the arguments are only known once they are
/// parsed. It happens once too, so the value does not change halfway through.
pub fn identify(command: &'static str, input: &std::path::Path) {
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
/// `--quiet` wins over `--format`. Asking for both means the quiet one rules,
/// so nothing is written whichever format was named.
pub fn is_quiet() -> bool {
    with_run(|run| run.quiet).unwrap_or(false)
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

/// Reports that a command finished the work it was asked to do.
pub fn success(output: Option<&std::path::Path>, pages: usize) {
    if is_quiet() {
        return;
    }

    let Some((format, command, input, timestamp)) = with_run(|run| {
        (
            run.format,
            run.command,
            run.input.clone(),
            run.timestamp.clone(),
        )
    }) else {
        return;
    };

    match format {
        Format::Plain => tracing::info!("success"),
        Format::Json | Format::Jsonl => tracing::info!(
            status = "success",
            command = command,
            input = %input,
            output = output.map(|path| path.display().to_string()).unwrap_or_default(),
            pages = pages,
            timestamp = %timestamp,
        ),
    }
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

    let Some((command, input, timestamp)) = with_run(|run| {
        (
            run.command.to_string(),
            run.input.clone(),
            run.timestamp.clone(),
        )
    }) else {
        return;
    };

    match format() {
        Format::Plain => tracing::warn!("warning: {reason}"),
        Format::Json | Format::Jsonl => tracing::warn!(
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
    let Some((command, input, timestamp)) = with_run(|run| {
        (
            run.command.to_string(),
            run.input.clone(),
            run.timestamp.clone(),
        )
    }) else {
        return;
    };

    match format() {
        Format::Plain => tracing::error!("error: {reason}"),
        Format::Json | Format::Jsonl => tracing::error!(
            status = "error",
            command = command,
            input = %input,
            reason = reason,
            timestamp = %timestamp,
        ),
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
        // The turn of the century: a leap day followed, so the count is the
        // interesting one.
        assert_eq!(iso8601(momento(946_684_800)), "2000-01-01T00:00:00Z");
    }

    #[test]
    fn despues_de_un_bisiesto() {
        // 2000 was a leap year, so the calendar has to know it when it counts
        // the last day of the year.
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
}
