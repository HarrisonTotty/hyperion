//! Golden files: exact expected output, checked in beside the tests.
//!
//! A golden file lives at `crates/<crate>/tests/golden/<name>.golden`. Its first line is
//! `# generator_version = <n>`, written by [`GoldenWriter::header`]. A test builds the text it
//! expects with a [`GoldenWriter`] and hands it to [`golden!`](crate::golden!), which compares it
//! with the file byte for byte.
//!
//! When generated output changes on purpose, `GENERATOR_VERSION` is bumped and `just bless`
//! rewrites the files in the same commit. `just bless` sets `HYPERION_BLESS=1`. CI sets `CI`, and
//! blessing under `CI` panics, so CI can never paper over a change.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Environment variable that, set to `1`, makes [`check`] write golden files.
pub const BLESS_VAR: &str = "HYPERION_BLESS";

/// Environment variable whose presence forbids blessing.
pub const CI_VAR: &str = "CI";

const HEADER_PREFIX: &str = "# generator_version = ";

const BLESS_HINT: &str = "if this change is intended, bump GENERATOR_VERSION and run `just bless`";

/// What [`check_in_mode`] does with the text it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Compare the text with the golden file and panic on any difference.
    Compare,
    /// Write the text to the golden file, creating directories as needed.
    Bless,
    /// Blessing was requested where it is forbidden (under CI): panic.
    BlessForbidden,
}

impl Mode {
    /// Reads the mode from the process environment.
    ///
    /// [`BLESS_VAR`] set to `1` asks for a bless. If [`CI_VAR`] is set as well, to anything, the
    /// result is [`Mode::BlessForbidden`].
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_vars(
            std::env::var_os(BLESS_VAR).as_deref(),
            std::env::var_os(CI_VAR).as_deref(),
        )
    }

    /// The mode for the given values of [`BLESS_VAR`] and [`CI_VAR`], `None` where unset.
    #[must_use]
    pub fn from_vars(bless: Option<&OsStr>, ci: Option<&OsStr>) -> Self {
        let bless = bless.is_some_and(|v| v == "1");
        match (bless, ci.is_some()) {
            (false, _) => Self::Compare,
            (true, false) => Self::Bless,
            (true, true) => Self::BlessForbidden,
        }
    }
}

/// Compares `actual` with the golden file `name` of the crate at `manifest_dir`.
///
/// Equivalent to [`check_in_mode`] with [`Mode::from_env`]. Tests normally call this through
/// [`golden!`](crate::golden!), which supplies the calling crate's manifest directory.
///
/// # Panics
///
/// As [`check_in_mode`].
pub fn check(manifest_dir: &str, name: &str, actual: &str) {
    check_in_mode(Mode::from_env(), Path::new(manifest_dir), name, actual);
}

/// Compares `actual` with `<manifest_dir>/tests/golden/<name>.golden`, or writes it there.
///
/// `actual` must begin with the header line that [`GoldenWriter::header`] writes. The header
/// carries the generator version the test was built against.
///
/// # Panics
///
/// - If `actual` does not begin with a header line.
/// - If `actual` has a line with trailing whitespace or does not end in exactly one line break.
///   The repository's pre-commit hooks strip both, so such a file would change on commit and then
///   fail to match.
/// - In [`Mode::Compare`]: if the file cannot be read; if the file's header names a different
///   generator version from `actual`'s, which is what forces goldens to be regenerated in the
///   commit that bumps the version; or if the two differ anywhere, naming the path, the first
///   differing line number and both lines.
/// - In [`Mode::Bless`]: if the file cannot be written.
/// - In [`Mode::BlessForbidden`]: always.
pub fn check_in_mode(mode: Mode, manifest_dir: &Path, name: &str, actual: &str) {
    let path = golden_path(manifest_dir, name);
    let shown = path.display();
    let actual_version = header_version(actual).unwrap_or_else(|| {
        panic!(
            "golden text for {shown} must begin with `{HEADER_PREFIX}<n>`: use GoldenWriter::header"
        )
    });

    if let Err(problem) = survives_the_commit_hooks(actual) {
        panic!("golden text for {shown} {problem}: the pre-commit hooks would rewrite the file");
    }

    match mode {
        Mode::Compare => {}
        Mode::Bless => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .unwrap_or_else(|e| panic!("cannot create {}: {e}", parent.display()));
            }
            std::fs::write(&path, actual).unwrap_or_else(|e| panic!("cannot write {shown}: {e}"));
            return;
        }
        Mode::BlessForbidden => panic!(
            "refusing to bless {shown}: {BLESS_VAR} is set under {CI_VAR}; \
             golden files are blessed on a developer machine and committed"
        ),
    }

    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read golden file {shown}: {e}; {BLESS_HINT}"));

    match header_version(&expected) {
        Some(v) if v == actual_version => {}
        Some(v) => panic!(
            "golden file {shown} has header generator_version = {v}, \
             but the test expects generator_version = {actual_version}; {BLESS_HINT}"
        ),
        None => panic!("golden file {shown} has no `{HEADER_PREFIX}<n>` header; {BLESS_HINT}"),
    }

    if expected == actual {
        return;
    }

    let mut expected_lines = expected.split('\n');
    let mut actual_lines = actual.split('\n');
    let mut number = 1_usize;
    loop {
        match (expected_lines.next(), actual_lines.next()) {
            (Some(e), Some(a)) if e == a => number += 1,
            (e, a) => panic!(
                "golden file {shown} differs at line {number}\n  \
                 golden: {}\n  actual: {}\n{BLESS_HINT}",
                e.unwrap_or("<end of file>"),
                a.unwrap_or("<end of text>"),
            ),
        }
    }
}

/// Whether `text` is left alone by the `trailing-whitespace` and `end-of-file-fixer` hooks.
fn survives_the_commit_hooks(text: &str) -> Result<(), String> {
    if !text.ends_with('\n') || text.ends_with("\n\n") {
        return Err("does not end in exactly one line break".to_owned());
    }
    match text
        .split('\n')
        .position(|line| line.ends_with([' ', '\t']))
    {
        Some(i) => Err(format!("has trailing whitespace on line {}", i + 1)),
        None => Ok(()),
    }
}

fn golden_path(manifest_dir: &Path, name: &str) -> PathBuf {
    let mut path = manifest_dir.join("tests").join("golden");
    path.extend(name.split('/'));
    path.set_extension("golden");
    path
}

fn header_version(text: &str) -> Option<u32> {
    let first = text.split('\n').next()?;
    first.strip_prefix(HEADER_PREFIX)?.parse().ok()
}

/// Checks `actual` against the calling crate's golden file `name`.
///
/// `golden!("rng/streams", &text)` reads `tests/golden/rng/streams.golden` under the calling
/// crate's manifest directory. See [`golden::check`](crate::golden::check).
#[macro_export]
macro_rules! golden {
    ($name:expr, $actual:expr $(,)?) => {
        $crate::golden::check(env!("CARGO_MANIFEST_DIR"), $name, $actual)
    };
}

/// Builds the text of a golden file, one line at a time.
///
/// # Examples
///
/// ```
/// use hyperion_testkit::golden::GoldenWriter;
///
/// let mut w = GoldenWriter::new();
/// w.header(1);
/// w.u64_hex("word[0]", 0xdead_beef);
/// w.f64("half", 0.5);
/// assert_eq!(
///     w.finish(),
///     "# generator_version = 1\nword[0] = 0x00000000deadbeef\nhalf = 0x3fe0000000000000  # 0.5\n",
/// );
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoldenWriter {
    text: String,
}

impl GoldenWriter {
    /// Creates an empty writer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes the header line, `# generator_version = <version>`. It must be the first line.
    ///
    /// # Panics
    ///
    /// If anything has been written already.
    pub fn header(&mut self, version: u32) {
        assert!(
            self.text.is_empty(),
            "the header must be the first line of a golden file"
        );
        self.push(format_args!("{HEADER_PREFIX}{version}"));
    }

    /// Writes `line` verbatim.
    ///
    /// # Panics
    ///
    /// If `line` contains a line break.
    pub fn line(&mut self, line: &str) {
        assert!(
            !line.contains(['\n', '\r']),
            "a golden line may not contain a line break"
        );
        self.push(format_args!("{line}"));
    }

    /// Writes `label = 0x<16 hexadecimal digits>`.
    pub fn u64_hex(&mut self, label: &str, value: u64) {
        self.push(format_args!("{label} = 0x{value:016x}"));
    }

    /// Writes `label = 0x<bits as 16 hexadecimal digits>  # <shortest round-trip decimal>`.
    ///
    /// The bits are what is pinned. The decimal is for the reader.
    ///
    /// # Panics
    ///
    /// If `value` is a NaN, whose bits are unspecified and must never be pinned.
    pub fn f64(&mut self, label: &str, value: f64) {
        assert!(
            !value.is_nan(),
            "{label} is NaN: the bits of a NaN are unspecified"
        );
        self.push(format_args!(
            "{label} = 0x{:016x}  # {value:?}",
            value.to_bits()
        ));
    }

    /// The text written so far.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Consumes the writer and returns the text.
    #[must_use]
    pub fn finish(self) -> String {
        self.text
    }

    fn push(&mut self, line: std::fmt::Arguments<'_>) {
        self.text
            .write_fmt(line)
            .expect("writing to a String cannot fail");
        self.text.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_formats_each_kind_of_line() {
        let mut w = GoldenWriter::new();
        w.header(7);
        w.line("free text");
        w.u64_hex("word", u64::MAX);
        w.f64("tenth", 0.1);
        w.f64("tiny", 1e-10);
        assert_eq!(
            w.as_str(),
            "# generator_version = 7\nfree text\nword = 0xffffffffffffffff\n\
             tenth = 0x3fb999999999999a  # 0.1\ntiny = 0x3ddb7cdfd9d7bdbb  # 1e-10\n",
        );
    }

    #[test]
    #[should_panic(expected = "bits of a NaN are unspecified")]
    fn writer_refuses_a_nan() {
        GoldenWriter::new().f64("bad", f64::NAN);
    }

    #[test]
    #[should_panic(expected = "header must be the first line")]
    fn writer_refuses_a_late_header() {
        let mut w = GoldenWriter::new();
        w.line("x");
        w.header(1);
    }

    #[test]
    fn bless_is_asked_for_by_one_and_forbidden_under_ci() {
        let one = Some(OsStr::new("1"));
        let ci = Some(OsStr::new("true"));
        assert_eq!(Mode::from_vars(None, None), Mode::Compare);
        assert_eq!(Mode::from_vars(None, ci), Mode::Compare);
        assert_eq!(Mode::from_vars(Some(OsStr::new("0")), None), Mode::Compare);
        assert_eq!(Mode::from_vars(one, None), Mode::Bless);
        assert_eq!(Mode::from_vars(one, ci), Mode::BlessForbidden);
        assert_eq!(
            Mode::from_vars(one, Some(OsStr::new(""))),
            Mode::BlessForbidden
        );
    }

    #[test]
    fn header_version_reads_only_a_well_formed_first_line() {
        assert_eq!(header_version("# generator_version = 12\nrest\n"), Some(12));
        assert_eq!(header_version("rest\n# generator_version = 12\n"), None);
        assert_eq!(header_version("# generator_version = x\n"), None);
    }

    #[test]
    fn text_the_commit_hooks_would_rewrite_is_refused() {
        assert_eq!(survives_the_commit_hooks("# h\na = 1\n\nb = 2\n"), Ok(()));
        let ending = Err("does not end in exactly one line break".to_owned());
        assert_eq!(survives_the_commit_hooks("# h\na = 1"), ending);
        assert_eq!(survives_the_commit_hooks("# h\na = 1\n\n"), ending);
        assert_eq!(
            survives_the_commit_hooks("# h\na = 1 \n"),
            Err("has trailing whitespace on line 2".to_owned())
        );
    }

    #[test]
    #[should_panic(expected = "has trailing whitespace on line 2")]
    fn check_refuses_a_trailing_space() {
        check_in_mode(
            Mode::Compare,
            Path::new("/nowhere"),
            "x",
            "# generator_version = 1\nx \n",
        );
    }

    #[test]
    fn golden_path_nests_under_tests_golden() {
        assert_eq!(
            golden_path(Path::new("/crate"), "rng/streams"),
            Path::new("/crate/tests/golden/rng/streams.golden"),
        );
    }
}
