//! R09.T12.d's tests of the coarse craters (Design notes 5, 10, 12 and 13; R09.T7.c's and T8's
//! contract for T12.d).

use hyperion_surface::craters::{SOLAR_SYSTEM_AGE, crater_shape, cumulative_density};
use hyperion_surface::field::{CoarseField, FieldView, ResolutionCode, coarse_level};
use hyperion_surface::synth::craters::{VISIBILITY_COEFFICIENT, diffusion_length};
use hyperion_testkit::float::assert_same_bits;
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

/// The coarse octave of `octaves` whose diameters hold `diameter`.
fn octave_of(octaves: &[CoarseOctave], diameter: Metres) -> &CoarseOctave {
    let d = diameter.value();
    octaves
        .iter()
        .find(|o| d >= o.diameters.lower().value() && d < o.diameters.upper().value())
        .expect("every listed crater is of an octave")
}

/// (15 ÷ 32)², the share of its fresh relief a crater keeps at the visibility limit where κ is
/// negligible: (b ÷ (`K_v` + b))² (R09.T7.j).
const AT_THE_LIMIT: f64 = (15.0 / 32.0) * (15.0 / 32.0);

/// Before degradation, the reference Moon's coarse craters' rim heights are Pike 1980b's
/// 0.236 D^0.399 km to 1% (USGS Prof. Paper 1046-C, eq. 4, for complex craters); the Pike test is on
/// the listed craters' fresh shapes, since the coarse elevation holds them filleted over the coarse
/// band. Every coarse octave is saturated on its 4.4 Gyr surface, so R09.T7.j's closure wears its
/// craters uniformly in q from fresh to the visibility limit: every byte is at most 199, the
/// nearest integer to 255 (1 − (15 ÷ 32)²) = 198.97, and the listed craters' mean g² lies in
/// 0.21–0.32 (E[g²] 0.2638, g²'s standard deviation 0.236: ±4 standard errors over about 320
/// craters; `decision-r09-t7ij-refresh.md` Q1.3).
#[test]
fn the_moon_s_coarse_rims_are_pike_s_and_worn_to_equilibrium() {
    let world = World::new(reference::moon_like());
    let pass = world.pass(reference::SEED);
    let (state, _) = cratered(&world);
    let octaves = octaves(&pass);
    for octave in &octaves {
        assert!(
            octave.production.value() > octave.saturation.value(),
            "octave {} is not saturated",
            octave.index
        );
    }
    let limit = (255.0 * (1.0 - AT_THE_LIMIT)).round();
    assert_same_bits(limit, 199.0);
    let mut square = 0.0;
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
        assert!(
            f64::from(crater.degradation) <= limit,
            "{d_km} km at {} Gyr: byte {}",
            crater.age.value(),
            crater.degradation
        );
        let wear = CraterWear::of_age(crater.age, &octave_of(&octaves, crater.diameter).wear);
        assert!(wear.share() <= 1.0 + 1e-12, "{d_km} km: q {}", wear.share());
        let g = kept_relief(crater);
        square += g * g;
    }
    #[expect(clippy::cast_precision_loss, reason = "a count of craters")]
    let mean = square / state.craters.len() as f64;
    eprintln!(
        "the reference Moon's {} coarse craters: mean g² {mean:.4}, largest byte {:?}",
        state.craters.len(),
        state.craters.iter().map(|c| c.degradation).max()
    );
    assert!((0.21..=0.32).contains(&mean), "{mean}");
}

/// On the reference Mars no coarse octave is saturated, so its craters' share q runs to their
/// octave's production over its saturation, P ÷ S, and no byte exceeds the nearest integer to
/// 255 (1 − g) at q = P ÷ S with κ at the octave's oldest age (about 105 in the first coarse
/// octave, P ÷ S near 0.27: R09.T7.j).
#[test]
fn the_reference_mars_s_coarse_craters_are_worn_no_further_than_their_octave_s_production() {
    let world = World::new(reference::mars_like());
    let pass = world.pass(reference::SEED);
    let (state, _) = cratered(&world);
    let octaves = octaves(&pass);
    let mut checked = 0;
    for octave in &octaves {
        let ratio = octave.production.value() / octave.saturation.value();
        let reach = num::min(ratio, 1.0);
        let oldest = diffusion_length(octave.wear.window());
        let mut largest = 0;
        for crater in state
            .craters
            .iter()
            .filter(|c| octave_of(&octaves, c.diameter).index == octave.index)
        {
            let g = relief_factor(crater.diameter * 0.5, CraterWear::new(oldest, reach));
            let bound = (255.0 * (1.0 - g)).round();
            assert!(
                f64::from(crater.degradation) <= bound,
                "octave {}: {} km byte {} over {bound}",
                octave.index,
                crater.diameter.value() / 1e3,
                crater.degradation
            );
            largest = largest.max(crater.degradation);
            checked += 1;
        }
        let lower = relief_factor(
            octave.diameters.lower() * 0.5,
            CraterWear::new(oldest, reach),
        );
        eprintln!(
            "Mars octave {}: P ÷ S {ratio:.3}, bound {:.0} at its lower diameter, largest byte \
             {largest}",
            octave.index,
            (255.0 * (1.0 - lower)).round()
        );
    }
    assert!(checked > 0);
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

/// The degradation byte is the nearest integer to 255 × (1 − g), g the relief factor of the rim
/// radius and the crater's wear in its octave (R09.T7.j), held to 255: a mid-range byte for a
/// kilometre crater 3 Gyr old on maria whose octave's production is 0.07 of its saturation, where
/// κ binds (g about 0.48); 199 for a 50 km crater at the end of a saturated octave's window, the
/// visibility limit; about 151 halfway through it (q = ½, g 0.407); none for a fresh one; and 255
/// where the relief is all but gone.
#[test]
fn the_degradation_byte_rounds_the_lost_relief() {
    let saturation = PerSquareKilometre::new(1e-2);
    let maria = OctaveWear::new(
        Gigayears::new(3.0),
        SOLAR_SYSTEM_AGE,
        saturation * 0.07,
        saturation,
    );
    let (d, age) = (Metres::new(1_000.0), Gigayears::new(3.0));
    let g = relief_factor(d * 0.5, CraterWear::of_age(age, &maria));
    assert_same_bits(
        g,
        relief_factor(d * 0.5, CraterWear::new(diffusion_length(age), 0.0)),
    );
    assert!((0.4..0.6).contains(&g), "{g}");
    let byte = degradation(d, age, &maria);
    assert!(
        (f64::from(byte) - 255.0 * (1.0 - g)).abs() <= 0.5,
        "{byte} for g {g}"
    );
    assert!((100..160).contains(&byte), "{byte}");
    assert_eq!(degradation(d, Gigayears::ZERO, &maria), 0);
    assert_eq!(
        degradation(Metres::new(2.0), Gigayears::new(3.0), &maria),
        255
    );
    // A saturated octave of 50 km craters on a 4.4 Gyr surface: its window's oldest crater at the
    // visibility limit, and one at half its window's density halfway there.
    let capped = OctaveWear::new(
        Gigayears::new(4.4),
        SOLAR_SYSTEM_AGE,
        saturation * 3.0,
        saturation,
    );
    let d = Metres::new(50e3);
    let oldest = chronology_age(capped.window_density(), SOLAR_SYSTEM_AGE);
    assert_eq!(degradation(d, oldest, &capped), 199);
    let middle = chronology_age(capped.window_density() * 0.5, SOLAR_SYSTEM_AGE);
    let g = relief_factor(d * 0.5, CraterWear::of_age(middle, &capped));
    let halfway = 0.15 / (0.5 * VISIBILITY_COEFFICIENT + 0.15);
    assert!((g - halfway * halfway).abs() < 1e-6, "{g}");
    assert_eq!(degradation(d, middle, &capped), 151);
    assert_eq!(degradation(d, Gigayears::ZERO, &capped), 0);
}

/// The unit direction `psi` radians from the unit direction `centre`, towards the azimuth `azimuth`
/// radians in a tangent basis of `centre`.
fn around(centre: [f64; 3], psi: f64, azimuth: f64) -> [f64; 3] {
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let helper = if centre[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = hyperion_surface::cube::unit_dir(cross(centre, helper));
    let e2 = cross(centre, e1);
    let (sin_psi, cos_psi) = math::sin_cos(psi);
    let (sin_a, cos_a) = math::sin_cos(azimuth);
    hyperion_surface::cube::unit_dir(
        [0, 1, 2].map(|k| cos_psi * centre[k] + sin_psi * (cos_a * e1[k] + sin_a * e2[k])),
    )
}

/// One fresh coarse crater of `diameter` centred on the unit direction `centre` of `world`, its
/// reach as the step draws it ([`reach_cells`]).
fn lone_crater(world: &World, centre: [f64; 3], diameter: Metres) -> CoarseCrater {
    let figure = world.inputs.figure();
    let shape = crater_shape(world.inputs.craters(), diameter);
    let home = cell_index(PatchKey::containing(world.grid.level(), centre).unwrap());
    let smallest = num::min(figure.equatorial_radius_m, figure.polar_radius_m);
    let reach_angle = 2.0 * math::asin(num::min(0.5 * shape.reach().value() / smallest, 1.0));
    CoarseCrater {
        centre,
        diameter,
        morphology: shape.morphology(),
        age: Gigayears::ZERO,
        degradation: 0,
        reach: Cover::from_cells(reach_cells(&world.grid, centre, home, reach_angle)),
    }
}

/// The placements of the lone-crater tests: 4 × 4 offsets within each of two coarse cells of the
/// reference Moon's level 6, one at the middle of face 0 and one between face 2's middle and its
/// edges, where the cells are sheared, both at least 18 cells from a face edge.
fn placements(level: u8) -> Vec<[f64; 3]> {
    let side = f64::from(1_u32 << level);
    let mut out = Vec::new();
    for (face, i, j) in [
        (hyperion_surface::cube::Face::PosX, 32.0, 32.0),
        (hyperion_surface::cube::Face::PosZ, 20.0, 41.0),
    ] {
        for a in 0..4 {
            for b in 0..4 {
                let s = (i + (f64::from(a) + 0.5) / 4.0) / side;
                let t = (j + (f64::from(b) + 0.5) / 4.0) / side;
                out.push(hyperion_surface::cube::unit_dir(
                    hyperion_surface::cube::face_uv_to_xyz(hyperion_surface::cube::FaceUv {
                        face,
                        u: hyperion_surface::cube::st_to_uv(s),
                        v: hyperion_surface::cube::st_to_uv(t),
                    }),
                ));
            }
        }
    }
    out
}

/// One placement of a lone crater, read through R09.T4's interpolant: its centre less its
/// area-weighted mean over 1.2–2 rim radii, metres, and the RMS over its reach's disc of the
/// reconstruction less the analytic coarse profile, metres.
#[derive(Debug, Clone, Copy)]
struct LonePlacement {
    centre_less_annulus_m: f64,
    residual_rms_m: f64,
}

/// A fresh coarse crater of `diameter` smoothed alone (by [`each_lift`]) into a flat state of
/// `world` at each of [`placements`], read through R09.T4's interpolant over the cells
/// ([`interpolate`](hyperion_surface::synth::interp::interpolate), through R09.T12.e's
/// `quadrature::Values`, a `CellValues` of them).
fn lone_placements(world: &World, diameter: Metres) -> Vec<LonePlacement> {
    use crate::planetary::surface::steps::relief::quadrature::Values;
    let pass = world.pass(reference::SEED);
    let figure = world.inputs.figure();
    let rims = CoarseRims::for_body(*world.inputs.craters(), figure, pass.level());
    let profile = rims.coarse_profile(diameter);
    let rim = 0.5 * diameter.value();
    let reach = 1.0 + profile.shape().exterior_width();
    let radius = figure.volumetric_radius_m();
    // The angle at which a chord of x rim radii is seen from the centre, on the volumetric sphere.
    let angle = |x: f64| 2.0 * math::asin(0.5 * x * rim / radius);
    let azimuth = |k: u32, n: u32| core::f64::consts::TAU * (f64::from(k) + 0.5) / f64::from(n);
    let mut out = Vec::new();
    for centre in placements(world.grid.level()) {
        let crater = lone_crater(world, centre, diameter);
        let mut z = vec![0.0; world.grid.cells().len()];
        each_lift(&world.grid, &figure, &rims, &crater, |cell, lift| {
            z[slot(cell)] += lift;
        });
        let field = Values::new(&world.grid, &z);
        let middle = field.interpolated(centre);
        // The annulus's area mean: 16 radii at the midpoints of 1.2–2 R, each weighted by x, and
        // 48 azimuths.
        let (mut sum, mut weight) = (0.0, 0.0);
        for i in 0..16 {
            let x = 1.2 + 0.8 * (f64::from(i) + 0.5) / 16.0;
            for k in 0..48 {
                sum += x * field.interpolated(around(centre, angle(x), azimuth(k, 48)));
                weight += x;
            }
        }
        // The residual over the reach's disc, against the profile at each point's own chord.
        let p0 = figure.point(centre);
        let (mut square, mut area) = (0.0, 0.0);
        for i in 0..32 {
            let x = reach * (f64::from(i) + 0.5) / 32.0;
            for k in 0..48 {
                let dir = around(centre, angle(x), azimuth(k, 48));
                let p = figure.point(dir);
                let gap = [p[0] - p0[0], p[1] - p0[1], p[2] - p0[2]];
                let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
                let analytic = profile.at(chord / rim).height_m;
                let r = field.interpolated(dir) - analytic;
                square += x * r * r;
                area += x;
            }
        }
        out.push(LonePlacement {
            centre_less_annulus_m: middle - sum / weight,
            residual_rms_m: (square / area).sqrt(),
        });
    }
    out
}

/// A 113 km lunar crater, smoothed alone into a flat state at the reference Moon's coarse level
/// and read through R09.T4's interpolant, is a depression: at each of 32 placements its centre
/// lies at least 1,000 m below its mean over 1.2–2 rim radii (R09.T7.i; main's planar estimates,
/// on the cells of the Moon's mean width reconstructed by the cubic B-spline, −2,008 m under the
/// fillet through the peak, −1,447 m under a Gaussian low-pass of s.d. `w_c` ÷ π, and −316 m as
/// built, the inverted profile).
#[test]
fn a_lone_113_km_lunar_crater_is_a_depression_in_the_coarse_field() {
    let world = World::new(reference::moon_like());
    let placed = lone_placements(&world, Metres::new(113e3));
    let mut mean = 0.0;
    for (n, p) in placed.iter().enumerate() {
        eprintln!(
            "placement {n}: centre less annulus {:.0} m, residual {:.0} m RMS",
            p.centre_less_annulus_m, p.residual_rms_m
        );
        mean += p.centre_less_annulus_m;
    }
    #[expect(clippy::cast_precision_loss, reason = "a count of placements")]
    let count = placed.len() as f64;
    eprintln!("113 km: mean centre less annulus {:.0} m", mean / count);
    for (n, p) in placed.iter().enumerate() {
        assert!(
            p.centre_less_annulus_m <= -1_000.0,
            "placement {n}: {:.0} m",
            p.centre_less_annulus_m
        );
    }
}

/// Whether `crater`'s centre's cell lies below the area mean of the cells whose centres lie at
/// chords of 1.2–2 rim radii from its centre, in the cell values `z` of `world`.
fn centre_below_annulus(world: &World, crater: &CoarseCrater, z: &[f64]) -> bool {
    let figure = world.inputs.figure();
    let rim = 0.5 * crater.diameter.value();
    let centre = figure.point(crater.centre);
    let home = cell_index(PatchKey::containing(world.grid.level(), crater.centre).unwrap());
    let (mut sum, mut area) = (0.0, 0.0);
    for cell in crater.reach.cells() {
        let p = figure.point(world.grid.centre(cell));
        let gap = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
        let x = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt() / rim;
        if (1.2..=2.0).contains(&x) {
            let w = world.grid.solid_angle_sr(cell);
            sum += w * z[slot(cell)];
            area += w;
        }
    }
    assert!(area > 0.0, "a crater's annulus holds a cell centre");
    z[slot(home)] < sum / area
}

/// ∫ F² 2x dx over a coarse crater's reach, square metres, of `profile`, by four-point
/// Gauss–Legendre on 64 panels of each of its pieces, cut at its centre, the cone's base, the
/// floor's edge, the fillet's ends, the rim and the reach: exact to rounding but for the rule's
/// own error on a degree-9 integrand, under 10⁻¹³.
pub(crate) fn analytic_square(profile: &hyperion_surface::synth::craters::CraterProfile) -> f64 {
    use crate::planetary::surface::steps::relief::quadrature::gauss_legendre;
    let shape = profile.shape();
    let rim = 0.5 * shape.diameter().value();
    let w = profile.fillet_half_width().value() / rim;
    let reach = 1.0 + shape.exterior_width();
    let mut cuts = [
        0.0,
        shape.peak_radius().value() / rim,
        shape.floor_radius().value() / rim,
        1.0 - w,
        1.0,
        1.0 + w,
        reach,
    ];
    cuts.sort_by(f64::total_cmp);
    let nodes = gauss_legendre(4);
    let mut total = 0.0;
    for pair in cuts.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        if hi <= lo {
            continue;
        }
        let panel = (hi - lo) / 64.0;
        for k in 0..64 {
            let mid = lo + panel * (f64::from(k) + 0.5);
            for &(node, weight) in &nodes {
                let x = mid + 0.5 * panel * node;
                let h = profile.at(x).height_m;
                total += 0.5 * panel * weight * h * h * 2.0 * x;
            }
        }
    }
    total
}

/// R09.T7.i's figures on the reference Moon, printed for the plan's record, run by hand before
/// and after the fillet through the peak: the share of the first coarse octave's craters whose
/// centre's cell lies below the mean of their 1.2–2 R cells (the craters alone after the step,
/// each alone, and the field after R09.T12.e's entry); the coarse craters' analytic rendered share
/// Σ g² ∫F(`w₂`)² dA ÷ the body's area at the band limit's 1 m fillet; and Table B's lone craters
/// through R09.T4's interpolant, with the reconstruction's residual.
#[test]
#[ignore = "measurement: R09.T7.i's figures on the reference Moon, printed"]
fn measure_r09_t7i_first_octave_depressions() {
    let world = World::new(reference::moon_like());
    let pass = world.pass(reference::SEED);
    let first = octaves(&pass)[0].diameters;
    let in_first = |c: &&CoarseCrater| {
        c.diameter.value() >= first.lower().value() && c.diameter.value() < first.upper().value()
    };
    let (alone, _) = cratered(&world);
    let mut entered = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut entered);
    crate::planetary::surface::steps::relief::run(&pass, &mut entered);
    run(&pass, &mut entered);
    crate::planetary::surface::steps::relief::run_after_craters(&pass, &mut entered);
    for (label, state) in [
        ("the coarse craters alone", &alone),
        ("after R09.T12.e's entry", &entered),
    ] {
        let z: Vec<f64> = state.cells.iter().map(|c| c.elevation.value()).collect();
        let listed: Vec<&CoarseCrater> = state.craters.iter().filter(in_first).collect();
        let below = listed
            .iter()
            .filter(|c| centre_below_annulus(&world, c, &z))
            .count();
        #[expect(clippy::cast_precision_loss, reason = "counts of craters")]
        let share = below as f64 / listed.len() as f64;
        eprintln!(
            "{label}: {below} of the first octave's {} craters ({:.0}–{:.0} km) lie below their \
             1.2–2 R cells: {:.1}%",
            listed.len(),
            first.lower().value() / 1e3,
            first.upper().value() / 1e3,
            100.0 * share
        );
    }
    // Each first-octave crater alone in a flat state.
    let figure = world.inputs.figure();
    let rims = CoarseRims::for_body(*world.inputs.craters(), figure, pass.level());
    let listed: Vec<&CoarseCrater> = alone.craters.iter().filter(in_first).collect();
    let mut below = 0;
    for crater in &listed {
        let mut z = vec![0.0; world.grid.cells().len()];
        each_lift(&world.grid, &figure, &rims, crater, |cell, lift| {
            z[slot(cell)] += lift;
        });
        if centre_below_annulus(&world, crater, &z) {
            below += 1;
        }
    }
    eprintln!(
        "each alone: {below} of {} lie below their 1.2–2 R cells",
        listed.len()
    );
    // The analytic rendered share, at T12.d's bytes and at T12.e's.
    let area_m2 = spheroid_area_m2(&figure);
    for (label, list) in [
        ("T12.d's bytes", &alone.craters),
        ("T12.e's bytes", &entered.craters),
    ] {
        let mut sum = 0.0;
        for crater in list {
            let g = kept_relief(crater);
            let shape = crater_shape(world.inputs.craters(), crater.diameter);
            let ground =
                hyperion_surface::synth::craters::CraterProfile::filleted(shape, Metres::new(1.0));
            let rim = 0.5 * crater.diameter.value();
            sum += g * g * analytic_square(&ground) * core::f64::consts::PI * rim * rim;
        }
        eprintln!(
            "the analytic rendered share at {label}: Σ g² ∫F(w₂)² dA ÷ A = {:.10e} m² ({:.4} m)",
            sum / area_m2,
            (sum / area_m2).sqrt()
        );
    }
    // Table B.
    for d_km in [95.0, 113.0, 150.0, 185.0, 228.0] {
        let placed = lone_placements(&world, Metres::new(d_km * 1e3));
        #[expect(clippy::cast_precision_loss, reason = "a count of placements")]
        let count = placed.len() as f64;
        let mean = placed.iter().map(|p| p.centre_less_annulus_m).sum::<f64>() / count;
        let worst = placed
            .iter()
            .map(|p| p.centre_less_annulus_m)
            .fold(f64::NEG_INFINITY, f64::max);
        let residual = (placed
            .iter()
            .map(|p| p.residual_rms_m * p.residual_rms_m)
            .sum::<f64>()
            / count)
            .sqrt();
        eprintln!(
            "lone {d_km} km: centre less 1.2–2 R annulus {mean:.0} m (worst {worst:.0} m) over \
             {count} placements; reconstruction residual {residual:.0} m RMS over the reach"
        );
    }
}

/// R09.T7.j's figures on the reference worlds, printed for the plan's record, run by hand: each
/// coarse octave's production over its saturation, its craters' mean g² by the closed form at its
/// eight diameter nodes ([`OctaveWear::mean_square_relief`]) and the listed craters' (their
/// bytes'), with the largest byte; and each small octave's (`CraterOctaves::for_body`).
#[test]
#[ignore = "measurement: R09.T7.j's wear on the reference worlds, printed"]
fn measure_r09_t7j_wear() {
    use crate::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};
    use hyperion_surface::craters::DiameterOctave;
    use hyperion_surface::synth::craters::CraterOctaves;
    let mean_square = |params: &CraterParams, diameters: DiameterOctave, wear: &OctaveWear| {
        let mut mean = 0.0;
        for (&node, &weight) in GL8_NODES.iter().zip(&GL8_WEIGHTS) {
            let share = UnitUniform::new(f64::midpoint(1.0, node)).unwrap();
            let d = diameter_in_octave(params, diameters, share);
            mean += 0.5 * weight * wear.mean_square_relief(d * 0.5);
        }
        mean
    };
    for (name, inputs) in [
        ("Earth", reference::earth_like()),
        ("Mars", reference::mars_like()),
        ("Moon", reference::moon_like()),
        ("Ceres", reference::ceres_like()),
    ] {
        let world = World::new(inputs);
        let pass = world.pass(reference::SEED);
        let (state, _) = cratered(&world);
        let params = world.inputs.craters();
        let list = octaves(&pass);
        eprintln!("== {name}: {} coarse craters", state.craters.len());
        let (mut all_square, mut all_largest) = (0.0, 0);
        for octave in &list {
            let listed: Vec<&CoarseCrater> = state
                .craters
                .iter()
                .filter(|c| octave_of(&list, c.diameter).index == octave.index)
                .collect();
            let square: f64 = listed.iter().map(|c| kept_relief(c) * kept_relief(c)).sum();
            all_square += square;
            let largest = listed.iter().map(|c| c.degradation).max().unwrap_or(0);
            all_largest = all_largest.max(largest);
            #[expect(clippy::cast_precision_loss, reason = "a count of craters")]
            let mean = square / num::max(listed.len() as f64, 1.0);
            eprintln!(
                "  coarse {} ({:.0}–{:.0} km): P ÷ S {:.3}, capped {}, E[g²] {:.4}; {} listed, \
                 mean g² {mean:.4}, largest byte {largest}",
                octave.index,
                octave.diameters.lower().value() / 1e3,
                octave.diameters.upper().value() / 1e3,
                octave.production.value() / octave.saturation.value(),
                octave.production.value() > octave.saturation.value(),
                mean_square(params, octave.diameters, &octave.wear),
                listed.len()
            );
        }
        #[expect(clippy::cast_precision_loss, reason = "a count of craters")]
        let mean = all_square / num::max(state.craters.len() as f64, 1.0);
        eprintln!("  all listed: mean g² {mean:.4}, largest byte {all_largest}");
        let d_b = boundary_diameter(pass.level(), world.inputs.radius());
        let small = CraterOctaves::for_body(
            *params,
            world.inputs.figure(),
            d_b,
            world.inputs.surface_age(),
        );
        for octave in small.octaves() {
            eprintln!(
                "  small {} ({:.3}–{:.3} km): P ÷ S {:.3}, capped {}, window {:.3} Gyr, E[g²] \
                 {:.4}, variance {:.4e} m²",
                octave.index(),
                octave.diameters().lower().value() / 1e3,
                octave.diameters().upper().value() / 1e3,
                octave.production().value() / octave.saturation().value(),
                octave.is_saturated(),
                octave.window().value(),
                mean_square(params, octave.diameters(), octave.wear()),
                octave.variance().value()
            );
        }
    }
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

    #[test]
    #[ignore = "cost: the pass on the reference Europa, timed by hand"]
    fn pass_europa() {
        pass_on(&reference::europa_like());
    }

    #[test]
    #[ignore = "cost: the pass on the reference locked Earth, timed by hand"]
    fn pass_locked_earth() {
        pass_on(&reference::locked_earth_like());
    }
}
