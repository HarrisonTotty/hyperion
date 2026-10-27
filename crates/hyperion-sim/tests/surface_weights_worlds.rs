//! The class weights on the sample worlds (rendering plan R10, T2): at the band levels n − 1 to
//! n − 6 from each world's finest level n, and at the class map's level (band level 4, Design note
//! 8), `weights_at` agrees with the classes `material_at` gives, counted over each texel's
//! footprint, to within ten percentage points in the mean over a thousand texels a level (a hundred
//! at the Mars-like world's class-map level, for its cost: [`MARS_CLASS_MAP_TEXELS`]); every
//! texel's bytes sum to 255; and the weights at a patch's 65 × 65 vertices never name more than
//! eight materials, a failure naming the patch.
//!
//! The worlds are R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`): the reference
//! Earth, Mars, Moon and Ceres, the icy moon `europa_like` and the locked `locked_earth_like`, each
//! run through the whole coarse pass under `reference::SEED`, behind the sim's `testing` feature.
//! Unlike R09's synthetic worlds, on which the surface crate's library tests hold the same
//! agreement first (`material::weights::tests`), their coarse pass places ice caps, frost and sea,
//! so the weights' ice and water rules are mixed at coarse levels here.
//!
//! **The agreement** is the surface crate's own measure, ported to its public interface. A texel of
//! band level m is the square of one vertex spacing about a vertex of the level's lattice, in the
//! face's (s, t): a cell of quadtree level m + 6 (R10's "Deviations in T2, as built"). Its weights
//! are `weights_at` at its centre, and its classes are `material_at`'s at the centres of an 8 × 8
//! grid of its equal steps at n − 1 to n − 6, and of a 64 × 64 grid at the class map's level. The
//! measure is the texel's total variation, ½ Σ |weight ÷ 255 − fraction| over the materials of
//! either: the share of the texel the weights give to the wrong material. Half the texels lie at
//! random points of the cells the weights find hardest ([`hard_kinds`]) and half at random over the
//! sphere. A failure names the level's five worst texels and their cells. Each level also prints
//! its wrong-dominant texels, whose largest weight's material is not the one counted at the most
//! points, by their pairs: recorded, not asserted (decision-r10-t2-weights §2.4.2).
//!
//! **The patch's palette** is the weights' materials over every vertex of a patch at its own level,
//! at the levels n − 1, n − 3, n − 6 and 4, [`PATCHES`] a level, about the hard cells. The
//! reference Earth's one level-4 patch of nine materials is checked too, and admitted at nine by
//! name ([`nine_material_patch`]); any other patch past eight fails, named.
//!
//! Each world's checks share its pass, so they are one test (decision-r09-t13-test-cost §1.2), and
//! every one is slow: `just test-slow -E 'binary(surface_weights_worlds)'`, or by name with
//! `cargo nextest run -p hyperion-sim --features testing --cargo-profile slow-test --profile slow
//! --test surface_weights_worlds --run-ignored only`. The one fast test checks that every sample
//! world has its slow test.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the weights' tests use only the sample worlds of the shared helpers"
)]
mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::surface_worlds::{SampleWorld, sample_world, sample_worlds};
use hyperion_base::math;
use hyperion_base::rng::DetailSeed;
use hyperion_surface::cube::{
    Face, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir, uv_to_st, xyz_to_face_uv,
};
use hyperion_surface::field::{CoarseField, FieldView, cell_at_index};
use hyperion_surface::geometry::finest_level;
use hyperion_surface::material::{
    ClassWeights, GroundMaterial, MAX_CLASSES, WEIGHT_TOTAL, material_at, weights_at,
};
use hyperion_surface::synth::relief::ReliefStyle;
use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
use hyperion_testkit::lcg::Lcg;

/// The detail seed the sample worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x7765_6967_6874_7377);

/// The seeds of the texels' points and of the patches' points, into which each world's name is
/// folded ([`sampler`]), so that the patches do not move with the texels' count.
const SAMPLES: u64 = 0x7765_6967_6874_7300;
const PATCH_SAMPLES: u64 = 0x7061_7463_6865_7300;

/// The FNV-1a 64 prime (Fowler, Noll and Vo), the testkit's `f64_digest`'s, which folds a world's
/// name into [`SAMPLES`] and [`PATCH_SAMPLES`].
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The sample worlds, each with a slow test of its own.
const EARTH: &str = "earth_like";
const MARS: &str = "mars_like";
const MOON: &str = "moon_like";
const CERES: &str = "ceres_like";
const EUROPA: &str = "europa_like";
const LOCKED_EARTH: &str = "locked_earth_like";

/// The worlds the slow tests run, one each.
const TESTED: [&str; 6] = [EARTH, MARS, MOON, CERES, EUROPA, LOCKED_EARTH];

/// The class map's band level (Design note 8).
const CLASS_MAP_LEVEL: u8 = 4;

/// The agreement the plan asks of the coarse weights in the mean over a level's texels: ten
/// percentage points.
const AGREEMENT: f64 = 0.10;

/// The texels a level, at n − 1 to n − 6 and at the class map's level: the plan's thousand, but for
/// the class map's level on the Mars-like world ([`MARS_CLASS_MAP_TEXELS`]).
const TEXELS: usize = 1_000;

/// The class map's texels on the Mars-like world: a hundred, by decision-r10-t2-weights §1
/// (2026-10-26), which settles R10.T2's deferred "slow cases' cost" for this binary. Its finest
/// points cost about 2–4 ms each away from a cache (this binary's 20-texel probe, and 1,536 s for
/// about 7.9 × 10⁵ points in its full run, 2026-10-26, both under load), so the plan's thousand
/// texels of 4,096 points would take some two to five hours alone.
const MARS_CLASS_MAP_TEXELS: usize = 100;

/// The points along a texel's side at n − 1 to n − 6 (64 a texel), and at the class map's level
/// (4,096 a texel): the plan's.
const LEVEL_SIDE: u32 = 8;
const CLASS_MAP_SIDE: u32 = 64;

/// The patches whose palette is checked at each of the levels n − 1, n − 3, n − 6 and 4, each about
/// a random point of a hard cell: the first cell of each kind, taken in turn.
const PATCHES: usize = 6;

/// The worst texels an [`Agreement`] keeps.
const WORST_KEPT: usize = 5;

/// A sampler of the world named `name`: an [`Lcg`] seeded with FNV-1a 64 over the name's bytes,
/// from `seed` ([`SAMPLES`] or [`PATCH_SAMPLES`]) in place of the offset basis, so that a world's
/// points depend on its name and field alone.
fn sampler(seed: u64, name: &str) -> Lcg {
    Lcg::new(name.bytes().fold(seed, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    }))
}

/// The point of `face` at (`s`, `t`).
fn uv_at(face: Face, s: f64, t: f64) -> FaceUv {
    FaceUv {
        face,
        u: st_to_uv(s),
        v: st_to_uv(t),
    }
}

/// The unit direction at (`x`, `y`) in [0, 1]² of `cell`.
fn point_in(cell: PatchKey, x: f64, y: f64) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    let s = (f64::from(cell.i()) + x) / side;
    let t = (f64::from(cell.j()) + y) / side;
    unit_dir(face_uv_to_xyz(uv_at(cell.face(), s, t)))
}

/// A random unit direction in `cell`.
fn random_point_in(cell: PatchKey, rng: &mut Lcg) -> [f64; 3] {
    let x = rng.next_f64();
    point_in(cell, x, rng.next_f64())
}

/// A random unit direction over the sphere (uniform in the cube, normalised).
fn random_dir(rng: &mut Lcg) -> [f64; 3] {
    unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0))
}

/// One texel of a band level: the vertex it is centred on and its side in the face's s and t.
#[derive(Debug, Clone, Copy)]
struct Texel {
    face: Face,
    s: f64,
    t: f64,
    side: f64,
    level: BandLevel,
}

impl Texel {
    /// The texel of `level` whose vertex is nearest the unit direction `dir`, kept off the face's
    /// edges.
    fn nearest(dir: [f64; 3], level: u8) -> Self {
        let at = xyz_to_face_uv(dir);
        let per_face = f64::from(64_u32 << level);
        let snap = |x: f64| ((x * per_face).round().clamp(1.0, per_face - 1.0)) / per_face;
        Self {
            face: at.face,
            s: snap(uv_to_st(at.u)),
            t: snap(uv_to_st(at.v)),
            side: 1.0 / per_face,
            level: BandLevel::new(level).expect("a band level"),
        }
    }

    /// The texel's centre.
    fn at(&self) -> FaceUv {
        uv_at(self.face, self.s, self.t)
    }

    /// The `n` × `n` points at the centres of the texel's equal steps.
    fn points(&self, n: u32) -> impl Iterator<Item = FaceUv> + '_ {
        let step = self.side / f64::from(n);
        (0..n).flat_map(move |a| {
            (0..n).map(move |b| {
                let s = self.s - 0.5 * self.side + (f64::from(a) + 0.5) * step;
                let t = self.t - 0.5 * self.side + (f64::from(b) + 0.5) * step;
                uv_at(self.face, s, t)
            })
        })
    }
}

/// The total variation between `weights` and the classes `material_at` gives at the texel's
/// `n` × `n` points, ½ Σ |weight ÷ 255 − fraction| over the materials of either; whether the
/// texel is mixed, more than one material among the weights or the points; and the material
/// counted at the most points, ties to the lower material.
fn total_variation(
    synth: &Synthesiser<'_, CoarseField>,
    cache: &mut SynthCache,
    texel: &Texel,
    weights: &ClassWeights,
    n: u32,
) -> (f64, bool, GroundMaterial) {
    let mut counts: BTreeMap<GroundMaterial, u32> = BTreeMap::new();
    for at in texel.points(n) {
        let m = material_at(synth, cache, at).expect("a whole field holds every cell");
        *counts.entry(m).or_default() += 1;
    }
    let all = f64::from(n * n);
    let materials: BTreeSet<GroundMaterial> = counts
        .keys()
        .copied()
        .chain(weights.iter().map(|(m, _)| m))
        .collect();
    let tv = 0.5
        * materials
            .iter()
            .map(|&m| {
                let w = f64::from(weights.weight(m)) / f64::from(WEIGHT_TOTAL);
                let f = f64::from(counts.get(&m).copied().unwrap_or(0)) / all;
                (w - f).abs()
            })
            .sum::<f64>();
    let dominant = counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(&m, _)| m)
        .expect("a texel has points");
    (tv, counts.len() > 1 || weights.len() > 1, dominant)
}

/// The cells of `field` under the rims of up to `count` of its coarse craters, spread through its
/// list: on each, the cell of the point whose chord from its centre is the rim radius, along a
/// tangent.
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
            // A tangent at the centre, and the arc whose chord is the rim radius.
            let e = if c[2].abs() < 0.9 {
                [-c[1], c[0], 0.0]
            } else {
                [0.0, -c[2], c[1]]
            };
            let n = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
            let half_chord = 0.25 * crater.diameter.value() / radius_m;
            let angle = 2.0 * math::asin(half_chord.min(1.0));
            let (sin, cos) = math::sin_cos(angle);
            let dir = unit_dir([0, 1, 2].map(|k| cos * c[k] + sin * e[k] / n));
            PatchKey::containing(level, dir).expect("a coarse level is a level")
        })
        .collect()
}

/// The cells of `field` whose ground the weights find hardest, each kind's in `cell_index` order:
/// the surface crate's five kinds (belts on dry land free of ice, partial ice, the sea under 300 m
/// deep, the channels' trunks on land, and the rims of up to [`TEXELS`] coarse craters) and, since
/// the sample worlds' passes place them, the cells of full ice.
fn hard_kinds(field: &CoarseField, synth: &Synthesiser<'_, CoarseField>) -> Vec<Vec<PatchKey>> {
    let level = field.header().level();
    let all: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).expect("a cell of the level"))
        .collect();
    let record = |k: PatchKey| field.cell(k).expect("a whole field holds every cell");
    let kind = |keep: &dyn Fn(PatchKey) -> bool| -> Vec<PatchKey> {
        all.iter().copied().filter(|&k| keep(k)).collect()
    };
    let belts = kind(&|k| {
        let c = record(k);
        ReliefStyle::of(c).belt() >= 0.5 && !c.is_under_water() && c.ice == 0
    });
    let partial_ice = kind(&|k| (1..u8::MAX).contains(&record(k).ice));
    let full_ice = kind(&|k| record(k).ice == u8::MAX);
    let coast = kind(&|k| {
        let c = record(k);
        c.is_under_water() && c.water_surface_mm - c.elevation_mm < 300_000
    });
    let mut network = synth.channels().network(field);
    let trunks = all
        .iter()
        .copied()
        .filter(|&k| {
            !record(k).is_under_water()
                && network
                    .segment(k)
                    .expect("a whole field holds every cell")
                    .is_some()
        })
        .collect();
    let rims = rim_cells(field, TEXELS);
    vec![belts, partial_ice, full_ice, coast, trunks, rims]
}

/// Up to `each` cells of each of `kinds`, spread through it, in turn.
fn spread(kinds: &[Vec<PatchKey>], each: usize) -> Vec<PatchKey> {
    kinds
        .iter()
        .flat_map(|keys| {
            let stride = (keys.len() / each.max(1)).max(1);
            keys.iter().copied().step_by(stride).take(each)
        })
        .collect()
}

/// `count` texels of band level `level`: half at random points of `hard`'s cells, taken evenly
/// through the list, and half at random over the sphere.
fn texels(hard: &[PatchKey], level: u8, count: usize, rng: &mut Lcg) -> Vec<Texel> {
    let mut out: Vec<Texel> = Vec::with_capacity(count);
    for k in 0..count / 2 {
        if hard.is_empty() {
            break;
        }
        let cell = hard[(k * hard.len()) / (count / 2).max(1) % hard.len()];
        out.push(Texel::nearest(random_point_in(cell, rng), level));
    }
    while out.len() < count {
        out.push(Texel::nearest(random_dir(rng), level));
    }
    out
}

/// The agreement at one band level of one world: the mean and worst total variation over its
/// texels, the mean over its mixed texels alone, and the worst texels with their cells, worst
/// first, so that a failure names where the weights miss.
#[derive(Debug, Clone)]
struct Agreement {
    level: u8,
    texels: usize,
    mean: f64,
    worst: f64,
    mixed: u32,
    mixed_mean: f64,
    worst_texels: Vec<(f64, Texel, PatchKey)>,
    /// The texels whose largest weight's material is not the material counted at the most points,
    /// by the pair (the weights', the points'): decision-r10-t2-weights §2.4.2's count, printed and
    /// recorded, not asserted.
    wrong_dominant: BTreeMap<(GroundMaterial, GroundMaterial), u32>,
}

/// The agreement at band level `level` over `count` texels from `hard` of `side` × `side` points
/// each, with a cache a texel, as a worker drops its own between descents (the channels' part is
/// never pruned, and a thousand texels about a body would hold every cell's grooves).
fn agreement(
    synth: &Synthesiser<'_, CoarseField>,
    hard: &[PatchKey],
    (level, count, side): (u8, usize, u32),
    rng: &mut Lcg,
) -> Agreement {
    let (mut sum, mut mixed_sum, mut mixed) = (0.0, 0.0, 0_u32);
    let mut worst: f64 = 0.0;
    let mut worst_texels: Vec<(f64, Texel, PatchKey)> = Vec::new();
    let mut wrong_dominant = BTreeMap::new();
    let set = texels(hard, level, count, rng);
    for texel in &set {
        let mut cache = SynthCache::new();
        let at = texel.at();
        let weights =
            weights_at(synth, &mut cache, at, texel.level).expect("a whole field holds it");
        assert_eq!(
            weights.weights().iter().map(|&b| u32::from(b)).sum::<u32>(),
            u32::from(WEIGHT_TOTAL),
            "level {level} at {at:?}: the weights' bytes sum to 255"
        );
        let (tv, is_mixed, dominant) = total_variation(synth, &mut cache, texel, &weights, side);
        if weights.largest() != dominant {
            *wrong_dominant
                .entry((weights.largest(), dominant))
                .or_insert(0_u32) += 1;
        }
        sum += tv;
        worst = worst.max(tv);
        if is_mixed {
            mixed += 1;
            mixed_sum += tv;
        }
        let (cell, _) = synth
            .cell_at(unit_dir(face_uv_to_xyz(at)))
            .expect("a whole field holds every cell");
        worst_texels.push((tv, *texel, cell));
        worst_texels.sort_by(|a, b| b.0.total_cmp(&a.0));
        worst_texels.truncate(WORST_KEPT);
    }
    let mean = sum / common::usize_as_f64(set.len());
    Agreement {
        level,
        texels: set.len(),
        mean,
        worst,
        mixed,
        mixed_mean: if mixed > 0 {
            mixed_sum / f64::from(mixed)
        } else {
            0.0
        },
        worst_texels,
        wrong_dominant,
    }
}

/// `material`'s form and entry, as a wrong-dominant pair prints it.
fn label(material: GroundMaterial) -> String {
    match material.entry() {
        Some(entry) => format!("{:?} of entry {entry}", material.class()),
        None => format!("{:?}", material.class()),
    }
}

/// The wrong-dominant pairs of `a`, each with its count and whether its two materials share an
/// entry and a photometric form.
fn wrong_dominant_pairs(a: &Agreement) -> Vec<String> {
    a.wrong_dominant
        .iter()
        .map(|(&(weighted, counted), n)| {
            format!(
                "{} for {} x{n} (entry {}, form {})",
                label(weighted),
                label(counted),
                if weighted.entry() == counted.entry() {
                    "shared"
                } else {
                    "differs"
                },
                if weighted.class().photometric_form() == counted.class().photometric_form() {
                    "shared"
                } else {
                    "differs"
                },
            )
        })
        .collect()
}

/// The materials of the weights at every vertex of `patch` at its own level, each vertex's bytes
/// summing to 255.
fn patch_palette(
    synth: &Synthesiser<'_, CoarseField>,
    cache: &mut SynthCache,
    patch: PatchKey,
) -> BTreeSet<GroundMaterial> {
    let level = BandLevel::new(patch.level()).expect("a band level");
    let mut out = BTreeSet::new();
    for y in 0..=64 {
        for x in 0..=64 {
            let at = xyz_to_face_uv(patch.vertex_dir(x, y));
            let weights = weights_at(synth, cache, at, level).expect("a whole field holds it");
            assert_eq!(
                weights.weights().iter().map(|&b| u32::from(b)).sum::<u32>(),
                u32::from(WEIGHT_TOTAL),
                "{patch:?} at {at:?}: the weights' bytes sum to 255"
            );
            out.extend(weights.iter().map(|(m, _)| m));
        }
    }
    out
}

/// The patches of the world named `name` whose palettes are checked: [`PATCHES`] at each of the
/// levels n − 1, n − 3, n − 6 and 4 from its finest level n, each the level's patch about a random
/// point of one of `cells` in turn, and on the reference Earth its nine-material patch as well
/// ([`nine_material_patch`]).
fn patches(name: &str, cells: &[PatchKey], finest: u8) -> Vec<PatchKey> {
    let mut rng = sampler(PATCH_SAMPLES, name);
    let mut out = Vec::new();
    for level in [finest - 1, finest - 3, finest - 6, CLASS_MAP_LEVEL] {
        for k in 0..PATCHES {
            let dir = if cells.is_empty() {
                random_dir(&mut rng)
            } else {
                random_point_in(cells[k * cells.len() / PATCHES], &mut rng)
            };
            out.push(PatchKey::containing(level, dir).expect("a patch of the level"));
        }
    }
    if name == EARTH {
        out.push(nine_material_patch());
    }
    out
}

/// The reference Earth's level-4 patch `(PosX, 4, 2, 1)`, where a coast between two crusts meets a
/// glaciated range: its vertices' weights name nine materials, Bedrock and Soil of both crusts,
/// Sand, Glacier and Snow of the ice entry, and both crusts' Seabed. T6.a's bake keeps the eight of
/// largest summed bytes, so nothing fails at run time, and a level-4 patch on the Earth is a
/// fallback leaf only; the test admits it at nine by name (decision-r10-t2-weights §3, 2026-10-26).
fn nine_material_patch() -> PatchKey {
    PatchKey::new(Face::PosX, CLASS_MAP_LEVEL, 2, 1).expect("a patch of level 4")
}

/// The most materials a patch of the world named `name` may need: [`MAX_CLASSES`], and nine for
/// the reference Earth's [`nine_material_patch`].
fn allowed_classes(name: &str, patch: PatchKey) -> usize {
    if name == EARTH && patch == nine_material_patch() {
        MAX_CLASSES + 1
    } else {
        MAX_CLASSES
    }
}

/// Holds `world`'s weights to [`AGREEMENT`] at every level, n − 1 to n − 6 and the class map's,
/// and its patches' palettes to [`MAX_CLASSES`], printing each level's figures as it is measured;
/// a failure names the level's worst texels and their cells, or the patch and its materials.
fn assert_the_weights_hold(world: &SampleWorld) {
    let name = world.name();
    let field = world.field();
    let synth = Synthesiser::new(field, DETAIL);
    let finest = finest_level(field.header().figure().equatorial_radius_m);
    let mut rng = sampler(SAMPLES, name);
    let kinds = hard_kinds(field, &synth);
    let hard = spread(&kinds, TEXELS);
    let counts: Vec<usize> = kinds.iter().map(Vec::len).collect();
    println!(
        "{name}: finest level {finest}; hard cells (belts, partial ice, full ice, shallow sea, \
         trunks, rims) {counts:?}, {} taken",
        hard.len()
    );
    let class_map_texels = if name == MARS {
        MARS_CLASS_MAP_TEXELS
    } else {
        TEXELS
    };
    let levels = (1..=6_u8)
        .map(|k| (finest - k, TEXELS, LEVEL_SIDE))
        .chain([(CLASS_MAP_LEVEL, class_map_texels, CLASS_MAP_SIDE)]);
    let mut misses = Vec::new();
    for at_level in levels {
        let a = agreement(&synth, &hard, at_level, &mut rng);
        println!(
            "{name} level {}: mean {:.4}, worst {:.4} over {} texels; mean {:.4} over the {} mixed; \
             wrong-dominant {}: {:?}",
            a.level,
            a.mean,
            a.worst,
            a.texels,
            a.mixed_mean,
            a.mixed,
            a.wrong_dominant.values().sum::<u32>(),
            wrong_dominant_pairs(&a)
        );
        if a.mean > AGREEMENT {
            misses.push(format!(
                "level {}: the weights are {:.3} from the classes in the mean; the worst texels \
                 (total variation, texel, cell): {:?}",
                a.level, a.mean, a.worst_texels
            ));
        }
    }
    let mut cache = SynthCache::new();
    let mut most = 0;
    for patch in patches(name, &spread(&kinds, 1), finest) {
        let palette = patch_palette(&synth, &mut cache, patch);
        most = most.max(palette.len());
        if palette.len() > allowed_classes(name, patch) {
            misses.push(format!(
                "{patch:?} needs {} classes: {palette:?}",
                palette.len()
            ));
        }
    }
    println!("{name}: the most classes a patch needed: {most}");
    assert!(misses.is_empty(), "{name}: {misses:#?}");
}

#[test]
fn every_sample_world_has_a_weights_test() {
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
#[ignore = "slow: the reference Earth's coarse pass at level 8, and 10^3 texels a level"]
fn the_weights_hold_on_the_earth_like_sample_world() {
    assert_the_weights_hold(sample_world(EARTH));
}

#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^5 finest points at the class map's level, about 4 ms \
            each"]
fn the_weights_hold_on_the_mars_like_sample_world() {
    assert_the_weights_hold(sample_world(MARS));
}

#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^6 finest points at the class map's level"]
fn the_weights_hold_on_the_moon_like_sample_world() {
    assert_the_weights_hold(sample_world(MOON));
}

#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^6 finest points at the class map's level"]
fn the_weights_hold_on_the_ceres_like_sample_world() {
    assert_the_weights_hold(sample_world(CERES));
}

#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^6 finest points at the class map's level"]
fn the_weights_hold_on_the_europa_like_sample_world() {
    assert_the_weights_hold(sample_world(EUROPA));
}

#[test]
#[ignore = "slow: the reference locked Earth's coarse pass at level 8, and 10^3 texels a level"]
fn the_weights_hold_on_the_locked_earth_like_sample_world() {
    assert_the_weights_hold(sample_world(LOCKED_EARTH));
}
