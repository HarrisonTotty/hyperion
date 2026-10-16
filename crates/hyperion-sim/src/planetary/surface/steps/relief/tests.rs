//! R09.T12's tests of the coarse elevation (Design note 7; `decision-r09-t5.md` item 5 and
//! `decision-r09-t12-coarse.md`), written at the step's level, before R09.T12.e's affine scale.

use std::sync::OnceLock;

use hyperion_surface::field::{
    BoundaryKind, CoarseField, Crust, FieldView, PaletteEntry, coarse_level,
};
use hyperion_surface::synth::relief::ReliefStyle;
use hyperion_testkit::float::assert_same_bits;

use super::seafloor::{
    EARTH_OCEANIC_SHARE, EARTH_OLDEST_FLOOR, basement_hill_relief, draped_hill_relief, gdh1_depth,
    oldest_floor_age, pelagic_drape, ponded_sediment, sediment_load,
};
use super::*;
use crate::planetary::hooks::SurfaceSeed;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::planetary::surface::{quantise, reference};
use crate::units::consts::{SECONDS_PER_JULIAN_YEAR, SECONDS_PER_MEGAYEAR};
use crate::units::{Gigayears, MetresPerSecond, Seconds, WattsPerSquareMetre};

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

/// A world's state after the plates and this step, and that state quantised; and, on a mobile
/// lid, the state after the populations and before the convergent landforms, with the arcs'
/// layout, or on a stagnant one the provinces' layout.
struct Relieved {
    world: World,
    relief: Working,
    field: CoarseField,
    populations: Option<Working>,
    arcs: Option<convergent::Arcs>,
    provinces: Option<provinces::Provinces>,
}

/// The plates and this step on `inputs` under the reference seed, its stages kept: the step's
/// [`run`] taken apart, which `the_staged_step_is_the_step` holds to it.
fn relieve(inputs: CoarseInputs) -> Relieved {
    let world = World::new(inputs);
    let pass = world.pass(reference::SEED);
    let mut relief = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut relief);
    let (populations, arcs, laid) = if relief.plates.is_empty() {
        lid_population(&pass, &mut relief);
        let laid = provinces::run(&pass, &mut relief);
        (None, None, laid)
    } else {
        let noise = noise::unit_field(
            &pass,
            noise::LANDFORM_INSTANCE,
            noise::LANDFORM_FIRST_LEVEL,
            pass.level().get(),
        );
        crust_populations(&pass, &mut relief, &noise);
        let populations = relief.clone();
        let arcs = convergent::run(&pass, &mut relief);
        (Some(populations), arcs, None)
    };
    interim_figures(&pass, &mut relief);
    let field = quantise(&world.inputs, relief.clone()).expect("the step's state quantises");
    Relieved {
        world,
        relief,
        field,
        populations,
        arcs,
        provinces: laid,
    }
}

/// The staged step of the tests' `relieve` is the step's [`run`], bit for bit, on a small mobile
/// lid and on a stagnant one.
#[test]
fn the_staged_step_is_the_step() {
    for inputs in [
        builder_of(&reference::earth_like())
            .figure(hyperion_surface::spheroid::Spheroid::sphere(2.0e6))
            .build()
            .unwrap(),
        reference::ceres_like(),
        reference::mars_like(),
    ] {
        let staged = relieve(inputs.clone());
        let world = World::new(inputs);
        let pass = world.pass(reference::SEED);
        let mut working = Working::initial(&pass);
        crate::planetary::surface::steps::plates::run(&pass, &mut working);
        run(&pass, &mut working);
        assert_eq!(staged.relief, working);
    }
}

/// The reference Earth, computed once for the tests that read it.
fn earth() -> &'static Relieved {
    static EARTH: OnceLock<Relieved> = OnceLock::new();
    EARTH.get_or_init(|| relieve(reference::earth_like()))
}

/// Myr as seconds.
fn myr(t: f64) -> Seconds {
    Seconds::new(t * SECONDS_PER_MEGAYEAR)
}

/// A half-rate of `full_mm_a` ÷ 2 mm a⁻¹, as a velocity.
fn half_of_full(full_mm_a: f64) -> MetresPerSecond {
    MetresPerSecond::new(0.5 * full_mm_a * 1e-3 / SECONDS_PER_JULIAN_YEAR)
}

#[test]
fn gdh1_s_basement_follows_its_two_branches_to_a_metre() {
    for (t, depth) in [(5.0, 3_416.2), (20.0, 4_232.7), (100.0, 5_497.6)] {
        let d = gdh1_depth(myr(t)).value();
        assert!((d - depth).abs() < 1.0, "{t} Myr: {d} m");
    }
    // The branches meet at 20 Myr within 0.41 m, and the floor is 9.5 m short of the asymptote
    // at 200 Myr.
    let shallow = 2_600.0 + 365.0 * 20.0_f64.sqrt();
    assert!((gdh1_depth(myr(20.0)).value() - shallow).abs() < 0.42);
    assert!((5_651.0 - gdh1_depth(myr(200.0)).value() - 9.5).abs() < 0.1);
    assert!((gdh1_depth(Seconds::ZERO).value() - 2_600.0).abs() < 1e-9);
}

#[test]
fn the_sediment_s_load_lifts_its_top_by_athy_s_column() {
    for (s, ratio) in [(100.0, 0.713), (2_400.0, 0.526)] {
        let l = sediment_load(Metres::new(s)).value() / s;
        assert!((l - ratio).abs() < 1e-3, "{s} m: {l}");
    }
    assert_same_bits(sediment_load(Metres::ZERO).value(), 0.0);
    // The series below cS = 10⁻⁶ (S = 1.96 mm) meets the closed form: the two sides differ by
    // the ratio's own slope, about 1.2 × 10⁻⁴ a metre, over the step.
    let (below, above) = (1.9e-3, 2.1e-3);
    let (lb, la) = (
        sediment_load(Metres::new(below)).value() / below,
        sediment_load(Metres::new(above)).value() / above,
    );
    assert!(
        (lb - la).abs() < 1.3e-4 * (above - below) + 1e-12,
        "{lb} {la}"
    );
}

/// Goff, Smith and Marks 2004's Table 2 (Oceanography 17(1), 24–37, p. 30), abyssal-hill RMS height
/// H by region and full spreading rate, as R09.T12's science check read it: northern MAR 235.8 m at
/// 28 mm a⁻¹; southern MAR (after Neumann and Forsyth 1995) 222 m at 35; PAR slow 140 m and
/// southern MAR 200.9 m at 40; PAR fast 90 m at 55; SEIR axial high 61.7 m, intermediate 97.4 m and
/// axial valley 169.8 m at 75; Pacific–Cocos 56.1 m at 110; Pacific–Nazca 58.7 m at 160
/// (`decision-r09-t12-coarse.md` item 5a). Untested: the fit's 207.3 m at 35 (−6.6%) and 116.0 m at
/// 55 (+28.9%). The fit is held within 30% of the mean of
/// the entries at each tested rate, the axial high alone at 75 mm a⁻¹ (the SEIR's other two entries
/// are other morphologies at that rate).
#[test]
fn h0_follows_goff_s_table_2_at_five_spreading_rates() {
    for (full, mean) in [
        (28.0, 235.8),
        (40.0, f64::midpoint(140.0, 200.9)),
        (75.0, 61.7),
        (110.0, 56.1),
        (160.0, 58.7),
    ] {
        let h0 = basement_hill_relief(half_of_full(full)).value();
        assert!(
            (h0 / mean - 1.0).abs() <= 0.3,
            "{full} mm a⁻¹: {h0} m against {mean} m"
        );
    }
    // The ruling's fit at 14 mm a⁻¹ half-rate, and the ultraslow term.
    assert!((basement_hill_relief(half_of_full(28.0)).value() - 222.8).abs() < 0.1);
    assert!((basement_hill_relief(half_of_full(10.0)).value() - 286.1).abs() < 0.1);
}

#[test]
fn the_pelagic_drape_is_468_m_at_81_myr_on_the_equator() {
    let drape = pelagic_drape(myr(81.0), Radians::ZERO).value();
    assert!((drape - 468.0).abs() < 1e-9, "{drape} m");
    // Held beyond 72°: the pole drapes as 72°.
    let polar = pelagic_drape(myr(81.0), Radians::new(core::f64::consts::FRAC_PI_2));
    let held = pelagic_drape(myr(81.0), Radians::new(72.0_f64.to_radians()));
    assert!((polar.value() - held.value()).abs() < 1e-9);
    // Goff's drape halves at most.
    let h0 = Metres::new(200.0);
    assert_same_bits(draped_hill_relief(h0, Metres::new(100.0)).value(), 150.0);
    assert_same_bits(draped_hill_relief(h0, Metres::new(1_000.0)).value(), 100.0);
}

#[test]
fn the_ponded_wedge_falls_off_from_its_margin_and_ends() {
    assert_same_bits(ponded_sediment(None).value(), 0.0);
    assert!((ponded_sediment(Some(Metres::ZERO)).value() - 2_400.0).abs() < 1e-9);
    let at = |km: f64| ponded_sediment(Some(Metres::new(km * 1e3))).value();
    assert!((at(200.0) / 2_400.0 - math::exp(-1.0)).abs() < 1e-12);
    assert!(at(600.0) > 0.0);
    assert_same_bits(at(600.001), 0.0);
}

#[test]
fn the_oldest_floor_is_earth_s_and_quadruples_when_the_heat_flow_halves() {
    let earth = reference::earth_like();
    let system = earth.craters().system_age();
    let f = earth.heat_flow();
    let t = oldest_floor_age(EARTH_OCEANIC_SHARE, f, system);
    assert!(
        (t.value() / SECONDS_PER_MEGAYEAR - 192.9).abs() < 1e-9,
        "{} Myr",
        t.value() / SECONDS_PER_MEGAYEAR
    );
    let half = WattsPerSquareMetre::new(0.5 * f.value());
    let slow = oldest_floor_age(EARTH_OCEANIC_SHARE, half, system);
    assert!((slow.value() / t.value() - 4.0).abs() < 1e-12);
    let still = oldest_floor_age(EARTH_OCEANIC_SHARE, WattsPerSquareMetre::ZERO, system);
    assert!(
        (still.value() - system.value() * crate::units::consts::SECONDS_PER_GIGAYEAR).abs() < 1.0
    );
    // A young system caps it.
    let young = oldest_floor_age(EARTH_OCEANIC_SHARE, half, Gigayears::new(0.5));
    assert!(young.value() < slow.value());
    // The realised share scales it.
    let wide = oldest_floor_age(2.0 * EARTH_OCEANIC_SHARE, f, system);
    assert!((wide.value() / t.value() - 2.0).abs() < 1e-12);
    assert!((EARTH_OLDEST_FLOOR.value() / SECONDS_PER_MEGAYEAR - 192.9).abs() < 1e-9);
}

/// The reference Earth's oceanic cells: (solid angle, kinematic key κ or `None`, `τ_ss`).
fn earth_ocean() -> Vec<(f64, Option<f64>, f64)> {
    let earth = earth();
    let omega = earth.world.grid.solid_angles_sr();
    earth
        .relief
        .cells
        .iter()
        .zip(omega)
        .filter_map(|(c, &w)| {
            let origin = c.seafloor_origin?;
            let key = origin
                .ridge_distance
                .map(|d| d.value() / origin.half_rate.value());
            Some((w, key, c.seafloor_age?.value()))
        })
        .collect()
}

#[test]
fn the_steady_state_ages_rank_the_kinematic_ones_onto_the_triangular_law() {
    let earth = earth();
    let ocean = earth_ocean();
    assert!(!ocean.is_empty());
    // Every oceanic cell has an age and no other cell has one.
    for c in &earth.relief.cells {
        assert_eq!(c.seafloor_age.is_some(), c.crust == Crust::Oceanic);
    }
    let area: f64 = ocean.iter().map(|o| o.0).sum();
    let mean = ocean.iter().map(|o| o.0 * o.2).sum::<f64>() / area;
    let sphere: f64 = earth.world.grid.solid_angles_sr().iter().sum();
    let inputs = &earth.world.inputs;
    let t_m = oldest_floor_age(
        area / sphere,
        inputs.heat_flow(),
        inputs.craters().system_age(),
    );
    assert!(
        (mean / (t_m.value() / 3.0) - 1.0).abs() < 1e-9,
        "{} Myr against {} Myr",
        mean / SECONDS_PER_MEGAYEAR,
        t_m.value() / 3.0 / SECONDS_PER_MEGAYEAR
    );
    // Never younger for a larger key, equal keys equal ages, and the ridgeless the oldest.
    let mut keyed: Vec<(f64, f64)> = ocean.iter().filter_map(|o| Some((o.1?, o.2))).collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    for pair in keyed.windows(2) {
        assert!(pair[1].1 >= pair[0].1, "{pair:?}");
        if pair[1].0.total_cmp(&pair[0].0).is_eq() {
            assert_eq!(pair[1].1.total_cmp(&pair[0].1), core::cmp::Ordering::Equal);
        }
    }
    let oldest_keyed = keyed.last().map_or(0.0, |k| k.1);
    for o in ocean.iter().filter(|o| o.1.is_none()) {
        assert!(o.2 > oldest_keyed);
    }
}

/// The area-weighted 250 m histogram of `cells`' elevations under `weights`, binned as Earth2014
/// was (edges on multiples of 250 m): bin k holds [250 k, 250 (k + 1)) m.
fn histogram(elevations: &[f64], weights: &[f64]) -> std::collections::BTreeMap<i64, f64> {
    let mut bins = std::collections::BTreeMap::new();
    for (&z, &w) in elevations.iter().zip(weights) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a coarse elevation's 250 m bin is within an i64"
        )]
        let k = (z / 250.0).floor() as i64;
        *bins.entry(k).or_insert(0.0) += w;
    }
    bins
}

/// The centre, metres, of the modal bin among those of `bins` whose bin indices `keep` holds.
fn modal_centre(bins: &std::collections::BTreeMap<i64, f64>, keep: impl Fn(i64) -> bool) -> f64 {
    let (k, _) =
        bins.iter()
            .filter(|(k, _)| keep(**k))
            .fold((0, f64::NEG_INFINITY), |best, (&k, &w)| {
                if w > best.1 { (k, w) } else { best }
            });
    #[expect(clippy::cast_precision_loss, reason = "a bin index is small")]
    let centre = (k as f64 + 0.5) * 250.0;
    centre
}

/// The reference Earth's hypsometry is bimodal as Earth2014's is (`decision-r09-t12-coarse.md`
/// item 1, replacing `09`'s mode and mean clauses): the land mode within 300 m of +125 m; the
/// ocean's modal bin among the centres −5,125 to −4,125 m, Earth2014's bins within 10% of its
/// −4,375 m peak, a plateau on which one modal bin is ill-posed (`09`'s "flat peak" made exact);
/// and the mean below −2 km from −4,731 to −3,981 m, Earth2014's −4,281 m with +300 m and −450 m.
/// The deep side is 150 m wider for the intraplate volcanic relief no task builds on a mobile lid
/// (Harris et al. 2014's plateaus, seamounts, guyots and non-spreading ridges, 10.2% of the ocean
/// standing 1.5–3 km above the floor, with hotspot swells: some 250–300 m of Earth2014's mean,
/// estimated), a recorded gap; the band returns to ±300 m when that volcanism is built.
#[test]
fn the_reference_earth_s_hypsometry_is_earth2014_s() {
    let earth = earth();
    let z: Vec<f64> = earth
        .relief
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let omega = earth.world.grid.solid_angles_sr();
    let bins = histogram(&z, omega);
    let land = modal_centre(&bins, |k| k >= -8);
    let ocean = modal_centre(&bins, |k| k < -8);
    let (mut area, mut sum) = (0.0, 0.0);
    for (&e, &w) in z.iter().zip(omega) {
        if e < -2_000.0 {
            area += w;
            sum += e * w;
        }
    }
    let deep_mean = sum / area;
    eprintln!("land mode {land} m, ocean modal bin {ocean} m, mean below −2 km {deep_mean:.0} m");
    assert!((land - 125.0).abs() <= 300.0, "land mode {land} m");
    assert!(
        (-5_125.0..=-4_125.0).contains(&ocean),
        "ocean mode {ocean} m"
    );
    assert!((-4_731.0..=-3_981.0).contains(&deep_mean), "{deep_mean} m");
}

/// On the reference Earth, oceanic cells whose style amplitude is at most 0.2 cover 25–40% of the
/// oceanic crust deeper than 3 km (Harris et al. 2014's abyssal plains, 32.9% of the abyss), and
/// the mean of `S_p` + `S_t` over oceanic crust farther than 200 km from a passive margin is
/// 250–600 m (GlobSed's 404 m).
#[test]
fn the_reference_earth_s_plains_and_sediment_are_earth_s() {
    let earth = earth();
    let omega = earth.world.grid.solid_angles_sr();
    let (mut deep, mut plains) = (0.0, 0.0);
    for ((record, state), &w) in earth
        .field
        .synthesis()
        .iter()
        .zip(&earth.relief.cells)
        .zip(omega)
    {
        if record.crust != Crust::Oceanic || state.elevation.value() >= -3_000.0 {
            continue;
        }
        deep += w;
        let style = ReliefStyle::from_codes(
            record.crust,
            record.boundary,
            record.boundary_distance_km,
            record.boundary_obliquity,
            record.seafloor,
        );
        if style.amplitude() <= 0.2 {
            plains += w;
        }
    }
    let share = plains / deep;
    let pass = earth.world.pass(reference::SEED);
    let margins = margin_distances(&pass, &earth.relief);
    let figure = earth.world.inputs.figure();
    let (mut far, mut sediment) = (0.0, 0.0);
    for (n, state) in earth.relief.cells.iter().enumerate() {
        let Some(age) = state.seafloor_age else {
            continue;
        };
        if margins[n].is_some_and(|x| x <= 200e3) {
            continue;
        }
        let normal = figure.normal(earth.world.grid.centre(cell_of(n)));
        let latitude = Radians::new(math::asin(normal[2]));
        let s = pelagic_drape(age, latitude).value() + state.ponded_sediment.value();
        far += omega[n];
        sediment += s * omega[n];
    }
    let mean = sediment / far;
    eprintln!(
        "plains {:.1}% of the floor below 3 km; S beyond 200 km {mean:.0} m",
        100.0 * share
    );
    assert!((0.25..=0.40).contains(&share), "{share}");
    assert!((250.0..=600.0).contains(&mean), "{mean} m");
}

#[test]
fn the_wedge_never_crosses_a_plate_boundary() {
    let earth = earth();
    let cells = &earth.relief.cells;
    let grid = &earth.world.grid;
    let mut margined = std::collections::BTreeSet::new();
    for (n, c) in cells.iter().enumerate() {
        if c.crust == Crust::Oceanic
            && grid.edge_neighbours(cell_of(n)).into_iter().any(|m| {
                let o = &cells[slot(m)];
                o.crust == Crust::Continental && o.plate == c.plate
            })
        {
            margined.insert(c.plate);
        }
    }
    for c in cells {
        if c.ponded_sediment.value() > 0.0 {
            assert_eq!(c.crust, Crust::Oceanic);
            assert!(margined.contains(&c.plate), "plate {}", c.plate);
        }
        if c.crust == Crust::Oceanic {
            assert!(c.hill_relief.is_some_and(|h| h.value() > 0.0));
        } else {
            assert_same_bits(c.ponded_sediment.value(), 0.0);
            assert!(c.hill_relief.is_none());
        }
    }
    // Over a plate boundary the wedge stops: on hand-set plates, the sediment of plate 0's margin
    // never reaches plate 1, which has none.
    let world = World::new(reference::earth_like());
    let pass = World::pass(&world, reference::SEED);
    let mut working = Working::initial(&pass);
    for (n, c) in working.cells.iter_mut().enumerate() {
        let x = world.grid.centre(cell_of(n));
        c.plate = u8::from(x[0] < 0.0);
        c.crust = if x[2] > 0.9 && x[0] > 0.0 {
            Crust::Continental
        } else {
            Crust::Oceanic
        };
    }
    let margins = margin_distances(&pass, &working);
    for (c, m) in working.cells.iter().zip(&margins) {
        assert_eq!(m.is_some(), c.plate == 0 && c.crust == Crust::Oceanic);
    }
}

/// The continental population is its core's mean and width exactly, its skew putting its mode at
/// Earth2014's +125 m, and with the convergent landforms on it the continental crust has
/// Earth2014's mean and width within the addendum's 100 m and 10%; the oceanic crust's spread is
/// plan 14's 0.93 km within the noise's correlation with the floor.
#[test]
fn the_populations_have_their_widths() {
    let s = CONTINENTAL_SKEW;
    let mode = (math::exp(-s * s) - math::exp(0.5 * s * s))
        / (math::exp_m1(s * s) * math::exp(s * s)).sqrt();
    let mode_m = CONTINENTAL_CORE_MEAN.value() + CONTINENTAL_CORE_WIDTH.value() * mode;
    // Earth2014's land mode, +125 m.
    assert!((mode_m - 125.0).abs() < 0.5, "{mode_m} m");
    let earth = earth();
    let omega = earth.world.grid.solid_angles_sr();
    // The populations as T12.a leaves them, before the convergent landforms.
    let populations = earth.populations.as_ref().unwrap();
    let z: Vec<f64> = populations
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let land: Vec<bool> = populations
        .cells
        .iter()
        .map(|c| c.crust == Crust::Continental)
        .collect();
    let (mean, spread) = subset_mean_and_spread(&z, omega, &land);
    assert!(
        (mean - CONTINENTAL_CORE_MEAN.value()).abs() < 1e-6,
        "{mean} m"
    );
    assert!(
        (spread - CONTINENTAL_CORE_WIDTH.value()).abs() < 1e-6,
        "{spread} m"
    );
    // With the convergent landforms on it, the continental crust keeps Earth2014's mean within
    // 100 m and its width within 10% (the addendum's item 3).
    let with: Vec<f64> = earth
        .relief
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let (crust_mean, crust_spread) = subset_mean_and_spread(&with, omega, &land);
    eprintln!(
        "continental crust with its landforms: mean {crust_mean:.0} m, s.d. {crust_spread:.0} m"
    );
    assert!((crust_mean - 405.0).abs() <= 100.0, "{crust_mean} m");
    assert!(
        (crust_spread / 1_080.0 - 1.0).abs() <= 0.1,
        "{crust_spread} m"
    );
    let sea: Vec<bool> = land.iter().map(|&l| !l).collect();
    let (ocean_mean, ocean_spread) = subset_mean_and_spread(&z, omega, &sea);
    eprintln!("oceanic mean {ocean_mean:.0} m, s.d. {ocean_spread:.0} m");
    assert!((ocean_spread / 930.0 - 1.0).abs() < 0.1, "{ocean_spread} m");
}

/// The builder values of `inputs`, as a builder to vary.
fn builder_of(inputs: &CoarseInputs) -> crate::planetary::surface::inputs::CoarseInputsBuilder {
    let mut builder = CoarseInputs::builder()
        .body(inputs.body())
        .figure(inputs.figure())
        .gravity(inputs.gravity())
        .sigma_h(inputs.sigma_h())
        .liquids(inputs.liquids().iter().copied())
        .ices(inputs.ices().iter().copied())
        .mean_surface_temperature(inputs.mean_surface_temperature())
        .equator_pole_contrast_k(inputs.equator_pole_contrast_k())
        .day_night_contrast_k(inputs.day_night_contrast_k())
        .spin(*inputs.spin())
        .host_flux(inputs.host_flux())
        .surface_pressure(inputs.surface_pressure())
        .gases(inputs.gases().iter().copied())
        .surface(inputs.surface_state(), inputs.surface_material())
        .tectonics(inputs.tectonics(), inputs.continental_fraction())
        .volcanism(inputs.volcanism())
        .heat_flow(inputs.heat_flow())
        .surface_age(inputs.surface_age())
        .craters(*inputs.craters())
        .climate(*inputs.climate())
        .crust(*inputs.crust())
        .condensates(inputs.condensates().iter().copied())
        .palette(inputs.palette().clone());
    if let Some(orbit) = inputs.seasonal_orbit() {
        builder = builder.seasonal_orbit(*orbit);
    }
    if let Some(epoch) = inputs.wet_epoch() {
        builder = builder.wet_epoch(*epoch);
    }
    builder
}

/// Every lid without plates is one population of plan 14's `STAGNANT_LID_RELIEF` about the datum:
/// its area mean zero and its area RMS the constant, both to 10⁻⁹, on the four regimes and on a
/// mobile lid R09.T11 lays out no plates for; and a mobile lid with plates has none
/// (`decision-r09-t12-coarse.md`'s addendum).
#[test]
fn every_lid_without_plates_is_one_population_about_the_datum() {
    // No volcanism, so that T12.c's provinces leave the population alone.
    let small = |regime: TectonicRegime| {
        builder_of(&reference::earth_like())
            .figure(hyperion_surface::spheroid::Spheroid::sphere(1.0e6))
            .tectonics(regime, 0.0)
            .volcanism(0.0)
            .build()
            .unwrap()
    };
    let ceres_mobile = builder_of(&reference::ceres_like())
        .tectonics(TectonicRegime::MobileLid, 0.0)
        .build()
        .unwrap();
    for (name, inputs) in [
        ("stagnant", small(TectonicRegime::StagnantLid)),
        ("heat pipe", small(TectonicRegime::HeatPipe)),
        ("episodic", small(TectonicRegime::Episodic)),
        ("ice shell", small(TectonicRegime::IceShell)),
        ("mobile Ceres", ceres_mobile),
    ] {
        let world = relieve(inputs);
        assert!(world.relief.plates.is_empty(), "{name}");
        let z: Vec<f64> = world
            .relief
            .cells
            .iter()
            .map(|c| c.elevation.value())
            .collect();
        let (mean, spread) = noise::mean_and_spread(&z, world.world.grid.solid_angles_sr());
        let width = STAGNANT_LID_RELIEF.value();
        assert!(mean.abs() < 1e-9 * width, "{name}: {mean} m");
        assert!((spread / width - 1.0).abs() < 1e-9, "{name}: {spread} m");
    }
    // The mobile Earth's two populations are not one about the datum.
    let earth = earth();
    let z: Vec<f64> = earth
        .relief
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let (mean, _) = noise::mean_and_spread(&z, earth.world.grid.solid_angles_sr());
    assert!(mean < -1_000.0, "{mean} m");
}

/// The lid's red field has no degree-1 term, and its degrees 2–5 carry about 72% of its variance
/// at β = 2 (`decision-r09-t12-coarse.md`'s addendum): on the reference Moon, the share the
/// polynomials of degree 5 and less (the spherical harmonics to degree 5) explain by area-weighted
/// least squares, a realisation's of 32 modes, within 0.55–0.9; and an ice shell's β₁ of 1.8 is
/// redder at the cell than silicate's 2.0.
#[test]
fn the_lid_s_field_is_red_from_degree_two() {
    let world = World::new(reference::moon_like());
    let pass = world.pass(reference::SEED);
    let omega = world.grid.solid_angles_sr();
    let centres = world.grid.centres();
    let field = noise::red_field(&pass, noise::LANDFORM_INSTANCE, 2.0);
    // Its degree-0 and degree-1 parts are gone: a second removal moves nothing.
    let mut again = field.clone();
    noise::remove_degree_one(&mut again, centres, omega);
    let moved = field
        .iter()
        .zip(&again)
        .fold(0.0, |m: f64, (a, b)| m.max((a - b).abs()));
    assert!(moved < 1e-9, "{moved}");
    let share = low_degree_share(&field, centres, omega, 5);
    eprintln!(
        "degrees 2–5 carry {:.1}% of the lid's variance",
        100.0 * share
    );
    assert!((0.55..=0.9).contains(&share), "{share}");
    let ice = noise::red_field(&pass, noise::LANDFORM_INSTANCE, 1.8);
    let neighbour_difference = |f: &[f64]| {
        let mut sum = 0.0;
        for n in 0..f.len() {
            for m in world.grid.edge_neighbours(cell_of(n)) {
                sum += (f[n] - f[slot(m)]) * (f[n] - f[slot(m)]);
            }
        }
        sum
    };
    assert!(neighbour_difference(&ice) > neighbour_difference(&field));
    // The class: an ice shell or an ice surface is ice-rich, any other rock surface silicate.
    let ice_shell = builder_of(&reference::moon_like())
        .tectonics(TectonicRegime::IceShell, 0.0)
        .build()
        .unwrap();
    assert_eq!(crust_shape(&ice_shell), SpectrumShape::ICE_RICH);
    assert_eq!(
        crust_shape(&reference::mars_like()),
        SpectrumShape::SILICATE
    );
}

/// The share of `values`' area variance that the polynomials in x, y and z of `centres` of degree
/// `degree` and less explain under `weights`: modified Gram–Schmidt over the monomials, dropping
/// those the sphere makes dependent (x² + y² + z² = 1).
fn low_degree_share(values: &[f64], centres: &[[f64; 3]], weights: &[f64], degree: u32) -> f64 {
    let dot = |a: &[f64], b: &[f64]| -> f64 {
        a.iter()
            .zip(b)
            .zip(weights)
            .map(|((x, y), w)| x * y * w)
            .sum()
    };
    let mut basis: Vec<Vec<f64>> = Vec::new();
    for total in 0..=degree {
        for a in 0..=total {
            for b in 0..=total - a {
                let c = total - a - b;
                let mut v: Vec<f64> = centres
                    .iter()
                    .map(|p| {
                        math::powi(p[0], i32::try_from(a).unwrap())
                            * math::powi(p[1], i32::try_from(b).unwrap())
                            * math::powi(p[2], i32::try_from(c).unwrap())
                    })
                    .collect();
                let norm0 = dot(&v, &v).sqrt();
                for u in &basis {
                    let k = dot(&v, u);
                    for (x, y) in v.iter_mut().zip(u) {
                        *x -= k * y;
                    }
                }
                let norm = dot(&v, &v).sqrt();
                if norm > 1e-8 * norm0 {
                    for x in &mut v {
                        *x /= norm;
                    }
                    basis.push(v);
                }
            }
        }
    }
    let total = dot(values, values);
    let explained: f64 = basis
        .iter()
        .map(|u| {
            let p = dot(values, u);
            p * p
        })
        .sum();
    explained / total
}

/// A stagnant lid's one population is plan 14's 0.9 km about the datum.
#[test]
fn a_stagnant_lid_is_one_population_about_the_datum() {
    let moon = relieve(reference::moon_like());
    let omega = moon.world.grid.solid_angles_sr();
    let z: Vec<f64> = moon
        .relief
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let (mean, spread) = noise::mean_and_spread(&z, omega);
    assert!(mean.abs() < 1e-6, "{mean} m");
    assert!((spread - 900.0).abs() < 1e-6, "{spread} m");
    assert!(moon.relief.cells.iter().all(|c| {
        c.crust == Crust::Lid
            && c.hill_relief.is_none()
            && c.water_surface
                .value()
                .total_cmp(&c.elevation.value())
                .is_eq()
    }));
    // The interim figures are the cells' own.
    assert!((moon.relief.realised_sigma_h.value() - spread).abs() < 1e-9);
    let lowest = z.iter().copied().fold(f64::INFINITY, f64::min);
    let highest = z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert_same_bits(moon.relief.sea_level.value(), lowest);
    assert_same_bits(moon.relief.realised_relief.value(), highest - lowest);
    assert_same_bits(moon.field.header().sea_level().value(), lowest);
}

#[test]
fn the_same_seed_gives_the_same_relief_and_another_seed_another() {
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
    assert!(
        first.cells.iter().zip(&other.cells).any(|(a, b)| a
            .elevation
            .value()
            .total_cmp(&b.elevation.value())
            .is_ne())
    );
}

/// The noise is zero-mean and unit-RMS, and its octaves are smooth: neighbouring cells correlate.
#[test]
fn the_landform_noise_is_a_smooth_unit_field() {
    let world = World::new(reference::moon_like());
    let pass = world.pass(reference::SEED);
    let field = noise::unit_field(&pass, noise::LANDFORM_INSTANCE, 2, 6);
    let omega = world.grid.solid_angles_sr();
    let (mean, spread) = noise::mean_and_spread(&field, omega);
    assert!(mean.abs() < 1e-12 && (spread - 1.0).abs() < 1e-12);
    let (mut sum, mut count) = (0.0, 0.0);
    for n in 0..field.len() {
        for m in world.grid.edge_neighbours(cell_of(n)) {
            sum += field[n] * field[slot(m)];
            count += 1.0;
        }
    }
    let correlation = sum / count;
    assert!(correlation > 0.9, "{correlation}");
}

/// Measurements for the record, run by hand: the reference Earth's ages, hypsometry against the
/// kinematic ages', ridge production, boundary lengths, sediment and noise (§7 of the brief).
#[test]
#[ignore = "measurement: the reference Earth's sea-floor figures, printed"]
fn measure_the_reference_earth_s_sea_floor() {
    let earth = earth();
    let grid = &earth.world.grid;
    let omega = grid.solid_angles_sr();
    let cells = &earth.relief.cells;
    let sphere: f64 = omega.iter().sum();
    let ocean = earth_ocean();
    let area: f64 = ocean.iter().map(|o| o.0).sum();
    let a_o = area / sphere;
    let inputs = &earth.world.inputs;
    let t_m = oldest_floor_age(a_o, inputs.heat_flow(), inputs.craters().system_age());
    let mean_ss = ocean.iter().map(|o| o.0 * o.2).sum::<f64>() / area;
    let kinematic: Vec<(f64, f64)> = cells
        .iter()
        .zip(omega)
        .filter_map(|(c, &w)| Some((w, c.seafloor_origin?.age.value())))
        .collect();
    let mean_k = kinematic.iter().map(|k| k.0 * k.1).sum::<f64>() / area;
    eprintln!(
        "a_o {a_o:.4}; t_m {:.1} Myr; τ_ss mean {:.2} Myr (t_m/3 {:.2}); kinematic mean {:.1} Myr",
        t_m.value() / SECONDS_PER_MEGAYEAR,
        mean_ss / SECONDS_PER_MEGAYEAR,
        t_m.value() / 3.0 / SECONDS_PER_MEGAYEAR,
        mean_k / SECONDS_PER_MEGAYEAR
    );
    for limit in [120.0, 150.0, 180.0] {
        let older: f64 = ocean
            .iter()
            .filter(|o| o.2 > limit * SECONDS_PER_MEGAYEAR)
            .map(|o| o.0)
            .sum();
        eprintln!("older than {limit} Myr: {:.2}%", 100.0 * older / area);
    }
    let capped: f64 = kinematic
        .iter()
        .filter(|k| k.1 >= crate::planetary::surface::steps::plates::OLDEST_FLOOR.value())
        .map(|k| k.0)
        .sum();
    eprintln!("kinematic floor at the cap: {:.1}%", 100.0 * capped / area);
    // Hypsometry, and the same from the kinematic ages (a test-only probe).
    let z: Vec<f64> = cells.iter().map(|c| c.elevation.value()).collect();
    hypsometry_line("τ_ss", &z, omega);
    let mut raw = z.clone();
    let pass = earth.world.pass(reference::SEED);
    let margins = margin_distances(&pass, &earth.relief);
    let figure = inputs.figure();
    for (n, c) in cells.iter().enumerate() {
        let (Some(origin), Some(age)) = (c.seafloor_origin, c.seafloor_age) else {
            continue;
        };
        let normal = figure.normal(grid.centre(cell_of(n)));
        let latitude = Radians::new(math::asin(normal[2]));
        let floor = |t: Seconds| {
            let s = pelagic_drape(t, latitude).value()
                + ponded_sediment(margins[n].map(Metres::new)).value();
            -gdh1_depth(t).value() + sediment_load(Metres::new(s)).value()
        };
        raw[n] = z[n] - floor(age) + floor(origin.age);
    }
    hypsometry_line("kinematic τ", &raw, omega);
}

/// Measurements for the record, run by hand: the reference Earth's sediment, load and noise, its
/// ridge production against the law's, its divergent boundary lengths and its interim figures.
#[test]
#[ignore = "measurement: the reference Earth's sediment and ridges, printed"]
fn measure_the_reference_earth_s_sediment_and_ridges() {
    let earth = earth();
    let grid = &earth.world.grid;
    let omega = grid.solid_angles_sr();
    let r = earth.world.inputs.radius().value();
    let cells = &earth.relief.cells;
    let sphere: f64 = omega.iter().sum();
    let area: f64 = earth_ocean().iter().map(|o| o.0).sum();
    let inputs = &earth.world.inputs;
    let t_m = oldest_floor_age(
        area / sphere,
        inputs.heat_flow(),
        inputs.craters().system_age(),
    );
    let figure = inputs.figure();
    // Sediment and load over the oceanic crust; the noise's RMS.
    let (mut s_sum, mut l_sum, mut base) = (0.0, 0.0, Vec::new());
    for (n, c) in cells.iter().enumerate() {
        let Some(age) = c.seafloor_age else { continue };
        let normal = figure.normal(grid.centre(cell_of(n)));
        let latitude = Radians::new(math::asin(normal[2]));
        let s = pelagic_drape(age, latitude).value() + c.ponded_sediment.value();
        let l = sediment_load(Metres::new(s)).value();
        s_sum += s * omega[n];
        l_sum += l * omega[n];
        base.push((omega[n], -gdh1_depth(age).value() + l));
    }
    let base_mean = base.iter().map(|b| b.0 * b.1).sum::<f64>() / area;
    let base_sd = (base
        .iter()
        .map(|b| b.0 * (b.1 - base_mean) * (b.1 - base_mean))
        .sum::<f64>()
        / area)
        .sqrt();
    eprintln!(
        "mean S {:.0} m, mean load {:.0} m; floor s.d. {base_sd:.0} m, noise RMS {:.0} m",
        s_sum / area,
        l_sum / area,
        (930.0_f64 * 930.0 - base_sd * base_sd).max(0.0).sqrt()
    );
    // The ridge production and the divergent boundary lengths.
    let (mut production, mut oceanic_ridge, mut continental_rift) = (0.0, 0.0, 0.0);
    for (n, c) in cells.iter().enumerate() {
        if c.boundary != hyperion_surface::field::BoundaryKind::Divergent || !c.on_boundary() {
            continue;
        }
        for m in grid.edge_neighbours(cell_of(n)) {
            if cells[slot(m)].plate == c.plate {
                continue;
            }
            let length = core::f64::consts::FRAC_PI_4 * grid.arc(cell_of(n), m).value() * r;
            match c.crust {
                Crust::Oceanic => {
                    oceanic_ridge += 0.5 * length;
                    if let Some(origin) = c.seafloor_origin
                        && origin
                            .ridge_distance
                            .is_some_and(|d| d.value().total_cmp(&0.0).is_eq())
                    {
                        production += length * origin.half_rate.value();
                    }
                }
                Crust::Continental | Crust::Lid | Crust::Province => {
                    continental_rift += 0.5 * length;
                }
            }
        }
    }
    let law = 2.0 * area * r * r / t_m.value();
    eprintln!(
        "ridge production {production:.4} m² s⁻¹ against the law's {law:.4} (ratio {:.3}); \
         divergent length oceanic {:.0} km, continental {:.0} km",
        production / law,
        oceanic_ridge / 1e3,
        continental_rift / 1e3
    );
    let h0: Vec<String> = [28.0, 35.0, 40.0, 55.0, 75.0, 110.0, 160.0]
        .iter()
        .map(|&f| {
            format!(
                "{f}: {:.1} m",
                basement_hill_relief(half_of_full(f)).value()
            )
        })
        .collect();
    eprintln!("H0 by full rate: {}", h0.join(", "));
    eprintln!(
        "realised interim σ_h {:.0} m, relief {:.0} m, sea level {:.0} m",
        earth.relief.realised_sigma_h.value(),
        earth.relief.realised_relief.value(),
        earth.relief.sea_level.value()
    );
}

/// The calibration of the ponded sediment's e-folding length (`decision-r09-t5.md` item 5: "T12.a's
/// test calibrates it"), run by hand: the reference Earth's plains' share of the floor below 3 km
/// and its mean sediment beyond 200 km of a margin, at each length from 200 to 500 km, the wedge
/// re-laid and the floor's depth moved by its load (the noise as built).
#[test]
#[ignore = "measurement: the ponded sediment's e-folding length, calibrated"]
fn calibrate_the_ponded_sediment_s_e_folding_length() {
    use hyperion_surface::field::SynthesisCell;
    let earth = earth();
    let grid = &earth.world.grid;
    let omega = grid.solid_angles_sr();
    let pass = earth.world.pass(reference::SEED);
    let margins = margin_distances(&pass, &earth.relief);
    let figure = earth.world.inputs.figure();
    for lambda_km in [200.0, 225.0, 250.0, 275.0, 300.0, 350.0, 400.0, 500.0] {
        let lambda = lambda_km * 1e3;
        let wedge = |x: Option<f64>| {
            x.filter(|&x| x <= 3.0 * lambda)
                .map_or(0.0, |x| 2_400.0 * math::exp(-x / lambda))
        };
        let (mut deep, mut plains, mut far, mut sediment) = (0.0, 0.0, 0.0, 0.0);
        for (n, (record, state)) in earth
            .field
            .synthesis()
            .iter()
            .zip(&earth.relief.cells)
            .enumerate()
        {
            let Some(age) = state.seafloor_age else {
                continue;
            };
            let normal = figure.normal(grid.centre(cell_of(n)));
            let latitude = Radians::new(math::asin(normal[2]));
            let drape = pelagic_drape(age, latitude).value();
            let ponded = wedge(margins[n]);
            let z = state.elevation.value()
                - sediment_load(Metres::new(drape + state.ponded_sediment.value())).value()
                + sediment_load(Metres::new(drape + ponded)).value();
            if margins[n].is_none_or(|x| x > 200e3) {
                far += omega[n];
                sediment += (drape + ponded) * omega[n];
            }
            if z >= -3_000.0 {
                continue;
            }
            deep += omega[n];
            let hill = SynthesisCell::quantise_hill_relief(state.hill_relief.unwrap()).unwrap();
            let pond = SynthesisCell::quantise_ponded_sediment(Metres::new(ponded)).unwrap();
            let style = ReliefStyle::from_codes(
                record.crust,
                record.boundary,
                record.boundary_distance_km,
                record.boundary_obliquity,
                SynthesisCell::pack_seafloor(hill, pond).unwrap(),
            );
            if style.amplitude() <= 0.2 {
                plains += omega[n];
            }
        }
        eprintln!(
            "λ {lambda_km} km: plains {:.1}% of the floor below 3 km; S beyond 200 km {:.0} m",
            100.0 * plains / deep,
            sediment / far
        );
    }
}

/// Prints `label`'s hypsometry of `z` under `weights`: the land mode, the ocean's modal bin, and
/// the mean, s.d., median and quartiles below −2 km.
fn hypsometry_line(label: &str, z: &[f64], weights: &[f64]) {
    let bins = histogram(z, weights);
    let land = modal_centre(&bins, |k| k >= -8);
    let ocean = modal_centre(&bins, |k| k < -8);
    let mut deep: Vec<(f64, f64)> = z
        .iter()
        .zip(weights)
        .filter(|(e, _)| **e < -2_000.0)
        .map(|(&e, &w)| (e, w))
        .collect();
    deep.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = deep.iter().map(|d| d.1).sum();
    let mean = deep.iter().map(|d| d.0 * d.1).sum::<f64>() / total;
    let sd = (deep
        .iter()
        .map(|d| d.1 * (d.0 - mean) * (d.0 - mean))
        .sum::<f64>()
        / total)
        .sqrt();
    let quantile = |q: f64| {
        let mut acc = 0.0;
        for d in &deep {
            acc += d.1;
            if acc >= q * total {
                return d.0;
            }
        }
        deep.last().map_or(0.0, |d| d.0)
    };
    eprintln!(
        "{label}: land mode {land} m, ocean modal bin {ocean} m; below −2 km mean {mean:.0} m, \
         s.d. {sd:.0} m, quartiles {:.0} / {:.0} / {:.0} m",
        quantile(0.25),
        quantile(0.5),
        quantile(0.75)
    );
}

#[test]
fn the_height_factor_is_g_earth_over_g() {
    assert_same_bits(
        convergent::height_factor(reference::earth_like().gravity()),
        1.0,
    );
    let mars = convergent::height_factor(reference::mars_like().gravity());
    assert!((mars - 2.6343).abs() < 1e-4, "{mars}");
    let doubled =
        crate::units::MetresPerSecondSquared::new(2.0 * convergent::EARTH_GRAVITY.value());
    assert_same_bits(convergent::height_factor(doubled), 0.5);
    assert!((convergent::EARTH_GRAVITY.value() - 9.820_25).abs() < 5e-6);
}

/// Each cell's lift by the convergent landforms on the reference Earth, metres: its elevation
/// less the populations'.
fn earth_lift() -> Vec<f64> {
    let earth = earth();
    let before = earth.populations.as_ref().unwrap();
    earth
        .relief
        .cells
        .iter()
        .zip(&before.cells)
        .map(|(after, before)| after.elevation.value() - before.elevation.value())
        .collect()
}

/// Whether cell `n` lies in an arc's band on the reference Earth, its rise included.
fn in_arc(arcs: &convergent::Arcs, n: usize) -> bool {
    match (arcs.distance_m[n], arcs.front_m[n], arcs.width_m[n]) {
        (Some(y), Some(g), Some(w)) => y > g - 0.25 * w && y < g + w,
        _ => false,
    }
}

/// Belts lie within their stated distance of collision boundaries, on continental crust: every
/// lift outside an arc's band is a belt's, within the widest half-width (250 km), and every
/// continental cell within the narrowest plateau (50 km) of a collision is lifted.
#[test]
fn belts_lie_within_their_distance_of_collisions() {
    let earth = earth();
    let arcs = earth.arcs.as_ref().unwrap();
    let lift = earth_lift();
    let mut belts = 0;
    for (n, cell) in earth.relief.cells.iter().enumerate() {
        let d = cell.boundary_distance.map(|d| d.value().abs());
        if lift[n] > 0.0 && !in_arc(arcs, n) {
            assert_eq!(cell.boundary, BoundaryKind::Collision, "cell {n}");
            assert_eq!(cell.crust, Crust::Continental, "cell {n}");
            assert!(d.is_some_and(|d| d <= 250e3), "cell {n}: {d:?}");
            belts += 1;
        }
        if cell.boundary == BoundaryKind::Collision
            && cell.crust == Crust::Continental
            && d.is_some_and(|d| d <= 50e3)
        {
            assert!(lift[n] > 4_000.0, "cell {n}: {}", lift[n]);
        }
    }
    assert!(belts > 100, "{belts} belt cells");
}

/// Trenches lie 2–4 km (times h, 1 here) below the sea floor they cut, at the subducting side's
/// boundary cells, and only on the subducting side.
#[test]
fn trenches_lie_two_to_four_km_below_their_floor() {
    let earth = earth();
    let arcs = earth.arcs.as_ref().unwrap();
    let lift = earth_lift();
    let mut axis = 0;
    for (n, cell) in earth.relief.cells.iter().enumerate() {
        if lift[n] < 0.0 {
            assert!(cell.subducts, "cell {n}");
            assert_eq!(cell.boundary, BoundaryKind::Subduction);
        }
        if cell.subducts && cell.on_boundary() && !in_arc(arcs, n) {
            assert!(
                (-4_000.0 - 1e-6..=-2_000.0 + 1e-6).contains(&lift[n]),
                "cell {n}: {} m",
                lift[n]
            );
            axis += 1;
        }
    }
    assert!(axis > 100, "{axis} trench cells");
}

/// The fronts on the reference Earth: each arc cell within half its spacing of its G, with its
/// distance from the nearest axis point of its plate's trenches by brute force, grouped by the
/// overriding crust at its source (oceanic first); and each front cell's path distance over its
/// great-circle one, the 8-connected excess.
fn fronts() -> ([Vec<f64>; 2], Vec<f64>) {
    let earth = earth();
    let arcs = earth.arcs.as_ref().unwrap();
    let grid = &earth.world.grid;
    let radius_m = earth.world.inputs.radius().value();
    let cells = &earth.relief.cells;
    let mut axes: std::collections::BTreeMap<u8, Vec<[f64; 3]>> = std::collections::BTreeMap::new();
    for (n, point) in arcs.axis.iter().enumerate() {
        if let Some(p) = point {
            axes.entry(cells[n].plate).or_default().push(*p);
        }
    }
    let mut groups = [Vec::new(), Vec::new()];
    let mut excess = Vec::new();
    for n in 0..cells.len() {
        let (Some(y), Some(g), Some(source), Some(path)) = (
            arcs.distance_m[n],
            arcs.front_m[n],
            arcs.source[n],
            arcs.path_m[n],
        ) else {
            continue;
        };
        let cell = cell_of(n);
        let spacing = grid
            .edge_neighbours(cell)
            .iter()
            .map(|&m| grid.arc(cell, m).value() * radius_m)
            .sum::<f64>()
            / 4.0;
        if (y - g).abs() > 0.5 * spacing {
            continue;
        }
        let x = grid.centre(cell);
        let nearest = axes[&cells[n].plate]
            .iter()
            .map(|p| {
                let gap = [x[0] - p[0], x[1] - p[1], x[2] - p[2]];
                let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
                2.0 * math::asin(0.5 * chord) * radius_m
            })
            .fold(f64::INFINITY, f64::min);
        let group = usize::from(cells[slot(source)].crust != Crust::Oceanic);
        groups[group].push(nearest);
        excess.push(path / y - 1.0);
    }
    for g in &mut groups {
        g.sort_by(f64::total_cmp);
    }
    excess.sort_by(f64::total_cmp);
    (groups, excess)
}

/// The `q` quantile of the sorted `values`.
fn quantile(values: &[f64], q: f64) -> f64 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "an index into a short sorted list"
    )]
    let k = ((values.len() - 1) as f64 * q).round() as usize;
    values[k]
}

/// The arcs' volcanic fronts lie behind their trenches with medians within 15 km of 190 km
/// (oceanic overriding plate) and 260 km (continental), 80% of fronts within 140–250 and
/// 180–330 km (`decision-r09-t5.md` item 6; Syracuse and Abers 2006), measured from the trench's
/// axis by brute force.
#[test]
fn the_fronts_lie_at_syracuse_and_abers_s_distances() {
    let ([oceanic, continental], excess) = fronts();
    for (name, values, median, range) in [
        ("oceanic", &oceanic, 190e3, 140e3..=250e3),
        ("continental", &continental, 260e3, 180e3..=330e3),
    ] {
        assert!(values.len() > 30, "{name}: {} fronts", values.len());
        let m = quantile(values, 0.5);
        #[expect(clippy::cast_precision_loss, reason = "a count of fronts")]
        let inside =
            values.iter().filter(|&&v| range.contains(&v)).count() as f64 / values.len() as f64;
        eprintln!(
            "{name} fronts: {} cells, median {:.0} km, 10–90% {:.0}–{:.0} km, {:.0}% in range",
            values.len(),
            m / 1e3,
            quantile(values, 0.1) / 1e3,
            quantile(values, 0.9) / 1e3,
            100.0 * inside
        );
        assert!((m - median).abs() <= 15e3, "{name}: median {m} m");
        assert!(inside >= 0.8, "{name}: {inside}");
    }
    eprintln!(
        "the 8-connected path's excess at the fronts: median {:.1}%, 90% {:.1}%, most {:.1}%",
        100.0 * quantile(&excess, 0.5),
        100.0 * quantile(&excess, 0.9),
        100.0 * excess.last().copied().unwrap_or(0.0)
    );
}

/// Measurements for the record, run by hand: the reference Earth's continental crust with the
/// convergent landforms in (`decision-r09-t12-coarse.md`'s addendum, item 3's condition), the
/// trenches against the floor beyond them, the belts' widths and heights, and h on each reference
/// world.
#[test]
#[ignore = "measurement: the reference Earth's convergent landforms, printed"]
fn measure_the_reference_earth_s_convergent_landforms() {
    let earth = earth();
    let omega = earth.world.grid.solid_angles_sr();
    let cells = &earth.relief.cells;
    let lift = earth_lift();
    let land: Vec<bool> = cells
        .iter()
        .map(|c| c.crust == Crust::Continental)
        .collect();
    let z: Vec<f64> = cells.iter().map(|c| c.elevation.value()).collect();
    let (mean, spread) = subset_mean_and_spread(&z, omega, &land);
    let land_z: Vec<f64> = z
        .iter()
        .zip(&land)
        .map(|(&e, &l)| if l { e } else { f64::NAN })
        .collect();
    let bins = histogram(
        &land_z
            .iter()
            .copied()
            .filter(|e| !e.is_nan())
            .collect::<Vec<_>>(),
        &omega
            .iter()
            .zip(&land)
            .filter(|(_, l)| **l)
            .map(|(w, _)| *w)
            .collect::<Vec<_>>(),
    );
    let modal = modal_centre(&bins, |_| true);
    let (lift_mean, lift_spread) = subset_mean_and_spread(&lift, omega, &land);
    eprintln!(
        "continental crust with the landforms: mean {mean:.0} m, s.d. {spread:.0} m, modal bin \
         {modal} m; the landforms' lift there mean {lift_mean:.0} m, s.d. {lift_spread:.0} m"
    );
    // Belts alone (lifted cells outside arcs), and arcs alone.
    let arcs = earth.arcs.as_ref().unwrap();
    let (mut belt_area, mut arc_area, mut land_area) = (0.0, 0.0, 0.0);
    for (n, &w) in omega.iter().enumerate() {
        if !land[n] {
            continue;
        }
        land_area += w;
        if lift[n] > 0.0 && in_arc(arcs, n) {
            arc_area += w;
        } else if lift[n] > 0.0 {
            belt_area += w;
        }
    }
    eprintln!(
        "lifted continental area: belts {:.1}%, continental arcs {:.1}%",
        100.0 * belt_area / land_area,
        100.0 * arc_area / land_area
    );
    // The belts' plateau heights at |d| ≤ 50 km.
    let plateau: Vec<f64> = cells
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.boundary == BoundaryKind::Collision
                && c.crust == Crust::Continental
                && c.boundary_distance.is_some_and(|d| d.value().abs() <= 50e3)
        })
        .map(|(n, _)| z[n])
        .collect();
    let plateau_mean =
        plateau.iter().sum::<f64>() / f64::from(u32::try_from(plateau.len()).unwrap());
    let plateau_max = plateau.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    eprintln!(
        "belt plateaus within 50 km: {} cells, mean {plateau_mean:.0} m, highest {plateau_max:.0} m",
        plateau.len()
    );
}

/// Measurements for the record, run by hand: the reference Earth's trenches against the floor
/// beyond them, and h on each reference world.
#[test]
#[ignore = "measurement: the reference Earth's trenches and the height factors, printed"]
fn measure_the_trenches_and_the_height_factors() {
    let earth = earth();
    let omega = earth.world.grid.solid_angles_sr();
    let cells = &earth.relief.cells;
    let z: Vec<f64> = cells.iter().map(|c| c.elevation.value()).collect();
    // Trenches: the axis cells' depth against the subducting plate's floor 150–300 km out.
    let mut gaps: Vec<f64> = Vec::new();
    let mut floor_by_plate: std::collections::BTreeMap<u8, (f64, f64)> =
        std::collections::BTreeMap::new();
    for (n, c) in cells.iter().enumerate() {
        let d = c.boundary_distance.map_or(0.0, Metres::value);
        if c.subducts && (-300e3..=-150e3).contains(&d) && c.crust == Crust::Oceanic {
            let e = floor_by_plate.entry(c.plate).or_insert((0.0, 0.0));
            e.0 += z[n] * omega[n];
            e.1 += omega[n];
        }
    }
    let mut axis_depths: Vec<f64> = Vec::new();
    for (n, c) in cells.iter().enumerate() {
        if c.subducts && c.on_boundary() {
            axis_depths.push(z[n]);
            if let Some(&(sum, area)) = floor_by_plate.get(&c.plate) {
                gaps.push(sum / area - z[n]);
            }
        }
    }
    gaps.sort_by(f64::total_cmp);
    axis_depths.sort_by(f64::total_cmp);
    eprintln!(
        "trench axis cells {}: depth median {:.0} m, deepest {:.0} m; below their plate's floor \
         150–300 km out: median {:.0} m, 10–90% {:.0}–{:.0} m",
        axis_depths.len(),
        quantile(&axis_depths, 0.5),
        axis_depths[0],
        quantile(&gaps, 0.5),
        quantile(&gaps, 0.1),
        quantile(&gaps, 0.9)
    );
    for (name, inputs) in [
        ("Earth", reference::earth_like()),
        ("Mars", reference::mars_like()),
        ("Moon", reference::moon_like()),
        ("Ceres", reference::ceres_like()),
    ] {
        eprintln!(
            "h on the reference {name}: {:.4}",
            convergent::height_factor(inputs.gravity())
        );
    }
}

/// Measurements for the record, run by hand: R09.T11's slivers (plates whose widest interior
/// point lies within 100 km of their boundary) and transforms (their share of the boundary and
/// their runs' lengths) on four reference Earths.
#[test]
#[ignore = "measurement: slivers and transforms on four reference Earths, printed"]
fn measure_slivers_and_transforms() {
    for seed in [
        reference::SEED,
        SurfaceSeed::new(0x0dd5),
        SurfaceSeed::new(0x5eed_0001),
        SurfaceSeed::new(0x5eed_0002),
    ] {
        let world = World::new(reference::earth_like());
        let pass = world.pass(seed);
        let mut working = Working::initial(&pass);
        crate::planetary::surface::steps::plates::run(&pass, &mut working);
        let grid = &world.grid;
        let r = world.inputs.radius().value();
        let mut widest = vec![0.0_f64; working.plates.len()];
        for c in &working.cells {
            if let Some(d) = c.boundary_distance {
                let k = usize::from(c.plate);
                widest[k] = widest[k].max(d.value().abs());
            }
        }
        let slivers: Vec<String> = widest
            .iter()
            .zip(&working.plates)
            .filter(|(w, p)| **w <= 100e3 && p.area_sr > 0.0)
            .map(|(w, p)| format!("{:.0} km wide over {:.4} sr", 2.0 * w / 1e3, p.area_sr))
            .collect();
        let (mut transform, mut total) = (0.0, 0.0);
        let mut runs = 0;
        let mut seen = vec![false; working.cells.len()];
        for (n, c) in working.cells.iter().enumerate() {
            if !c.on_boundary() {
                continue;
            }
            for m in grid.edge_neighbours(cell_of(n)) {
                if working.cells[slot(m)].plate != c.plate {
                    let length = core::f64::consts::FRAC_PI_4 * grid.arc(cell_of(n), m).value() * r;
                    total += 0.5 * length;
                    if c.boundary == BoundaryKind::Transform {
                        transform += 0.5 * length;
                    }
                }
            }
            if c.boundary == BoundaryKind::Transform && !seen[n] {
                runs += 1;
                let mut stack = vec![n];
                seen[n] = true;
                while let Some(k) = stack.pop() {
                    for m in grid
                        .edge_neighbours(cell_of(k))
                        .into_iter()
                        .chain(grid.vertex_neighbours(cell_of(k)))
                    {
                        let o = &working.cells[slot(m)];
                        if !seen[slot(m)]
                            && o.boundary == BoundaryKind::Transform
                            && o.on_boundary()
                        {
                            seen[slot(m)] = true;
                            stack.push(slot(m));
                        }
                    }
                }
            }
        }
        eprintln!(
            "seed {seed:?}: {} plates, {} slivers [{}]; transforms {:.1}% of {:.0} km of boundary, \
             {runs} runs, mean {:.0} km (both sides' cells together)",
            working.plates.len(),
            slivers.len(),
            slivers.join("; "),
            100.0 * transform / total,
            total / 1e3,
            2.0 * transform / 1e3 / f64::from(runs.max(1))
        );
    }
}

/// The calibration of the continental population net of the convergent landforms
/// (`decision-r09-t12-coarse.md`'s addendum, item 3), run once by hand on the reference Earth under
/// `reference::SEED`: the mean μ that leaves the crust with them at +405 m, the width √(1,084² −
/// their variance there) and the skew whose mode at that mean and width is +125 m. The belts'
/// lift goes as (plateau − μ), so μ solves μ + B̄ (5,000 − μ) + Ā = 405, with B the belts' shape
/// and A the arcs' lift over the continental cells.
#[test]
#[ignore = "measurement: the continental population net of the landforms, calibrated"]
fn calibrate_the_continental_population_net_of_the_landforms() {
    let earth = earth();
    let pass = earth.world.pass(reference::SEED);
    let omega = earth.world.grid.solid_angles_sr();
    let before = earth.populations.as_ref().unwrap();
    let mut strike = noise::unit_field(
        &pass,
        convergent::STRIKE_INSTANCE,
        convergent::STRIKE_LEVEL,
        convergent::STRIKE_LEVEL,
    );
    for n in &mut strike {
        *n = n.clamp(-convergent::STRIKE_CLAMP, convergent::STRIKE_CLAMP);
    }
    let lift = earth_lift();
    let span = convergent::BELT_PLATEAU.value() - CONTINENTAL_CORE_MEAN.value();
    let (mut area, mut b_sum, mut a_sum) = (0.0, 0.0, 0.0);
    let mut parts = Vec::new();
    for (n, cell) in before.cells.iter().enumerate() {
        if cell.crust != Crust::Continental {
            continue;
        }
        let b = convergent::belt_lift_m(cell, strike[n], 1.0) / span;
        let a = lift[n] - b * span;
        area += omega[n];
        b_sum += b * omega[n];
        a_sum += a * omega[n];
        parts.push((omega[n], b, a));
    }
    let (b_bar, a_bar) = (b_sum / area, a_sum / area);
    let target_mean = 405.0;
    let plateau = convergent::BELT_PLATEAU.value();
    let mu = (target_mean - plateau * b_bar - a_bar) / (1.0 - b_bar);
    let lift_at = |w: &(f64, f64, f64)| w.1 * (plateau - mu) + w.2;
    let lift_mean = parts.iter().map(|w| w.0 * lift_at(w)).sum::<f64>() / area;
    let lift_var = parts
        .iter()
        .map(|w| w.0 * (lift_at(w) - lift_mean) * (lift_at(w) - lift_mean))
        .sum::<f64>()
        / area;
    let sigma = (1_084.0_f64 * 1_084.0 - lift_var).sqrt();
    let mode_of = |s: f64| {
        let s2 = s * s;
        (math::exp(-s2) - math::exp(0.5 * s2)) / (math::exp_m1(s2) * math::exp(s2)).sqrt()
    };
    let target = (125.0 - mu) / sigma;
    let (mut lo, mut hi) = (1e-6, 1.0);
    for _ in 0..100 {
        let mid = f64::midpoint(lo, hi);
        if mode_of(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    eprintln!(
        "belts' shape over the continents B̄ {b_bar:.5}, arcs' lift Ā {a_bar:.1} m; netted mean \
         {mu:.1} m, the landforms' lift there {lift_mean:.1} m (s.d. {:.1} m), netted width \
         {sigma:.1} m, skew {lo:.5} (mode {:.2} m)",
        lift_var.sqrt(),
        mu + sigma * mode_of(lo)
    );
}

/// A line load's bulge crest lies at πα with height e^(−π) `w₀` = 0.043 `w₀` above the
/// undeflected plate, and the step response is the line load integrated (Design note 7).
#[test]
fn a_line_load_s_bulge_crest_lies_at_pi_alpha() {
    let alpha = Metres::new(100e3);
    let (k, weight) = (1e4, 1e12);
    let peak = provinces::line_load_deflection(0.0, alpha, k, weight);
    assert!((peak - weight / (2.0 * k * alpha.value())).abs() < 1e-9 * peak);
    let (mut crest, mut lowest) = (0.0, f64::INFINITY);
    for i in 0..=200_000 {
        let x = alpha.value() * (2.0 + 2.5 * f64::from(i) / 200_000.0);
        let w = provinces::line_load_deflection(x, alpha, k, weight);
        if w < lowest {
            (crest, lowest) = (x, w);
        }
    }
    assert!(
        (crest / alpha.value() - core::f64::consts::PI).abs() < 1e-4,
        "{crest}"
    );
    assert!((lowest / peak + math::exp(-core::f64::consts::PI)).abs() < 1e-9);
    assert!((lowest / peak + 0.043).abs() < 5e-4);
    // The step response's slope is the line load's, per unit weight and foundation.
    for u in [-2.5e5, -3e4, 1e3, 4e4, 3e5] {
        let delta = 1.0;
        let slope = (provinces::step_response(u + delta, alpha)
            - provinces::step_response(u - delta, alpha))
            / (2.0 * delta);
        let line = provinces::line_load_deflection(u, alpha, 1.0, 1.0);
        assert!(
            (slope - line).abs() < 1e-9 / alpha.value(),
            "{u}: {slope} {line}"
        );
    }
    assert_same_bits(
        provinces::step_response(-1.0, alpha) + provinces::step_response(1.0, alpha) - 1.0,
        0.0,
    );
}

/// α at `T_e` = 70 km under Mars's gravity (3.71 m s⁻²) and radius (3,389.5 km), with `ρ_m`
/// 3,300 kg m⁻³ and an open moat (`ρ_fill` = 0), is 180 km ± 10 (`decision-r09-t12-coarse.md` item
/// 3: its value is 175.5 km, 177.7 km planar; Design note 7's "as observed" was the formula's own
/// value). The filled moat of the reference Mars's provinces (2,648 kg m⁻³) gives 251.7 km at 70 km
/// and 337.4 km at its own 107.0 km.
#[test]
fn alpha_at_70_km_under_mars_s_gravity_is_180_km() {
    let (g, r) = (
        crate::units::MetresPerSecondSquared::new(3.71),
        Metres::new(3_389.5e3),
    );
    let t_e = Metres::new(70e3);
    let open = provinces::flexural_parameter(
        t_e,
        crate::units::KilogramsPerCubicMetre::new(3_300.0),
        crate::units::KilogramsPerCubicMetre::new(0.0),
        g,
        r,
    )
    .value();
    assert!((open - 180e3).abs() <= 10e3, "{open} m");
    assert!((open - 175.5e3).abs() < 100.0, "{open} m");
    let planar = provinces::flexural_parameter(
        t_e,
        crate::units::KilogramsPerCubicMetre::new(3_300.0),
        crate::units::KilogramsPerCubicMetre::new(0.0),
        g,
        Metres::new(1e15),
    )
    .value();
    assert!((planar - 177.7e3).abs() < 100.0, "{planar} m");
    let mars = reference::mars_like();
    let fill =
        crate::units::KilogramsPerCubicMetre::new(3_310.0 * (1.0 - provinces::FILL_POROSITY));
    let filled = provinces::flexural_parameter(
        t_e,
        crate::units::KilogramsPerCubicMetre::new(3_300.0),
        fill,
        mars.gravity(),
        mars.radius(),
    )
    .value();
    let own = provinces::elastic_thickness(&mars).unwrap();
    let at_own = provinces::flexural_parameter(
        own,
        crate::units::KilogramsPerCubicMetre::new(3_300.0),
        fill,
        mars.gravity(),
        mars.radius(),
    )
    .value();
    eprintln!(
        "α filled: {:.1} km at 70 km, {:.1} km at the reference Mars's {:.1} km",
        filled / 1e3,
        at_own / 1e3,
        own.value() / 1e3
    );
    assert!((filled - 251.7e3).abs() < 100.0, "{filled} m");
    assert!((at_own - 337.4e3).abs() < 500.0, "{at_own} m");
}

/// `T_e` is 108 km ± 1 at Mars's 19 mW m⁻² and 210 K, and zero where `T_s` ≥ 870 K, where each
/// load is compensated locally (α = 0); on a water-ice lid it is 2.516 km at 200 mW m⁻² and 70 K
/// and zero at 170 K and above; and it is the body's radius where the law passes it (the
/// reference Ceres's 2,192 km).
#[test]
fn the_elastic_thickness_follows_the_lid_s_substance() {
    let at = |temperature: f64, flux: f64| {
        builder_of(&reference::mars_like())
            .mean_surface_temperature(crate::units::Kelvin::new(temperature))
            .heat_flow(WattsPerSquareMetre::new(flux))
            .build()
            .unwrap()
    };
    let mars = provinces::elastic_thickness(&at(210.0, 0.019))
        .unwrap()
        .value();
    assert!((mars - 108e3).abs() <= 1e3, "{mars} m");
    let hot = provinces::elastic_thickness(&at(900.0, 0.019)).unwrap();
    assert_same_bits(hot.value(), 0.0);
    let g = reference::mars_like().gravity();
    let r = reference::mars_like().radius();
    assert_same_bits(
        provinces::flexural_parameter(
            hot,
            crate::units::KilogramsPerCubicMetre::new(3_300.0),
            crate::units::KilogramsPerCubicMetre::new(2_648.0),
            g,
            r,
        )
        .value(),
        0.0,
    );
    // Airy: a load's deflection under it is its whole compensation, and nothing beyond it.
    assert_same_bits(provinces::step_response(0.0, Metres::ZERO), 1.0);
    assert_same_bits(provinces::step_response(-1.0, Metres::ZERO), 0.0);
    let ice = PaletteEntry {
        substance: hyperion_surface::substance_key::SubstanceKey::new("H2O").unwrap(),
        role: hyperion_surface::field::PaletteRole::PrimaryCrust,
        normal_albedo_bvr: [0.6; 3],
        phase_row: 0,
        grain_density_kg_m3: 930.0,
        transition: crate::units::Kelvin::new(273.15),
        mechanics: hyperion_surface::field::MechanicsFamily::WaterIce,
        liquid: None,
    };
    let shell = |t: f64, f: f64| {
        provinces::ice_thickness(
            &ice,
            crate::units::Kelvin::new(t),
            WattsPerSquareMetre::new(f),
        )
        .unwrap()
        .value()
    };
    assert!(
        (shell(70.0, 0.2) - 2_516.0).abs() < 1.0,
        "{} m",
        shell(70.0, 0.2)
    );
    assert_same_bits(shell(170.0, 0.2), 0.0);
    assert_same_bits(shell(200.0, 0.2), 0.0);
    assert!((shell(70.0, 0.27) - 1_860.0).abs() < 10.0);
    assert!(provinces::mantle_density(&ice).is_some_and(|d| d.value().total_cmp(&930.0).is_eq()));
    let ceres = reference::ceres_like();
    let rigid = provinces::elastic_thickness(&ceres).unwrap();
    assert_same_bits(rigid.value(), ceres.radius().value());
}

/// The reference Mars's provinces: edifices at the cap about Mars's provinces, never lowering the
/// ground, the `Province` crust where their load or fill is at least 10 m; none on the Moon (V = 0)
/// or on a mobile lid.
#[test]
fn the_reference_mars_has_volcanic_provinces() {
    let mars = relieve(reference::mars_like());
    let laid = mars
        .provinces
        .as_ref()
        .expect("the reference Mars has provinces");
    eprintln!(
        "Mars: T_e {:.1} km, α {:.1} km, h_max {:.0} m, {} provinces, {} edifices over {:.3} of \
         the surface; realised s.d. {:.0} m ({:.0} m before the correction) against σ_volc {:.0} m",
        laid.elastic.value() / 1e3,
        laid.alpha.value() / 1e3,
        laid.cap.value(),
        laid.count,
        laid.edifices,
        laid.coverage,
        laid.realised.value(),
        laid.first.value(),
        laid.target.value()
    );
    assert!(laid.count >= 1 && laid.edifices >= 1);
    assert!(laid.lift.iter().all(|&l| l >= 0.0));
    // The lean: the realised share approaches σ_volc (0.76 of it as built).
    assert!(
        laid.realised.value() >= 0.7 * laid.target.value(),
        "{} m",
        laid.realised.value()
    );
    let provinces = mars
        .relief
        .cells
        .iter()
        .filter(|c| c.crust == Crust::Province)
        .count();
    assert!(provinces > 0);
    assert!(
        mars.relief
            .cells
            .iter()
            .all(|c| matches!(c.crust, Crust::Lid | Crust::Province))
    );
    assert!(relieve(reference::moon_like()).provinces.is_none());
    assert!(earth().provinces.is_none());
    assert!(
        earth()
            .relief
            .cells
            .iter()
            .all(|c| c.crust != Crust::Province)
    );
}

/// Measurements for the record, run by hand: `T_e` on each reference world, α beside a mantle of
/// 3,500 kg m⁻³, and the reference Mars's provinces in detail.
#[test]
#[ignore = "measurement: the lithospheres and the reference Mars's provinces, printed"]
fn measure_the_lithospheres_and_the_provinces() {
    for (name, inputs) in [
        ("Earth", reference::earth_like()),
        ("Mars", reference::mars_like()),
        ("Moon", reference::moon_like()),
        ("Ceres", reference::ceres_like()),
    ] {
        let t_e = provinces::elastic_thickness(&inputs).map(Metres::value);
        let law = crate::planetary::hooks::figures::relief::lithosphere_thickness(
            crate::planetary::hooks::figures::Crust::Silicate,
            inputs.mean_surface_temperature(),
            inputs.heat_flow(),
        );
        eprintln!(
            "{name}: T_e {:.1} km (the law {:.1} km, R {:.1} km), V {}",
            t_e.unwrap_or(f64::NAN) / 1e3,
            law.value() / 1e3,
            inputs.radius().value() / 1e3,
            inputs.volcanism()
        );
    }
    let mars = reference::mars_like();
    let own = provinces::elastic_thickness(&mars).unwrap();
    let fill =
        crate::units::KilogramsPerCubicMetre::new(3_310.0 * (1.0 - provinces::FILL_POROSITY));
    for mantle in [3_300.0, 3_500.0] {
        let mantle_density = crate::units::KilogramsPerCubicMetre::new(mantle);
        let at = |t: Metres| {
            provinces::flexural_parameter(t, mantle_density, fill, mars.gravity(), mars.radius())
                .value()
                / 1e3
        };
        eprintln!(
            "ρ_m {mantle}: α {:.1} km at 70 km, {:.1} km at {:.1} km; bulge crest near {:.0} km",
            at(Metres::new(70e3)),
            at(own),
            own.value() / 1e3,
            core::f64::consts::PI * at(own)
        );
    }
    let world = relieve(mars);
    let laid = world.provinces.as_ref().unwrap();
    let omega = world.world.grid.solid_angles_sr();
    let sphere: f64 = omega.iter().sum();
    let province_area: f64 = world
        .relief
        .cells
        .iter()
        .zip(omega)
        .filter(|(c, _)| c.crust == Crust::Province)
        .map(|(_, w)| w)
        .sum();
    let highest = laid.lift.iter().copied().fold(0.0, f64::max);
    let raised: f64 = laid
        .lift
        .iter()
        .zip(omega)
        .filter(|(l, _)| **l > 0.0)
        .map(|(_, w)| w)
        .sum();
    eprintln!(
        "Mars's provinces: {} of them, {} edifices of {:.0} km base and {:.0} m height, coverage \
         {:.3}; raised {:.1}% of the surface, the Province crust over {:.1}%, the highest lift \
         {highest:.0} m; realised s.d. {:.0} m against σ_volc {:.0} m",
        laid.count,
        laid.edifices,
        laid.cap.value() / provinces::EDIFICE_ASPECT / 1e3,
        laid.cap.value(),
        laid.coverage,
        100.0 * raised / sphere,
        100.0 * province_area / sphere,
        laid.realised.value(),
        laid.target.value()
    );
    let z: Vec<f64> = world
        .relief
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let (_, spread) = noise::mean_and_spread(&z, omega);
    eprintln!(
        "Mars after T12.a–c: s.d. {spread:.0} m (lid 900 m with provinces), relief {:.0} m",
        world.relief.realised_relief.value()
    );
}

/// On the finished field, the reference Earth's trench axes lie a median 2–4 km below their
/// subducting plate's floor 150–300 km out (Design note 7's "2–4 km below the neighbouring sea
/// floor" on the field as built, where the axis-cell test holds each cell's own cut).
#[test]
fn the_trenches_median_depth_below_the_floor_beyond_is_two_to_four_km() {
    let earth = earth();
    let omega = earth.world.grid.solid_angles_sr();
    let cells = &earth.relief.cells;
    let mut floor: std::collections::BTreeMap<u8, (f64, f64)> = std::collections::BTreeMap::new();
    for (n, c) in cells.iter().enumerate() {
        let d = c.boundary_distance.map_or(0.0, Metres::value);
        if c.subducts && c.crust == Crust::Oceanic && (-300e3..=-150e3).contains(&d) {
            let entry = floor.entry(c.plate).or_insert((0.0, 0.0));
            entry.0 += c.elevation.value() * omega[n];
            entry.1 += omega[n];
        }
    }
    let mut gaps: Vec<f64> = cells
        .iter()
        .filter(|c| c.subducts && c.on_boundary())
        .filter_map(|c| {
            let &(sum, area) = floor.get(&c.plate)?;
            Some(sum / area - c.elevation.value())
        })
        .collect();
    gaps.sort_by(f64::total_cmp);
    assert!(gaps.len() > 100, "{} trench cells", gaps.len());
    let median = quantile(&gaps, 0.5);
    assert!((2_000.0..=4_000.0).contains(&median), "{median} m");
}

/// Another seed gives other convergent landforms on a mobile lid and other provinces on the
/// reference Mars: the along-strike, province and edifice draws read the seed (the determinism
/// audit's hardening).
#[test]
fn another_seed_gives_other_arcs_and_provinces() {
    let small = World::new(
        builder_of(&reference::earth_like())
            .figure(hyperion_surface::spheroid::Spheroid::sphere(2.0e6))
            .build()
            .unwrap(),
    );
    let fronts = |seed: SurfaceSeed| {
        let pass = small.pass(seed);
        let mut working = Working::initial(&pass);
        crate::planetary::surface::steps::plates::run(&pass, &mut working);
        let noise = noise::unit_field(
            &pass,
            noise::LANDFORM_INSTANCE,
            noise::LANDFORM_FIRST_LEVEL,
            pass.level().get(),
        );
        crust_populations(&pass, &mut working, &noise);
        convergent::run(&pass, &mut working)
            .unwrap()
            .front_m
            .into_iter()
            .flatten()
            .collect::<Vec<f64>>()
    };
    let (first, other) = (fronts(reference::SEED), fronts(SurfaceSeed::new(0x0dd5)));
    assert!(!first.is_empty() && !other.is_empty());
    assert!(
        first.len() != other.len()
            || first
                .iter()
                .zip(&other)
                .any(|(a, b)| a.total_cmp(b).is_ne())
    );
    let mars = World::new(reference::mars_like());
    let lift = |seed: SurfaceSeed| {
        let pass = mars.pass(seed);
        let mut working = Working::initial(&pass);
        lid_population(&pass, &mut working);
        provinces::run(&pass, &mut working).unwrap().lift
    };
    let (a, b) = (lift(reference::SEED), lift(SurfaceSeed::new(0x0dd5)));
    assert!(a.iter().zip(&b).any(|(x, y)| x.total_cmp(y).is_ne()));
}
