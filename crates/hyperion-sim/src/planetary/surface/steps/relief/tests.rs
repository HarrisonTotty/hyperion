//! R09.T12's tests of the coarse elevation (Design note 7; `decision-r09-t5.md` item 5 and
//! `decision-r09-t12-coarse.md`), written at the step's level, before R09.T12.e's affine scale.

use std::sync::OnceLock;

use hyperion_surface::field::{CoarseField, Crust, FieldView, coarse_level};
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

/// A world's state after the plates and this step, and that state quantised.
struct Relieved {
    world: World,
    relief: Working,
    field: CoarseField,
}

/// The plates and this step on `inputs` under the reference seed.
fn relieve(inputs: CoarseInputs) -> Relieved {
    let world = World::new(inputs);
    let pass = world.pass(reference::SEED);
    let mut relief = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut relief);
    run(&pass, &mut relief);
    let field = quantise(&world.inputs, relief.clone()).expect("the step's state quantises");
    Relieved {
        world,
        relief,
        field,
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

/// Goff, Smith and Marks 2004's Table 2 (Oceanography 17(1), 24–37), abyssal-hill RMS height H by
/// region and full spreading rate: northern MAR 235.8 m at 28 mm a⁻¹; an entry of 207.3 m at 35;
/// PAR slow 140 m and southern MAR 200.9 m at 40; PAR fast 116.0 m at 55; SEIR axial high 61.7 m,
/// intermediate 97.4 m and axial valley 169.8 m at 75; Pacific–Cocos 56.1 m at 110; Pacific–Nazca
/// 58.7 m at 160 (`decision-r09-t12-coarse.md` item 5a). The fit is held within 30% of the mean of
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
    assert_eq!(ponded_sediment(None), Metres::ZERO);
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
            assert_eq!(c.ponded_sediment, Metres::ZERO);
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

/// The continental population has Earth2014's mean and width, and its skew puts its mode at
/// Earth2014's +125 m; the oceanic crust's spread is plan 14's 0.93 km within the noise's
/// correlation with the floor.
#[test]
fn the_populations_have_their_widths() {
    let s = CONTINENTAL_SKEW;
    let mode = (math::exp(-s * s) - math::exp(0.5 * s * s))
        / (math::exp_m1(s * s) * math::exp(s * s)).sqrt();
    let mode_m = CONTINENTAL_MEAN.value() + CONTINENTAL_WIDTH.value() * mode;
    // Earth2014's land mode, +125 m.
    assert!((mode_m - 125.0).abs() < 0.5, "{mode_m} m");
    let earth = earth();
    let omega = earth.world.grid.solid_angles_sr();
    let z: Vec<f64> = earth
        .relief
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let land: Vec<bool> = earth
        .relief
        .cells
        .iter()
        .map(|c| c.crust == Crust::Continental)
        .collect();
    let (mean, spread) = subset_mean_and_spread(&z, omega, &land);
    assert!((mean - 405.0).abs() < 1e-6, "{mean} m");
    assert!((spread - 1_080.0).abs() < 1e-6, "{spread} m");
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
    let small = |regime: TectonicRegime| {
        builder_of(&reference::earth_like())
            .figure(hyperion_surface::spheroid::Spheroid::sphere(1.0e6))
            .tectonics(regime, 0.0)
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
    assert!(moon.relief.cells.iter().all(|c| c.crust == Crust::Lid
        && c.hill_relief.is_none()
        && c.water_surface == c.elevation));
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
    assert_ne!(first.cells[0].elevation, other.cells[0].elevation);
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
        if c.boundary != hyperion_surface::field::BoundaryKind::Divergent
            || c.boundary_distance.is_none_or(|d| d.value() != 0.0)
        {
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
                        && origin.ridge_distance.is_some_and(|d| d.value() == 0.0)
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
