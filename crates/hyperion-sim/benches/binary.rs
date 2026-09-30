//! Benchmarks of the binary evolution engine (plan 11, P11.T4).
//!
//! The target is plan 11's: a median under 200 µs per interacting binary, so that a system stays
//! inside the brainstorm's millisecond. `binary/distribution` times each of a fixed sample of close
//! pairs once, after a warm-up, and prints the median and 99th percentile, and each timing also in
//! calls of `math::exp` timed in the same process (`binary/reference`), since this laptop's clock
//! swings 1.9–4.8 GHz and other work shares it. A regression is a finding to raise, not a CI
//! failure: CI compiles these and never runs them.

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::math;
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::binary::{BinaryInput, BinaryParams, can_interact, evolve};
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::units::{GravitationalParameter, Radians, Seconds, SolarMasses, Years};

/// The age each pair is run to, years.
const UNTIL: f64 = 1.2e10;

/// The pairs of the distribution.
const SAMPLE: usize = 200;

/// A circular or eccentric pair of `m1` and `m2` M☉ on an orbit of `period` days.
fn pair(m1: f64, m2: f64, period: f64, e: f64) -> BinaryInput {
    let orbit = KeplerElements::from_period(
        Seconds::new(period * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::new(e).expect("an eccentricity in [0, 1)"),
        Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
            .expect("an orientation"),
        Radians::new(0.0),
    )
    .expect("an orbit");
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::SOLAR,
        orbit,
        [StarDraws::median(), StarDraws::median()],
        Years::new(1.0e10),
    )
    .expect("a pair")
}

/// A fixed sample of close pairs: log-uniform in primary mass (0.8–25 M☉) and period (0.5–3,000
/// d), uniform in mass ratio (0.1–1) and, above 5 d, in eccentricity (0–0.6), from a splitmix64
/// stream, keeping those that can interact ([`can_interact`]): the plan's figure is per
/// interacting binary.
#[expect(
    clippy::cast_precision_loss,
    reason = "a 53-bit word and 2⁵³ are exact in an f64"
)]
fn sample() -> Vec<BinaryInput> {
    let mut state: u64 = 0x5eed_0b1e;
    let mut next = || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z >> 11) as f64 / (1_u64 << 53) as f64
    };
    (0..SAMPLE)
        .map(|_| {
            let m1 = math::exp(math::ln(0.8) + next() * math::ln(25.0 / 0.8));
            let m2 = (m1 * (0.1 + 0.9 * next())).max(0.1);
            let period = math::exp(math::ln(0.5) + next() * math::ln(6_000.0));
            let e = if period > 5.0 { 0.6 * next() } else { 0.0 };
            pair(m1, m2, period, e)
        })
        .filter(|input| can_interact(input, Years::new(UNTIL)))
        .collect()
}

/// One `math::exp`, the unit the figures are normalised by.
fn exp_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary/reference");
    group.bench_function("math::exp", |b| {
        b.iter(|| math::exp(black_box(0.5)));
    });
    group.finish();
}

/// BSE's worked examples (Hurley, Tout and Pols 2002, sections 3.1 and 3.2), each run once per
/// iteration.
fn examples(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary");
    group.sample_size(20);
    let algol = pair(2.9, 0.9, 8.0, 0.7).with_params(BinaryParams::GENERATOR.with_alpha_ce(3.0));
    group.bench_function("binary_evolve/algol", |b| {
        b.iter(|| evolve(black_box(&algol), Years::new(UNTIL)));
    });
    let cv = pair(6.0, 1.3, 630.0, 0.0);
    group.bench_function("binary_evolve/cataclysmic_variable", |b| {
        b.iter(|| evolve(black_box(&cv), Years::new(UNTIL)));
    });
    group.finish();
}

/// Each pair of [`sample`] timed once after a warm-up: the median and 99th percentile per
/// interacting binary, in µs and in calls of `math::exp`.
fn distribution(_: &mut Criterion) {
    let pairs = sample();
    for input in pairs.iter().take(16) {
        black_box(evolve(input, Years::new(UNTIL)));
    }
    let exp = {
        let n = 10_000_000_u32;
        let start = Instant::now();
        let mut x = 0.0;
        for i in 0..n {
            x += math::exp(black_box(f64::from(i % 7) * 0.1));
        }
        black_box(x);
        start.elapsed().as_secs_f64() / f64::from(n)
    };
    let mut times: Vec<Duration> = pairs
        .iter()
        .map(|input| {
            let start = Instant::now();
            black_box(evolve(input, Years::new(UNTIL)));
            start.elapsed()
        })
        .collect();
    times.sort();
    let at = |percent: usize| times[(times.len() - 1) * percent / 100].as_secs_f64();
    let (median, p99) = (at(50), at(99));
    println!(
        "binary/distribution: {} pairs, median {:.1} µs ({:.0} exp), 99th percentile {:.1} µs \
         ({:.0} exp), exp {:.2} ns",
        times.len(),
        median * 1e6,
        median / exp,
        p99 * 1e6,
        p99 / exp,
        exp * 1e9,
    );
}

criterion_group!(binary, exp_reference, examples, distribution);
criterion_main!(binary);
