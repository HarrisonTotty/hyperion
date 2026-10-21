//! Benchmarks of plan 09's coarse pass, its steps one at a time (R09.T14.d). A miss is a finding,
//! not a CI failure: CI compiles these and never runs them.
//!
//! - `coarse/erosion_l8`: the erosion step ([`steps::erosion::run`]) on the reference Earth at
//!   level 8 (393,216 cells), wet now: the climate's runoff read, the construction's lakes by their
//!   water balance, the multigrid's steady state from level 4 to level 8, the remap onto the
//!   construction, the re-match of `σ_h`, the sea placed again, the lakes on the final field, the
//!   climate's re-application and the final routing. The world is staged to the end of its climate
//!   step once, outside the timed closure, and each iteration runs the step on a clone of that
//!   state (`iter_batched`, `BatchSize::LargeInput`), ten samples.
//!
//! The target is the brainstorm's "erosion alone is a few seconds at level 8 (step 4)"
//! (`docs/agent/brainstorming/rendering-and-planets.md`, the coarse pass's grid), as plan 09's
//! R09.T14 records it, set against Tzathas et al. 2024's own timings of their method (CGF 43(2),
//! doi:10.1111/cgf.15033, Table 2): 1.79 s on a 512² grid and 8.18 s on a 1,024² one, in Python
//! with numba on one Xeon E5-2650 v4. Level 8's 393,216 cells lie between their 262,144 and
//! 1,048,576.
//!
//! Measured 2026-10-20, before R09.T14's review fixes: 10.476 s (10.425–10.527 s) per
//! `erosion::run` on the reference Earth at level 8, under the heavy-test lock at a 1-minute load
//! of about 4–9 with the schedutil governor (this machine: a Ryzen 7 3700X, 16 threads). That is
//! above the target's few seconds, a finding for plan 09's R09.T14 records, not a CI failure. The
//! lane re-times it after the fixes.

use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperion_sim::planetary::surface::CellGraph;
use hyperion_sim::planetary::surface::reference;
use hyperion_sim::planetary::surface::steps::{self, Pass, Working};
use hyperion_surface::field::coarse_level;

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

criterion_group!(benches, erosion_l8);
criterion_main!(benches);
