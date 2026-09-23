//! Placement against the field: the density the thinning realises, the marks it draws, and the
//! bound where a violation is likeliest (plan 03, P03.T8.a–c).
//!
//! These are the brainstorm's "density against the field" and "bound checks" for the grid
//! ("Testing"). The thinning is exact only if two things hold at once: a cell's candidate count
//! follows its bound, and acceptance follows density ÷ bound. Then the systems that survive are a
//! Poisson process of exactly the layer's density, so the count in any region is Poisson with the
//! density's integral over that region as its mean. That is what is checked here, by counting placed
//! systems in blocks of whole cells against an independent midpoint sum of the field
//! (`common::reference_box_integral`) through plan 01's Poisson interval.
//!
//! A block is a cube of whole cells of the layer's own grid, so every system of its cells lies
//! inside it and the count needs no distance test. Blocks sit where the field is hardest: at the
//! Sun-like point, high above the plane, in the bulge at 1,000 ly, at the nuclear disc's scale
//! length, and across a young arm ridge just outside the bar's end, where the bound is tightest.
//! The arm block is also cut into eighths of a cell along each axis, which is how the failure a
//! broken bound would cause — a cell-shaped patch of missing stars, which "no ordinary test would
//! notice" — becomes visible.
//!
//! One block's count resolves a bias of only a few per cent in the sparse layers, so the counts and
//! the slabs are also summed over every block and galaxy and tested once more, where a bias of a
//! per cent in layer A and of a few tenths in layers C to E shows.

#[expect(
    dead_code,
    reason = "these checks use the placement helpers of tests/common alone"
)]
mod common;

use common::{
    for_each_box_midpoint, reference_box_component_integrals, reference_box_integral, sunlike_point,
};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::fields::MAX_COMPONENTS;
use hyperion_sim::galaxy::imf::{MassBand, MassFunctionKind};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, STELLAR_LAYERS, SystemRecord, generate_cell};
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::units::Years;
use hyperion_testkit::stats::{
    ALPHA, KolmogorovSmirnov, assert_p_value, assert_poisson_count, chi_square_gof, ks_one_sample,
};

/// The seed of the Milky Way fixture in these checks.
const FIXTURE_SEED: u64 = 0x0308_0a00_0000_0000;

/// The three drawn galaxies the counts are checked in beside the fixture, chosen by nothing but
/// their seeds: their parameters, and so their densities, differ by factors of a few.
const SEEDS: [u64; 3] = [
    0x0308_0a00_0000_0001,
    0x0308_0a00_5eed_0002,
    0x0308_0a00_cafe_0003,
];

/// How many midpoints of the reference sum fall inside one generation cell on each axis.
///
/// Eight makes the sum's step an eighth of a cell — 1 ly in layer A, 16 ly in layer E — whose
/// relative error is about `(h ÷ L)² ÷ 24` against the shortest scale the layers meet, the nuclear
/// disc's 90–150 ly height: under a part in a thousand, well inside the Poisson interval of every
/// count below. It also makes each step an eighth of a cell, so one pass gives both a block's
/// expected count and the expected count of each eighth for the arm block's slabs.
const STEPS_PER_CELL: u32 = 8;

/// How many eighths of a cell the arm block's counts are cut into along each axis.
const SLABS: usize = 8;

/// One place the counts are checked: a cube of `cells_per_axis³` cells of each layer's own grid about
/// a point.
///
/// The cell count is chosen per place so that layer A holds several hundred systems and layer E tens
/// of thousands — the densest places need one cell and the thinnest a dozen — which keeps the Poisson
/// interval tight enough to catch a per-cent error and the reference sum cheap enough to run over
/// four galaxies.
#[derive(Debug, Clone, Copy)]
struct Block {
    what: &'static str,
    at: [f64; 3],
    cells_per_axis: u32,
}

/// A count as an `f64`, exact for every count these tests reach.
fn count_as_f64(count: u64) -> f64 {
    assert!(count < 1 << 52, "{count} is too large to count exactly");
    #[expect(
        clippy::cast_precision_loss,
        reason = "the assertion above holds the count below 2^52, where every integer is exact"
    )]
    let value = count as f64;
    value
}

/// The blocks of one galaxy, in the order the task names them.
///
/// The last two follow the galaxy's own parameters: the nuclear disc's scale length, where its
/// density falls fastest, and a young arm ridge just outside the bar's end, where the brainstorm
/// found the true maximum beating a cell's corner estimate by 12% ("Exact placement by thinning").
fn blocks(galaxy: &Galaxy) -> Vec<Block> {
    let sun = sunlike_point(galaxy).to_light_years_f64();
    // Radius `L` at 45° to both axes, so the block stays inside one octant.
    let nuclear = galaxy.params().nuclear_disc().length().value() * 0.5_f64.sqrt();
    vec![
        Block {
            what: "the Sun-like point",
            at: sun,
            cells_per_axis: 10,
        },
        Block {
            what: "high above the plane",
            at: [sun[0], sun[1], 1_500.0],
            cells_per_axis: 12,
        },
        Block {
            what: "the bulge at 1,000 ly",
            at: [700.0, 700.0, 0.0],
            cells_per_axis: 4,
        },
        Block {
            what: "the nuclear disc's scale length",
            at: [nuclear, nuclear, 0.0],
            cells_per_axis: 1,
        },
        arm_block(galaxy),
    ]
}

/// A block straddling a young arm ridge just outside the bar's end, wide enough to cut across six
/// cells of every layer, layer E's 128 ly included.
fn arm_block(galaxy: &Galaxy) -> Block {
    let arms = galaxy.fields().arms();
    let r = 1.15 * arms.bar_half_length().value();
    let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, 0));
    Block {
        what: "a young arm ridge outside the bar",
        at: [r * cos, r * sin, 0.0],
        cells_per_axis: 6,
    }
}

/// The galaxies the counts are checked in: the fixture the brainstorm's comparisons use, and three
/// drawn from seeds.
fn galaxies() -> Vec<(String, Galaxy)> {
    let mut all = vec![(
        "milky_way_like".to_owned(),
        Galaxy::from_params(Seed::new(FIXTURE_SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral"),
    )];
    for seed in SEEDS {
        all.push((format!("seed {seed:#018x}"), Galaxy::new(Seed::new(seed))));
    }
    all
}

/// The lowest cell of a block on a layer's grid.
fn min_cell(layer: Layer, block: &Block) -> CellKey {
    let at = GalacticPosition::from_light_years(block.at).expect("a block's point is in the cube");
    let centre = CellKey::containing(layer, &at).expect("a stellar layer at a point in the cube");
    let half = i32::try_from(block.cells_per_axis / 2).expect("a dozen cells at most");
    let cell = centre.gen_cell().to_array().map(|c| c - half);
    CellKey::new(layer, cell).expect("a block lies well inside the root cube")
}

/// Calls `f` for every system of a block, cell by cell in `CellKey` order, with the cell it came
/// from.
fn for_each_system_of_block(
    galaxy: &Galaxy,
    layer: Layer,
    block: &Block,
    mut f: impl FnMut(CellKey, &SystemRecord),
) {
    let first = min_cell(layer, block).gen_cell().to_array();
    let span = i32::try_from(block.cells_per_axis).expect("a dozen cells at most");
    let mut cell = Vec::new();
    for dx in 0..span {
        for dy in 0..span {
            for dz in 0..span {
                let key = CellKey::new(layer, [first[0] + dx, first[1] + dy, first[2] + dz])
                    .expect("a block lies well inside the root cube");
                generate_cell(galaxy, key, &mut cell);
                for record in &cell {
                    f(key, record);
                }
            }
        }
    }
}

/// How many systems of a layer a block holds.
fn block_count(galaxy: &Galaxy, layer: Layer, block: &Block) -> u64 {
    let mut count = 0_u64;
    for_each_system_of_block(galaxy, layer, block, |_, _| count += 1);
    count
}

/// The expected number of systems of a layer in a block, from the field alone.
fn block_mean(galaxy: &Galaxy, layer: Layer, block: &Block, band: MassBand) -> f64 {
    reference_box_integral(
        galaxy,
        band,
        min_cell(layer, block).origin_ly(),
        block.cells_per_axis * layer.cell_size_ly(),
        block.cells_per_axis * STEPS_PER_CELL,
    )
}

/// Each stellar layer's placed count and expected count, summed over blocks, in `STELLAR_LAYERS`
/// order.
type LayerTotals = [(u64, f64); STELLAR_LAYERS.len()];

/// Every layer's placed count in every given block of one galaxy against the field's own integral,
/// returning each layer's count and mean summed over the blocks.
fn assert_blocks_match_the_field(name: &str, galaxy: &Galaxy, blocks: &[Block]) -> LayerTotals {
    let mut totals: LayerTotals = [(0, 0.0); STELLAR_LAYERS.len()];
    for block in blocks {
        for (spec, total) in STELLAR_LAYERS.iter().zip(&mut totals) {
            let layer = spec.layer();
            let count = block_count(galaxy, layer, block);
            let mean = block_mean(galaxy, layer, block, spec.band());
            total.0 += count;
            total.1 += mean;
            println!(
                "{name}, {}, layer {}: {count} systems of {mean:.1} expected over {} cells",
                block.what,
                layer.letter(),
                block.cells_per_axis.pow(3),
            );
            assert!(
                mean > 0.0 && mean < 4.0e6,
                "{name}, {}, layer {}: {mean} expected systems is no test",
                block.what,
                layer.letter()
            );
            assert_poisson_count(
                &format!("{name}, {}, layer {}", block.what, layer.letter()),
                count,
                mean,
                ALPHA,
            );
        }
    }
    totals
}

/// Each layer's count summed over every block given, against the summed mean.
///
/// The blocks are disjoint and the galaxies independent, so the sum of the counts is Poisson with
/// the sum of the means. A single block of layer A or B holds a few hundred to a few tens of
/// thousands of systems, so the best of them resolves a bias of 2–3% at α and most resolve only
/// 5–15%; the sum over the slow test's twenty blocks resolves about 1% in layers A and B and 0.2–0.3%
/// in C to E. Found by biasing the generator in validation: a thinning 1% short in layer A passed
/// every block of every galaxy (and, at −0.87% against the sum's ±0.97%, this check too).
fn assert_pooled_counts_match_the_field(name: &str, totals: &LayerTotals) {
    for (spec, &(count, mean)) in STELLAR_LAYERS.iter().zip(totals) {
        let deviation = 100.0 * (count_as_f64(count) - mean) / mean;
        println!(
            "{name}, layer {}: {count} systems of {mean:.1} expected, {deviation:+.3}%",
            spec.layer().letter()
        );
        assert_poisson_count(
            &format!("{name}, layer {}", spec.layer().letter()),
            count,
            mean,
            ALPHA,
        );
    }
}

/// Adds one set of per-layer totals into another.
fn add_totals(sum: &mut LayerTotals, more: &LayerTotals) {
    for (total, &(count, mean)) in sum.iter_mut().zip(more) {
        total.0 += count;
        total.1 += mean;
    }
}

// --- P03.T8.a: density against the field, per seed ---

/// The Sun-like point and the arm ridge of the fixture: the fast guard on the slow check below.
#[test]
fn placed_density_matches_the_field_at_the_sun_like_point() {
    let (name, galaxy) = galaxies().swap_remove(0);
    let all = blocks(&galaxy);
    let totals = assert_blocks_match_the_field(&name, &galaxy, &[all[0], all[4]]);
    assert_pooled_counts_match_the_field(&format!("{name}, both blocks"), &totals);
}

/// Every layer's placed count in every block of four galaxies against the field (P03.T8.a).
#[test]
#[ignore = "slow: five blocks of every layer in four galaxies, each against a midpoint sum"]
fn placed_density_matches_the_field_in_every_block_of_every_seed() {
    let mut pooled: LayerTotals = [(0, 0.0); STELLAR_LAYERS.len()];
    for (name, galaxy) in galaxies() {
        let totals = assert_blocks_match_the_field(&name, &galaxy, &blocks(&galaxy));
        add_totals(&mut pooled, &totals);
    }
    assert_pooled_counts_match_the_field("every block of the four galaxies", &pooled);
}

/// One layer's counts in the eighths of a cell along each axis, and the field's unscaled integrals
/// over the same eighths.
#[derive(Debug, Clone, Copy)]
struct Slabs {
    observed: [[u64; SLABS]; 3],
    expected: [[f64; SLABS]; 3],
}

impl Slabs {
    const EMPTY: Self = Self {
        observed: [[0; SLABS]; 3],
        expected: [[0.0; SLABS]; 3],
    };

    /// Adds another block's slabs of the same layer: the counts are independent Poisson variables,
    /// so the sums are Poisson with the summed means, and conditioning on the summed total is as
    /// sound as conditioning on one block's.
    fn add(&mut self, other: &Self) {
        for axis in 0..3 {
            for slab in 0..SLABS {
                self.observed[axis][slab] += other.observed[axis][slab];
                self.expected[axis][slab] += other.expected[axis][slab];
            }
        }
    }
}

/// Counts in the eighths of a cell along each axis, across the arm ridge, against their own
/// integrals: a cell-shaped patch of missing systems would show as a deficit away from the cells'
/// faces, and a bound that is not a true bound would put one there (P03.T8.a). Returns each layer's
/// slabs, in `STELLAR_LAYERS` order, for the pooled check.
fn assert_no_cell_shaped_patches(
    name: &str,
    galaxy: &Galaxy,
    block: &Block,
) -> [Slabs; STELLAR_LAYERS.len()] {
    let mut all = [Slabs::EMPTY; STELLAR_LAYERS.len()];
    for (spec, slabs) in STELLAR_LAYERS.iter().zip(&mut all) {
        let layer = spec.layer();
        let size_ly = layer.cell_size_ly();
        let slab_ly = i32::try_from(size_ly).expect("a cell is at most 128 ly")
            / i32::try_from(SLABS).expect("eight slabs");
        let mut observed = [[0_u64; SLABS]; 3];
        for_each_system_of_block(galaxy, layer, block, |key, record| {
            let origin = key.origin_ly();
            let ly = record.epoch_position().cell().to_array();
            for (axis, counts) in observed.iter_mut().enumerate() {
                let slab = usize::try_from((ly[axis] - origin[axis]) / slab_ly)
                    .expect("a system lies inside its own cell");
                counts[slab] += 1;
            }
        });

        // The same sum as the block's mean, with each midpoint's weight added to the eighth of the
        // cell it falls in on each axis: `STEPS_PER_CELL` steps to a cell, so the step index modulo
        // eight is the slab.
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        let mut expected = [[0.0; SLABS]; 3];
        for_each_box_midpoint(
            min_cell(layer, block).origin_ly(),
            block.cells_per_axis * size_ly,
            block.cells_per_axis * STEPS_PER_CELL,
            |point, weight, steps| {
                let mass = weight * fields.layer_density(shares, spec.band(), point);
                for (axis, means) in expected.iter_mut().enumerate() {
                    let step = usize::try_from(steps[axis]).expect("a step index is small");
                    means[step % SLABS] += mass;
                }
            },
        );

        *slabs = Slabs { observed, expected };
        assert_slabs_match(name, layer, slabs);
    }
    all
}

/// One layer's slabs against the field, conditioned on the observed total (P03.T8.a).
fn assert_slabs_match(name: &str, layer: Layer, slabs: &Slabs) {
    let Slabs { observed, expected } = slabs;
    for axis in 0..3 {
        let total = observed[axis].iter().sum::<u64>();
        let mean = expected[axis].iter().sum::<f64>();
        assert!(
            total > 0 && mean > 0.0,
            "{name}, layer {}, axis {axis}: nothing to compare",
            layer.letter()
        );
        // Scaled to the observed total, as the chi-square requires. This is a test of the shape
        // inside a cell and not of the total, which `assert_blocks_match_the_field` tests: a
        // fluctuation in the total must not be counted against the shape twice.
        let scale = count_as_f64(total) / mean;
        let scaled: Vec<f64> = expected[axis].iter().map(|e| e * scale).collect();
        let fit = chi_square_gof(&observed[axis], &scaled);
        // The two slabs either side of a cell face, which adjoin across it: the pair a patch
        // thinning the cells' interiors would leave standing. Conditional on the total the pair
        // is binomial, so the Poisson interval of its scaled mean is the conservative reading.
        let faces = observed[axis][0] + observed[axis][SLABS - 1];
        let faces_mean = (expected[axis][0] + expected[axis][SLABS - 1]) * scale;
        println!(
            "{name}, layer {}, axis {axis}: {total} systems, χ² p = {:.3} over {} eighths, \
             {faces} of {faces_mean:.1} beside the faces",
            layer.letter(),
            fit.p_value,
            fit.bins,
        );
        assert_p_value(
            &format!(
                "{name}, layer {} across the arm, eighths of a cell on axis {axis}",
                layer.letter()
            ),
            fit.p_value,
            ALPHA,
        );
        assert_poisson_count(
            &format!(
                "{name}, layer {}, the slabs either side of a cell face on axis {axis}",
                layer.letter()
            ),
            faces,
            faces_mean,
            ALPHA,
        );
    }
}

#[test]
#[ignore = "slow: the arm block of every layer in four galaxies, cut into eighths of a cell"]
fn placed_density_has_no_cell_shaped_patches_across_an_arm_ridge() {
    let mut pooled = [Slabs::EMPTY; STELLAR_LAYERS.len()];
    for (name, galaxy) in galaxies() {
        let slabs = assert_no_cell_shaped_patches(&name, &galaxy, &arm_block(&galaxy));
        for (sum, more) in pooled.iter_mut().zip(&slabs) {
            sum.add(more);
        }
    }
    // The four arm blocks together: one galaxy's slabs resolve a patch of about a tenth, the four
    // together a few per cent. A 3% deficit in the middle quarter of every cell passed each
    // galaxy's check alone when the generator was perturbed in validation, and failed this one at
    // p = 1.6 × 10⁻⁷.
    for (spec, slabs) in STELLAR_LAYERS.iter().zip(&pooled) {
        assert_slabs_match("the four galaxies pooled", spec.layer(), slabs);
    }
}

/// The arm block sits where the task says: on a ridge of the young disc, outside the bar's end, and
/// wide enough to cross several 128 ly cells.
#[test]
fn the_arm_block_straddles_a_young_arm_ridge_outside_the_bar() {
    for (name, galaxy) in galaxies() {
        let block = arm_block(&galaxy);
        let arms = galaxy.fields().arms();
        let [x, y, _] = block.at;
        let at = arms.point(x, y);
        // On a ridge the phase is a multiple of 2π, so its cosine is 1 and the sharp arm's profile
        // is at its peak.
        assert!(
            (at.cos_phase() - 1.0).abs() < 1e-9,
            "{name}: the arm block is off the ridge, cos φ = {}",
            at.cos_phase()
        );
        // Outside the bar's end the fade-in has all but finished, which is why the arm is sharpest
        // against a cell here.
        let r = at.r_sq().sqrt();
        assert!(
            arms.fade(r) > 0.7 && r > arms.bar_half_length().value(),
            "{name}: the arm block sits at {r} ly, inside the bar's {} ly",
            arms.bar_half_length().value()
        );
        assert!(block.cells_per_axis >= 3, "the block crosses no cell face");
        // The layer's density at the ridge is under the bound of the layer-E cell holding it.
        let position =
            GalacticPosition::from_light_years(block.at).expect("a ridge point is in the cube");
        let key = CellKey::containing(Layer::E, &position).expect("a layer-E cell holds the ridge");
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        let density = fields.layer_density(shares, MassBand::E, &PointLy::from(&position));
        let bound = fields.layer_bound(shares, MassBand::E, &key.cell_box());
        assert!(
            density > 0.0 && density <= bound,
            "{name}: layer E holds {density} at the ridge against a bound of {bound}"
        );
    }
}

// --- P03.T8.b: the marks ---

/// Calls `f` for the systems of `layer` in the cells around `at`, in widening blocks about the
/// nearest cell, until at least `wanted` of them have been seen; returns how many that was.
fn take_systems(
    galaxy: &Galaxy,
    layer: Layer,
    at: &GalacticPosition,
    wanted: usize,
    mut f: impl FnMut(&SystemRecord),
) -> usize {
    let centre = CellKey::containing(layer, at).expect("a stellar layer at a point in the cube");
    let [x, y, z] = centre.gen_cell().to_array();
    let mut cell = Vec::new();
    let mut seen = 0;
    for shell in 0..1_000_i32 {
        for dx in -shell..=shell {
            for dy in -shell..=shell {
                for dz in -shell..=shell {
                    if dx.abs().max(dy.abs()).max(dz.abs()) != shell {
                        continue;
                    }
                    let Ok(key) = CellKey::new(layer, [x + dx, y + dy, z + dz]) else {
                        continue;
                    };
                    generate_cell(galaxy, key, &mut cell);
                    for record in &cell {
                        f(record);
                    }
                    seen += cell.len();
                }
            }
        }
        if seen >= wanted {
            return seen;
        }
    }
    panic!(
        "layer {} holds no {wanted} systems near the point",
        layer.letter()
    )
}

/// A sample against a cumulative distribution at α.
#[track_caller]
fn assert_ks(name: &str, samples: &mut [f64], cdf: impl Fn(f64) -> f64) -> KolmogorovSmirnov {
    let ks = ks_one_sample(samples, cdf);
    assert_p_value(name, ks.p_value, ALPHA);
    ks
}

/// Primary initial masses, per layer, against the galaxy's mass function restricted to the layer's
/// band, under both functions the sim supports (P03.T8.b).
#[test]
fn marks_draw_primary_masses_from_the_mass_function_in_each_band() {
    /// Masses per layer. The sampler takes one uniform through a closed-form quantile, so this is a
    /// sharp test of the band it draws in, not of a fit.
    const DRAWS: usize = 20_000;
    for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
        let galaxy = Galaxy::with_mass_function(Seed::new(FIXTURE_SEED | 0x10), kind);
        let at = sunlike_point(&galaxy);
        for spec in STELLAR_LAYERS {
            let band = spec.band();
            let mut masses = Vec::with_capacity(DRAWS);
            take_systems(&galaxy, spec.layer(), &at, DRAWS, |record| {
                masses.push(record.primary_initial_mass().value());
            });
            assert!(
                masses.iter().all(|m| (band.lo()..=band.hi()).contains(m)),
                "{kind:?}, layer {}: a primary outside its band",
                spec.layer().letter()
            );
            let function = galaxy.mass_function();
            let whole = function.integral(band.lo(), band.hi());
            assert!(whole > 0.0, "{kind:?}: the band {band:?} holds nothing");
            let ks = assert_ks(
                &format!("{kind:?} masses in layer {}", spec.layer().letter()),
                &mut masses,
                |m| (function.integral(band.lo(), m) / whole).clamp(0.0, 1.0),
            );
            println!(
                "{kind:?}, layer {}: {} masses, D = {:.4}, p = {:.3}",
                spec.layer().letter(),
                ks.effective_n,
                ks.statistic,
                ks.p_value
            );
        }
    }
}

/// Ages, per density component, against that component's own distribution (P03.T8.b).
///
/// The pick is over components and not populations precisely because a component — an old thin
/// sub-disc, a halo component — owns the age distribution (plan 03, Design note 3), so this is also
/// the check that the pick and the age draw agree on which component a system came from. The places
/// are chosen to reach every component: the discs near the Sun, the bulge and the bar in the inner
/// galaxy, the nuclear disc at its own scale length, the halo far off the plane.
#[test]
fn marks_draw_ages_from_the_picked_components_distribution() {
    /// The fewest systems a component needs before its ages are tested.
    const LEAST: usize = 400;
    /// How many systems each layer contributes at each place.
    const PER_PLACE: usize = 4_000;
    let galaxy = Galaxy::from_params(
        Seed::new(FIXTURE_SEED | 0x20),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let sun = sunlike_point(&galaxy).to_light_years_f64();
    let nuclear = galaxy.params().nuclear_disc().length().value() * 0.5_f64.sqrt();
    let places = [
        sun,
        [700.0, 700.0, 0.0],
        [nuclear, nuclear, 0.0],
        [sun[0], sun[1], 6_000.0],
        // The young disc is under half a per cent of the field and is found on an arm ridge or not
        // at all; it is also the one component whose ages reach below zero.
        arm_block(&galaxy).at,
    ];
    let mut ages: Vec<Vec<f64>> = vec![Vec::new(); MAX_COMPONENTS];
    for place in places {
        let at = GalacticPosition::from_light_years(place).expect("a place inside the cube");
        for spec in STELLAR_LAYERS {
            take_systems(&galaxy, spec.layer(), &at, PER_PLACE, |record| {
                let component = record
                    .component()
                    .expect("a placed system comes from a density component");
                ages[component.index()].push(record.age_at_epoch().value());
            });
        }
    }

    let mut tested = 0;
    for id in galaxy.fields().component_ids() {
        let component = galaxy.fields().component(id);
        let (index, name) = (id.index(), component.population().name());
        let sample = &mut ages[index];
        if sample.len() < LEAST {
            println!(
                "component {index} ({name}): {} systems, too few",
                sample.len()
            );
            continue;
        }
        let distribution = component.ages();
        let (lo, hi) = (distribution.min().value(), distribution.max().value());
        assert!(
            sample.iter().all(|age| (lo..=hi).contains(age)),
            "component {index} ({name}): an age outside {lo}–{hi} yr"
        );
        let ks = assert_ks(
            &format!("ages of component {index} ({name})"),
            sample,
            |age| distribution.cdf(Years::new(age)),
        );
        println!(
            "component {index} ({name}): {} ages, D = {:.4}, p = {:.3}",
            ks.effective_n, ks.statistic, ks.p_value
        );
        tested += 1;
    }
    // Twelve of the fixture's sixteen components are reached this way; the four that are not are
    // accreted halo components of a per-cent share each, which no place the five layers reach holds
    // enough of. Ten is the floor, so that a change to the field's component list is a finding here
    // and not a silent loss of coverage.
    assert!(
        tested >= 10,
        "only {tested} of the field's components were reached, so the ages are barely tested"
    );
}

/// The components the thinning picks in a block, against the odds the field gives them (P03.T8.b).
///
/// The weights are `share × density` integrated over the block, which is what Design note 3 makes
/// the pick proportional to. Coverage is a hard assertion and not a printout: a component the block
/// holds a real share of must return systems, since a pick that silently never reached one would
/// leave its stars, and their ages and metallicities, out of the galaxy altogether.
fn assert_component_odds(name: &str, galaxy: &Galaxy, layer: Layer, block: &Block) {
    let spec = STELLAR_LAYERS
        .into_iter()
        .find(|spec| spec.layer() == layer)
        .expect("a stellar layer");
    let integrals = reference_box_component_integrals(
        galaxy,
        min_cell(layer, block).origin_ly(),
        block.cells_per_axis * layer.cell_size_ly(),
        block.cells_per_axis * STEPS_PER_CELL,
    );
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let weights: Vec<f64> = fields
        .components()
        .iter()
        .zip(integrals)
        .map(|(component, integral)| shares.component_share(spec.band(), component) * integral)
        .collect();

    let mut observed = vec![0_u64; weights.len()];
    for_each_system_of_block(galaxy, layer, block, |_, record| {
        let component = record
            .component()
            .expect("a placed system comes from a density component");
        observed[component.index()] += 1;
    });

    let total = observed.iter().sum::<u64>();
    let mean: f64 = weights.iter().sum();
    assert!(total > 0 && mean > 0.0, "{name}: nothing to compare");
    let scale = count_as_f64(total) / mean;
    let scaled: Vec<f64> = weights.iter().map(|w| w * scale).collect();
    for (index, &expected) in scaled.iter().enumerate() {
        if expected >= 5.0 {
            let id = fields
                .component_id(index)
                .expect("an index of the field's own components");
            assert!(
                observed[index] > 0,
                "{name}: component {index} ({}) expects {expected:.1} systems and got none",
                fields.component(id).population().name()
            );
        }
    }
    let fit = chi_square_gof(&observed, &scaled);
    println!(
        "{name}, layer {}: {total} systems over {} components in {} bins, p = {:.3}",
        layer.letter(),
        weights.len(),
        fit.bins,
        fit.p_value
    );
    assert_p_value(
        &format!("{name}, the component odds of layer {}", layer.letter()),
        fit.p_value,
        ALPHA,
    );
}

#[test]
fn marks_pick_components_with_the_odds_the_field_gives_them() {
    let galaxy = Galaxy::from_params(
        Seed::new(FIXTURE_SEED | 0x30),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let all = blocks(&galaxy);
    // The Sun-like point, where the discs and the halo compete, and the outer bulge, where the
    // bulge, the bar and the nuclear disc do.
    for (block, layer) in [(all[0], Layer::A), (all[2], Layer::A), (all[2], Layer::C)] {
        assert_component_odds(
            &format!("the fixture at {}", block.what),
            &galaxy,
            layer,
            &block,
        );
    }
}

// --- P03.T8.c: the bound exercised where violations are likeliest ---

/// Every cell of every layer along the young arm ridges, from the bar's end outward for 2,000 ly,
/// for ten seeds, so that the bound-check assertion of P03.T4.b runs where the brainstorm says a
/// violation is likeliest (P03.T8.c).
///
/// The assertion this drives is a `debug_assert!`, so the test is worth nothing in a build without
/// debug assertions and says so at the top. `just test` runs the `dev` profile and `just test-slow`
/// the `slow-test` profile, and both keep them on.
///
/// It complements plan 02's hunting test, which scans the bound function itself over random cells:
/// this one drives the whole placement path — the bound, the Poisson draw, each candidate's position
/// and the weighted density folded in component order — over every cell the ridges cross and each
/// cell's neighbours.
#[test]
#[ignore = "slow: every layer's cells along the arm ridges of ten seeds"]
fn the_thinning_bound_holds_along_the_young_arm_ridges() {
    /// How far past the bar's end the ridges are followed, light-years.
    const OUTWARD_LY: f64 = 2_000.0;
    #[expect(
        clippy::assertions_on_constants,
        reason = "the point is to fail at run time in a build whose debug assertions are off, not \
                  to refuse to compile one"
    )]
    {
        assert!(
            cfg!(debug_assertions),
            "this test drives a debug_assert! and is worthless without debug assertions: run it \
             under `just test` (dev) or `just test-slow` (slow-test), never a plain release build"
        );
    }
    let mut cells = 0_u64;
    let mut systems = 0_u64;
    for n in 0..10_u64 {
        let galaxy = Galaxy::new(Seed::new(FIXTURE_SEED | 0x40 | n));
        let arms = galaxy.fields().arms();
        let start = arms.bar_half_length().value();
        for spec in STELLAR_LAYERS {
            let layer = spec.layer();
            let step = f64::from(layer.cell_size_ly()) / 4.0;
            let mut walked = Vec::new();
            for ridge in 0..arms.count().get() {
                let mut r = start;
                while r <= start + OUTWARD_LY {
                    let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, ridge));
                    let at = GalacticPosition::from_light_years([r * cos, r * sin, 0.0])
                        .expect("a point on a ridge is inside the cube");
                    let on_ridge = CellKey::containing(layer, &at)
                        .expect("a stellar layer at a point in the cube");
                    // The ridge's own cell and its neighbours: a bound is taken over a whole cell,
                    // and a ridge that crosses a face stresses the cell on the other side as much.
                    let [x, y, z] = on_ridge.gen_cell().to_array();
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                if let Ok(key) = CellKey::new(layer, [x + dx, y + dy, z + dz]) {
                                    walked.push(key);
                                }
                            }
                        }
                    }
                    r += step;
                }
            }
            walked.sort_unstable();
            walked.dedup();
            let mut cell = Vec::new();
            for key in walked {
                generate_cell(&galaxy, key, &mut cell);
                cells += 1;
                systems += u64::try_from(cell.len()).expect("a cell holds few systems");
            }
        }
    }
    println!("{cells} cells along the arm ridges of ten seeds held {systems} systems");
    // The walk must have placed stars, or the assertion never ran on a candidate.
    assert!(
        systems > 100_000,
        "only {systems} systems over {cells} cells: the bound was barely exercised"
    );
}
