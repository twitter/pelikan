use crate::Record;
use rustcommon_time::{DateTime, Local, SecondsFormat};

pub type FormatFunction = fn(
    write: &mut dyn std::io::Write,
    now: DateTime<Local>,
    record: &Record,
) -> Result<(), std::io::Error>;

pub fn default_format(
    w: &mut dyn std::io::Write,
    now: DateTime<Local>,
    record: &Record,
) -> Result<(), std::io::Error> {
    writeln!(
        w,
        "{} {} [{}] {}",
        now.to_rfc3339_opts(SecondsFormat::Secs, true),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.args()
    )
}
