//! Benchmarks of the stellar stage (plan 06).
//!
//! Targets are recorded in plan 06's "Verification": `ZCoeffs::new` under 5 µs; `stellar::lifetime`
//! about 5 µs for a primary of layer D or E (2.5–8 and 8–150 M☉, plan 03), which plan 08's
//! placement calls up to twice per accepted record; and a remnant's full track within the 150 µs
//! that plan 06 gives `generate` for a remnant. A regression is a finding to raise, not a CI
//! failure: CI compiles these and never runs them.
//!
//! Every track figure is also given in calls of `math::exp`, timed in the same process before and
//! after the other groups (`stellar/reference`), because this laptop's clock swings 1.9–4.8 GHz
//! and other work shares it; a per-call cost only means something against that. The measurement
//! is recorded in plan 06's "Deviations in T10.c–e, as built".

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::math;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::sse::{Track, ZCoeffs, zams};
use hyperion_sim::stellar::{Composition, evolve, lifetime};
use hyperion_sim::units::{MetalFraction, SolarMasses, Years};

/// One `math::exp`, the unit the track figures are normalised by, before the other groups.
fn exp_before(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar/reference");
    group.bench_function("math::exp (before)", |b| {
        b.iter(|| math::exp(black_box(0.5)));
    });
    group.finish();
}

/// `math::powf` against `math::powf_positive`, the stellar formulae's power (ruling 77.1), at a
/// mass-luminosity law's arguments.
fn powers(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar/reference");
    group.bench_function("math::powf", |b| {
        b.iter(|| math::powf(black_box(5.3), black_box(3.8)));
    });
    group.bench_function("math::powf_positive", |b| {
        b.iter(|| math::powf_positive(black_box(5.3), black_box(3.8)));
    });
    group.finish();
}

/// [`exp_before`] again, after every other group.
fn exp_after(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar/reference");
    group.bench_function("math::exp (after)", |b| {
        b.iter(|| math::exp(black_box(0.5)));
    });
    group.finish();
}

/// Every metallicity-dependent coefficient at one Z, and the zero-age main sequence from them.
fn backbone(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar");
    group.bench_function("zcoeffs_new", |b| {
        b.iter(|| ZCoeffs::new(black_box(MetalFraction::new(0.004))));
    });
    let coeffs = ZCoeffs::new(MetalFraction::new(0.004));
    group.bench_function("zams_luminosity_and_radius", |b| {
        b.iter(|| {
            let m = black_box(SolarMasses::new(3.7));
            (zams::luminosity(m, &coeffs), zams::radius(m, &coeffs))
        });
    });
    group.finish();
}

/// The track integrator (P06.T10.c–e) under the generator's options: the lifetime of layer D and E
/// primaries, tracks built to an age for a main-sequence dwarf and a giant, full tracks of stars
/// dead at the epoch, and one state of a built track.
fn tracks(c: &mut Criterion) {
    let solar = Composition::SOLAR;
    let draws = StarDraws::median();
    let mut group = c.benchmark_group("stellar/track");
    for (name, m) in [
        ("lifetime (4 Msun, layer D)", 4.0),
        ("lifetime (20 Msun, layer E)", 20.0),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| lifetime(black_box(SolarMasses::new(m)), &solar, &draws));
        });
    }
    for (name, m, age) in [
        ("to_age (0.4 Msun dwarf at 5 Gyr)", 0.4, 5e9),
        ("to_age (1 Msun at 4.6 Gyr)", 1.0, 4.6e9),
        ("to_age (2 Msun giant at 1.2 Gyr)", 2.0, 1.2e9),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                Track::to_age(
                    black_box(SolarMasses::new(m)),
                    &solar,
                    &draws,
                    Years::new(age),
                )
            });
        });
    }
    for (name, m) in [
        ("full (1 Msun, white dwarf)", 1.0),
        ("full (5 Msun, white dwarf)", 5.0),
        ("full (20 Msun, neutron star)", 20.0),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| Track::full(black_box(SolarMasses::new(m)), &solar, &draws));
        });
    }
    // `evolve` at a typical age for its mass: a dwarf, the Sun, an intermediate-mass star on its
    // main sequence and a massive one halfway through its main sequence.
    for (name, m, age) in [
        ("evolve (0.3 Msun at 5 Gyr)", 0.3, 5e9),
        ("evolve (1 Msun at 4.6 Gyr)", 1.0, 4.6e9),
        ("evolve (2 Msun at 0.6 Gyr)", 2.0, 6e8),
        ("evolve (20 Msun at 4 Myr)", 20.0, 4e6),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                evolve(
                    black_box(SolarMasses::new(m)),
                    &solar,
                    &draws,
                    Years::new(age),
                )
            });
        });
    }
    let sun = Track::full(SolarMasses::new(1.0), &solar, &draws);
    group.bench_function("state_at (1 Msun at 4.6 Gyr)", |b| {
        b.iter(|| sun.state_at(black_box(Years::new(4.6e9))));
    });
    let giant = Track::full(SolarMasses::new(5.0), &solar, &draws);
    let late = giant
        .lifetime()
        .map_or(Years::new(1e8), |l| Years::new(l.value() * 0.999));
    group.bench_function("state_at (5 Msun on the AGB)", |b| {
        b.iter(|| giant.state_at(black_box(late)));
    });
    group.finish();
}

criterion_group!(stellar, exp_before, powers, backbone, tracks, exp_after);
criterion_main!(stellar);
