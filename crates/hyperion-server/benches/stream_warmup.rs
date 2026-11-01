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
//!
//! `STREAM_WARMUP=27` compares instead every orphan's tube under generator version 27's rules
//! (P10.T10.b's orphan spray, held at 27) sprayed four ways: exactly, at `τ_p ÷ 32`, from 500
//! tracers an arm, and both, the 27 arm's. Measured on the fixture's 355 orphans at 27 (pod, 16
//! vCPU, 2026-11-01): the sprays cost 860, 440, 421 and 216 s; against the exact tube, the step
//! alone moves the centre line 0.07 of a width (median) and the widths and dispersions under
//! 0.1 %, while the tracers halved move it 0.31 of a width and the widths by +2.7 %, the
//! dispersions by +7.7 % and the lengths by −6 %; with both, the worst tracer's energy drift is
//! 5.0 × 10⁻⁴, against 9.8 × 10⁻⁵ exactly.

use std::num::{NonZeroU32, NonZeroUsize};
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

/// The sprays an orphan's tube is compared under at 27: the exact one, then the step halved
/// alone, the tracers halved alone, and both, the 27 arm's.
const VARIANTS: [(&str, u32, u32); 4] = [
    ("exact", 1_000, 64),
    ("step", 1_000, 32),
    ("tracers", 500, 64),
    ("both", 500, 32),
];

/// How one variant's tube of an orphan lies against the exact one's.
#[derive(Debug, Clone, Copy, Default)]
struct Against {
    /// The centre line's distance from the exact centre line, over the exact width `√(w_n w_b)`
    /// at the nearest point: the median and the greatest over the tube.
    across: [f64; 2],
    /// How far along the exact tube the nearest point lies from the same share of the mass: the
    /// median of `|Δu|`.
    along: f64,
    /// The widths' and the dispersions' ratios at the nearest points, their medians.
    width_ratio: f64,
    dispersion_ratio: f64,
    /// The tubes' lengths' ratio.
    length_ratio: f64,
    /// The members within two exact widths of nine points of the exact centre line, over the
    /// exact tube's.
    members_ratio: f64,
}

/// What one orphan's tube moves from the exact spray to the others (P10.T10.b's orphan spray, held
/// at 27).
#[derive(Debug, Clone, Default)]
struct Moved {
    number: u16,
    /// Each variant's spray, CPU-s on its thread, and its tracers' worst energy drift from release
    /// to the epoch.
    spray_s: [f64; 4],
    drift: [f64; 4],
    /// The step's, the tracers' and both against the exact tube.
    against: [Against; 3],
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

/// The median of `values`, which it sorts, NaNs last.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(f64::NAN)
}

/// The members of `tube` in the cells about `centre` within `radius` ly, counted.
fn members_near(
    galaxy: &Galaxy,
    (tube, shares): (&TubeTable, &StreamBandShares),
    centre: &GalacticPosition,
    radius: f64,
) -> usize {
    let mut found = 0;
    let mut cell_members = Vec::new();
    for cell in tube.cells_touching(centre, LightYears::new(radius)) {
        for band in MassBand::ALL {
            stream_cell_members(galaxy, tube, shares, cell, band, &mut cell_members);
            found += cell_members
                .iter()
                .filter(|m| ly(m.record().epoch_position(), centre) <= radius)
                .count();
        }
    }
    found
}

/// The width `√(w_n w_b)` of `tube` at `u`, ly.
fn width(tube: &TubeTable, u: f64) -> f64 {
    let c = tube.locate(u, 0.0, 0.0);
    (ly(&c, &tube.locate(u, 1.0, 0.0)) * ly(&c, &tube.locate(u, 0.0, 1.0))).sqrt()
}

/// The rms of `tube`'s three velocity dispersions at `u`, m/s.
fn dispersion(tube: &TubeTable, u: f64) -> f64 {
    let d = tube.dispersions_at(u).map(MetresPerSecond::value);
    ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) / 3.0).sqrt()
}

/// The nearest point of the polyline `line` (each vertex's share of the mass and place, in ly
/// from the first) to `point`: its share and its distance, ly.
fn nearest(line: &[(f64, [f64; 3])], point: [f64; 3]) -> (f64, f64) {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    line.windows(2)
        .fold((f64::NAN, f64::INFINITY), |best, pair| {
            let [(u0, p0), (u1, p1)] = [pair[0], pair[1]];
            let d = sub(p1, p0);
            let t = (dot(sub(point, p0), d) / dot(d, d).max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
            let on = [p0[0] + t * d[0], p0[1] + t * d[1], p0[2] + t * d[2]];
            let gap = dot(sub(point, on), sub(point, on)).sqrt();
            if gap < best.1 {
                (u0 + t * (u1 - u0), gap)
            } else {
                best
            }
        })
}

/// `other` against `exact`, two tubes of `spec`'s stream in `galaxy`.
fn against(galaxy: &Galaxy, spec: &StreamSpec, exact: &TubeTable, other: &TubeTable) -> Against {
    let origin = exact.locate(0.5, 0.0, 0.0);
    let place = |t: &TubeTable, u: f64| {
        let d = origin.displacement_to(&t.locate(u, 0.0, 0.0)).metres();
        d.map(|c| c / METRES_PER_LIGHT_YEAR)
    };
    let line: Vec<(f64, [f64; 3])> = (0..=2_000)
        .map(|i| {
            let u = f64::from(i) / 2_000.0;
            (u, place(exact, u))
        })
        .collect();
    let (mut across, mut along, mut widths, mut dispersions) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for i in 0..200 {
        let u = (f64::from(i) + 0.5) / 200.0;
        let (at, gap) = nearest(&line, place(other, u));
        across.push(gap / width(exact, at));
        along.push((at - u).abs());
        widths.push(width(other, u) / width(exact, at));
        dispersions.push(dispersion(other, u) / dispersion(exact, at));
    }
    let greatest = across.iter().copied().fold(0.0, f64::max);
    let shares = StreamBandShares::of(galaxy, spec);
    let (mut of_exact, mut of_other) = (0, 0);
    for k in 1..10 {
        let u = f64::from(k) / 10.0;
        let centre = exact.locate(u, 0.0, 0.0);
        let radius = 2.0 * width(exact, u);
        of_exact += members_near(galaxy, (exact, &shares), &centre, radius);
        of_other += members_near(galaxy, (other, &shares), &centre, radius);
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "counts of members, far below 2^52"
    )]
    let members_ratio = of_other as f64 / of_exact.max(1) as f64;
    Against {
        across: [median(&mut across), greatest],
        along: median(&mut along),
        width_ratio: median(&mut widths),
        dispersion_ratio: median(&mut dispersions),
        length_ratio: other.length().value() / exact.length().value(),
        members_ratio,
    }
}

/// `spec`'s tube in `galaxy` under each of [`VARIANTS`], compared with the exact one.
fn moved(galaxy: &Galaxy, force: &SprayForce, spec: &StreamSpec) -> Moved {
    let (seed, tables, mass) = (
        galaxy.seed(),
        galaxy.potential(),
        galaxy.mean_system_mass(Population::Halo),
    );
    let built = VARIANTS.map(|(_, per_arm, steps)| {
        let params = SprayParams::new(NonZeroU32::new(per_arm).expect("not zero"))
            .with_steps_per_crossing(NonZeroU32::new(steps).expect("not zero"));
        let started = Instant::now();
        let spray = Spray::new(seed, force, tables, spec, &params).expect("a finite span");
        let tracers = spray.tracers();
        let spray_s = started.elapsed().as_secs_f64();
        let drift = worst_drift(force, &spray, &tracers);
        let table = TubeTable::from_tracers(seed, mass, spec, &tracers).expect("its tube builds");
        (table, spray_s, drift)
    });
    let both = SprayParams::of_stream(galaxy, spec);
    assert_eq!(
        (both.tracers_per_arm(), both.steps_per_crossing()),
        (VARIANTS[3].1, VARIANTS[3].2),
        "the 27 arm's spray"
    );
    let exact = &built[0].0;
    Moved {
        number: spec.number().get(),
        spray_s: core::array::from_fn(|k| built[k].1),
        drift: core::array::from_fn(|k| built[k].2),
        against: core::array::from_fn(|k| against(galaxy, spec, exact, &built[k + 1].0)),
    }
}

/// Every orphan's tube of the fixture under generator version 27's rules, at each of
/// [`VARIANTS`], compared with the exact one, the orphans spread over `threads` threads.
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
        let both = m.against[2];
        println!(
            "orphan {:3}: sprays {:.2?} s; drift {:.1e} {:.1e} {:.1e} {:.1e}; both: across \
             {:.3} / {:.3} widths, along {:.4}, widths x{:.3}, dispersions x{:.3}, length \
             x{:.3}, members x{:.3}",
            m.number,
            m.spray_s,
            m.drift[0],
            m.drift[1],
            m.drift[2],
            m.drift[3],
            both.across[0],
            both.across[1],
            both.along,
            both.width_ratio,
            both.dispersion_ratio,
            both.length_ratio,
            both.members_ratio
        );
    }
    summarise(&done);
}

/// The median, 95th percentile and greatest of `values`, NaNs dropped.
fn quantiles(mut values: Vec<f64>, scientific: bool) -> String {
    values.retain(|v| !v.is_nan());
    values.sort_by(f64::total_cmp);
    let at = |q: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a quantile's index among a few hundred values"
        )]
        let i = ((values.len().saturating_sub(1)) as f64 * q).round() as usize;
        values.get(i).copied().unwrap_or(f64::NAN)
    };
    if scientific {
        format!("{:.2e} / {:.2e} / {:.2e}", at(0.5), at(0.95), at(1.0))
    } else {
        format!("{:.3} / {:.3} / {:.3}", at(0.5), at(0.95), at(1.0))
    }
}

/// The orphans' tubes' figures at 27 against the exact spray, for each variant: the median, 95th
/// percentile and greatest over the orphans.
fn summarise(done: &[Moved]) {
    println!("at 27, over the orphans (median / 95th percentile / greatest):");
    for (k, (name, per_arm, steps)) in VARIANTS.iter().enumerate() {
        let spray: f64 = done.iter().map(|m| m.spray_s[k]).sum();
        println!(
            "  {name} ({per_arm} an arm, tau_p / {steps}): sprays {spray:.1} s; energy drift {}",
            quantiles(done.iter().map(|m| m.drift[k]).collect(), true)
        );
    }
    for (k, (name, _, _)) in VARIANTS.iter().enumerate().skip(1) {
        let of = |f: fn(&Against) -> f64| {
            quantiles(done.iter().map(|m| f(&m.against[k - 1])).collect(), false)
        };
        println!("  {name} against exact:");
        println!(
            "    across, median over the tube: {} widths",
            of(|a| a.across[0])
        );
        println!(
            "    across, greatest over the tube: {} widths",
            of(|a| a.across[1])
        );
        println!("    along, median |du|: {}", of(|a| a.along));
        println!("    widths' ratio: {}", of(|a| a.width_ratio));
        println!("    dispersions' ratio: {}", of(|a| a.dispersion_ratio));
        println!("    lengths' ratio: {}", of(|a| a.length_ratio));
        println!(
            "    members' ratio near the track: {}",
            of(|a| a.members_ratio)
        );
    }
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
