//! Benchmark of a cold background scan of the accreting white dwarfs across the whole galaxy (plan
//! 12, P12.T8's `awd_scan_cold`, built with P12.T8.b.2 once P12.T5.c walked the classes), on the CPU
//! pool's default workers, as the alert service walks one: the class's process built once and
//! shared, and the root cube's 256 x-slabs of 512 ly cells in 256 bulk jobs of one, each sweeping
//! its slabs' occupied cells and reading their hosts' scan marks. Nothing is cached between runs:
//! the white dwarfs' cells are read from their marks each time. CI compiles it and never runs it.
//!
//! The query is the sim's own example of the novae: the fast hosts' class, through a V limit of
//! twelfth magnitude at the Sun-like point [0, 26,000, 20] ly of seed 9's galaxy over the first
//! decade, 63 novae (re-measured at 27, P12.T13; 91 at 26, measured 2026-11-01). The brainstorm's
//! "seconds" is the target, and more than 10 s a finding for plan 11's scan marks (plan 12's
//! P12.T8). Measured at 27 (P12.T13; pod, 48 vCPU, loaded): 841 ms on 47 workers.

use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_server::ServerConfig;
use hyperion_server::compute::{CancelToken, CpuPool, JobError, Priority};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, ClassSet, Detection, RecurrentProcess,
    SensorHorizon, class_reach,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::{ClassId, cells_in_slab, class_slabs};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::query::QuerySphere;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, WattsPerSquareMetre};

/// The slabs of each job, the alert service's (`SLABS_PER_JOB`).
const SLABS_PER_JOB: usize = 1;

/// The class scanned.
const CLASS: ClassId = ClassId::ACCRETING_WHITE_DWARF;

/// The fast white dwarfs' query: V = 12, Vega's 3.631 × 10⁻⁸ W m⁻² µm⁻¹ through the V band's
/// 0.084 µm, 10^4.8 fainter, over the first decade.
fn query() -> AlertQuery {
    let limit = WattsPerSquareMetre::new(3.631e-8 * 0.084 * 1.584_893e-5);
    let limits = BandLimits::new(limit, limit).expect("limits in order");
    let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).expect("one band");
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).expect("in the cube");
    let end = UniverseTime::from_julian_years(10).expect("on the clock");
    let window = TimeWindow::new(UniverseTime::EPOCH, end).expect("a window forward");
    AlertQuery::builder(sun, window, sensor)
        .classes(ClassSet::of(CLASS))
        .build()
        .expect("a query of the clock window")
}

/// The whole cube's novae for `query`, walked on `pool` in jobs of [`SLABS_PER_JOB`] slabs of the
/// class's sphere: the process built once, cold, and lent to every job.
fn walk(
    runtime: &tokio::runtime::Runtime,
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    query: &AlertQuery,
) -> Vec<Detection> {
    let process = Arc::new(RecurrentProcess::of(galaxy, CLASS).expect("a recurrent class"));
    let sphere = QuerySphere::new(
        *query.observer_position(),
        class_reach(CLASS, query.sensor()),
        query.window().start(),
        LightYears::ZERO,
    )
    .expect("a reach within the cube");
    let slabs = class_slabs(CLASS, &sphere);
    runtime.block_on(async {
        let token = CancelToken::new();
        let step = i32::try_from(SLABS_PER_JOB).expect("one fits in i32");
        let mut jobs = Vec::new();
        for first in slabs.clone().step_by(SLABS_PER_JOB) {
            let range: Range<i32> = first..(first + step).min(slabs.end);
            let (galaxy, process, query) =
                (Arc::clone(galaxy), Arc::clone(&process), query.clone());
            jobs.push(
                pool.submit(Priority::Bulk, token.clone(), move |_: &CancelToken| {
                    let mut noise = NoiseCache::with_capacity(4_096);
                    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
                    let mut out = Vec::new();
                    for x in range {
                        let cells = cells_in_slab(CLASS, &sphere, x);
                        for (cell, _) in process.occupied_cells(&galaxy, cells) {
                            process.scan_cell(&galaxy, &mut ctx, cell, &query, &mut out);
                        }
                    }
                    out
                })
                .await
                .expect("the pool takes the job"),
            );
        }
        let mut detections = Vec::new();
        for job in jobs {
            detections.extend(
                job.await
                    .unwrap_or_else(|closed| Err(JobError::from(closed)))
                    .expect("the job runs"),
            );
        }
        detections
    })
}

/// The whole cube's fast white dwarfs, cold, on the default workers.
fn awd_scan(c: &mut Criterion) {
    let galaxy = Arc::new(Galaxy::new(Seed::new(9)));
    let workers = ServerConfig::builder().build().workers();
    let pool = CpuPool::new(workers, INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY)
        .expect("the pool starts");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let query = query();
    assert_eq!(
        walk(&runtime, &pool, &galaxy, &query).len(),
        63,
        "the decade's novae"
    );
    let mut group = c.benchmark_group("awd_scan_cold");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.bench_function(
        format!(
            "fast white dwarfs, V 12 mag, 10 yr, whole cube, {} workers",
            workers.get()
        ),
        |b| {
            b.iter(|| walk(&runtime, &pool, &galaxy, &query));
        },
    );
    group.finish();
    runtime
        .block_on(pool.shutdown())
        .expect("the pool shuts down");
}

criterion_group!(benches, awd_scan);
criterion_main!(benches);
