//! The authoritative material on the sample worlds (rendering plan R10, T1.b): `material_at`
//! answers at every sampled point of each, its entry is one of the body's palette, its ice is the
//! coarse field's, cell by cell, its seabed lies exactly below the water surface, and an airless
//! world's bare ground is its crust's regolith, or bedrock where a small crater's wall is steeper
//! than the cliff slope; and the surface crate's copy of the sim's thin-air threshold is the sim's.
//!
//! The worlds are R09.T16's `sample_worlds()` (`tests/common/surface_worlds.rs`): the reference
//! Earth, Mars, Moon and Ceres, the icy moon `europa_like` and the locked `locked_earth_like`, each
//! run through the whole coarse pass under `reference::SEED`, behind the sim's `testing` feature.
//!
//! **The material's ice is the field's, cell by cell.** At each point the test reads the point's
//! coarse cell, as the classifier does (`Synthesiser::cell_at`), and its ice share in 255ths. The
//! ice rule is the classifier's sea-ice and land-ice rules: `SeaIce`, `Glacier`, `Frost`, and on an
//! airless world `Regolith` of an `Ice`-role entry, which no other rule names (`Snow` is a rule of
//! its own).
//!
//! - In a cell of share 0 the answer is not of the ice rule.
//! - In a cell of full share it is, of the cell's ice entry: `SeaIce` or `Glacier` under air, and
//!   `Regolith` on an airless world.
//! - In a cell of partial share an answer of the ice rule is of the cell's ice entry: massive ice
//!   (`Glacier` under air, `Regolith` on an airless world) only at a share of at least
//!   `MASSIVE_ICE_SHARE`, `Frost` only below it, and `SeaIce` under air only above a half.
//!
//! That one property carries the plan's two examples (main's ruling, 2026-10-23). No sample world
//! has a warm pole, so "a warm pole has no ice cap" is held as "no ice cap the field does not
//! place": the property at both rotation poles of every world, its share-0 clause wherever a
//! pole's cell is bare, the classifier adding no cap by latitude. On the Moon, Ceres and the
//! Europa, whose fields ice no cell, no point is of the ice rule, both poles among them. "A locked
//! world's ice lies where R09's field puts it" is the same property on the locked Earth. Its
//! limit: inside a partial cell the ice lies by the classifier's shadow index, whose tilt falls
//! towards the nearer rotation pole, and the header carries no substellar point, so which side of
//! a partial cell the ice takes is not checked against the night side.
//!
//! **The seabed lies exactly below the water surface.** At each point the test reads the point's
//! height at R05's finest level, metres above the datum (the spheroid, along its normal), the frame
//! of the cell's water surface, as the classifier does (`Synthesiser::height_at` at
//! `finest_level`'s band, at the classifier's own direction of the point). The point lies below
//! the water surface where its cell is under water and the height is below the cell's water
//! surface, the cell's and not the header's sea level (T1.b's "_The coastline follows cells_").
//! There, and only there, the answer is `SeaIce` or `Seabed`, the first two of the classifier's
//! rules; a `Seabed` is of the floor's entry, the crust's where the crust names one, and never the
//! main liquid's (signoff-9).
//!
//! On each world the points are 64 random directions from a sampler seeded by the world's name
//! ([`sampler`]), so that they do not depend on which test runs the world, the order the worlds
//! run in or the world's place in the list; a cube corner `(1, 1, −1)`, a face edge's `(0, 1, 1)`
//! and both poles; the centres of up to 16 cells of full share and up to 16 of partial share,
//! each set spaced evenly in `cell_index` order over the field's cells of its kind; and in each
//! partial cell whose centre is sampled, the centres of its 4 × 4 equal steps
//! ([`inner_points`]), since a partial cell's ice covers only its share of the cell and its centre
//! may lie outside it. The checks are not empty where they could be:
//!
//! - on a world whose field has a cell of full share, some point in such a cell is of the ice rule;
//! - on a world with sampled partial cells that are not under water, some point in one of them is
//!   of the ice rule, so that the partial share's clause runs there;
//! - on the two worlds with a sea ([`SEAS`]), some point lies below its cell's water surface.
//!
//! The test prints each world's counts of cells, points and answers.
//!
//! Under nextest each test runs in a process of its own, so each test pays for the passes of the
//! worlds it reads. The four `Tier::Fast` worlds share one test, and each `Tier::Slow` world, the
//! reference Earth and the locked Earth, has its own, ignored. The slow two run with
//! `cargo nextest run -p hyperion-sim --features testing --test surface_material_worlds
//! --run-ignored only`.
#![cfg(feature = "testing")]

#[expect(
    dead_code,
    reason = "the material's tests use only the sample worlds of the shared helpers"
)]
mod common;

use std::collections::BTreeMap;

use common::surface_worlds::{SampleWorld, Tier, sample_world, sample_worlds};
use hyperion_base::rng::DetailSeed;
use hyperion_sim::planetary::derive::photometry;
use hyperion_surface::cube::{PATCH_QUADS, PatchKey, face_uv_to_xyz, unit_dir, xyz_to_face_uv};
use hyperion_surface::field::{
    FieldHeader, FieldView, MaterialPalette, PaletteRole, SynthesisCell, cell_at_index,
};
use hyperion_surface::geometry::finest_level;
use hyperion_surface::material::{
    GroundMaterial, MASSIVE_ICE_SHARE, MaterialClass, THIN_ATMOSPHERE_PRESSURE, material_at,
};
use hyperion_surface::synth::{BandLevel, SynthCache, Synthesiser};
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;

/// The detail seed the sample worlds' ground is drawn with.
const DETAIL: DetailSeed = DetailSeed::new(0x6d61_7465_7269_616c);

/// The seed of the sample points, into which each world's name is folded ([`sampler`]).
const SAMPLES: u64 = 0x6d61_7465_7269_6177;

/// The FNV-1a 64 prime (Fowler, Noll and Vo), the testkit's `f64_digest`'s, which folds a world's
/// name into [`SAMPLES`].
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The reference Earth, a slow world with a test of its own.
const EARTH: &str = "earth_like";

/// The locked Earth, a slow world with a test of its own.
const LOCKED_EARTH: &str = "locked_earth_like";

/// The slow worlds, each with a test of its own.
const SLOW: [&str; 2] = [EARTH, LOCKED_EARTH];

/// The airless sample worlds, whose headers' surface pressure is zero.
const AIRLESS: [&str; 3] = ["moon_like", "ceres_like", "europa_like"];

/// The sample worlds with a sea, on each of which some sampled point lies below its cell's water
/// surface.
const SEAS: [&str; 2] = [EARTH, LOCKED_EARTH];

/// The most cells of each kind, of full share and of partial share, whose centres a world samples.
const TARGETED: usize = 16;

/// The equal steps along each side of a sampled partial cell, at whose centres its inner points
/// lie ([`inner_points`]).
const INNER_STEPS: u8 = 4;

/// The share above which a partial cell's sea ice covers a point under its water: the majority, by
/// which the classifier places a sea's ice (T1.b's "Where a partial share lies").
const MAJORITY: f64 = 0.5;

/// The sampler of the world named `name`: an [`Lcg`] seeded with FNV-1a 64 over the name's bytes,
/// from [`SAMPLES`] in place of the offset basis.
fn sampler(name: &str) -> Lcg {
    Lcg::new(name.bytes().fold(SAMPLES, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    }))
}

/// Up to [`TARGETED`] of `indices`, spaced evenly through them in their order: all of them where
/// there are no more.
fn spaced(indices: &[u32]) -> impl Iterator<Item = u32> + '_ {
    let n = indices.len();
    let k = n.min(TARGETED);
    (0..k).map(move |i| indices[i * n / k])
}

/// The unit direction of `cell`'s centre: vertex (32, 32) of its patch's 65 × 65.
fn centre(cell: PatchKey) -> [f64; 3] {
    let half = u8::try_from(PATCH_QUADS / 2).expect("a patch's half side fits a byte");
    cell.vertex_dir(half, half)
}

/// The unit directions of the centres of `cell`'s [`INNER_STEPS`] × [`INNER_STEPS`] equal steps:
/// vertices (16a + 8, 16b + 8) of its patch's 65 × 65 for a and b below 4, each at least 8 vertices
/// from the cell's edges.
fn inner_points(cell: PatchKey) -> impl Iterator<Item = [f64; 3]> {
    let step = u8::try_from(PATCH_QUADS).expect("a patch's side fits a byte") / INNER_STEPS;
    let at = move |k: u8| step * k + step / 2;
    (0..INNER_STEPS).flat_map(move |a| (0..INNER_STEPS).map(move |b| cell.vertex_dir(at(a), at(b))))
}

/// The points sampled on the world named `name`, whose field is of level `level` and whose cells of
/// full and of partial share have the indices `full_cells` and `partial_cells`, in `cell_index`
/// order: 64 random directions from its [`sampler`], the cube corner `(1, 1, −1)`, the face edge's
/// `(0, 1, 1)` and both poles, the centres of the cells [`spaced`] of each kind, and then the
/// [`inner_points`] of each partial cell whose centre is sampled, cell by cell in that order.
fn sample_dirs(name: &str, level: u8, full_cells: &[u32], partial_cells: &[u32]) -> Vec<[f64; 3]> {
    let cell = |index| cell_at_index(level, index).expect("a field's cell is of its level");
    let mut rng = sampler(name);
    let mut dirs: Vec<[f64; 3]> = (0..64)
        .map(|_| unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)))
        .collect();
    dirs.extend([
        unit_dir([1.0, 1.0, -1.0]),
        unit_dir([0.0, 1.0, 1.0]),
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ]);
    dirs.extend(
        spaced(full_cells)
            .chain(spaced(partial_cells))
            .map(|index| centre(cell(index))),
    );
    dirs.extend(spaced(partial_cells).flat_map(|index| inner_points(cell(index))));
    dirs
}

/// The form of an answer of the ice rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IceForm {
    /// `SeaIce`, a sea's ice, of the sea-ice rule.
    SeaIce,
    /// `Glacier`, massive ice under air.
    Glacier,
    /// `Frost`, thin ice on any body.
    Frost,
    /// `Regolith` of an `Ice`-role entry, an airless body's massive ice, gardened.
    Regolith,
}

/// The form of `ground` where it is an answer of the ice rule: `SeaIce`, `Glacier` or `Frost`, or
/// `Regolith` of an `Ice`-role entry of `palette`, which no other rule names; else `None`.
fn ice_form(ground: GroundMaterial, palette: &MaterialPalette) -> Option<IceForm> {
    match ground.class() {
        MaterialClass::SeaIce => Some(IceForm::SeaIce),
        MaterialClass::Glacier => Some(IceForm::Glacier),
        MaterialClass::Frost => Some(IceForm::Frost),
        MaterialClass::Regolith => ground
            .entry()
            .and_then(|entry| palette.get(entry))
            .is_some_and(|entry| entry.role == PaletteRole::Ice)
            .then_some(IceForm::Regolith),
        MaterialClass::Bedrock
        | MaterialClass::Soil
        | MaterialClass::Sand
        | MaterialClass::Sediment
        | MaterialClass::VolcanicRock
        | MaterialClass::Melt
        | MaterialClass::Snow
        | MaterialClass::OrganicSediment
        | MaterialClass::Seabed
        | MaterialClass::Evaporite
        | MaterialClass::Dust => None,
    }
}

/// What a world's points found, for the record.
#[derive(Debug, Default)]
struct Tally {
    /// The points in cells of share 0.
    bare: usize,
    /// The points in cells of full share.
    full: usize,
    /// The points in cells of partial share.
    partial: usize,
    /// The answers of the ice rule in cells of full share.
    full_ice: usize,
    /// The answers of the ice rule in cells of partial share.
    partial_ice: usize,
    /// The answers of the ice rule in cells of partial share, by their form.
    partial_ice_classes: BTreeMap<MaterialClass, usize>,
    /// The points in the sampled cells of partial share that are not under water.
    dry_target: usize,
    /// The answers of the ice rule in the sampled cells of partial share that are not under water.
    dry_target_ice: usize,
    /// The points below their cell's water surface.
    below: usize,
    /// The points in cells under water that lie at or above their cell's water surface.
    above_in_sea: usize,
    /// Every answer, by its form.
    classes: BTreeMap<MaterialClass, usize>,
}

impl Tally {
    /// Counts the answer `ground` at a point of `cell`, which is of the ice rule where `of_ice`,
    /// lies below the cell's water surface where `below`, and is in a sampled partial cell not
    /// under water where `in_dry_target`.
    fn count(
        &mut self,
        ground: GroundMaterial,
        cell: &SynthesisCell,
        of_ice: bool,
        below: bool,
        in_dry_target: bool,
    ) {
        *self.classes.entry(ground.class()).or_default() += 1;
        match cell.ice {
            0 => self.bare += 1,
            u8::MAX => {
                self.full += 1;
                self.full_ice += usize::from(of_ice);
            }
            _ => {
                self.partial += 1;
                self.partial_ice += usize::from(of_ice);
                if of_ice {
                    *self.partial_ice_classes.entry(ground.class()).or_default() += 1;
                }
            }
        }
        if in_dry_target {
            self.dry_target += 1;
            self.dry_target_ice += usize::from(of_ice);
        }
        if below {
            self.below += 1;
        } else if cell.is_under_water() {
            self.above_in_sea += 1;
        }
    }

    /// Asserts that the checks on the world named `name` were not empty: where its field has
    /// `full_cells` cells of full share, some point in such a cell is of the ice rule; where
    /// `dry_targets` of its sampled partial cells are not under water, some point in one of them
    /// is; and on a world of [`SEAS`], some point lies below its cell's water surface.
    fn assert_not_empty(&self, name: &str, full_cells: usize, dry_targets: usize) {
        // The full share's clause has run on the world.
        if full_cells > 0 {
            assert!(
                self.full_ice > 0,
                "{name}: no point in a cell of full share is of the ice rule, though \
                 {full_cells} cells are"
            );
        }
        // The partial share's clause has run on the world, outside the majority by which a sea's
        // ice lies.
        if dry_targets > 0 {
            assert!(
                self.dry_target_ice > 0,
                "{name}: no point in the {dry_targets} sampled partial cells not under water is of \
                 the ice rule, of {} points in them",
                self.dry_target
            );
        }
        // The seabed's check has run on the world.
        if SEAS.contains(&name) {
            assert!(
                self.below > 0,
                "{name}: no sampled point lies below its cell's water surface, {} in cells under \
                 water lying at or above it",
                self.above_in_sea
            );
        }
    }
}

/// Asserts that the answer `ground` at `site` holds `cell`'s ice (main's ruling 2), and returns
/// whether it is of the ice rule.
///
/// On a body of palette `palette`, airless or not: no answer of the ice rule where the share is 0;
/// one of it, of the cell's ice entry and in a full share's form, where the share is whole; and
/// where it is partial, an answer of it only of the cell's ice entry and in a form the share
/// allows.
fn assert_the_ice_is_the_field_s(
    site: &str,
    ground: GroundMaterial,
    cell: &SynthesisCell,
    palette: &MaterialPalette,
    airless: bool,
) -> bool {
    let share = cell.ice;
    let ice_entry = cell.ice_entry();
    let form = ice_form(ground, palette);
    let at =
        format!("{site}: {ground:?} in a cell of ice share {share}/255 and entry {ice_entry:?}");
    match (share, form) {
        (0, form) => assert_eq!(form, None, "{at}: ice the field does not place"),
        (u8::MAX, None) => panic!("{at}: not of the ice rule in a full cell"),
        (u8::MAX, Some(form)) => {
            assert_eq!(
                ground.entry(),
                ice_entry,
                "{at}: not of the cell's ice entry"
            );
            let full_form = match form {
                IceForm::SeaIce | IceForm::Glacier => !airless,
                IceForm::Regolith => airless,
                IceForm::Frost => false,
            };
            assert!(
                full_form,
                "{at}: not a full share's form, airless {airless}"
            );
        }
        (_, Some(form)) => {
            assert_eq!(
                ground.entry(),
                ice_entry,
                "{at}: not of the cell's ice entry"
            );
            let fraction = cell.ice_fraction();
            let massive = fraction >= MASSIVE_ICE_SHARE;
            let partial_form = match form {
                IceForm::Glacier => massive && !airless,
                IceForm::Regolith => massive && airless,
                IceForm::Frost => !massive,
                IceForm::SeaIce => fraction > MAJORITY && !airless,
            };
            assert!(
                partial_form,
                "{at}: not a form share {share}/255 allows, airless {airless}"
            );
        }
        (_, None) => {}
    }
    form.is_some()
}

/// Asserts that the answer `ground` at `site`, whose ground is `h_m` metres above the datum at the
/// finest level, the frame of the cell's water surface, in `cell` on the body of `header`, is
/// `SeaIce` or `Seabed` exactly where the point lies below the cell's water surface, the
/// classifier's first two rules, and that a `Seabed` is of the floor's entry, the crust's where
/// the crust names one, and never the main liquid's (signoff-9); and returns whether the point
/// lies below the water surface.
fn assert_the_seabed_is_below_the_water(
    site: &str,
    ground: GroundMaterial,
    h_m: f64,
    cell: &SynthesisCell,
    header: &FieldHeader,
) -> bool {
    let surface_m = cell.water_surface().value();
    let wet = cell.is_under_water();
    let below = wet && h_m < surface_m;
    let at = format!(
        "{site}: {ground:?} at {h_m} m, its cell's water surface {surface_m} m, under water {wet}"
    );
    let of_the_sea = matches!(
        ground.class(),
        MaterialClass::SeaIce | MaterialClass::Seabed
    );
    assert_eq!(
        of_the_sea, below,
        "{at}: sea ice or seabed, and the point below the water surface"
    );
    if ground.class() == MaterialClass::Seabed {
        if let Some(liquid) = header.main_liquid() {
            assert_ne!(
                ground.entry(),
                Some(liquid),
                "{at}: the seabed is never its liquid"
            );
        }
        if let Some(crust) = header.crust_palette()[usize::from(u8::from(cell.crust))] {
            assert_eq!(
                ground.entry(),
                Some(crust),
                "{at}: the seabed is not of the crust's entry"
            );
        }
    }
    below
}

/// Asserts the material on `world` at every sampled point: it answers, its entry is its palette's
/// (none only on an empty palette), its ice is the field's cell by cell, its `SeaIce` and `Seabed`
/// lie exactly below the cell's water surface, a `Seabed` of the crust's entry and never the
/// liquid's, and on an airless world the ground of a cell without ice is a crust entry's regolith
/// or bedrock; that the header's air agrees with [`AIRLESS`]; and that the checks are not empty:
/// where the field has a cell of full share, some point in such a cell is of the ice rule; where
/// sampled partial cells are not under water, some point in one of them is; and on a world of
/// [`SEAS`], some point lies below its cell's water surface.
fn assert_the_material_holds(world: &SampleWorld) {
    let name = world.name();
    let field = world.field();
    let header = field.header();
    let palette = header.palette();
    let airless = AIRLESS.contains(&name);
    assert_eq!(
        header.surface_pressure().value() <= 0.0,
        airless,
        "{name}'s air"
    );
    let mut full_cells = Vec::new();
    let mut partial_cells = Vec::new();
    for (index, cell) in (0_u32..).zip(field.synthesis()) {
        match cell.ice {
            0 => {}
            u8::MAX => full_cells.push(index),
            _ => partial_cells.push(index),
        }
    }
    let level = header.level().get();
    let dirs = sample_dirs(name, level, &full_cells, &partial_cells);
    let synthesis = field.synthesis();
    // The sampled partial cells not under water, whose points run the partial share's clause.
    let dry_targets: Vec<PatchKey> = spaced(&partial_cells)
        .filter(|&index| {
            let at = usize::try_from(index).expect("a cell index fits a usize");
            !synthesis[at].is_under_water()
        })
        .map(|index| cell_at_index(level, index).expect("a field's cell is of its level"))
        .collect();
    // The classifier's level: R05's finest (the surface crate's `finest_band`).
    let finest = BandLevel::new(finest_level(header.figure().equatorial_radius_m))
        .expect("R05's finest level is a band level");
    let synth = Synthesiser::new(field, DETAIL);
    let mut cache = SynthCache::new();
    let mut tally = Tally::default();
    for dir in dirs {
        let site = format!("{name} {dir:?}");
        let at = xyz_to_face_uv(dir);
        let ground = material_at(&synth, &mut cache, at).unwrap_or_else(|e| panic!("{site}: {e}"));
        // The classifier's own direction of `at`, whose cell and height it reads.
        let read = unit_dir(face_uv_to_xyz(at));
        let (key, cell) = synth
            .cell_at(read)
            .unwrap_or_else(|e| panic!("{site}: {e}"));
        let h_m = synth
            .height_at(&mut cache, read, finest)
            .unwrap_or_else(|e| panic!("{site}: {e}"))
            .height_m;
        let of_ice = assert_the_ice_is_the_field_s(&site, ground, cell, palette, airless);
        let below = assert_the_seabed_is_below_the_water(&site, ground, h_m, cell, header);
        tally.count(ground, cell, of_ice, below, dry_targets.contains(&key));
        if airless && cell.ice == 0 {
            assert!(
                matches!(
                    ground.class(),
                    MaterialClass::Regolith | MaterialClass::Bedrock
                ),
                "{site}: {ground:?} on an airless body's bare ground"
            );
        }
        let Some(entry) = ground.entry() else {
            assert!(palette.is_empty(), "{site}: no entry on a modelled body");
            continue;
        };
        let entry = palette
            .get(entry)
            .unwrap_or_else(|| panic!("{site}: {ground:?} past the palette"));
        if airless && cell.ice == 0 {
            assert!(entry.role.is_crust(), "{site}: {entry:?}");
        }
    }
    eprintln!(
        "{name}: cells of full share {}, of partial share {}, of {}; points in bare cells {}, \
         in full {}, in partial {}; answers of the ice rule in full cells {}, in partial {}; \
         answers by form {:?}; sampled partial cells {}, of them not under water {}; points in \
         those not under water {}, of the ice rule {}; the partial cells' answers of the ice rule \
         by form {:?}; points below the water surface {}, in cells under water at or \
         above it {}",
        full_cells.len(),
        partial_cells.len(),
        synthesis.len(),
        tally.bare,
        tally.full,
        tally.partial,
        tally.full_ice,
        tally.partial_ice,
        tally.classes,
        spaced(&partial_cells).count(),
        dry_targets.len(),
        tally.dry_target,
        tally.dry_target_ice,
        tally.partial_ice_classes,
        tally.below,
        tally.above_in_sea,
    );
    tally.assert_not_empty(name, full_cells.len(), dry_targets.len());
}

/// The material holds on Mars, the Moon, Ceres and the Europa; and every sample world runs in
/// exactly one test, these four here and each slow world in its own.
#[test]
fn the_material_holds_on_every_fast_sample_world() {
    let fast: Vec<&SampleWorld> = sample_worlds()
        .iter()
        .filter(|world| world.tier() == Tier::Fast)
        .collect();
    assert_eq!(fast.len(), 4, "Mars, the Moon, Ceres and the Europa");
    let mut covered: Vec<&str> = fast
        .iter()
        .copied()
        .map(SampleWorld::name)
        .chain(SLOW)
        .collect();
    let mut names: Vec<&str> = sample_worlds().iter().map(SampleWorld::name).collect();
    covered.sort_unstable();
    names.sort_unstable();
    assert_eq!(
        covered, names,
        "every sample world runs in exactly one test: the fast ones here, each slow one in its own"
    );
    for world in fast {
        assert_the_material_holds(world);
    }
}

#[test]
#[ignore = "slow: the reference Earth's coarse pass at level 8, about 100 s in the test profile"]
fn the_material_holds_on_the_earth_like_sample_world() {
    let earth = sample_world(EARTH);
    assert_eq!(earth.tier(), Tier::Slow);
    assert_the_material_holds(earth);
}

#[test]
#[ignore = "slow: the reference locked Earth's coarse pass at level 8, about 22 s in the test \
            profile"]
fn the_material_holds_on_the_locked_earth_like_sample_world() {
    let locked = sample_world(LOCKED_EARTH);
    assert_eq!(locked.tier(), Tier::Slow);
    assert_the_material_holds(locked);
}

/// The surface crate repeats the sim's thin-air threshold, which it cannot see; the two are one.
#[test]
fn the_thin_air_threshold_is_the_sim_s() {
    assert_eq!(
        bits(THIN_ATMOSPHERE_PRESSURE.value()),
        bits(photometry::THIN_ATMOSPHERE_PRESSURE.value())
    );
}
