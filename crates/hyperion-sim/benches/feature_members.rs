//! Benchmarks of plan 09's feature members in the range query (P09.T23). Targets are the plan's; a
//! miss is a finding, not a CI failure: CI compiles these and never runs them.
//!
//! - A 50 ly query in a globular's core, warm (its interior cached, as P09.T40's model cache will
//!   hold it): under 20 ms (ruling 139.6). The globular is the fixture's first that has not
//!   collapsed and expects 10⁴–6 × 10⁴ members.
//! - The same query cold, with an empty [`KeepInteriors`] for the query, so its census and hits
//!   build the interior once: measured, not gated.
//! - `FeatureInterior::of` for that globular: the cluster model, the class table and the grid's
//!   width, which a server caches.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
use hyperion_sim::galaxy::features::members::{
    FeatureInterior, FeatureMemberSource, KeepInteriors,
};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, SystemSource};
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;

fn feature_members(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(0x0923_0000), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    let interior = FeatureCatalogue::walk_process(&galaxy, FeatureProcess::Globular)
        .filter_map(|f| FeatureInterior::of(&galaxy, &f))
        .find(|i| {
            let n: f64 = MassBand::ALL.iter().map(|&b| i.table().expected(b)).sum();
            !i.model().is_core_collapsed() && (10_000.0..60_000.0).contains(&n)
        })
        .expect("the fixture has such a globular");
    let feature = *interior.feature();
    let centre: GalacticPosition = *feature.position();
    let sphere = QuerySphere::new(
        centre,
        LightYears::new(50.0),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a 50 ly sphere");
    let layers: LayerSet = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .collect();
    let mut group = c.benchmark_group("feature_members");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    let query = |interiors: &KeepInteriors| {
        let source = FeatureMemberSource::new(&NoFeatureCache, interiors);
        let mut hits = Vec::new();
        let expected = source.expected_in_sphere(&galaxy, black_box(&sphere));
        source.systems_in_sphere(&galaxy, black_box(&sphere), layers, &mut hits);
        (expected, hits.len())
    };
    group.bench_function("globular_core_50ly_cold", |b| {
        b.iter(|| query(&KeepInteriors::new(&galaxy)));
    });
    let warm = KeepInteriors::new(&galaxy);
    let _ = query(&warm);
    group.bench_function("globular_core_50ly_warm", |b| {
        b.iter(|| query(&warm));
    });
    group.bench_function("FeatureInterior::of", |b| {
        b.iter(|| FeatureInterior::of(&galaxy, black_box(&feature)));
    });
    group.finish();
}

criterion_group!(benches, feature_members);
criterion_main!(benches);
