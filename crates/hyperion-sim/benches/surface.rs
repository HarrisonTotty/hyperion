//! Benchmarks of plan 09's coarse pass (R09.T14.d, R09.T16). A miss is a finding, not a CI
//! failure: CI compiles these and never runs them.
//!
//! - `coarse/pass_l5` to `coarse/pass_l8` (R09.T16): the whole pass ([`coarse_pass`]), from the
//!   inputs to the quantised field, on the reference world of each level: Ceres at level 5 (6,144
//!   cells), the Moon at 6 (24,576), Mars at 7 (98,304) and the Earth at 8 (393,216). Each world
//!   runs the steps its regime names, so a level's figure is its world's, not the level's alone:
//!   the Earth alone runs the moving plates, the sea, the energy balance and its water cycle, and
//!   the steady state of a world wet now, and Mars its wet epoch's erosion. Ten samples a level,
//!   one pass each (`SamplingMode::Flat`). Once a level's samples are taken, the bench prints the
//!   field's heap bytes ([`CoarseField::heap_bytes`]) and, where the platform reports it (Linux's
//!   `/proc/self/status`), the process's peak resident set (`VmHWM`). The levels run from 5 to 8,
//!   each pass's working state freed before the next, so each level's peak is its own pass's
//!   (plan 09's "Concurrent passes" lean: a pool of passes holds that a pass). With
//!   `HYPERION_COARSE_STEPS` set, each level's pass runs once more step by step, its field
//!   asserted to be the pass's, printing each step's wall time: a run whose filter leaves out every
//!   sample (`-- coarse/steps`) prints the breakdown alone.
//! - `coarse/erosion_l8` (R09.T14.d): the erosion step ([`steps::erosion::run`]) on the reference
//!   Earth at level 8, wet now: the climate's runoff read, the construction's lakes by their water
//!   balance, the multigrid's steady state from level 4 to level 8, the remap onto the
//!   construction, the re-match of `σ_h`, the sea placed again, the lakes on the final field, the
//!   climate's re-application and the final routing. The world is staged to the end of its climate
//!   step once, outside the timed closure, and each iteration runs the step on a clone of that
//!   state (`iter_batched`, `BatchSize::LargeInput`), ten samples.
//!
//! The pass's target is the brainstorm's "of order seconds to tens of seconds of Rust rather than
//! minutes, with erosion and the climate model dominating"
//! (`docs/agent/brainstorming/rendering-and-planets.md`, the coarse pass's grid), for a pass the
//! server starts "while the ship is still on approach". The erosion step's is the same passage's
//! "erosion alone is a few seconds at level 8 (step 4)", as plan 09's R09.T14 records it, set
//! against Tzathas et al. 2024's own timings of their method (CGF 43(2), doi:10.1111/cgf.15033,
//! Table 2): 1.79 s on a 512² grid and 8.18 s on a 1,024² one, in Python with numba on one Xeon
//! E5-2650 v4. Level 8's 393,216 cells lie between their 262,144 and 1,048,576.
//!
//! Measured 2026-10-20, before R09.T14's review fixes: 10.476 s (10.425–10.527 s) per
//! `erosion::run` on the reference Earth at level 8, under the heavy-test lock at a 1-minute load
//! of about 4–9 with the schedutil governor (this machine: a Ryzen 7 3700X, 16 threads). That is
//! above the target's few seconds, a finding for plan 09's R09.T14 records, not a CI failure; after
//! the review's fixes it took 10.504 s (plan 09, R09.T14's _The bench_).
//!
//! Measured 2026-10-23 (R09.T16), under the heavy-test lock at `CPUQuota` 1200%, one thread a
//! pass, with the 1-minute load and `/proc/pressure/io`'s "full" avg10 over each bench's samples:
//!
//! | Bench | Time | Field heap | Peak resident set | Load; I/O full |
//! | ----- | ---- | ---------- | ----------------- | -------------- |
//! | `coarse/pass_l5` (Ceres) | 279.98 ms (279.49–280.56) | 351,476 B | 22.3 MB, the process's floor | 1.7–5.0; ≤ 0.14 |
//! | `coarse/pass_l6` (the Moon) | 460.89 ms (460.32–461.49) | 1,467,156 B | 22.3 MB | 1.7–5.0; ≤ 0.14 |
//! | `coarse/pass_l7` (Mars) | 5.783 s (5.751–5.829) | 5,019,132 B | 80.9 MB | 1.7–5.0; ≤ 0.14 |
//! | `coarse/pass_l8` (the Earth) | 94.884 s (94.376–95.572) | 19,073,448 B | 408.0 MB | 1.4–6.9; 0–26.8 |
//! | `coarse/erosion_l8` | 17.881 s (17.809–17.954) | | | 2.2–4.8; ≤ 9.4 |
//!
//! By step (`HYPERION_COARSE_STEPS`, one pass each, load 3.6–5.8, I/O full ≤ 9.7), the Earth's
//! 94.44 s is the climate's 68.88 s, the erosion's 18.09 s, the relief after the craters' 5.10 s,
//! the plates' 1.25 s, the relief's 0.82 s and 0.32 s for the rest; Mars's 5.82 s the erosion's
//! 2.29 s, the relief after the craters' 2.21 s and the late craters' 0.84 s. The Earth's pass is
//! over the target's tens of seconds, its climate step over the 30 s that R09.T13.e's ruling set
//! for the reference Earth, and its erosion over T14's 10.50 s: findings for plan 09's R09.T16
//! records, not CI failures.

use std::time::{Duration, Instant};

use criterion::{BatchSize, Criterion, SamplingMode, criterion_group, criterion_main};
use hyperion_sim::planetary::surface::steps::{self, Pass, Working};
use hyperion_sim::planetary::surface::{CellGraph, CoarseInputs, coarse_pass, quantise, reference};
use hyperion_surface::field::{CoarseField, coarse_level};

/// One of the pass's levels in the bench: the reference world of that level, and the time its ten
/// samples may take.
struct PassBench {
    /// The coarse level, 5 to 8.
    level: u8,
    /// The reference world whose radius gives that level.
    world: fn() -> CoarseInputs,
    /// Criterion's measurement time for the ten samples, s.
    seconds: u64,
}

/// The reference world of each of the pass's levels, 5 to 8.
const LEVELS: [PassBench; 4] = [
    PassBench {
        level: 5,
        world: reference::ceres_like,
        seconds: 10,
    },
    PassBench {
        level: 6,
        world: reference::moon_like,
        seconds: 20,
    },
    PassBench {
        level: 7,
        world: reference::mars_like,
        seconds: 120,
    },
    PassBench {
        level: 8,
        world: reference::earth_like,
        seconds: 1_000,
    },
];

/// The whole pass on the reference world of each level.
fn pass(c: &mut Criterion) {
    let mut group = c.benchmark_group("coarse");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    for PassBench {
        level,
        world,
        seconds,
    } in LEVELS
    {
        let inputs = world();
        assert_eq!(
            coarse_level(inputs.radius()).get(),
            level,
            "the reference world of level {level}"
        );
        group.measurement_time(Duration::from_secs(seconds));
        let mut field: Option<CoarseField> = None;
        group.bench_function(format!("pass_l{level}"), |b| {
            b.iter(|| {
                // The previous sample's field is dropped before the pass, so that the peak
                // resident set is one pass's; the last is kept, so that its heap is measured.
                field = None;
                field = Some(coarse_pass(reference::SEED, &inputs));
            });
        });
        if let Some(field) = &field {
            eprintln!(
                "coarse/pass_l{level}: field heap {} B; peak resident set {}",
                field.heap_bytes(),
                peak_resident_set().unwrap_or_else(|| "not reported".to_owned())
            );
        }
        if std::env::var_os(STEPS_VARIABLE).is_some() {
            print_steps(level, &inputs, field.as_ref());
        }
    }
    group.finish();
}

/// The variable that asks for each level's pass step by step ([`print_steps`]).
const STEPS_VARIABLE: &str = "HYPERION_COARSE_STEPS";

/// The pass on `inputs` once more, step by step in [`coarse_pass`]'s order, printing each step's
/// wall time, its field asserted to be `field` (the samples' last) or, with none (a filter that
/// left the level's samples out), a pass's: the pass's cost by step, which plan 09's performance
/// list reads.
fn print_steps(level: u8, inputs: &CoarseInputs, field: Option<&CoarseField>) {
    let mut laps: Vec<(&str, Duration)> = Vec::new();
    let mut lap = |name: &'static str, start: Instant| laps.push((name, start.elapsed()));
    let start = Instant::now();
    let grid = CellGraph::for_field(coarse_level(inputs.radius()));
    let climate_grid = CellGraph::for_climate(coarse_level(inputs.radius()));
    let pass = Pass::new(reference::SEED, inputs, &grid, &climate_grid);
    let mut working = Working::initial(&pass);
    lap("grid", start);
    let start = Instant::now();
    steps::plates::run(&pass, &mut working);
    lap("plates", start);
    let start = Instant::now();
    steps::relief::run(&pass, &mut working);
    lap("relief", start);
    let start = Instant::now();
    steps::craters::run(&pass, &mut working);
    lap("craters", start);
    let start = Instant::now();
    steps::relief::run_after_craters(&pass, &mut working);
    lap("relief after craters", start);
    let start = Instant::now();
    steps::climate::run(&pass, &mut working);
    lap("climate", start);
    let start = Instant::now();
    steps::erosion::run(&pass, &mut working);
    lap("erosion", start);
    let start = Instant::now();
    steps::craters::run_after_erosion(&pass, &mut working);
    lap("craters after erosion", start);
    let start = Instant::now();
    steps::classes::run(&pass, &mut working);
    lap("classes", start);
    let start = Instant::now();
    let staged = quantise(inputs, working).expect("the reference world's field quantises");
    lap("quantise", start);
    match field {
        Some(field) => assert!(
            staged == *field,
            "level {level}: the staged pass is the pass"
        ),
        None => assert!(
            staged == coarse_pass(reference::SEED, inputs),
            "level {level}: the staged pass is the pass"
        ),
    }
    let total: f64 = laps.iter().map(|(_, d)| d.as_secs_f64()).sum();
    let steps: Vec<String> = laps
        .iter()
        .map(|(name, d)| format!("{name} {:.3} s", d.as_secs_f64()))
        .collect();
    eprintln!(
        "coarse/pass_l{level} by step ({total:.3} s): {}",
        steps.join(", ")
    );
}

/// The process's peak resident set, as Linux's `/proc/self/status` reports it (`VmHWM`); none on
/// a platform that has no such file.
fn peak_resident_set() -> Option<String> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .map(|value| value.trim().to_owned())
}

/// The erosion step on the reference Earth at level 8, from its state at the end of its climate
/// step.
fn erosion_l8(c: &mut Criterion) {
    let inputs = reference::earth_like();
    let level = coarse_level(inputs.radius());
    assert_eq!(level.get(), 8, "the reference Earth is a level-8 field");
    let grid = CellGraph::for_field(level);
    let climate_grid = CellGraph::for_climate(level);
    let pass = Pass::new(reference::SEED, &inputs, &grid, &climate_grid);
    let mut staged = Working::initial(&pass);
    steps::plates::run(&pass, &mut staged);
    steps::relief::run(&pass, &mut staged);
    steps::craters::run(&pass, &mut staged);
    steps::relief::run_after_craters(&pass, &mut staged);
    steps::climate::run(&pass, &mut staged);
    let mut group = c.benchmark_group("coarse");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.bench_function("erosion_l8", |b| {
        b.iter_batched(
            || staged.clone(),
            |mut working| {
                steps::erosion::run(&pass, &mut working);
                working
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

criterion_group!(benches, pass, erosion_l8);
criterion_main!(benches);
