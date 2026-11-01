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
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::global_list::members::{StreamBandShares, stream_cell_members};
use hyperion_sim::galaxy::global_list::spray::{Spray, SprayForce, SprayParams, SprayPlan, Tracer};
use hyperion_sim::galaxy::global_list::{GlobalList, MassLoss, StreamSpec, TubeTable};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::SystemId;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{LightYears, MetresPerSecond};
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion, Seed};

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

/// What one orphan's tube moves from the exact spray to the coarse one (P10.T10.b's orphan spray,
/// held at 27), along its exact tube.
#[derive(Debug, Clone, Default)]
struct Moved {
    number: u16,
    /// The spray's CPU time, s: exact, coarse.
    spray_s: [f64; 2],
    /// The centre line's offset over the exact width, `√(w_n w_b)`: median and greatest over u.
    offset: [f64; 2],
    /// The widths' and the dispersions' ratios, coarse over exact, their medians over u.
    width_ratio: f64,
    dispersion_ratio: f64,
    /// The tubes' lengths' ratio.
    length_ratio: f64,
    /// The worst energy drift from release to the epoch over the tracers: exact, coarse.
    drift: [f64; 2],
    /// Members near nine points of the exact tube: exact's, coarse's, shared by ID, and the shared
    /// ones' median displacement over the exact width there.
    members: [usize; 3],
    member_shift: f64,
}

/// Every tracer's energy drift from its release to the epoch, the worst, in `spray`'s potential:
/// the leapfrog's error, the potential being static.
fn worst_drift(force: &SprayForce, spray: &Spray<'_>, tracers: &[Tracer]) -> f64 {
    let energy = |x: [f64; 3], v: [f64; 3]| {
        0.5 * (v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
            + force.potential_j_kg(&hyperion_sim::coords::GalacticDisplacement::new(x))
    };
    tracers.iter().fold(0.0_f64, |worst, t| {
        let (x, v) = spray
            .progenitor(t.release_step())
            .expect("released on the track");
        let released = energy(x.metres(), v.metres_per_second()) + t.energy_offset_j_kg();
        let now = energy(t.position().metres(), t.velocity().metres_per_second());
        worst.max((now / released - 1.0).abs())
    })
}

/// The distance from `a` to `b`, ly.
fn ly(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    a.distance_to(b).value() / METRES_PER_LIGHT_YEAR
}

/// The median of `values`, which it sorts.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(f64::NAN)
}

/// The members of `tube` in the cells about `centre` within `radius`, by ID with their places.
fn members_near(
    galaxy: &Galaxy,
    (tube, shares): (&TubeTable, &StreamBandShares),
    centre: &GalacticPosition,
    radius: f64,
) -> Vec<(SystemId, GalacticPosition)> {
    let mut found = Vec::new();
    let mut cell_members = Vec::new();
    for cell in tube.cells_touching(centre, LightYears::new(radius)) {
        for band in MassBand::ALL {
            stream_cell_members(galaxy, tube, shares, cell, band, &mut cell_members);
            found.extend(
                cell_members
                    .iter()
                    .map(|m| (m.record().id(), *m.record().epoch_position()))
                    .filter(|(_, at)| ly(at, centre) <= radius),
            );
        }
    }
    found
}

/// `spec`'s tube in `galaxy` at the exact spray and at its rules' coarse one, compared.
fn moved(galaxy: &Galaxy, force: &SprayForce, spec: &StreamSpec) -> Moved {
    let (seed, tables, mass) = (
        galaxy.seed(),
        galaxy.potential(),
        galaxy.mean_system_mass(Population::Halo),
    );
    let sprayed = [SprayParams::default(), SprayParams::of_stream(galaxy, spec)].map(|params| {
        let started = Instant::now();
        let spray = Spray::new(seed, force, tables, spec, &params).expect("a finite span");
        let tracers = spray.tracers();
        let spray_s = started.elapsed().as_secs_f64();
        let drift = worst_drift(force, &spray, &tracers);
        let table = TubeTable::from_tracers(seed, mass, spec, &tracers).expect("its tube builds");
        (table, spray_s, drift)
    });
    let [
        (exact, exact_s, exact_drift),
        (coarse, coarse_s, coarse_drift),
    ] = sprayed;
    let width = |t: &TubeTable, u: f64| {
        let c = t.locate(u, 0.0, 0.0);
        (ly(&c, &t.locate(u, 1.0, 0.0)) * ly(&c, &t.locate(u, 0.0, 1.0))).sqrt()
    };
    let rms = |t: &TubeTable, u: f64| {
        let d = t.dispersions_at(u).map(MetresPerSecond::value);
        ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) / 3.0).sqrt()
    };
    let samples: Vec<f64> = (0..400).map(|i| (f64::from(i) + 0.5) / 400.0).collect();
    let mut offsets: Vec<f64> = samples
        .iter()
        .map(|&u| ly(&exact.locate(u, 0.0, 0.0), &coarse.locate(u, 0.0, 0.0)) / width(&exact, u))
        .collect();
    let greatest = offsets.iter().copied().fold(0.0, f64::max);
    let mut widths: Vec<f64> = samples
        .iter()
        .map(|&u| width(&coarse, u) / width(&exact, u))
        .collect();
    let mut dispersions: Vec<f64> = samples
        .iter()
        .map(|&u| rms(&coarse, u) / rms(&exact, u))
        .collect();
    let shares = StreamBandShares::of(galaxy, spec);
    let mut members = [0; 3];
    let mut shifts = Vec::new();
    for k in 1..10 {
        let u = f64::from(k) / 10.0;
        let centre = exact.locate(u, 0.0, 0.0);
        let radius = 2.0 * width(&exact, u);
        let near = |t: &TubeTable| members_near(galaxy, (t, &shares), &centre, radius);
        let (of_exact, of_coarse) = (near(&exact), near(&coarse));
        members[0] += of_exact.len();
        members[1] += of_coarse.len();
        for (id, at) in &of_exact {
            if let Some((_, there)) = of_coarse.iter().find(|(other, _)| other == id) {
                members[2] += 1;
                shifts.push(ly(at, there) / (radius / 2.0));
            }
        }
    }
    Moved {
        number: spec.number().get(),
        spray_s: [exact_s, coarse_s],
        offset: [median(&mut offsets), greatest],
        width_ratio: median(&mut widths),
        dispersion_ratio: median(&mut dispersions),
        length_ratio: coarse.length().value() / exact.length().value(),
        drift: [exact_drift, coarse_drift],
        members,
        member_shift: median(&mut shifts),
    }
}

/// Every orphan's tube of the fixture under generator version 27's rules, at the exact spray and
/// at the coarse, compared, the orphans spread over `threads` threads.
fn at_27(threads: usize) {
    let galaxy = Galaxy::from_params_at_version(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GeneratorVersion::new(27),
    )
    .expect("the fixture's gas is mostly neutral")
    .with_full_potential();
    let list = GlobalList::build(&galaxy);
    let force = SprayForce::new(galaxy.potential()).expect("the full potential");
    let all = list.debris().streams();
    let orphans: Vec<&StreamSpec> = all
        .iter()
        .filter(|s| SprayParams::of_stream(&galaxy, s) != SprayParams::default())
        .collect();
    println!(
        "at 27: {} streams, {} sprayed coarse, every one an orphan: {}",
        all.len(),
        orphans.len(),
        orphans
            .iter()
            .all(|s| matches!(s.mass_loss(), MassLoss::Dissolved { .. }))
    );
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::with_capacity(orphans.len()));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(spec) = orphans.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let one = moved(&galaxy, &force, spec);
                    done.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(one);
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap_or_else(PoisonError::into_inner);
    done.sort_by_key(|m| m.number);
    for m in &done {
        println!(
            "orphan {:3}: spray {:.2} s / {:.2} s; offset {:.3} (median) {:.3} (greatest) widths; \
             widths x{:.3}, dispersions x{:.3}, length x{:.3}; drift {:.1e} / {:.1e}; members \
             {} / {}, {} shared, shifted {:.3} widths",
            m.number,
            m.spray_s[0],
            m.spray_s[1],
            m.offset[0],
            m.offset[1],
            m.width_ratio,
            m.dispersion_ratio,
            m.length_ratio,
            m.drift[0],
            m.drift[1],
            m.members[0],
            m.members[1],
            m.members[2],
            m.member_shift
        );
    }
    summarise(&done);
}

/// The orphans' tubes' figures at 27 against the exact spray: each one's median, 95th percentile
/// and greatest.
fn summarise(done: &[Moved]) {
    let column = |f: fn(&Moved) -> f64| {
        let mut v: Vec<f64> = done.iter().map(f).collect();
        v.sort_by(f64::total_cmp);
        let at = |q: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "a quantile's index among a few hundred values"
            )]
            let i = ((v.len() - 1) as f64 * q).round() as usize;
            v[i]
        };
        format!("{:.3} / {:.3} / {:.3}", at(0.5), at(0.95), at(1.0))
    };
    let sum = |f: fn(&Moved) -> usize| done.iter().map(f).sum::<usize>();
    println!("at 27, over the orphans, median / 95th percentile / greatest:");
    println!(
        "  centre offset, median over u: {} widths",
        column(|m| m.offset[0])
    );
    println!(
        "  centre offset, greatest over u: {} widths",
        column(|m| m.offset[1])
    );
    println!("  widths' ratio: {}", column(|m| m.width_ratio));
    println!("  dispersions' ratio: {}", column(|m| m.dispersion_ratio));
    println!("  lengths' ratio: {}", column(|m| m.length_ratio));
    println!("  energy drift, exact: {}", column(|m| m.drift[0]));
    println!("  energy drift, coarse: {}", column(|m| m.drift[1]));
    println!(
        "  shared members' shift: {} widths",
        column(|m| m.member_shift)
    );
    println!(
        "  members near the sample points: {} exact, {} coarse, {} shared by ID; sprays {:.1} s \
         exact, {:.1} s coarse",
        sum(|m| m.members[0]),
        sum(|m| m.members[1]),
        sum(|m| m.members[2]),
        done.iter().map(|m| m.spray_s[0]).sum::<f64>(),
        done.iter().map(|m| m.spray_s[1]).sum::<f64>()
    );
}

fn main() {
    let workers = std::env::var("HYPERION_WORKERS")
        .ok()
        .and_then(|v| v.parse::<NonZeroUsize>().ok())
        .unwrap_or(NonZeroUsize::new(8).expect("eight is not zero"));
    // `STREAM_WARMUP=27` compares the orphans' tubes at 27 instead.
    if std::env::var("STREAM_WARMUP").is_ok_and(|v| v == "27") {
        at_27(workers.get());
        return;
    }
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the fixture's gas is mostly neutral")
        .with_full_potential();
    let list = GlobalList::build(&galaxy);
    let streams = list.debris().streams();
    by_parts(&galaxy, streams, workers.get());
    on_the_pool(galaxy, streams, workers);
}
