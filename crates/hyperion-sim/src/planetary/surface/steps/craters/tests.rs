//! R09.T12.d's tests of the coarse craters (Design notes 5, 10, 12 and 13; R09.T7.c's and T8's
//! contract for T12.d).

use hyperion_surface::craters::{crater_shape, cumulative_density};
use hyperion_surface::field::{CoarseField, FieldView, ResolutionCode, coarse_level};
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

use super::*;
use crate::planetary::hooks::SurfaceSeed;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::planetary::surface::steps::plates::cell_of;
use crate::planetary::surface::{quantise, reference};

/// A body's grids, for its passes.
struct World {
    inputs: CoarseInputs,
    grid: CellGraph,
    climate: CellGraph,
}

impl World {
    fn new(inputs: CoarseInputs) -> Self {
        let level = coarse_level(inputs.radius());
        Self {
            grid: CellGraph::for_field(level),
            climate: CellGraph::for_climate(level),
            inputs,
        }
    }

    fn pass(&self, seed: SurfaceSeed) -> Pass<'_> {
        Pass::new(seed, &self.inputs, &self.grid, &self.climate)
    }
}

/// The step on `inputs`' initial state under the reference seed: the state after [`run`] and after
/// [`run_after_erosion`].
fn cratered(world: &World) -> (Working, Working) {
    let pass = world.pass(reference::SEED);
    let mut first = Working::initial(&pass);
    run(&pass, &mut first);
    let mut second = first.clone();
    run_after_erosion(&pass, &mut second);
    (first, second)
}

/// The counts over the reference Moon match min(production, saturation) per octave above `D_b`
/// (each within its Poisson interval at α = 10⁻³), and about 300 lie above 100 km with N(1 km) at
/// 4.4 Gyr (Trask's saturation over its 3.79 × 10⁷ km²: 299.7; Design note 10).
#[test]
fn the_moon_s_counts_are_min_production_saturation() {
    let world = World::new(reference::moon_like());
    let pass = world.pass(reference::SEED);
    let (state, _) = cratered(&world);
    let octaves = octaves(&pass);
    assert!(octaves.len() >= 4, "{} octaves", octaves.len());
    let params = world.inputs.craters();
    let mut above_100 = 0.0;
    for octave in &octaves {
        let (low, high) = (octave.diameters.lower(), octave.diameters.upper());
        let drawn = state
            .craters
            .iter()
            .filter(|c| c.diameter.value() >= low.value() && c.diameter.value() < high.value())
            .count();
        eprintln!(
            "octave {} ({:.0}–{:.0} km): {drawn} drawn, {:.1} expected; production {:.3e}, \
             saturation {:.3e} km⁻²",
            octave.index,
            low.value() / 1e3,
            high.value() / 1e3,
            octave.expected,
            octave.production.value(),
            octave.saturation.value()
        );
        assert_poisson_count(
            &format!("octave {}", octave.index),
            u64::try_from(drawn).unwrap(),
            octave.expected,
            ALPHA,
        );
        // The share of the octave's craters above 100 km, by the body's distribution.
        let hundred = Metres::new(100e3);
        let share = if high.value() <= hundred.value() {
            0.0
        } else if low.value() >= hundred.value() {
            1.0
        } else {
            (cumulative_density(params, hundred).value() - cumulative_density(params, high).value())
                / (cumulative_density(params, low).value()
                    - cumulative_density(params, high).value())
        };
        above_100 += share * octave.expected;
    }
    let drawn_100 = state
        .craters
        .iter()
        .filter(|c| c.diameter.value() >= 100e3)
        .count();
    eprintln!("above 100 km: {drawn_100} drawn, {above_100:.1} expected (Trask's 299.7)");
    assert!((above_100 - 299.7).abs() < 15.0, "{above_100}");
    assert_poisson_count(
        "above 100 km",
        u64::try_from(drawn_100).unwrap(),
        above_100,
        ALPHA,
    );
}

/// On the reference Mars, whose wet epoch ended 3.6 Gyr ago on a 3.9 Gyr surface, only the craters
/// older than the epoch's end are smoothed in before the erosion, and the younger after it; every
/// crater is listed from the first (Design note 5).
#[test]
fn the_ages_split_about_the_wet_epoch_on_the_reference_mars() {
    let world = World::new(reference::mars_like());
    let pass = world.pass(reference::SEED);
    let (before, after) = cratered(&world);
    assert_eq!(before.craters, after.craters);
    let end = world.inputs.wet_epoch().unwrap().end.value();
    let (older, younger): (Vec<_>, Vec<_>) =
        before.craters.iter().partition(|c| c.age.value() > end);
    eprintln!(
        "Mars: {} craters older than the epoch's end, {} younger",
        older.len(),
        younger.len()
    );
    assert!(!older.is_empty() && !younger.is_empty());
    // The state before the erosion holds exactly the older craters' relief, and after it all.
    let figure = world.inputs.figure();
    let rims = CoarseRims::for_body(*world.inputs.craters(), figure, pass.level());
    let relief_of = |craters: &[&CoarseCrater], n: usize| {
        let p = figure.point(world.grid.centre(cell_of(n)));
        let mut z = 0.0;
        for c in craters {
            if !c.reach.contains(cell_of(n)) {
                continue;
            }
            let centre = figure.point(c.centre);
            let gap = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
            let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
            z += kept_relief(c)
                * rims
                    .coarse_profile(c.diameter)
                    .at(chord / (0.5 * c.diameter.value()))
                    .height_m;
        }
        z
    };
    let all: Vec<&CoarseCrater> = before.craters.iter().collect();
    for n in (0..before.cells.len()).step_by(37) {
        let early = relief_of(&older, n);
        let late = relief_of(&all, n);
        assert!(
            (before.cells[n].elevation.value() - early).abs() < 1e-6,
            "cell {n}"
        );
        assert!(
            (after.cells[n].elevation.value() - late).abs() < 1e-6,
            "cell {n}"
        );
    }
    // A world wet now, or without an epoch, waits for nothing.
    for inputs in [reference::earth_like(), reference::moon_like()] {
        let w = World::new(inputs);
        assert!(!waits(&w.pass(reference::SEED), Gigayears::new(0.0)));
    }
}

/// The list is sorted by (centre cell, diameter) with no key twice, and each crater's reach is
/// complete: every cell with a corner, an edge's midpoint or its centre within the chord (1 + a) R
/// of the crater's centre is in it, with no resolution code (R09.T7.c's contract).
#[test]
fn the_list_is_sorted_and_each_reach_complete() {
    let world = World::new(reference::moon_like());
    let (state, _) = cratered(&world);
    let field: CoarseField = quantise(&world.inputs, state).unwrap();
    let craters = field.craters();
    assert_ne!(craters, []);
    let level = field.header().level().get();
    let key = |c: &CoarseCrater| {
        (
            cell_index(PatchKey::containing(level, c.centre).unwrap()),
            c.diameter.value(),
        )
    };
    for pair in craters.windows(2) {
        let (a, b) = (key(&pair[0]), key(&pair[1]));
        assert!(a.0 < b.0 || (a.0 == b.0 && a.1 < b.1), "{a:?} {b:?}");
    }
    let figure = world.inputs.figure();
    let samples = [
        (0, 0),
        (64, 0),
        (0, 64),
        (64, 64),
        (32, 0),
        (0, 32),
        (64, 32),
        (32, 64),
        (32, 32),
    ];
    let mut reaches = 0_usize;
    for crater in craters {
        let shape = crater_shape(field.header().craters(), crater.diameter);
        let reach = shape.reach().value();
        let centre = figure.point(crater.centre);
        assert!(
            crater
                .reach
                .ranges()
                .iter()
                .all(|r| r.code() == ResolutionCode::NONE)
        );
        for (n, cell) in world.grid.cells().iter().enumerate() {
            let within = samples.iter().any(|&(x, y)| {
                let p = figure.point(cell.vertex_dir(x, y));
                let gap = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
                gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2] < reach * reach
            });
            if within {
                assert!(
                    crater.reach.contains(cell_of(n)),
                    "cell {n} of a crater's reach"
                );
            }
        }
        reaches += crater.reach.cells().count();
    }
    #[expect(clippy::cast_precision_loss, reason = "counts of cells")]
    let mean = reaches as f64 / f64::from(world.grid.len());
    eprintln!("the Moon's reaches: {mean:.2} entries a cell on average");
    assert!(mean < 32.0);
}

/// Before degradation, the reference Moon's coarse craters' rim heights are Pike 1980b's
/// 0.236 D^0.399 km to 1% (USGS Prof. Paper 1046-C, eq. 4, for complex craters); the Pike test is on
/// the listed craters' fresh shapes, since the coarse elevation holds them filleted over the coarse
/// band. Their degradation bytes are 0: T7.b's relief factor keeps at least 99.95% of each.
#[test]
fn the_moon_s_coarse_rims_are_pike_s_and_near_fresh() {
    let world = World::new(reference::moon_like());
    let (state, _) = cratered(&world);
    for crater in &state.craters {
        let shape = crater_shape(world.inputs.craters(), crater.diameter);
        let d_km = crater.diameter.value() / 1e3;
        let pike = 0.236 * math::powf(d_km, 0.399) * 1e3;
        let rim = shape.rim_height().value();
        assert!(
            (rim / pike - 1.0).abs() < 0.01,
            "{d_km} km: {rim} m against {pike} m"
        );
        assert_eq!(crater.morphology, shape.morphology());
        assert_eq!(
            crater.degradation,
            0,
            "{d_km} km at {} Gyr",
            crater.age.value()
        );
    }
}

/// The coarse elevation holds each crater as its kept relief times its coarse profile at the chord
/// from its centre, sampled at each cell's centre (ruling B of R09.T7.c), so that the client's
/// sharpening refines that one profile; and the interim figures follow the elevation.
#[test]
fn the_elevation_holds_each_crater_s_coarse_profile() {
    let world = World::new(reference::ceres_like());
    let pass = world.pass(reference::SEED);
    let (state, _) = cratered(&world);
    assert_ne!(
        state.craters,
        [] as [hyperion_surface::field::CoarseCrater; 0]
    );
    let figure = world.inputs.figure();
    let rims = CoarseRims::for_body(*world.inputs.craters(), figure, pass.level());
    let mut expected = vec![0.0; state.cells.len()];
    for c in &state.craters {
        let profile = rims.coarse_profile(c.diameter);
        let centre = figure.point(c.centre);
        for cell in c.reach.cells() {
            let p = figure.point(world.grid.centre(cell));
            let gap = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
            let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
            expected[slot(cell)] +=
                kept_relief(c) * profile.at(chord / (0.5 * c.diameter.value())).height_m;
        }
    }
    for (cell, &z) in state.cells.iter().zip(&expected) {
        hyperion_testkit::float::assert_same_bits(cell.elevation.value(), z);
        hyperion_testkit::float::assert_same_bits(
            cell.water_surface.value(),
            cell.elevation.value(),
        );
    }
    let lowest = expected.iter().copied().fold(f64::INFINITY, f64::min);
    hyperion_testkit::float::assert_same_bits(state.sea_level.value(), lowest);
}

#[test]
fn the_same_seed_gives_the_same_craters_and_another_seed_others() {
    let world = World::new(reference::moon_like());
    let pass = world.pass(reference::SEED);
    let mut first = Working::initial(&pass);
    run(&pass, &mut first);
    let mut again = Working::initial(&pass);
    run(&pass, &mut again);
    assert_eq!(first, again);
    let other_pass = world.pass(SurfaceSeed::new(0x0dd5));
    let mut other = Working::initial(&other_pass);
    run(&other_pass, &mut other);
    assert_ne!(first.craters, other.craters);
}

/// Measurements for the record, run by hand: each reference world's coarse craters, their counts
/// per octave against min(production, saturation), the largest mean reach count a cell and the
/// largest degradation byte.
#[test]
#[ignore = "measurement: the reference worlds' coarse craters, printed"]
fn measure_the_reference_worlds_coarse_craters() {
    for (name, inputs) in [
        ("Earth", reference::earth_like()),
        ("Mars", reference::mars_like()),
        ("Moon", reference::moon_like()),
        ("Ceres", reference::ceres_like()),
    ] {
        let world = World::new(inputs);
        let pass = world.pass(reference::SEED);
        let (state, _) = cratered(&world);
        let reaches: usize = state.craters.iter().map(|c| c.reach.cells().count()).sum();
        #[expect(clippy::cast_precision_loss, reason = "counts of cells")]
        let mean = reaches as f64 / f64::from(world.grid.len());
        let worst = state
            .craters
            .iter()
            .map(|c| c.degradation)
            .max()
            .unwrap_or(0);
        let counts: Vec<String> = octaves(&pass)
            .iter()
            .map(|o| {
                let drawn = state
                    .craters
                    .iter()
                    .filter(|c| {
                        c.diameter.value() >= o.diameters.lower().value()
                            && c.diameter.value() < o.diameters.upper().value()
                    })
                    .count();
                format!("{drawn}/{:.1}", o.expected)
            })
            .collect();
        eprintln!(
            "{name}: D_b {:.1} km, {} craters, octaves drawn/expected [{}], mean reach {mean:.2} a \
             cell, largest degradation byte {worst}",
            boundary_diameter(pass.level(), world.inputs.radius()).value() / 1e3,
            state.craters.len(),
            counts.join(", ")
        );
    }
}

/// The body's area the counts take is its spheroid's closed form, the cells' true areas summed
/// (`synth::craters::cell_area`) to 10⁻⁹, on the flattest reference world and on a sphere.
#[test]
fn the_body_s_area_is_its_spheroid_s() {
    for inputs in [reference::ceres_like(), reference::moon_like()] {
        let world = World::new(inputs);
        let figure = world.inputs.figure();
        let summed: f64 = world
            .grid
            .cells()
            .iter()
            .map(|&cell| hyperion_surface::synth::craters::cell_area(cell, &figure).value())
            .sum();
        let closed = spheroid_area_m2(&figure);
        assert!(
            (closed / summed - 1.0).abs() < 1e-9,
            "{closed} against {summed} m²"
        );
    }
    let sphere = hyperion_surface::spheroid::Spheroid::sphere(1.0e6);
    hyperion_testkit::float::assert_same_bits(
        spheroid_area_m2(&sphere),
        4.0 * core::f64::consts::PI * 1e12,
    );
}

/// The degradation byte is the nearest integer to 255 × (1 − g), g T7.b's relief factor of the rim
/// radius and the age's diffusion length, held to 255: a mid-range byte for a kilometre crater
/// 3 Gyr old (g about 0.48), none for a fresh one, and 255 where the relief is all but gone.
#[test]
fn the_degradation_byte_rounds_the_lost_relief() {
    let (d, age) = (Metres::new(1_000.0), Gigayears::new(3.0));
    let g = relief_factor(d * 0.5, diffusion_length(age));
    assert!((0.4..0.6).contains(&g), "{g}");
    let byte = degradation(d, age);
    assert!(
        (f64::from(byte) - 255.0 * (1.0 - g)).abs() <= 0.5,
        "{byte} for g {g}"
    );
    assert!((100..160).contains(&byte), "{byte}");
    assert_eq!(degradation(d, Gigayears::ZERO), 0);
    assert_eq!(degradation(Metres::new(2.0), Gigayears::new(4.5)), 255);
}

/// The cost probes, timed by hand from outside the test binary (the sim reads no clock): each runs
/// one stage on one reference world, so that the differences between them are each step's cost.
mod cost {
    use super::*;

    /// The plates, the relief and the coarse craters on the reference Earth, through `through`
    /// steps (1 to 3).
    fn earth_steps(through: u8) {
        let world = World::new(reference::earth_like());
        let pass = world.pass(reference::SEED);
        let mut working = Working::initial(&pass);
        crate::planetary::surface::steps::plates::run(&pass, &mut working);
        if through >= 2 {
            crate::planetary::surface::steps::relief::run(&pass, &mut working);
        }
        if through >= 3 {
            run(&pass, &mut working);
        }
        assert_ne!(working.cells, []);
    }

    #[test]
    #[ignore = "cost: the plates on the reference Earth, timed by hand"]
    fn earth_plates() {
        earth_steps(1);
    }

    #[test]
    #[ignore = "cost: the plates and the relief on the reference Earth, timed by hand"]
    fn earth_relief() {
        earth_steps(2);
    }

    #[test]
    #[ignore = "cost: the plates, the relief and the craters on the reference Earth, timed by hand"]
    fn earth_craters() {
        earth_steps(3);
    }

    /// The whole pass on `inputs`.
    fn pass_on(inputs: &CoarseInputs) {
        let field = crate::planetary::surface::coarse_pass(reference::SEED, inputs);
        assert_ne!(field.synthesis(), []);
    }

    #[test]
    #[ignore = "cost: the pass on the reference Earth, timed by hand"]
    fn pass_earth() {
        pass_on(&reference::earth_like());
    }

    #[test]
    #[ignore = "cost: the pass on the reference Mars, timed by hand"]
    fn pass_mars() {
        pass_on(&reference::mars_like());
    }

    #[test]
    #[ignore = "cost: the pass on the reference Moon, timed by hand"]
    fn pass_moon() {
        pass_on(&reference::moon_like());
    }

    #[test]
    #[ignore = "cost: the pass on the reference Ceres, timed by hand"]
    fn pass_ceres() {
        pass_on(&reference::ceres_like());
    }
}
