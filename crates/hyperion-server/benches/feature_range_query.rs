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
//! it reads is generated, and warm, over caches the group filled before measuring: in one thread;
//! as one pool job on a pool of 1, 4 and 8 workers, as the served job ran it before P09.T40.a's
//! parallel cells; and served so (`served_range_query`), its cold cells generated across the
//! workers ahead of its walk.
//!
//! `classes_at_27` is the Sun-like query of 50 ly and of 1 ly under generator version 27's rules,
//! where the catalogue classes' hosts are registered beside the members (P09.T37.a), cold and warm
//! with every source, the classes' cells read through the server's class cell cache (P09.T40); and
//! at 50 ly each class's source alone, its census and its hits through no cache, for each class's
//! share.

use std::hint::black_box;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperion_server::FeatureMembers;
use hyperion_server::compute::{
    CancelToken, CpuPool, GalaxyKey, Priority, RangeCaches, ServedQuery, SharedCellCache,
    SharedClassCellCache, SharedClusterModelCache, SharedFeatureCellCache, served_range_query,
};
use hyperion_server::config::{
    DEFAULT_CELL_CACHE_MIB, DEFAULT_CLASS_CELL_CACHE_MIB, DEFAULT_CLUSTER_MODEL_CACHE_MIB,
    DEFAULT_FEATURE_CACHE_MIB,
};
use hyperion_server::limits::{
    BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY, MAX_CENSUS_LIMIT, MAX_QUERY_CELLS,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::{
    ALL_CLASSES, CatalogueClassSource, ClassId, ClassLookup, ClassVisitor, ProductionClasses,
};
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord};
use hyperion_sim::galaxy::features::members::FeatureInterior;
use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::query::{
    LayerSet, QuerySphere, RangeQuery, RangeResult, SystemSource, range_query,
};
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion, Seed};

/// The seed of the galaxy every query here runs over: the server tests' universe.
const SEED: u64 = 0x4d2;

/// Bytes in a mebibyte, the unit of the server's cache budgets.
const MIB: usize = 1 << 20;

/// The caches a server holds for its galaxies, at its default budgets, shared with pool jobs.
#[derive(Clone)]
struct Caches {
    cells: Arc<SharedCellCache>,
    features: Arc<SharedFeatureCellCache>,
    interiors: Arc<SharedClusterModelCache>,
    classes: Arc<SharedClassCellCache>,
}

impl Caches {
    fn new() -> Self {
        Self {
            cells: Arc::new(SharedCellCache::new(DEFAULT_CELL_CACHE_MIB * MIB)),
            features: Arc::new(SharedFeatureCellCache::new(DEFAULT_FEATURE_CACHE_MIB * MIB)),
            interiors: Arc::new(SharedClusterModelCache::new(
                DEFAULT_CLUSTER_MODEL_CACHE_MIB * MIB,
            )),
            classes: Arc::new(SharedClassCellCache::new(
                DEFAULT_CLASS_CELL_CACHE_MIB * MIB,
            )),
        }
    }

    /// `query` as the served `systems_in_range` job runs it: one kept view of the interiors for
    /// the job, the features' members beside the grid, and from generator version 27 the
    /// catalogue classes' hosts beside them.
    fn query(&self, galaxy: &Galaxy, key: GalaxyKey, query: &RangeQuery) -> RangeResult {
        let mut cells = self.cells.handle(key);
        let (features, interiors) = (self.features.handle(key), self.interiors.kept(key));
        let members = FeatureMemberSource::new(&features, &interiors);
        let class_cells = self.classes.handle(key);
        let classes = CatalogueClassSource::registered_in(galaxy)
            .map(|classes| classes.with_cells(&class_cells));
        let mut sources: Vec<&dyn SystemSource> = vec![&members];
        sources.extend(classes.as_ref().map(|classes| classes as &dyn SystemSource));
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

/// Fresh caches at the server's default budgets, as the served path shares them with its jobs.
fn range_caches() -> RangeCaches {
    RangeCaches::new(
        Arc::new(SharedCellCache::new(DEFAULT_CELL_CACHE_MIB * MIB)),
        Arc::new(SharedFeatureCellCache::new(DEFAULT_FEATURE_CACHE_MIB * MIB)),
        Arc::new(SharedClusterModelCache::new(
            DEFAULT_CLUSTER_MODEL_CACHE_MIB * MIB,
        )),
        Arc::new(SharedClassCellCache::new(
            DEFAULT_CLASS_CELL_CACHE_MIB * MIB,
        )),
    )
}

/// The Sun-like and the globular core's 1 ly queries served on pools of 1, 4 and 8 workers, cold
/// and warm, as `systems_in_range` serves them: the result alone, without the rows' conversion.
fn served_on_the_pool(c: &mut Criterion) {
    let galaxy = Arc::new(Galaxy::new(Seed::new(SEED)).with_full_potential());
    let key = GalaxyKey::new(SEED, GENERATOR_VERSION);
    let globular = *a_globular(&galaxy).position();
    let queries = [
        ("Sun-like 50 ly", query_at(sun_like(), 50.0)),
        ("globular core 1 ly", query_at(globular, 1.0)),
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let mut group = c.benchmark_group("served_on_the_pool");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(20));
    for workers in [1, 4, 8] {
        let pool = CpuPool::new(
            NonZeroUsize::new(workers).expect("at least one worker"),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .expect("the pool starts");
        let token = CancelToken::new();
        for (name, query) in &queries {
            let served = ServedQuery::new(
                Arc::clone(&galaxy),
                key,
                query.clone(),
                FeatureMembers::Served,
            );
            let serve = |caches: &RangeCaches| {
                runtime
                    .block_on(served_range_query(
                        &pool,
                        caches,
                        &served,
                        &token,
                        |_, result, _| result,
                    ))
                    .expect("the query's jobs run")
            };
            let one_job = |caches: &Caches| {
                let (caches, query) = (caches.clone(), query.clone());
                let galaxy = Arc::clone(&galaxy);
                let receiver = pool
                    .try_submit(
                        Priority::Interactive,
                        token.clone(),
                        move |_: &CancelToken| caches.query(&galaxy, key, &query),
                    )
                    .expect("room for the job");
                runtime
                    .block_on(receiver)
                    .expect("the job replies")
                    .expect("the job runs")
            };
            group.bench_function(format!("{name}, {workers} workers, one job (cold)"), |b| {
                b.iter_batched_ref(
                    Caches::new,
                    |caches| one_job(caches),
                    BatchSize::PerIteration,
                );
            });
            let warm = Caches::new();
            drop(one_job(&warm));
            group.bench_function(format!("{name}, {workers} workers, one job (warm)"), |b| {
                b.iter(|| one_job(&warm));
            });
            group.bench_function(format!("{name}, {workers} workers (cold)"), |b| {
                b.iter_batched_ref(
                    range_caches,
                    |caches| serve(caches),
                    BatchSize::PerIteration,
                );
            });
            let warm = range_caches();
            drop(serve(&warm));
            group.bench_function(format!("{name}, {workers} workers (warm)"), |b| {
                b.iter(|| serve(&warm));
            });
        }
        runtime
            .block_on(pool.shutdown())
            .expect("the pool shuts down");
    }
    group.finish();
}

/// One class of the production lookup, for its share of the classes' source.
#[derive(Debug, Clone, Copy)]
struct Only(ClassId);

impl ClassLookup for Only {
    fn visit_class<V: ClassVisitor>(
        &self,
        galaxy: &Galaxy,
        class: ClassId,
        visitor: V,
    ) -> Option<V::Output> {
        if class == self.0 {
            ProductionClasses.visit_class(galaxy, class, visitor)
        } else {
            None
        }
    }
}

/// The Sun-like 50 ly query cold under generator version 27's rules, with every source and each
/// class's source alone (module documentation).
fn classes_at_27(c: &mut Criterion) {
    let seed = Seed::new(SEED);
    let version = GeneratorVersion::new(27);
    let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
    let galaxy = Galaxy::from_params_at_version(seed, params, version)
        .expect("a drawn galaxy builds")
        .with_full_potential();
    let key = GalaxyKey::new(SEED, version);
    let stellar: LayerSet = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .collect();
    let mut group = c.benchmark_group("classes_at_27");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    for radius in [50.0, 1.0] {
        let query = query_at(sun_like(), radius);
        group.bench_function(format!("Sun-like {radius} ly, every source (cold)"), |b| {
            b.iter_batched_ref(
                Caches::new,
                |caches| caches.query(&galaxy, key, black_box(&query)),
                BatchSize::PerIteration,
            );
        });
        let warm = Caches::new();
        drop(warm.query(&galaxy, key, &query));
        group.bench_function(format!("Sun-like {radius} ly, every source (warm)"), |b| {
            b.iter(|| warm.query(&galaxy, key, black_box(&query)));
        });
        if radius > 1.0 {
            let held = warm.classes.counters();
            println!(
                "classes_at_27: the class cells of the Sun-like {radius} ly query: {} cells, {} bytes",
                held.entries(),
                held.bytes()
            );
        }
    }
    let sphere = QuerySphere::new(
        sun_like(),
        LightYears::new(50.0),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a sphere");
    for class in ALL_CLASSES {
        let source = CatalogueClassSource::with_lookup(Only(class));
        group.bench_function(format!("Sun-like 50 ly, {class} alone (cold)"), |b| {
            b.iter(|| {
                let counts = source.expected_in_sphere(&galaxy, black_box(&sphere));
                let mut hits = Vec::new();
                source.systems_in_sphere(&galaxy, &sphere, stellar, &mut hits);
                (counts, hits)
            });
        });
    }
    group.finish();
}

criterion_group!(benches, served, served_on_the_pool, classes_at_27);
criterion_main!(benches);
