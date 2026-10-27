//! The surveyed readings' uncertainty on generated ground (rendering plan R10, T5): at an orbital
//! survey, pooled over 200 cells × 100 points a world, the RMS of the authoritative ground less the
//! quoted elevation is within 10% of the pooled quoted σ, and the points' z-scores, each by its
//! cell's σ, have unit variance within 15%. The quoted σ is R09's level-keyed `omitted_rms` at the
//! reading's level, whose small craters count the sharpening of the octaves the level keeps
//! (R09.T7.h). One case, the synthetic Ceres-like world at the 1 km survey, holds its z-scores'
//! variance to a wider bound for its heavy-tailed craters (`CRATER_GAP`), and three sample-world
//! cases, the reference Earth's and the locked Earth's, to one for the closed forms' cell styles
//! (`STYLE_GAP`).
//!
//! Each world is surveyed through a client's own field: a payload of 200 cells at the survey's code,
//! with their margin, decoded into a `PartialField`, as the client holds it. The worlds are the
//! four bodies of R09's synthetic worlds (`hyperion_surface::testing`, under the sim's `testing`
//! feature), in one test, and R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`): the
//! reference Earth, Mars, Moon and Ceres, the icy moon `europa_like` and the locked
//! `locked_earth_like`, each run through the whole coarse pass under `reference::SEED`, each in a
//! test of its own, since under nextest each test pays for the passes of the worlds it reads. A
//! sample world's cells and points come from a sampler seeded by its name ([`sampler`]).
//!
//! Every pooled case is slow: `just test-slow -E 'binary(surface_reading_worlds)'`, or by name with
//! `cargo nextest run -p hyperion-sim --features testing --cargo-profile slow-test --profile slow
//! --test surface_reading_worlds --run-ignored only`. The one fast test checks that every sample
//! world has its slow test.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the readings' tests use only the sample worlds of the shared helpers"
)]
mod common;

use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use common::surface_worlds::{SampleWorld, sample_world, sample_worlds};
use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;
use hyperion_surface::cube::{Face, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use hyperion_surface::field::{
    CoarseField, Cover, CoverRange, FieldView, PartialField, ResolutionCode, cell_index,
};
use hyperion_surface::ground::ground_at;
use hyperion_surface::reading::{SLOPE_BASELINES, surveyed_reading};
use hyperion_surface::synth::{SynthCache, Synthesiser};
use hyperion_surface::testing::{SyntheticWorld, synthetic_field};
use hyperion_surface::wire::{decode_payload, encode_payload};
use hyperion_testkit::lcg::Lcg;

/// The detail seed the worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x7265_6164_696e_6773);

/// The seed of a sample world's cells and points, into which its name is folded ([`sampler`]).
const SAMPLES: u64 = 0x7265_6164_7361_6d70;

/// The FNV-1a 64 prime (Fowler, Noll and Vo), the testkit's `f64_digest`'s, which folds a world's
/// name into [`SAMPLES`].
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The sample worlds, each with a slow test of its own.
const EARTH: &str = "earth_like";
const MARS: &str = "mars_like";
const MOON: &str = "moon_like";
const CERES: &str = "ceres_like";
const EUROPA: &str = "europa_like";
const LOCKED_EARTH: &str = "locked_earth_like";

/// The sample worlds the slow tests run, one each.
const TESTED: [&str; 6] = [EARTH, MARS, MOON, CERES, EUROPA, LOCKED_EARTH];

/// The bodies of the synthetic worlds, which have ground finer than their cells.
const BODIES: [SyntheticWorld; 4] = [
    SyntheticWorld::EarthLike,
    SyntheticWorld::MarsLike,
    SyntheticWorld::MoonLike,
    SyntheticWorld::CeresLike,
];

/// The orbital surveys, metres: a stereo DEM's 100 m (LOLA's and CTX's scale) and an altimeter's
/// 1 km (MOLA's).
const SURVEYS_M: [f64; 2] = [100.0, 1_000.0];

/// Cells surveyed a world, and points read a cell.
const CELLS: usize = 200;
const POINTS: usize = 100;

/// The largest miss of the pooled RMS from the pooled σ, and of the z-scores' variance from 1.
const RMS_TOLERANCE: f64 = 0.10;
const Z_TOLERANCE: f64 = 0.15;

/// The one case whose z-scores' variance is held to [`GAP_Z`] rather than the plan's 15%: the
/// Ceres-like world at the 1 km survey (`SURVEYS_M[1]`), by main's ruling of 2026-10-14 on R09.T7.h's
/// figures, under `decision-r10-t5-closed-forms.md`'s deferred finding on heavy-tailed crater
/// checks, and recorded in R10's "Deviations in T5, as built". Its RMS ratio is held at the plan's
/// 10%, as every other case is in both.
///
/// The cause: an octave's E[g²] is carried by its youngest few tenths of a per cent of craters, so
/// 200 × 100 points sample a few dozen; the closed form agrees with a 10⁴-point measurement within
/// sampling (kept sharpening 15.0 ± 3.5 vs 15.4 m², left-out octaves 116 ± 26 vs 108 m²; R09's
/// "Deviations in T7.h, as built"). A check weighted by crater count, or a larger cratered sample,
/// would retire it (R10.T15's verification pass).
const CRATER_GAP: (SyntheticWorld, usize) = (SyntheticWorld::CeresLike, 1);

/// The crater gap's bounds on the z-scores' variance, \[0.85, 1.25\]: the plan's tolerance from
/// below, so that a σ quoted too high still fails, and from above 1.176, measured with R09.T7.h,
/// with headroom for a change of seed.
const GAP_Z: RangeInclusive<f64> = 1.0 - Z_TOLERANCE..=1.25;

/// The sample-world cases, each a world's name and a survey's index in [`SURVEYS_M`], whose
/// z-scores' variance is held to [`STYLE_GAP_Z`] rather than the plan's 15%: the reference Earth at
/// both surveys and the locked Earth at 100 m. Since R09.T6.f (2026-10-27, generator version 23)
/// they measure 1.1751, 1.3568 and 1.1635, each with its RMS ratio within 1% of 1 (the reference
/// Earth's at 1000 m 1.0072) and held at the plan's 10%; before it, 1.1772, 1.1584 and 1.1658.
///
/// The ground a level leaves out is wider than the cell's σ at some points of the Earths, which
/// raises the z-scores' variance where σ is small and barely moves the pooled RMS. The gap was
/// named for decision-r10-t2-weights §2's cause, R09's level-keyed forms reading each cell's own
/// relief style while the synthesis interpolates the styles, a small effect that stands
/// (decision-r09-channel-term §6). Since R09.T6.f the reference Earth's 1000 m case has a second
/// cause: the synthesis holds a coastal cell's trunk to its base level, which shrinks the cell's own
/// closed-form channel term, while its neighbours' trunks reaching into it, which the closed forms
/// omit, and the floor's flats at the sea stay realised (plan R09's "Deferred findings (2026-10-13
/// rule) (R09.T6.f)"). R10.T15.b re-records these figures at 24 (plan R10's "Deferred findings
/// (2026-10-13 rule) (R10.T5's `sample_worlds()` follow-up)").
const STYLE_GAP: [(&str, usize); 3] = [(EARTH, 0), (EARTH, 1), (LOCKED_EARTH, 0)];

/// The style gap's bounds on the z-scores' variance, \[0.85, 1.36\]: the plan's tolerance from
/// below, so that a σ quoted too high still fails, and from above the largest measured, 1.3568
/// (the reference Earth at 1000 m since R09.T6.f), rounded up to the next 0.01 (main's ruling,
/// 2026-10-27, by decision-r10-t2-weights §2.4's rule for a pin; 1.18 before).
const STYLE_GAP_Z: RangeInclusive<f64> = 1.0 - Z_TOLERANCE..=1.36;

/// `count` distinct cells of `level` at random.
fn random_cells(level: u8, count: usize, rng: &mut Lcg) -> Vec<PatchKey> {
    let side = 1_u64 << level;
    let mut cells = BTreeSet::new();
    while cells.len() < count {
        let face = Face::ALL[usize::try_from(rng.next_below(6)).unwrap()];
        let i = u32::try_from(rng.next_below(side)).unwrap();
        let j = u32::try_from(rng.next_below(side)).unwrap();
        cells.insert(PatchKey::new(face, level, i, j).unwrap());
    }
    cells.into_iter().collect()
}

/// The face coordinates of the point at (`x`, `y`) in [0, 1]² of `cell`.
fn point_in(cell: PatchKey, x: f64, y: f64) -> FaceUv {
    let side = f64::from(1_u32 << cell.level());
    FaceUv {
        face: cell.face(),
        u: st_to_uv((f64::from(cell.i()) + x) / side),
        v: st_to_uv((f64::from(cell.j()) + y) / side),
    }
}

/// A client's field of `field` from a payload of `cells` surveyed at `code`, with their margin.
fn client_field(field: &CoarseField, cells: &[PatchKey], code: ResolutionCode) -> PartialField {
    let mut indices: Vec<u32> = cells.iter().map(|&c| cell_index(c)).collect();
    indices.sort_unstable();
    let cover = Cover::from_ranges(
        indices
            .iter()
            .map(|&i| CoverRange::new(i, i + 1, code).unwrap()),
    )
    .unwrap();
    let blocks = decode_payload(&encode_payload(field, &cover, None)).unwrap();
    let mut client = PartialField::new(blocks[0].header().unwrap().clone());
    for block in &blocks {
        client.insert(block).unwrap();
    }
    client
}

/// The sampler of the sample world named `name`: an [`Lcg`] seeded with FNV-1a 64 over the
/// name's bytes, from [`SAMPLES`] in place of the offset basis, so that a world's cells and points
/// depend on its name alone.
fn sampler(name: &str) -> Lcg {
    Lcg::new(name.bytes().fold(SAMPLES, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    }))
}

/// The pooled figures of one world and survey.
#[derive(Debug, Default)]
struct Pooled {
    residual2: f64,
    sigma2: f64,
    z2: f64,
    points: f64,
}

impl Pooled {
    /// The pooled RMS of the ground less the quoted value over the pooled σ.
    fn rms_ratio(&self) -> f64 {
        (self.residual2 / self.sigma2).sqrt()
    }

    /// The z-scores' variance about zero.
    fn z_variance(&self) -> f64 {
        self.z2 / self.points
    }
}

/// The pooled figures of `field` surveyed at `metres` over `cells`, [`POINTS`] a cell at random
/// from `rng`, printed under `label`.
///
/// # Panics
///
/// If a point of a surveyed cell has no reading or no σ, naming `label`, the survey and the cell.
fn pool_survey(
    field: &CoarseField,
    cells: &[PatchKey],
    metres: f64,
    rng: &mut Lcg,
    label: &str,
) -> Pooled {
    let figure = field.header().figure();
    let code = ResolutionCode::from_resolution(Metres::new(metres)).unwrap();
    let client = client_field(field, cells, code);
    let synth = Synthesiser::new(&client, DETAIL);
    let mut cache = SynthCache::new();
    let mut pooled = Pooled::default();
    for &cell in cells {
        for _ in 0..POINTS {
            let at = point_in(cell, rng.next_f64(), rng.next_f64());
            let reading = surveyed_reading(&synth, &mut cache, at, SLOPE_BASELINES[0])
                .unwrap()
                .unwrap_or_else(|| panic!("{label} at {metres} m: no reading in {cell:?}"));
            let dir = unit_dir(face_uv_to_xyz(at));
            let ground = ground_at(&synth, &mut cache, &figure.point(dir)).unwrap();
            let r = ground.height().value() - reading.elevation().value();
            let s = reading.elevation_sigma().value();
            assert!(s > 0.0, "{label} at {metres} m: no σ in {cell:?}");
            pooled.residual2 += r * r;
            pooled.sigma2 += s * s;
            pooled.z2 += (r / s) * (r / s);
            pooled.points += 1.0;
        }
    }
    eprintln!(
        "{label} at {metres} m: pooled RMS {:.4} m against σ {:.4} m, ratio {:.4}; z-score \
         variance {:.4}",
        (pooled.residual2 / pooled.points).sqrt(),
        (pooled.sigma2 / pooled.points).sqrt(),
        pooled.rms_ratio(),
        pooled.z_variance(),
    );
    pooled
}

#[test]
#[ignore = "slow: 200 cells × 100 points on each of four worlds at two surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_every_world() {
    let mut misses = Vec::new();
    for (n, &world) in BODIES.iter().enumerate() {
        let field = synthetic_field(world);
        let mut rng = Lcg::new(0x7265_6164_0000 + u64::try_from(n).unwrap());
        let cells = random_cells(field.header().level().get(), CELLS, &mut rng);
        for (survey, metres) in SURVEYS_M.into_iter().enumerate() {
            let pooled = pool_survey(&field, &cells, metres, &mut rng, &format!("{world:?}"));
            let (rms, z) = (pooled.rms_ratio(), pooled.z_variance());
            let z_within = if (world, survey) == CRATER_GAP {
                GAP_Z.contains(&z)
            } else {
                (z - 1.0).abs() <= Z_TOLERANCE
            };
            let within = (rms - 1.0).abs() <= RMS_TOLERANCE && z_within;
            if !within {
                misses.push(format!(
                    "{world:?} at {metres} m: RMS ÷ σ {rms:.4}, var z {z:.4}"
                ));
            }
        }
    }
    assert!(misses.is_empty(), "outside the tolerances: {misses:#?}");
}

/// Holds the pooled σ of `world` at both orbital surveys to the plan's tolerances, 10% in the RMS
/// ratio and 15% in the z-scores' variance, the [`STYLE_GAP`] cases' variance to [`STYLE_GAP_Z`].
fn assert_the_sigma_holds(world: &SampleWorld) {
    let name = world.name();
    let field = world.field();
    let mut rng = sampler(name);
    let cells = random_cells(field.header().level().get(), CELLS, &mut rng);
    let mut misses = Vec::new();
    for (survey, metres) in SURVEYS_M.into_iter().enumerate() {
        let pooled = pool_survey(field, &cells, metres, &mut rng, name);
        let (rms, z) = (pooled.rms_ratio(), pooled.z_variance());
        let z_within = if STYLE_GAP.contains(&(name, survey)) {
            STYLE_GAP_Z.contains(&z)
        } else {
            (z - 1.0).abs() <= Z_TOLERANCE
        };
        if (rms - 1.0).abs() > RMS_TOLERANCE || !z_within {
            misses.push(format!(
                "{name} at {metres} m: RMS ÷ σ {rms:.4}, var z {z:.4}"
            ));
        }
    }
    assert!(misses.is_empty(), "outside the tolerances: {misses:#?}");
}

#[test]
fn every_sample_world_has_a_reading_test() {
    let mut tested = TESTED.to_vec();
    let mut names: Vec<&str> = sample_worlds().iter().map(SampleWorld::name).collect();
    tested.sort_unstable();
    names.sort_unstable();
    assert_eq!(
        tested, names,
        "every sample world runs in exactly one of this binary's slow tests"
    );
}

#[test]
#[ignore = "slow: the reference Earth's coarse pass at level 8, and 200 cells × 100 points at two \
            surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_the_earth_like_sample_world() {
    assert_the_sigma_holds(sample_world(EARTH));
}

#[test]
#[ignore = "slow: 200 cells × 100 points at two surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_the_mars_like_sample_world() {
    assert_the_sigma_holds(sample_world(MARS));
}

#[test]
#[ignore = "slow: 200 cells × 100 points at two surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_the_moon_like_sample_world() {
    assert_the_sigma_holds(sample_world(MOON));
}

#[test]
#[ignore = "slow: 200 cells × 100 points at two surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_the_ceres_like_sample_world() {
    assert_the_sigma_holds(sample_world(CERES));
}

#[test]
#[ignore = "slow: 200 cells × 100 points at two surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_the_europa_like_sample_world() {
    assert_the_sigma_holds(sample_world(EUROPA));
}

#[test]
#[ignore = "slow: the reference locked Earth's coarse pass at level 8, and 200 cells × 100 points \
            at two surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_the_locked_earth_like_sample_world() {
    assert_the_sigma_holds(sample_world(LOCKED_EARTH));
}
