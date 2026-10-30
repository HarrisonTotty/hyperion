//! Benchmarks of the range query with plan 09's feature members, as the server serves it once
//! P09.T40.a's gate opens (`FeatureMembers::Served`): the grid over the shared cell cache and the
//! features' members through the shared feature-cell and interior caches, with the job's
//! [`KeptInteriors`] view, as `systems_in_range`'s job runs it. Targets are plan 09's
//! (Verification); a miss is a finding, not a CI failure: CI compiles these and never runs them.
//!
//! - **Sun-like, 50 ly**: the brainstorm's reference query at the Sun-like point of the server's
//!   seed, under 5 ms cold "with every source registered".
//! - **Globular core, 1 ly**: the server's test query (`tests/feature_members.rs`), 1 ly about the
//!   centre of the globular with members nearest the Sun-like point; and the plan's own, 50 ly
//!   about it, under 20 ms cold.
//!
//! Each is timed cold, over caches built fresh for the iteration, so that every cell and interior
//! it reads is generated, and warm, over caches the group filled before measuring.

use std::hint::black_box;
use std::num::NonZeroU32;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperion_server::compute::{
    GalaxyKey, SharedCellCache, SharedClusterModelCache, SharedFeatureCellCache,
};
use hyperion_server::config::{
    DEFAULT_CELL_CACHE_MIB, DEFAULT_CLUSTER_MODEL_CACHE_MIB, DEFAULT_FEATURE_CACHE_MIB,
};
use hyperion_server::limits::{MAX_CENSUS_LIMIT, MAX_QUERY_CELLS};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord};
use hyperion_sim::galaxy::features::members::FeatureInterior;
use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
use hyperion_sim::galaxy::query::{RangeQuery, RangeResult, SystemSource, range_query};
use hyperion_sim::units::LightYears;
use hyperion_sim::{GENERATOR_VERSION, Seed};

/// The seed of the galaxy every query here runs over: the server tests' universe.
const SEED: u64 = 0x4d2;

/// Bytes in a mebibyte, the unit of the server's cache budgets.
const MIB: usize = 1 << 20;

/// The caches a server holds for its galaxies, at its default budgets.
struct Caches {
    cells: SharedCellCache,
    features: SharedFeatureCellCache,
    interiors: SharedClusterModelCache,
}

impl Caches {
    fn new() -> Self {
        Self {
            cells: SharedCellCache::new(DEFAULT_CELL_CACHE_MIB * MIB),
            features: SharedFeatureCellCache::new(DEFAULT_FEATURE_CACHE_MIB * MIB),
            interiors: SharedClusterModelCache::new(DEFAULT_CLUSTER_MODEL_CACHE_MIB * MIB),
        }
    }

    /// `query` as the served `systems_in_range` job runs it: one kept view of the interiors for
    /// the job, the features' members beside the grid.
    fn query(&self, galaxy: &Galaxy, key: GalaxyKey, query: &RangeQuery) -> RangeResult {
        let mut cells = self.cells.handle(key);
        let (features, interiors) = (self.features.handle(key), self.interiors.kept(key));
        let members = FeatureMemberSource::new(&features, &interiors);
        let sources: [&dyn SystemSource; 1] = [&members];
        range_query(galaxy, &mut cells, &sources, query)
    }
}

/// The query of `radius_ly` about `centre` at the epoch, as the server builds one from a request
/// for every layer: its census limit and cell budget.
fn query_at(centre: GalacticPosition, radius_ly: f64) -> RangeQuery {
    RangeQuery::builder(centre, LightYears::new(radius_ly))
        .limit(NonZeroU32::new(MAX_CENSUS_LIMIT).expect("the limit is above zero"))
        .cell_budget(MAX_QUERY_CELLS)
        .build()
        .expect("a sphere of positive radius at the epoch within the limits is a valid query")
}

/// The Sun-like point, 26,000 ly from the centre on +y.
fn sun_like() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("inside the root cube")
}

/// The globular with members nearest the Sun-like point, as the server's tests choose it.
fn a_globular(galaxy: &Galaxy) -> FeatureRecord {
    let sun = sun_like();
    let mut globulars: Vec<_> =
        FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular).collect();
    globulars.sort_by(|a, b| {
        let away = |f: &FeatureRecord| f.position().distance_to(&sun).value();
        away(a).total_cmp(&away(b))
    });
    *globulars
        .iter()
        .find_map(|feature| FeatureInterior::of(galaxy, feature))
        .expect("the galaxy has a globular with members")
        .feature()
}

/// Each query cold and warm.
fn served(c: &mut Criterion) {
    let galaxy = Galaxy::new(Seed::new(SEED)).with_full_potential();
    let key = GalaxyKey::new(SEED, GENERATOR_VERSION);
    let globular = *a_globular(&galaxy).position();
    let queries = [
        ("Sun-like 50 ly", query_at(sun_like(), 50.0)),
        ("globular core 1 ly", query_at(globular, 1.0)),
        ("globular core 50 ly", query_at(globular, 50.0)),
    ];
    let mut group = c.benchmark_group("served_range_query");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    for (name, query) in &queries {
        group.bench_function(format!("{name} (cold)"), |b| {
            b.iter_batched_ref(
                Caches::new,
                |caches| caches.query(&galaxy, key, black_box(query)),
                // Fresh caches per iteration: a warm entry would make this the warm measurement.
                BatchSize::PerIteration,
            );
        });
        let warm = Caches::new();
        drop(warm.query(&galaxy, key, query));
        group.bench_function(format!("{name} (warm)"), |b| {
            b.iter(|| warm.query(&galaxy, key, black_box(query)));
        });
    }
    group.finish();
}

criterion_group!(benches, served);
criterion_main!(benches);
