//! Benchmarks of the galaxy model (plan 02). Targets are recorded in the plan; a miss is a finding
//! to raise, not a CI failure: CI compiles these and never runs them.
//!
//! - `PotentialTables::in_plane`: 50 ms; `PotentialTables::full`: 2 s (P02.T6).
//! - `GalaxyParams::from_seed`, which includes the black hole's σ estimator (P02.T6.e).
//! - `Fields::densities` at a disc point: 400 ns, on which plan 03's 1–2 µs per sparse cell rests
//!   (P02.T7); `Fields::new`, most of it the sub-discs' Jeans solve.
//! - `Fields::layer_bound` over a cell, once per cell in placement (P02.T8): 1 µs, the part of
//!   plan 03's 1–2 µs per sparse cell that `Fields::densities` leaves.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::bounds::CellBox;
use hyperion_sim::galaxy::fields::{Fields, MAX_COMPONENTS};
use hyperion_sim::galaxy::imf::{BandShares, MassBand, MassFunctionKind};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::galaxy::shares::ShareMatrix;
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
        b.iter(|| GalaxyParams::from_seed(black_box(Seed::new(7)), MassFunctionKind::default()));
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

/// The bounds of the Milky Way fixture over a cell: every component's, and a layer's, which
/// placement computes once per cell (plan 03).
fn bounds(c: &mut Criterion) {
    let params = GalaxyParams::milky_way_like();
    let fields = Fields::new(&params, &MassModel::new(&params));
    let shares = ShareMatrix::uniform(&BandShares::of(
        MassFunctionKind::default().to_mass_function().as_ref(),
    ));
    let mut group = c.benchmark_group("bounds");
    let mut out = [0.0; MAX_COMPONENTS];
    // A layer-A cell at the solar circle, 30° from the bar; a layer-E cell there; a layer-A cell
    // in the bulge.
    let cells = [
        (
            "layer A, disc",
            CellBox::new([22_512, 12_992, 48], 8),
            MassBand::A,
        ),
        (
            "layer E, disc",
            CellBox::new([22_400, 12_928, 0], 128),
            MassBand::E,
        ),
        (
            "layer A, bulge",
            CellBox::new([896, 400, 144], 8),
            MassBand::A,
        ),
    ];
    for (label, cell, band) in cells {
        let cell = cell.expect("a grid cell");
        group.bench_function(format!("Fields::layer_bound ({label})"), |b| {
            b.iter(|| fields.layer_bound(&shares, band, black_box(&cell)));
        });
        group.bench_function(format!("Fields::component_bounds ({label})"), |b| {
            b.iter(|| {
                fields.component_bounds(black_box(&cell), &mut out);
                black_box(&out);
            });
        });
    }
    group.finish();
}

criterion_group!(galaxy, potential, params, fields, bounds);
criterion_main!(galaxy);
