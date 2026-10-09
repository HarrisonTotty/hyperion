//! Knowledge logs: versioned JSON lines in the universe's reserved `knowledge/` directory, shared
//! by the contacts (plan 12, P12.T7.b) and the survey passes (plan R09, R09.T18).
//!
//! A log is `<data_dir>/universes/<id>/knowledge/<stem>.v<format>.jsonl`. Its first line is the
//! header, `{"format":<format>}`; every other line is one change, in the order it was made, and
//! what the log holds is rebuilt by applying them in order. A change is appended, synced, and only
//! then applied in memory, so memory never runs ahead of the disk.
//!
//! On load, a header of another format, or a log of a later format beside this one
//! (`<stem>.v2.jsonl` and on, for format 1), is [`LoadKnowledgeError::UnsupportedFormat`]. A last
//! line with no newline is what a crash in the middle of an append leaves: it is dropped with a
//! warning, and the next append writes over it. Any other line that does not parse, or that does
//! not fit the lines before it, is [`LoadKnowledgeError::MalformedLine`], and nothing is loaded.
//!
//! Every function here is blocking: the callers run them on [`tokio::task::spawn_blocking`], never
//! on the runtime.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use hyperion_sim::time::UniverseTime;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::persist::{LoadKnowledgeError, SaveKnowledgeError};

/// A log's name, `<stem>.v<format>.jsonl`, and the one format this build writes and reads in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LogName {
    stem: &'static str,
    format: u32,
}

impl LogName {
    /// The log `<stem>.v<format>.jsonl`.
    ///
    /// # Panics
    ///
    /// If `format` is 0, which no log writes.
    #[must_use]
    pub(super) const fn new(stem: &'static str, format: u32) -> Self {
        assert!(format > 0, "format 0 is never written");
        Self { stem, format }
    }

    /// The file's name.
    #[must_use]
    pub(super) fn file_name(self) -> String {
        format!("{}.v{}.jsonl", self.stem, self.format)
    }

    /// The format of a file of this log named `name`, in any format, or `None` if `name` is not
    /// one of this log's.
    #[must_use]
    fn format_of(self, name: &str) -> Option<u32> {
        name.strip_prefix(self.stem)
            .and_then(|name| name.strip_prefix(".v"))
            .and_then(|name| name.strip_suffix(".jsonl"))
            .and_then(|number| number.parse::<u32>().ok())
    }
}

/// A log file, as far as it holds good lines.
#[derive(Debug)]
pub(super) struct KnowledgeLog {
    dir: PathBuf,
    path: PathBuf,
    /// The format its header states.
    format: u32,
    /// Open once the first append needs it.
    file: Option<File>,
    /// The length of the good lines: an append starts here, cutting off whatever a failed append
    /// or a crash left beyond it.
    len: u64,
    /// The directories synced, in order, for the tests.
    #[cfg(test)]
    synced: Vec<PathBuf>,
}

impl KnowledgeLog {
    /// Appends `line`, after the header if the file holds nothing yet, and syncs it.
    pub(super) fn append(&mut self, line: &impl Serialize) -> Result<(), SaveKnowledgeError> {
        let mut text = String::new();
        if self.len == 0 {
            push_line(
                &mut text,
                &HeaderLine {
                    format: self.format,
                },
            );
        }
        push_line(&mut text, line);
        let len = self.len;
        let path = self.path.clone();
        let file = self.file()?;
        file.set_len(len).map_err(save_io("truncate", &path))?;
        file.seek(SeekFrom::Start(len))
            .map_err(save_io("seek", &path))?;
        file.write_all(text.as_bytes())
            .map_err(save_io("write", &path))?;
        file.sync_data().map_err(save_io("sync", &path))?;
        self.len = len + u64::try_from(text.len()).expect("a line's length fits in 64 bits");
        Ok(())
    }

    /// The open file, created with its directory if need be. The directory is created inside the
    /// universe's, never the universe's itself, so that no directory appears for a universe with
    /// no save.
    ///
    /// The first open in a process syncs the `knowledge/` directory and then the universe's,
    /// whether or not this log made either: the logs share `knowledge/`, so one may find it made by
    /// another whose sync has not finished, or left unsynced by a sync that failed, and its own
    /// lines must not be reported saved before the names that lead to them are durable (R09.T18).
    /// That is at most two directory syncs a log a process.
    fn file(&mut self) -> Result<&mut File, SaveKnowledgeError> {
        if self.file.is_none() {
            match fs::create_dir(&self.dir) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(save_io("create directory", &self.dir)(error)),
            }
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&self.path)
                .map_err(save_io("open", &self.path))?;
            let universe = self.dir.parent().map(Path::to_path_buf);
            for dir in std::iter::once(self.dir.clone()).chain(universe) {
                sync_directory(&dir).map_err(save_io("sync directory", &dir))?;
                #[cfg(test)]
                self.synced.push(dir);
            }
            self.file = Some(file);
        }
        Ok(self.file.as_mut().expect("the file was opened above"))
    }
}

/// The error of a failed step of an append.
fn save_io(operation: &'static str, path: &Path) -> impl FnOnce(io::Error) -> SaveKnowledgeError {
    let path = path.to_path_buf();
    move |source| SaveKnowledgeError::Io {
        operation,
        path,
        source,
    }
}

/// Appends `line` as JSON and a newline.
fn push_line(text: &mut String, line: &impl Serialize) {
    text.push_str(&serde_json::to_string(line).expect("a line of strings and numbers serialises"));
    text.push('\n');
}

/// Makes the entries of `dir` durable on Unix, and does nothing elsewhere.
///
/// On Windows std's `File::open` cannot open a directory, and flushing a directory handle is
/// undocumented, as [`UniverseStore::write`](crate::universe::UniverseStore::write) explains (plan
/// 04, P04.T17.b). There the new `knowledge/` directory and a log's name are left to NTFS, which
/// logs metadata changes in one sequential log. Each append's own `sync_data`, a
/// `FlushFileBuffers` of the file, is expected to carry them with it, but Microsoft documents no
/// such guarantee. On FAT, exFAT and network shares nothing is promised beyond the file's own data.
///
/// On macOS std's `sync_all` and `sync_data` are `fcntl(F_FULLFSYNC)`. This sync and every
/// append's therefore flush the drive's cache.
fn sync_directory(dir: &Path) -> io::Result<()> {
    if cfg!(unix) {
        File::open(dir).and_then(|handle| handle.sync_all())
    } else {
        Ok(())
    }
}

/// Reads the log `name` in `dir` (module documentation), handing each change's line to `apply` in
/// order, and returns the log, ready to append after its good lines.
///
/// `apply` refuses a line that does not fit those before it with the reason the error names: the
/// field that does not hold a valid value, or what is wrong with the line as a whole.
pub(super) fn load<T: DeserializeOwned>(
    dir: PathBuf,
    name: LogName,
    mut apply: impl FnMut(T) -> Result<(), &'static str>,
) -> Result<KnowledgeLog, LoadKnowledgeError> {
    refuse_later_formats(&dir, name)?;
    let path = dir.join(name.file_name());
    let mut log = KnowledgeLog {
        dir,
        path,
        format: name.format,
        file: None,
        len: 0,
        #[cfg(test)]
        synced: Vec::new(),
    };
    let bytes = match fs::read(&log.path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(log),
        Err(source) => {
            return Err(LoadKnowledgeError::Io {
                operation: "read",
                path: log.path,
                source,
            });
        }
    };
    let mut good = 0;
    let mut rest = bytes.as_slice();
    let mut number = 0;
    while !rest.is_empty() {
        number += 1;
        let Some(end) = rest.iter().position(|&byte| byte == b'\n') else {
            tracing::warn!(
                path = %log.path.display(),
                line = number,
                bytes = rest.len(),
                "dropping a torn last line of a knowledge file"
            );
            break;
        };
        let line = &rest[..end];
        let malformed = |reason, source| LoadKnowledgeError::MalformedLine {
            path: log.path.clone(),
            line: number,
            reason,
            source,
        };
        if number == 1 {
            let header: HeaderLine = serde_json::from_slice(line)
                .map_err(|error| malformed("not a header", Some(error)))?;
            match header.format {
                0 => return Err(malformed("format 0 is never written", None)),
                format if format == name.format => {}
                format => {
                    return Err(LoadKnowledgeError::UnsupportedFormat {
                        path: log.path,
                        format,
                    });
                }
            }
        } else {
            let parsed: T = serde_json::from_slice(line)
                .map_err(|error| malformed("not an entry", Some(error)))?;
            apply(parsed).map_err(|reason| malformed(reason, None))?;
        }
        good += end + 1;
        rest = &rest[end + 1..];
    }
    log.len = u64::try_from(good).expect("a file's length fits in 64 bits");
    Ok(log)
}

/// Refuses a directory that holds a file of log `name` in a later format than this build's.
fn refuse_later_formats(dir: &Path, name: LogName) -> Result<(), LoadKnowledgeError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(LoadKnowledgeError::Io {
                operation: "list",
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    let mut latest = None;
    for entry in entries {
        let entry = entry.map_err(|source| LoadKnowledgeError::Io {
            operation: "list",
            path: dir.to_path_buf(),
            source,
        })?;
        let format = entry
            .file_name()
            .to_str()
            .and_then(|file| name.format_of(file));
        if let Some(format) = format
            && format > name.format
        {
            latest = latest.max(Some((format, entry.path())));
        }
    }
    match latest {
        Some((format, path)) => Err(LoadKnowledgeError::UnsupportedFormat { path, format }),
        None => Ok(()),
    }
}

/// The header line.
#[derive(Debug, Serialize, Deserialize)]
struct HeaderLine {
    format: u32,
}

/// An instant of the universe clock, exactly.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct TimeLine {
    seconds: i64,
    nanos: u32,
}

impl From<UniverseTime> for TimeLine {
    fn from(time: UniverseTime) -> Self {
        Self {
            seconds: time.seconds(),
            nanos: time.subsec_nanos(),
        }
    }
}

impl TimeLine {
    /// The time, or `None` for nanoseconds past a second, which only damage makes; the caller
    /// names the field.
    #[must_use]
    pub(super) fn to_time(&self) -> Option<UniverseTime> {
        UniverseTime::new(self.seconds, self.nanos).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log's name is its stem, its format and `.jsonl`, and only names of that form are its.
    #[test]
    fn knowledge_log_names_carry_their_stem_and_format() {
        let contacts = LogName::new("contacts", 1);
        assert_eq!(contacts.file_name(), "contacts.v1.jsonl");
        assert_eq!(contacts.format_of("contacts.v12.jsonl"), Some(12));
        for other in [
            "surveys.v2.jsonl",
            "contacts.v2.json",
            "contacts.2.jsonl",
            "contacts.vx.jsonl",
            "xcontacts.v2.jsonl",
        ] {
            assert_eq!(contacts.format_of(other), None, "{other}");
        }
    }

    /// The first append of a process syncs `knowledge/` and the universe's directory even when
    /// another log made them and the file exists, and later appends sync neither again.
    #[test]
    fn knowledge_log_first_append_syncs_both_directories_it_did_not_make() {
        let root = tempfile::tempdir().unwrap();
        let universe = root.path().join("universe");
        let dir = universe.join("knowledge");
        fs::create_dir_all(&dir).unwrap();
        let name = LogName::new("surveys", 1);
        let path = dir.join(name.file_name());
        fs::write(&path, "{\"format\":1}\n{\"n\":0}\n").unwrap();
        let mut lines = 0;
        let mut log = load(dir.clone(), name, |_: serde_json::Value| {
            lines += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(lines, 1);
        assert!(log.synced.is_empty(), "loading syncs nothing");
        log.append(&serde_json::json!({ "n": 1 })).unwrap();
        log.append(&serde_json::json!({ "n": 2 })).unwrap();
        assert_eq!(log.synced, [dir, universe]);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{\"format\":1}\n{\"n\":0}\n{\"n\":1}\n{\"n\":2}\n"
        );
    }
}
