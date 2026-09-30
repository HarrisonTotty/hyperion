//! Benchmarks of the gas and dust field (plan 07). Targets are recorded in the plan; a miss is a
//! finding to raise, not a CI failure: CI compiles these and never runs them.
//!
//! Every figure is also given in calls of `math::exp`, timed in the same process immediately before
//! and after the other groups (`gas/reference`), because this laptop's clock swings 1.9–4.8 GHz and
//! other work shares it; a per-call cost only means something against that.
//!
//! The targets, which are findings and never gates:
//!
//! - `log_normal_factor`, `Full`, cold (no cache): 2 µs (P07.T4.c).
//! - `log_normal_factor`, `Full`, per evaluation along a line in 32 ly steps with a 4,096-entry
//!   cache: 0.5 µs (P07.T4.c).
//! - `sightline` from 26,000 ly to the centre, `Realised`: 3 ms at `Full`, 0.2 ms at `Budget(64)`;
//!   2,000 lines of 500 ly from one origin with a shared cache: 100 ms on one core (P07.T8.e).
//! - `render_extinction_rows`, one thread: a 512 × 512 face-on map in 1 s, the 512 × 256 edge-on
//!   raster in 10 s (P07.T9).
//!
//! Measured on 2026-09-23 with `just bench -- gas`'s binary, at a load average of 16.6 falling to
//! 12.1 and 2.6–2.9 GHz (`scaling_cur_freq`) while six other lanes built, where `math::exp` took
//! 35.3 ns before the groups and 17.7 ns after, against 7.4–7.9 ns idle. A figure's `math::exp`
//! calls are its time over each of the two; the idle estimate is that range times 7.6 ns.
//!
//! | Benchmark | Time | `math::exp` calls | At idle | Target |
//! | --- | --- | --- | --- | --- |
//! | `log_normal_factor`, cold | 6.45 µs | 183–365 | 1.4–2.8 µs | 2 µs |
//! | `log_normal_factor`, on a line | 1.81 µs | 51–102 | 0.39–0.78 µs | 0.5 µs |
//! | `GasField::new` | 73 µs | 2,080–4,150 | 16–32 µs | — |
//! | `sightline` to the centre, `Full` | 6.9 ms | 195,000–389,000 | 1.5–3.0 ms | 3 ms |
//! | `sightline` to the centre, `Budget(64)` | 259 µs | 7,300–14,600 | 56–111 µs | 0.2 ms |
//! | 2,000 lines of 500 ly | 189 ms | 5.4–10.7 × 10⁶ | 41–81 ms | 100 ms |
//! | 512 × 512 face-on map | 2.25 s | 64–127 × 10⁶ | 0.48–0.97 s | 1 s |
//! | 512 × 256 edge-on map | 1.68 s | 47–95 × 10⁶ | 0.36–0.72 s | 10 s |
//!
//! The two noise figures straddle their targets within the load's spread and want a quiet
//! machine; everything else meets its target with room. A cold factor draws forty lattice normals,
//! each a Threefry block, a logarithm and a sine and cosine; on a line the cache leaves some four
//! of the forty to draw, by the lattice faces a 32 ly step crosses at each octave.

use std::hint::black_box;

use std::num::NonZeroU32;
use std::time::Duration;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticDisplacement;
use hyperion_sim::coords::{GalacticPosition, UnitVector};
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, sightline};
use hyperion_sim::galaxy::gas::field::GasField;
use hyperion_sim::galaxy::gas::map::render_extinction_rows;
use hyperion_sim::galaxy::gas::noise::{NoiseCache, SmoothingScale, log_normal_factor};
use hyperion_sim::galaxy::gas::params::GasParams;
use hyperion_sim::galaxy::map::{MapSelection, MapSpec, MapView};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::math;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;

/// The fixture's `σ_ln`.
const SIGMA_LN: f64 = 2.3;

/// The number of points on the benchmarked line: 32,000 ly in 32 ly steps.
const LINE_POINTS: u32 = 1_000;

/// One `math::exp`, the unit every figure here is normalised by, timed before the other groups and
/// again after them.
fn exp_before(c: &mut Criterion) {
    let mut group = c.benchmark_group("gas/reference");
    group.bench_function("math::exp (before)", |b| {
        b.iter(|| math::exp(black_box(0.5)));
    });
    group.finish();
}

/// [`exp_before`] again, after every other group.
fn exp_after(c: &mut Criterion) {
    let mut group = c.benchmark_group("gas/reference");
    group.bench_function("math::exp (after)", |b| {
        b.iter(|| math::exp(black_box(0.5)));
    });
    group.finish();
}

/// The lattice noise: one factor with nothing cached, and the mean cost along a line of sight.
fn noise(c: &mut Criterion) {
    let seed = Seed::new(0x0700_be4c);
    let p = GalacticPosition::from_light_years([26_013.7, -417.3, 30.9]).expect("in the cube");
    let mut group = c.benchmark_group("gas/noise");
    // A cache of no slots draws every one of the forty lattice normals afresh, which is a cold
    // evaluation without the cost of emptying a large table inside the timed loop.
    let mut none = NoiseCache::with_capacity(0);
    group.bench_function("log_normal_factor (Full, cold)", |b| {
        b.iter(|| {
            log_normal_factor(
                seed,
                black_box(&p),
                SIGMA_LN,
                SmoothingScale::Full,
                &mut none,
            )
        });
    });
    // From 26,000 ly towards the centre in the plane, 32 ly a step; the cache starts empty for
    // every walk, so the figure includes the misses a real line takes.
    let line: Vec<GalacticPosition> = (0..LINE_POINTS)
        .map(|i| {
            let x = 26_000.0 - 32.0 * f64::from(i);
            GalacticPosition::from_light_years([x, 1_234.5, 12.25]).expect("in the cube")
        })
        .collect();
    group.throughput(Throughput::Elements(u64::from(LINE_POINTS)));
    group.bench_function("log_normal_factor (Full, 32 ly line, 4,096 entries)", |b| {
        b.iter_batched_ref(
            || NoiseCache::with_capacity(4_096),
            |cache| {
                let mut sum = 0.0;
                for p in &line {
                    sum += log_normal_factor(seed, p, SIGMA_LN, SmoothingScale::Full, cache);
                }
                sum
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// The Milky Way fixture's gas on its own stellar fields, and those fields.
fn milky_way() -> (Fields, GasField) {
    let params = GalaxyParams::milky_way_like();
    let fields = Fields::new(&params, &MassModel::new(&params));
    let gas = GasField::with_params(Seed::new(0x0700_be4c), GasParams::milky_way_like(), &fields);
    (fields, gas)
}

/// Building the field, which the galaxy handle does once per galaxy.
fn field(c: &mut Criterion) {
    let params = GalaxyParams::milky_way_like();
    let fields = Fields::new(&params, &MassModel::new(&params));
    let mut group = c.benchmark_group("gas/field");
    group.bench_function("GasField::new", |b| {
        b.iter(|| GasField::new(black_box(Seed::new(7)), &params, &fields));
    });
    group.finish();
}

/// The extinction integral: the line from the Sun's radius to the centre, realised, at full
/// quality and at 64 steps, and a chart's worth of short lines from one origin (P07.T8.e).
fn extinction(c: &mut Criterion) {
    let (_, gas) = milky_way();
    let at = |ly| GalacticPosition::from_light_years(ly).expect("in the cube");
    let (sun, centre) = (at([26_000.0, 0.0, 0.0]), at([0.0, 0.0, 0.0]));
    let sixty_four = Quality::Budget(NonZeroU32::new(64).expect("sixty-four"));
    let mut group = c.benchmark_group("gas/extinction");
    group.sample_size(20);
    for (name, quality) in [("Full", Quality::Full), ("Budget(64)", sixty_four)] {
        group.bench_function(format!("sightline to the centre (Realised, {name})"), |b| {
            b.iter_batched_ref(
                || NoiseCache::with_capacity(4_096),
                |cache| {
                    sightline(
                        &gas,
                        black_box(&sun),
                        &centre,
                        NoiseMode::Realised,
                        quality,
                        &[],
                        cache,
                    )
                },
                BatchSize::SmallInput,
            );
        });
    }
    // Two thousand directions from a Sun-like origin, each 500 ly long, over a Fibonacci sphere.
    let origin = at([0.0, 26_000.0, 20.0]);
    let ends: Vec<GalacticPosition> = (0..2_000_u32)
        .map(|i| {
            let k = f64::from(i) + 0.5;
            let z = 1.0 - 2.0 * k / 2_000.0;
            let (sin, cos) = math::sin_cos(k * core::f64::consts::PI * (3.0 - 5.0_f64.sqrt()));
            let rho = (1.0 - z * z).sqrt();
            let direction = UnitVector::from_components([rho * cos, rho * sin, z]).expect("a unit");
            let metres = direction
                .components()
                .map(|c| c * 500.0 * METRES_PER_LIGHT_YEAR);
            origin
                .translated(GalacticDisplacement::new(metres))
                .expect("in the cube")
        })
        .collect();
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    group.bench_function(
        "2,000 lines of 500 ly from one origin (Realised, Full, shared cache)",
        |b| {
            b.iter_batched_ref(
                || NoiseCache::with_capacity(4_096),
                |cache| {
                    ends.iter()
                        .map(|end| {
                            sightline(
                                &gas,
                                &origin,
                                end,
                                NoiseMode::Realised,
                                Quality::Full,
                                &[],
                                cache,
                            )
                            .a_v()
                            .value()
                        })
                        .sum::<f64>()
                },
                BatchSize::SmallInput,
            );
        },
    );
    group.finish();
}

/// The extinction maps at the M1 display's largest raster, on one thread (P07.T9).
fn map(c: &mut Criterion) {
    let (_, gas) = milky_way();
    let mut group = c.benchmark_group("gas/map");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    for (name, view, size) in [
        (
            "render_extinction_rows (512 × 512 face-on)",
            MapView::FaceOn,
            [512, 512],
        ),
        (
            "render_extinction_rows (512 × 256 edge-on)",
            MapView::EdgeOn,
            [512, 256],
        ),
    ] {
        let spec = MapSpec::new(view, MapSelection::AllSystems, size, [0.0, 0.0], 256.0)
            .expect("the whole root cube");
        let mut out = Vec::new();
        group.bench_function(name, |b| {
            b.iter(|| render_extinction_rows(&gas, black_box(&spec), 0..size[1], &mut out));
        });
    }
    group.finish();
}

criterion_group!(gas, exp_before, noise, field, extinction, map, exp_after);
criterion_main!(gas);
