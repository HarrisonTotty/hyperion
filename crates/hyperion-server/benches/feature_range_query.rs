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
//! with the members and the classes, the classes' cells read through the server's class cell cache
//! (P09.T40); and at 50 ly each class's source alone, its census and its hits through no cache, for
//! each class's share.
//!
//! `served_at_27` is the Sun-like query of 50 ly under 27's rules as `systems_in_range` serves it
//! (`served_range_query`), with the members and the classes, on pools of 1, 4 and 8 workers, cold
//! and warm: a cold query's class cells made across the workers ahead of its hits (P09.T37's
//! bench). It is served so with the global list too, its tables built ahead as the server builds
//! them, and so is the query of 50 ly at the black hole; and the Sun-like query is served as the
//! first query of a galaxy built fresh for it as the server's galaxy cache builds one, its query
//! tables made with it (`Galaxy::make_query_tables`), so that it pays for any table still made
//! lazily.
//!
//! `list_at_27` is the Sun-like query of 50 ly and of 1 ly, and the query of 50 ly at the black
//! hole, where nothing fits, at the epoch under 27's rules with every source, the global list's
//! among them (P10.T10.b's registration and P09.T37.b), cold and warm: the list's tube tables
//! built before, as the server builds them ahead of a query, its models kept with it, as the
//! server keeps them with the list (`GlobalListModels`), and the centre's orbits read through the
//! server's centre orbit cache; and at 50 ly the list's source alone through no orbit cache.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperion_server::FeatureMembers;
use hyperion_server::compute::{
    CancelToken, CpuPool, GalaxyKey, HeldGlobalList, ListedTubes, Priority, RangeCaches,
    ServedQuery, SharedCellCache, SharedCentreOrbitCache, SharedClassCellCache,
    SharedClusterModelCache, SharedFeatureCellCache, TubeTableCache, served_range_query,
};
use hyperion_server::config::{
    DEFAULT_CELL_CACHE_MIB, DEFAULT_CLASS_CELL_CACHE_MIB, DEFAULT_CLUSTER_MODEL_CACHE_MIB,
    DEFAULT_FEATURE_CACHE_MIB,
};
use hyperion_server::limits::{
    BULK_QUEUE_CAPACITY, CENTRE_ORBIT_CACHE_BYTES, INTERACTIVE_QUEUE_CAPACITY, MAX_CENSUS_LIMIT,
    MAX_QUERY_CELLS, TUBE_TABLE_CACHE_BYTES,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::{
    ALL_CLASSES, CatalogueClassSource, ClassId, ClassLookup, ClassVisitor, ProductionClasses,
};
use hyperion_sim::galaxy::features::FeatureProcess;
use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord};
use hyperion_sim::galaxy::features::centre::NoCentreOrbitCache;
use hyperion_sim::galaxy::features::members::FeatureInterior;
use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
use hyperion_sim::galaxy::global_list::{
    GlobalList, GlobalListModels, GlobalListSource, StreamNumber, TubeLookup, TubeTable,
};
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
    centre: Arc<SharedCentreOrbitCache>,
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
            centre: Arc::new(SharedCentreOrbitCache::new(CENTRE_ORBIT_CACHE_BYTES)),
        }
    }

    /// `query` with every source a served query takes from generator version 27: the members, the
    /// global list's through `list`, the centre's orbits read through the centre orbit cache, and
    /// the catalogue classes'.
    fn query_listed(
        &self,
        galaxy: &Galaxy,
        key: GalaxyKey,
        query: &RangeQuery,
        list: &dyn TubeLookup,
    ) -> RangeResult {
        let mut cells = self.cells.handle(key);
        let (features, interiors) = (self.features.handle(key), self.interiors.kept(key));
        let members = FeatureMemberSource::new(&features, &interiors);
        let class_cells = self.classes.handle(key);
        let classes = CatalogueClassSource::new().with_cells(&class_cells);
        let orbits = self.centre.handle(key);
        let listed = GlobalListSource::new(galaxy, list, &orbits).expect("the centre builds");
        range_query(galaxy, &mut cells, &[&members, &listed, &classes], query)
    }

    /// `query` as the served `systems_in_range` job runs it: one kept view of the interiors for
    /// the job, the features' members beside the grid, and from generator version 27 the
    /// catalogue classes' hosts beside them.
    fn query(&self, galaxy: &Galaxy, key: GalaxyKey, query: &RangeQuery) -> RangeResult {
        self.query_with(galaxy, key, query, Classes::Registered)
    }

    /// `query` as [`query`](Self::query) runs it, with the catalogue classes' hosts as `classes`
    /// says.
    fn query_with(
        &self,
        galaxy: &Galaxy,
        key: GalaxyKey,
        query: &RangeQuery,
        classes: Classes,
    ) -> RangeResult {
        let mut cells = self.cells.handle(key);
        let (features, interiors) = (self.features.handle(key), self.interiors.kept(key));
        let members = FeatureMemberSource::new(&features, &interiors);
        let class_cells = self.classes.handle(key);
        let classes = CatalogueClassSource::registered_in(galaxy)
            .filter(|_| matches!(classes, Classes::Registered))
            .map(|classes| classes.with_cells(&class_cells));
        let mut sources: Vec<&dyn SystemSource> = vec![&members];
        sources.extend(classes.as_ref().map(|classes| classes as &dyn SystemSource));
        range_query(galaxy, &mut cells, &sources, query)
    }
}

/// Whether a bench's query takes the catalogue classes' hosts.
#[derive(Debug, Clone, Copy)]
enum Classes {
    /// Where the galaxy's rules register them, as a server does.
    Registered,
    /// Never: the grid and the members alone, for the classes' share.
    Left,
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
        (
            Arc::new(SharedClassCellCache::new(
                DEFAULT_CLASS_CELL_CACHE_MIB * MIB,
            )),
            Arc::new(SharedCentreOrbitCache::new(CENTRE_ORBIT_CACHE_BYTES)),
        ),
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

/// The galaxy of the server's seed under generator version 27's rules, with its full potential.
fn galaxy_at_27() -> Galaxy {
    let seed = Seed::new(SEED);
    let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
    Galaxy::from_params_at_version(seed, params, GeneratorVersion::new(27))
        .expect("a drawn galaxy builds")
        .with_full_potential()
}

/// `galaxy`'s global list as the server serves it to a query: the list with its models
/// ([`HeldGlobalList`]), and the tables of the streams `queries` read built ahead, as the server
/// builds them before a query.
fn listed_tubes(galaxy: &Arc<Galaxy>, key: GalaxyKey, queries: &[&RangeQuery]) -> ListedTubes {
    let list = Arc::new(HeldGlobalList::new(galaxy, GlobalList::build(galaxy)));
    let listed = ListedTubes::new(
        (Arc::clone(galaxy), key),
        Arc::clone(&list),
        Arc::new(TubeTableCache::new(TUBE_TABLE_CACHE_BYTES)),
    );
    for query in queries {
        for number in GlobalListSource::streams_read(galaxy, list.list(), query) {
            drop(listed.tube(number));
        }
    }
    listed
}

/// The Sun-like 50 ly query under generator version 27's rules served on pools of 1, 4 and 8
/// workers, cold and warm, without the global list and with it, and with it the black hole's 50 ly
/// query too; and the Sun-like query as a galaxy's first, over a galaxy built fresh for it as the
/// server builds one (module documentation).
fn served_at_27(c: &mut Criterion) {
    let version = GeneratorVersion::new(27);
    let galaxy = Arc::new(galaxy_at_27());
    let key = GalaxyKey::new(SEED, version);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let (sun, black_hole) = (
        query_at(sun_like(), 50.0),
        query_at(GalacticPosition::ORIGIN, 50.0),
    );
    let listed = listed_tubes(&galaxy, key, &[&sun, &black_hole]);
    let unlisted = ServedQuery::new(
        Arc::clone(&galaxy),
        key,
        sun.clone(),
        FeatureMembers::Served,
    );
    let queries = [
        ("Sun-like 50 ly", unlisted.clone()),
        (
            "Sun-like 50 ly with the list",
            unlisted.clone().with_list(listed.clone()),
        ),
        (
            "the black hole 50 ly with the list",
            ServedQuery::new(Arc::clone(&galaxy), key, black_hole, FeatureMembers::Served)
                .with_list(listed),
        ),
    ];
    let mut group = c.benchmark_group("served_at_27");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    for workers in [1, 4, 8] {
        let pool = CpuPool::new(
            NonZeroUsize::new(workers).expect("at least one worker"),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .expect("the pool starts");
        let token = CancelToken::new();
        let serve = |caches: &RangeCaches, served: &ServedQuery| {
            runtime
                .block_on(served_range_query(
                    &pool,
                    caches,
                    served,
                    &token,
                    |_, result, _| result,
                ))
                .expect("the query's jobs run")
        };
        for (name, served) in &queries {
            group.bench_function(format!("{name}, {workers} workers (cold)"), |b| {
                b.iter_batched_ref(
                    range_caches,
                    |caches| serve(caches, served),
                    BatchSize::PerIteration,
                );
            });
            let warm = range_caches();
            drop(serve(&warm, served));
            group.bench_function(format!("{name}, {workers} workers (warm)"), |b| {
                b.iter(|| serve(&warm, served));
            });
        }
        group.bench_function(
            format!("Sun-like 50 ly, {workers} workers (a galaxy's first)"),
            |b| {
                b.iter_batched(
                    || {
                        // As the server's galaxy cache builds it (`Galaxy::make_query_tables`).
                        let galaxy = galaxy_at_27();
                        galaxy.make_query_tables();
                        let galaxy = Arc::new(galaxy);
                        let served =
                            ServedQuery::new(galaxy, key, sun.clone(), FeatureMembers::Served);
                        (range_caches(), served)
                    },
                    |(caches, served)| serve(&caches, &served),
                    BatchSize::PerIteration,
                );
            },
        );
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
        group.bench_function(
            format!("Sun-like {radius} ly, without the classes (cold)"),
            |b| {
                b.iter_batched_ref(
                    Caches::new,
                    |caches| caches.query_with(&galaxy, key, black_box(&query), Classes::Left),
                    BatchSize::PerIteration,
                );
            },
        );
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

/// A lookup that keeps every tube table it builds, as the server's warm-up keeps a galaxy's, and
/// the list's models, as the server keeps them with the list.
struct KeepTubes<'a> {
    galaxy: &'a Galaxy,
    list: GlobalList,
    tubes: Mutex<BTreeMap<StreamNumber, Arc<TubeTable>>>,
    models: GlobalListModels,
}

impl TubeLookup for KeepTubes<'_> {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn models(&self) -> Option<&GlobalListModels> {
        Some(&self.models)
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        let spec = self.list.stream(stream).expect("a stream of the list");
        let mut tubes = self.tubes.lock().expect("not poisoned");
        Arc::clone(tubes.entry(stream).or_insert_with(|| {
            Arc::new(TubeTable::build(self.galaxy, spec).expect("the full potential"))
        }))
    }
}

/// The queries under generator version 27's rules with every source, the global list's among
/// them (module documentation).
fn list_at_27(c: &mut Criterion) {
    let seed = Seed::new(SEED);
    let version = GeneratorVersion::new(27);
    let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
    let galaxy = Galaxy::from_params_at_version(seed, params, version)
        .expect("a drawn galaxy builds")
        .with_full_potential();
    let key = GalaxyKey::new(SEED, version);
    let list = GlobalList::build(&galaxy);
    let lookup = KeepTubes {
        galaxy: &galaxy,
        models: GlobalListModels::new(&galaxy, &list),
        list,
        tubes: Mutex::default(),
    };
    // Not the black hole's 0.05 ly, whose complete census walks the grid's cells about the origin,
    // some 3 × 10⁶ candidates in layers C to E: some 600 s cold on one thread a century on
    // (measured 2026-11-01, pod, release), too long to sample. At 50 ly nothing fits, and the
    // query is its census.
    let queries = [
        ("Sun-like 50 ly", query_at(sun_like(), 50.0)),
        ("Sun-like 1 ly", query_at(sun_like(), 1.0)),
        (
            "the black hole 50 ly",
            query_at(GalacticPosition::ORIGIN, 50.0),
        ),
    ];
    for (name, query) in &queries {
        let streams = GlobalListSource::streams_read(&galaxy, &lookup.list, query);
        for &number in &streams {
            drop(lookup.tube(number));
        }
        println!("list_at_27: {name} reads {} streams' tables", streams.len());
    }
    let mut group = c.benchmark_group("list_at_27");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    for (name, query) in &queries {
        group.bench_function(format!("{name}, every source (cold)"), |b| {
            b.iter_batched_ref(
                Caches::new,
                |caches| caches.query_listed(&galaxy, key, black_box(query), &lookup),
                BatchSize::PerIteration,
            );
        });
        let warm = Caches::new();
        let result = warm.query_listed(&galaxy, key, query, &lookup);
        println!(
            "list_at_27: {name}: {} systems, complete down to {:?}",
            result.systems().len(),
            result.census().complete_down_to()
        );
        group.bench_function(format!("{name}, every source (warm)"), |b| {
            b.iter(|| warm.query_listed(&galaxy, key, black_box(query), &lookup));
        });
    }
    let stellar: LayerSet = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .collect();
    let sphere = QuerySphere::new(
        sun_like(),
        LightYears::new(50.0),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .expect("a sphere");
    let listed =
        GlobalListSource::new(&galaxy, &lookup, &NoCentreOrbitCache).expect("the centre builds");
    group.bench_function("Sun-like 50 ly, the list alone", |b| {
        b.iter(|| {
            let counts = listed.expected_in_sphere(&galaxy, black_box(&sphere));
            let mut hits = Vec::new();
            listed.systems_in_sphere(&galaxy, &sphere, stellar, &mut hits);
            (counts, hits)
        });
    });
    group.bench_function("the list's source, made", |b| {
        b.iter(|| {
            GlobalListSource::new(&galaxy, black_box(&lookup), &NoCentreOrbitCache)
                .expect("the centre builds")
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    served,
    served_on_the_pool,
    classes_at_27,
    served_at_27,
    list_at_27
);
criterion_main!(benches);
