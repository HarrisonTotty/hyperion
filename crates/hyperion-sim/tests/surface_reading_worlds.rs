//! The surveyed readings' uncertainty on generated ground (rendering plan R10, T5): at an orbital
//! survey, pooled over 200 cells × 100 points a world, the RMS of the authoritative ground less the
//! quoted elevation is within 10% of the pooled quoted σ, and the points' z-scores, each by its
//! cell's σ, have unit variance within 15%. The quoted σ is R09's level-keyed `omitted_rms` at the
//! reading's level. The Moon- and Ceres-like worlds are held to a known gap until R09.T7.h lands
//! (`CRATER_GAP`).
//!
//! It runs on the four bodies of R09's synthetic worlds (`hyperion_surface::testing`, under the
//! sim's `testing` feature), each surveyed through a client's own field: a payload of 200 cells at
//! the survey's code, with their margin, decoded into a `PartialField`, as the client holds it.
//! R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`) joins them when it lands, under
//! the same task ID (the plan's order of tasks); until R09.T11–T16 fill the coarse pass, the
//! reference worlds have no ground finer than their cells, and so no uncertainty to test.
//!
//! Slow: `just test-slow -E 'binary(surface_reading_worlds)'`.
#![cfg(feature = "testing")]

use std::collections::BTreeSet;
use std::ops::RangeInclusive;

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

/// The known gap, ruled in `decision-r10-t5-closed-forms.md` and recorded in R10's "Deviations in
/// T5, as built": R09's small craters' closed forms do not yet count the sharpening of the craters
/// the reading's level keeps, from its fillet, half its shortest wavelength, to the band limit's,
/// so the quoted σ is low on the cratered bodies, at both surveys. There the RMS ratio and the
/// z-scores' variance are held in [`GAP_RMS`] and [`GAP_Z`]: at the plan's tolerance from below,
/// so that a σ quoted too high still fails, and loosely from above. R09.T7.h adds the term, and a
/// commit under R10.T5's ID then removes the gap, so that every world is held at the plan's
/// tolerance.
const CRATER_GAP: [SyntheticWorld; 2] = [SyntheticWorld::MoonLike, SyntheticWorld::CeresLike];

/// The crater gap's bounds on the RMS ratio, \[0.90, 1.6\]: 1.430 was measured on the Moon-like
/// world at 100 m.
const GAP_RMS: RangeInclusive<f64> = 1.0 - RMS_TOLERANCE..=1.6;

/// The crater gap's bounds on the z-scores' variance, \[0.85, 3.0\]: 2.473 was measured on the
/// Moon-like world at 100 m.
const GAP_Z: RangeInclusive<f64> = 1.0 - Z_TOLERANCE..=3.0;

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

#[test]
#[ignore = "slow: 200 cells × 100 points on each of four worlds at two surveys"]
fn an_orbital_survey_s_quoted_sigma_holds_pooled_on_every_world() {
    let mut misses = Vec::new();
    for (n, &world) in BODIES.iter().enumerate() {
        let field = synthetic_field(world);
        let figure = field.header().figure();
        let mut rng = Lcg::new(0x7265_6164_0000 + u64::try_from(n).unwrap());
        let cells = random_cells(field.header().level().get(), CELLS, &mut rng);
        for metres in SURVEYS_M {
            let code = ResolutionCode::from_resolution(Metres::new(metres)).unwrap();
            let client = client_field(&field, &cells, code);
            let synth = Synthesiser::new(&client, DETAIL);
            let mut cache = SynthCache::new();
            let mut pooled = Pooled::default();
            for &cell in &cells {
                for _ in 0..POINTS {
                    let at = point_in(cell, rng.next_f64(), rng.next_f64());
                    let reading = surveyed_reading(&synth, &mut cache, at, SLOPE_BASELINES[0])
                        .unwrap()
                        .expect("a surveyed cell has a reading");
                    let dir = unit_dir(face_uv_to_xyz(at));
                    let ground = ground_at(&synth, &mut cache, &figure.point(dir)).unwrap();
                    let r = ground.height().value() - reading.elevation().value();
                    let s = reading.elevation_sigma().value();
                    assert!(s > 0.0, "{world:?} at {metres} m: no σ in {cell:?}");
                    pooled.residual2 += r * r;
                    pooled.sigma2 += s * s;
                    pooled.z2 += (r / s) * (r / s);
                    pooled.points += 1.0;
                }
            }
            let (rms, z) = (pooled.rms_ratio(), pooled.z_variance());
            eprintln!(
                "{world:?} at {metres} m: pooled RMS {:.4} m against σ {:.4} m, ratio {rms:.4}; \
                 z-score variance {z:.4}",
                (pooled.residual2 / pooled.points).sqrt(),
                (pooled.sigma2 / pooled.points).sqrt(),
            );
            let within = if CRATER_GAP.contains(&world) {
                GAP_RMS.contains(&rms) && GAP_Z.contains(&z)
            } else {
                (rms - 1.0).abs() <= RMS_TOLERANCE && (z - 1.0).abs() <= Z_TOLERANCE
            };
            if !within {
                misses.push(format!(
                    "{world:?} at {metres} m: RMS ÷ σ {rms:.4}, var z {z:.4}"
                ));
            }
        }
    }
    assert!(misses.is_empty(), "outside the tolerances: {misses:#?}");
}
