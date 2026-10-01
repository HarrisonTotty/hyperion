//! The testkit's `should_panic` tests, in a binary of their own.
//!
//! On `wasm32-unknown-unknown` a panic is a trap, and every test of a binary runs in one instance,
//! which carries on after a trap as best it can; so each states `expected`, which proves the
//! intended panic happened, and none shares a binary with a golden test (plan R04, Design note
//! 12).

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::Cell;
use std::path::Path;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::golden::{
    GoldenWriter, Mode, check_embedded, check_embedded_in_mode, check_in_mode,
};
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{
    ALPHA, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample, ks_two_sample,
    regularised_gamma_q,
};

fn text(version: u32, lines: &[&str]) -> String {
    let mut w = GoldenWriter::new();
    w.header(version);
    for line in lines {
        w.line(line);
    }
    w.finish()
}

// float

#[test]
#[should_panic(expected = "float bits differ")]
fn different_bits_fail() {
    assert_same_bits(0.1 + 0.2, 0.3);
}

#[test]
#[should_panic(expected = "bits of a NaN are unspecified")]
fn nan_is_refused() {
    let _ = bits(f64::NAN);
}

// golden

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
#[should_panic(
    expected = "e.golden (embedded) differs at line 3\n  golden: b = 2\n  actual: b = 9"
)]
fn embedded_mismatch_names_the_first_differing_line() {
    check_embedded(
        "e",
        &text(1, &["a = 1", "b = 2", "c = 3"]),
        &text(1, &["a = 1", "b = 9", "c = 3"]),
    );
}

#[test]
#[should_panic(
    expected = "has header generator_version = 1, but the test expects generator_version = 2"
)]
fn embedded_header_version_mismatch_is_its_own_failure() {
    check_embedded("h", &text(1, &["a = 1"]), &text(2, &["a = 1"]));
}

#[test]
#[should_panic(expected = "golden files are blessed natively")]
fn embedded_check_refuses_to_bless() {
    let same = text(1, &["a = 1"]);
    check_embedded_in_mode(Mode::Bless, "b", &same, &same);
}

#[test]
#[should_panic(expected = "golden files are blessed natively")]
fn embedded_check_refuses_to_bless_under_ci_too() {
    let same = text(1, &["a = 1"]);
    check_embedded_in_mode(Mode::BlessForbidden, "b", &same, &same);
}

#[test]
#[should_panic(expected = "has trailing whitespace on line 2")]
fn embedded_check_refuses_a_trailing_space() {
    let bad = "# generator_version = 1\nx \n";
    check_embedded("x", bad, bad);
}

// order

#[test]
#[should_panic(expected = "order dependence at position 0")]
fn hidden_counter_fails() {
    let calls = Cell::new(0_u64);
    let keys: Vec<u64> = (0..10).collect();
    assert_order_independent(&keys, |k| {
        calls.set(calls.get() + 1);
        k + calls.get()
    });
}

// stats

#[test]
#[should_panic(expected = "expected counts sum to")]
fn mismatched_totals_panic() {
    let _ = chi_square_gof(&[50, 50], &[60.0, 60.0]);
}

#[test]
#[should_panic(expected = "sample 1 is NaN")]
fn a_nan_in_both_samples_panics_instead_of_hanging() {
    let _ = ks_two_sample(&mut [0.5, f64::NAN], &mut [0.25, f64::NAN]);
}

#[test]
#[should_panic(expected = "sample 0 is NaN")]
fn a_nan_in_one_sample_panics() {
    let _ = ks_one_sample(&mut [f64::NAN, 0.5], |x| x.clamp(0.0, 1.0));
}

#[test]
#[should_panic(expected = "loaded: p-value 1e-4 is below alpha")]
fn p_value_below_alpha_fails_with_the_name() {
    assert_p_value("loaded", 1e-4, ALPHA);
}

#[test]
#[should_panic(expected = "is not a probability")]
fn nan_p_value_fails() {
    assert_p_value("broken", f64::NAN, ALPHA);
}

#[test]
#[should_panic(
    expected = "outside: observed 121 outside the Poisson interval [81, 120] of mean 100"
)]
fn count_outside_the_interval_fails_naming_both() {
    assert_poisson_count("outside", 121, 100.0, 0.05);
}

#[test]
#[should_panic(expected = "must be positive and finite")]
fn non_positive_shape_panics() {
    let _ = regularised_gamma_q(0.0, 1.0);
}
