//! The free-floating brown dwarfs and rogue planets, placed: counts against the field, populations
//! and ages against the stars', resolution, order independence, the golden file and the bound
//! (plan 13, P13.T3.a–d).
//!
//! The two substellar layers are placed by plan 03's code with the layer as its only difference, so
//! these are plan 03's own checks turned on the new layers: a count in a block of whole cells is
//! Poisson with the field's integral over the block as its mean (`common::reference_box_integral`),
//! the component picked follows the odds `share × density` that the stellar layers use at the same
//! place, and the ages follow the picked components' distributions mixed by those odds, which is
//! what layer A's ages there follow too.

#[expect(
    dead_code,
    reason = "these checks use the placement helpers of tests/common alone"
)]
mod common;

use common::{reference_box_component_integrals, reference_box_integral, sunlike_point};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CandidateOutcome, CellKey, Existence, ResolveSystemError, SUBSTELLAR_LAYERS, SystemKind,
    SystemRecord, candidate_count, evaluate_candidate, generate_cell, resolve,
};
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, PointLy};
use hyperion_sim::id::{Designation, Layer, SystemId, SystemIdKind};
use hyperion_sim::math;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Years;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{
    ALPHA, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample,
};

/// The seed of the Milky Way fixture here.
const FIXTURE_SEED: u64 = 0x1303_0000_0000_0000;

/// The seed of the drawn galaxy the golden file pins beside the fixture.
const DRAWN_SEED: u64 = 0x1303_0000_5eed_0001;

/// The two substellar layers.
const LAYERS: [Layer; 2] = [Layer::BrownDwarf, Layer::RoguePlanet];

/// The edge every test block is a multiple of, and aligned to, light-years: the brown dwarfs'
/// cell, four of the rogue planets' and two of layer A's.
const BLOCK_UNIT_LY: i32 = 16;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(FIXTURE_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// A count as an `f64`, exact for every count these tests reach.
fn count_as_f64(count: usize) -> f64 {
    f64::from(u32::try_from(count).expect("a test count fits in 32 bits"))
}

/// A cube of whole cells of every layer: its low corner, a multiple of 16 ly on every axis, and its
/// edge, a multiple of 16 ly.
#[derive(Debug, Clone, Copy)]
struct Block {
    min_ly: [i32; 3],
    edge_ly: u32,
}

impl Block {
    /// The block about `at` large enough that `layer` expects about `target` objects in it, judged
    /// from the density at `at`, of at least one 16 ly unit and at most `max_units` of them.
    fn sized_for(galaxy: &Galaxy, layer: Layer, at: [f64; 3], target: f64, max_units: u32) -> Self {
        let density = galaxy.fields().layer_density(
            galaxy.shares(),
            MassBand::from(layer),
            &PointLy::new(at[0], at[1], at[2]),
        );
        assert!(density > 0.0, "{layer:?} has no density at {at:?}");
        let unit = f64::from(BLOCK_UNIT_LY);
        let wanted = math::cbrt(target / density) / unit;
        let units = (1..=max_units)
            .find(|&n| f64::from(n) >= wanted)
            .unwrap_or(max_units);
        let half = i32::try_from(units / 2).expect("a few dozen units");
        let min_ly = at.map(|x| {
            let whole = common::whole_ly(x.floor());
            (whole.div_euclid(BLOCK_UNIT_LY) - half) * BLOCK_UNIT_LY
        });
        Self {
            min_ly,
            edge_ly: units * BLOCK_UNIT_LY.unsigned_abs(),
        }
    }

    /// Every cell of `layer` in the block, in `CellKey` order.
    fn cells(&self, layer: Layer) -> Vec<CellKey> {
        let size = i32::try_from(layer.cell_size_ly()).expect("a small cell");
        let per_axis = i32::try_from(self.edge_ly).expect("a small block") / size;
        let first = self.min_ly.map(|ly| ly / size);
        let mut keys = Vec::new();
        for dx in 0..per_axis {
            for dy in 0..per_axis {
                for dz in 0..per_axis {
                    let cell = [first[0] + dx, first[1] + dy, first[2] + dz];
                    keys.push(CellKey::new(layer, cell).expect("a block lies inside the cube"));
                }
            }
        }
        keys
    }

    /// Every object of `layer` in the block.
    fn records(&self, galaxy: &Galaxy, layer: Layer) -> Vec<SystemRecord> {
        let mut all = Vec::new();
        let mut cell = Vec::new();
        for key in self.cells(layer) {
            generate_cell(galaxy, key, &mut cell);
            all.extend_from_slice(&cell);
        }
        all
    }

    /// The midpoint steps per axis of the reference sums: a light-year or finer, up to 112.
    fn steps(&self) -> u32 {
        self.edge_ly.min(112)
    }
}

/// The five places of P13.T3.a's acceptance: the plane at 26,000 ly, 3,000 ly above it, the bulge
/// at 1,000 ly, a young arm ridge outside the bar's end and the halo at 40,000 ly.
fn five_places(galaxy: &Galaxy) -> [(&'static str, [f64; 3]); 5] {
    let sun = sunlike_point(galaxy).to_light_years_f64();
    let arms = galaxy.fields().arms();
    let r = 1.15 * arms.bar_half_length().value();
    let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, 0));
    let halo = 40_000.0 * 0.5_f64.sqrt();
    [
        ("the plane at 26,000 ly", sun),
        ("3,000 ly above the plane", [sun[0], sun[1], 3_000.0]),
        ("the bulge at 1,000 ly", [707.0, 707.0, 0.0]),
        ("a young arm ridge outside the bar", [r * cos, r * sin, 0.0]),
        ("the halo at 40,000 ly", [0.0, halo, halo]),
    ]
}

/// The three statistical checks of P13.T3.a–b for one layer in one block: the count against the
/// field's integral, the population mix against the stellar layers' odds, and the ages against
/// the mixture of the picked components' distributions that layer A's ages follow there.
fn assert_layer_follows_the_stars(name: &str, galaxy: &Galaxy, layer: Layer, block: &Block) {
    let band = MassBand::from(layer);
    let records = block.records(galaxy, layer);
    let mean = reference_box_integral(galaxy, band, block.min_ly, block.edge_ly, block.steps());
    let what = format!("{name}, layer {}", layer.letter());
    println!(
        "{what}: {} objects against {mean:.1} in a {} ly block",
        records.len(),
        block.edge_ly
    );
    assert_poisson_count(
        &format!("{what}: the count"),
        u64::try_from(records.len()).expect("a count fits"),
        mean,
        ALPHA,
    );
    let (lo, hi) = (band.lo(), band.hi());
    for record in &records {
        assert_eq!(record.layer(), layer);
        assert_eq!(record.kind(), SystemKind::of_layer(layer));
        let m = record.primary_initial_mass().value();
        assert!((lo..=hi).contains(&m), "{what}: a mass of {m} M☉");
    }

    // The odds the stellar layers pick with at this place: layer A's share × each component's
    // integral. The substellar rows are one number in every column, so these are their odds too.
    let integrals =
        reference_box_component_integrals(galaxy, block.min_ly, block.edge_ly, block.steps());
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let weights: Vec<f64> = fields
        .components()
        .iter()
        .zip(integrals)
        .map(|(c, integral)| shares.component_share(MassBand::A, c) * integral)
        .collect();
    let total: f64 = weights.iter().sum();
    let mut observed = [0_u64; POPULATIONS.len()];
    let mut expected = [0.0; POPULATIONS.len()];
    for (c, w) in fields.components().iter().zip(&weights) {
        expected[c.population().index()] += w / total * count_as_f64(records.len());
    }
    for record in &records {
        observed[record.population().index()] += 1;
    }
    // Where one population is nearly the whole field (the halo far out), there is no mix to test:
    // every object must then come from a population the field has there.
    if expected.iter().filter(|&&e| e >= 5.0).count() >= 2 {
        let fit = chi_square_gof(&observed, &expected);
        assert_p_value(&format!("{what}: the population mix"), fit.p_value, ALPHA);
    }
    for (population, (&o, &e)) in observed.iter().zip(&expected).enumerate() {
        assert!(
            o == 0 || e > 0.0,
            "{what}: population {population} has none expected"
        );
    }

    let cdf = |age: f64| {
        let mixed = fields
            .components()
            .iter()
            .zip(&weights)
            .fold(0.0, |sum, (c, w)| sum + w * c.ages().cdf(Years::new(age)));
        (mixed / total).clamp(0.0, 1.0)
    };
    let mut ages: Vec<f64> = records.iter().map(|r| r.age_at_epoch().value()).collect();
    let ks = ks_one_sample(&mut ages, cdf);
    assert_p_value(&format!("{what}: the ages"), ks.p_value, ALPHA);
}

// --- P13.T3.a–b: counts, populations and ages ---

/// Where the system density is 0.003 per ly³, a 16 ly cell averages 2.6–3.4 brown dwarfs and a
/// 4 ly cell 4.8–6 rogue planets (P13.T3.a–b): the per-system abundances times the reference
/// density times the cell's volume.
#[test]
fn a_cell_at_the_reference_density_holds_the_plan_s_counts() {
    let galaxy = milky_way();
    let per_cell = |layer: Layer| {
        let edge = f64::from(layer.cell_size_ly());
        let per_system = galaxy.shares().share(MassBand::from(layer), POPULATIONS[1]);
        per_system * 0.003 * edge * edge * edge
    };
    let brown_dwarfs = per_cell(Layer::BrownDwarf);
    let rogue_planets = per_cell(Layer::RoguePlanet);
    assert!((2.6..=3.4).contains(&brown_dwarfs), "{brown_dwarfs}");
    assert!((4.8..=6.0).contains(&rogue_planets), "{rogue_planets}");
}

/// The three checks at the Sun-like point and in the bulge, for both layers.
#[test]
fn substellar_objects_follow_the_stars_at_the_sun_and_in_the_bulge() {
    let galaxy = milky_way();
    let [sun, _, bulge, ..] = five_places(&galaxy);
    for (name, at) in [sun, bulge] {
        for layer in LAYERS {
            let block = Block::sized_for(&galaxy, layer, at, 600.0, 16);
            assert_layer_follows_the_stars(name, &galaxy, layer, &block);
        }
    }
}

/// The three checks at all five places of P13.T3.a, for both layers.
#[test]
#[ignore = "slow: both substellar layers in five blocks, the halo's some 10⁸ ly³"]
fn substellar_objects_follow_the_stars_at_five_places() {
    let galaxy = milky_way();
    for (name, at) in five_places(&galaxy) {
        for layer in LAYERS {
            let block = Block::sized_for(&galaxy, layer, at, 600.0, 64);
            assert_layer_follows_the_stars(name, &galaxy, layer, &block);
        }
    }
}

/// A cell at every corner of the root cube, on both grids, generates, and its objects lie inside
/// it (P13.T3.b: code that assumed a cell of at least 8 ly would be found here).
#[test]
fn a_cell_at_every_corner_of_the_root_cube_generates() {
    let galaxy = milky_way();
    let mut cell = Vec::new();
    for layer in LAYERS {
        let size = i32::try_from(layer.cell_size_ly()).unwrap();
        let edge = 65_536 / size;
        for corner in 0..8_u8 {
            let face = |bit: u8| {
                if corner >> bit & 1 == 0 {
                    -edge
                } else {
                    edge - 1
                }
            };
            let key = CellKey::new(layer, [face(0), face(1), face(2)]).unwrap();
            generate_cell(&galaxy, key, &mut cell);
            let origin = key.origin_ly();
            for record in &cell {
                let ly = record.epoch_position().cell().to_array();
                for axis in 0..3 {
                    assert!((origin[axis]..origin[axis] + size).contains(&ly[axis]));
                }
            }
        }
        // And the cells touching the centre, where the rogue planets' cells are fullest.
        for corner in 0..8_u8 {
            let touching = |bit: u8| if corner >> bit & 1 == 0 { -1 } else { 0 };
            let key = CellKey::new(layer, [touching(0), touching(1), touching(2)]).unwrap();
            generate_cell(&galaxy, key, &mut cell);
            assert!(
                !cell.is_empty(),
                "{layer:?} {corner}: the centre is not empty"
            );
            let capacity = usize::try_from(key.index_capacity()).unwrap();
            assert!(cell.len() < capacity);
        }
    }
}

/// Every generated ID round-trips through plan 01's canonical decode, and an ID of another layer
/// with a spare bit set is still rejected (P13.T3.b).
#[test]
fn substellar_ids_are_canonical_and_spare_bits_stay_reserved_elsewhere() {
    let galaxy = milky_way();
    let at = sunlike_point(&galaxy).to_light_years_f64();
    for layer in LAYERS {
        let block = Block::sized_for(&galaxy, layer, at, 300.0, 8);
        for record in block.records(&galaxy, layer) {
            let id = record.id();
            assert_eq!(SystemId::from_raw(id.raw()), Ok(id));
            let SystemIdKind::Grid(grid) = id.kind() else {
                panic!("a placed object has a grid ID");
            };
            assert_eq!(grid.layer(), layer);
        }
    }
    let a = CellKey::containing(Layer::A, &sunlike_point(&galaxy))
        .unwrap()
        .candidate_id(3)
        .unwrap();
    for bit in 58..=60 {
        assert!(SystemId::from_raw(a.raw() | (1 << bit)).is_err(), "{bit}");
    }
}

// --- P13.T3.c: resolution, designations, order independence, the golden file ---

/// The cells of `layer` about the Sun-like point, `n` along x.
fn sun_cells(galaxy: &Galaxy, layer: Layer, n: i32) -> Vec<CellKey> {
    let first = CellKey::containing(layer, &sunlike_point(galaxy)).unwrap();
    let [x, y, z] = first.gen_cell().to_array();
    (0..n)
        .map(|i| CellKey::new(layer, [x + i, y, z]).unwrap())
        .collect()
}

/// Every object of a generated cell resolves to the same record; an index at or above the
/// candidate count, and a thinned candidate's index, resolve to "no such system".
///
/// Thinned candidates are looked for where the density falls steeply across a cell, at the
/// nuclear disc's edge: at the Sun-like point a 4 ly or 16 ly cell's density varies by well under
/// a per cent, and the bound rejects almost nothing.
#[test]
fn every_substellar_object_resolves_to_its_record() {
    let galaxy = milky_way();
    let mut cell = Vec::new();
    for layer in LAYERS {
        let mut resolved = 0;
        for key in sun_cells(&galaxy, layer, 24) {
            generate_cell(&galaxy, key, &mut cell);
            for record in &cell {
                assert_eq!(resolve(&galaxy, record.id()), Ok(*record));
                resolved += 1;
            }
            let count = candidate_count(&galaxy, key);
            if let Some(beyond) = key.candidate_id(count) {
                assert_eq!(
                    resolve(&galaxy, beyond),
                    Err(ResolveSystemError::NoSuchSystem)
                );
            }
        }
        assert!(resolved > 20, "{layer:?}: {resolved} objects resolved");
        let mut thinned = 0;
        let steep = GalacticPosition::from_light_years([400.0, 400.0, 150.0]).unwrap();
        let first = CellKey::containing(layer, &steep).unwrap();
        let [x, y, z] = first.gen_cell().to_array();
        for step in 0..16 {
            let key = CellKey::new(layer, [x, y, z + step]).unwrap();
            for index in 0..candidate_count(&galaxy, key).min(2_000) {
                if evaluate_candidate(&galaxy, key, index) == CandidateOutcome::Thinned {
                    let id = key.candidate_id(index).unwrap();
                    assert_eq!(resolve(&galaxy, id), Err(ResolveSystemError::NoSuchSystem));
                    thinned += 1;
                }
            }
            if thinned >= 12 {
                break;
            }
        }
        assert!(thinned > 0, "{layer:?}: no thinned candidate was tried");
    }
}

/// Cells generated in several orders and objects resolved alone agree bit for bit.
#[test]
fn substellar_cells_do_not_depend_on_the_order_they_are_generated_in() {
    let galaxy = milky_way();
    for layer in LAYERS {
        let keys = sun_cells(&galaxy, layer, 12);
        assert_order_independent(&keys, |key| {
            let mut cell = Vec::new();
            generate_cell(&galaxy, *key, &mut cell);
            cell
        });
        let mut cell = Vec::new();
        generate_cell(&galaxy, keys[5], &mut cell);
        let ids: Vec<SystemId> = cell.iter().map(SystemRecord::id).collect();
        assert_order_independent(&ids, |id| resolve(&galaxy, *id));
    }
}

/// A designation names the object and reads back to its ID.
#[test]
fn substellar_designations_read_back_to_their_ids() {
    let galaxy = milky_way();
    let mut cell = Vec::new();
    for layer in LAYERS {
        for key in sun_cells(&galaxy, layer, 8) {
            generate_cell(&galaxy, key, &mut cell);
            for record in &cell {
                let text = record.id().designation().to_string();
                let parsed: Designation = text.parse().unwrap();
                assert_eq!(parsed.system(), record.id(), "{text}");
            }
        }
    }
}

/// Objects with a negative age exist in the young disc, and are "no system yet" until they are born
/// (P13.T3.c).
///
/// The clock window H is 1,000 years against the young disc's hundreds of millions, so only the
/// rogue planets are numerous enough to show one in a test-sized block; the brown dwarfs draw
/// their ages from the same components through the same `system.age` stream.
#[test]
#[ignore = "slow: millions of rogue planets on an arm ridge, to meet the few unborn"]
fn unborn_substellar_objects_exist_in_the_young_disc() {
    let galaxy = milky_way();
    let [.., (_, ridge), _] = five_places(&galaxy);
    {
        let layer = Layer::RoguePlanet;
        let block = Block::sized_for(&galaxy, layer, ridge, 3.0e6, 16);
        let mut unborn: Vec<SystemRecord> = Vec::new();
        let mut cell = Vec::new();
        for key in block.cells(layer) {
            generate_cell(&galaxy, key, &mut cell);
            unborn.extend(cell.iter().filter(|r| r.age_at_epoch().value() < 0.0));
        }
        println!(
            "{layer:?}: {} unborn in a {} ly block",
            unborn.len(),
            block.edge_ly
        );
        assert!(!unborn.is_empty(), "{layer:?}: nothing unborn on the ridge");
        for record in unborn {
            assert_eq!(record.population().name(), "young_thin_disc");
            assert_eq!(
                record.existence_at(UniverseTime::EPOCH),
                Existence::NoSystemYet
            );
            // Resolving needs no time: the ID names the object before it is born.
            assert_eq!(resolve(&galaxy, record.id()), Ok(record));
        }
    }
}

/// Writes one object: its ID and designation, where it is at the epoch, its mass, age and picked
/// component.
fn write_object(w: &mut GoldenWriter, label: &str, record: &SystemRecord) {
    let component = record.component().expect("a grid object has a component");
    let ly = record.epoch_position().cell().to_array();
    w.u64_hex(&format!("{label}.id"), record.id().raw());
    w.line(&format!(
        "{label}.designation = {}",
        record.id().designation()
    ));
    w.line(&format!(
        "{label}.ly = {} {} {}, component {} ({})",
        ly[0],
        ly[1],
        ly[2],
        component.index(),
        record.population().name()
    ));
    for (axis, offset) in ["x", "y", "z"]
        .into_iter()
        .zip(record.epoch_position().offset_metres())
    {
        w.f64(&format!("{label}.offset_{axis}_m"), offset);
    }
    w.f64(
        &format!("{label}.mass_msun"),
        record.primary_initial_mass().value(),
    );
    w.f64(&format!("{label}.age_yr"), record.age_at_epoch().value());
}

/// Twenty objects of each substellar layer for two galaxies, pinned (P13.T3.c): the first twenty
/// of the cells walked along x from the Sun-like point, with each cell's candidate count.
#[test]
fn substellar_objects_are_pinned() {
    const PER_LAYER: usize = 20;
    let galaxies = [
        ("mw", milky_way()),
        ("drawn", Galaxy::new(Seed::new(DRAWN_SEED))),
    ];
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut cell = Vec::new();
    for (tag, galaxy) in &galaxies {
        w.line(&format!("# galaxy {tag}, {}", galaxy.seed()));
        let abundance = galaxy.substellar();
        w.f64(
            &format!("{tag}.brown_dwarfs_per_system"),
            abundance.brown_dwarfs_per_system(),
        );
        w.f64(
            &format!("{tag}.rogue_planets_per_system"),
            abundance.rogue_planets_per_system(),
        );
        w.f64(
            &format!("{tag}.rogue_planet_cap_per_system"),
            abundance.rogue_planet_cap_per_system(),
        );
        for spec in SUBSTELLAR_LAYERS {
            let layer = spec.layer();
            let mut written = 0;
            for key in sun_cells(galaxy, layer, 64) {
                if written == PER_LAYER {
                    break;
                }
                generate_cell(galaxy, key, &mut cell);
                let [cx, cy, cz] = key.gen_cell().to_array();
                let label = format!("{tag}.{}", layer.letter());
                w.line(&format!(
                    "{label}.cell ({cx}, {cy}, {cz}): {} candidates, {} objects",
                    candidate_count(galaxy, key),
                    cell.len()
                ));
                for record in cell.iter().take(PER_LAYER - written) {
                    write_object(&mut w, &format!("{label}.o{written}"), record);
                    written += 1;
                }
            }
            assert_eq!(written, PER_LAYER, "{tag}: layer {}", layer.letter());
        }
    }
    golden!("placement/substellar", w.as_str());
}

// --- P13.T3.d: the bound where a violation is likeliest ---

/// Candidates of both substellar layers along the young arm ridges near the bar's ends, in the
/// flared outer disc and at the centre, 10⁶ or more per layer, with the thinning's debug assertion
/// that the density never exceeds the bound active (P13.T3.d).
#[test]
#[ignore = "slow: 10⁶ candidates of each substellar layer where the bound is tightest"]
fn the_thinning_bound_holds_for_the_substellar_layers() {
    /// How far past the bar's end the ridges are followed, light-years.
    const OUTWARD_LY: f64 = 2_000.0;
    /// The candidates each layer must reach.
    const LEAST: u64 = 1_000_000;
    #[expect(
        clippy::assertions_on_constants,
        reason = "the point is to fail at run time in a build whose debug assertions are off"
    )]
    {
        assert!(
            cfg!(debug_assertions),
            "this test drives a debug_assert! and is worthless without debug assertions"
        );
    }
    for n in 0..4_u64 {
        let galaxy = Galaxy::new(Seed::new(FIXTURE_SEED | 0x40 | n));
        let arms = galaxy.fields().arms();
        let start = arms.bar_half_length().value();
        for layer in LAYERS {
            let step = f64::from(layer.cell_size_ly());
            let mut keys = Vec::new();
            for ridge in 0..arms.count().get() {
                let mut r = start;
                while r <= start + OUTWARD_LY {
                    let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, ridge));
                    for z in [-step, 0.0] {
                        let at = GalacticPosition::from_light_years([r * cos, r * sin, z]).unwrap();
                        keys.push(CellKey::containing(layer, &at).unwrap());
                    }
                    r += step;
                }
            }
            // The flared outer disc, where the thin disc's height grows, and the central cells.
            for ring in 0..64 {
                let phi = f64::from(ring) * 0.098_174_770_424_681_04;
                let (sin, cos) = math::sin_cos(phi);
                for (r, z) in [(50_000.0, 2_000.0), (45_000.0, -1_500.0)] {
                    let at = GalacticPosition::from_light_years([r * cos, r * sin, z]).unwrap();
                    keys.push(CellKey::containing(layer, &at).unwrap());
                }
            }
            // The central cube, 128 ly across for the brown dwarfs, whose cells there hold
            // thousands of candidates each, and 16 ly for the rogue planets, whose hold tens of
            // thousands: together they bring each layer past 10⁶.
            let reach = if layer == Layer::BrownDwarf { 4 } else { 2 };
            for x in -reach..reach {
                for y in -reach..reach {
                    for z in -reach..reach {
                        keys.push(CellKey::new(layer, [x, y, z]).unwrap());
                    }
                }
            }
            keys.sort_unstable();
            keys.dedup();
            let mut candidates = 0_u64;
            let mut cell = Vec::new();
            for key in &keys {
                candidates += u64::from(candidate_count(&galaxy, *key));
                generate_cell(&galaxy, *key, &mut cell);
            }
            println!(
                "seed {n}, layer {}: {} cells, {candidates} candidates",
                layer.letter(),
                keys.len()
            );
            if n == 0 {
                assert!(candidates >= LEAST, "{layer:?}: {candidates}");
            }
        }
    }
}

/// Every record's bits, for the determinism check below.
fn record_bits(record: &SystemRecord) -> [u64; 7] {
    let [x, y, z] = record.epoch_position().offset_metres();
    [
        record.id().raw(),
        bits(x),
        bits(y),
        bits(z),
        bits(record.primary_initial_mass().value()),
        bits(record.age_at_epoch().value()),
        u64::try_from(record.population().index()).unwrap(),
    ]
}

/// Two galaxies built from one seed place the same objects, bit for bit.
#[test]
fn two_runs_place_the_same_objects() {
    let [a, b] = [milky_way(), milky_way()];
    for layer in LAYERS {
        for key in sun_cells(&a, layer, 4) {
            let (mut x, mut y) = (Vec::new(), Vec::new());
            generate_cell(&a, key, &mut x);
            generate_cell(&b, key, &mut y);
            let xs: Vec<_> = x.iter().map(record_bits).collect();
            let ys: Vec<_> = y.iter().map(record_bits).collect();
            assert_eq!(xs, ys);
        }
    }
}

/// The planetary stage's entry refuses a free-floating object until plan 14's P14.T27 gives brown
/// dwarfs and rogue planets their hosts; P13.T5.a routes brown dwarfs through the stellar stage,
/// which the planetary context does not yet read for them.
#[test]
fn a_free_floating_object_has_no_stellar_context_yet() {
    use hyperion_sim::planetary::SystemContext;
    let galaxy = milky_way();
    let mut cell = Vec::new();
    for layer in LAYERS {
        let record = sun_cells(&galaxy, layer, 64)
            .into_iter()
            .find_map(|key| {
                generate_cell(&galaxy, key, &mut cell);
                cell.first().copied()
            })
            .expect("the solar circle holds free-floating objects");
        assert_eq!(
            SystemContext::for_system(&galaxy, record.id()).err(),
            Some(ResolveSystemError::LayerNotGenerated(layer))
        );
    }
}
