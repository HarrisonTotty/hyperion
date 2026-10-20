//! R09.T13.d's tests: the normalisation, the guard, the lapse rate, the mean, the ices and the
//! liquids (the plan's amended list, decision-r09-t13d-normalisation's "Plan text" (a), with its
//! §1.4 specifications).

use hyperion_surface::field::{FieldView, MaterialPalette, PaletteEntry, PaletteRole};
use hyperion_surface::substance_key::SubstanceKey;

use super::super::ebm::{EnergyBalance, Freezing, ICE_FREE_ALBEDO, WATER_FREEZING_POINT_K};
use super::super::grid::{LATITUDE_BANDS, ModelGrid};
use super::super::normalise::{self, Correction, legendre_2, measure};
use super::super::{
    Diagnostics, airless, finish, floor_climate, lapse, normalised, point_sea_shares, run,
    run_diagnosed, seas,
};
use super::{builder_of, grids, set_ground, with_airless_contrasts};
use crate::math;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::hooks::surface::night_floor;
use crate::planetary::surface::grid::{CellGraph, parent_index};
use crate::planetary::surface::inputs::{
    AreaShare, ClimateRegime, CoarseInputs, Forcing, GasShare, SeasonalOrbit, Spin, ThermalRegime,
};
use crate::planetary::surface::quantise;
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::climate::coordinates::{Coordinates, substellar_cosine};
use crate::planetary::surface::steps::{CellState, Pass, Working};
use crate::units::consts::{SECONDS_PER_DAY, SOLAR_CONSTANT_W_PER_M2, STEFAN_BOLTZMANN};
use crate::units::{Kelvin, Metres, Pascals, Radians, Seconds, WattsPerSquareMetre};
use hyperion_surface::field::ClimateModelKind;

fn key(k: &str) -> SubstanceKey {
    SubstanceKey::new(k).unwrap()
}

/// The latitude of the body-frame unit vector `d`, degrees.
fn latitude_deg(d: [f64; 3]) -> f64 {
    math::asin(d[2].clamp(-1.0, 1.0)).to_degrees()
}

/// The field cells' area-weighted mean surface temperature of `working` on `grid`, K, the client's
/// view: each cell's climate cell's sea-level temperature less Γ above the sea level.
fn surface_mean_k(grid: &CellGraph, working: &Working) -> f64 {
    let (lapse, sea) = (working.lapse_rate_k_per_m, working.sea_level.value());
    let (sum, total) = (0_u32..)
        .zip(&working.cells)
        .zip(grid.solid_angles_sr())
        .fold((0.0, 0.0), |(s, t), ((n, cell), &omega)| {
            let parent = usize::try_from(parent_index(n)).unwrap();
            let temperature = working.climate[parent].sea_level_temperature.value()
                - lapse * (cell.water_surface.value() - sea);
            (s + omega * temperature, t + omega)
        });
    sum / total
}

/// The share of the surface each ice entry of `cells` covers, Σ Ω f ÷ Σ Ω, by entry.
fn ice_areas(grid: &CellGraph, cells: &[CellState]) -> Vec<(u8, f64)> {
    let total: f64 = grid.solid_angles_sr().iter().sum();
    let mut areas: Vec<(u8, f64)> = Vec::new();
    for (cell, &omega) in cells.iter().zip(grid.solid_angles_sr()) {
        if let Some(entry) = cell.ice_entry {
            match areas.iter_mut().find(|(e, _)| *e == entry) {
                Some((_, a)) => *a += omega * cell.ice_fraction / total,
                None => areas.push((entry, omega * cell.ice_fraction / total)),
            }
        }
    }
    areas
}

/// One cell's share of the surface at most, on `grid`.
fn one_cell(grid: &CellGraph) -> f64 {
    let total: f64 = grid.solid_angles_sr().iter().sum();
    grid.solid_angles_sr()
        .iter()
        .fold(0.0, |m: f64, &o| m.max(o))
        / total
}

/// Every climate record of `working` (each sea-level temperature, and each month's in a
/// twelve-month year) is at least `floor`.
fn every_record_at_least(working: &Working, floor: f64) -> bool {
    let months = usize::from(working.months);
    working.climate.iter().all(|c| {
        let t = c.sea_level_temperature.value();
        t >= floor - 1e-9
            && (months == 1
                || c.month_anomaly_k[..months]
                    .iter()
                    .all(|a| t + a >= floor - 1e-9))
    })
}

/// Runs the climate step on `inputs` at level `level` from the initial state: the working state
/// and what the step did.
fn run_at(inputs: &CoarseInputs, level: u8) -> (CellGraph, CellGraph, Working, Diagnostics) {
    let (field, climate) = grids(level);
    let pass = Pass::new(reference::SEED, inputs, &field, &climate);
    let mut working = Working::initial(&pass);
    let diagnostics = run_diagnosed(&pass, &mut working);
    (field, climate, working, diagnostics)
}

/// FILLET v1.1's ice-edge states (Barnes, Deitrick et al. 2025, arXiv:2511.11957, as the lane's
/// research-d §1 read it), over the placed perennial ice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IceState {
    /// No icy band.
    IceFree,
    /// Every band icy.
    Snowball,
    /// Ice that encompasses one pole or both, but not the other or not all.
    Caps { north: bool, south: bool },
    /// An ice sheet covering neither pole.
    Belt,
    /// Caps and a belt apart from them (Wilhelm et al. 2022's mixed state).
    CapsAndBelt,
}

/// The share of a 2.5° band under ice at which the band is icy: ½ (`Adopted`, the lane's
/// research-d §1.2: POISE's latitude is binary, `poise.c`; ½ puts the edge within one band).
const ICE_BAND_SHARE: f64 = 0.5;

/// The FILLET state of `cells` on `grid` by the zonal model's 72 bands of 2.5° (research-d §1.2's
/// classifier, which changes no output), and each hemisphere's least and greatest |latitude| of
/// ice, degrees, as band edges (FILLET v1.1's four edges; a belt across the equator reports 0 for
/// the northern least and the southern least).
fn ice_state(grid: &CellGraph, cells: &[CellState]) -> (IceState, [Option<(f64, f64)>; 2]) {
    let (mut ice, mut all) = ([0.0; LATITUDE_BANDS], [0.0; LATITUDE_BANDS]);
    for ((cell, centre), &omega) in cells.iter().zip(grid.centres()).zip(grid.solid_angles_sr()) {
        let band = ModelGrid::band_of(centre[2]);
        all[band] += omega;
        ice[band] += omega * cell.ice_fraction;
    }
    let icy: Vec<bool> = (0..LATITUDE_BANDS)
        .map(|k| all[k] > 0.0 && ice[k] / all[k] >= ICE_BAND_SHARE)
        .collect();
    let edge = |k: usize| -90.0 + 2.5 * f64::from(u32::try_from(k).unwrap());
    let hemisphere = |bands: core::ops::Range<usize>| {
        bands
            .filter(|&k| icy[k])
            .map(|k| {
                let (a, b) = (edge(k).abs(), edge(k + 1).abs());
                (a.min(b), a.max(b))
            })
            .reduce(|x, y| (x.0.min(y.0), x.1.max(y.1)))
    };
    let edges = [
        hemisphere(LATITUDE_BANDS / 2..LATITUDE_BANDS),
        hemisphere(0..LATITUDE_BANDS / 2),
    ];
    let (north, south) = (icy[LATITUDE_BANDS - 1], icy[0]);
    let state = if icy.iter().all(|&i| i) {
        IceState::Snowball
    } else if !icy.iter().any(|&i| i) {
        IceState::IceFree
    } else {
        // An icy run touching neither pole is a belt.
        let mut belt = false;
        let mut k = 0;
        while k < LATITUDE_BANDS {
            if icy[k] {
                let start = k;
                while k < LATITUDE_BANDS && icy[k] {
                    k += 1;
                }
                if start > 0 && k < LATITUDE_BANDS {
                    belt = true;
                }
            } else {
                k += 1;
            }
        }
        match (north || south, belt) {
            (true, true) => IceState::CapsAndBelt,
            (true, false) => IceState::Caps { north, south },
            (false, _) => IceState::Belt,
        }
    };
    (state, edges)
}

/// The plan's tests on the reference Earth: its field cells' area-weighted surface temperature is
/// plan 14's mean, to rounding in the working state and to 0.1 K in the quantised field the
/// client reads; its annual field's ends give its 42 K exactly; its ice area is plan 14's 0.041 to
/// one cell, caps at both poles, all of it poleward of 65° (the south's to 67.5°, its aphelion
/// winter the longer); and its header carries Γ, about 5.8 K km⁻¹. All land, as the field is until
/// R09.T12 (whose relief and seas move the mean's constant and the ice's cells).
#[test]
fn the_reference_earth_takes_its_mean_its_contrast_and_its_ice() {
    let inputs = reference::earth_like();
    let (field, climate_grid) = grids(8);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    let model = normalised(&pass, &working);
    let ends = measure::ends(&model.grid, model.coordinates, &model.climate.annual_k);
    assert!((ends - 42.0).abs() < 1e-9, "{ends}");
    let diagnostics = finish(&pass, &mut working, model);
    let mean = surface_mean_k(&field, &working);
    assert!((mean - 288.0).abs() < 1e-9, "{mean}");
    let quantised = quantise(&inputs, working.clone()).unwrap();
    let header = quantised.header();
    let (sum, total) = quantised
        .synthesis()
        .iter()
        .zip(field.solid_angles_sr())
        .enumerate()
        .fold((0.0, 0.0), |(s, t), (n, (cell, &omega))| {
            let parent = &quantised.climate_layer()[n >> 2];
            let temperature = header.sea_level_temperature(parent).value()
                - header.lapse_rate_k_per_m()
                    * (cell.water_surface().value() - header.sea_level().value());
            (s + omega * temperature, t + omega)
        });
    assert!((sum / total - 288.0).abs() < 0.1, "{}", sum / total);
    let gamma = lapse::lapse_rate_k_per_m(&inputs);
    hyperion_testkit::float::assert_same_bits(header.lapse_rate_k_per_m(), gamma);
    assert!((gamma * 1e3 - 5.8).abs() < 0.1);
    let areas = ice_areas(&field, &working.cells);
    assert_eq!(areas.len(), 1);
    assert!((areas[0].1 - 0.041).abs() <= one_cell(&field), "{areas:?}");
    for (cell, centre) in working.cells.iter().zip(field.centres()) {
        if cell.ice_fraction > 0.0 {
            assert!(latitude_deg(*centre).abs() > 65.0, "{centre:?}");
        }
    }
    let (state, edges) = ice_state(&field, &working.cells);
    assert_eq!(
        state,
        IceState::Caps {
            north: true,
            south: true
        }
    );
    eprintln!(
        "Earth: a {:.3} K, c {:.3} K, ice {:.5} in {} cells, edges {edges:?}, Γ {:.3} K km⁻¹, \
         clamped {}",
        diagnostics.correction.equator_pole_k,
        diagnostics.correction.mean_k,
        diagnostics.ices[0].placed,
        diagnostics.ices[0].cells,
        gamma * 1e3,
        diagnostics.clamped_values
    );
}

/// A Venus-like body for the isothermal model (the T13.c tests' own).
fn venus() -> CoarseInputs {
    super::other_regimes::venus_like()
}

/// The isothermal model takes plan 14's contrast, a = −⅔ of it on its uniform field, so that its
/// ends give it exactly, and its mean, after a lapse term that carries a hand relief (a continent
/// 3 km up over the hemisphere x > 0, dry, its water surface its ground as the relief step leaves
/// it): the field cells' surface mean is plan 14's to rounding, the lapse rate its carbon
/// dioxide's dry α g ÷ `c_p` at 737 K, 7.6 K km⁻¹ under the reference Earth's gravity that the
/// Venus-like body keeps (Venus's own 8.87 m s⁻² gives the ruling's 6.6,
/// decision-r09-t13a-relief §2.2), and the continent's area-weighted surface mean lies Γ × 3 km
/// below the lowlands' to 10⁻⁶ K, the sea-level field being the same over the two mirror-image
/// hemispheres. A locked twin takes its day–night contrast on its μ-weighted day and night
/// exactly, with b = ¾ of it to the locked grid's leakage, DN(cos γ) 4/3 to 8 × 10⁻⁵.
#[test]
fn the_isothermal_model_takes_plan_14_s_contrasts_and_mean() {
    let inputs = venus();
    let (field, climate_grid) = grids(6);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    let sea_level = working.sea_level;
    for (cell, centre) in working.cells.iter_mut().zip(field.centres()) {
        if centre[0] > 0.0 {
            set_ground(cell, Metres::new(3_000.0), sea_level, None);
        }
    }
    let model = normalised(&pass, &working);
    let ends = measure::ends(&model.grid, model.coordinates, &model.climate.annual_k);
    assert!((ends - 2.8).abs() < 1e-9, "{ends}");
    assert!((model.correction.equator_pole_k + 2.0 / 3.0 * 2.8).abs() < 1e-4);
    let _diagnostics = finish(&pass, &mut working, model);
    let gamma = working.lapse_rate_k_per_m;
    eprintln!("Venus: Γ {:.3} K km⁻¹", gamma * 1e3);
    assert!((gamma * 1e3 - 7.6).abs() < 0.2, "{gamma}");
    let mean = surface_mean_k(&field, &working);
    assert!((mean - 737.0).abs() < 1e-9, "{mean}");
    // The relief lowers the continent's surface by Γ × 3 km against the lowlands.
    let (mut low, mut high) = ([0.0_f64; 2], [0.0_f64; 2]);
    for ((n, cell), &omega) in (0_u32..).zip(&working.cells).zip(field.solid_angles_sr()) {
        let parent = usize::try_from(parent_index(n)).unwrap();
        let t = working.climate[parent].sea_level_temperature.value()
            - gamma * (cell.water_surface.value() - working.sea_level.value());
        let side = if cell.elevation.value() > 0.0 {
            &mut high
        } else {
            &mut low
        };
        side[0] += omega * t;
        side[1] += omega;
    }
    let (lowlands, continent) = (low[0] / low[1], high[0] / high[1]);
    eprintln!(
        "Venus: lowlands {lowlands:.9} K, continent {continent:.9} K, Γ × 3 km {:.9} K",
        gamma * 3_000.0
    );
    assert!(
        (lowlands - continent - gamma * 3_000.0).abs() < 1e-6,
        "the continent's surface mean {continent} K against the lowlands' {lowlands} K less \
         Γ × 3 km, {} K",
        gamma * 3_000.0
    );
    let locked = builder_of(&inputs)
        .spin(Spin {
            solar_day: None,
            ..*inputs.spin()
        })
        .climate(ClimateRegime {
            forcing: Forcing::Locked,
            ..*inputs.climate()
        })
        .day_night_contrast_k(5.0)
        .build()
        .unwrap();
    let pass = Pass::new(reference::SEED, &locked, &field, &climate_grid);
    let model = normalised(&pass, &Working::initial(&pass));
    assert_eq!(model.coordinates, Coordinates::TidallyLocked);
    let annual = &model.climate.annual_k;
    let day_night = measure::day_night(&model.grid, model.coordinates, annual);
    let ends = measure::ends(&model.grid, model.coordinates, annual);
    assert!((day_night - 5.0).abs() < 1e-9 && (ends - 2.8).abs() < 1e-9);
    let b = model.correction.day_night_k;
    eprintln!("Venus, locked: b {b:.9} K against {:.9} K", 0.75 * 5.0);
    // The locked grid's leakage, DN(cos γ) = 4/3 to 8 × 10⁻⁵, moves b by 7.9 × 10⁻⁵ of it as
    // built.
    assert!((b - 0.75 * 5.0).abs() < 0.75 * 5.0 * 1.5e-4, "{b}");
}

/// On fields shaped like P₂ and P₁ the normalisation is Design note 8's rule: a field
/// T₀ + T₂ P₂(sin φ) on the geographic grid comes out with its coefficient −⅔ of the input
/// contrast, and T₀ + T₁ cos γ on the locked grid with its own ¾ of the day–night contrast, to the
/// grids' leakage, each bound about twice it as built: E(P₂) is −3/2 to 2.04 × 10⁻⁶, which moves
/// the P₂ coefficient by 9 × 10⁻⁷ of the contrast, and DN(cos γ) 4/3 to 8 × 10⁻⁵, which moves the
/// P₁ coefficient by 6 × 10⁻⁵ of it; both conditions hold exactly in the measures.
#[test]
fn on_fields_shaped_like_p2_and_p1_the_coefficients_are_minus_two_thirds_and_three_quarters() {
    let earth = reference::earth_like();
    let grid = ModelGrid::geographic();
    let shaped: Vec<f64> = (0..grid.points())
        .map(|p| 280.0 - 20.0 * legendre_2(grid.centre_direction(p)[2]))
        .collect();
    let p2: Vec<f64> = (0..grid.points())
        .map(|p| legendre_2(grid.centre_direction(p)[2]))
        .collect();
    let leakage = measure::ends(&grid, Coordinates::Geographic, &p2) + 1.5;
    eprintln!("E(P₂) + 3/2 = {leakage:e}");
    // 2.04 × 10⁻⁶ as built.
    assert!(leakage.abs() < 3e-6, "{leakage}");
    let correction = normalise::arithmetic(&earth, &grid, Coordinates::Geographic, &shaped);
    let coefficient = -20.0 + correction.equator_pole_k;
    eprintln!(
        "P₂ coefficient {coefficient:.9} against {:.9}",
        -2.0 / 3.0 * 42.0
    );
    assert!(
        (coefficient + 2.0 / 3.0 * 42.0).abs() < 42.0 * 2e-6,
        "{coefficient}"
    );
    let corrected: Vec<f64> = (0..grid.points())
        .map(|p| shaped[p] + correction.at(grid.centre_direction(p)))
        .collect();
    let ends = measure::ends(&grid, Coordinates::Geographic, &corrected);
    assert!((ends - 42.0).abs() < 1e-9, "{ends}");
    let locked = builder_of(&earth)
        .climate(ClimateRegime {
            forcing: Forcing::Locked,
            model: ClimateModelKind::LockedEnergyBalance,
            ..*earth.climate()
        })
        .day_night_contrast_k(60.0)
        .equator_pole_contrast_k(0.0)
        .build()
        .unwrap();
    let grid = ModelGrid::locked();
    let to_body = |p: usize| Coordinates::TidallyLocked.to_body(grid.centre_direction(p));
    let shaped: Vec<f64> = (0..grid.points())
        .map(|p| 270.0 + 30.0 * substellar_cosine(to_body(p)))
        .collect();
    let correction = normalise::arithmetic(&locked, &grid, Coordinates::TidallyLocked, &shaped);
    let coefficient = 30.0 + correction.day_night_k;
    eprintln!("P₁ coefficient {coefficient:.6} against {:.6}", 0.75 * 60.0);
    assert!(
        (coefficient - 0.75 * 60.0).abs() < 60.0 * 1e-4,
        "{coefficient}"
    );
    let corrected: Vec<f64> = (0..grid.points())
        .map(|p| shaped[p] + correction.at(to_body(p)))
        .collect();
    let day_night = measure::day_night(&grid, Coordinates::TidallyLocked, &corrected);
    let ends = measure::ends(&grid, Coordinates::TidallyLocked, &corrected);
    assert!((day_night - 60.0).abs() < 1e-9, "{day_night}");
    assert!(ends.abs() < 1e-9, "{ends}");
}

/// A locked world's μ-weighted day and night give its day–night contrast exactly, and its ends
/// its equator–pole contrast, on the locked energy-balance model (the reference Earth locked 1:1
/// on a 24-day circular, untilted orbit, one month).
#[test]
fn a_locked_world_s_day_and_night_give_its_contrast_exactly() {
    let earth = super::locked_earth(0.0, 0.0);
    let inputs = builder_of(&earth)
        .day_night_contrast_k(40.0)
        .build()
        .unwrap();
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let model = normalised(&pass, &Working::initial(&pass));
    let annual = &model.climate.annual_k;
    let day_night = measure::day_night(&model.grid, model.coordinates, annual);
    let ends = measure::ends(&model.grid, model.coordinates, annual);
    eprintln!(
        "locked Earth: a {:.3} K, b {:.3} K",
        model.correction.equator_pole_k, model.correction.day_night_k
    );
    assert!((day_night - 40.0).abs() < 1e-9, "{day_night}");
    assert!((ends - 42.0).abs() < 1e-9, "{ends}");
}

/// The radiative model's brightness mean of its moments is plan 14's to 10⁻⁶ K, and its ends and
/// (locked) day–night give its contrasts to the same, on the reference Moon and Mars and on locked
/// Moons; and a Moon-like airless input under plan 14's own airless contrasts
/// ([`with_airless_contrasts`]), turning or locked under a still sun, takes a correction under
/// 0.5 K at every grid point, since its field holds plan 14's physics already
/// (decision-r09-t13d-normalisation §2.2). The locked Moon on its own orbit, whose sun librates by
/// ±2e and nods by its 1.5°, takes up to 1.6 K: its terminator's light lifts its night's
/// brightness above plan 14's floor, which its fixed eyeball leaves dark (R09's Risks).
#[test]
fn the_radiative_model_s_moments_give_the_input_s_figures() {
    let moon = reference::moon_like();
    let year = moon.seasonal_orbit().unwrap().period;
    let still = with_airless_contrasts(
        &builder_of(&moon)
            .spin(Spin {
                obliquity: Radians::ZERO,
                rotation_period: year,
                solar_day: None,
                equinox_true_anomaly: Radians::ZERO,
            })
            .seasonal_orbit(SeasonalOrbit {
                eccentricity: 0.0,
                period: year,
            })
            .climate(ClimateRegime {
                forcing: Forcing::Locked,
                ..*moon.climate()
            })
            .build()
            .unwrap(),
    );
    let worlds = [
        ("Moon", moon, false),
        ("Mars", reference::mars_like(), false),
        (
            "Moon-like",
            with_airless_contrasts(&reference::moon_like()),
            true,
        ),
        ("locked Moon, still", still, true),
        (
            "locked Moon, librating",
            super::other_regimes::locked_moon(),
            false,
        ),
    ];
    for (name, inputs, plan_14) in worlds {
        let solution = airless::solve(&inputs);
        let (correction, residuals) = normalise::radiative(&inputs, &solution);
        let [mean, ends, day_night] = measure::radiative(&inputs, &solution, &correction);
        let largest = (0..solution.grid.points()).fold(0.0_f64, |m, p| {
            let d = solution
                .coordinates
                .to_body(solution.grid.centre_direction(p));
            m.max(correction.at(d).abs())
        });
        eprintln!(
            "{name}: c {:.4} K, a {:.4} K, b {:.4} K, largest {largest:.4} K, residuals \
             {residuals:?}",
            correction.mean_k, correction.equator_pole_k, correction.day_night_k
        );
        let t = inputs.mean_surface_temperature().value();
        assert!((mean - t).abs() < 1e-6, "{name}: {mean}");
        assert!(
            (ends - inputs.equator_pole_contrast_k()).abs() < 1e-6,
            "{name}: {ends}"
        );
        if inputs.climate().forcing == Forcing::Locked {
            assert!(
                (day_night - inputs.day_night_contrast_k()).abs() < 1e-6,
                "{name}: {day_night}"
            );
        }
        if plan_14 {
            assert!(largest < 0.5, "{name}: {largest} K");
        }
    }
}

/// The locked lava world of the T13.c tests under the reference Moon's placeholder contrasts,
/// 120 K and 200 K, which no field of its physics meets: the solve keeps every sample in the
/// physical range and the guard holds every interpolated value to its stencil's, its clamps
/// counted, every record at the night floor or above, and nothing panics; under plan 14's own
/// contrasts its correction is under 1 K, the floor holding its night against the part of it
/// that is negative there.
#[test]
fn the_guard_holds_a_locked_lava_world_at_its_floor() {
    let (placeholders, _) = super::other_regimes::lava_world();
    for (name, inputs) in [
        ("placeholders", placeholders.clone()),
        ("plan 14's", with_airless_contrasts(&placeholders)),
    ] {
        let (_, _, working, diagnostics) = run_at(&inputs, 6);
        eprintln!(
            "lava world, {name}: c {:.2} K, a {:.2} K, b {:.2} K, residuals {:?}, floored {} \
             points and {} records, clamped {} values",
            diagnostics.correction.mean_k,
            diagnostics.correction.equator_pole_k,
            diagnostics.correction.day_night_k,
            diagnostics.residuals_k,
            diagnostics.floored_points,
            diagnostics.floored_records,
            diagnostics.clamped_values
        );
        assert!(every_record_at_least(&working, diagnostics.lower_bound_k));
        let floor = night_floor(inputs.heat_flow()).value();
        hyperion_testkit::float::assert_same_bits(diagnostics.lower_bound_k, floor);
        let c = diagnostics.correction;
        if name == "plan 14's" {
            // The night sits at the floor, so the night side's share of a negative shift is the
            // guard's.
            for k in [c.mean_k, c.equator_pole_k, c.day_night_k] {
                assert!(k.abs() < 1.0, "{c:?}");
            }
        } else {
            assert!(diagnostics.clamped_values > 0);
        }
    }
}

/// No record lies below its model's lower bound on any reference world, each through the step
/// alone at its own level; and the corrections and the ices placed, printed (R09's Risks record
/// them). The reference Moon's and Ceres's contrasts are placeholders, not plan 14's airless
/// figures, so their corrections are tens of kelvin.
#[test]
fn no_record_lies_below_the_model_s_lower_bound_on_any_reference_world() {
    for (name, inputs, level) in [
        ("Earth", reference::earth_like(), 6),
        ("Mars", reference::mars_like(), 7),
        ("Moon", reference::moon_like(), 6),
        ("Ceres", reference::ceres_like(), 5),
    ] {
        let (_, _, working, diagnostics) = run_at(&inputs, level);
        let ices: Vec<String> = diagnostics
            .ices
            .iter()
            .map(|ice| {
                format!(
                    "{} {:.5} of {:.5} in {} cells ({} above {:.2} K)",
                    ice.substance.as_str(),
                    ice.placed,
                    ice.target,
                    ice.cells,
                    ice.above_frost_point,
                    ice.frost_point_k
                )
            })
            .collect();
        eprintln!(
            "{name}: a {:.3} K, b {:.3} K, c {:.3} K, residuals {:?}, lower bound {:.2} K, \
             floored {} + {}, clamped {}; ices {ices:?}",
            diagnostics.correction.equator_pole_k,
            diagnostics.correction.day_night_k,
            diagnostics.correction.mean_k,
            diagnostics.residuals_k,
            diagnostics.lower_bound_k,
            diagnostics.floored_points,
            diagnostics.floored_records,
            diagnostics.clamped_values
        );
        assert!(
            every_record_at_least(&working, diagnostics.lower_bound_k),
            "{name}"
        );
    }
}

/// Water's freezing-point shift is nothing: the reference Earth's model turns about T13.a's
/// 273.15 K, its albedo and heat capacity the same to the bit; Mars's carbon dioxide under 636 Pa
/// moves them to its frost point, 125.5 K colder; a world with no condensable takes the flat
/// albedo and an open sea's capacity at every temperature.
#[test]
fn water_s_freezing_point_shift_is_nothing() {
    let earth = EnergyBalance::for_inputs(&reference::earth_like());
    let Freezing::At { point_k } = earth.freezing else {
        panic!("the reference Earth's ice forms, under the ice albedo: {earth:?}");
    };
    hyperion_testkit::float::assert_same_bits(point_k, WATER_FREEZING_POINT_K);
    for t in [200.0, 263.15, 268.15, 270.0, 273.15, 290.0] {
        let ramp = math::tanh((t - 268.15) / 5.0);
        hyperion_testkit::float::assert_same_bits(earth.albedo_at(t), 0.525 - 0.245 * ramp);
    }
    hyperion_testkit::float::assert_same_bits(earth.heat_capacity(273.15, 1.0), 40.0 * 5.25e6);
    hyperion_testkit::float::assert_same_bits(earth.heat_capacity(270.0, 1.0), 9.2 * 5.25e6);
    hyperion_testkit::float::assert_same_bits(earth.heat_capacity(263.15, 1.0), 2.0 * 5.25e6);
    let mars = EnergyBalance::for_inputs(&reference::mars_like());
    let shifted = mars.freezing.point_k().unwrap();
    assert!((shifted - (273.15 - 125.5)).abs() < 0.3, "{shifted}");
    let dry = builder_of(&reference::earth_like())
        .climate(ClimateRegime {
            condensable: None,
            ..*reference::earth_like().climate()
        })
        .build()
        .unwrap();
    let dry = EnergyBalance::for_inputs(&dry);
    let Freezing::Never { albedo } = dry.freezing else {
        panic!("a world with no condensable forms no ice: {dry:?}");
    };
    hyperion_testkit::float::assert_same_bits(albedo, ICE_FREE_ALBEDO);
    assert!((dry.albedo_at(150.0) - 0.28).abs() < 1e-15);
    hyperion_testkit::float::assert_same_bits(dry.heat_capacity(150.0, 1.0), 40.0 * 5.25e6);
}

/// The reference Mars (its ices given by hand: water 0.007, carbon dioxide 0.0006) places its
/// carbon dioxide first, at its 147.7 K frost point against water's 273.16 K melting line, each
/// to its area to one cell, no cell holding both, every carbon dioxide cell poleward of the water
/// ice's equatorward edge in its own hemisphere, and every iced cell poleward of 75°, which the
/// warmest month as a key fails (decision-r09-t13d-normalisation §1.4).
#[test]
fn the_reference_mars_places_its_carbon_dioxide_first_inside_its_water_ice() {
    let inputs = reference::mars_like();
    let (field, _, working, diagnostics) = run_at(&inputs, 7);
    let order: Vec<&str> = diagnostics
        .ices
        .iter()
        .map(|ice| ice.substance.as_str())
        .collect();
    assert_eq!(order, ["CO2", "H2O"]);
    let palette = inputs.palette();
    let (co2, water) = (
        palette.find(PaletteRole::Ice, key("CO2")).unwrap(),
        palette.find(PaletteRole::Ice, key("H2O")).unwrap(),
    );
    let areas = ice_areas(&field, &working.cells);
    for (entry, target) in [(co2, 0.0006), (water, 0.007)] {
        let area = areas.iter().find(|(e, _)| *e == entry).unwrap().1;
        assert!((area - target).abs() <= one_cell(&field), "{entry}: {area}");
    }
    let mut water_edge = [f64::INFINITY; 2];
    for (cell, centre) in working.cells.iter().zip(field.centres()) {
        let latitude = latitude_deg(*centre);
        if cell.ice_fraction > 0.0 {
            assert!(cell.ice_fraction <= 1.0);
            assert!(latitude.abs() > 75.0, "ice at {latitude}°");
        }
        if cell.ice_entry == Some(water) && cell.ice_fraction > 0.0 {
            let h = usize::from(latitude < 0.0);
            water_edge[h] = water_edge[h].min(latitude.abs());
        }
    }
    let mut hemispheres = [0_u32; 2];
    for (cell, centre) in working.cells.iter().zip(field.centres()) {
        if cell.ice_entry == Some(co2) && cell.ice_fraction > 0.0 {
            let latitude = latitude_deg(*centre);
            let h = usize::from(latitude < 0.0);
            hemispheres[h] += 1;
            assert!(
                latitude.abs() >= water_edge[h],
                "CO₂ at {latitude}° outside water's edge {}°",
                water_edge[h]
            );
        }
    }
    eprintln!(
        "Mars: CO₂ cells north {}, south {}; water's equatorward edges {water_edge:.2?}°; {:?}",
        hemispheres[0], hemispheres[1], diagnostics.ices
    );
}

/// A 90°-obliquity world, all land at an Earth-like mean (the reference Earth at 90° on a
/// circular orbit, its contrast plan 14's sign there, warm poles, −20 K by hand, its water ice
/// 0.25 by hand), whose history plan 14 says started cold (`Forcing::IceBelt`), is labelled an
/// ice belt by FILLET's states: across the equator, edges at ±asin 0.25 = ±14.48° to one band,
/// both polar bands ice-free (a warm pole gets no cap). Its warm-start twin
/// (`Forcing::Seasonal`) places the same cells, since plan 14's cover reads no history.
#[test]
fn a_world_at_90_degrees_is_an_ice_belt_and_its_warm_start_twin_places_the_same_cells() {
    let earth = reference::earth_like();
    let tilted = |forcing: Forcing| {
        builder_of(&earth)
            .spin(Spin {
                obliquity: Radians::new(core::f64::consts::FRAC_PI_2),
                equinox_true_anomaly: Radians::ZERO,
                ..*earth.spin()
            })
            .seasonal_orbit(SeasonalOrbit {
                eccentricity: 0.0,
                period: earth.seasonal_orbit().unwrap().period,
            })
            .equator_pole_contrast_k(-20.0)
            .ices([AreaShare {
                substance: key("H2O"),
                area_fraction: 0.25,
            }])
            .climate(ClimateRegime {
                forcing,
                ..*earth.climate()
            })
            .build()
            .unwrap()
    };
    let (field, _, cold, diagnostics) = run_at(&tilted(Forcing::IceBelt), 6);
    let (state, [north, south]) = ice_state(&field, &cold.cells);
    eprintln!(
        "90°: a {:.2} K, c {:.2} K; {state:?}, north {north:?}, south {south:?}; {:?}",
        diagnostics.correction.equator_pole_k, diagnostics.correction.mean_k, diagnostics.ices
    );
    assert_eq!(state, IceState::Belt);
    let edge = math::asin(0.25).to_degrees();
    for (least, greatest) in [north.unwrap(), south.unwrap()] {
        assert!(least.abs() < 1e-12, "{least}");
        assert!((greatest - edge).abs() <= 2.5, "{greatest}");
    }
    let areas = ice_areas(&field, &cold.cells);
    assert!((areas[0].1 - 0.25).abs() <= one_cell(&field));
    let (_, _, warm, _) = run_at(&tilted(Forcing::Seasonal), 6);
    for (a, b) in cold.cells.iter().zip(&warm.cells) {
        hyperion_testkit::float::assert_same_bits(a.ice_fraction, b.ice_fraction);
        assert_eq!(a.ice_entry, b.ice_entry);
    }
}

/// A warm pole gets no cap: a Moon-like airless world at 80° of obliquity, whose poles take more
/// of the year's light than its equator, under plan 14's own contrasts (negative, warm poles),
/// places a water ice given by hand (0.05) about its equator, none of it poleward of 45°.
#[test]
fn a_warm_pole_gets_no_cap() {
    let moon = reference::moon_like();
    let inputs = with_airless_contrasts(
        &builder_of(&moon)
            .spin(Spin {
                obliquity: Radians::new(80_f64.to_radians()),
                ..*moon.spin()
            })
            .ices([AreaShare {
                substance: key("H2O"),
                area_fraction: 0.05,
            }])
            .build()
            .unwrap(),
    );
    assert!(inputs.equator_pole_contrast_k() < 0.0);
    let (field, _, working, diagnostics) = run_at(&inputs, 6);
    let (state, edges) = ice_state(&field, &working.cells);
    eprintln!("80°: {state:?} {edges:?}; {:?}", diagnostics.ices);
    for (cell, centre) in working.cells.iter().zip(field.centres()) {
        if cell.ice_fraction > 0.0 {
            assert!(latitude_deg(*centre).abs() < 45.0, "{centre:?}");
        }
    }
    assert!(!matches!(
        state,
        IceState::Caps { .. } | IceState::CapsAndBelt
    ));
}

/// The ruling's recommended case (decision-r09-t13d-normalisation §1.4): a Pluto-like airless
/// world (60° of obliquity on a circular orbit at 39.5 au, plan 14's airless ice, its nitrogen ice
/// given by hand, 0.05) places its ice between 15° and 40° of latitude, neither at the poles nor at
/// the equator, where its annual radiative mean is least (Hamilton et al. 2016, Nature 540, 97: ice
/// "quickly accumulates on Pluto near latitudes of 30 degrees north and south", as the ruling read
/// it). Its year, 248 years, its day of 6.387 days and its 39.5 au are from memory.
#[test]
fn a_pluto_like_world_ices_its_middle_latitudes() {
    let moon = reference::moon_like();
    let au = 39.5;
    let flux = WattsPerSquareMetre::new(SOLAR_CONSTANT_W_PER_M2 / (au * au));
    let albedo = SurfaceState::Airless.albedo(SurfaceMaterial::Ice).value();
    let mean = Kelvin::new(
        ((1.0 - albedo) * flux.value() / (4.0 * STEFAN_BOLTZMANN))
            .sqrt()
            .sqrt(),
    );
    let year = Seconds::new(248.0 * 365.25 * SECONDS_PER_DAY);
    let day = Seconds::new(6.387 * SECONDS_PER_DAY);
    let mut entries: Vec<PaletteEntry> = moon.palette().entries().to_vec();
    let water_ice = entries
        .iter()
        .find(|e| e.role == PaletteRole::Ice)
        .unwrap()
        .clone();
    // A stand-in entry for the test: nitrogen ice at its triple point, 63.15 K, its density about
    // 1,027 kg m⁻³ near 40 K (from memory), the water ice's albedo and mechanics.
    entries.push(PaletteEntry {
        substance: key("N2"),
        transition: Kelvin::new(63.15),
        grain_density_kg_m3: 1_027.0,
        ..water_ice
    });
    entries.sort_by_key(|e| (e.role, e.substance));
    let inputs = with_airless_contrasts(
        &builder_of(&moon)
            .spin(Spin {
                obliquity: Radians::new(60_f64.to_radians()),
                rotation_period: day,
                solar_day: Some(day),
                equinox_true_anomaly: Radians::ZERO,
            })
            .seasonal_orbit(SeasonalOrbit {
                eccentricity: 0.0,
                period: year,
            })
            .host_flux(flux)
            .mean_surface_temperature(mean)
            .surface(SurfaceState::Airless, SurfaceMaterial::Ice)
            .ices([AreaShare {
                substance: key("N2"),
                area_fraction: 0.05,
            }])
            .palette(MaterialPalette::new(entries).unwrap())
            .build()
            .unwrap(),
    );
    let (field, _, working, diagnostics) = run_at(&inputs, 6);
    let (state, edges) = ice_state(&field, &working.cells);
    eprintln!(
        "Pluto-like: T_eq {:.2} K, contrast {:.3} K; {state:?} {edges:?}; c {:.4} a {:.4}; {:?}",
        mean.value(),
        inputs.equator_pole_contrast_k(),
        diagnostics.correction.mean_k,
        diagnostics.correction.equator_pole_k,
        diagnostics.ices
    );
    let (mut least, mut greatest) = (90.0_f64, 0.0_f64);
    for (cell, centre) in working.cells.iter().zip(field.centres()) {
        if cell.ice_fraction > 0.0 {
            let latitude = latitude_deg(*centre).abs();
            least = least.min(latitude);
            greatest = greatest.max(latitude);
        }
    }
    assert!(least >= 15.0 && greatest <= 40.0, "{least}–{greatest}°");
}

/// The Titan-like world of [`a_titan_like_world_s_methane_fills_its_basins_to_its_share`].
pub(super) fn titan_like() -> CoarseInputs {
    let earth = reference::earth_like();
    let mut entries: Vec<PaletteEntry> = earth.palette().entries().to_vec();
    let water = entries
        .iter()
        .find(|e| e.role == PaletteRole::Liquid)
        .unwrap()
        .clone();
    for (k, transition) in [("CH4", 90.69), ("C2H6", 90.37)] {
        // Stand-ins for the test: the liquid's triple point (from memory), with the water's record,
        // its density and its mechanics.
        entries.push(PaletteEntry {
            substance: key(k),
            transition: Kelvin::new(transition),
            ..water.clone()
        });
    }
    entries.sort_by_key(|e| (e.role, e.substance));
    let day = Seconds::new(15.945 * SECONDS_PER_DAY);
    builder_of(&earth)
        .liquids([
            AreaShare {
                substance: key("CH4"),
                area_fraction: 0.006,
            },
            AreaShare {
                substance: key("C2H6"),
                area_fraction: 0.002,
            },
        ])
        .ices([])
        .condensates([])
        .mean_surface_temperature(Kelvin::new(93.7))
        .equator_pole_contrast_k(3.0)
        .surface_pressure(Pascals::new(146_700.0))
        .gases([
            GasShare {
                substance: key("N2"),
                mole_fraction: 0.95,
            },
            GasShare {
                substance: key("CH4"),
                mole_fraction: 0.05,
            },
        ])
        .spin(Spin {
            obliquity: Radians::ZERO,
            rotation_period: day,
            solar_day: Some(day),
            equinox_true_anomaly: Radians::ZERO,
        })
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: 0.0,
            period: Seconds::new(29.46 * 365.25 * SECONDS_PER_DAY),
        })
        .host_flux(WattsPerSquareMetre::new(
            SOLAR_CONSTANT_W_PER_M2 / (9.58 * 9.58),
        ))
        .climate(ClimateRegime {
            thermal: ThermalRegime::Efficient,
            forcing: Forcing::Seasonal,
            condensable: Some(key("CH4")),
            model: ClimateModelKind::EnergyBalance,
        })
        .palette(MaterialPalette::new(entries).unwrap())
        .build()
        .unwrap()
}

/// The Titan-like world of the liquids' tests at level `level`, its relief and flood set by hand
/// (a synthetic mask: three basins at 80° N and one at 40° S, each 5° in radius, 100 m under the
/// sea level and flooded to it with the main liquid, methane, all else 100 m above and dry,
/// [`set_ground`]), from the initial state.
pub(super) fn titan_with_basins(level: u8) -> (CellGraph, CellGraph, CoarseInputs, Working) {
    let inputs = titan_like();
    let (field, climate_grid) = grids(level);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    let basin = |d: [f64; 3]| {
        let centres = [(80.0, 0.0), (80.0, 120.0), (80.0, 240.0), (-40.0, 60.0)];
        centres.iter().position(|&(lat, lon): &(f64, f64)| {
            let (s, c) = math::sin_cos(lat.to_radians());
            let (sl, cl) = math::sin_cos(lon.to_radians());
            let centre = [c * cl, c * sl, s];
            let dot = d[0] * centre[0] + d[1] * centre[1] + d[2] * centre[2];
            dot > math::cos(5_f64.to_radians())
        })
    };
    let flood = inputs.main_liquid();
    for (cell, centre) in working.cells.iter_mut().zip(field.centres()) {
        let wet = basin(*centre).is_some();
        let elevation = Metres::new(if wet { -100.0 } else { 100.0 });
        set_ground(cell, elevation, Metres::ZERO, flood);
    }
    working.sea_level = Metres::ZERO;
    (field, climate_grid, inputs, working)
}

/// A Titan-like world's methane liquid fills its basins to its share: the reference Earth's
/// figure under Titan's air (1.467 bar, about 5% methane, from memory) at Titan's 93.7 K mean and
/// light (Saturn's 9.58 au), untilted on a circular orbit (one month, the steady state), its
/// four basins a hand-set flood ([`titan_with_basins`]; R09.T14's lakes would supply such
/// basins), its liquids methane 0.006 and ethane 0.002 (their palette records the water's,
/// stand-ins). Every flooded cell names a liquid, its water surface the sea level as the flood
/// left it, and no dry cell does; each liquid's share of the flooded area is its own to one cell;
/// methane, the more volatile, lies in the colder basins.
#[test]
fn a_titan_like_world_s_methane_fills_its_basins_to_its_share() {
    let (field, climate_grid, inputs, mut working) = titan_with_basins(8);
    assert_eq!(inputs.months(), 1);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let diagnostics = run_diagnosed(&pass, &mut working);
    eprintln!("Titan-like: {:?}", diagnostics.liquids);
    let palette = inputs.palette();
    let (methane, ethane) = (
        palette.find(PaletteRole::Liquid, key("CH4")).unwrap(),
        palette.find(PaletteRole::Liquid, key("C2H6")).unwrap(),
    );
    let (mut sea, mut by_liquid) = (0.0, [0.0_f64; 2]);
    let mut temperature = [(0.0_f64, 0.0_f64); 2];
    for ((n, cell), &omega) in (0_u32..).zip(&working.cells).zip(field.solid_angles_sr()) {
        if cell.elevation.value() < 0.0 {
            assert!(seas::under_sea(cell));
            sea += omega;
            hyperion_testkit::float::assert_same_bits(cell.water_surface.value(), 0.0);
            let k = if cell.liquid_entry == Some(methane) {
                0
            } else {
                assert_eq!(cell.liquid_entry, Some(ethane));
                1
            };
            by_liquid[k] += omega;
            let t = working.climate[usize::try_from(parent_index(n)).unwrap()]
                .sea_level_temperature
                .value();
            temperature[k].0 += omega * t;
            temperature[k].1 += omega;
        } else {
            assert_eq!(cell.liquid_entry, None);
            hyperion_testkit::float::assert_same_bits(
                cell.water_surface.value(),
                cell.elevation.value(),
            );
        }
    }
    let total: f64 = field.solid_angles_sr().iter().sum();
    let cell = one_cell(&field) * total;
    assert!(
        (by_liquid[0] - 0.75 * sea).abs() <= cell,
        "{by_liquid:?} of {sea}"
    );
    assert!(
        (by_liquid[1] - 0.25 * sea).abs() <= cell,
        "{by_liquid:?} of {sea}"
    );
    let [methane_k, ethane_k] = temperature.map(|(s, w)| s / w);
    eprintln!("methane's basins {methane_k:.3} K, ethane's {ethane_k:.3} K");
    assert!(methane_k < ethane_k);
    // The quantiser takes the sea.
    let field_out = quantise(&inputs, working).unwrap();
    assert!(
        field_out
            .synthesis()
            .iter()
            .any(|c| c.liquid_entry() == Some(methane))
    );
}

/// The correction's shape is P₂(sin φ) and P₁(cos γ) at the body frame's directions, with its
/// constant.
#[test]
fn the_correction_is_its_constant_and_its_two_shapes() {
    let correction = Correction {
        mean_k: 1.0,
        equator_pole_k: 2.0,
        day_night_k: 3.0,
    };
    hyperion_testkit::float::assert_same_bits(correction.at([0.0, 0.0, 1.0]), 3.0);
    hyperion_testkit::float::assert_same_bits(correction.at([1.0, 0.0, 0.0]), 1.0 - 1.0 + 3.0);
    hyperion_testkit::float::assert_same_bits(correction.at([-1.0, 0.0, 0.0]), 1.0 - 1.0 - 3.0);
}

/// The guard after the mean's constant (`floor_climate`): a climate cell whose annual mean and
/// some months lie below the bound holds its mean at the bound and re-bases its months' anomalies
/// on it, each month held at the bound or kept; a cell with one month below keeps its mean and
/// re-bases that month alone; a cell whose records all lie at or above the bound is left to the
/// bit; and the count is the records raised.
#[test]
fn the_guard_holds_a_cell_s_mean_and_re_bases_its_anomalies() {
    use hyperion_testkit::float::assert_same_bits;
    let inputs = reference::earth_like();
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    assert_eq!(working.months, 12);
    let mut cold = [0.0; 12];
    cold[..4].copy_from_slice(&[-10.0, 5.0, 20.0, -30.0]);
    let mut one_low = [0.0; 12];
    one_low[..2].copy_from_slice(&[-100.0, 10.0]);
    working.climate[0].sea_level_temperature = Kelvin::new(100.0);
    working.climate[0].month_anomaly_k = cold;
    working.climate[1].sea_level_temperature = Kelvin::new(200.0);
    working.climate[1].month_anomaly_k = one_low;
    let untouched = working.climate[2];
    let raised = floor_climate(&mut working, 110.0);
    // Cell 0's mean and eleven of its months (90, 105, 70 K and eight at 100 K), and one of cell
    // 1's (100 K).
    assert_eq!(raised, 13);
    let mut held = [0.0; 12];
    held[2] = 10.0;
    let mut kept = [0.0; 12];
    kept[..2].copy_from_slice(&[-90.0, 10.0]);
    for (state, (mean, anomalies)) in working.climate[..2]
        .iter()
        .zip([(110.0, held), (200.0, kept)])
    {
        assert_same_bits(state.sea_level_temperature.value(), mean);
        for (&a, e) in state.month_anomaly_k.iter().zip(anomalies) {
            assert_same_bits(a, e);
        }
    }
    let after = working.climate[2];
    assert_same_bits(
        after.sea_level_temperature.value(),
        untouched.sea_level_temperature.value(),
    );
    for (&a, &b) in after.month_anomaly_k.iter().zip(&untouched.month_anomaly_k) {
        assert_same_bits(a, b);
    }
}

/// A world with no liquid whose ground lies below its sea level stays dry: the relief step
/// floods nothing there (decision-r09-t12-coarse R8), and the climate step reads only its flood.
/// The reference Earth with its liquids taken away and its northern hemisphere 100 m under the
/// sea level, dry, takes no sea share on the geographic model's grid, and after the step every
/// cell's water surface is its ground, to the bit, with no liquid entry, and no liquid is placed.
#[test]
fn a_world_with_no_liquid_and_ground_below_its_sea_level_stays_dry() {
    use hyperion_testkit::float::assert_same_bits;
    let inputs = builder_of(&reference::earth_like())
        .liquids([])
        .build()
        .unwrap();
    assert_eq!(inputs.main_liquid(), None);
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    for (cell, centre) in working.cells.iter_mut().zip(field.centres()) {
        let elevation = Metres::new(if centre[2] > 0.0 { -100.0 } else { 100.0 });
        set_ground(cell, elevation, Metres::ZERO, inputs.main_liquid());
    }
    working.sea_level = Metres::ZERO;
    let grid = ModelGrid::geographic();
    for share in point_sea_shares(&pass, &working, &grid, Coordinates::Geographic) {
        assert_same_bits(share, 0.0);
    }
    let diagnostics = run_diagnosed(&pass, &mut working);
    assert_eq!(diagnostics.liquids, []);
    for cell in &working.cells {
        assert!(!seas::under_sea(cell));
        assert_eq!(cell.liquid_entry, None);
        assert_same_bits(cell.water_surface.value(), cell.elevation.value());
    }
}

/// Every cell field and climate record of `b` is `a`'s, bit for bit, and so are the figures the
/// climate step writes beside them, on the world `name`.
fn assert_same_working(name: &str, a: &Working, b: &Working) {
    use hyperion_testkit::float::assert_same_bits;
    let bits = |m: Option<Metres>| m.map(|m| hyperion_testkit::float::bits(m.value()));
    assert_same_bits(a.lapse_rate_k_per_m, b.lapse_rate_k_per_m);
    assert_same_bits(a.sea_level.value(), b.sea_level.value());
    assert_eq!(a.months, b.months, "{name}");
    assert_eq!(a.climate.len(), b.climate.len(), "{name}");
    for (x, y) in a.climate.iter().zip(&b.climate) {
        assert_same_bits(
            x.sea_level_temperature.value(),
            y.sea_level_temperature.value(),
        );
        for (p, q) in x.month_anomaly_k.iter().zip(&y.month_anomaly_k) {
            assert_same_bits(*p, *q);
        }
        for (p, q) in x
            .month_precipitation_kg_m2_s
            .iter()
            .zip(&y.month_precipitation_kg_m2_s)
        {
            assert_same_bits(*p, *q);
        }
        for (p, q) in x.month_wind.iter().zip(&y.month_wind) {
            assert_same_bits(p.speed.value(), q.speed.value());
            assert_same_bits(p.azimuth.value(), q.azimuth.value());
        }
    }
    assert_eq!(a.cells.len(), b.cells.len(), "{name}");
    for (n, (x, y)) in a.cells.iter().zip(&b.cells).enumerate() {
        for (p, q) in [
            (x.elevation.value(), y.elevation.value()),
            (x.water_surface.value(), y.water_surface.value()),
            (x.boundary_obliquity.value(), y.boundary_obliquity.value()),
            (x.drainage.value(), y.drainage.value()),
            (x.steepness_index_m0_9, y.steepness_index_m0_9),
            (x.runoff_m_per_year, y.runoff_m_per_year),
            (x.catchment_runoff_m_per_year, y.catchment_runoff_m_per_year),
            (x.ice_fraction, y.ice_fraction),
            (x.ponded_sediment.value(), y.ponded_sediment.value()),
        ] {
            assert_same_bits(p, q);
        }
        let origin = |c: &CellState| {
            c.seafloor_origin.map(|o| {
                (
                    bits(o.ridge_distance),
                    hyperion_testkit::float::bits(o.half_rate.value()),
                    hyperion_testkit::float::bits(o.age.value()),
                )
            })
        };
        let kinds = |c: &CellState| {
            (
                c.plate,
                c.crust,
                c.boundary,
                c.flow,
                c.ice_entry,
                c.liquid_entry,
                c.class,
                c.crater_state,
                c.subducts,
            )
        };
        let heights = |c: &CellState| {
            (
                bits(c.boundary_distance),
                bits(c.hill_relief),
                origin(c),
                c.seafloor_age
                    .map(|t| hyperion_testkit::float::bits(t.value())),
            )
        };
        assert_eq!(kinds(x), kinds(y), "{name}, cell {n}");
        assert_eq!(heights(x), heights(y), "{name}, cell {n}");
    }
}

/// Runs the climate step twice on `inputs` at level 5 from `start`, or the initial state, and
/// holds the two answers the same, bit for bit ([`assert_same_working`]).
fn runs_twice_to_the_same_bits(name: &str, inputs: &CoarseInputs, start: Option<Working>) {
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, inputs, &field, &climate_grid);
    let start = start.unwrap_or_else(|| Working::initial(&pass));
    let (mut first, mut second) = (start.clone(), start);
    run(&pass, &mut first);
    run(&pass, &mut second);
    assert_same_working(name, &first, &second);
}

/// The step is a function of its inputs (sim-determinism): run twice from the same working state
/// at level 5, on the locked Moon (the radiative model on the tidally locked grid, twelve months),
/// the reference Mars (the radiative model under R09.T13.h's thin air: its slab carried from orbit
/// to orbit, its snow and its frost), the Venus-like body (the isothermal model), the Titan-like
/// world with its basins (the energy-balance model and two liquids) and the reference Earth locked
/// 1:1 in a one-month year (the locked energy-balance model), it writes every climate record and
/// cell field to the same bits. The locked model's seasonal march is the slow test's below.
#[test]
fn the_step_writes_the_same_bits_from_the_same_state() {
    let (_, _, titan, basins) = titan_with_basins(5);
    runs_twice_to_the_same_bits("locked Moon", &super::other_regimes::locked_moon(), None);
    runs_twice_to_the_same_bits("reference Mars", &reference::mars_like(), None);
    runs_twice_to_the_same_bits("Venus-like", &venus(), None);
    runs_twice_to_the_same_bits("Titan-like", &titan, Some(basins));
    runs_twice_to_the_same_bits("locked Earth", &super::locked_earth(0.0, 0.0), None);
}

/// [`the_step_writes_the_same_bits_from_the_same_state`] on the reference Earth locked 1:1 on an
/// eccentric orbit, e = 0.05: the locked energy-balance model's spin-up and seasonal orbits.
#[test]
#[ignore = "slow: two seasonal runs of the 72 × 144 locked model, about 10 s"]
fn a_seasonal_locked_world_writes_the_same_bits_from_the_same_state() {
    runs_twice_to_the_same_bits(
        "locked Earth, e 0.05",
        &super::locked_earth(0.05, 0.0),
        None,
    );
}
