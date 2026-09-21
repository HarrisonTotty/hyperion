//! Benchmarks of the galaxy model (plan 02). Targets are recorded in the plan; a miss is a finding
//! to raise, not a CI failure: CI compiles these and never runs them.
//!
//! - `PotentialTables::in_plane`: 50 ms; `PotentialTables::full`: 2 s (P02.T6).
//! - `GalaxyParams::from_seed`, which includes the black hole's σ estimator (P02.T6.e).
//! - `Fields::densities` at a disc point: 400 ns, on which plan 03's 1–2 µs per sparse cell rests
//!   (P02.T7); `Fields::new`, most of it the sub-discs' Jeans solve.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::fields::{Fields, MAX_COMPONENTS};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::units::LightYears;

/// The potential of the Milky Way fixture: the model, its tables, and lookups.
fn potential(c: &mut Criterion) {
    let params = GalaxyParams::milky_way_like();
    let model = MassModel::new(&params);
    let mut group = c.benchmark_group("potential");
    group.bench_function("MassModel::new", |b| {
        b.iter(|| MassModel::new(black_box(&params)));
    });
    group.bench_function("MassModel::v_circ_sq", |b| {
        b.iter(|| model.v_circ_sq(black_box(LightYears::new(26_000.0))));
    });
    group.bench_function("PotentialTables::in_plane", |b| {
        b.iter(|| PotentialTables::in_plane(black_box(&model)));
    });
    let tables = PotentialTables::in_plane(&model);
    group.bench_function("PotentialTables::v_circ", |b| {
        b.iter(|| tables.v_circ(black_box(LightYears::new(26_000.0))));
    });
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.bench_function("PotentialTables::full", |b| {
        b.iter(|| PotentialTables::full(black_box(&model)));
    });
    group.finish();
}

/// A seed's parameters, the black hole's two-phase build included.
fn params(c: &mut Criterion) {
    let mut group = c.benchmark_group("params");
    group.bench_function("GalaxyParams::from_seed", |b| {
        b.iter(|| GalaxyParams::from_seed(black_box(Seed::new(7)), MassFunctionKind::Kroupa));
    });
    group.finish();
}

/// The density fields of the Milky Way fixture: building them, and every component's density at a
/// point, the placement's innermost loop.
fn fields(c: &mut Criterion) {
    let params = GalaxyParams::milky_way_like();
    let model = MassModel::new(&params);
    let fields = Fields::new(&params, &model);
    let mut group = c.benchmark_group("fields");
    let mut out = [0.0; MAX_COMPONENTS];
    // 26,000 ly out, 30° from the bar, 50 ly above the plane: the solar circle.
    let disc = PointLy::new(22_516.7, 13_000.0, 50.0);
    group.bench_function("Fields::densities (disc point)", |b| {
        b.iter(|| fields.densities(black_box(&disc), &mut out));
    });
    // Inside the bulge and the bar, off every axis.
    let bulge = PointLy::new(900.0, 400.0, 150.0);
    group.bench_function("Fields::densities (bulge point)", |b| {
        b.iter(|| fields.densities(black_box(&bulge), &mut out));
    });
    group.sample_size(10);
    group.bench_function("Fields::new", |b| {
        b.iter(|| Fields::new(black_box(&params), &model));
    });
    group.finish();
}

criterion_group!(galaxy, potential, params, fields);
criterion_main!(galaxy);
