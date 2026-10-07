//! Benchmarks of the binary evolution engine (plan 11, P11.T4).
//!
//! The target is plan 11's: a median under 200 µs per interacting binary, so that a system stays
//! inside the brainstorm's millisecond. `binary/distribution` times each of a fixed sample of close
//! pairs once, after a warm-up, and prints the median and 99th percentile, and each timing also in
//! calls of `math::exp` timed in the same process (`binary/reference`), since this laptop's clock
//! swings 1.9–4.8 GHz and other work shares it. A regression is a finding to raise, not a CI
//! failure: CI compiles these and never runs them.
//!
//! `binary/pair_light_bound` times the census's pair bound (P11.T17) over a fixed sample of
//! queries, against its target of at most 1 µs a call.

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use hyperion_sim::math;
use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::binary::{
    BinaryInput, BinaryParams, can_interact, evolve, pair_light_bound,
};
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::units::consts::SOLAR_RADIUS_M;
use hyperion_sim::units::{
    Dex, GravitationalParameter, HeliumExcess, Metres, Radians, Seconds, SolarMasses, Years,
};

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

/// A splitmix64 stream of uniform variates in [0, 1) from `seed`.
fn uniforms(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit word and 2⁵³ are exact in an f64"
        )]
        let u = (z >> 11) as f64 / (1_u64 << 53) as f64;
        u
    }
}

/// The census's queries of the pair bound: masses log-uniform over 0.08–150 M☉ with the lighter
/// second, the least periastron log-uniform over 1–10⁵ R☉, the age log-uniform over
/// 10⁶–1.4 × 10¹⁰ years and \[Fe/H\] uniform over −2.5 to +0.3.
type Query = (SolarMasses, SolarMasses, Metres, Composition, Years);

/// [`Query`]s, `n` of them, from a fixed stream.
fn queries(n: usize) -> Vec<Query> {
    let mut next = uniforms(0x5eed_0b1e_11c7);
    let log_uniform = |u: f64, lo: f64, hi: f64| math::exp(math::ln(lo) + u * math::ln(hi / lo));
    (0..n)
        .map(|_| {
            let a = log_uniform(next(), 0.08, 150.0);
            let b = log_uniform(next(), 0.08, a);
            let periastron = log_uniform(next(), 1.0, 1.0e5) * SOLAR_RADIUS_M;
            let age = log_uniform(next(), 1.0e6, 1.4e10);
            let fe_h = -2.5 + 2.8 * next();
            (
                SolarMasses::new(a),
                SolarMasses::new(b),
                Metres::new(periastron),
                Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
                Years::new(age),
            )
        })
        .collect()
}

/// The pair bound over 1,024 queries an iteration, in queries a second.
fn light_bound(c: &mut Criterion) {
    let sample = queries(1_024);
    let detached = sample
        .iter()
        .filter(|(a, b, p, comp, age)| pair_light_bound(*a, *b, *p, comp, *age..=*age).is_some())
        .count();
    println!(
        "binary/pair_light_bound: {detached} of {} queries Detached",
        sample.len()
    );
    let mut group = c.benchmark_group("binary/pair_light_bound");
    group.throughput(Throughput::Elements(
        u64::try_from(sample.len()).expect("a small sample"),
    ));
    group.bench_function("queries", |bench| {
        bench.iter(|| {
            for (a, b, p, comp, age) in &sample {
                black_box(pair_light_bound(*a, *b, *p, comp, *age..=*age));
            }
        });
    });
    group.finish();
}

/// A fixed sample of close pairs: log-uniform in primary mass (0.8–25 M☉) and period (0.5–3,000
/// d), uniform in mass ratio (0.1–1) and, above 5 d, in eccentricity (0–0.6), from a splitmix64
/// stream, keeping those that can interact ([`can_interact`]): the plan's figure is per
/// interacting binary.
fn sample() -> Vec<BinaryInput> {
    let mut next = uniforms(0x5eed_0b1e);
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

criterion_group!(binary, exp_reference, examples, distribution, light_bound);
criterion_main!(binary);
