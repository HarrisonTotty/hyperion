//! Benchmarks of the sky (rendering plan R06): the luminosity tables' build, the census near the
//! Sun with a cold and a warm cell cache and under a camera's cut, the census in the nuclear disc
//! (R06.T8), the census's bound star by star, step by step (R06.T8.g), the illumination of the
//! diffuse galactic light (R06.T9.g), the band near the Sun, marched with and without that light
//! and summed (R06.T9.b, T9.f, T9.g) and under a camera's cut (R06.T9.j), and the limit map
//! (R06.T9.i).
//!
//! They run on `GalaxyParams::milky_way_like()` with a fixed seed. A miss is a finding to record,
//! not a CI failure: CI compiles these and never runs them. Every figure below is provisional
//! until taken on a quiet machine (the roadmap's "Measurements on a quiet machine").
//!
//! The censuses run much as the server will run them (R06.T11): the plan on one thread (its caps,
//! `layer_caps`, 768 realised profiles, which the server will split into pool jobs, so the printed
//! wall time overstates its), then the plan's slabs (`CensusPlan::slabs`, an x slab of one layer's
//! walk each, its cells streamed, R06.T8.f) as jobs on a pool of the machine's threads less one,
//! each job with its own noise cache, then `merge_census`. Their time is the **CPU time** a
//! census costs, the brainstorm's measure: the plan's time plus every job's, summed over the
//! threads (wall time per job, which on a quiet machine is its CPU time). Each prints its tallies
//! once, on its first census, with each layer's records past the floor, the systems generated and
//! their share. The cut is the eye's near the Sun, V 7.95 with the eye asked (Crumey
//! 2014's 7.4 at the Sun's darkest band texel, plus 0.45 for the colour offset and 0.1 of pad:
//! R06.T7 and `decision-r06-t7-caps.md`), until R06.T9's `eye_cut` computes it. The nuclear disc
//! takes it as a stand-in: its own eye cut will be shallower, so its figure is an upper bound.
//!
//! **Sampled** (R06.T8.f): with `HYPERION_SKY_BENCH_SAMPLE=k`, a census takes only the cells whose
//! cache block's hash ([`sample_hash`], a fixed mixer of the layer and the block's coordinates;
//! since R06.T8.h whole blocks of 4³ cells, so that the cache's bytes scale) is 0 modulo k, and
//! prints its tallies and CPU time scaled by k, labelled an estimate: the plan's time and the walk
//! of every slab's cells unscaled, the sampled cells' census times k. The time each iteration
//! returns to criterion is that estimate. Unset, or 1, censuses every cell.
//!
//! The cell cache is the benches' own, built on `SkyBlock::with_cell` as the server's is
//! (R06.T11.b, T8.h): blocks of 4³ cells, least recently used out, bounded by
//! `HYPERION_SKY_CACHE_MB` (default 64 MiB) divided by the sample, so that a sampled warm census
//! is the shipped size's (`decision-r06-census-cost-signoff.md`, condition 4), each block weighing
//! its held records plus its own size. The cold census starts each iteration with an empty cache;
//! `census_near_sun/cold` reads no cache, R06.T8.g's path, so that its record reads it, as do
//! `/cold_eye_visibility` and `/cold_spheres`, the same path on R06.T7.b's caps by the eye's
//! visibility and on R06.T7's spheres (the query's own are T7.b's caps by ray); the warm
//! one starts with the cache one census of the same query left, whose fill is its cold figure;
//! the warm jump
//! (`census_near_sun/warm_jump`, R06.T8.h) with the cache the Sun's census left, for a census
//! 1,000 ly toward the galactic centre, against that destination's cold census. Each prints, per
//! layer, the records held and pre-filtered and the cells served and rebuilt by cause, the cache's
//! blocks, cells built and bytes (times the sample, the sky's estimate), and the warm ratio.
//!
//! | Bench | Target | Figure |
//! | ----- | ------ | ------ |
//! | `sky/luminosity_tables` | ≤ 30 CPU-s (T17) | pending a quiet machine |
//! | `sky/census_near_sun/cold` | ≤ 4,000 CPU-s (T17); recorded, not gated (T8.g) | 9.46 × 10⁵ CPU-s on 15 workers, 7.43 × 10⁵ on 3, at P11.T17.c (below) |
//! | `sky/census_near_sun/cold_eye_visibility` | none: recorded (T8.g) | 7.16 × 10⁵ CPU-s on 15 workers, at P11.T17.c |
//! | `sky/census_near_sun/cold_spheres` | none: recorded (T8.g; the first gate's 6 × 10⁴) | 1.45 × 10⁶ CPU-s on 15 workers, 1.13 × 10⁶ on 3, at P11.T17.c |
//! | `sky/census_near_sun/cold_camera` | none: recorded beside the eye's (T8.g) | 2.80 × 10⁶ CPU-s on 15 workers, at P11.T17.c |
//! | `sky/census_near_sun/warm` | gate: none rebuilt, the same reply, within the default (T17, T8.n); target ≤ 25% of cold | provisional at P11.T17.a (below) |
//! | `sky/census_near_sun/warm_jump` | gate: none rebuilt, the same reply, within the default (T17, T8.n); target ≤ 25% of cold | provisional at P11.T17.a (below) |
//! | `sky/star_bound/*` | about 6 µs a record but `hierarchy_bound` (T8.g) | at most 2.7 µs a step, at P11.T17.c (below) |
//! | `sky/census_nuclear_disc` | none like for like (below) | not yet run |
//! | `sky/caps_by_ray_near_sun` | none: a record (R06.T7.b) | 1,475 CPU-s, sampled 1 in 1,000 |
//! | `sky/illumination` | with the march's increase, ≤ 10% of `/march_no_dgl` (R06.T9.g) | 0.947 CPU-s, provisional |
//! | `sky/band_near_sun/march` | within the first sky's (T17) | 16.11 CPU-s with the diffuse light, provisional |
//! | `sky/band_near_sun/march_no_dgl` | none (the lit march's reference, R06.T9.g) | 15.73 CPU-s, provisional |
//! | `sky/band_near_sun/sum` | within each reply's (T17) | 0.002 CPU-s, provisional |
//! | `sky/band_near_sun/march_camera` | within the first sky's (T17) | 29.5 CPU-s, provisional |
//! | `sky/band_near_sun/march_camera_no_eye` | none (the camera's march's reference) | 22.5 CPU-s, provisional |
//! | `sky/limit_map/near_sun` | within the first sky's (T17) | 0.32 CPU-s, provisional |
//! | `sky/limit_map/synthetic_300k` | ≤ 3 CPU-s (R06.T9.i) | 1.03 CPU-s, provisional |
//!
//! The cold near-Sun figure is R06.T8.f's sampled run (2026-10-05, `HYPERION_SKY_BENCH_SAMPLE`
//! 1,000, criterion's `--test`, 15 workers, load about 15, so provisional): 1.83 × 10⁶ CPU-s
//! estimated, 125 s wall for the sample, over 1.08 × 10⁸ cells and some 4.0 × 10⁸ records past the
//! floor, of which C generates 98.3%, D 99.9% and E 99.99%. Nearly all of it is generating
//! systems. The census-cost ruling (`decision-r06-census-cost.md`) retires the brainstorm's 5–10
//! CPU-s and orders the levers, of which R06.T8.g's bound star by star is the one that moves it.
//! The nuclear-disc bench is left for R06.T17 or the owner, on a quiet machine; the warm ones'
//! provisional run is R06.T8.h's (below).
//!
//! `sky/caps_by_ray_near_sun` (R06.T7.b) censuses the union of three plans near the Sun at 7.95,
//! sampled: R06.T7's spheres, the caps by ray, and the caps by the eye's visibility, sorting each
//! star by the plans that open its cell. Its one run (2026-10-08, sampled 1 in 1,000, under the
//! heavy-test lock, three workers at `CPUQuota=400%`, so provisional): 134,535 cells in 1,475
//! CPU-s and 40 stars, none dropped or gained by either per-ray plan in the sample (under one is
//! expected a layer); the plans open 1.08 × 10⁸, 1.04 × 10⁸ and 8.7 × 10⁷ cells, walked in 184 s
//! on one thread; the caps by ray took about 12 s a plan.
//!
//! The brainstorm's 400–800 CPU-s and 5 × 10⁹ candidates are the inner bulge's under the near-Sun
//! caps held fixed; the nuclear disc's bench takes its own caps, which are far smaller.
//!
//! Since R06.T8.g each census also prints, per layer, its bound star by star: the records it
//! bounded (those the widened envelope's bound before the drift passed), those holding a pair
//! plan 11 cannot bound, and their pairs' shares by plan 11's verdict. `census_near_sun/
//! cold_camera` is the cold census of a camera at V 10.06 with the eye asked at 7.95, whose figure
//! T8.g records beside the eye's (`decision-r06-census-cost-signoff.md`). `sky/star_bound` times
//! the bound's steps one by one over the 400 records of each stellar layer nearest the Sun, plan
//! 11's P11.T16 bench's sample: `draw_metallicity`, `hierarchy_bound`, the pairs' verdicts and η
//! draws, the phase envelope's reads, and the whole; it prints each layer's stars bounded, pairs
//! and records unbounded.
//!
//! R06.T8.g's provisional runs (2026-10-07, written against P11.T17.a, whose `pair_light_bound`
//! answers only `Detached` or none; unlocked, inside a 400% CPU quota, load 7–10):
//! - `sky/star_bound`, a record's cost, µs, in A–E: `draw_metallicity` 0.06 in each;
//!   `hierarchy_bound` 4.8, 6.9, 17.8, 51.9 and 143; the pairs and η 0.33, 0.44, 0.81, 1.39 and
//!   2.04; the phase reads 0.33, 0.34, 0.30, 0.09 and 0.015 (a record with a pair plan 11 cannot
//!   bound reads none); the whole 5.1, 7.7, 16.9, 53.2 and 159.
//! - `census_near_sun/cold`, `HYPERION_SKY_BENCH_SAMPLE` 1,000, criterion's `--test`, three
//!   workers: 1.44 × 10⁶ CPU-s estimated (486 s wall for the sample), against T8.f's 1.83 × 10⁶.
//!   C generates 20.8% of its records past the floor, D 55.9% and E 86.3% (98.3%, 99.9% and
//!   99.99% before). Plan 11 cannot bound 27.7% of C's pairs, 77.1% of D's and 99.4% of E's, so
//!   17.8%, 55.7% and 86.0% of their records keep the widened bound alone. T8.g's gates wait for
//!   P11.T17.c's verdicts (R06's Risks, "Deviations in T8.g, as built").
//!
//! R06.T8.g's final runs (2026-10-08, at P11.T17.c's answers, before its bulge fix; under the
//! heavy-test lock, load 1–15; `HYPERION_SKY_BENCH_SAMPLE` 1,000 by block, criterion's `--test`,
//! at 7.95). The cold census is recorded, not gated (`decision-p11-t17c-bright.md`). A census's
//! CPU-s is its jobs' wall time summed, so 15 workers on the dev machine's 8 cores with SMT read
//! about 1.28 times what 3 workers at `CPUQuota=400%` read, with the same tallies:
//! - `cold`, T7.b's caps by ray at the uniform cut (1.04 × 10⁸ cells): 9.46 × 10⁵ CPU-s on 15
//!   workers, 7.43 × 10⁵ on 3. C generates 19.7% of its records past the floor, D 49.0% and E
//!   69.4%, 35.3% of C to E.
//! - `cold_eye_visibility` (8.70 × 10⁷ cells): 7.16 × 10⁵ CPU-s on 15 workers; C 19.9%, D 48.9%
//!   and E 71.3%.
//! - `cold_spheres` (C 8,193, D 9,925 and E 21,369 ly; 1.08 × 10⁸ cells): 1.45 × 10⁶ CPU-s on 15
//!   workers, 1.13 × 10⁶ on 3, against 1.44 × 10⁶ at P11.T17.a on 3. C 19.9%, D 48.9% and E 64.1%,
//!   40.0% of C to E (20.8%, 55.9% and 86.3% at P11.T17.a).
//! - `cold_camera`, at V 10.06 on T7.b's caps by ray (1.46 × 10⁹ cells, walked in 994 s on one
//!   thread): 2.80 × 10⁶ CPU-s on 15 workers, 3.0 times the eye's. C 20.7%, D 49.7% and E 77.5%.
//! - The pairs' verdicts on the spheres, `Detached` / `Unchanged` / `Remnants` / `Bright` / none:
//!   C 72.0 / 0.01 / 0.45 / 25.2 / 2.3%, D 20.9 / 0.59 / 16.2 / 58.7 / 3.6% and E 0.56 / 1.07 /
//!   33.0 / 63.2 / 2.1%.
//! - `sky/star_bound`, a record's cost, µs, in A–E: `draw_metallicity` 0.06 in each;
//!   `hierarchy_bound` 4.4, 6.9, 15.9, 51.9 and 138; the pairs and η 0.39, 0.45, 0.73, 1.25 and
//!   2.72; the phase reads 0.27, 0.32, 0.38, 0.43 and 1.16; the whole 5.1, 7.7, 17.1, 55.3 and
//!   142.
//!
//! R06.T8.h's provisional run (2026-10-08, written against P11.T17.a; `census_near_sun/warm` and
//! `/warm_jump`, `HYPERION_SKY_BENCH_SAMPLE` 1,000 by block, criterion's `--test`, 15 workers under
//! the heavy-test lock while other lanes built, load about 15, so provisional). The caps are
//! T7.b's by ray at the uniform cut: C 14,563, D 13,232 and E 46,010 ly at most, 1.04 × 10⁸ cells
//! near the Sun. `HYPERION_SKY_CACHE_MB` was 65,536, so that the sampled cache (65.5 MiB) held
//! the sample. At the shipped 64 MiB, the 64 KiB a sampled run gets would hold under 1% of it, as
//! the whole sky's would. Estimated, CPU-s:
//! - the Sun's cold census (the fills) 1.08 and 1.14 × 10⁶, about 200 s wall each;
//! - the same query again, every cell served and none rebuilt: 1.10 × 10⁶, 102% of its fill;
//! - 1,000 ly toward the centre, the destination cold 1.06 × 10⁶; through the Sun's entries 1.28 ×
//!   10⁶, 121%. 84,581 of the sample's 93,514 cells (90.4%) were served, 8,933 (9.6%) new ones
//!   merged in, none rebuilt.
//! - Both warm replies equal their cold ones, stars and tallies, with the same systems generated
//!   (133,946 and 133,687 in the sample).
//! - The warm jump bounds C's records star by star 5.9 × 10⁷ times against 2.0 × 10⁸ cold, D's 3.1
//!   against 5.0 × 10⁷ and E's 6.3 against 7.3 × 10⁷: some 5,000 CPU-s saved. That is under 1% of a
//!   census whose generation is 98% of its cost, so both ratios are the noise of a shared machine
//!   about 100%.
//! - The entries: 1.36 × 10⁸ records held, 13.6 GB for the Sun's sky and 14.4 GB with the jump's
//!   new cells, 5.0 bytes a built cell beyond its records. Generated systems with a star past the
//!   cut unextinguished, which no bound of their own stars could skip: 2.25 × 10⁵ of 1.34 × 10⁸.
//!
//! The band benches (R06.T9.f) split the band of a near-Sun request at the eye's cut as the server
//! runs it (R06.T11.c, T11.d), one face row a job over all six faces of `BandSpec::STANDARD` (64²
//! texels a face, 24,576 rays). `band_near_sun/march` marches the rays once, keeping every reply of
//! R06.T8.i's shell plan (C, D and E complete to 125, 250 and 500 ly, then 1,000 × 2^k ly, held at
//! their caps, then the caps; A, B and the brown dwarfs to their caps: the plan's own replies at
//! R06.T11.g's edges), and prints its heap.
//! `band_near_sun/sum` sums the final reply's texels from that march. Their census is empty, since
//! the overflow's points cost nothing beside the rays. Their time is the CPU time, summed over the
//! jobs, as the censuses'. R06.T9.f's one run of each (2026-10-07, criterion's `--test`, three
//! workers at `CPUQuota=400%`, without the heavy-test lock, load 13–17, so provisional): seven
//! replies and 22 slots a ray, a heap of 21.8 MB, 22.7 CPU-s for the march (7.6 s wall) against
//! 18.5 CPU-s for one of the caps alone, the band before the split, and 0.002 CPU-s for the sum.
//! R06.T9.b's one run of the band before the split (2026-10-05, under the heavy-test lock, load
//! 3–5): 20.2 CPU-s on 15 workers, 1.37 s wall.
//!
//! The diffuse galactic light's benches (R06.T9.g): `sky/illumination` builds the near-Sun
//! request's illumination as a server does, its 1,536 rays one face row a job, then its scattered
//! field as one job, and prints each part's CPU time, the field's steps and the heap.
//! `band_near_sun/march` marches the request lit by it and `band_near_sun/march_no_dgl` the same
//! request unlit; the gate, provisional, is the illumination and the march's increase together at
//! most 10% of the unlit march, in one run. R06.T9.g's run (2026-10-07, criterion's medians of ten
//! samples each, three workers at `CPUQuota=400%`, without the heavy-test lock, which the
//! orchestrator's `just ci` held, load about 4–8, so provisional): the illumination 0.947 CPU-s
//! (its scattered field about 0.15 of it, 11 steps, 0.25 MB), the lit march 16.11 CPU-s and the
//! unlit 15.73: 1.33 CPU-s, 8.4% of the march. The lit march's heap is 22.95 MB, the unlit
//! 21.97 MB. The march alone moved by up to 2 CPU-s between single runs at load 7–17.
//!
//! `band_near_sun/march_camera` marches a camera's request at V 10.06 with the eye at 7.95 beside
//! it, so the march keeps the light fainter than the eye's cut too (R06.T9.j), and
//! `band_near_sun/march_camera_no_eye` the same request with no eye; the second sums cost the
//! difference. R06.T9.j's one run of each (2026-10-07, criterion's `--test`, three workers at
//! `CPUQuota=400%`, without the heavy-test lock, which another lane held, load about 10, so
//! provisional): at 10.06 the caps are A 46, B 260, C 19,416, D 19,416, E 41,804 and the brown
//! dwarfs 1 ly, with eight replies; with the eye's light the march holds 49.3 MB and took 29.5
//! CPU-s (9.9 s wall), and without it 24.7 MB and 22.5 CPU-s (7.5 s wall). The second sums double
//! the heap and add 31% to the time. In the same run `band_near_sun/march` took 20.0 CPU-s. A 16² band took 1.7 s on one thread in the test
//! profile, about 1.1 ms a ray, four fifths of it the luminosity functions' reads and one fifth the
//! ray's profile.
//!
//! The limit map's benches (R06.T9.i) set the eye's limits of all six faces of `BandSpec::STANDARD`
//! against a glare, one face row a job, as the server will after each reply's band (R06.T11.c).
//! Their band is the near-Sun band at V 8.15 with no census, complete everywhere. `limit_map/
//! near_sun` takes the glare of the stars a census lists there within 200 ly brighter than V 8.15,
//! R06.T9.i's fixture. `limit_map/synthetic_300k` takes the glare ruling's synthetic sky of 300,000
//! stars to V 10.06 (`MAX_N_MAX`, a camera's cut with the eye open), the same stars as the sim's
//! test of the pyramid. Their time is the glare's build, on one thread, plus the jobs' CPU time.
//! The gate is provisional (`decision-r06-t9c-glare.md`, item 2): at most 3 CPU-s at 300,000 stars
//! on the dev machine. The exact sum, every star over every texel, would take about 140 CPU-s at
//! R06.T9.c's 19 ns a pair; R06.T9.i measured 161 CPU-s on one thread (21.9 ns a pair). R06.T9.i's
//! one run (2026-10-07, without the heavy-test lock, which another lane's long run held, so at
//! `CPUQuota=400%`, 3 workers, load 7.7–9.5: provisional): `near_sun` 0.32 CPU-s, 0.11 s wall
//! (criterion 289 ms an iteration); `synthetic_300k` 1.03 CPU-s, 0.38 s wall (criterion 1.03 s).
//! By the process's CPU clock on one thread, in the slow-test profile, 300,000 stars took 1.45
//! CPU-s: 1.38 for the map and 0.07 for the glare's build. A locked re-timing is pending.

use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap};
use std::env::VarError;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, UnitVector};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::{
    BandMarch, BandSpec, BandTexel, CompleteTo, CubeFace, band_rows, march_rows, sum_rows,
};
use hyperion_sim::sky::caps::{
    CAPPED_LAYERS, CapCount, CapResolution, LayerCap, RADIAL_STEPS_PER_DECADE, layer_caps,
};
use hyperion_sim::sky::census::{
    BlockKey, BlockParams, CellOffsets, CellSlab, CensusCost, CensusTallies, HeldRecord, LayerCost,
    LayerTally, MAX_N_MAX, NoSkyCellCache, Rebuild, SkyBlock, SkyCellCache, SkyCensus, SkyContext,
    SkyQuery, SkyStar, StarBounds, census_cell, census_cell_with_cost, census_plan, census_plan_of,
    merge_census,
};
use hyperion_sim::sky::dgl::{ILLUMINATION_SPEC, Illumination, IlluminationRows};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::eye::{SpRatio, illuminance_of_magnitude};
use hyperion_sim::sky::limits::{Glare, eye_visibility, limit_rows};
use hyperion_sim::sky::luminosity::{BinSums, LuminosityTables};
use hyperion_sim::sky::phase::PhaseEnvelope;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::multiplicity::{HierarchyBound, hierarchy_bound};
use hyperion_sim::stellar::system::draw_metallicity;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::consts::RADIANS_PER_DEGREE;
use hyperion_sim::units::{LightYears, Lux, Magnitudes, Years};
use hyperion_testkit::golden::f64_digest;

/// The fixture's seed, the sim's sky tests' own.
const SEED: u64 = 0x0926_0000;

/// The eye's cut near the Sun, V: 7.4 + 0.45 + 0.1 (R06.T7, `decision-r06-t7-caps.md`), the cut
/// the caps' decision and the census's slow tests use, until R06.T9's `eye_cut` computes it.
const EYE_CUT_V: f64 = 7.95;

/// A camera's cut near the Sun beside the eye's, V (R06.T9.j): the glare ruling's 10.06, a camera
/// with the eye open, at which a census near the Sun lists `MAX_N_MAX` stars.
const CAMERA_CUT_V: f64 = 10.06;

/// Near the Sun, ly: the plan's Sun-like test place, 26,000 ly from the centre on +y (the
/// fixture's solar circle is 3.8 disc lengths, about 26,600 ly; R₀ = 8.178 kpc, GRAVITY
/// Collaboration 2019, A&A 625, L10) and 68 ly above the plane (z☉ = 20.8 ± 0.3 pc, Bennett and
/// Bovy 2019, MNRAS 482, 1417).
const SUN_LY: [f64; 3] = [0.0, 26_000.0, 68.0];

/// The warm jump's destination, ly: 1,000 ly from [`SUN_LY`] toward the galactic centre, the jump
/// drive's range (R06.T8.h).
const JUMP_LY: [f64; 3] = [0.0, 25_000.0, 68.0];

/// In the nuclear disc, ly: 150 ly from Sgr A* in the plane.
const NUCLEAR_DISC_LY: [f64; 3] = [0.0, 150.0, 0.0];

/// Noise-cache slots per job: 1 MiB of the sightlines' lattice words.
const NOISE_SLOTS: usize = 1 << 16;

/// The server's default cell-cache budget, MiB (R06.T11.b), when `HYPERION_SKY_CACHE_MB` is unset.
const DEFAULT_CACHE_MB: usize = 64;

/// Whether each census bench has printed its first census.
static PRINTED_COLD: AtomicBool = AtomicBool::new(false);
static PRINTED_COLD_SEEN: AtomicBool = AtomicBool::new(false);
static PRINTED_COLD_SPHERES: AtomicBool = AtomicBool::new(false);
static PRINTED_COLD_CAMERA: AtomicBool = AtomicBool::new(false);
static PRINTED_FILL: AtomicBool = AtomicBool::new(false);
static PRINTED_WARM: AtomicBool = AtomicBool::new(false);
static PRINTED_WARM_RATIO: AtomicBool = AtomicBool::new(false);
static PRINTED_JUMP_COLD: AtomicBool = AtomicBool::new(false);
static PRINTED_JUMP_FILL: AtomicBool = AtomicBool::new(false);
static PRINTED_JUMP: AtomicBool = AtomicBool::new(false);
static PRINTED_JUMP_RATIO: AtomicBool = AtomicBool::new(false);
static PRINTED_NUCLEAR: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND_SUM: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND_CAMERA: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND_CAMERA_NO_EYE: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND_NO_DGL: AtomicBool = AtomicBool::new(false);
static PRINTED_ILLUMINATION: AtomicBool = AtomicBool::new(false);
static PRINTED_LIMIT_MAP_NEAR_SUN: AtomicBool = AtomicBool::new(false);
static PRINTED_LIMIT_MAP_SYNTHETIC: AtomicBool = AtomicBool::new(false);

/// The limit map's near-Sun fixture (R06.T9.i): the eye's cut there, V 8.15 (R06.T9.d's figure
/// for the real sky, at which T9.c's to T9.i's tests are ruled), and the census within 200 ly.
const LIMIT_MAP_CUT_V: f64 = 8.15;
const LIMIT_MAP_RADIUS_LY: f64 = 200.0;

/// The synthetic sky's depth, V: a camera's cut with the eye open, at which the census lists its
/// largest number of stars, `MAX_N_MAX` (the glare ruling's sky at `N_max`).
const SYNTHETIC_CUT_V: f64 = 10.06;

/// The synthetic sky's fingerprint at `MAX_N_MAX` stars to `SYNTHETIC_CUT_V`: `f64_digest` of each
/// star's direction, illuminance and ratio, in order. The sim's test of the pyramid asserts the
/// same (`sky::limits`' `SYNTHETIC_SKY_DIGEST`), so this copy of its sky cannot part from it.
const SYNTHETIC_SKY_DIGEST: u64 = 0x8b8b_938a_6811_c91f;

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid")
}

/// The pool's width: the machine's threads less one, as the server leaves one to its runtime.
fn workers() -> usize {
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .saturating_sub(1)
        .max(1)
}

/// Runs `run` over `jobs` on `workers` threads, each taking the next job, and returns the
/// results with the summed time of the jobs.
fn on_pool<J: Sync, R: Send>(
    jobs: &[J],
    workers: usize,
    run: &(impl Fn(&J) -> R + Sync),
) -> (Vec<(usize, R)>, Duration) {
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut mine = Vec::new();
        let mut busy = Duration::ZERO;
        loop {
            let k = next.fetch_add(1, Ordering::Relaxed);
            let Some(job) = jobs.get(k) else {
                return (mine, busy);
            };
            let start = Instant::now();
            mine.push((k, run(job)));
            busy += start.elapsed();
        }
    };
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers).map(|_| scope.spawn(worker)).collect();
        let mut all = Vec::with_capacity(jobs.len());
        let mut busy = Duration::ZERO;
        for h in handles {
            let (mine, b) = h.join().expect("a pool job does not panic");
            all.extend(mine);
            busy += b;
        }
        all.sort_unstable_by_key(|(k, _)| *k);
        (all, busy)
    })
}

/// The fixture's galaxy, its full luminosity tables (built once, on the pool), the envelope and
/// the cells' offset bounds.
struct Sky {
    galaxy: Galaxy,
    tables: LuminosityTables,
    envelope: BrightnessEnvelope,
    offsets: CellOffsets,
}

fn sky() -> &'static Sky {
    static SKY: OnceLock<Sky> = OnceLock::new();
    SKY.get_or_init(|| {
        let galaxy = galaxy();
        let plan = LuminosityTables::plan(&galaxy);
        let workers = workers();
        // Stage by stage, one metallicity's samples held at a time; each bin's sums behind a lock
        // of its own, since a stage's accumulation jobs are of different bins.
        let sums: Vec<Mutex<BinSums>> = plan.bin_sums().into_iter().map(Mutex::new).collect();
        for stage in plan.stages() {
            let sample_jobs: Vec<_> = plan.sample_jobs(stage).collect();
            let (chunks, _) = on_pool(&sample_jobs, workers, &|job| plan.run_samples(job.clone()));
            let samples = plan.track_samples(chunks.into_iter().map(|(_, c)| c));
            let accumulate_jobs: Vec<_> = plan.accumulate_jobs(stage).collect();
            on_pool(&accumulate_jobs, workers, &|job| {
                let mut bin = sums[job.bin()]
                    .lock()
                    .expect("an accumulation job does not panic");
                plan.run_accumulate(&samples, *job, &mut bin);
            });
        }
        let tables = plan.assemble(sums.into_iter().map(|bin| {
            bin.into_inner()
                .expect("an accumulation job does not panic")
        }));
        let envelope = BrightnessEnvelope::build(&galaxy);
        let offsets = CellOffsets::build(&galaxy);
        Sky {
            galaxy,
            tables,
            envelope,
            offsets,
        }
    })
}

/// The census benches' sample, `HYPERION_SKY_BENCH_SAMPLE` (1, every cell, when unset).
fn sample() -> u64 {
    match std::env::var("HYPERION_SKY_BENCH_SAMPLE") {
        Err(VarError::NotPresent) => 1,
        Err(VarError::NotUnicode(_)) => panic!("HYPERION_SKY_BENCH_SAMPLE is not Unicode"),
        Ok(v) => v
            .parse::<u64>()
            .ok()
            .filter(|&k| k > 0)
            .unwrap_or_else(|| panic!("HYPERION_SKY_BENCH_SAMPLE={v} is not a positive count")),
    }
}

/// One `SplitMix64` step from the state `word`: the golden gamma, then Stafford's Mix13 (Stafford
/// 2011, "Better bit mixing"; Steele, Lea and Flood 2014's `mix64variant13`, doi:10.1145/2660193.
/// 2660195, and JDK 8's `SplittableRandom.mix64`), as Vigna's 2015 `splitmix64.c` gives it: a fixed
/// mixer of a word.
fn mix(word: u64) -> u64 {
    let mut z = word.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The hash a sampled bench takes `key` by: [`mix`] of its layer, then of each of its block's
/// coordinates in turn ([`BlockKey`]), so that a sample is spread over the sky, the same on every
/// run, and takes whole blocks of the cell cache, whose bytes a sample then scales (R06.T8.h).
fn sample_hash(key: CellKey) -> u64 {
    let block = BlockKey::of(key);
    block
        .coords()
        .iter()
        .fold(mix(u64::from(block.layer().value())), |h, &c| {
            mix(h ^ u64::from(c.cast_unsigned()))
        })
}

fn eye_query(ly: [f64; 3]) -> SkyQuery {
    let at = GalacticPosition::from_light_years(ly).expect("in the root cube");
    let observer = Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock");
    SkyQuery::builder(observer, Magnitudes::new(EYE_CUT_V))
        .eye(EyeObserver::default())
        .build()
        .expect("a valid query")
}

/// [`eye_query`] lit by the observer's illumination ([`sun_illumination`] near the Sun), so that
/// its band holds the diffuse galactic light (R06.T9.g).
fn lit_eye_query(ly: [f64; 3], illumination: &Arc<Illumination>) -> SkyQuery {
    let at = GalacticPosition::from_light_years(ly).expect("in the root cube");
    let observer = Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock");
    SkyQuery::builder(observer, Magnitudes::new(EYE_CUT_V))
        .eye(EyeObserver::default())
        .illumination(Arc::clone(illumination))
        .build()
        .expect("a valid query")
}

/// The illumination of `observer` as a server builds it (R06.T9.g, R06.T11.a): its rays one face
/// row a job on `workers` threads, then assembled, with its scattered field, as one job. Gives it,
/// the rays' CPU time and the assembly's.
fn march_illumination(observer: &Observer, workers: usize) -> (Illumination, Duration, Duration) {
    let sky = sky();
    let side = ILLUMINATION_SPEC.face_texels();
    let jobs: Vec<(CubeFace, u16)> = CubeFace::ALL
        .iter()
        .flat_map(|&face| (0..side).map(move |row| (face, row)))
        .collect();
    let (parts, busy) = on_pool(&jobs, workers, &|&(face, row): &(CubeFace, u16)| {
        Illumination::march_rows(
            &sky.galaxy,
            &mut job_context(sky),
            observer,
            face,
            row..row + 1,
        )
    });
    let began = Instant::now();
    let parts: Vec<IlluminationRows> = parts.into_iter().map(|(_, part)| part).collect();
    let light = Illumination::assemble(observer, black_box(parts));
    (light, busy, began.elapsed())
}

/// The near-Sun request's illumination, marched once, outside every timing but its own bench's.
fn sun_illumination() -> &'static Arc<Illumination> {
    static LIGHT: OnceLock<Arc<Illumination>> = OnceLock::new();
    LIGHT.get_or_init(|| {
        let observer = *eye_query(SUN_LY).observer();
        Arc::new(march_illumination(&observer, workers()).0)
    })
}

/// The near-Sun request's illumination (R06.T9.g): its 1,536 rays one face row a job, then its
/// scattered field as one job, as the server builds it once a request before the eye's cut.
/// Prints, once, the rays' CPU time, the fixed point's, its steps, its heap and the wall time.
fn illumination(c: &mut Criterion) {
    let observer = *eye_query(SUN_LY).observer();
    let workers = workers();
    // The tables, built outside the timing.
    let _ = sky();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("illumination", |b| {
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let began = Instant::now();
                let (light, rays, field) = march_illumination(&observer, workers);
                if !PRINTED_ILLUMINATION.swap(true, Ordering::Relaxed) {
                    println!(
                        "sky/illumination: {} rays at {}² a face, {:.2} CPU-s on {workers} \
                         workers; the scattered field in {} steps, {:.3} CPU-s; heap {} bytes; \
                         {:.2} s wall",
                        CubeFace::ALL.len() * usize::from(ILLUMINATION_SPEC.face_texels()).pow(2),
                        ILLUMINATION_SPEC.face_texels(),
                        rays.as_secs_f64(),
                        light.iterations(),
                        field.as_secs_f64(),
                        light.heap_bytes(),
                        began.elapsed().as_secs_f64()
                    );
                }
                cpu += rays + field;
            }
            cpu
        });
    });
    group.finish();
}

/// One census's result and costs.
struct Run {
    census: SkyCensus,
    /// Its cost, summed over the jobs: the records past the floor, held, pre-filtered and bounded
    /// star by star, the systems generated that a realised bound could not skip, and the cells
    /// served and rebuilt (R06.T8.h).
    cost: CensusCost,
    /// Each capped layer's cap, ly.
    caps: Vec<LightYears>,
    /// The plan's cells, every one, sampled or not.
    cells: u64,
    /// The sample: one block of cells in this many is censused.
    sample: u64,
    plan: Duration,
    /// The jobs' time walking their slabs' cells, the sampled cells' census aside.
    walk: Duration,
    /// The jobs' time censusing the sampled cells.
    cell_census: Duration,
    wall: Duration,
}

impl Run {
    /// The census's CPU time: the plan's and every job's, with the sampled cells' census scaled
    /// by the sample, so an estimate when the census is sampled.
    fn cpu(&self) -> Duration {
        let scale = u32::try_from(self.sample).expect("a sample of under 2³² cells");
        self.plan + self.walk + self.cell_census * scale
    }
}

/// The census of `query` through `cells`, as the server will run it, taking one block of cells in
/// `sample`.
fn census(
    sky: &Sky,
    query: &SkyQuery,
    cells: &dyn SkyCellCache,
    workers: usize,
    sample: u64,
) -> Run {
    let began = Instant::now();
    let mut caps_noise = NoiseCache::with_capacity(NOISE_SLOTS);
    let plan = census_plan(
        &sky.galaxy,
        &sky.tables,
        &sky.envelope,
        query,
        &mut caps_noise,
    );
    let planned = began.elapsed();
    let slabs: Vec<CellSlab> = plan.slabs().collect();
    let (parts, jobs) = on_pool(&slabs, workers, &|slab: &CellSlab| {
        let mut ctx = SkyContext {
            tables: &sky.tables,
            envelope: &sky.envelope,
            offsets: &sky.offsets,
            noise: NoiseCache::with_capacity(NOISE_SLOTS),
            cells,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let mut stars = Vec::new();
        let (mut tallies, mut cost) = (CensusTallies::default(), CensusCost::default());
        let mut censusing = Duration::ZERO;
        for key in slab.cells() {
            if sample > 1 && !sample_hash(key).is_multiple_of(sample) {
                continue;
            }
            let opened = Instant::now();
            let (t, c) = census_cell_with_cost(&sky.galaxy, &mut ctx, key, query, &mut stars);
            tallies.add(&t);
            cost.add(&c);
            censusing += opened.elapsed();
        }
        ((stars, tallies), cost, censusing)
    });
    let cell_census: Duration = parts.iter().map(|(_, (_, _, c))| *c).sum();
    let mut cost = CensusCost::default();
    for (_, (_, c, _)) in &parts {
        cost.add(c);
    }
    let census = merge_census(parts.into_iter().map(|(_, (p, _, _))| p), query.n_max());
    Run {
        census,
        cost,
        caps: plan.caps().iter().map(LayerCap::radius).collect(),
        cells: plan.cell_count(),
        sample,
        plan: planned,
        walk: jobs.saturating_sub(cell_census),
        cell_census,
        wall: began.elapsed(),
    }
}

/// `n` as a share of `of`, percent, for printing; 0 when `of` is.
#[expect(
    clippy::cast_precision_loss,
    reason = "a share of counts under 2⁵³, for printing"
)]
fn percent(n: u64, of: u64) -> f64 {
    if of == 0 {
        0.0
    } else {
        100.0 * n as f64 / of as f64
    }
}

/// Prints `run`'s tallies and costs the first time `printed` is unset, and the cache's blocks if
/// it was read through one.
fn report_once(printed: &AtomicBool, name: &str, run: &Run, cache: Option<&BenchCellCache<'_>>) {
    if printed.swap(true, Ordering::Relaxed) {
        return;
    }
    let sample = run.sample;
    // A sampled census's counts and time, scaled by the sample: estimates, and labelled so.
    let (what, scale) = if sample > 1 {
        (
            format!("ESTIMATE from 1 block of cells in {sample}, scaled by {sample}"),
            sample,
        )
    } else {
        ("every cell".to_owned(), 1)
    };
    eprintln!(
        "{name} ({what}): {} cells in the plan, plan {:.1} s, walk {:.1} s, cell census {:.1} s \
         sampled, CPU {:.1} s, wall {:.1} s on {} workers; {} listed, {} overflow (sampled)",
        run.cells,
        run.plan.as_secs_f64(),
        run.walk.as_secs_f64(),
        run.cell_census.as_secs_f64(),
        run.cpu().as_secs_f64(),
        run.wall.as_secs_f64(),
        workers(),
        run.census.listed().len(),
        run.census.overflow().len(),
    );
    let mut outcomes = [0_u64; 5];
    for (cap, &layer) in run.caps.iter().zip(&CAPPED_LAYERS) {
        let cost = run.cost.layer(layer);
        report_layer(layer, *cap, run.census.tallies().layer(layer), cost, scale);
        for (sum, n) in outcomes.iter_mut().zip([
            cost.served(),
            cost.missed(),
            cost.rebuilt(Rebuild::Key),
            cost.rebuilt(Rebuild::Window),
            cost.rebuilt(Rebuild::Parameters),
        ]) {
            *sum += n;
        }
    }
    let [served, missed, key, window, parameters] = outcomes;
    eprintln!(
        "  cells: {served} served, {missed} missed, rebuilt {key} for the key, {window} for the \
         window, {parameters} for the parameters (sampled)",
    );
    if let Some(cache) = cache {
        report_cache(cache, scale);
    }
}

/// Prints one layer's tallies and cost, scaled by `scale`.
fn report_layer(layer: Layer, cap: LightYears, tally: &LayerTally, cost: &LayerCost, scale: u64) {
    eprintln!(
        "  {layer:?}: cap {:.0} ly; {} cells, {} records past the floor, {} generated ({:.2}% of \
         the records; {} of them with a star past the cut unextinguished), {} accepted, {} listed \
         (sampled)",
        cap.value(),
        tally.cells() * scale,
        cost.candidates() * scale,
        tally.generated() * scale,
        percent(tally.generated(), cost.candidates()),
        cost.generated_listable() * scale,
        tally.accepted() * scale,
        tally.listed(),
    );
    // R06.T8.g's bound star by star: the records it bounded, those holding a pair plan 11
    // cannot bound, and their pairs by plan 11's verdict, each share of the pairs counted.
    let pairs = cost.pairs();
    let share = |n: u64| percent(n, pairs.total());
    eprintln!(
        "    star by star: {} records bounded, {} unbounded; {} pairs: detached {:.2}%, \
         unchanged {:.2}%, remnants {:.2}%, bright {:.2}%, none {:.2}%",
        cost.star_bounded() * scale,
        cost.unbounded_records() * scale,
        pairs.total() * scale,
        share(pairs.detached()),
        share(pairs.unchanged()),
        share(pairs.remnants()),
        share(pairs.bright()),
        share(pairs.unbounded()),
    );
    // R06.T8.h's cache: the records its entries hold of the cells read or built, and those the
    // pre-filter skipped.
    eprintln!(
        "    cache: {} records held, {} pre-filtered; cells {} served, {} missed, rebuilt {} for \
         the key, {} for the window, {} for the parameters",
        cost.held() * scale,
        cost.prefiltered() * scale,
        cost.served() * scale,
        cost.missed() * scale,
        cost.rebuilt(Rebuild::Key) * scale,
        cost.rebuilt(Rebuild::Window) * scale,
        cost.rebuilt(Rebuild::Parameters) * scale,
    );
}

/// Prints the cache's blocks, cells built, records held and bytes, the bytes also scaled by
/// `scale`, the sky's estimate.
fn report_cache(cache: &BenchCellCache<'_>, scale: u64) {
    let kept = cache.lock();
    let built: u64 = kept
        .blocks
        .values()
        .map(|e| u64::from(e.block.built_cells()))
        .sum();
    let records: usize = kept.blocks.values().map(|e| e.block.held().len()).sum();
    let bytes = u64::try_from(kept.bytes).unwrap_or(u64::MAX);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a mean of counts under 2⁵³, for printing"
    )]
    let per_cell = if built == 0 {
        0.0
    } else {
        (bytes as f64 - (size_of::<HeldRecord>() * records) as f64) / built as f64
    };
    eprintln!(
        "  cache: {} blocks, {built} cells built, {records} records held; {bytes} of {} bytes \
         (the sky's estimate {} bytes, {per_cell:.1} bytes a cell beyond its records); {} blocks \
         evicted, {} refused (sampled)",
        kept.blocks.len(),
        cache.max_bytes,
        bytes * scale,
        kept.evicted,
        kept.refused,
    );
}

/// Prints, the first time `printed` is unset, `warm`'s CPU time against `cold`'s: R06.T17's warm
/// ratio, whose target is at most 25% (a target since R06.T8.h, `decision-r06-t8h-warm.md`).
fn report_ratio(printed: &AtomicBool, name: &str, warm: &Run, cold: &Run) {
    if printed.swap(true, Ordering::Relaxed) {
        return;
    }
    let (w, c) = (warm.cpu().as_secs_f64(), cold.cpu().as_secs_f64());
    let generated = |run: &Run| -> u64 {
        CAPPED_LAYERS
            .iter()
            .map(|&l| run.census.tallies().layer(l).generated())
            .sum()
    };
    eprintln!(
        "{name}: warm {w:.1} CPU-s against cold {c:.1} CPU-s: {:.1}% (target at most 25%); \
         generated {} warm, {} cold; the same reply: {}",
        100.0 * w / c,
        generated(warm),
        generated(cold),
        warm.census == cold.census,
    );
}

/// One block's entry in [`BenchCellCache`]: the block shared, so that a lookup reads it outside
/// the lock.
#[derive(Debug)]
struct Entry {
    block: Arc<SkyBlock>,
    bytes: usize,
    used: u64,
}

/// What a block weighs beyond its held records: its key twice (in both maps), its entry, itself,
/// its place in the recency order and its reference counts.
const ENTRY_OVERHEAD_BYTES: usize =
    2 * size_of::<BlockKey>() + size_of::<Entry>() + size_of::<SkyBlock>() + 3 * size_of::<u64>();

#[derive(Debug, Default)]
struct Kept {
    blocks: HashMap<BlockKey, Entry>,
    by_use: BTreeMap<u64, BlockKey>,
    bytes: usize,
    clock: u64,
    evicted: u64,
    refused: u64,
}

/// The benches' cell cache, built on `SkyBlock::with_cell` as the server's is (R06.T11.b, T8.h):
/// one galaxy's blocks behind a lock, the least recently used out first, at most `max_bytes`
/// bytes held, a block weighing its held records plus [`ENTRY_OVERHEAD_BYTES`] (the maps' nodes
/// are not counted). A block is read outside the lock, and merged under it.
///
/// Its budget is `HYPERION_SKY_CACHE_MB` divided by the bench's sample, so that a sampled warm
/// census is the shipped size's (`decision-r06-census-cost-signoff.md`, condition 4).
#[derive(Debug)]
struct BenchCellCache<'g> {
    galaxy: &'g Galaxy,
    max_bytes: usize,
    kept: Mutex<Kept>,
}

impl<'g> BenchCellCache<'g> {
    /// An empty cache for `galaxy` under `HYPERION_SKY_CACHE_MB`, divided by `sample`.
    fn new(galaxy: &'g Galaxy, sample: u64) -> Self {
        let mb = match std::env::var("HYPERION_SKY_CACHE_MB") {
            Err(VarError::NotPresent) => DEFAULT_CACHE_MB,
            Err(VarError::NotUnicode(_)) => panic!("HYPERION_SKY_CACHE_MB is not Unicode"),
            Ok(v) => v.parse::<usize>().unwrap_or_else(|e| {
                panic!("HYPERION_SKY_CACHE_MB={v} is not a whole number of MiB: {e}")
            }),
        };
        let sample = usize::try_from(sample).expect("a sample of under 2⁶⁴ blocks");
        Self {
            galaxy,
            max_bytes: mb.saturating_mul(1 << 20) / sample,
            kept: Mutex::new(Kept::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("a cache user does not panic")
    }
}

impl SkyCellCache for BenchCellCache<'_> {
    fn keeps_entries(&self) -> bool {
        true
    }

    fn block(&self, galaxy: &Galaxy, key: BlockKey) -> Option<Arc<SkyBlock>> {
        assert!(
            std::ptr::eq(galaxy, self.galaxy),
            "a cell cache is for the galaxy it was made for"
        );
        let mut guard = self.lock();
        let kept = &mut *guard;
        kept.clock += 1;
        let clock = kept.clock;
        let entry = kept.blocks.get_mut(&key)?;
        kept.by_use.remove(&entry.used);
        entry.used = clock;
        kept.by_use.insert(clock, key);
        Some(Arc::clone(&entry.block))
    }

    fn keep(&self, galaxy: &Galaxy, cell: CellKey, params: &BlockParams, records: &[HeldRecord]) {
        assert!(std::ptr::eq(galaxy, self.galaxy));
        let key = BlockKey::of(cell);
        let mut guard = self.lock();
        let kept = &mut *guard;
        let old = kept.blocks.get(&key).map(|e| Arc::clone(&e.block));
        let Some(block) = SkyBlock::with_cell(old.as_deref(), cell, params, records) else {
            return;
        };
        let bytes = block.heap_bytes().saturating_add(ENTRY_OVERHEAD_BYTES);
        if bytes > self.max_bytes {
            kept.refused += 1;
            return;
        }
        kept.clock += 1;
        let used = kept.clock;
        if let Some(old) = kept.blocks.insert(
            key,
            Entry {
                block: Arc::new(block),
                bytes,
                used,
            },
        ) {
            kept.bytes -= old.bytes;
            kept.by_use.remove(&old.used);
        }
        kept.by_use.insert(used, key);
        kept.bytes += bytes;
        while kept.bytes > self.max_bytes {
            let (_, oldest) = kept
                .by_use
                .pop_first()
                .expect("over the bound, so not empty");
            let gone = kept.blocks.remove(&oldest).expect("in both maps");
            kept.bytes -= gone.bytes;
            kept.evicted += 1;
        }
    }
}

/// R06.T8.g's path near the Sun, with no cache: the cold figure it records.
///
/// It runs on R06.T7.b's caps by ray at the uniform cut, on its caps by the eye's visibility,
/// which the server asks of an eye-only request, and on R06.T7's spheres, at which the gate was
/// first set (`decision-p11-t17c-bright.md`). The warm benches' fills ([`census_near_sun`]) are
/// the cold census through an empty cache, which also builds each entry (R06.T8.h).
fn census_near_sun_cold(c: &mut Criterion) {
    // The tables and each plan's caps are made only when a bench that reads them is run, so that
    // a filtered run builds nothing it does not time.
    let query = eye_query(SUN_LY);
    let workers = workers();
    let sample = sample();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    let cold = [
        ("census_near_sun/cold", &PRINTED_COLD, Caps::ByRay),
        (
            "census_near_sun/cold_eye_visibility",
            &PRINTED_COLD_SEEN,
            Caps::ByEyeVisibility,
        ),
        (
            "census_near_sun/cold_spheres",
            &PRINTED_COLD_SPHERES,
            Caps::Spheres,
        ),
    ];
    for (name, printed, caps) in cold {
        group.bench_function(name, |b| {
            let sky = sky();
            let query = caps.of(sky, query.clone());
            b.iter_custom(|iters| {
                let mut cpu = Duration::ZERO;
                for _ in 0..iters {
                    let run = census(sky, black_box(&query), &NoSkyCellCache, workers, sample);
                    report_once(printed, name, &run, None);
                    cpu += run.cpu();
                }
                cpu
            });
        });
    }
    group.finish();
}

fn census_near_sun(c: &mut Criterion) {
    // The tables and the warm cache are made only when a bench that reads them is run, so that a
    // filtered run builds nothing it does not time.
    let query = eye_query(SUN_LY);
    let workers = workers();
    let sample = sample();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    // The camera's cut beside the eye's (R06.T8.g's record, `decision-r06-census-cost-signoff.md`):
    // a camera at V 10.06 with the eye asked at 7.95.
    let camera = camera_query(SUN_LY, true);
    group.bench_function("census_near_sun/cold_camera", |b| {
        let sky = sky();
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let cache = BenchCellCache::new(&sky.galaxy, sample);
                let run = census(sky, black_box(&camera), &cache, workers, sample);
                report_once(
                    &PRINTED_COLD_CAMERA,
                    "census_near_sun/cold_camera",
                    &run,
                    Some(&cache),
                );
                cpu += run.cpu();
            }
            cpu
        });
    });
    let warm: OnceCell<(BenchCellCache<'static>, Run)> = OnceCell::new();
    group.bench_function("census_near_sun/warm", |b| {
        let sky = sky();
        // The warm cache is what one cold census of the same query leaves; that census is the
        // ratio's cold.
        let (warm, fill) = warm.get_or_init(|| {
            let cache = BenchCellCache::new(&sky.galaxy, sample);
            let fill = census(sky, &query, &cache, workers, sample);
            report_once(&PRINTED_FILL, "census_near_sun/fill", &fill, Some(&cache));
            (cache, fill)
        });
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let run = census(sky, black_box(&query), warm, workers, sample);
                report_once(&PRINTED_WARM, "census_near_sun/warm", &run, Some(warm));
                report_ratio(&PRINTED_WARM_RATIO, "census_near_sun/warm", &run, fill);
                cpu += run.cpu();
            }
            cpu
        });
    });
    // R06.T8.h's jump: the Sun's census fills the cache, then a census 1,000 ly toward the
    // galactic centre at the same time and cut reads it. Its reference is the destination's cold
    // census, with an empty cache.
    let destination = eye_query(JUMP_LY);
    let jump_cold: OnceCell<Run> = OnceCell::new();
    group.bench_function("census_near_sun/warm_jump", |b| {
        let sky = sky();
        let cold = jump_cold.get_or_init(|| {
            let cache = BenchCellCache::new(&sky.galaxy, sample);
            let cold = census(sky, &destination, &cache, workers, sample);
            report_once(
                &PRINTED_JUMP_COLD,
                "census_near_sun/warm_jump (the destination, cold)",
                &cold,
                Some(&cache),
            );
            cold
        });
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let cache = BenchCellCache::new(&sky.galaxy, sample);
                let fill = census(sky, &query, &cache, workers, sample);
                report_once(
                    &PRINTED_JUMP_FILL,
                    "census_near_sun/warm_jump (the Sun's fill)",
                    &fill,
                    Some(&cache),
                );
                let run = census(sky, black_box(&destination), &cache, workers, sample);
                report_once(
                    &PRINTED_JUMP,
                    "census_near_sun/warm_jump",
                    &run,
                    Some(&cache),
                );
                report_ratio(&PRINTED_JUMP_RATIO, "census_near_sun/warm_jump", &run, cold);
                cpu += run.cpu();
            }
            cpu
        });
    });
    group.finish();
}

fn census_nuclear_disc(c: &mut Criterion) {
    let query = eye_query(NUCLEAR_DISC_LY);
    let workers = workers();
    let sample = sample();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("census_nuclear_disc", |b| {
        let sky = sky();
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let run = census(sky, black_box(&query), &NoSkyCellCache, workers, sample);
                report_once(&PRINTED_NUCLEAR, "census_nuclear_disc", &run, None);
                cpu += run.cpu();
            }
            cpu
        });
    });
    group.finish();
}

/// The first `n` records of `layer` in the cells nearest the Sun, shell by shell of cells about
/// the cell holding it: a neighbourhood's systems as placement makes them (plan 11's P11.T16
/// bench's sample).
fn records_nearest_sun(galaxy: &Galaxy, layer: Layer, n: usize) -> Vec<SystemRecord> {
    let sun = GalacticPosition::from_light_years(SUN_LY).expect("in the root cube");
    let centre = CellKey::containing(layer, &sun)
        .expect("the Sun is in the root cube")
        .gen_cell()
        .to_array();
    let mut records = Vec::with_capacity(n);
    let mut cell = Vec::new();
    for r in 0_i32..40 {
        for i in -r..=r {
            for j in -r..=r {
                for k in -r..=r {
                    if i.abs().max(j.abs()).max(k.abs()) != r {
                        continue;
                    }
                    let Ok(key) =
                        CellKey::new(layer, [centre[0] + i, centre[1] + j, centre[2] + k])
                    else {
                        continue;
                    };
                    generate_cell(galaxy, key, &mut cell);
                    records.extend(cell.iter().take(n - records.len()));
                    if records.len() == n {
                        return records;
                    }
                }
            }
        }
    }
    records
}

/// One layer's records for [`star_bound`], with their compositions, hierarchy bounds and star
/// bounds over [`star_bound_window`].
struct StarBoundFixture {
    records: Vec<SystemRecord>,
    parts: Vec<(SystemRecord, Composition, HierarchyBound)>,
    bounds: Vec<StarBounds>,
}

/// The pairs' window of [`star_bound`]: the 300 years before `record`'s epoch age.
fn star_bound_window(record: &SystemRecord) -> (Years, Years) {
    let age = record.age_at(UniverseTime::EPOCH);
    (Years::new(age.value() - 300.0), age)
}

/// The fixture of [`star_bound`] for `layer`, made on first use and printed: a filtered run
/// builds nothing it does not time.
fn star_bound_fixture(galaxy: &Galaxy, layer: Layer) -> StarBoundFixture {
    let records = records_nearest_sun(galaxy, layer, 400);
    let parts: Vec<(SystemRecord, Composition, HierarchyBound)> = records
        .iter()
        .map(|r| {
            let composition = draw_metallicity(galaxy, r);
            let hierarchy = hierarchy_bound(galaxy, r, &composition);
            (*r, composition, hierarchy)
        })
        .collect();
    let bounds: Vec<StarBounds> = parts
        .iter()
        .map(|(r, composition, hierarchy)| {
            StarBounds::from_hierarchy(galaxy, r, *composition, hierarchy, star_bound_window(r))
        })
        .collect();
    let listed: usize = bounds.iter().map(|b| 1 + b.companions().len()).sum();
    let pairs: u64 = bounds.iter().map(|b| b.pairs().total()).sum();
    let unbounded = bounds.iter().filter(|b| b.pairs().unbounded() > 0).count();
    eprintln!(
        "sky/star_bound {layer:?}, {} records: {listed} stars bounded, {pairs} pairs, {unbounded} \
         records unbounded",
        records.len()
    );
    StarBoundFixture {
        records,
        parts,
        bounds,
    }
}

/// R06.T8.g's bound star by star, step by step, per stellar layer near the Sun: the composition
/// (`draw_metallicity`), plan 11's `hierarchy_bound`, the pairs' verdicts and η draws
/// (`StarBounds::from_hierarchy`), and the phase envelope's reads (`StarBounds::brightest`), each
/// over the same 400 records; then the whole. The pairs are bounded over the 300 years before
/// each record's epoch age, about a light-time window far out, and the stars read at that age.
/// The target is about 6 µs a record for every step but `hierarchy_bound`, whose own cost is
/// P11.T16's (`decision-p11-t16-hierarchy-bound.md` §7).
fn star_bound(c: &mut Criterion) {
    let galaxy = galaxy();
    let phase = PhaseEnvelope::shared();
    let mut group = c.benchmark_group("sky/star_bound");
    group.sample_size(10);
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        let fixture: OnceCell<StarBoundFixture> = OnceCell::new();
        let get = || fixture.get_or_init(|| star_bound_fixture(&galaxy, layer));
        let name = format!("{layer:?}, 400 records");
        group.bench_function(format!("draw_metallicity ({name})"), |b| {
            let f = get();
            b.iter(|| {
                for r in &f.records {
                    black_box(draw_metallicity(&galaxy, black_box(r)));
                }
            });
        });
        group.bench_function(format!("hierarchy_bound ({name})"), |b| {
            let f = get();
            b.iter(|| {
                for (r, composition, _) in &f.parts {
                    black_box(hierarchy_bound(&galaxy, black_box(r), composition));
                }
            });
        });
        group.bench_function(format!("pairs and eta ({name})"), |b| {
            let f = get();
            b.iter(|| {
                for (r, composition, hierarchy) in &f.parts {
                    black_box(StarBounds::from_hierarchy(
                        &galaxy,
                        black_box(r),
                        *composition,
                        hierarchy,
                        star_bound_window(r),
                    ));
                }
            });
        });
        group.bench_function(format!("phase reads ({name})"), |b| {
            let f = get();
            b.iter(|| {
                for (r, bound) in f.records.iter().zip(&f.bounds) {
                    let age = r.age_at(UniverseTime::EPOCH);
                    black_box(bound.brightest(phase, black_box((age, age))));
                }
            });
        });
        group.bench_function(format!("the whole ({name})"), |b| {
            let f = get();
            b.iter(|| {
                for r in &f.records {
                    let age = r.age_at(UniverseTime::EPOCH);
                    let bound = StarBounds::of(&galaxy, black_box(r), star_bound_window(r));
                    black_box(bound.brightest(phase, (age, age)));
                }
            });
        });
    }
    group.finish();
}

/// R06.T7.b's record near the Sun at V 7.95, the eye's cut of R06.T7's tests: the stars R06.T7's
/// spherical caps (768 rays, one profile a ray) list that the caps by ray drop, and those the rays
/// gain, realised, beside their expected counts (`decision-r06-census-cost-signoff.md`, question
/// 3's condition 3; no gate), for the caps by ray at the uniform cut and by the eye's visibility at
/// the same cut.
///
/// It censuses the sampled cells of the three plans' union once (`HYPERION_SKY_BENCH_SAMPLE`; every
/// cell when unset) and sorts each star by the plans whose cells hold it. A sampled run's realised
/// counts are those of its sample times the sample: estimates, labelled so, which R06.T17 re-takes
/// on the final census. Its time is the union's census.
fn caps_by_ray_near_sun(c: &mut Criterion) {
    let workers = workers();
    let sample = sample();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("caps_by_ray_near_sun", |b| {
        let sky = sky();
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                cpu += caps_record(sky, workers, sample);
            }
            cpu
        });
    });
    group.finish();
}

/// The caps a cold census near the Sun is planned on (R06.T8.g's record).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Caps {
    /// R06.T7.b's caps by ray at the uniform cut: the query's own.
    ByRay,
    /// R06.T7.b's caps by the eye's visibility ([`by_eye_visibility`]).
    ByEyeVisibility,
    /// R06.T7's spheres ([`with_spheres`]).
    Spheres,
}

impl Caps {
    /// `query` planned on these caps.
    ///
    /// The caps are counted here, outside the census's time.
    #[must_use]
    fn of(self, sky: &Sky, query: SkyQuery) -> SkyQuery {
        match self {
            Self::ByRay => query,
            Self::ByEyeVisibility => by_eye_visibility(sky, &query),
            Self::Spheres => {
                with_spheres(sky, query, &mut NoiseCache::with_capacity(NOISE_SLOTS)).0
            }
        }
    }
}

/// `query` with R06.T7's spherical caps forced per layer, and those caps.
///
/// They are 768 rays, one profile a ray, counted at `query`'s cut (`CapCount::spheres` at the
/// standard resolution are not these): the caps R06.T8.g's cold gate was first set at, before
/// R06.T7.b.
#[must_use]
fn with_spheres(sky: &Sky, query: SkyQuery, noise: &mut NoiseCache) -> (SkyQuery, Vec<LayerCap>) {
    let spheres = CapCount::measure(
        &sky.galaxy,
        &sky.tables,
        &sky.envelope,
        query.observer(),
        query.cut(),
        CapResolution::new(768, RADIAL_STEPS_PER_DECADE).expect("non-zero"),
        noise,
    )
    .spheres();
    let radii: Vec<(Layer, LightYears)> = spheres.iter().map(|c| (c.layer(), c.radius())).collect();
    let forced = query
        .with_caps_forced_per_layer(&radii)
        .expect("the spheres are caps");
    (forced, spheres)
}

/// The eye-only request of `query`'s observer at its cut, with R06.T7.b's caps by the eye's
/// visibility at that cut.
///
/// The server asks these caps of an eye-only request. Here they take no illumination, as
/// R06.T7.b measured them; the server passes the request's.
#[must_use]
fn by_eye_visibility(sky: &Sky, query: &SkyQuery) -> SkyQuery {
    let eye = EyeObserver::default();
    let observer = *query.observer();
    let cut = query.cut();
    let mut ctx = job_context(sky);
    let visibility = eye_visibility(&sky.galaxy, &mut ctx, &observer, &eye, cut, None);
    SkyQuery::builder(observer, cut)
        .eye(eye)
        .eye_visibility(visibility)
        .build()
        .expect("an eye-only request at its cut")
}

/// A star of [`caps_by_ray_near_sun`]'s union, with whether each plan (the spheres, by ray, by
/// visibility) opens its cell.
type Sorted = (SkyStar, [bool; 3]);

/// One run of [`caps_by_ray_near_sun`]: prints its record the first time, and returns the union's
/// census time, scaled by the sample.
fn caps_record(sky: &Sky, workers: usize, sample: u64) -> Duration {
    static PRINTED: AtomicBool = AtomicBool::new(false);
    let query = eye_query(SUN_LY);
    let observer = *query.observer();
    let cut = query.cut();
    let seen_query = by_eye_visibility(sky, &query);
    let mut noise = NoiseCache::with_capacity(NOISE_SLOTS);
    let counted = Instant::now();
    let (sphere_query, spheres) = with_spheres(sky, query.clone(), &mut noise);
    let count_time = counted.elapsed();
    let planned = Instant::now();
    let plans = [&sphere_query, &query, &seen_query]
        .map(|q| census_plan(&sky.galaxy, &sky.tables, &sky.envelope, q, &mut noise));
    let plan_time = planned.elapsed();
    // The sampled cells of each plan, and the union's, each cell with the plans that open it.
    let walked = Instant::now();
    let mut opened: BTreeMap<CellKey, [bool; 3]> = BTreeMap::new();
    let mut counts = [0_u64; 3];
    for (k, plan) in plans.iter().enumerate() {
        for key in plan.cells() {
            counts[k] += 1;
            if sample > 1 && !sample_hash(key).is_multiple_of(sample) {
                continue;
            }
            opened.entry(key).or_default()[k] = true;
        }
    }
    let walk = walked.elapsed();
    let cells: Vec<(CellKey, [bool; 3])> = opened.into_iter().collect();
    let jobs: Vec<&[(CellKey, [bool; 3])]> = cells.chunks(64).collect();
    let (parts, busy) = on_pool(&jobs, workers, &|job: &&[(CellKey, [bool; 3])]| {
        let mut ctx = job_context(sky);
        let mut found: Vec<Sorted> = Vec::new();
        for &(key, by) in *job {
            let mut stars = Vec::new();
            census_cell(&sky.galaxy, &mut ctx, key, &query, &mut stars);
            found.extend(stars.into_iter().map(|star| (star, by)));
        }
        found
    });
    if !PRINTED.swap(true, Ordering::Relaxed) {
        let stars: Vec<Sorted> = parts.into_iter().flat_map(|(_, p)| p).collect();
        eprintln!(
            "caps_by_ray_near_sun: one count of the caps {:.1} s; the three plans {:.1} s (the \
             spheres' forced, two counts); {} cells of the three plans' union censused in {:.1} \
             CPU-s ({:.1} s walking the plans' {}, {} and {} cells, one thread), {} stars",
            count_time.as_secs_f64(),
            plan_time.as_secs_f64(),
            cells.len(),
            busy.as_secs_f64(),
            walk.as_secs_f64(),
            counts[0],
            counts[1],
            counts[2],
            stars.len(),
        );
        let caps = [spheres.as_slice(), plans[1].caps(), plans[2].caps()];
        print_caps_record(sky, &observer, cut, sample, &caps, &stars);
    }
    busy * u32::try_from(sample).expect("a sample of under 2³² cells")
}

/// Prints [`caps_by_ray_near_sun`]'s record: per layer, the stars each plan of `caps` (the
/// spheres, by ray, by visibility) lists among `stars`, and those the spheres list that each of
/// the others drops and those it gains, scaled by `sample`, beside their expected counts and each
/// one's systems from a count at 3,072 rays and twice the radial steps.
fn print_caps_record(
    sky: &Sky,
    observer: &Observer,
    cut: Magnitudes,
    sample: u64,
    caps: &[&[LayerCap]; 3],
    stars: &[Sorted],
) {
    use std::fmt::Write as _;
    let mut noise = NoiseCache::with_capacity(NOISE_SLOTS);
    let fine = CapCount::measure(
        &sky.galaxy,
        &sky.tables,
        &sky.envelope,
        observer,
        cut,
        CapResolution::new(3_072, 48).expect("non-zero"),
        &mut noise,
    );
    if sample > 1 {
        eprintln!("  realised counts are an ESTIMATE from 1 cell in {sample}, scaled by {sample}");
    }
    for (l, &layer) in CAPPED_LAYERS.iter().enumerate() {
        // The stars of `layer` whose cells plan `a` opens and plan `b`, if any, does not, scaled
        // by the sample.
        let only = |a: usize, b: Option<usize>| {
            let n = stars
                .iter()
                .filter(|(star, by)| star.layer() == layer && by[a] && b.is_none_or(|b| !by[b]))
                .count();
            u64::try_from(n).expect("fewer stars than 2⁶⁴") * sample
        };
        let mut line = format!(
            "  {layer:?}: listed {} (sphere), {} (by ray), {} (by visibility)",
            only(0, None),
            only(1, None),
            only(2, None),
        );
        for (shape, by) in [(1, "by ray"), (2, "by visibility")] {
            let (sphere, other) = (&caps[0][l], &caps[shape][l]);
            write!(
                line,
                "; {by}: drops {} (expected {:.3}), gains {} (expected {:.3}), systems {:.1}% of \
                 the sphere's",
                only(0, Some(shape)),
                fine.stars_within_and_beyond(sphere, other),
                only(shape, Some(0)),
                fine.stars_within_and_beyond(other, sphere),
                100.0 * fine.systems_within(other)
                    / fine.systems_within(sphere).max(f64::MIN_POSITIVE),
            )
            .expect("a string takes any text");
        }
        eprintln!("{line}");
    }
}

fn luminosity_tables(c: &mut Criterion) {
    let galaxy = galaxy();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("luminosity_tables", |b| {
        b.iter(|| LuminosityTables::build(black_box(&galaxy)));
    });
    group.finish();
}

/// The replies of a near-Sun request at `caps`, nearest first, as the census's plan states them
/// at the fixed shell edges (`SHELL_EDGES_LY`: 125, 250 and 500 ly, then 1,000 × 2<sup>k</sup> ly;
/// R06.T8.i and R06.T11.g), which the server's march keeps (R06.T11.d): C, D and E complete to
/// each edge below their caps, held at them, then to their caps; A, B and the brown dwarfs, one
/// shell each, to their caps in every reply.
fn shell_replies(query: &SkyQuery, caps: &[LayerCap]) -> Vec<CompleteTo> {
    census_plan_of(query, caps.to_vec()).replies()
}

/// One job's context: the sky's tables, envelope and offsets, and a noise cache of its own.
fn job_context(sky: &Sky) -> SkyContext<'_> {
    SkyContext {
        tables: &sky.tables,
        envelope: &sky.envelope,
        offsets: &sky.offsets,
        noise: NoiseCache::with_capacity(NOISE_SLOTS),
        cells: &NoSkyCellCache,
        sources: &[],
        modifiers: &NoModifiers,
    }
}

/// The near-Sun request's caps at the eye's cut, as its final reply is complete to them.
fn band_caps(sky: &Sky, query: &SkyQuery) -> Vec<LayerCap> {
    let mut noise = NoiseCache::with_capacity(NOISE_SLOTS);
    layer_caps(
        &sky.galaxy,
        &sky.tables,
        &sky.envelope,
        query.observer(),
        query.cut(),
        &mut noise,
    )
}

/// The near-Sun request of a camera at [`CAMERA_CUT_V`], with the eye asked beside it at
/// [`EYE_CUT_V`] (`eye` true) or not at all (R06.T9.j).
fn camera_query(ly: [f64; 3], eye: bool) -> SkyQuery {
    let at = GalacticPosition::from_light_years(ly).expect("in the root cube");
    let observer = Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock");
    let builder = SkyQuery::builder(observer, Magnitudes::new(CAMERA_CUT_V));
    let builder = if eye {
        builder
            .eye(EyeObserver::default())
            .eye_cut(Magnitudes::new(EYE_CUT_V))
    } else {
        builder
    };
    builder.build().expect("a valid query")
}

/// A request's caps and the replies of its shell plan ([`shell_replies`]), made once.
type Replies = OnceCell<(Vec<LayerCap>, Vec<CompleteTo>)>;

/// The caps and replies of `query`, made in `cell` on first use.
fn replies_of<'a>(cell: &'a Replies, query: &SkyQuery) -> &'a (Vec<LayerCap>, Vec<CompleteTo>) {
    cell.get_or_init(|| {
        let caps = band_caps(sky(), query);
        let replies = shell_replies(query, &caps);
        (caps, replies)
    })
}

/// Marches all six faces of `spec` for `query`, one face row of `jobs` a job on `workers` threads,
/// keeping `replies`: each job's march and the jobs' CPU time.
fn march_all(
    query: &SkyQuery,
    replies: &[CompleteTo],
    spec: BandSpec,
    jobs: &[(CubeFace, u16)],
    workers: usize,
) -> (Vec<(usize, BandMarch)>, Duration) {
    let sky = sky();
    on_pool(jobs, workers, &|&(face, row): &(CubeFace, u16)| {
        march_rows(
            &sky.galaxy,
            &mut job_context(sky),
            black_box(query),
            replies.iter().cloned(),
            &spec,
            face,
            row..row + 1,
        )
    })
}

/// The march's bench `name` (`sky/band_near_sun/<name>`) for `query`: all six faces of
/// `BandSpec::STANDARD`, one face row a job, keeping the replies of its shell plan. Prints once
/// (`printed`) the rows, the replies and caps, the heap, the CPU time and the wall time.
fn bench_march(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    query: &SkyQuery,
    printed: &AtomicBool,
) {
    let workers = workers();
    let spec = BandSpec::STANDARD;
    let jobs: Vec<(CubeFace, u16)> = CubeFace::ALL
        .iter()
        .flat_map(|&face| (0..spec.face_texels()).map(move |row| (face, row)))
        .collect();
    // The caps and the replies, made outside the timing.
    let replies: Replies = OnceCell::new();
    group.bench_function(format!("band_near_sun/{name}"), |b| {
        let (caps, replies) = replies_of(&replies, query);
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let began = Instant::now();
                let (marches, busy) = march_all(query, replies, spec, &jobs, workers);
                if !printed.swap(true, Ordering::Relaxed) {
                    let heap = marches.iter().map(|(_, m)| m.heap_bytes()).sum::<usize>();
                    let eye = marches.first().and_then(|(_, m)| m.eye_cut()).map_or_else(
                        || "no eye light".to_owned(),
                        |cut| format!("the eye's light to V {}", cut.value()),
                    );
                    let radii: Vec<String> = caps
                        .iter()
                        .map(|cap| format!("{:?} {:.0}", cap.layer(), cap.radius().value()))
                        .collect();
                    println!(
                        "sky/band_near_sun/{name}: cut V {}, {eye}; {} rows, {} replies (caps \
                         {}), heap {heap} bytes, {:.1} CPU-s on {workers} workers, {:.2} s wall",
                        query.cut().value(),
                        marches.len(),
                        replies.len(),
                        radii.join(", "),
                        busy.as_secs_f64(),
                        began.elapsed().as_secs_f64()
                    );
                }
                cpu += busy;
            }
            cpu
        });
    });
}

/// The band of a near-Sun request, split as the server runs it (R06.T9.f): `band_near_sun/march`
/// marches all six faces once, one face row a job, keeping every reply of [`shell_replies`], lit
/// by the request's illumination (R06.T9.g), and `band_near_sun/march_no_dgl` the same request
/// with none; `band_near_sun/sum` sums the final reply's texels from the lit march, one face row a
/// job. Each prints, once, its CPU time and wall time, and the march its heap. `band_near_sun/march_camera`
/// marches a camera's request at [`CAMERA_CUT_V`] with the eye at [`EYE_CUT_V`] beside it, keeping
/// the eye's light too (R06.T9.j), and `band_near_sun/march_camera_no_eye` the same request with
/// no eye: the second sums cost their difference.
fn band_near_sun(c: &mut Criterion) {
    let query = lit_eye_query(SUN_LY, sun_illumination());
    let workers = workers();
    let spec = BandSpec::STANDARD;
    let jobs: Vec<(CubeFace, u16)> = CubeFace::ALL
        .iter()
        .flat_map(|&face| (0..spec.face_texels()).map(move |row| (face, row)))
        .collect();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    bench_march(&mut group, "march", &query, &PRINTED_BAND);
    bench_march(
        &mut group,
        "march_no_dgl",
        &eye_query(SUN_LY),
        &PRINTED_BAND_NO_DGL,
    );
    let held: OnceCell<Vec<BandMarch>> = OnceCell::new();
    let replies: Replies = OnceCell::new();
    group.bench_function("band_near_sun/sum", |b| {
        let (caps, replies) = replies_of(&replies, &query);
        let marches = held.get_or_init(|| {
            march_all(&query, replies, spec, &jobs, workers)
                .0
                .into_iter()
                .map(|(_, march)| march)
                .collect()
        });
        let last = CompleteTo::of_caps(caps);
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let began = Instant::now();
                let (rows, busy) = on_pool(marches, workers, &|march: &BandMarch| {
                    let mut out = Vec::with_capacity(usize::from(spec.face_texels()));
                    sum_rows(march, &SkyCensus::empty(), black_box(&last), &mut out);
                    out
                });
                if !PRINTED_BAND_SUM.swap(true, Ordering::Relaxed) {
                    let texels = rows.iter().map(|(_, row)| row.len()).sum::<usize>();
                    println!(
                        "sky/band_near_sun/sum: {texels} texels of the final reply, {:.3} CPU-s on \
                         {workers} workers, {:.3} s wall",
                        busy.as_secs_f64(),
                        began.elapsed().as_secs_f64()
                    );
                }
                cpu += busy;
            }
            cpu
        });
    });
    bench_march(
        &mut group,
        "march_camera",
        &camera_query(SUN_LY, true),
        &PRINTED_BAND_CAMERA,
    );
    bench_march(
        &mut group,
        "march_camera_no_eye",
        &camera_query(SUN_LY, false),
        &PRINTED_BAND_CAMERA_NO_EYE,
    );
    group.finish();
}

/// The limit map's near-Sun fixture: the 64² band of the stars fainter than the cut with no
/// census, complete everywhere, its faces in `CubeFace::ALL`'s order, and the stars the census
/// lists within 200 ly brighter than the cut, with their observer.
struct LimitMapFixture {
    observer: Observer,
    band: Vec<BandTexel>,
    listed: Vec<SkyStar>,
}

/// The near-Sun fixture of the limit map, the band and the census each on the pool.
fn limit_map_fixture(sky: &Sky, workers: usize, jobs: &[(CubeFace, u16)]) -> LimitMapFixture {
    let at = GalacticPosition::from_light_years(SUN_LY).expect("in the root cube");
    let observer = Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock");
    let query = SkyQuery::builder(observer, Magnitudes::new(LIMIT_MAP_CUT_V))
        .build()
        .expect("a valid query");
    let spec = BandSpec::STANDARD;
    let (rows, _) = on_pool(jobs, workers, &|&(face, row): &(CubeFace, u16)| {
        let mut ctx = SkyContext {
            tables: &sky.tables,
            envelope: &sky.envelope,
            offsets: &sky.offsets,
            noise: NoiseCache::with_capacity(NOISE_SLOTS),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let mut out = Vec::with_capacity(usize::from(spec.face_texels()));
        band_rows(
            &sky.galaxy,
            &mut ctx,
            &query,
            &SkyCensus::empty(),
            &CompleteTo::everywhere(),
            &spec,
            face,
            row..row + 1,
            &mut out,
        );
        out
    });
    let forced = query
        .with_caps_forced(LightYears::new(LIMIT_MAP_RADIUS_LY))
        .expect("a forced cap");
    let run = census(sky, &forced, &NoSkyCellCache, workers, 1);
    LimitMapFixture {
        observer,
        band: rows.into_iter().flat_map(|(_, row)| row).collect(),
        listed: run.census.listed().to_vec(),
    }
}

/// The glare ruling's synthetic sky (`decision-r06-t9c-glare.md`, item 2), as the sim's own test of
/// the pyramid takes it, star for star (`sky::limits`' `synthetic_sky`, on the same `SplitMix64`
/// stream as the sky tests' `uniforms`): `n` stars over the whole sky, concentrated towards the
/// plane as 1 + 3 exp(−|b| ÷ 10°), of V from −1.5 to `faintest` with N(< V) ∝ 10^(0.45 V), and of
/// S/P ratio uniform in 1.5–3.
fn synthetic_sky(n: usize, faintest: f64) -> Vec<(UnitVector, Lux, SpRatio)> {
    const SLOPE: f64 = 0.45;
    let (lo, hi) = (math::exp10(SLOPE * -1.5), math::exp10(SLOPE * faintest));
    let mut state = 0x0009_1a00_u64;
    let mut next = || {
        let z = mix(state);
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        #[expect(clippy::cast_precision_loss, reason = "53 bits, exact in f64")]
        let u = (z >> 11) as f64 / (1_u64 << 53) as f64;
        u
    };
    let mut sky = Vec::with_capacity(n);
    while sky.len() < n {
        let z = 2.0 * next() - 1.0;
        let (sin, cos) = math::sin_cos(std::f64::consts::TAU * next());
        let b = math::asin(z).abs() / RADIANS_PER_DEGREE;
        if 4.0 * next() > 1.0 + 3.0 * math::exp(-b / 10.0) {
            continue;
        }
        let across = (1.0 - z * z).sqrt();
        let direction =
            UnitVector::from_components([across * cos, across * sin, z]).expect("a direction");
        let v = math::log10(lo + next() * (hi - lo)) / SLOPE;
        let rho = SpRatio::new(1.5 + 1.5 * next()).expect("a ratio within 1.5–3");
        sky.push((direction, illuminance_of_magnitude(Magnitudes::new(v)), rho));
    }
    let values: Vec<f64> = sky
        .iter()
        .flat_map(|(u, e, rho)| {
            let [x, y, z] = u.components();
            [x, y, z, e.value(), rho.value()]
        })
        .collect();
    let digest = f64_digest(&values);
    assert_eq!(
        digest, SYNTHETIC_SKY_DIGEST,
        "the synthetic sky's digest {digest:#018x}: the sim's test's sky, star for star"
    );
    sky
}

/// One limit map as the server will run it (R06.T11.c): the glare `build` gives, on one thread,
/// then each of `jobs` (a face's row) on the pool, `band`'s rows' limits set against it. Its CPU
/// time is the build's and the jobs' together. Prints the first run's costs as `name`.
fn limit_map_once(
    name: &str,
    printed: &AtomicBool,
    build: &dyn Fn() -> Glare,
    band: &[BandTexel],
    jobs: &[(CubeFace, u16)],
    workers: usize,
) -> Duration {
    let spec = BandSpec::STANDARD;
    let side = usize::from(spec.face_texels());
    let began = Instant::now();
    let glare = build();
    let glare_time = began.elapsed();
    let (rows, busy) = on_pool(jobs, workers, &|&(face, row): &(CubeFace, u16)| {
        let first = (usize::from(face.layer()) * side + usize::from(row)) * side;
        let mut texels = band[first..first + side].to_vec();
        limit_rows(
            &EyeObserver::default(),
            &spec,
            black_box(&glare),
            face,
            row..row + 1,
            &mut texels,
        );
        texels
    });
    black_box(rows);
    if !printed.swap(true, Ordering::Relaxed) {
        println!(
            "sky/limit_map/{name}: {} stars over {} texels: the glare built in {:.3} s, the map \
             {:.2} CPU-s on {workers} workers ({:.2} s wall), {:.2} CPU-s in all",
            glare.len(),
            band.len(),
            glare_time.as_secs_f64(),
            busy.as_secs_f64(),
            began.elapsed().as_secs_f64(),
            (glare_time + busy).as_secs_f64()
        );
    }
    glare_time + busy
}

fn limit_map(c: &mut Criterion) {
    let workers = workers();
    let spec = BandSpec::STANDARD;
    let jobs: Vec<(CubeFace, u16)> = CubeFace::ALL
        .iter()
        .flat_map(|&face| (0..spec.face_texels()).map(move |row| (face, row)))
        .collect();
    // The fixture and the synthetic sky are made only when a bench that reads them is run.
    let fixture: OnceCell<LimitMapFixture> = OnceCell::new();
    let synthetic: OnceCell<Vec<(UnitVector, Lux, SpRatio)>> = OnceCell::new();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("limit_map/near_sun", |b| {
        let fixture = fixture.get_or_init(|| limit_map_fixture(sky(), workers, &jobs));
        let cut = Magnitudes::new(LIMIT_MAP_CUT_V);
        let build = || Glare::of_listed(&fixture.observer, &fixture.listed, &spec, cut);
        b.iter_custom(|iters| {
            (0..iters)
                .map(|_| {
                    let name = "near_sun";
                    let printed = &PRINTED_LIMIT_MAP_NEAR_SUN;
                    limit_map_once(name, printed, &build, &fixture.band, &jobs, workers)
                })
                .sum()
        });
    });
    group.bench_function("limit_map/synthetic_300k", |b| {
        let fixture = fixture.get_or_init(|| limit_map_fixture(sky(), workers, &jobs));
        let stars = synthetic.get_or_init(|| {
            let n = usize::try_from(MAX_N_MAX).expect("300,000 fits a usize");
            synthetic_sky(n, SYNTHETIC_CUT_V)
        });
        let build = || Glare::of_points(stars.iter().copied(), &spec);
        b.iter_custom(|iters| {
            (0..iters)
                .map(|_| {
                    let name = "synthetic_300k";
                    let printed = &PRINTED_LIMIT_MAP_SYNTHETIC;
                    limit_map_once(name, printed, &build, &fixture.band, &jobs, workers)
                })
                .sum()
        });
    });
    group.finish();
}

criterion_group!(
    sky_benches,
    luminosity_tables,
    census_near_sun_cold,
    census_near_sun,
    census_nuclear_disc,
    star_bound,
    illumination,
    caps_by_ray_near_sun,
    band_near_sun,
    limit_map
);
criterion_main!(sky_benches);
