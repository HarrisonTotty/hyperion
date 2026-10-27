//! The level bound on generated ground (rendering plan R10, T4.a): R09's `level_bound_m` on the
//! sample worlds, the whole coarse pass's fields at their own coarse levels, holds for every pair
//! of levels at ten thousand points a world, and its looseness is measured for R10.T4.b.
//!
//! The bound is R09.T8's as accepted: hard in the structural octaves and the channels, and a
//! 1 − 10⁻⁹ per-point quantile in the small craters' and the coarse rims' terms
//! (`synth::levels::CRATER_BOUND_TAIL`), so the test holds that stated bound and names which term
//! each level's worst case came from.
//!
//! On each world, at every band level n below the band limit's level c (the shallowest whose
//! shortest wavelength is the 2 m band limit, from which every level's height is c's to the bit
//! and the bound is zero):
//!
//! - **The bound holds.** At 10⁴ points, 25 at random in each of 400 distinct random cells of the
//!   world's coarse level (the *uniform* points), and at 25 more in each of the cube's seam cells
//!   (a corner and two face edges) and of up to 40 cells under coarse craters' rims (the *extra*
//!   points), |h(n) − h(m)| ≤ `level_bound_m(n)` for every m in n + 1 to c, and every level from
//!   c + 1 to the deepest gives c's height to the bit. At every vertex of the per-patch samples
//!   below, |h(n) − h(c)| ≤ `level_bound_m(n)` too.
//! - **Its looseness is measured**, over the uniform points: the 99.9th percentile (nearest rank)
//!   of |h(n) − h(c)| over the bound, which R10's Design note 5 expects at about 0.3–0.5; the same
//!   over four times σ of the point's own cell, σ R09's level-keyed `Synthesiser::omitted_rms`
//!   (R10.T5); and the largest |h(n) − h(m)| over the bound for any m and any point, uniform or
//!   extra, with the contribution whose change between the two levels made most of it.
//! - **Per patch**, at four distinct random patches of level n ([`PATCHES`], fewer than R05's 32
//!   for their cost): the largest |h(n) − h(c)| over its 65 × 65 = 4,225 vertices, level n's
//!   heights from `Synthesiser::bake_patch` and c's from `height_at`, over the bound; the share of
//!   patches whose largest passes 4σ, σ the patch's own (the largest `omitted_rms` at n over the
//!   cells its vertices lie in, as R10.T6.b's bake takes it) and the body's (below); and the spread
//!   between the body-wide and a per-patch bound, the largest patch's maximum over the median
//!   patch's (the lower median). Design note 5 adopts a per-patch bound only if that spread passes
//!   1.5.
//!
//! The figures go to `apps/hyperion/src/renderer/src/view/terrain/fixtures/level-bounds.json`, for
//! R10.T4.b: each world's hard bound by level with the body's σ, the largest `omitted_rms` over
//! up to 16,384 cells of its coarse level spread evenly through it ([`SIGMA_CELLS`]; every cell at
//! coarse level 5) and their root mean square (each cell weighted alike); what a level table's
//! heights are made of, the field's lowest and highest cell elevation and each level's
//! `LevelBounds::contribution_range_m`; and the measured figures above, `null` where a level has
//! none (no point's or no median patch's height differs from c's). `HYPERION_BLESS=1` writes it;
//! otherwise the test holds it to the figures byte for byte, line ends aside. The heights are the
//! generator's, so it moves with them: it was measured at generator version 23, and at 24 the
//! coarse fields move by plan 14's T53.a water term, so R10.T15.b re-records the figures there
//! (main's process call, 2026-10-26; the fixture's description says so while the version is 23).
//!
//! The worlds are R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`), all six, each
//! through the whole coarse pass under `reference::SEED`, drawn with one detail seed ([`DETAIL`]).
//! A world's points and patches come from a sampler seeded by its name ([`sampler`]), all drawn
//! before any is read ([`draw`]). Natively the worlds are measured on a thread each, each world's
//! body σ on one more and its points and patches read on [`THREADS`] more, and the figures do not
//! depend on the threads: a run with every world's reads on one thread gave the same fixture. On
//! WebAssembly, which has no threads, they would run in turn; no suite runs the binary there, which
//! needs the sim's `testing` feature.
//!
//! Slow: `just test-slow -E 'binary(level_bound_worlds)'`, or by name,
//! `cargo nextest run -p hyperion-sim --features testing --cargo-profile slow-test --profile slow
//! --run-ignored only -E 'binary(level_bound_worlds)'`, with `HYPERION_BLESS=1` to write the
//! fixture.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the bound's tests use only the sample worlds of the shared helpers"
)]
mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use common::surface_worlds::{SampleWorld, sample_worlds};
use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;
use hyperion_sim::GENERATOR_VERSION;
use hyperion_surface::cube::{
    Face, FaceUv, MAX_LEVEL, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir,
};
use hyperion_surface::field::{CoarseField, FieldView, cell_at_index};
use hyperion_surface::geometry::{BAND_LIMIT_M, finest_level};
use hyperion_surface::synth::craters::CraterCache;
use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden::Mode;
use hyperion_testkit::lcg::Lcg;

/// The detail seed the sample worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x6c65_7665_6c62_6e64);

/// The seed of the sample points and patches, into which each world's name is folded
/// ([`sampler`]).
const SAMPLES: u64 = 0x6c65_7665_6c62_6e73;

/// The FNV-1a 64 prime (Fowler, Noll and Vo), the testkit's `f64_digest`'s, which folds a world's
/// name into [`SAMPLES`].
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Distinct random cells of a world's coarse level, and points at random in each: 10⁴ uniform
/// points a world.
const CELLS: usize = 400;
const POINTS: usize = 25;

/// The most coarse craters whose rim cells add extra points.
const RIM_CELLS: usize = 40;

/// The patches sampled a level: four, of level 0's six too. A patch costs about 5–8 s, its 4,225
/// queries at the complete level each drawing the finest crater octaves' cells afresh (about
/// 1.4 ms a query on the Ceres-like world, where R05's 32 a level took 47 s for level 0's six and
/// 204 s at level 2; provisional, taken beside other lanes' runs at a 1-minute load of 5 to 18),
/// so a world's 60 to 76 patches are most of the test's time.
const PATCHES: usize = 4;

/// The vertices along a patch's side, R05's 65.
const SIDE: u8 = 65;

/// The most cells the body's σ is taken over, spread evenly through the coarse level's index order:
/// every cell of a world at coarse level 5, every second at 6, every sixth at 7 and every 24th at
/// 8. An `omitted_rms` took about 70 µs on the Ceres-like world (the craters' and rims' closed
/// forms; provisional, as [`PATCHES`]'s figures), so every cell of a level-8 world at 20 levels
/// would take some 550 to 1,100 s.
const SIGMA_CELLS: u32 = 16_384;

/// The quantile measured over the uniform points: the 99.9th percentile.
const QUANTILE: f64 = 0.999;

/// The fixture, from the repository's root.
const FIXTURE: &str = "apps/hyperion/src/renderer/src/view/terrain/fixtures/level-bounds.json";

/// The fixture's description, before its note on the generator version ([`version_note`]).
const DESCRIPTION: &str = "The level bound measured on the sample worlds (rendering plan R10, \
     R10.T4.a): for each of R09's six sample worlds, its coarse field from the whole coarse pass \
     under reference::SEED and its ground drawn with detail_seed. elevation_lowest_m and \
     elevation_highest_m are the lowest and highest elevation of every cell of the field (metres \
     above the datum). levels gives, by band level from 0 to complete_level (the band limit's \
     level, from which the bound is zero): R09's level bound bound_m (metres; hard in the \
     structural octaves and the channels, a 1 - 1e-9 per-point quantile in the craters' and the \
     coarse rims' terms; R09.T8); the body's sigma, R09's level-keyed \
     Synthesiser::omitted_rms over up to 16384 cells of the world's coarse level, every k-th in \
     index order (every cell at coarse level 5, every 2nd at 6, 6th at 7, 24th at 8): sigma_m its \
     largest, sigma_rms_m the root of the cells' mean square (each cell weighted alike; metres); \
     and contribution_low_m and contribution_high_m, the most the level's contributions can take \
     from and add to the base elevation (R09's LevelBounds::contribution_range_m; metres), from \
     which, with the cells' elevations, the worker's level table makes a level's lowest and \
     highest height. measured gives, by level below complete_level, the bound's looseness: \
     p999_over_bound, the 99.9th percentile (nearest rank) over the uniform points of \
     |h(n) - h(c)| over bound_m, c the complete level; p999_over_4sigma_cell, the same over 4 \
     sigma of the point's own cell; worst_over_bound, the largest |h(n) - h(m)| over bound_m for \
     any finer m at any point, uniform or extra, at worst_finer_level, with worst_term the \
     contribution (relief, channels, craters or rims) whose change made most of it and \
     worst_term_share that change over the whole, the three null where no point's height differs \
     from c's; and over patches distinct random patches of the level (65 x 65 vertices each, level \
     n's heights baked and c's queried): patch_max_over_bound, the largest per-patch maximum of \
     |h(n) - h(c)| over bound_m, patch_median_over_bound the lower median's, patch_spread the \
     largest over the median (null where the median is zero), patches_over_4sigma the patches \
     whose maximum passes 4 sigma of their own (the largest omitted_rms over the cells their \
     vertices lie in), and patches_over_4sigma_body those whose maximum passes 4 sigma_m. Written \
     by crates/hyperion-sim/tests/level_bound_worlds.rs under HYPERION_BLESS=1 and checked byte \
     for byte otherwise; never edit it by hand.";

/// The fixture's note on the generator version its figures were measured at. At 23, main's
/// process call of 2026-10-26: the coarse fields move at 24, and R10.T15.b re-records there.
fn version_note() -> String {
    match GENERATOR_VERSION.get() {
        23 => "Measured at generator_version 23: the coarse fields move at 24 by plan 14's T53.a \
               water term, and R10.T15.b re-records the figures there."
            .to_owned(),
        version => format!("Measured at generator_version {version}."),
    }
}

/// A contribution to the height above the base elevation, which every level evaluates alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Term {
    /// The structural octaves.
    Relief,
    /// The channels' incision.
    Channels,
    /// The small craters.
    Craters,
    /// The coarse craters' rims.
    Rims,
}

impl Term {
    /// Every term, in the order a height sums them.
    const ALL: [Self; 4] = [Self::Relief, Self::Channels, Self::Craters, Self::Rims];

    /// Its name in the fixture.
    const fn name(self) -> &'static str {
        match self {
            Self::Relief => "relief",
            Self::Channels => "channels",
            Self::Craters => "craters",
            Self::Rims => "rims",
        }
    }
}

/// Which of a world's sampled cells a point lies in (the module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sample {
    /// One of the distinct random cells, over whose points the quantiles are taken.
    Uniform,
    /// A seam or rim cell, whose points join the bound and the worst case only.
    Extra,
}

/// The body's σ at a band level, over the cells [`body_sigma`] reads.
#[derive(Debug, Clone, Copy)]
struct BodySigma {
    /// The largest `omitted_rms`.
    largest: Metres,
    /// The root of the mean square.
    rms: Metres,
}

/// A band level's row of the fixture: its bound, the body's σ, and the range its contributions
/// can add to the base elevation.
#[derive(Debug, Clone, Copy)]
struct LevelRow {
    bound: Metres,
    sigma: BodySigma,
    contribution_low: Metres,
    contribution_high: Metres,
}

/// What the test measures at a band level below the complete level (the module documentation).
#[derive(Debug, Clone, Copy)]
struct Measured {
    p999_over_bound: f64,
    p999_over_4sigma_cell: f64,
    worst_over_bound: f64,
    /// Where the worst gap is not zero: its finer level, and the term that made most of it.
    worst: Option<Worst>,
    patches: usize,
    patch_max_over_bound: f64,
    patch_median_over_bound: f64,
    /// The largest patch's maximum over the median's, where the median's is not zero.
    patch_spread: Option<f64>,
    patches_over_4sigma: usize,
    patches_over_4sigma_body: usize,
}

/// A level's worst gap over the bound: the finer level it was found at, the contribution whose
/// change made most of it, and that change over the whole gap.
#[derive(Debug, Clone, Copy)]
struct Worst {
    finer: u8,
    term: Term,
    share: f64,
}

/// A world's figures, the fixture's entry for it.
#[derive(Debug, Clone)]
struct WorldFigures {
    name: &'static str,
    radius_m: f64,
    coarse_level: u8,
    finest_level: u8,
    complete_level: u8,
    elevation_lowest: Metres,
    elevation_highest: Metres,
    uniform_points: usize,
    extra_points: usize,
    levels: Vec<LevelRow>,
    measured: Vec<Measured>,
}

/// Band level `n`.
fn band(n: u8) -> BandLevel {
    BandLevel::new(n).expect("a band level of the cube")
}

/// A count as an `f64`, exactly: every count here is far below 2³².
fn count_f64(n: usize) -> f64 {
    f64::from(u32::try_from(n).expect("a test count fits in 32 bits"))
}

/// The sampler of the world named `name`: an [`Lcg`] seeded with FNV-1a 64 over the name's bytes,
/// from [`SAMPLES`] in place of the offset basis.
fn sampler(name: &str) -> Lcg {
    Lcg::new(name.bytes().fold(SAMPLES, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    }))
}

/// A random patch of `level`, any face, i and j alike likely.
fn random_patch(level: u8, rng: &mut Lcg) -> PatchKey {
    let face = Face::ALL[usize::try_from(rng.next_below(6)).expect("a face index")];
    let side = 1_u64 << level;
    let mut pick = || u32::try_from(rng.next_below(side)).expect("an index of the level");
    let (i, j) = (pick(), pick());
    PatchKey::new(face, level, i, j).expect("a patch of the level")
}

/// `count` distinct patches of `level` at random, or every patch of the level where it has no more
/// than `count`, in key order.
fn distinct_patches(level: u8, count: usize, rng: &mut Lcg) -> Vec<PatchKey> {
    let side = 1_u32 << level;
    let total = 6 * u64::from(side) * u64::from(side);
    if total <= u64::try_from(count).expect("a small count") {
        let mut every = Vec::new();
        for face in Face::ALL {
            for i in 0..side {
                for j in 0..side {
                    every.push(PatchKey::new(face, level, i, j).expect("a patch of the level"));
                }
            }
        }
        every.sort_unstable();
        return every;
    }
    let mut chosen = BTreeSet::new();
    while chosen.len() < count {
        chosen.insert(random_patch(level, rng));
    }
    chosen.into_iter().collect()
}

/// The unit direction at face coordinates (`x`, `y`) of `cell`, each in [0, 1].
fn point_in(cell: PatchKey, x: f64, y: f64) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    let s = (f64::from(cell.i()) + x) / side;
    let t = (f64::from(cell.j()) + y) / side;
    unit_dir(face_uv_to_xyz(FaceUv {
        face: cell.face(),
        u: st_to_uv(s),
        v: st_to_uv(t),
    }))
}

/// The cube's seam cells at `level`: a corner's and two face edges'.
fn seam_cells(level: u8) -> [PatchKey; 3] {
    let last = (1_u32 << level) - 1;
    [
        PatchKey::new(Face::PosX, level, 0, 0).expect("a cube corner's cell"),
        PatchKey::new(Face::NegY, level, last, last / 3).expect("a face edge's cell"),
        PatchKey::new(Face::PosZ, level, last / 2, 0).expect("a face edge's cell"),
    ]
}

/// The coarse cells under the rims of up to `count` of `field`'s listed craters, spread through
/// its list: each crater's cell under a point of its rim, a rim radius's chord from its centre
/// (R09.T8's `rim_cells`).
fn rim_cells(field: &CoarseField, count: usize) -> Vec<PatchKey> {
    let header = field.header();
    let level = header.level().get();
    let radius_m = header.radius().value();
    let craters = field.craters();
    let stride = (craters.len() / count.max(1)).max(1);
    craters
        .iter()
        .step_by(stride)
        .take(count)
        .map(|crater| {
            let c = crater.centre;
            let e = if c[2].abs() < 0.9 {
                [-c[1], c[0], 0.0]
            } else {
                [0.0, -c[2], c[1]]
            };
            let n = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
            let half_chord = 0.25 * crater.diameter.value() / radius_m;
            let angle = 2.0 * hyperion_base::math::asin(half_chord.min(1.0));
            let (sin, cos) = hyperion_base::math::sin_cos(angle);
            let dir = unit_dir([0, 1, 2].map(|k| cos * c[k] + sin * e[k] / n));
            PatchKey::containing(level, dir).expect("a coarse level is a level")
        })
        .collect()
}

/// The value at quantile `q` of `values` by nearest rank: the ⌈q N⌉-th smallest.
fn nearest_rank(values: &mut [f64], q: f64) -> f64 {
    assert!(!values.is_empty(), "a quantile of nothing");
    values.sort_unstable_by(f64::total_cmp);
    let n = count_f64(values.len());
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "⌈q N⌉ lies in [1, N] for q in (0, 1], and N is far below 2³²"
    )]
    let rank = (q * n).ceil() as usize;
    values[rank.clamp(1, values.len()) - 1]
}

/// Each term's height at `dir` and `level`, through the term's own public evaluator, with the
/// contributions `level` evaluates ([`Synthesiser::level_set`]): what `height_at` sums above the
/// base elevation.
fn terms_at(
    synth: &Synthesiser<'_, CoarseField>,
    cache: &mut SynthCache,
    dir: [f64; 3],
    level: BandLevel,
) -> [f64; 4] {
    let field = synth.field();
    let set = *synth.level_set(level);
    let relief = set.relief_through().map_or(0.0, |through| {
        synth
            .relief()
            .at(field, dir, through, cache)
            .expect("the field holds every cell")
            .height_m
    });
    let channels = set.channels_through().map_or(0.0, |through| {
        synth
            .channels()
            .incision_at(field, dir, through, cache)
            .expect("the field holds every cell")
            .height_m
    });
    let craters = set.craters_through().map_or(0.0, |through| {
        synth
            .craters()
            .at(dir, level, through, &mut CraterCache::new())
            .height_m
    });
    let rims = synth
        .rims()
        .at(field, dir, level)
        .expect("the field holds every cell")
        .height_m;
    [relief, channels, craters, rims]
}

/// The term whose change between levels `n` and `m` at `dir` makes most of h(n) − h(m), and its
/// share of the whole, `gap`, which the terms' changes sum to within rounding.
fn worst_term(
    synth: &Synthesiser<'_, CoarseField>,
    dir: [f64; 3],
    n: u8,
    m: u8,
    gap: Metres,
) -> (Term, f64) {
    let gap = gap.value();
    let mut cache = SynthCache::new();
    let (at_n, at_m) = (
        terms_at(synth, &mut cache, dir, band(n)),
        terms_at(synth, &mut cache, dir, band(m)),
    );
    let changes: [f64; 4] = core::array::from_fn(|k| at_n[k] - at_m[k]);
    let sum: f64 = changes.iter().sum();
    assert!(
        (sum - gap).abs() <= 1e-6,
        "{dir:?} {n}→{m}: the terms' changes sum to {sum} m, not h(n) − h(m) = {gap} m"
    );
    let (k, change) = changes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .expect("four terms");
    (Term::ALL[k], change / gap)
}

/// The body's σ by level 0 to `complete`: the largest `omitted_rms` over [`SIGMA_CELLS`] cells of
/// `field`'s coarse level, every k-th in index order, and the root of their mean square. At the
/// complete level each is zero.
fn body_sigma(field: &CoarseField, complete: u8) -> Vec<BodySigma> {
    let synth = Synthesiser::new(field, DETAIL);
    let level = field.header().level();
    let levels = usize::from(complete) + 1;
    let mut largest = vec![0.0_f64; levels];
    let mut squares = vec![0.0_f64; levels];
    let stride = level.cell_count().div_ceil(SIGMA_CELLS);
    let mut cells = 0_u32;
    for index in (0..level.cell_count()).step_by(usize::try_from(stride).expect("a stride")) {
        cells += 1;
        let cell = cell_at_index(level.get(), index).expect("an index of the coarse level");
        for n in 0..=complete {
            let sigma = synth
                .omitted_rms(cell, band(n))
                .expect("the field holds every cell")
                .value();
            assert!(sigma.is_finite() && sigma >= 0.0, "{cell:?} {n}: σ {sigma}");
            let k = usize::from(n);
            largest[k] = largest[k].max(sigma);
            squares[k] += sigma * sigma;
        }
    }
    assert_eq!(
        bits(largest[usize::from(complete)]),
        bits(0.0),
        "the complete level omits nothing"
    );
    let cells = f64::from(cells);
    largest
        .into_iter()
        .zip(squares)
        .map(|(max, sum)| BodySigma {
            largest: Metres::new(max),
            rms: Metres::new((sum / cells).sqrt()),
        })
        .collect()
}

/// The lowest and highest elevation of every cell of `field`, metres above the datum.
fn elevation_range(field: &CoarseField) -> (Metres, Metres) {
    let level = field.header().level();
    let (mut lowest, mut highest) = (f64::INFINITY, f64::NEG_INFINITY);
    for index in 0..level.cell_count() {
        let key = cell_at_index(level.get(), index).expect("an index of the coarse level");
        let elevation = field
            .cell(key)
            .expect("the field holds every cell")
            .elevation()
            .value();
        lowest = lowest.min(elevation);
        highest = highest.max(elevation);
    }
    assert!(
        lowest.is_finite() && highest.is_finite(),
        "the field's elevations are finite"
    );
    (Metres::new(lowest), Metres::new(highest))
}

/// What a world is sampled at, all drawn from its sampler before any height is read, so that the
/// figures do not depend on how the reading is spread over threads: each sampled cell, which kind
/// of cell it is, and its points; and by level below the complete one, its patches.
struct Draw {
    cells: Vec<(PatchKey, Sample, Vec<[f64; 3]>)>,
    patches: Vec<Vec<PatchKey>>,
}

/// Draws `field`'s uniform and extra cells and their points, then each level's patches (the
/// module documentation).
fn draw(field: &CoarseField, complete: u8, rng: &mut Lcg) -> Draw {
    let level = field.header().level().get();
    let mut uniform = BTreeSet::new();
    while uniform.len() < CELLS {
        uniform.insert(random_patch(level, rng));
    }
    let mut extra = seam_cells(level).to_vec();
    extra.extend(rim_cells(field, RIM_CELLS));
    let cells = uniform
        .into_iter()
        .map(|cell| (cell, Sample::Uniform))
        .chain(extra.into_iter().map(|cell| (cell, Sample::Extra)))
        .map(|(cell, kind)| {
            let points = (0..POINTS)
                .map(|_| point_in(cell, rng.next_f64(), rng.next_f64()))
                .collect();
            (cell, kind, points)
        })
        .collect();
    let patches = (0..complete)
        .map(|n| distinct_patches(n, PATCHES, rng))
        .collect();
    Draw { cells, patches }
}

/// A sampled cell's σ at each level below the complete one, and each of its points' heights at
/// every level to the complete one, asserting that every deeper level gives the complete one's
/// height to the bit.
fn read_cell(
    synth: &Synthesiser<'_, CoarseField>,
    cell: PatchKey,
    points: &[[f64; 3]],
    complete: u8,
) -> (Vec<Metres>, Vec<Vec<f64>>) {
    let sigmas = (0..complete)
        .map(|n| {
            synth
                .omitted_rms(cell, band(n))
                .expect("the field holds every cell")
        })
        .collect();
    let mut cache = SynthCache::new();
    let mut height = |dir, n| {
        synth
            .height_at(&mut cache, dir, band(n))
            .expect("the field holds every cell")
            .height_m
    };
    let heights = points
        .iter()
        .map(|&dir| {
            let heights: Vec<f64> = (0..=complete).map(|n| height(dir, n)).collect();
            for deeper in complete + 1..=MAX_LEVEL {
                assert_eq!(
                    bits(height(dir, deeper)),
                    bits(heights[usize::from(complete)]),
                    "{dir:?}: level {deeper} gives the complete level {complete}'s height"
                );
            }
            heights
        })
        .collect();
    (sigmas, heights)
}

/// A level's worst gap over the bound so far: the ratio, the finer level and point it was found
/// at, and the gap h(n) − h(m) with its sign.
#[derive(Debug, Clone, Copy)]
struct WorstGap {
    ratio: f64,
    finer: u8,
    dir: [f64; 3],
    signed_gap: Metres,
}

/// By level below the complete one: the finest gaps' ratios over the bound and over 4σ of the
/// point's cell at the uniform points, and the worst gap over the bound at any point.
struct PointFigures {
    over_bound: Vec<Vec<f64>>,
    over_4sigma: Vec<Vec<f64>>,
    worst: Vec<WorstGap>,
    uniform: usize,
    extra: usize,
}

/// Reads the drawn cells' points ([`read_cell`], spread over threads by [`par_map`]) and gathers
/// their figures in the draw's order, asserting the bound for every pair of levels.
fn sample_points(
    name: &str,
    synth: &Synthesiser<'_, CoarseField>,
    bounds: &[f64],
    complete: u8,
    cells: &[(PatchKey, Sample, Vec<[f64; 3]>)],
) -> PointFigures {
    let below = usize::from(complete);
    let read = par_map(cells, |(cell, _, points)| {
        read_cell(synth, *cell, points, complete)
    });
    let quantiles = || {
        (0..below)
            .map(|_| Vec::with_capacity(CELLS * POINTS))
            .collect()
    };
    let none = WorstGap {
        ratio: 0.0,
        finer: 0,
        dir: [0.0; 3],
        signed_gap: Metres::new(0.0),
    };
    let mut figures = PointFigures {
        over_bound: quantiles(),
        over_4sigma: quantiles(),
        worst: vec![none; below],
        uniform: 0,
        extra: 0,
    };
    for ((_, kind, points), (sigmas, heights)) in cells.iter().zip(read) {
        for (&dir, heights) in points.iter().zip(heights) {
            for n in 0..below {
                let bound = bounds[n];
                for (m, &finer) in heights.iter().enumerate().skip(n + 1) {
                    let signed = heights[n] - finer;
                    let gap = signed.abs();
                    assert!(
                        gap <= bound,
                        "{name} {dir:?}: |h({n}) − h({m})| = {gap} m over the bound {bound} m"
                    );
                    let ratio = gap / bound;
                    if ratio > figures.worst[n].ratio {
                        figures.worst[n] = WorstGap {
                            ratio,
                            finer: u8::try_from(m).expect("a band level"),
                            dir,
                            signed_gap: Metres::new(signed),
                        };
                    }
                }
                if *kind == Sample::Uniform {
                    let gap = (heights[n] - heights[below]).abs();
                    let sigma = sigmas[n].value();
                    figures.over_bound[n].push(gap / bound);
                    assert!(
                        sigma > 0.0 || bits(gap) == bits(0.0),
                        "{name} {dir:?} {n}: a gap of {gap} m where σ is zero"
                    );
                    figures.over_4sigma[n].push(if sigma > 0.0 {
                        gap / (4.0 * sigma)
                    } else {
                        0.0
                    });
                }
            }
            match kind {
                Sample::Uniform => figures.uniform += 1,
                Sample::Extra => figures.extra += 1,
            }
        }
    }
    figures
}

/// A sampled patch's figures: its largest |h(n) − h(c)| over its vertices, and its own σ, the
/// largest `omitted_rms` at n over the cells its vertices lie in.
#[derive(Debug, Clone, Copy)]
struct PatchFigure {
    largest_gap: Metres,
    own_sigma: Metres,
}

/// [`PatchFigure`] of `patch`, of level n, asserting the bound at every vertex.
fn sample_patch(
    synth: &Synthesiser<'_, CoarseField>,
    patch: PatchKey,
    bound: f64,
    complete: u8,
) -> PatchFigure {
    let field = synth.field();
    let level = field.header().level().get();
    let n = band(patch.level());
    let mut cache = SynthCache::new();
    let baked = synth
        .bake_patch(&mut cache, patch)
        .expect("the field holds every cell");
    let mut sigmas: BTreeMap<PatchKey, f64> = BTreeMap::new();
    let (mut largest, mut sigma) = (0.0_f64, 0.0_f64);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let dir = patch.vertex_dir(x, y);
            let finest = synth
                .height_at(&mut cache, dir, band(complete))
                .expect("the field holds every cell")
                .height_m;
            let gap = (baked.sample(x, y).height_m - finest).abs();
            assert!(
                gap <= bound,
                "{patch:?} ({x}, {y}): |h(n) − h({complete})| = {gap} m over the bound {bound} m"
            );
            largest = largest.max(gap);
            let cell = PatchKey::containing(level, dir).expect("a coarse level is a level");
            let own = *sigmas.entry(cell).or_insert_with(|| {
                synth
                    .omitted_rms(cell, n)
                    .expect("the field holds every cell")
                    .value()
            });
            sigma = sigma.max(own);
        }
    }
    PatchFigure {
        largest_gap: Metres::new(largest),
        own_sigma: Metres::new(sigma),
    }
}

/// The measured figures at band level `n`, of bound `bound` and body σ `sigma`, from its points'
/// ratios over the bound and over 4σ of their cells, its worst gap and its patches.
fn level_figures(
    synth: &Synthesiser<'_, CoarseField>,
    n: u8,
    bound: f64,
    sigma: BodySigma,
    (over_bound, over_4sigma): (&mut [f64], &mut [f64]),
    gap: WorstGap,
    patches: &[PatchFigure],
) -> Measured {
    let mut maxima = Vec::with_capacity(patches.len());
    let (mut over_own, mut over_body) = (0, 0);
    for patch in patches {
        let largest = patch.largest_gap.value();
        maxima.push(largest / bound);
        over_own += usize::from(largest > 4.0 * patch.own_sigma.value());
        over_body += usize::from(largest > 4.0 * sigma.largest.value());
    }
    maxima.sort_unstable_by(f64::total_cmp);
    let median = maxima[(maxima.len() - 1) / 2];
    let max = maxima[maxima.len() - 1];
    let worst = (gap.ratio > 0.0).then(|| {
        let (term, share) = worst_term(synth, gap.dir, n, gap.finer, gap.signed_gap);
        Worst {
            finer: gap.finer,
            term,
            share,
        }
    });
    Measured {
        p999_over_bound: nearest_rank(over_bound, QUANTILE),
        p999_over_4sigma_cell: nearest_rank(over_4sigma, QUANTILE),
        worst_over_bound: gap.ratio,
        worst,
        patches: patches.len(),
        patch_max_over_bound: max,
        patch_median_over_bound: median,
        patch_spread: (median > 0.0).then(|| max / median),
        patches_over_4sigma: over_own,
        patches_over_4sigma_body: over_body,
    }
}

/// Measures `world` (the module documentation).
fn measure(world: &SampleWorld) -> WorldFigures {
    let name = world.name();
    let field = world.field();
    let header = field.header();
    let synth = Synthesiser::new(field, DETAIL);
    let complete = (0..=MAX_LEVEL)
        .find(|&n| synth.level_set(band(n)).shortest_wavelength().value() <= BAND_LIMIT_M)
        .expect("the deepest level resolves the band limit");
    for n in complete..=MAX_LEVEL {
        assert_eq!(
            bits(synth.level_bound_m(band(n))),
            bits(0.0),
            "{name}: the bound is zero from the band limit's level {complete}, not at {n}"
        );
    }
    let bounds: Vec<f64> = (0..complete)
        .map(|n| synth.level_bound_m(band(n)))
        .collect();
    for (n, &bound) in bounds.iter().enumerate() {
        assert!(
            bound.is_finite() && bound > 0.0,
            "{name}: the bound at {n} is {bound} m"
        );
    }
    let (sigma, (mut points, sampled)) = both(
        || body_sigma(field, complete),
        || {
            let draw = draw(field, complete, &mut sampler(name));
            let points = sample_points(name, &synth, &bounds, complete, &draw.cells);
            let every: Vec<PatchKey> = draw.patches.iter().flatten().copied().collect();
            let mut read = par_map(&every, |&patch| {
                sample_patch(&synth, patch, bounds[usize::from(patch.level())], complete)
            })
            .into_iter();
            let patches: Vec<Vec<PatchFigure>> = draw
                .patches
                .iter()
                .map(|level| read.by_ref().take(level.len()).collect())
                .collect();
            (points, patches)
        },
    );
    let measured = (0..complete)
        .map(|n| {
            let k = usize::from(n);
            let quantiles = (
                &mut points.over_bound[k][..],
                &mut points.over_4sigma[k][..],
            );
            let worst = points.worst[k];
            level_figures(
                &synth,
                n,
                bounds[k],
                sigma[k],
                quantiles,
                worst,
                &sampled[k],
            )
        })
        .collect();
    let levels = (0..=complete)
        .map(|n| {
            let (low, high) = synth.bounds().contribution_range_m(band(n));
            LevelRow {
                bound: Metres::new(synth.level_bound_m(band(n))),
                sigma: sigma[usize::from(n)],
                contribution_low: Metres::new(low),
                contribution_high: Metres::new(high),
            }
        })
        .collect();
    let (elevation_lowest, elevation_highest) = elevation_range(field);
    WorldFigures {
        name,
        radius_m: header.radius().value(),
        coarse_level: header.level().get(),
        finest_level: finest_level(header.figure().equatorial_radius_m),
        complete_level: complete,
        elevation_lowest,
        elevation_highest,
        uniform_points: points.uniform,
        extra_points: points.extra,
        levels,
        measured,
    }
}

/// The threads a world's points and patches are read on, natively.
const THREADS: usize = 4;

/// `f` of each of `items`, in their order: natively on [`THREADS`] threads, item i on thread
/// i mod [`THREADS`], so that each thread takes its share of every level's patches; on
/// WebAssembly, which has no threads, in turn.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    if cfg!(target_family = "wasm") {
        return items.iter().map(f).collect();
    }
    let threads = THREADS.min(items.len()).max(1);
    let f = &f;
    let shares: Vec<Vec<(usize, R)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                scope.spawn(move || {
                    items
                        .iter()
                        .enumerate()
                        .skip(t)
                        .step_by(threads)
                        .map(|(i, item)| (i, f(item)))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    let mut mapped: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    for (i, r) in shares.into_iter().flatten() {
        mapped[i] = Some(r);
    }
    mapped
        .into_iter()
        .map(|r| r.expect("every item is mapped once"))
        .collect()
}

/// `a()` and `b()`, natively `a` on a thread of its own beside `b`; on WebAssembly, which has no
/// threads, `a` first.
fn both<A: Send, B>(a: impl FnOnce() -> A + Send, b: impl FnOnce() -> B) -> (A, B) {
    if cfg!(target_family = "wasm") {
        let first = a();
        return (first, b());
    }
    std::thread::scope(|scope| {
        let handle = scope.spawn(a);
        let second = b();
        let first = handle
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        (first, second)
    })
}

/// Every sample world measured, in `sample_worlds()`'s order, each world's figures printed as it
/// is done ([`report`]): natively on a thread each.
fn measure_every_world() -> Vec<WorldFigures> {
    let worlds = sample_worlds();
    let measured = |world: &SampleWorld| {
        let figures = measure(world);
        report(&figures);
        figures
    };
    if cfg!(target_family = "wasm") {
        return worlds.iter().map(measured).collect();
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = worlds
            .iter()
            .map(|world| scope.spawn(move || measured(world)))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// `value` as the fixture writes a number: its shortest form that reads back to the bit, which
/// Prettier leaves as it is.
fn number(value: f64) -> String {
    assert!(
        value.is_finite(),
        "the fixture holds finite figures, not {value}"
    );
    format!("{value:?}")
}

/// [`number`] of a length in metres.
fn metres(value: Metres) -> String {
    number(value.value())
}

/// The fixture's `null`, for a figure a level does not have.
fn null() -> String {
    "null".to_owned()
}

/// Writes `rows` as a JSON object at `indent`, one key a line, followed by a comma unless it is
/// `last`.
fn object(line: &mut impl FnMut(usize, &str), indent: usize, rows: &[(&str, String)], last: bool) {
    line(indent, "{");
    for (r, (key, value)) in rows.iter().enumerate() {
        let comma = if r + 1 < rows.len() { "," } else { "" };
        line(indent + 2, &format!("\"{key}\": {value}{comma}"));
    }
    line(indent, if last { "}" } else { "}," });
}

/// The fixture's text for `worlds`, one key a line, as Prettier lays JSON out.
fn fixture(worlds: &[WorldFigures]) -> String {
    let mut text = String::new();
    let mut line = |indent: usize, s: &str| {
        writeln!(text, "{:indent$}{s}", "").expect("writing to a String cannot fail");
    };
    line(0, "{");
    line(
        2,
        &format!("\"description\": \"{DESCRIPTION} {}\",", version_note()),
    );
    line(
        2,
        &format!("\"generator_version\": {},", GENERATOR_VERSION.get()),
    );
    line(2, &format!("\"detail_seed\": \"{:#018x}\",", DETAIL.get()));
    line(2, "\"worlds\": [");
    for (w, world) in worlds.iter().enumerate() {
        line(4, "{");
        line(6, &format!("\"name\": \"{}\",", world.name));
        line(6, &format!("\"radius_m\": {},", number(world.radius_m)));
        line(6, &format!("\"coarse_level\": {},", world.coarse_level));
        line(6, &format!("\"finest_level\": {},", world.finest_level));
        line(6, &format!("\"complete_level\": {},", world.complete_level));
        line(
            6,
            &format!(
                "\"elevation_lowest_m\": {},",
                metres(world.elevation_lowest)
            ),
        );
        line(
            6,
            &format!(
                "\"elevation_highest_m\": {},",
                metres(world.elevation_highest)
            ),
        );
        line(6, &format!("\"uniform_points\": {},", world.uniform_points));
        line(6, &format!("\"extra_points\": {},", world.extra_points));
        line(6, "\"levels\": [");
        for (n, row) in world.levels.iter().enumerate() {
            let rows = [
                ("level", n.to_string()),
                ("bound_m", metres(row.bound)),
                ("sigma_m", metres(row.sigma.largest)),
                ("sigma_rms_m", metres(row.sigma.rms)),
                ("contribution_low_m", metres(row.contribution_low)),
                ("contribution_high_m", metres(row.contribution_high)),
            ];
            object(&mut line, 8, &rows, n + 1 == world.levels.len());
        }
        line(6, "],");
        line(6, "\"measured\": [");
        for (n, m) in world.measured.iter().enumerate() {
            let rows = [
                ("level", n.to_string()),
                ("p999_over_bound", number(m.p999_over_bound)),
                ("p999_over_4sigma_cell", number(m.p999_over_4sigma_cell)),
                ("worst_over_bound", number(m.worst_over_bound)),
                (
                    "worst_finer_level",
                    m.worst.map_or_else(null, |w| w.finer.to_string()),
                ),
                (
                    "worst_term",
                    m.worst
                        .map_or_else(null, |w| format!("\"{}\"", w.term.name())),
                ),
                (
                    "worst_term_share",
                    m.worst.map_or_else(null, |w| number(w.share)),
                ),
                ("patches", m.patches.to_string()),
                ("patch_max_over_bound", number(m.patch_max_over_bound)),
                ("patch_median_over_bound", number(m.patch_median_over_bound)),
                ("patch_spread", m.patch_spread.map_or_else(null, number)),
                ("patches_over_4sigma", m.patches_over_4sigma.to_string()),
                (
                    "patches_over_4sigma_body",
                    m.patches_over_4sigma_body.to_string(),
                ),
            ];
            object(&mut line, 8, &rows, n + 1 == world.measured.len());
        }
        line(6, "]");
        line(4, if w + 1 == worlds.len() { "}" } else { "}," });
    }
    line(2, "]");
    line(0, "}");
    text
}

/// The repository's root, two levels above this crate.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lies two levels below the repository")
        .to_path_buf()
}

/// Prints `world`'s figures a level a line, for the plan's record, in one write so that the worlds'
/// threads do not interleave their lines.
fn report(world: &WorldFigures) {
    let mut text = format!(
        "{}: R {} m, L {}, finest {}, complete {}, elevations {} to {} m, {} uniform and {} \
         extra points\n",
        world.name,
        world.radius_m,
        world.coarse_level,
        world.finest_level,
        world.complete_level,
        world.elevation_lowest.value(),
        world.elevation_highest.value(),
        world.uniform_points,
        world.extra_points
    );
    for (n, (row, m)) in world.levels.iter().zip(&world.measured).enumerate() {
        let worst = m.worst.map_or_else(
            || "none".to_owned(),
            |w| format!("at {} ({} {:.2})", w.finer, w.term.name(), w.share),
        );
        let spread = m
            .patch_spread
            .map_or_else(|| "none".to_owned(), |s| format!("{s:.2}"));
        writeln!(
            text,
            "  {n:>2}: ε {:>10.3} m, σ {:>9.3} m (rms {:>8.3}); p99.9/ε {:.3}, p99.9/4σ {:.3}, \
             worst/ε {:.3} {worst}; patches {}: max/ε {:.3}, median/ε {:.3}, spread {spread}, \
             > 4σ own {}, > 4σ body {}",
            row.bound.value(),
            row.sigma.largest.value(),
            row.sigma.rms.value(),
            m.p999_over_bound,
            m.p999_over_4sigma_cell,
            m.worst_over_bound,
            m.patches,
            m.patch_max_over_bound,
            m.patch_median_over_bound,
            m.patches_over_4sigma,
            m.patches_over_4sigma_body,
        )
        .expect("writing to a String cannot fail");
    }
    eprint!("{text}");
}

#[test]
#[ignore = "slow: 10⁴ points, four patches a level and the body's σ on each of the six sample \
            worlds, the reference Earth's coarse pass at level 8 among them"]
fn the_level_bound_holds_on_every_sample_world_and_the_fixture_holds_its_figures() {
    let worlds = measure_every_world();
    assert_eq!(worlds.len(), 6, "every sample world is measured");
    let text = fixture(&worlds);
    let path = repository().join(FIXTURE);
    match Mode::from_env() {
        Mode::Compare => {
            // A checkout that converts line ends (Git for Windows' default) holds the same
            // figures.
            let committed = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| {
                    panic!(
                        "cannot read {}: {e}; run this test with HYPERION_BLESS=1",
                        path.display()
                    )
                })
                .replace("\r\n", "\n");
            if let Some((k, (old, new))) = committed
                .lines()
                .zip(text.lines())
                .enumerate()
                .find(|(_, (old, new))| old != new)
            {
                panic!(
                    "{} differs from the figures at line {}: {old:?}, measured {new:?}; if the \
                     generator moved on purpose, run this test with HYPERION_BLESS=1 and commit it",
                    path.display(),
                    k + 1
                );
            }
            assert_eq!(
                committed,
                text,
                "{} differs from the figures in its length; run this test with HYPERION_BLESS=1",
                path.display()
            );
        }
        Mode::Bless => {
            let dir = path.parent().expect("the fixture lies in a directory");
            std::fs::create_dir_all(dir)
                .unwrap_or_else(|e| panic!("cannot create {}: {e}", dir.display()));
            std::fs::write(&path, &text)
                .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
        }
        Mode::BlessForbidden => panic!(
            "refusing to write {} under CI: the fixture is blessed on a developer machine",
            path.display()
        ),
    }
}
