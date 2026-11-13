//! Pinned figures: the values a test measures, kept in a data file and blessed by test name.
//!
//! A test that pins what it measures (a census's counts, a χ², a share) opens a [`Pins`] handle by
//! its own name with [`pins!`](crate::pins!) and pins each figure under a key, exactly or within a
//! tolerance that the code states:
//!
//! ```no_run
//! use hyperion_testkit::pins::Tolerance;
//!
//! let mut pins = hyperion_testkit::pins!("census/compact", "the_compact_census_is_pinned");
//! pins.exact("members", 471_u32);
//! pins.exact("bands", (125_u32, 92_u32, 12_u32));
//! pins.within("median", 2.558_032_271_359_235, Tolerance::absolute(1e-12));
//! ```
//!
//! The figures live in `crates/<crate>/tests/pins/<file>.pins`, beside the goldens, a file named
//! after the module whose tests pin into it. Its first line is [`HEADER`]; then each pin is one line,
//! `<test>.<key> = <value>`, sorted by `<test>.<key>` byte by byte. A value is an integer in
//! decimal (`-3`, `471`), a float as the shortest decimal that reads back to its bits (written
//! plainly from 10⁻⁴ up to 10¹⁶, `0.25`, `471.0`, `-0.0`, and in exponent form outside it,
//! `1e16`, `2.5e-7`; `inf`, `-inf`), or a list of values, `[1, 2.5, [3, 4]]`, for a tuple or a
//! small table. The file holds exact values and is LF text, the same bytes on every platform; a
//! file in any other form fails to read. A pin's tolerance is part of the test's code, never of
//! the file.
//!
//! When a figure changes on purpose, `just repin <test name>…` runs the named tests with
//! `HYPERION_BLESS=1` set, as `golden::Mode` reads it, and each pin then writes the value it
//! received where comparing would fail. Blessing writes only the keys the test reads, leaves every
//! other line as it was, and writes nothing where the file's value still matches, so a second run
//! is a no-op. Blessing under `CI` panics, and under [`KEEP_VAR`], which `just bless` sets, pins
//! compare. A key the file lacks fails unless blessing.
//!
//! Where [`LOG_VAR`] names a file, every pin appends its file's path and its key there, which `just
//! repin` reads to drop the keys a test no longer pins.
//!
//! On `wasm32-unknown-unknown`, where a test cannot read files, [`pins!`](crate::pins!) embeds the
//! file in the test binary and reads it with [`Pins::embedded`], which never blesses: figures are
//! pinned natively, then compared on every target, as goldens are.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt::{self, Write as _};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read as _, Seek as _, Write as _};
use std::num::FpCategory;
use std::path::{Path, PathBuf};

use crate::golden::{BLESS_VAR, CI_VAR, Mode};

/// The first line of every pins file.
pub const HEADER: &str =
    "# Pinned figures (hyperion_testkit::pins): re-pin them by test name with `just repin <test>`.";

/// Environment variable that, naming a file, makes every pin append `<pins file>\t<test>.<key>` to
/// it, one line each.
pub const LOG_VAR: &str = "HYPERION_PINS_LOG";

/// Environment variable whose presence keeps every pin comparing, even under [`BLESS_VAR`]. `just
/// bless` sets it, so that re-blessing every golden never re-pins a figure: figures are re-pinned
/// by test name alone.
pub const KEEP_VAR: &str = "HYPERION_PINS_KEEP";

/// A pinned value: an integer, a float, or a list of values (a tuple, or a small table's rows).
///
/// Floats compare by their bits, so `-0.0` and `0.0` differ. A float pinned must not be a NaN,
/// whose bits are unspecified.
#[derive(Debug, Clone)]
pub enum Value {
    /// An integer of any primitive type.
    Int(i128),
    /// A float, `f32` values widened exactly.
    Float(f64),
    /// A list of values, in order.
    List(Vec<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Int(_) | Self::Float(_) | Self::List(_), _) => false,
        }
    }
}

impl Eq for Value {}

impl Value {
    /// Whether the value holds a NaN anywhere.
    #[must_use]
    fn has_nan(&self) -> bool {
        match self {
            Self::Int(_) => false,
            Self::Float(x) => x.is_nan(),
            Self::List(values) => values.iter().any(Self::has_nan),
        }
    }

    /// Whether `received` matches this pinned value: the same shape, integers equal and floats
    /// within `tolerance`.
    #[must_use]
    fn admits(&self, received: &Self, tolerance: Tolerance) -> bool {
        match (self, received) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => tolerance.admits(*a, *b),
            (Self::List(a), Self::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.admits(b, tolerance))
            }
            (Self::Int(_) | Self::Float(_) | Self::List(_), _) => false,
        }
    }
}

/// The canonical text of the value, as the file holds it.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(x) => write_float(f, *x),
            Self::List(values) => {
                f.write_char('[')?;
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{value}")?;
                }
                f.write_char(']')
            }
        }
    }
}

/// Writes `x` as the shortest decimal that reads back to its bits: plainly, with at least one
/// fractional digit, where 10⁻⁴ ≤ |x| < 10¹⁶ or x is zero, and in exponent form elsewhere.
fn write_float(f: &mut fmt::Formatter<'_>, x: f64) -> fmt::Result {
    let plain = matches!(x.classify(), FpCategory::Zero) || (1e-4..1e16).contains(&x.abs());
    if x.is_infinite() {
        f.write_str(if x > 0.0 { "inf" } else { "-inf" })
    } else if plain {
        let text = format!("{x}");
        f.write_str(&text)?;
        if text.contains('.') {
            Ok(())
        } else {
            f.write_str(".0")
        }
    } else {
        write!(f, "{x:e}")
    }
}

macro_rules! value_from_integer {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(value: $t) -> Self {
                Self::Int(i128::from(value))
            }
        }
    )*};
}

value_from_integer!(i8, i16, i32, i64, u8, u16, u32, u64);

impl From<i128> for Value {
    fn from(value: i128) -> Self {
        Self::Int(value)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Self::Int(i128::try_from(value).expect("a usize has at most 64 bits"))
    }
}

impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Self::Int(i128::try_from(value).expect("an isize has at most 64 bits"))
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Float(f64::from(value))
    }
}

impl<T: Into<Value>, const N: usize> From<[T; N]> for Value {
    fn from(values: [T; N]) -> Self {
        Self::List(values.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(values: Vec<T>) -> Self {
        Self::List(values.into_iter().map(Into::into).collect())
    }
}

impl<T: Copy + Into<Value>> From<&[T]> for Value {
    fn from(values: &[T]) -> Self {
        Self::List(values.iter().copied().map(Into::into).collect())
    }
}

macro_rules! value_from_tuple {
    ($($t:ident $i:tt),+) => {
        impl<$($t: Into<Value>),+> From<($($t,)+)> for Value {
            fn from(values: ($($t,)+)) -> Self {
                Self::List(vec![$(values.$i.into()),+])
            }
        }
    };
}

value_from_tuple!(A 0);
value_from_tuple!(A 0, B 1);
value_from_tuple!(A 0, B 1, C 2);
value_from_tuple!(A 0, B 1, C 2, D 3);
value_from_tuple!(A 0, B 1, C 2, D 3, E 4);
value_from_tuple!(A 0, B 1, C 2, D 3, E 4, F 5);
value_from_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
value_from_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);

/// How far a received float may lie from its pinned value and still match it.
///
/// Integers always match exactly, and an infinite pinned value matches itself alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance(Bound);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Bound {
    Exact,
    Absolute(f64),
    Relative(f64),
}

impl Tolerance {
    /// The same bits.
    pub const EXACT: Self = Self(Bound::Exact);

    /// At most `bound` from the pinned value, in the figure's own unit.
    ///
    /// # Panics
    ///
    /// Unless `bound` is finite and not negative.
    #[must_use]
    pub fn absolute(bound: f64) -> Self {
        assert!(
            bound.is_finite() && bound >= 0.0,
            "a tolerance is finite and not negative, got {bound:?}"
        );
        Self(Bound::Absolute(bound))
    }

    /// At most `bound` times the pinned value's magnitude from it.
    ///
    /// # Panics
    ///
    /// Unless `bound` is finite and not negative.
    #[must_use]
    pub fn relative(bound: f64) -> Self {
        assert!(
            bound.is_finite() && bound >= 0.0,
            "a tolerance is finite and not negative, got {bound:?}"
        );
        Self(Bound::Relative(bound))
    }

    /// Whether `received` matches `pinned`: the same bits always do, and nothing else matches an
    /// infinity, which any bound would otherwise admit everything beside.
    #[must_use]
    fn admits(self, pinned: f64, received: f64) -> bool {
        let gap = (received - pinned).abs();
        pinned.to_bits() == received.to_bits()
            || pinned.is_finite()
                && match self.0 {
                    Bound::Exact => false,
                    Bound::Absolute(bound) => gap <= bound,
                    Bound::Relative(bound) => gap <= bound * pinned.abs(),
                }
    }
}

impl fmt::Display for Tolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Bound::Exact => f.write_str("exactly"),
            Bound::Absolute(bound) => write!(f, "within {bound:e}"),
            Bound::Relative(bound) => write!(f, "within {bound:e} of it relatively"),
        }
    }
}

/// The pins of one file, by `<test>.<key>`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinFile {
    pins: BTreeMap<String, Value>,
}

impl PinFile {
    /// An empty file.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a file's text, which must be in the canonical form [`PinFile::render`] writes.
    ///
    /// # Errors
    ///
    /// [`ParsePinsError`] naming the first line that is not in that form: the header missing, a
    /// line break other than one LF ending each line, a line not of the form `<test>.<key> =
    /// <value>`, a test name or key of other characters than [`Pins`] takes, a value not written
    /// canonically or holding a NaN, or a line out of order or repeated.
    pub fn parse(text: &str) -> Result<Self, ParsePinsError> {
        let error = |line: usize, kind: ParsePinsErrorKind| ParsePinsError { line, kind };
        let Some(body) = text.strip_suffix('\n') else {
            return Err(error(
                text.split('\n').count(),
                ParsePinsErrorKind::LineBreaks,
            ));
        };
        let mut lines = body.split('\n').enumerate().map(|(i, line)| (i + 1, line));
        match lines.next() {
            Some((_, HEADER)) => {}
            _ => return Err(error(1, ParsePinsErrorKind::Header)),
        }
        let mut pins = BTreeMap::new();
        let mut last: Option<&str> = None;
        for (number, line) in lines {
            if line.contains('\r') || line.is_empty() {
                return Err(error(number, ParsePinsErrorKind::LineBreaks));
            }
            let Some((key, value)) = line.split_once(" = ") else {
                return Err(error(number, ParsePinsErrorKind::Line));
            };
            let Some((name, rest)) = key.split_once('.') else {
                return Err(error(number, ParsePinsErrorKind::Key));
            };
            if !is_test_name(name) || !is_key(rest) {
                return Err(error(number, ParsePinsErrorKind::Key));
            }
            if last.is_some_and(|last| last >= key) {
                return Err(error(number, ParsePinsErrorKind::Order));
            }
            last = Some(key);
            let value = parse_value(value)
                .filter(|v| !v.has_nan() && v.to_string() == value)
                .ok_or_else(|| error(number, ParsePinsErrorKind::Value))?;
            pins.insert(key.to_owned(), value);
        }
        Ok(Self { pins })
    }

    /// The canonical text of the file: [`HEADER`], then each pin's line in order.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = String::with_capacity(HEADER.len() + 1 + 48 * self.pins.len());
        text.push_str(HEADER);
        text.push('\n');
        for (key, value) in &self.pins {
            writeln!(text, "{key} = {value}").expect("writing to a String cannot fail");
        }
        text
    }

    /// The value pinned under `<test>.<key>`, if any.
    #[must_use]
    pub fn get(&self, full_key: &str) -> Option<&Value> {
        self.pins.get(full_key)
    }

    /// Pins `value` under `<test>.<key>`, and says whether that changed the file.
    pub fn set(&mut self, full_key: &str, value: Value) -> bool {
        if self.pins.get(full_key) == Some(&value) {
            return false;
        }
        self.pins.insert(full_key.to_owned(), value);
        true
    }

    /// Every `<test>.<key>` the file pins, in order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.pins.keys().map(String::as_str)
    }
}

/// Why a pins file could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParsePinsError {
    line: usize,
    kind: ParsePinsErrorKind,
}

impl ParsePinsError {
    /// The line, from 1, at which the file leaves its canonical form.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// What is wrong there.
    #[must_use]
    pub fn kind(&self) -> ParsePinsErrorKind {
        self.kind
    }
}

/// What is wrong with a pins file's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParsePinsErrorKind {
    /// The first line is not [`HEADER`].
    Header,
    /// A line is empty or holds a CR, or the text does not end in one LF.
    LineBreaks,
    /// A line is not `<test>.<key> = <value>`.
    Line,
    /// A test name or key of other characters than [`Pins`] takes.
    Key,
    /// A value that does not read, is not written canonically, or holds a NaN.
    Value,
    /// A key not after the line before's.
    Order,
}

impl fmt::Display for ParsePinsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.kind {
            ParsePinsErrorKind::Header => "the first line is not the pins header",
            ParsePinsErrorKind::LineBreaks => "lines must each end in one LF, with none empty",
            ParsePinsErrorKind::Line => "a pin's line is `<test>.<key> = <value>`",
            ParsePinsErrorKind::Key => "a test name or key of characters a pin's key may not hold",
            ParsePinsErrorKind::Value => "the value is not written canonically",
            ParsePinsErrorKind::Order => "the key is not after the line before's",
        };
        write!(f, "line {}: {what}", self.line)
    }
}

impl std::error::Error for ParsePinsError {}

/// Reads a whole value's text, leniently: [`PinFile::parse`] then requires it to be canonical.
#[must_use]
fn parse_value(text: &str) -> Option<Value> {
    let (value, rest) = parse_prefix(text)?;
    rest.is_empty().then_some(value)
}

/// Reads one value from the start of `text`, returning it and what follows.
#[must_use]
fn parse_prefix(text: &str) -> Option<(Value, &str)> {
    if let Some(mut rest) = text.strip_prefix('[') {
        let mut values = Vec::new();
        if let Some(after) = rest.strip_prefix(']') {
            return Some((Value::List(values), after));
        }
        loop {
            let (value, after) = parse_prefix(rest)?;
            values.push(value);
            if let Some(after) = after.strip_prefix(", ") {
                rest = after;
            } else {
                return Some((Value::List(values), after.strip_prefix(']')?));
            }
        }
    }
    let end = text.find([',', ']']).unwrap_or(text.len());
    let (scalar, rest) = text.split_at(end);
    let digits = scalar.strip_prefix('-').unwrap_or(scalar);
    let value = if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        Value::Int(scalar.parse().ok()?)
    } else {
        Value::Float(scalar.parse().ok()?)
    };
    Some((value, rest))
}

/// Whether `name` may name a test in a key: ASCII letters, digits and underscores.
#[must_use]
fn is_test_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Whether `key` may follow a test's name: segments of ASCII letters, digits, `_` and `-`,
/// joined by dots.
#[must_use]
fn is_key(key: &str) -> bool {
    key.split('.').all(|segment| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    })
}

/// Whether `name` may name a pins file: segments of ASCII letters, digits, `_` and `-`, joined by
/// slashes.
#[must_use]
fn is_file_name(name: &str) -> bool {
    name.split('/').all(|segment| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    })
}

/// A test's handle on one pins file, through which it pins its figures under its own name.
///
/// Open it with [`pins!`](crate::pins!). Each key may be pinned once per handle.
#[derive(Debug)]
pub struct Pins {
    test: String,
    source: Source,
    pinned: BTreeSet<String>,
    log: Option<PathBuf>,
}

#[derive(Debug)]
enum Source {
    /// A file on disk, compared with its pins as read when the handle opened, or with none if it
    /// did not exist then.
    Compared {
        path: PathBuf,
        pins: Option<PinFile>,
    },
    /// A file on disk, read again and written at each pin.
    Blessed { path: PathBuf },
    /// The file's pins, from its text embedded in the test binary, compared.
    Embedded { shown: String, pins: PinFile },
}

impl Pins {
    /// Opens the pins file `file` of the crate at `manifest_dir` for the test `test`, which must be
    /// the running test's own name, comparing or blessing as [`Mode::from_env`] says, but
    /// comparing wherever [`KEEP_VAR`] is set.
    ///
    /// Tests normally call this through [`pins!`](crate::pins!), which supplies the calling crate's
    /// manifest directory. Where [`LOG_VAR`] names a file, the handle logs its pins there, as
    /// [`Pins::logging_to`] makes it.
    ///
    /// # Panics
    ///
    /// - If the running thread is named after a test other than `test`: a test pins under its own
    ///   name, so that `just repin <test>` re-pins what it pins.
    /// - As [`Pins::open_in_mode`].
    #[must_use]
    #[track_caller]
    pub fn open(manifest_dir: &str, file: &str, test: impl Into<String>) -> Self {
        let test = test.into();
        assert_own_name(&test);
        let pins = Self::open_in_mode(mode_from_env(), Path::new(manifest_dir), file, test);
        match std::env::var_os(LOG_VAR) {
            Some(log) => pins.logging_to(log),
            None => pins,
        }
    }

    /// Opens `<manifest_dir>/tests/pins/<file>.pins` for the test `test`, comparing in
    /// [`Mode::Compare`] and writing in [`Mode::Bless`].
    ///
    /// # Panics
    ///
    /// - If `file` is not segments of ASCII letters, digits, `_` and `-` joined by slashes, or
    ///   `test` not ASCII letters, digits and underscores.
    /// - In [`Mode::Compare`], if the file exists and cannot be read, or is not in its canonical
    ///   form, naming the line.
    /// - In [`Mode::BlessForbidden`]: always.
    #[must_use]
    #[track_caller]
    pub fn open_in_mode(
        mode: Mode,
        manifest_dir: &Path,
        file: &str,
        test: impl Into<String>,
    ) -> Self {
        let test = test.into();
        let path = pins_path(manifest_dir, file, &test);
        let source = match mode {
            Mode::Compare => {
                let pins = read_file(&path);
                Source::Compared { path, pins }
            }
            Mode::Bless => Source::Blessed { path },
            Mode::BlessForbidden => panic!(
                "refusing to bless {}: {BLESS_VAR} is set under {CI_VAR}; \
                 figures are pinned on a developer machine and committed",
                path.display()
            ),
        };
        Self {
            test,
            source,
            pinned: BTreeSet::new(),
            log: None,
        }
    }

    /// Makes every pin of the handle append `<pins file>\t<test>.<key>` to the file `log`, as
    /// [`LOG_VAR`] does for a handle [`Pins::open`] opens. An embedded handle logs nothing.
    #[must_use]
    pub fn logging_to(mut self, log: impl Into<PathBuf>) -> Self {
        self.log = Some(log.into());
        self
    }

    /// Opens the pins file `file` for the test `test` from `contents`, the file's text embedded
    /// in the test binary, as [`Mode::from_env`] says.
    ///
    /// This is what [`pins!`](crate::pins!) calls on `wasm32-unknown-unknown`, the browser target,
    /// where a test cannot read files; there the environment reads nothing, so the mode is always
    /// [`Mode::Compare`].
    ///
    /// # Panics
    ///
    /// - If the running thread is named after a test other than `test`, as [`Pins::open`].
    /// - As [`Pins::embedded_in_mode`].
    #[must_use]
    #[track_caller]
    pub fn embedded(file: &str, contents: &str, test: impl Into<String>) -> Self {
        let test = test.into();
        assert_own_name(&test);
        Self::embedded_in_mode(mode_from_env(), file, contents, test)
    }

    /// Opens the embedded text of the pins file `file` for the test `test`.
    ///
    /// It never blesses: figures are pinned natively, where the file can be written, and then
    /// embedded.
    ///
    /// # Panics
    ///
    /// - If `mode` is not [`Mode::Compare`].
    /// - If `file` or `test` is not a name [`Pins::open_in_mode`] takes.
    /// - If `contents` is not in the canonical form, naming the line.
    #[must_use]
    #[track_caller]
    pub fn embedded_in_mode(
        mode: Mode,
        file: &str,
        contents: &str,
        test: impl Into<String>,
    ) -> Self {
        let test = test.into();
        let shown = format!("{}.pins (embedded)", checked_file_name(file, &test));
        assert!(
            mode == Mode::Compare,
            "cannot bless {shown}: figures are pinned natively, then embedded"
        );
        let pins = PinFile::parse(contents).unwrap_or_else(|e| panic!("cannot read {shown}: {e}"));
        Self {
            test,
            source: Source::Embedded { shown, pins },
            pinned: BTreeSet::new(),
            log: None,
        }
    }

    /// Pins `value` under `key` exactly: integers equal, floats the same bits.
    ///
    /// # Panics
    ///
    /// As [`Pins::within`].
    #[track_caller]
    pub fn exact(&mut self, key: &str, value: impl Into<Value>) {
        self.pin(key, value.into(), Tolerance::EXACT);
    }

    /// Pins `value` under `key`, its floats within `tolerance` and its integers exactly.
    ///
    /// Comparing, it checks `value` against the file's. Blessing, it writes `value` to the file
    /// where comparing would fail, that is where the file lacks the key or holds a value that
    /// `value` does not match, and leaves the file as it is elsewhere. It writes under an
    /// exclusive lock on the file (wherever the platform has file locks: not WASI), so that tests
    /// blessing one file at once lose none of each other's pins.
    ///
    /// # Panics
    ///
    /// - If `key` is not segments of ASCII letters, digits, `_` and `-` joined by dots, or this
    ///   handle has pinned it already.
    /// - If `value` holds a NaN, whose bits are unspecified.
    /// - If the handle logs its pins to a file that cannot be written.
    /// - Comparing: if the file lacks the key, or holds a value that `value` does not match in
    ///   shape, integers or floats within `tolerance`, naming the key, both values and the command
    ///   that re-pins it.
    /// - Blessing: if the file cannot be locked, read or written, or is not in its canonical form.
    #[track_caller]
    pub fn within(&mut self, key: &str, value: impl Into<Value>, tolerance: Tolerance) {
        self.pin(key, value.into(), tolerance);
    }

    #[track_caller]
    fn pin(&mut self, key: &str, received: Value, tolerance: Tolerance) {
        let test = &self.test;
        assert!(
            is_key(key),
            "the pin key {key:?} of {test} must be segments of ASCII letters, digits, `_` and `-` \
             joined by dots"
        );
        let full = format!("{test}.{key}");
        assert!(
            !received.has_nan(),
            "{full} received a NaN, which cannot be pinned: its bits are unspecified"
        );
        assert!(
            self.pinned.insert(full.clone()),
            "{full} is pinned twice through one handle"
        );
        match &self.source {
            Source::Compared { path, pins } => {
                if let Some(log) = &self.log {
                    log_pin(log, path, &full);
                }
                let shown = path.display().to_string();
                compare(pins.as_ref(), &shown, test, &full, &received, tolerance);
            }
            Source::Blessed { path } => {
                if let Some(log) = &self.log {
                    log_pin(log, path, &full);
                }
                bless(path, &full, received, tolerance);
            }
            Source::Embedded { shown, pins } => {
                compare(Some(pins), shown, test, &full, &received, tolerance);
            }
        }
    }
}

/// The mode [`Pins::open`] and [`Pins::embedded`] take: [`Mode::from_env`]'s, or
/// [`Mode::Compare`] wherever [`KEEP_VAR`] is set.
#[must_use]
fn mode_from_env() -> Mode {
    kept(std::env::var_os(KEEP_VAR).as_deref(), Mode::from_env())
}

/// `mode`, or [`Mode::Compare`] where [`KEEP_VAR`]'s value `keep` is set to anything.
#[must_use]
fn kept(keep: Option<&OsStr>, mode: Mode) -> Mode {
    if keep.is_some() { Mode::Compare } else { mode }
}

/// Panics if the running thread is named after a test (as libtest and nextest name a test's
/// thread, by its path) whose name is not `test`. A thread of the test's own, unnamed or named
/// otherwise, is passed over.
#[track_caller]
fn assert_own_name(test: &str) {
    let thread = std::thread::current();
    let Some(running) = thread.name() else {
        return;
    };
    let last = running.rsplit("::").next().unwrap_or(running);
    assert!(
        running == "main" || !is_test_name(last) || last == test,
        "the test {running} opens pins as `{test}`: a test pins under its own name, so that \
         `just repin {last}` re-pins them"
    );
}

/// `file`, once known to name a pins file, for the test `test`.
#[must_use]
#[track_caller]
fn checked_file_name<'a>(file: &'a str, test: &str) -> &'a str {
    assert!(
        is_file_name(file),
        "the pins file {file:?} must be segments of ASCII letters, digits, `_` and `-` joined by \
         slashes"
    );
    assert!(
        is_test_name(test),
        "the test name {test:?} must be ASCII letters, digits and underscores"
    );
    file
}

#[must_use]
#[track_caller]
fn pins_path(manifest_dir: &Path, file: &str, test: &str) -> PathBuf {
    let mut path = manifest_dir.join("tests").join("pins");
    path.extend(checked_file_name(file, test).split('/'));
    path.set_extension("pins");
    path
}

/// The pins of the file at `path`, or `None` if there is no file, read under a shared lock so that
/// a blessing test's write is never seen half done.
#[track_caller]
fn read_file(path: &Path) -> Option<PinFile> {
    let shown = path.display();
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(e) => panic!("cannot open {shown}: {e}"),
    };
    match file.lock_shared() {
        // WASI has no file locks, and no threads.
        Err(e) if e.kind() != ErrorKind::Unsupported => panic!("cannot lock {shown}: {e}"),
        Ok(()) | Err(_) => {}
    }
    let mut text = String::new();
    file.read_to_string(&mut text)
        .unwrap_or_else(|e| panic!("cannot read {shown}: {e}"));
    Some(PinFile::parse(&text).unwrap_or_else(|e| panic!("cannot read {shown}: {e}")))
}

#[track_caller]
fn compare(
    pins: Option<&PinFile>,
    shown: &str,
    test: &str,
    full: &str,
    received: &Value,
    tolerance: Tolerance,
) {
    let Some(pinned) = pins.and_then(|pins| pins.get(full)) else {
        panic!(
            "{full} is not pinned: received {received}; pin it with `just repin {test}`\n  \
             file: {shown}"
        );
    };
    assert!(
        pinned.admits(received, tolerance),
        "{full} is stale\n  pinned:   {pinned}\n  received: {received}\nto match {tolerance}; \
         if the change is intended, re-pin it with `just repin {test}`\n  file: {shown}"
    );
}

/// Writes `received` under `full` in the file at `path`, under an exclusive lock, where the file
/// lacks `full` or holds a value that `received` does not match within `tolerance`.
#[track_caller]
fn bless(path: &Path, full: &str, received: Value, tolerance: Tolerance) {
    let shown = path.display();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("cannot create {}: {e}", parent.display()));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap_or_else(|e| panic!("cannot open {shown} to bless it: {e}"));
    match file.lock() {
        // WASI has no file locks, and no threads; `just repin` blesses natively.
        Err(e) if e.kind() != ErrorKind::Unsupported => panic!("cannot lock {shown}: {e}"),
        Ok(()) | Err(_) => {}
    }
    let mut text = String::new();
    file.read_to_string(&mut text)
        .unwrap_or_else(|e| panic!("cannot read {shown}: {e}"));
    let mut pins = if text.is_empty() {
        PinFile::new()
    } else {
        PinFile::parse(&text).unwrap_or_else(|e| panic!("cannot read {shown}: {e}"))
    };
    if pins
        .get(full)
        .is_some_and(|pinned| pinned.admits(&received, tolerance))
    {
        return;
    }
    pins.set(full, received);
    let text = pins.render();
    file.rewind()
        .and_then(|()| file.set_len(0))
        .and_then(|()| file.write_all(text.as_bytes()))
        .unwrap_or_else(|e| panic!("cannot write {shown}: {e}"));
}

/// Appends `<path>\t<full>` to the file `log`.
#[track_caller]
fn log_pin(log: &Path, path: &Path, full: &str) {
    let line = format!("{}\t{full}\n", path.display());
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(log)
        .and_then(|mut file| file.write_all(line.as_bytes()))
        .unwrap_or_else(|e| panic!("cannot append to the pins log {}: {e}", log.display()));
}

/// Opens the calling crate's pins file `file` for the test `test`, which must be the running
/// test's own name.
///
/// `pins!("census/compact", "the_compact_census_is_pinned")` opens
/// `tests/pins/census/compact.pins` under the calling crate's manifest directory. See
/// [`pins::Pins::open`](crate::pins::Pins::open).
///
/// On `wasm32-unknown-unknown`, the browser target, where a test cannot read files, it embeds the
/// file with `include_str!` instead and opens it with
/// [`pins::Pins::embedded`](crate::pins::Pins::embedded): there `file` must be a string literal
/// and the file must exist when the test is compiled. Elsewhere `file` may be computed. Each arm
/// sits behind a `#[cfg]` that the calling crate resolves, as in [`golden!`](crate::golden!).
#[macro_export]
macro_rules! pins {
    ($file:expr, $test:expr $(,)?) => {{
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        let pins = $crate::pins::Pins::embedded(
            $file,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/pins/",
                $file,
                ".pins"
            )),
            $test,
        );
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        let pins = $crate::pins::Pins::open(env!("CARGO_MANIFEST_DIR"), $file, $test);
        pins
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// A file of every kind of value, and its text, which must be the same bytes on every target.
    fn every_kind() -> (PinFile, String) {
        let mut pins = PinFile::new();
        let rows: [(&str, Value); 16] = [
            ("t.int", Value::from(471_u32)),
            ("t.negative", Value::from(-3_i8)),
            ("t.big", Value::from(u64::MAX)),
            ("t.count", Value::from(12_usize)),
            ("t.tenth", Value::from(0.1)),
            ("t.whole", Value::from(471.0)),
            ("t.negative_zero", Value::from(-0.0)),
            ("t.small", Value::from(1e-4)),
            ("t.smaller", Value::from(9.9e-5)),
            ("t.large", Value::from(9_999_999_999_999_998.0)),
            ("t.larger", Value::from(1e16)),
            ("t.least", Value::from(f64::from_bits(1))),
            ("t.infinity", Value::from(f64::NEG_INFINITY)),
            ("t.single", Value::from(0.1_f32)),
            ("t.tuple", Value::from((3_u8, 2.5, -1_i64))),
            ("u.table", Value::from([[1.5, 2.0], [-0.25, 1e-7]])),
        ];
        for (key, value) in rows {
            assert!(pins.set(key, value));
        }
        let text = format!(
            "{HEADER}\n\
             t.big = 18446744073709551615\n\
             t.count = 12\n\
             t.infinity = -inf\n\
             t.int = 471\n\
             t.large = 9999999999999998.0\n\
             t.larger = 1e16\n\
             t.least = 5e-324\n\
             t.negative = -3\n\
             t.negative_zero = -0.0\n\
             t.single = 0.10000000149011612\n\
             t.small = 0.0001\n\
             t.smaller = 9.9e-5\n\
             t.tenth = 0.1\n\
             t.tuple = [3, 2.5, -1]\n\
             t.whole = 471.0\n\
             u.table = [[1.5, 2.0], [-0.25, 1e-7]]\n"
        );
        (pins, text)
    }

    #[test]
    fn the_format_is_the_same_bytes_on_every_target_and_reads_back_to_the_same_bits() {
        let (pins, text) = every_kind();
        assert_eq!(pins.render(), text);
        assert_eq!(PinFile::parse(&text), Ok(pins));
        let least = PinFile::parse(&text).unwrap();
        assert_eq!(least.get("t.least"), Some(&Value::Float(f64::from_bits(1))));
    }

    /// The committed `tests/pins/every_target.pins`, through [`pins!`](crate::pins!), on every
    /// target the testkit's tests run on (embedded in the browser's test binary): its decimals read
    /// back to the bits computed here.
    #[test]
    fn the_committed_file_reads_back_to_the_same_bits_on_every_target() {
        let mut pins = crate::pins!(
            "every_target",
            "the_committed_file_reads_back_to_the_same_bits_on_every_target"
        );
        pins.exact("sum", 0.1 + 0.2);
        pins.exact("tiny", 1e-7);
        pins.exact("negative_zero", -0.0);
        pins.exact("least", f64::from_bits(1));
        pins.exact("single", 0.1_f32);
        pins.exact("largest", u64::MAX);
        pins.exact("row", (3_u8, 2.5, -1_i64));
    }

    #[test]
    fn floats_compare_by_their_bits_and_integers_never_match_floats() {
        assert_ne!(Value::from(0.0), Value::from(-0.0));
        assert_ne!(Value::from(1_u8), Value::from(1.0));
        assert_eq!(
            Value::from((1_u8, 2.0)),
            Value::from(vec![Value::Int(1), Value::Float(2.0)])
        );
        assert_eq!(Value::from(&[1_u16, 2][..]), Value::from([1_i32, 2]));
    }

    #[test]
    fn tolerances_admit_what_they_say() {
        assert!(Tolerance::EXACT.admits(0.1, 0.1));
        assert!(!Tolerance::EXACT.admits(0.1, 0.1_f64.next_up()));
        assert!(!Tolerance::EXACT.admits(0.0, -0.0));
        assert!(Tolerance::absolute(1e-3).admits(2.0, 2.001));
        assert!(!Tolerance::absolute(1e-3).admits(2.0, 2.0011));
        assert!(Tolerance::relative(1e-3).admits(2_000.0, 2_002.0));
        assert!(!Tolerance::relative(1e-3).admits(2_000.0, 2_002.1));
        assert!(Tolerance::absolute(1.0).admits(f64::INFINITY, f64::INFINITY));
        assert!(!Tolerance::absolute(1.0).admits(f64::INFINITY, f64::MAX));
        // Any bound times an infinity is infinite: an infinite pin matches itself alone.
        assert!(Tolerance::relative(0.01).admits(f64::NEG_INFINITY, f64::NEG_INFINITY));
        assert!(!Tolerance::relative(0.01).admits(f64::INFINITY, 1.0));
        assert!(!Tolerance::relative(0.01).admits(f64::INFINITY, f64::NEG_INFINITY));
        assert!(!Tolerance::absolute(1.0).admits(f64::NEG_INFINITY, f64::INFINITY));
        assert!(!Tolerance::absolute(1.0).admits(1.0, f64::INFINITY));
        let pinned = Value::from((1_u8, 2.0));
        assert!(pinned.admits(&Value::from((1_u8, 2.000_5)), Tolerance::absolute(1e-3)));
        assert!(!pinned.admits(&Value::from((2_u8, 2.0)), Tolerance::absolute(1e-3)));
        assert!(!pinned.admits(&Value::from((1_u8, 2.0, 3.0)), Tolerance::absolute(1e-3)));
        assert!(!pinned.admits(&Value::from((1.0, 2.0)), Tolerance::absolute(1e-3)));
    }

    #[test]
    fn the_keep_switch_makes_every_mode_compare() {
        let keep = Some(OsStr::new("1"));
        assert_eq!(kept(keep, Mode::Bless), Mode::Compare);
        assert_eq!(kept(keep, Mode::BlessForbidden), Mode::Compare);
        assert_eq!(kept(Some(OsStr::new("")), Mode::Bless), Mode::Compare);
        assert_eq!(kept(None, Mode::Bless), Mode::Bless);
        assert_eq!(kept(None, Mode::Compare), Mode::Compare);
    }

    #[test]
    fn setting_a_pin_says_whether_it_changed_the_file() {
        let mut pins = PinFile::new();
        assert!(pins.set("a.x", Value::from(0.0)));
        assert!(!pins.set("a.x", Value::from(0.0)));
        assert!(pins.set("a.x", Value::from(-0.0)));
        assert!(pins.set("a.x", Value::from(0_u8)));
    }

    #[test]
    fn a_file_out_of_its_canonical_form_does_not_read() {
        use ParsePinsErrorKind as K;
        let line = |text: &str| {
            PinFile::parse(&format!("{HEADER}\n{text}")).map_err(|e| (e.line(), e.kind()))
        };
        assert_eq!(line("a.x = 1\n").map(|p| p.keys().count()), Ok(1));
        assert_eq!(PinFile::parse(&format!("{HEADER}\n")), Ok(PinFile::new()));
        assert_eq!(
            PinFile::parse("a.x = 1\n").map_err(|e| e.kind()),
            Err(K::Header)
        );
        assert_eq!(line("a.x = 1"), Err((2, K::LineBreaks)));
        assert_eq!(line("a.x = 1\r\n"), Err((2, K::LineBreaks)));
        assert_eq!(line("a.x = 1\n\n"), Err((3, K::LineBreaks)));
        assert_eq!(line("a.x=1\n"), Err((2, K::Line)));
        assert_eq!(line("ax = 1\n"), Err((2, K::Key)));
        assert_eq!(line("a.x y = 1\n"), Err((2, K::Key)));
        assert_eq!(line("a..x = 1\n"), Err((2, K::Key)));
        for value in [
            "01", "-0", "+1", "1.", ".5", "1e3", "1.0e-7", "NaN", "[1,2]", "[1, 2", "x",
        ] {
            assert_eq!(
                line(&format!("a.x = {value}\n")),
                Err((2, K::Value)),
                "{value}"
            );
        }
        assert_eq!(line("a.y = 1\na.x = 2\n"), Err((3, K::Order)));
        assert_eq!(line("a.x = 1\na.x = 2\n"), Err((3, K::Order)));
    }
}
