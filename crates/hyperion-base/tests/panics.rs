//! Every test of `hyperion-base` that expects a panic, in a test binary of its own.
//!
//! On `wasm32-unknown-unknown` a panic is a trap, after which every later test of the same binary
//! runs in a best-effort state (a leaked shadow stack, `std::thread::panicking()` left true), so
//! these tests are kept apart from the goldens and the unit tests (plan R04, Design note 12). Each
//! states `expected`, because on that target a bare `should_panic` passes on any trap. They moved
//! here from the unit tests of `rng` when it moved from the sim (R04.T4.d), unchanged but for
//! reaching the crate through its public API.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_base::Seed;
use hyperion_base::math::normal_quantile;
use hyperion_base::rng::{
    DomainTag, Mark, ObjectKey, RawEventKey, Stream, TagScope, Threshold, Thresholds,
    assert_registries_disjoint, assert_tag_names, tags,
};

/// A stream of `seed` under the self-test tag, for item `n` of the galaxy's lists.
fn stream(seed: u64, n: u64) -> Stream {
    Stream::open(
        Seed::new(seed),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy_item(n),
    )
}

// `math.rs`, which moved in T4.a.

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "normal_quantile needs 0 < p < 1, got 1")]
fn normal_quantile_rejects_one_in_debug_builds() {
    let _ = normal_quantile(1.0);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "normal_quantile needs 0 < p < 1, got 0")]
fn normal_quantile_rejects_zero_in_debug_builds() {
    let _ = normal_quantile(0.0);
}

// `rng/stream.rs`.

const STREAM_SEED: u64 = 0x5eed_0000_0000_0001;

#[test]
#[should_panic(expected = "past its end")]
fn drawing_past_the_last_block_panics() {
    let mut stream = stream(STREAM_SEED, 0);
    stream.seek(Stream::WORDS - 1);
    stream.next_u64();
    stream.next_u64();
}

#[test]
#[should_panic(expected = "cannot seek")]
fn seeking_past_the_end_panics() {
    stream(STREAM_SEED, 0).seek(Stream::WORDS + 1);
}

#[test]
#[should_panic(expected = "scope")]
fn a_scope_mismatch_panics() {
    const CELL_TAG: DomainTag = DomainTag::registered("selftest.cell", TagScope::Cell);
    let _ = Stream::open(Seed::new(STREAM_SEED), CELL_TAG, ObjectKey::galaxy());
}

#[test]
#[should_panic(expected = "scope")]
fn an_event_tag_is_never_opened_as_a_stream() {
    const EVENT_TAG: DomainTag = DomainTag::registered("selftest.event_tag", TagScope::Event);
    let _ = Stream::open(Seed::new(STREAM_SEED), EVENT_TAG, ObjectKey::galaxy());
}

// `rng/raw_event.rs`.

#[test]
#[should_panic(expected = "an event key needs a tag of scope Event")]
fn a_raw_event_key_refuses_a_tag_of_another_scope() {
    let _ = RawEventKey::derive(
        Seed::new(0x0e7e_0000_0000_0002),
        tags::SELFTEST_STREAM,
        [0, 0],
    );
}

// `rng/domain_tag.rs`.

#[test]
#[should_panic(expected = "duplicate domain tag name")]
fn a_duplicate_name_panics() {
    assert_tag_names(&["star.mass", "moon.count", "star.mass"]);
}

#[test]
#[should_panic(expected = "must match")]
fn a_malformed_name_panics() {
    assert_tag_names(&["star.mass", "Moon.count"]);
}

#[test]
#[should_panic(expected = "a domain tag name is in two registries")]
fn a_name_in_two_registries_panics() {
    let a = DomainTag::registered("selftest.a", TagScope::SelfTest);
    let b = DomainTag::registered("selftest.b", TagScope::SelfTest);
    assert_registries_disjoint(&[&[a], &[b], &[a]]);
}

// `rng/decide.rs`.

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "exceeds its thinning bound")]
fn a_ratio_above_its_bound_panics_in_debug() {
    let _ = Threshold::from_ratio(1.5, 1.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "exceeds its thinning bound")]
fn weights_above_their_bound_panic_in_debug() {
    let _ = Thresholds::from_weights(&[0.5, 0.7], 1.0);
}

/// Mark 0 picks class 0 at once; the total is still checked.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "exceeds its thinning bound")]
fn pick_weighted_checks_the_total_after_an_early_pick() {
    let _ = Mark::from_word(0).pick_weighted(&[0.5, 0.7], 1.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "must lie in [0, 1]")]
fn a_probability_above_one_panics_in_debug() {
    let _ = Threshold::from_probability(1.0 + f64::EPSILON);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not a density")]
fn a_negative_weight_panics_in_debug() {
    let _ = Mark::from_word(u64::MAX).pick_weighted(&[0.5, -0.1], 1.0);
}

// `rng/sample/poisson.rs`.

const POISSON_SEED: u64 = 0x0901_5504;

#[test]
#[should_panic(expected = "a Poisson mean must lie in [0, 2^31]")]
fn a_negative_mean_panics() {
    let _ = stream(POISSON_SEED, 5).poisson(-0.5);
}

#[test]
#[should_panic(expected = "a Poisson mean must lie in [0, 2^31]")]
fn a_nan_mean_panics() {
    let _ = stream(POISSON_SEED, 5).poisson(f64::NAN);
}

#[test]
#[should_panic(expected = "a Poisson mean must lie in [0, 2^31]")]
fn an_infinite_mean_panics() {
    let _ = stream(POISSON_SEED, 5).poisson(f64::INFINITY);
}

// `rng/sample/normal.rs`.

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "standard deviation must be finite and non-negative")]
fn a_negative_sigma_panics_in_debug() {
    let _ = Stream::open(
        Seed::new(0x0b0e_d0e5),
        tags::SELFTEST_STREAM,
        ObjectKey::galaxy(),
    )
    .normal(0.0, -1.0);
}
