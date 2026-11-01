//! Benchmarks of a background alert scan that reaches the whole galaxy (plan 12, P12.T8.b.2), on
//! the CPU pool's default workers, as the alert service walks one: the class's process built once
//! and shared, and the root cube's 256 x-slabs of 512 ly cells in 64 bulk jobs of four, each
//! sweeping its slabs' occupied cells and building only the candidates whose event time, read
//! first, can light the window. CI compiles these and never runs them.
//!
//! The fixture is the Milky Way-like galaxy of seed `0x0926_0000` and the Sun-like point
//! [0, 26,000, 20] ly, research's (research-alerts-galaxy-scan, 2026-11-01: 0.5 s and 0.55 s on the
//! pod's 16 vCPU). The brainstorm's "seconds" (10 s) is the target.
//!
//! - The core collapses' supernovae through a V limit of 10⁻¹² W m⁻² over the first 1,000 years.
//! - Their magnetars through an X-ray limit of 10⁻¹⁴ W m⁻² over the first century.

use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_server::ServerConfig;
use hyperion_server::compute::{CancelToken, CpuPool, JobError, Priority};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, ClassSet, Detection, DetectionPhase,
    SensorHorizon, TransientKind, kind_reach, magnetar_scan_cell, one_shot_scan_cell,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    ClassId, cells_in_slab, class_slabs, occupied_cells,
};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::query::QuerySphere;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, WattsPerSquareMetre};

/// The slabs of each job, the alert service's (`SLABS_PER_JOB`).
const SLABS_PER_JOB: usize = 4;

/// The query of the core collapses from the Sun-like point through one band's detection limit,
/// W m⁻², over `[0, years)`.
fn query(band: AlertBand, detect: f64, years: i64) -> AlertQuery {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(detect * 100.0)).expect("limits in order");
    let sensor = SensorHorizon::new([(band, limits)]).expect("one band");
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).expect("in the cube");
    let end = UniverseTime::from_julian_years(years).expect("on the clock");
    let window = TimeWindow::new(UniverseTime::EPOCH, end).expect("a window forward");
    AlertQuery::builder(sun, window, sensor)
        .classes(ClassSet::of(ClassId::CORE_COLLAPSE))
        .build()
        .expect("a query of the clock window")
}

/// What a walk of the core collapses lists.
#[derive(Debug, Clone, Copy)]
enum Walk {
    /// Their supernovae.
    Supernovae,
    /// Their magnetars.
    Magnetars,
}

impl Walk {
    /// The kind the walk lists, whose reach its sphere is.
    const fn kind(self) -> TransientKind {
        match self {
            Self::Supernovae => TransientKind::CoreCollapse,
            Self::Magnetars => TransientKind::MagnetarBurst,
        }
    }
}

/// The whole cube's detections of `what` for `query`, walked on `pool` in jobs of
/// [`SLABS_PER_JOB`] slabs of its kind's sphere: the process built once and lent to every job.
fn walk(
    runtime: &tokio::runtime::Runtime,
    pool: &CpuPool,
    galaxy: &Arc<Galaxy>,
    query: &AlertQuery,
    what: Walk,
) -> Vec<Detection> {
    let process = Arc::new(CoreCollapseProcess::new(galaxy));
    let sphere = QuerySphere::new(
        *query.observer_position(),
        kind_reach(what.kind(), query.sensor()),
        query.window().start(),
        LightYears::ZERO,
    )
    .expect("a reach within the cube");
    let slabs = class_slabs(ClassId::CORE_COLLAPSE, &sphere);
    runtime.block_on(async {
        let token = CancelToken::new();
        let mut jobs = Vec::new();
        let step = i32::try_from(SLABS_PER_JOB).expect("four fits in i32");
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
                        let cells = cells_in_slab(ClassId::CORE_COLLAPSE, &sphere, x);
                        for (cell, _) in occupied_cells(&galaxy, process.as_ref(), cells) {
                            match what {
                                Walk::Magnetars => magnetar_scan_cell(
                                    &galaxy,
                                    &mut ctx,
                                    &query,
                                    process.as_ref(),
                                    cell,
                                    &mut out,
                                ),
                                Walk::Supernovae => one_shot_scan_cell(
                                    &galaxy,
                                    &mut ctx,
                                    &query,
                                    process.as_ref(),
                                    cell,
                                    &mut out,
                                ),
                            }
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

/// The whole cube's supernovae and magnetars on the default workers.
fn galaxy_scan(c: &mut Criterion) {
    let galaxy = Arc::new(
        Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
            .expect("the fixture builds"),
    );
    let workers = ServerConfig::builder().build().workers();
    let pool = CpuPool::new(workers, INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY)
        .expect("the pool starts");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let supernovae = query(AlertBand::Photometric(Band::V), 1e-12, 1_000);
    let magnetars = query(AlertBand::XRay, 1e-14, 100);
    // Research's counts: 4 onsets, and 27,059 magnetar detections from 71 hosts.
    let seen = walk(&runtime, &pool, &galaxy, &supernovae, Walk::Supernovae);
    let onsets = seen
        .iter()
        .filter(|detection| detection.phase() == DetectionPhase::Onset)
        .count();
    assert_eq!(onsets, 4, "{seen:?}");
    let bursts = walk(&runtime, &pool, &galaxy, &magnetars, Walk::Magnetars);
    assert_eq!(bursts.len(), 27_059);
    let mut group = c.benchmark_group("oneshot_scan_galaxy");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.bench_function(
        format!(
            "core collapses, V 1e-12 W m^-2, 1,000 yr, {} workers",
            workers.get()
        ),
        |b| {
            b.iter(|| walk(&runtime, &pool, &galaxy, &supernovae, Walk::Supernovae));
        },
    );
    group.bench_function(
        format!(
            "magnetars, X-ray 1e-14 W m^-2, 100 yr, {} workers",
            workers.get()
        ),
        |b| {
            b.iter(|| walk(&runtime, &pool, &galaxy, &magnetars, Walk::Magnetars));
        },
    );
    group.finish();
    runtime
        .block_on(pool.shutdown())
        .expect("the pool shuts down");
}

criterion_group!(benches, galaxy_scan);
criterion_main!(benches);
