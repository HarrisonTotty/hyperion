//! The cost of P10.T10.b's warm-up: every stream's tube table of the server tests' galaxy at the
//! Milky Way fixture's values (seed `0x4d2`, 261 streams), timed by its parts. CI compiles this
//! and never runs it; `cargo bench -p hyperion-server --bench stream_warmup` runs it once.
//!
//! - **The parts**, each table built alone on one thread, the tables spread over
//!   `HYPERION_WORKERS` threads (eight if unset): the plan ([`SprayPlan::new`], the progenitor's
//!   two passes), the spray ([`Spray::tracers`]) and the reduction ([`TubeTable::from_tracers`]),
//!   with each table's tracer steps and a digest of its bits.
//! - **On the pool**, the tables one after another through [`tube_table_on`] at bulk priority on
//!   as many workers, as a warm-up one table at a time builds them: the wall time, the process's
//!   CPU time (Linux) and the digest of every table's bits, which `compute::global_list`'s slow
//!   test pins.
//!
//! Measured (pod, 16 vCPU, 2026-11-01, the release profile, two runs of each, interleaved): 7.71 ×
//! 10⁹ tracer steps at some 62 ns a step, 98.7 % of them the 256 orphans', against 1.1 s of plans
//! and 0.4 s of reductions in all; on eight workers 62.1–62.8 s and 485–486 CPU-s when each share
//! made its own plan, and 62.0–64.2 s and 477–485 CPU-s with one plan a table. The warm-up's own
//! schedule, several tables in flight, is timed by the slow test.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use hyperion_server::compute::{
    CancelToken, CpuPool, GalaxyKey, Priority, SharedSprayForces, Shares, tube_table_on,
};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_sim::galaxy::global_list::spray::{Spray, SprayForce, SprayParams, SprayPlan};
use hyperion_sim::galaxy::global_list::{GlobalList, MassLoss, StreamSpec, TubeTable};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::{GENERATOR_VERSION, Seed};

/// The server tests' seed.
const SEED: u64 = 0x4d2;

/// FNV-1a 64's offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64 over `bytes`, continuing from `hash`.
fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// A digest of `table`'s bits, as its debug form spells every float in it (the slow test's).
fn digest(table: &TubeTable) -> u64 {
    fnv1a(FNV_OFFSET, format!("{table:?}").as_bytes())
}

/// The digest of every table's digest, in stream order: the slow test's pin.
fn digest_of(digests: impl IntoIterator<Item = u64>) -> u64 {
    digests
        .into_iter()
        .fold(FNV_OFFSET, |hash, d| fnv1a(hash, &d.to_le_bytes()))
}

/// The process's CPU time so far, s, user and system: `/proc/self/stat`'s ticks at Linux's 100 a
/// second.
#[cfg(target_os = "linux")]
fn cpu_s() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // The fields after the command's closing parenthesis, from the third: `utime` and `stime`
    // are the 14th and 15th of the whole line.
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let ticks = |i: usize| fields.get(i)?.parse::<f64>().ok();
    Some((ticks(11)? + ticks(12)?) / 100.0)
}

/// Not measured off Linux.
#[cfg(not(target_os = "linux"))]
fn cpu_s() -> Option<f64> {
    None
}

/// CPU time since `start`, as text.
fn cpu_since(start: Option<f64>) -> String {
    match (start, cpu_s()) {
        (Some(start), Some(now)) => format!("{:.1} CPU-s", now - start),
        _ => "CPU time not measured".to_owned(),
    }
}

/// One table built alone, by its parts.
#[derive(Debug, Clone)]
struct Parts {
    number: u16,
    kind: &'static str,
    steps: u32,
    tracer_steps: u64,
    plan_s: f64,
    spray_s: f64,
    reduce_s: f64,
    digest: u64,
}

/// The kind of `spec`'s progenitor, by its mass loss.
fn kind(spec: &StreamSpec) -> &'static str {
    match spec.mass_loss() {
        MassLoss::Steady(_) => "globular",
        MassLoss::Dissolved { .. } => "orphan",
        MassLoss::Pulsed { .. } => "dwarf",
    }
}

/// `spec`'s table in `galaxy`, its plan, spray and reduction timed apart.
fn parts(galaxy: &Galaxy, force: &SprayForce, spec: &StreamSpec) -> Parts {
    let params = SprayParams::default();
    let started = Instant::now();
    let plan = SprayPlan::new(galaxy.seed(), force, galaxy.potential(), spec, &params)
        .expect("a stream of the fixture has a finite stripping time");
    let planned = Instant::now();
    let tracers = Spray::with_plan(force, galaxy.potential(), &plan).tracers();
    let sprayed = Instant::now();
    let table = TubeTable::from_tracers(
        galaxy.seed(),
        galaxy.mean_system_mass(Population::Halo),
        spec,
        &tracers,
    )
    .expect("every stream of the fixture builds its tube");
    let reduced = Instant::now();
    let steps = plan.schedule().step().count();
    Parts {
        number: spec.number().get(),
        kind: kind(spec),
        steps,
        tracer_steps: plan
            .schedule()
            .releases()
            .iter()
            .map(|r| 2 * u64::from(steps - r.step()))
            .sum(),
        plan_s: (planned - started).as_secs_f64(),
        spray_s: (sprayed - planned).as_secs_f64(),
        reduce_s: (reduced - sprayed).as_secs_f64(),
        digest: digest(&table),
    }
}

/// The parts of every table, the tables spread over `threads` threads.
fn by_parts(galaxy: &Galaxy, streams: &[StreamSpec], threads: usize) {
    let force = SprayForce::new(galaxy.potential()).expect("the full potential");
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::with_capacity(streams.len()));
    let (cpu, started) = (cpu_s(), Instant::now());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(spec) = streams.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let table = parts(galaxy, &force, spec);
                    done.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(table);
                }
            });
        }
    });
    let wall = started.elapsed().as_secs_f64();
    let mut done = done.into_inner().unwrap_or_else(PoisonError::into_inner);
    done.sort_by_key(|p| p.number);
    for p in &done {
        println!(
            "stream {:3} {:8} {:6} steps, {:10} tracer steps: plan {:.4} s, spray {:.3} s, \
             reduction {:.4} s, digest {:016x}",
            p.number, p.kind, p.steps, p.tracer_steps, p.plan_s, p.spray_s, p.reduce_s, p.digest
        );
    }
    for kind in ["globular", "orphan", "dwarf"] {
        let of: Vec<&Parts> = done.iter().filter(|p| p.kind == kind).collect();
        let steps: u64 = of.iter().map(|p| p.tracer_steps).sum();
        let spray: f64 = of.iter().map(|p| p.spray_s).sum();
        println!(
            "{kind}: {} tables, {steps} tracer steps, spray {spray:.1} s",
            of.len()
        );
    }
    let sum = |f: fn(&Parts) -> f64| done.iter().map(f).sum::<f64>();
    let steps: u64 = done.iter().map(|p| p.tracer_steps).sum();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a bench's mean, of some 10^10 steps"
    )]
    let per_step_ns = sum(|p| p.spray_s) * 1e9 / steps as f64;
    println!(
        "by parts on {threads} threads: {} tables, plans {:.2} s, sprays {:.1} s, reductions \
         {:.2} s; {steps} tracer steps, {per_step_ns:.1} ns a step; {wall:.1} s wall, {}; digest \
         {:016x}",
        done.len(),
        sum(|p| p.plan_s),
        sum(|p| p.spray_s),
        sum(|p| p.reduce_s),
        cpu_since(cpu),
        digest_of(done.iter().map(|p| p.digest))
    );
}

/// Every table built on a pool of `workers`, one after another.
fn on_the_pool(galaxy: Galaxy, streams: &[StreamSpec], workers: NonZeroUsize) {
    let galaxy = Arc::new(galaxy);
    let key = GalaxyKey::new(SEED, GENERATOR_VERSION);
    let pool = CpuPool::new(workers, INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY)
        .expect("the pool starts");
    let forces = Arc::new(SharedSprayForces::new(1 << 20));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let (cpu, started) = (cpu_s(), Instant::now());
    let digests = runtime.block_on(async {
        let mut digests = Vec::with_capacity(streams.len());
        for spec in streams {
            let table = tube_table_on(
                &pool,
                &forces,
                (&galaxy, key),
                spec,
                (Priority::Bulk, Shares::for_workers(workers)),
                &CancelToken::new(),
            )
            .await
            .expect("every stream of the fixture builds its tube");
            digests.push(digest(&table));
        }
        digests
    });
    println!(
        "on the pool, one table after another on {workers} workers: {} tables in {:.1} s wall, \
         {}; digest {:016x}",
        digests.len(),
        started.elapsed().as_secs_f64(),
        cpu_since(cpu),
        digest_of(digests)
    );
    runtime
        .block_on(pool.shutdown())
        .expect("the pool shuts down");
}

fn main() {
    let workers = std::env::var("HYPERION_WORKERS")
        .ok()
        .and_then(|v| v.parse::<NonZeroUsize>().ok())
        .unwrap_or(NonZeroUsize::new(8).expect("eight is not zero"));
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the fixture's gas is mostly neutral")
        .with_full_potential();
    let list = GlobalList::build(&galaxy);
    let streams = list.debris().streams();
    by_parts(&galaxy, streams, workers.get());
    on_the_pool(galaxy, streams, workers);
}
