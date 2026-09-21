//! The golden harness against real files, in Cargo's per-target temporary directory.

use std::path::PathBuf;

use hyperion_testkit::golden::{GoldenWriter, Mode, check_in_mode};

/// A fresh fake crate directory for one test.
fn scratch(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("golden-harness")
        .join(test);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("scratch directory is removable");
    }
    dir
}

fn text(version: u32, lines: &[&str]) -> String {
    let mut w = GoldenWriter::new();
    w.header(version);
    for line in lines {
        w.line(line);
    }
    w.finish()
}

#[test]
fn bless_writes_the_file_and_a_match_then_passes() {
    let dir = scratch("bless");
    let expected = text(1, &["a = 1", "b = 2"]);
    check_in_mode(Mode::Bless, &dir, "nested/name", &expected);
    let written = std::fs::read_to_string(dir.join("tests/golden/nested/name.golden"))
        .expect("bless created the file and its directories");
    assert_eq!(written, expected);
    check_in_mode(Mode::Compare, &dir, "nested/name", &expected);
}

#[test]
#[should_panic(expected = "differs at line 3")]
fn mismatch_names_the_first_differing_line() {
    let dir = scratch("mismatch");
    check_in_mode(
        Mode::Bless,
        &dir,
        "m",
        &text(1, &["a = 1", "b = 2", "c = 3"]),
    );
    check_in_mode(
        Mode::Compare,
        &dir,
        "m",
        &text(1, &["a = 1", "b = 9", "c = 3"]),
    );
}

#[test]
#[should_panic(expected = "bump GENERATOR_VERSION and run `just bless`")]
fn mismatch_says_how_to_bless() {
    let dir = scratch("mismatch-hint");
    check_in_mode(Mode::Bless, &dir, "m", &text(1, &["a = 1"]));
    check_in_mode(Mode::Compare, &dir, "m", &text(1, &["a = 1", "extra"]));
}

#[test]
#[should_panic(
    expected = "has header generator_version = 1, but the test expects generator_version = 2"
)]
fn header_version_mismatch_is_its_own_failure() {
    let dir = scratch("header");
    check_in_mode(Mode::Bless, &dir, "h", &text(1, &["a = 1"]));
    check_in_mode(Mode::Compare, &dir, "h", &text(2, &["a = 1"]));
}

#[test]
#[should_panic(expected = "cannot read golden file")]
fn missing_file_fails() {
    let dir = scratch("missing");
    check_in_mode(Mode::Compare, &dir, "nothing", &text(1, &[]));
}

#[test]
#[should_panic(expected = "refusing to bless")]
fn bless_under_ci_panics() {
    let dir = scratch("ci");
    check_in_mode(Mode::BlessForbidden, &dir, "c", &text(1, &["a = 1"]));
}

#[test]
#[should_panic(expected = "must begin with")]
fn text_without_a_header_is_refused() {
    let dir = scratch("no-header");
    check_in_mode(Mode::Bless, &dir, "n", "a = 1\n");
}
