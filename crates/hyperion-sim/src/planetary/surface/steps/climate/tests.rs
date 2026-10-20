//! The climate step's tests (R09.T13.a–h, j): the plan's, with the pipeline's own.

use hyperion_surface::field::{ClimateModelKind, CoarseLevel};

use super::coordinates::{Coordinates, substellar_cosine};
use super::ebm::{
    self, Air, EnergyBalance, Freezing, Greenhouse, Ground, MAX_ORBITS, Moisture, Relief, contrast,
};
use super::grid::{GEOGRAPHIC_COLUMNS, GridClimate, LATITUDE_BANDS, LOCKED_COLUMNS, ModelGrid};
use super::hypsometry::{Hypsometry, Shape};
use super::ocean::OceanTransport;
use super::sun::{NOMINAL_STEP, Sunlight};
use super::transport::{
    Diffusing, EARTH_TRANSPORT_KG_M2_S, PRESSURE_REFERENCE_CONTRAST_K, PRESSURE_TABLE,
    REFERENCE_PRESSURE_PA, ROTATION_REFERENCE_CONTRAST_K, ROTATION_TABLE, TRANSPORT_CAP,
    TransportNode, rotation_ratio, transport_kg_m2_s,
};
use super::{lapse, normalise, point_sea_shares, run, sea_shares, seas};
use crate::math;
use crate::planetary::hooks::surface::EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::{
    ClimateRegime, CoarseInputs, Forcing, SeasonalOrbit, Spin,
};
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::{CellState, Pass, Working};
use crate::substance::by_key;
use crate::units::consts::SECONDS_PER_DAY;
use crate::units::{Metres, Radians, Seconds};
use earth_land::{EARTH_BOX_MEDIAN_M, EARTH_HYPSOMETRY_M, EARTH_LAND_BOXES, EARTH_LAND_SHARE};
use hyperion_testkit::float::assert_same_bits;

// The builder values of a world, as a builder to vary: `reference`'s one copy, which the step's
// tests in its other files reach here (decision-r09-t13-test-cost §3.1).
pub(super) use crate::planetary::surface::reference::builder_of;

/// `inputs` under the air of `air`, its pressure and its gases, with its mean set 2% above its own
/// so that its slab's β is solved positive (R09.T13.h's tests, decision-r09-t13h-scope §3: the 3:2
/// Mercury and a seasonal locked world under the reference Mars's air).
pub(super) fn with_thin_air(inputs: &CoarseInputs, air: &CoarseInputs) -> CoarseInputs {
    builder_of(inputs)
        .surface_pressure(air.surface_pressure())
        .gases(air.gases().iter().copied())
        .mean_surface_temperature(crate::units::Kelvin::new(
            1.02 * inputs.mean_surface_temperature().value(),
        ))
        .build()
        .expect("a world's inputs hold under a thin air")
}

/// The first band ERA-Interim's curve reaches: band 5, centred on 76.25° S. South of about 77° S
/// Siler et al.'s Fig. 2b shows only their model's curve.
const ERA_FIRST_BAND: usize = 5;

/// ERA-Interim's annual, zonal-mean air temperature at sea level over 1981–2010, K, at each band
/// centre from 76.25° S to 88.75° N: Siler, Roe and Armour 2018 (J. Climate 31, 7481), Fig. 2b's
/// blue curve, read from the authors' PDF rendered at 800 dpi, the line's centre in each column
/// (19.8 px per kelvin, the line 0.76 K wide), to about ±0.3 K; where their model's red curve hid
/// it (near 51° S, and along part of the tropics' top), from the visible edge or the red line's
/// centre, to about ±0.4 K.
const ERA_INTERIM_K: [f64; LATITUDE_BANDS - ERA_FIRST_BAND] = [
    256.8, 260.6, 262.4, 264.3, 266.2, 268.8, 271.4, 273.5, 275.2, 276.8, 278.3, 279.8, 281.6,
    283.6, 285.6, 287.4, 289.0, 290.4, 291.8, 293.1, 294.3, 295.3, 296.1, 297.0, 297.8, 298.3,
    298.8, 299.1, 299.4, 299.4, 299.4, 299.6, 299.9, 300.1, 300.1, 300.0, 299.8, 299.2, 298.6,
    297.9, 297.0, 296.0, 294.8, 293.5, 292.1, 290.8, 289.0, 286.6, 284.3, 282.4, 281.1, 280.3,
    279.1, 277.7, 276.3, 274.4, 273.9, 273.2, 270.5, 267.6, 264.0, 261.6, 260.0, 258.8, 258.2,
    258.2, 258.2,
];

/// The first band the Earth test holds to 5 K: band 12, centred on 58.75° S (see
/// [`an_earth_like_world_converges_and_lies_within_5_k_of_era_interim`]).
const EARTH_ASSERTED_FROM_BAND: usize = 12;

/// The bound the Earth test holds the bands south of [`EARTH_ASSERTED_FROM_BAND`] to, K: 12, the
/// sea-level ice sheet's warm bias (+11.3 K at 76.25° S as built), which waits on R09.T12.
const SOUTHERN_BOUND_K: f64 = 12.0;

/// The grids of a coarse level `level`.
fn grids(level: u8) -> (CellGraph, CellGraph) {
    let level = CoarseLevel::new(level).expect("a coarse level");
    (CellGraph::for_field(level), CellGraph::for_climate(level))
}

/// Sets field cell `cell`'s ground to `elevation` as R09.T12's relief step leaves it against the
/// sea level `sea_level`: below it with a liquid entry `flood`, flooded as R09.T12.e floods its
/// ocean (decision-r09-t12-coarse R8: its water surface the sea level, its liquid entry `flood`,
/// the main liquid's); otherwise dry, its water surface its ground and no liquid, as an enclosed
/// depression below the sea level is, or any cell of a world with no liquid. A test that sets a
/// sea by hand sets its flood so.
pub(super) fn set_ground(
    cell: &mut CellState,
    elevation: Metres,
    sea_level: Metres,
    flood: Option<u8>,
) {
    cell.elevation = elevation;
    match flood {
        Some(entry) if elevation.value() < sea_level.value() => {
            cell.water_surface = sea_level;
            cell.liquid_entry = Some(entry);
        }
        Some(_) | None => {
            cell.water_surface = elevation;
            cell.liquid_entry = None;
        }
    }
}

/// Land at the sea level in every band: the synthetic sea with no relief.
const SEA_LEVEL_LAND: [[i16; 8]; LATITUDE_BANDS] = [[0; 8]; LATITUDE_BANDS];

/// Lays out Earth's land on `working`'s cells as a synthetic sea, its land at the heights
/// `hypsometry` gives each band; the sea level is 0 (decision-r09-t13e-asbuilt §1.3–§1.4).
///
/// - **The land** is Natural Earth's by 2.5° × 5° box ([`EARTH_LAND_BOXES`], thousandths; a cell's
///   box the one its centre lies in, the geographic grid's point): in each box the cells in order
///   of longitude, then cell index, are land until their solid angle reaches the box's share (a
///   cell is land while half of it fits), and the rest sea, 100 m below the sea level and flooded
///   to it with the main liquid ([`set_ground`]).
/// - **The heights**: each band's land cells, in order of their box's Earth2014 median land height
///   ([`EARTH_BOX_MEDIAN_M`]; a box with no Earth2014 land lowest), then longitude, then cell
///   index, take the band's eight heights in turn, the q-th eighth of the band's land solid angle
///   (by each cell's midpoint) at height q, dry ([`SEA_LEVEL_LAND`]: at the sea level). Each
///   band's heights are so R09.T13.g's eighths, and its relief stands where Earth's does: the
///   Tibetan Plateau, the Andes, the Rockies, the ice sheets.
fn synthetic_sea(pass: &Pass<'_>, working: &mut Working, hypsometry: &[[i16; 8]; LATITUDE_BANDS]) {
    let grid = pass.grid();
    let solid_angles = grid.solid_angles_sr();
    let flood = pass.inputs().main_liquid();
    let points = ModelGrid::geographic();
    let mut boxes: Vec<Vec<(f64, usize)>> = vec![Vec::new(); LATITUDE_BANDS * GEOGRAPHIC_COLUMNS];
    for (cell, centre) in grid.centres().iter().enumerate() {
        boxes[points.point_of(*centre)].push((math::atan2(centre[1], centre[0]), cell));
    }
    for (band, row) in boxes
        .as_chunks_mut::<GEOGRAPHIC_COLUMNS>()
        .0
        .iter_mut()
        .enumerate()
    {
        // The band's land cells, each with its box's median height and its longitude.
        let mut lands = Vec::new();
        for ((cells, &share), &median) in row
            .iter_mut()
            .zip(&EARTH_LAND_BOXES[band])
            .zip(&EARTH_BOX_MEDIAN_M[band])
        {
            cells.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let total = cells
                .iter()
                .fold(0.0, |sum, &(_, cell)| sum + solid_angles[cell]);
            let target = f64::from(share) / 1_000.0 * total;
            let mut held = 0.0;
            for &(longitude, cell) in cells.iter() {
                let solid_angle = solid_angles[cell];
                if held + 0.5 * solid_angle <= target {
                    held += solid_angle;
                    lands.push((median, longitude, cell));
                } else {
                    set_ground(
                        &mut working.cells[cell],
                        Metres::new(-100.0),
                        Metres::ZERO,
                        flood,
                    );
                }
            }
        }
        lands.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        let held = lands
            .iter()
            .fold(0.0, |sum, &(_, _, cell)| sum + solid_angles[cell]);
        let mut below = 0.0;
        for (_, _, cell) in lands {
            let solid_angle = solid_angles[cell];
            // The eighths of the band's land its midpoint has passed.
            let share = (below + 0.5 * solid_angle) / held;
            let q = (1..8_u8).filter(|&k| 8.0 * share >= f64::from(k)).count();
            below += solid_angle;
            set_ground(
                &mut working.cells[cell],
                Metres::new(f64::from(hypsometry[band][q])),
                Metres::ZERO,
                flood,
            );
        }
    }
    working.sea_level = Metres::ZERO;
}

/// The reference Earth at level 7 with Earth's land by 2.5° × 5° box as a synthetic sea
/// ([`EARTH_LAND_BOXES`]; each band's land share within 0.03 of [`EARTH_LAND_SHARE`]'s, the worst
/// band printed), its land at the heights `hypsometry` gives it ([`synthetic_sea`]), and its zonal
/// model's answer over that ground, the zonal relief from its cells as the step builds it: the
/// pass's grids, the working state, the relief and the answer.
fn era_earth(hypsometry: &[[i16; 8]; LATITUDE_BANDS]) -> (ebm::Solution, Relief, Working) {
    let inputs = reference::earth_like();
    let (field, climate) = grids(7);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
    let mut working = Working::initial(&pass);
    synthetic_sea(&pass, &mut working, hypsometry);
    let sea = sea_shares(&pass, &working);
    let mut worst = (0, 0.0_f64);
    for (band, (&s, &l)) in sea.iter().zip(&EARTH_LAND_SHARE).enumerate() {
        assert!((s + l - 1.0).abs() < 0.03, "band {band}: sea {s}, land {l}");
        if (s + l - 1.0).abs() > worst.1.abs() {
            worst = (band, s + l - 1.0);
        }
    }
    eprintln!(
        "the synthetic sea's land share against Earth's: {:+.4} at worst, band {}",
        -worst.1, worst.0
    );
    let grid = ModelGrid::zonal();
    let relief = Relief::of(
        &inputs,
        Hypsometry::of_field(&pass, &working, &grid, Coordinates::Geographic),
    );
    let ground = Ground::Relief {
        grid: &relief,
        zonal: &relief,
    };
    let solution = ebm::solve(
        &inputs,
        &grid,
        &sea,
        NOMINAL_STEP,
        ground,
        super::sea_transport(&pass, &working),
    );
    (solution, relief, working)
}

/// The plan's Earth test. The reference Earth, with Earth's land by box as a synthetic sea
/// (R09.T12's relief is not built; [`era_earth`]) and its land at the sea level, converges within
/// [`MAX_ORBITS`], and its annual-mean zonal temperature at sea level lies within 5 K of
/// ERA-Interim's (Siler et al. 2018, Fig. 2b) from 58.75° S to the North Pole.
///
/// South of that it is too warm, by up to about 11 K at 76° S, where ERA-Interim's curve ends: the
/// model's Antarctica lies at sea level, so its summer rises above the albedo's ramp and its ice
/// darkens, where the real ice sheet, 2–3 km up, stays bright. Those bands are held to
/// [`SOUTHERN_BOUND_K`], and every band is printed. With Earth's hypsometry
/// ([`an_earth_with_its_land_s_hypsometry_lies_within_the_ruled_bounds_of_era_interim`]) the model
/// reads the ice sheet's height (R09.T13.g).
#[test]
fn an_earth_like_world_converges_and_lies_within_5_k_of_era_interim() {
    let (solution, relief, _) = era_earth(&SEA_LEVEL_LAND);
    assert!(
        relief
            .hypsometry()
            .all()
            .iter()
            .all(|point| point.shape == Shape::SeaLevel)
    );
    let grid = ModelGrid::zonal();
    let earth = &solution.climate;
    assert!(earth.converged, "{} orbits", earth.orbits);
    assert!(earth.orbits <= MAX_ORBITS);
    let mut worst: f64 = 0.0;
    for (band, &era) in (ERA_FIRST_BAND..).zip(&ERA_INTERIM_K) {
        let model = earth.annual_k[band];
        let latitude = math::asin(grid.centre_sine(band)).to_degrees();
        eprintln!(
            "{latitude:7.2}° model {model:6.1} K, ERA-Interim {era:6.1} K, {:+5.1} K",
            model - era
        );
        if band >= EARTH_ASSERTED_FROM_BAND {
            worst = worst.max((model - era).abs());
        } else {
            assert!(
                (model - era).abs() <= SOUTHERN_BOUND_K,
                "{latitude}°: {model} K"
            );
        }
    }
    eprintln!(
        "orbits {}, τ {:.3}, global mean {:.2} K, worst asserted {worst:.2} K",
        earth.orbits,
        (solution.emission_factor - 1.0) / 0.75,
        grid.mean(&earth.annual_k)
    );
    assert!(worst <= 5.0, "worst {worst} K");
}

/// The bound R09.T13.g's Earth test holds the band of 76.25° S to, K: 11, the measured +9.29 K
/// with a margin (decision-r09-t13ge §1.3, (i)).
const RELIEF_POLAR_BOUND_K: f64 = 11.0;

/// The bound it holds the bands from 73.75° S to 61.25° S to, K: 8, the measured +6.85 K with a
/// margin (decision-r09-t13ge §1.3, (i)).
const RELIEF_SOUTHERN_BOUND_K: f64 = 8.0;

/// R09.T13.g's Earth test (decision-r09-t13ge §1.6). The reference Earth with its land's
/// hypsometry by hand ([`EARTH_HYPSOMETRY_M`], each band's land cells at its eight heights in order
/// of their 2.5° × 5° box's Earth2014 median land height, its sea flooded to the sea level; Natural
/// Earth's land by box, [`EARTH_LAND_BOXES`], its band totals [`EARTH_LAND_SHARE`]'s;
/// [`synthetic_sea`], decision-r09-t13e-asbuilt): its annual-mean zonal
/// temperature at sea level lies within 5 K of Siler et al.'s Fig. 2b from 58.75° S northward,
/// within 8 K from 73.75° S to 61.25° S and within 11 K at 76.25° S, every band printed, beside the
/// same Earth with every land cell at the sea level; its 76.25° S and 73.75° S bands are each at
/// least 1 K colder than that Earth's; and its τ solve's steady state has converged, with τ, the
/// steady and seasonal surface means printed.
///
/// The remaining warm bias in the south (+9.3 K at 76.25° S in the ruling's prototype) is the
/// model's, deferred to R09.T13.i (decision-r09-t13ge §1.2–§1.3, G1): the Southern Ocean's own
/// bias, the temperature-only albedo's summer coast and the grey column's surface. ERA-Interim's
/// sea-level temperatures over the plateau are reduced by ECMWF's own method, unread, where the
/// model's sea level reads Γ ≈ 5.8 K km⁻¹; the bounds hold either way
/// (decision-r09-t13a-relief §2.1).
#[test]
fn an_earth_with_its_land_s_hypsometry_lies_within_the_ruled_bounds_of_era_interim() {
    let (solution, relief, _) = era_earth(&EARTH_HYPSOMETRY_M);
    let (flat, _, _) = era_earth(&SEA_LEVEL_LAND);
    let grid = ModelGrid::zonal();
    let (earth, level) = (&solution.climate, &flat.climate);
    assert!(earth.converged, "{} orbits", earth.orbits);
    let mut worst: f64 = 0.0;
    for (band, &era) in (ERA_FIRST_BAND..).zip(&ERA_INTERIM_K) {
        let (model, sea_level) = (earth.annual_k[band], level.annual_k[band]);
        let latitude = math::asin(grid.centre_sine(band)).to_degrees();
        let heights = relief.hypsometry().point(band);
        eprintln!(
            "{latitude:7.2}° relief {model:6.2} K ({:+6.2} K), at sea level {sea_level:6.2} K \
             ({:+6.2} K), ERA-Interim {era:6.1} K; quantiles {:?} m",
            model - era,
            sea_level - era,
            heights.quantiles_m.map(f64::round)
        );
        let bound = match band {
            ERA_FIRST_BAND => RELIEF_POLAR_BOUND_K,
            _ if band < EARTH_ASSERTED_FROM_BAND => RELIEF_SOUTHERN_BOUND_K,
            _ => {
                worst = worst.max((model - era).abs());
                5.0
            }
        };
        assert!((model - era).abs() <= bound, "{latitude}°: {model} K");
    }
    for band in [ERA_FIRST_BAND, ERA_FIRST_BAND + 1] {
        assert!(
            earth.annual_k[band] <= level.annual_k[band] - 1.0,
            "band {band}: {} K against {} K at sea level",
            earth.annual_k[band],
            level.annual_k[band]
        );
    }
    assert!(solution.steady_converged, "{:?}", solution.optical_depth);
    eprintln!(
        "relief: orbits {}, τ {:.4} ({:?}), steady surface mean {:.3} K, seasonal surface mean \
         {:.3} K, seasonal sea-level mean {:.3} K, worst from 58.75° S {worst:.2} K",
        earth.orbits,
        (solution.emission_factor - 1.0) / ebm::EDDINGTON_COEFFICIENT,
        solution.optical_depth,
        solution.steady_surface_mean_k,
        relief.surface_mean_k(&grid, &earth.annual_k),
        grid.mean(&earth.annual_k)
    );
    eprintln!(
        "at sea level: orbits {}, τ {:.4} ({:?}), steady mean {:.3} K, seasonal mean {:.3} K",
        level.orbits,
        (flat.emission_factor - 1.0) / ebm::EDDINGTON_COEFFICIENT,
        flat.optical_depth,
        flat.steady_surface_mean_k,
        grid.mean(&level.annual_k)
    );
    assert!(worst <= 5.0, "worst {worst} K");
}

/// R09.T13.i's liquid (decision-r09-t13i §2.5, test 5): the reference Earth's sea is water's, its
/// heat capacities' factor and its specific heat ratio exactly 1; a Titan-like world's methane sea
/// takes its own, `ρ_l c_l` ÷ (`ρ_w c_w`) at the model's freezing point plus 10 K (100.7 K for
/// methane, water held at its column's 275 K), within 2% of the reference equations' own ratio at
/// those states: Setzmann and Wagner 1991 (J. Phys. Chem. Ref. Data 20, 1061), Table 39, at 100 K,
/// 438.89 kg m⁻³ and 3.4084 J g⁻¹ K⁻¹ (the state `substance::liquid`'s tests hold), over IAPWS-95
/// at 275 K as NIST's Chemistry WebBook (SRD 69) evaluates it, 999.887 406 kg m⁻³ and 4.213 946 42
/// J g⁻¹ K⁻¹ (the same tests' state; the cross-check of asserted values only, decision-composition
/// §5): 0.355. Its Ekman cells carry methane's `c_l` and its gyres `c_l` ÷ `c_w`.
#[test]
fn a_methane_sea_holds_its_own_heat_and_water_s_ratio_is_one() {
    let earth = reference::earth_like();
    let water = super::ocean::SeaLiquid::of(&earth, Freezing::of(&earth));
    assert!(water.has_column());
    assert_same_bits(water.capacity_ratio(), 1.0);
    assert_same_bits(water.heat_ratio(), 1.0);
    let titan = normalisation::titan_like();
    let methane = super::ocean::SeaLiquid::of(&titan, Freezing::of(&titan));
    let expected = (438.89 * 3_408.4) / (999.887_406 * 4_213.946_42);
    eprintln!(
        "water: c_l {:.1} J kg⁻¹ K⁻¹; methane: c_l {:.1} J kg⁻¹ K⁻¹, r_l {:.4} against {expected:.4}, \
         c_l ÷ c_w {:.4}, its column {}",
        water.specific_heat_j_per_kg_k(),
        methane.specific_heat_j_per_kg_k(),
        methane.capacity_ratio(),
        methane.heat_ratio(),
        methane.has_column()
    );
    assert!(methane.has_column());
    assert!(
        (methane.capacity_ratio() / expected - 1.0).abs() < 0.02,
        "{} against {expected}",
        methane.capacity_ratio()
    );
    // The model's sea capacities take it, and water's are as built.
    let model = EnergyBalance::for_inputs(&titan);
    assert_same_bits(model.sea_capacity_ratio, methane.capacity_ratio());
    assert_same_bits(EnergyBalance::for_inputs(&earth).sea_capacity_ratio, 1.0);
}

/// R09.T13.g's Earth through the step (decision-r09-t13ge §3, G3): the reference Earth with its
/// land's hypsometry by hand, run through the climate step, whose geographic model reads each
/// point's own quantiles while its τ solve reads each band's pooled ones. The step's mean constant
/// c reconciles the two: after it the field cells' surface mean is the input's to 0.1 K
/// (R09.T13.d's test, on a world with relief). The zonal τ solve's surface mean, the geographic
/// seasonal run's and c are printed.
#[test]
#[ignore = "slow: the 72 × 72 model over the relief Earth's synthetic sea, about 6.5 s"]
fn the_step_s_mean_constant_reconciles_the_relief_earth_s_surface_means() {
    let inputs = reference::earth_like();
    let (field, climate) = grids(7);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
    let mut working = Working::initial(&pass);
    synthetic_sea(&pass, &mut working, &EARTH_HYPSOMETRY_M);
    let diagnostics = super::run_diagnosed(&pass, &mut working);
    let means = diagnostics
        .surface_means
        .expect("the energy-balance model's surface means");
    let surface = super::surface_mean_k(&pass, &working);
    eprintln!(
        "zonal τ solve's surface mean {:.3} K, the geographic seasonal run's {:.3} K, the mean \
         constant c {:+.3} K, a {:+.3} K; the field's surface mean after it {surface:.4} K",
        means.steady_k,
        means.seasonal_k,
        diagnostics.correction.mean_k,
        diagnostics.correction.equator_pole_k
    );
    let mean = inputs.mean_surface_temperature().value();
    assert!((surface - mean).abs() < 0.1, "{surface} K against {mean} K");
}

/// The two answers of the model are the same to the bit: every record, the orbits, τ and its
/// steady state's mean.
fn assert_same_solution(name: &str, a: &ebm::Solution, b: &ebm::Solution) {
    assert_eq!(a.climate.orbits, b.climate.orbits, "{name}");
    assert_eq!(a.climate.converged, b.climate.converged, "{name}");
    assert_eq!(a.optical_depth, b.optical_depth, "{name}");
    assert_eq!(a.steady_converged, b.steady_converged, "{name}");
    assert_same_bits(a.emission_factor, b.emission_factor);
    assert_same_bits(a.steady_surface_mean_k, b.steady_surface_mean_k);
    assert_eq!(a.climate.months_k.len(), b.climate.months_k.len(), "{name}");
    for (x, y) in a.climate.annual_k.iter().zip(&b.climate.annual_k) {
        assert_same_bits(*x, *y);
    }
    for (m, n) in a.climate.months_k.iter().zip(&b.climate.months_k) {
        for (x, y) in m.iter().zip(n) {
            assert_same_bits(*x, *y);
        }
    }
}

/// R09.T13.g's first test (decision-r09-t13ge §1.6): with every height zero, the model the step
/// runs, over the relief it builds from the working state, is T13.a's to the bit. On the
/// reference Earth's synthetic sea (land and sea at the sea level) through the zonal model, the
/// reference Earth all land at level 5 (about a fifth of the geographic points holding no cell)
/// through the geographic model, and a one-month locked Earth through the locked model, each
/// against the model with no relief at all ([`Ground::SeaLevel`]).
#[test]
fn with_every_height_zero_the_model_is_t13a_s_to_the_bit() {
    let inputs = reference::earth_like();
    let (field, climate) = grids(7);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
    let mut working = Working::initial(&pass);
    synthetic_sea(&pass, &mut working, &SEA_LEVEL_LAND);
    let zonal = ModelGrid::zonal();
    let sea = sea_shares(&pass, &working);
    let (_, relief) = super::reliefs(&pass, &working, &zonal, Coordinates::Geographic);
    let ground = Ground::Relief {
        grid: &relief,
        zonal: &relief,
    };
    let ocean = super::sea_transport(&pass, &working);
    assert_same_solution(
        "the zonal Earth",
        &ebm::solve(&inputs, &zonal, &sea, NOMINAL_STEP, ground, ocean),
        &ebm::solve(&inputs, &zonal, &sea, NOMINAL_STEP, Ground::SeaLevel, ocean),
    );
    for (name, inputs, grid, coordinates) in [
        (
            "the geographic Earth",
            reference::earth_like(),
            ModelGrid::geographic(),
            Coordinates::Geographic,
        ),
        (
            "the locked Earth",
            locked_earth(0.0, 0.0),
            ModelGrid::locked(),
            Coordinates::TidallyLocked,
        ),
    ] {
        let (field, climate) = grids(5);
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
        let working = Working::initial(&pass);
        let sea = point_sea_shares(&pass, &working, &grid, coordinates);
        let (on_grid, zonal) = super::reliefs(&pass, &working, &grid, coordinates);
        let ground = Ground::Relief {
            grid: &on_grid,
            zonal: &zonal,
        };
        let ocean = super::sea_transport(&pass, &working);
        let solve = |ground| match coordinates {
            Coordinates::Geographic => {
                ebm::solve(&inputs, &grid, &sea, NOMINAL_STEP, ground, ocean)
            }
            Coordinates::TidallyLocked => {
                ebm::solve_locked(&inputs, &grid, &sea, NOMINAL_STEP, ground, ocean)
            }
        };
        assert_same_solution(name, &solve(ground), &solve(Ground::SeaLevel));
    }
}

/// Kaspi and Showman's GCM configuration (2015, §2) as far as the model takes it, seasons off
/// (the tables' documentation): their insolation, eq. 10, every band sea under a fixed albedo of
/// 0.35, no internal heat, Earth's dry air with water at 80% relative humidity, and one optical
/// depth, solved so that the reference run's mean is 288 K and then held.
struct KaspiShowman {
    grid: ModelGrid,
    sunlight: Sunlight,
    emission_factor: f64,
    reference_state: Vec<f64>,
    reference_contrast: f64,
}

impl KaspiShowman {
    fn new() -> Self {
        let grid = ModelGrid::zonal();
        let insolation = (0..LATITUDE_BANDS)
            .map(|k| {
                let s = grid.centre_sine(k);
                1_360.0 / 4.0 * (1.0 + 1.2 / 4.0 * (1.0 - 3.0 * s * s))
            })
            .collect();
        let sunlight = Sunlight::constant(insolation);
        let model = Self::model(REFERENCE_PRESSURE_PA, 1.0);
        let fitted = model.emission_factor(&grid, sunlight.annual(), 288.0);
        assert!(fitted.steady.converged);
        let (emission_factor, reference_state) = (fitted.emission_factor, fitted.steady.t_k);
        let reference_contrast = contrast(&reference_state);
        Self {
            grid,
            sunlight,
            emission_factor,
            reference_state,
            reference_contrast,
        }
    }

    /// The model at surface pressure `pressure_pa` with the transport `factor` × Λ⊕.
    fn model(pressure_pa: f64, factor: f64) -> EnergyBalance {
        let water = by_key("H2O").expect("water is a registry row");
        let row = water.substance();
        let mass = row.molar_mass_g_per_mol().unwrap();
        let air = Air::EARTH;
        EnergyBalance {
            // No ice, as theirs (§2): a steady state stores no heat, so the sea's capacity, which
            // alone reads a freezing point, never enters.
            freezing: Freezing::Never { albedo: 0.35 },
            specific_heat_j_per_kg_k: EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
            moisture: Some(Moisture {
                condensable: water,
                latent_heat_j_per_kg: row.phase().unwrap().vaporisation_enthalpy_j_per_mol()
                    / (mass * 1e-3),
                mass_ratio: mass / air.molar_mass_g_per_mol,
                surface_pressure_pa: pressure_pa,
            }),
            transport_kg_m2_s: factor * EARTH_TRANSPORT_KG_M2_S,
            heat_flow_w_m2: 0.0,
            // Their GCM's grey radiation holds its optical depth fixed, with no vapour feedback
            // (§2, §3.3): one τ (decision-r09-t13i §2.2).
            greenhouse: Greenhouse::Grey,
            sea_capacity_ratio: 1.0,
        }
    }

    /// The model's contrast at surface pressure `pressure_pa` and transport factor `factor`.
    fn contrast(&self, pressure_pa: f64, factor: f64) -> f64 {
        let steady = Self::model(pressure_pa, factor).steady_state(
            &self.grid,
            self.emission_factor,
            self.sunlight.annual(),
            self.reference_state.clone(),
        );
        assert!(steady.converged, "p {pressure_pa}, f {factor}");
        contrast(&steady.t_k)
    }

    /// The factor at which the model's contrast at `pressure_pa`, scaled by `reference_k` over its
    /// own reference contrast, is `gcm_k`: 48 bisections of ln f over [10⁻³, 10³].
    fn fit(&self, pressure_pa: f64, gcm_k: f64, reference_k: f64) -> f64 {
        let target = self.reference_contrast * gcm_k / reference_k;
        let (mut low, mut high) = (math::ln(1e-3), math::ln(1e3));
        for _ in 0..48 {
            let middle = f64::midpoint(low, high);
            // The contrast falls as the transport grows.
            if self.contrast(pressure_pa, math::exp(middle)) > target {
                low = middle;
            } else {
                high = middle;
            }
        }
        math::exp(f64::midpoint(low, high))
    }
}

/// A sweep: its name, its table, its reference run's contrast, K, and each node's surface
/// pressure, Pa, from its ratio.
type Sweep = (&'static str, &'static [TransportNode], f64, fn(f64) -> f64);

/// The two sweeps' runs.
fn sweeps() -> [Sweep; 2] {
    [
        (
            "rotation",
            &ROTATION_TABLE,
            ROTATION_REFERENCE_CONTRAST_K,
            |_| REFERENCE_PRESSURE_PA,
        ),
        (
            "pressure",
            &PRESSURE_TABLE,
            PRESSURE_REFERENCE_CONTRAST_K,
            |ratio| ratio * REFERENCE_PRESSURE_PA,
        ),
    ]
}

/// The plan's sweep test: with seasons off, the model's equator–pole contrast at each of the
/// tables' factors, scaled so that the 1 Ω⊕, 1 bar run reads Kaspi and Showman's 40.2 K (Fig. 8a)
/// or 42.1 K (Fig. 15c), is theirs to 2 K over Ω⊕ ÷ 24 to 12 Ω⊕ and 0.2 to 50 bar.
#[test]
fn the_contrast_follows_kaspi_and_showman_s_sweeps() {
    let ks = KaspiShowman::new();
    eprintln!(
        "reference: τ {:.4}, contrast {:.3} K",
        (ks.emission_factor - 1.0) / 0.75,
        ks.reference_contrast
    );
    for (name, table, reference_k, pressure) in sweeps() {
        for node in table {
            let scaled = ks.contrast(pressure(node.ratio), node.factor) * reference_k
                / ks.reference_contrast;
            assert!(
                (scaled - node.gcm_contrast_k).abs() <= 2.0,
                "{name} {}: {scaled} K against the GCM's {} K",
                node.ratio,
                node.gcm_contrast_k
            );
        }
    }
}

/// The tables are the fit their documentation describes: refitted, every factor is its constant's
/// to 10⁻⁴.
#[test]
fn the_transport_tables_are_their_fit() {
    let ks = KaspiShowman::new();
    let mut failures = Vec::new();
    for (name, table, reference_k, pressure) in sweeps() {
        for node in table {
            let fitted = ks.fit(pressure(node.ratio), node.gcm_contrast_k, reference_k);
            eprintln!(
                "{name}: node({}, {}, {fitted:.5}),",
                node.ratio, node.gcm_contrast_k
            );
            if (fitted / node.factor - 1.0).abs() > 1e-4 {
                failures.push((name, node.ratio, fitted, node.factor));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// The rotation periods of the reference set (Earth, Mars, the Moon, Ceres) and Venus's, s.
/// Venus's sidereal period is 5,832.6 h, retrograde (NASA's Venus fact sheet), which sets only
/// the rate here.
fn rotations() -> [(&'static str, f64); 5] {
    [
        (
            "Earth",
            reference::earth_like().spin().rotation_period.value(),
        ),
        (
            "Mars",
            reference::mars_like().spin().rotation_period.value(),
        ),
        (
            "Moon",
            reference::moon_like().spin().rotation_period.value(),
        ),
        (
            "Ceres",
            reference::ceres_like().spin().rotation_period.value(),
        ),
        ("Venus", 5_832.6 * 3_600.0),
    ]
}

/// The plan's stability test: the reference Earth, all land as the field is until R09.T12 (the
/// stiffest seasons), at each reference world's rotation and Venus's, and at the transport's cap,
/// converges at the six-hour step to a finite climate within 150–350 K that a step of 1.5 h
/// reproduces to 0.5 K in the annual mean and 1 K in every month.
#[test]
fn the_scheme_is_stable_at_a_six_hour_step_for_every_rotation_and_at_venus_s() {
    let inputs = reference::earth_like();
    let grid = ModelGrid::zonal();
    let land = [0.0; LATITUDE_BANDS];
    let base = EnergyBalance::for_inputs(&inputs);
    let air = Air::of(&inputs);
    let pressure = inputs.surface_pressure().value() / REFERENCE_PRESSURE_PA;
    let mut cases: Vec<(&str, f64)> = rotations()
        .into_iter()
        .map(|(name, period)| {
            let lambda = transport_kg_m2_s(
                rotation_ratio(period),
                pressure,
                air.molar_mass_g_per_mol,
                air.specific_heat_j_per_kg_k,
                Diffusing::MoistStaticEnergy,
            );
            (name, lambda)
        })
        .collect();
    let cap = TRANSPORT_CAP * EARTH_TRANSPORT_KG_M2_S * EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K
        / air.specific_heat_j_per_kg_k;
    cases.push(("the cap", cap));
    for (name, lambda) in cases {
        let model = EnergyBalance {
            transport_kg_m2_s: lambda,
            ..base
        };
        let mean = inputs.mean_surface_temperature().value();
        let fine = |step: Seconds| {
            let sunlight = Sunlight::of(&inputs, &grid, step);
            model.run(&grid, &sunlight, mean, &land, Ground::SeaLevel, None)
        };
        let coarse = fine(NOMINAL_STEP).climate;
        let reference = fine(NOMINAL_STEP / 4.0).climate;
        assert!(coarse.converged, "{name}: {} orbits", coarse.orbits);
        for (k, (&a, &b)) in coarse.annual_k.iter().zip(&reference.annual_k).enumerate() {
            assert!((150.0..350.0).contains(&a), "{name}, band {k}: {a} K");
            assert!((a - b).abs() < 0.5, "{name}, band {k}: {a} against {b} K");
        }
        for (month, (a, b)) in coarse.months_k.iter().zip(&reference.months_k).enumerate() {
            for (k, (&a, &b)) in a.iter().zip(b).enumerate() {
                assert!(
                    (a - b).abs() < 1.0,
                    "{name}, month {month}, band {k}: {a} against {b}"
                );
            }
        }
    }
}

/// Each month's record is the mean of its span's steps: the annual mean is the months' mean
/// weighted by their steps, and a one-month year's month is its annual mean.
#[test]
fn each_month_is_the_mean_of_its_steps() {
    let inputs = reference::earth_like();
    let grid = ModelGrid::zonal();
    let land = [0.0; LATITUDE_BANDS];
    let sunlight = Sunlight::of(&inputs, &grid, NOMINAL_STEP);
    let model = EnergyBalance::for_inputs(&inputs);
    let climate = model
        .run(&grid, &sunlight, 288.0, &land, Ground::SeaLevel, None)
        .climate;
    let mut counts = [0_u32; 12];
    for step in 0..sunlight.steps() {
        counts[usize::from(sunlight.month_of(step))] += 1;
    }
    for k in 0..LATITUDE_BANDS {
        let weighted = climate
            .months_k
            .iter()
            .zip(counts)
            .fold(0.0, |sum, (month, n)| sum + month[k] * f64::from(n))
            / f64::from(sunlight.steps());
        assert!((weighted - climate.annual_k[k]).abs() < 1e-9, "band {k}");
    }
    // The northern summer is warm in the north and cold in the south.
    let (july, january) = (&climate.months_k[6], &climate.months_k[0]);
    assert!(july[60] > january[60] + 10.0 && january[11] > july[11] + 10.0);
    let steady = Sunlight::constant(sunlight.annual().to_vec());
    let one = model
        .run(&grid, &steady, 288.0, &land, Ground::SeaLevel, None)
        .climate;
    assert_eq!(one.months_k.len(), 1);
    assert_eq!(one.orbits, 0);
    assert_eq!(one.months_k[0].len(), one.annual_k.len());
    for (&month, &annual) in one.months_k[0].iter().zip(&one.annual_k) {
        assert_same_bits(month, annual);
    }
}

/// The step writes every climate cell from the geographic model: the model's field corrected on
/// its grid by plan 14's contrast (R09.T13.d), its annual mean interpolated to each climate cell's
/// centre bicubically, held to its stencil's range, plus the mean's constant, and each month's
/// departure from it; the lapse rate is plan 14's Γ. All land, as the field is until R09.T12, the
/// geographic model's own field is the zonal model's to 0.01 K.
#[test]
fn the_step_writes_each_climate_cell_from_the_geographic_model() {
    let inputs = reference::earth_like();
    assert_eq!(inputs.climate().model, ClimateModelKind::EnergyBalance);
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    let diagnostics = super::run_diagnosed(&pass, &mut working);
    let grid = ModelGrid::geographic();
    let mut climate = ebm::solve(
        &inputs,
        &grid,
        &vec![0.0; grid.points()],
        NOMINAL_STEP,
        Ground::SeaLevel,
        OceanTransport::None,
    )
    .climate;
    let zonal = ebm::solve(
        &inputs,
        &ModelGrid::zonal(),
        &[0.0; LATITUDE_BANDS],
        NOMINAL_STEP,
        Ground::SeaLevel,
        OceanTransport::None,
    );
    for centre in climate_grid.centres() {
        let own = grid.stencil_at(*centre).apply(&climate.annual_k);
        let zonal_annual = ModelGrid::stencil(centre[2]).apply(&zonal.climate.annual_k);
        assert!((own - zonal_annual).abs() < 0.01);
    }
    let correction =
        normalise::arithmetic(&inputs, &grid, Coordinates::Geographic, &climate.annual_k);
    assert_same_bits(
        correction.equator_pole_k,
        diagnostics.correction.equator_pole_k,
    );
    normalise::apply(&mut climate, &grid, Coordinates::Geographic, &correction);
    let mean = diagnostics.correction.mean_k;
    for (state, centre) in working.climate.iter().zip(climate_grid.centres()) {
        let stencil = grid.stencil_at(*centre);
        let annual = stencil.apply_clamped(&climate.annual_k);
        assert!((state.sea_level_temperature.value() - (annual + mean)).abs() < 1e-9);
        for (month, &anomaly) in climate.months_k.iter().zip(&state.month_anomaly_k) {
            assert!((anomaly - (stencil.apply_clamped(month) - annual)).abs() < 1e-9);
        }
    }
    assert_same_bits(
        working.lapse_rate_k_per_m,
        lapse::lapse_rate_k_per_m(&inputs),
    );
    let warmest = working
        .climate
        .iter()
        .map(|c| c.sea_level_temperature.value())
        .fold(0.0, f64::max);
    assert!(warmest > 290.0, "{warmest}");
}

/// An empty band takes the nearest band's sea share, equatorward first: on a world flooded
/// everywhere, every band is all sea.
#[test]
fn every_band_has_a_sea_share() {
    let inputs = reference::earth_like();
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    for cell in &mut working.cells {
        set_ground(cell, Metres::new(-1.0), Metres::ZERO, inputs.main_liquid());
    }
    let sea = sea_shares(&pass, &working);
    assert!(sea.iter().all(|&s| (s - 1.0).abs() < 1e-15), "{sea:?}");
}

/// `inputs` with plan 14's own contrasts for an airless body of its mean, read in the radiative
/// model's brightness kind (decision-r09-t13d-normalisation §2.2), in place of a reference world's
/// placeholders: the equator–pole contrast `T_eq` (q(0)^¼ − q(1)^¼), the ends of the no-transport
/// profile `T_s` q(x)^¼ with q plan 14's annual-mean insolation over S ÷ 4 at its obliquity
/// (`annual_insolation_at`; a locked world's zonal mean is a rotator's); and on a world locked 1:1
/// the day–night contrast of Cowan and Agol's eyeball at no redistribution (2011, ApJ 729, 54,
/// eq. 4: `T_d` = (8/3)^¼ `T_eq`), its night at plan 14's floor (`night_floor`).
pub(super) fn with_airless_contrasts(inputs: &CoarseInputs) -> CoarseInputs {
    let t_eq = inputs.mean_surface_temperature().value();
    let obliquity = inputs.spin().obliquity;
    let end = |x: f64| {
        t_eq * crate::planetary::hooks::surface::annual_insolation_at(obliquity, x)
            .sqrt()
            .sqrt()
    };
    let mut builder = builder_of(inputs).equator_pole_contrast_k(end(0.0) - end(1.0));
    if inputs.climate().forcing == Forcing::Locked {
        let floor = crate::planetary::hooks::surface::night_floor(inputs.heat_flow()).value();
        builder = builder.day_night_contrast_k((8.0_f64 / 3.0).sqrt().sqrt() * t_eq - floor);
    }
    builder
        .build()
        .expect("plan 14's contrasts are valid inputs")
}

/// The locked test world's year, s: 24 days, about the orbit on which an M dwarf of 0.3 M☉ and
/// 0.012 L☉ gives Earth's flux, a = √0.012 au = 0.11 au (Kepler's third law; the dwarf's
/// figures illustrative, from memory). The model reads only the period.
const LOCKED_YEAR_S: f64 = 24.0 * SECONDS_PER_DAY;

/// The reference Earth locked 1:1 to its star on an orbit of eccentricity `e` and
/// [`LOCKED_YEAR_S`], at obliquity `obliquity_deg` with its northern spring equinox at a true
/// anomaly of 1 rad, under its own mean flux: its regime's forcing locked and its model the
/// locked energy balance.
fn locked_earth(e: f64, obliquity_deg: f64) -> CoarseInputs {
    let earth = reference::earth_like();
    let year = Seconds::new(LOCKED_YEAR_S);
    builder_of(&earth)
        .spin(Spin {
            obliquity: Radians::new(obliquity_deg.to_radians()),
            rotation_period: year,
            solar_day: None,
            equinox_true_anomaly: Radians::new(1.0),
        })
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: e,
            period: year,
        })
        .climate(ClimateRegime {
            forcing: Forcing::Locked,
            model: ClimateModelKind::LockedEnergyBalance,
            ..*earth.climate()
        })
        .build()
        .expect("a locked Earth's inputs are valid")
}

/// The unit vector at latitude `latitude_deg` and longitude `longitude_deg`, degrees.
fn direction(latitude_deg: f64, longitude_deg: f64) -> [f64; 3] {
    let (s, c) = math::sin_cos(latitude_deg.to_radians());
    let (sl, cl) = math::sin_cos(longitude_deg.to_radians());
    [c * cl, c * sl, s]
}

/// The index of the largest of `values`, finite, the first of equals.
fn argmax(values: &[f64]) -> usize {
    values
        .iter()
        .enumerate()
        .fold((0, f64::NEG_INFINITY), |(best, most), (i, &v)| {
            if v > most { (i, v) } else { (best, most) }
        })
        .0
}

/// The plan's reproduction test at sea share `sea` everywhere: every point of the geographic
/// model is its band of the zonal model to 0.01 K, in the annual mean and in every month, after
/// the same orbits.
fn reproduces_the_zonal_model(sea: f64) {
    let inputs = reference::earth_like();
    let (zonal, geographic) = (ModelGrid::zonal(), ModelGrid::geographic());
    let one = ebm::solve(
        &inputs,
        &zonal,
        &[sea; LATITUDE_BANDS],
        NOMINAL_STEP,
        Ground::SeaLevel,
        OceanTransport::None,
    )
    .climate;
    let wide = ebm::solve(
        &inputs,
        &geographic,
        &vec![sea; geographic.points()],
        NOMINAL_STEP,
        Ground::SeaLevel,
        OceanTransport::None,
    )
    .climate;
    assert_eq!(wide.orbits, one.orbits);
    assert_eq!(wide.months_k.len(), 12);
    let mut worst: f64 = 0.0;
    for p in 0..geographic.points() {
        let band = p / GEOGRAPHIC_COLUMNS;
        worst = worst.max((wide.annual_k[p] - one.annual_k[band]).abs());
        for (a, b) in wide.months_k.iter().zip(&one.months_k) {
            worst = worst.max((a[p] - b[band]).abs());
        }
    }
    eprintln!("sea {sea}: {} orbits, worst {worst:e} K", one.orbits);
    assert!(worst < 0.01, "sea {sea}: {worst} K");
}

/// The plan's reproduction test with no land–sea contrast, all land (the field until R09.T12):
/// the geographic model is the zonal one to 0.01 K ([`reproduces_the_zonal_model`]).
#[test]
fn the_geographic_model_reproduces_the_zonal_one_all_land() {
    reproduces_the_zonal_model(0.0);
}

/// The plan's reproduction test with no land–sea contrast, all sea: the geographic model is the
/// zonal one to 0.01 K ([`reproduces_the_zonal_model`]), over the eleven orbits a sea takes.
#[test]
#[ignore = "slow: eleven orbits of the 72 × 72 model, about 7 s"]
fn the_geographic_model_reproduces_the_zonal_one_all_sea() {
    reproduces_the_zonal_model(1.0);
}

/// The seasonal amplitude of point `point` of `climate`, K: its warmest month less its coldest.
fn amplitude(climate: &GridClimate, point: usize) -> f64 {
    let (low, high) = climate
        .months_k
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), month| {
            (low.min(month[point]), high.max(month[point]))
        });
    high - low
}

/// The land–sea test, on a synthetic mask (R09.T12's relief is not built): an aquaplanet, the
/// reference Earth with every point sea, but for a continent from the equator to the north pole
/// between 30° W and 30° E. At 46.25° N its land's seasonal amplitude exceeds the open sea's at
/// the same latitude half a turn away, and in its warmest month the continent is warmer than the
/// sea 10° off its coast; the sea there, warmed by it, is warmer than the open sea.
#[test]
#[ignore = "slow: ten orbits of the 72 × 72 model over a sea, about 6 s"]
fn a_continent_has_larger_seasons_and_a_warmer_summer_than_the_sea_beside_it() {
    let inputs = reference::earth_like();
    let grid = ModelGrid::geographic();
    let sea: Vec<f64> = (0..grid.points())
        .map(|p| {
            let d = grid.centre_direction(p);
            let longitude = math::atan2(d[1], d[0]).to_degrees();
            if d[2] > 0.0 && longitude.abs() < 30.0 {
                0.0
            } else {
                1.0
            }
        })
        .collect();
    let climate = ebm::solve(
        &inputs,
        &grid,
        &sea,
        NOMINAL_STEP,
        Ground::SeaLevel,
        OceanTransport::None,
    )
    .climate;
    assert!(climate.converged, "{} orbits", climate.orbits);
    let at = |longitude: f64| grid.point_of(direction(46.25, longitude));
    let (land, coast, open) = (at(0.0), at(40.0), at(180.0));
    assert!(sea[land] < 0.5 && sea[coast] > 0.5 && sea[open] > 0.5);
    let summer = climate
        .months_k
        .iter()
        .enumerate()
        .fold((0, f64::NEG_INFINITY), |(best, most), (m, month)| {
            if month[land] > most {
                (m, month[land])
            } else {
                (best, most)
            }
        })
        .0;
    let warmest = &climate.months_k[summer];
    eprintln!(
        "orbits {}; amplitude land {:.1} K, coast {:.1} K, open sea {:.1} K; month {summer}: land \
         {:.1} K, coast {:.1} K, open sea {:.1} K",
        climate.orbits,
        amplitude(&climate, land),
        amplitude(&climate, coast),
        amplitude(&climate, open),
        warmest[land],
        warmest[coast],
        warmest[open]
    );
    assert!(amplitude(&climate, land) > 1.5 * amplitude(&climate, open));
    assert!(warmest[land] > warmest[coast] + 1.0);
    assert!(warmest[coast] > warmest[open]);
    // The northern summer, as the zonal model has it.
    assert!((4..=7).contains(&summer), "{summer}");
}

/// The sea shares bin the field by point: with a continent from the equator to the north pole
/// between 30° W and 30° E and the sea flooded about it, the geographic points well inside it are
/// land, those well outside sea, and every share lies in [0, 1]; a point that holds no cell's
/// centre, about a fifth of them at level 5, takes the state of the cell its own centre falls in.
/// In tidally locked coordinates a dayside continent, within 60° of the substellar point, is land
/// at high tidally locked latitudes and sea on the nightside.
#[test]
fn the_sea_shares_bin_the_field_by_point() {
    let inputs = reference::earth_like();
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    let flood = inputs.main_liquid();
    let mask = |working: &mut Working, land: &dyn Fn([f64; 3]) -> bool| {
        for (cell, centre) in working.cells.iter_mut().zip(field.centres()) {
            let elevation = Metres::new(if land(*centre) { 100.0 } else { -100.0 });
            set_ground(cell, elevation, Metres::ZERO, flood);
        }
        working.sea_level = Metres::ZERO;
    };
    mask(&mut working, &|c| {
        c[2] > 0.0 && math::atan2(c[1], c[0]).to_degrees().abs() < 30.0
    });
    let grid = ModelGrid::geographic();
    let shares = point_sea_shares(&pass, &working, &grid, Coordinates::Geographic);
    assert!(shares.iter().all(|s| (0.0..=1.0).contains(s)));
    let at = |shares: &[f64], latitude: f64, longitude: f64| {
        shares[grid.point_of(direction(latitude, longitude))]
    };
    assert!(at(&shares, 45.0, 0.0) < 1e-12);
    assert!((at(&shares, 45.0, 90.0) - 1.0).abs() < 1e-12);
    assert!((at(&shares, -45.0, 0.0) - 1.0).abs() < 1e-12);
    // At level 5 about a fifth of the points hold no cell's centre.
    let mut holds = vec![false; grid.points()];
    for centre in field.centres() {
        holds[grid.point_of(*centre)] = true;
    }
    let mut empty = 0;
    for (point, &held) in holds.iter().enumerate() {
        if !held {
            empty += 1;
            let key = hyperion_surface::cube::PatchKey::containing(
                field.level(),
                grid.centre_direction(point),
            )
            .unwrap();
            let cell =
                &working.cells[usize::try_from(hyperion_surface::field::cell_index(key)).unwrap()];
            let expected = if seas::under_sea(cell) { 1.0 } else { 0.0 };
            assert!((shares[point] - expected).abs() < 1e-15, "point {point}");
        }
    }
    assert!(empty > grid.points() / 8, "{empty}");
    mask(&mut working, &|c| substellar_cosine(c) > 0.5);
    let locked = ModelGrid::locked();
    let shares = point_sea_shares(&pass, &working, &locked, Coordinates::TidallyLocked);
    for column in 0..LOCKED_COLUMNS {
        assert!(shares[locked.point(LATITUDE_BANDS - 4, column)] < 1e-12);
        assert!((shares[locked.point(20, column)] - 1.0).abs() < 1e-12);
    }
}

/// The plan's first locked test: a world locked 1:1 to its star with no seasonal forcing (a
/// circular orbit, no obliquity) has one month, and is warmest at the substellar point and coldest
/// at the antistellar one: on its grid, whose bands warm towards the tidally locked pole and whose
/// columns agree, and in the climate cells the step writes from it in tidally locked coordinates
/// (the model's own field, before R09.T13.d imposes plan 14's contrasts).
#[test]
fn a_locked_world_is_warmest_at_the_substellar_point() {
    let inputs = locked_earth(0.0, 0.0);
    assert_eq!(inputs.months(), 1);
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    let grid = ModelGrid::locked();
    let solution = ebm::solve_locked(
        &inputs,
        &grid,
        &vec![0.0; grid.points()],
        NOMINAL_STEP,
        Ground::SeaLevel,
        OceanTransport::None,
    );
    let climate = &solution.climate;
    let _clamped = super::write_climate(
        &pass,
        &mut working,
        &grid,
        Coordinates::TidallyLocked,
        climate,
    );
    assert_eq!(climate.months_k.len(), 1);
    let bands = grid.band_means(&climate.annual_k);
    for (k, row) in climate
        .annual_k
        .as_chunks::<LOCKED_COLUMNS>()
        .0
        .iter()
        .enumerate()
    {
        assert!(row.iter().all(|t| (t - bands[k]).abs() < 1e-9), "band {k}");
    }
    assert!(bands.windows(2).all(|w| w[1] >= w[0]), "{bands:?}");
    eprintln!(
        "substellar {:.1} K, terminator {:.1} K, antistellar {:.1} K, mean {:.2} K",
        bands[LATITUDE_BANDS - 1],
        f64::midpoint(bands[35], bands[36]),
        bands[0],
        grid.mean(&climate.annual_k)
    );
    let temperatures: Vec<f64> = working
        .climate
        .iter()
        .map(|c| c.sea_level_temperature.value())
        .collect();
    let (warmest, coldest) = (
        argmax(&temperatures),
        argmax(&temperatures.iter().map(|t| -t).collect::<Vec<f64>>()),
    );
    let near = math::cos(6_f64.to_radians());
    assert!(substellar_cosine(climate_grid.centres()[warmest]) > near);
    assert!(substellar_cosine(climate_grid.centres()[coldest]) < -near);
    for (state, centre) in working.climate.iter().zip(climate_grid.centres()) {
        let model = Coordinates::TidallyLocked.to_model(*centre);
        let expected = grid.stencil_at(model).apply_clamped(&climate.annual_k);
        assert!((state.sea_level_temperature.value() - expected).abs() < 1e-12);
        assert!(state.month_anomaly_k.iter().all(|a| a.abs() < 1e-300));
    }
}

/// The plan's second locked test, for a world locked 1:1 to its star on an orbit of eccentricity
/// `e` at obliquity `obliquity` degrees, one of them above 0: it has twelve months, and its months
/// differ. On the eccentric orbit the globe is warmest in the half-year after periapsis and
/// coldest in the half after apoapsis (the response lags the flux by under a quarter of the
/// year); under obliquity the northern dayside is warmer than the southern at the northern
/// solstice and colder at the southern. The step writes the months' anomalies to the climate
/// cells in tidally locked coordinates.
fn a_locked_world_has_twelve_months_that_differ(e: f64, obliquity: f64) {
    let grid = ModelGrid::locked();
    let (field, climate_grid) = grids(5);
    let inputs = locked_earth(e, obliquity);
    assert_eq!(inputs.months(), 12);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    assert_eq!(working.months, 12);
    let solution = ebm::solve_locked(
        &inputs,
        &grid,
        &vec![0.0; grid.points()],
        NOMINAL_STEP,
        Ground::SeaLevel,
        OceanTransport::None,
    );
    let climate = &solution.climate;
    assert!(climate.converged, "{} orbits", climate.orbits);
    assert_eq!(climate.months_k.len(), 12);
    let spread = (0..grid.points()).fold(0.0, |most: f64, p| most.max(amplitude(climate, p)));
    let means: Vec<f64> = climate.months_k.iter().map(|m| grid.mean(m)).collect();
    eprintln!(
        "e {e}, obliquity {obliquity}°: {} orbits, largest amplitude {spread:.1} K, monthly \
         means {means:.1?}",
        climate.orbits
    );
    assert!(spread > 1.0);
    // The warm spot stays at the substellar point in the annual mean.
    assert!(argmax(&climate.annual_k) / LOCKED_COLUMNS >= LATITUDE_BANDS - 4);
    if e > 0.0 {
        let warmest = argmax(&means);
        let coldest = argmax(&means.iter().map(|m| -m).collect::<Vec<f64>>());
        assert!(warmest < 6 && coldest >= 6, "{warmest}, {coldest}");
    } else {
        // The solstices at true anomalies 1 rad ± 90°, months 4 and 10 of a circular orbit.
        let north = Coordinates::TidallyLocked.to_model(direction(30.0, 0.0));
        let south = Coordinates::TidallyLocked.to_model(direction(-30.0, 0.0));
        let lean = |month: usize| {
            let m = &climate.months_k[month];
            grid.stencil_at(north).apply(m) - grid.stencil_at(south).apply(m)
        };
        eprintln!("north less south: {:.1} K, {:.1} K", lean(4), lean(10));
        assert!(lean(4) > 1.0 && lean(10) < -1.0);
    }
    let _clamped = super::write_climate(
        &pass,
        &mut working,
        &grid,
        Coordinates::TidallyLocked,
        climate,
    );
    let cells = working.climate.iter().fold(0.0, |most: f64, c| {
        let (low, high) = c
            .month_anomaly_k
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &a| {
                (l.min(a), h.max(a))
            });
        most.max(high - low)
    });
    assert!(cells > 1.0, "{cells}");
}

/// The plan's second locked test on an eccentric orbit, e = 0.05 under P14.T14.b's 0.1, above
/// which a lock is 3:2 ([`a_locked_world_has_twelve_months_that_differ`]).
#[test]
fn a_locked_world_on_an_eccentric_orbit_has_twelve_months_that_differ() {
    a_locked_world_has_twelve_months_that_differ(0.05, 0.0);
}

/// The plan's second locked test at an obliquity of 20° on a circular orbit
/// ([`a_locked_world_has_twelve_months_that_differ`]).
#[test]
fn a_locked_world_with_obliquity_has_twelve_months_that_differ() {
    a_locked_world_has_twelve_months_that_differ(0.0, 20.0);
}

mod cold_traps;
mod earth_land;
mod hydrology;
mod normalisation;
mod thin_air;

/// R09.T13.c's tests: the radiative model with thermal inertia and the isothermal surface.
pub(super) mod other_regimes {
    use super::*;
    use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
    use crate::planetary::hooks::surface::night_floor;
    use crate::planetary::surface::coarse_pass;
    use crate::planetary::surface::inputs::{GasShare, ThermalRegime};
    use crate::planetary::surface::steps::climate::airless::{
        self, End, EndColumn, RadiativeSolution,
    };
    use crate::planetary::surface::steps::climate::isothermal;
    use crate::substance::SubstanceKey;
    use crate::units::consts::{SOLAR_CONSTANT_W_PER_M2, STEFAN_BOLTZMANN};
    use crate::units::{Kelvin, Pascals, WattsPerSquareMetre};

    /// The Sun's light averaged over an orbit of semi-major axis `a_au` and eccentricity `e`,
    /// S☉ ÷ (a² √(1 − e²)) (P14.T12's orbit average).
    fn solar_flux(a_au: f64, e: f64) -> WattsPerSquareMetre {
        WattsPerSquareMetre::new(SOLAR_CONSTANT_W_PER_M2 / (a_au * a_au * (1.0 - e * e).sqrt()))
    }

    /// Plan 14's mean for an airless body under `flux` at Bond albedo `albedo`: its equilibrium
    /// temperature ((1 − A) F ÷ 4σ)^¼ at unit emissivity (P14.T13.c).
    fn equilibrium(flux: WattsPerSquareMetre, albedo: f64) -> Kelvin {
        Kelvin::new(
            ((1.0 - albedo) * flux.value() / (4.0 * STEFAN_BOLTZMANN))
                .sqrt()
                .sqrt(),
        )
    }

    /// The end column of `solution` at `end`, the first of them.
    fn end_of(solution: &RadiativeSolution, end: End) -> &EndColumn {
        solution.ends.iter().find(|c| c.end == end).unwrap()
    }

    /// The mean of `solution` in the brightness kind of decision-r09-t13d-normalisation §2.2:
    /// (Σ Ω (εσ⟨T⁴⟩ − `E_nf`) ÷ σ Σ Ω)^¼ over the grid's points, from their moments.
    fn brightness_mean(solution: &RadiativeSolution) -> f64 {
        let excess: Vec<f64> = solution
            .moments
            .iter()
            .map(|m| {
                solution.emissivity * STEFAN_BOLTZMANN * m.fourth_power_mean(0.0)
                    - solution.floor_flux_w_m2
            })
            .collect();
        (solution.grid.mean(&excess) / STEFAN_BOLTZMANN)
            .sqrt()
            .sqrt()
    }

    /// The reference Moon's inputs locked 1:1 to its star on its own orbit (1 au, e 0.0167, its
    /// 1.535° obliquity), its regime's forcing locked: an airless world with twelve months, under
    /// plan 14's own airless contrasts ([`with_airless_contrasts`]).
    pub(super) fn locked_moon() -> CoarseInputs {
        let moon = reference::moon_like();
        let year = moon.seasonal_orbit().unwrap().period;
        with_airless_contrasts(
            &builder_of(&moon)
                .spin(Spin {
                    rotation_period: year,
                    solar_day: None,
                    ..*moon.spin()
                })
                .climate(ClimateRegime {
                    forcing: Forcing::Locked,
                    ..*moon.climate()
                })
                .build()
                .expect("a locked Moon's inputs are valid"),
        )
    }

    /// The plan's test, "an airless world's day–night contrast exceeds 300 K", honestly
    /// (research-c §4): a locked airless world, the reference Moon locked 1:1 at 1 au, whose
    /// substellar band's annual mean is more than 300 K above its antistellar band's, on the
    /// model's grid and in its climate cells. A lunar-like rotator falls just short: its equator's
    /// noon less its predawn is about 290 K ([`the_reference_moon_s_equator_matches_diviner`]).
    #[test]
    fn an_airless_world_s_day_night_contrast_exceeds_300_k() {
        let inputs = locked_moon();
        assert_eq!(inputs.months(), 12);
        let solution = airless::solve(&inputs);
        assert_eq!(solution.coordinates, Coordinates::TidallyLocked);
        assert!(
            solution.climate.converged,
            "{} orbits",
            solution.climate.orbits
        );
        let bands = solution.grid.band_means(&solution.climate.annual_k);
        let (substellar, antistellar) = (bands[LATITUDE_BANDS - 1], bands[0]);
        let floor = night_floor(inputs.heat_flow()).value();
        eprintln!(
            "{} orbits: substellar {substellar:.1} K, antistellar {antistellar:.1} K (floor \
             {floor:.1} K), contrast {:.1} K",
            solution.climate.orbits,
            substellar - antistellar
        );
        assert!(substellar - antistellar > 300.0);
        assert!(
            (antistellar - floor).abs() < 0.5,
            "{antistellar} against {floor}"
        );
        let (field, climate_grid) = grids(6);
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
        let mut working = Working::initial(&pass);
        run(&pass, &mut working);
        let temperatures: Vec<f64> = working
            .climate
            .iter()
            .map(|c| c.sea_level_temperature.value())
            .collect();
        let negated: Vec<f64> = temperatures.iter().map(|t| -t).collect();
        let (warmest, coldest) = (argmax(&temperatures), argmax(&negated));
        assert!(temperatures[warmest] - temperatures[coldest] > 300.0);
        let near = math::cos(6_f64.to_radians());
        assert!(substellar_cosine(climate_grid.centres()[warmest]) > near);
        assert!(substellar_cosine(climate_grid.centres()[coldest]) < 0.0);
    }

    /// A Mercury in its 3:2 resonance (research-c §4's slow rotator: 0.387 au, e 0.2056, a sidereal
    /// day of 58.646 days and a solar day of 175.94, two orbits; NASA's Mercury fact sheet, from
    /// memory) of the Moon's surface, which steps in true time over phase columns, its solar day
    /// being longer than its months.
    pub(in crate::planetary::surface::steps::climate) fn mercury_like() -> CoarseInputs {
        let moon = reference::moon_like();
        let (e, a) = (0.2056, 0.387_098);
        let flux = solar_flux(a, e);
        builder_of(&moon)
            .spin(Spin {
                obliquity: Radians::new(0.034_f64.to_radians()),
                rotation_period: Seconds::new(58.646 * SECONDS_PER_DAY),
                solar_day: Some(Seconds::new(175.94 * SECONDS_PER_DAY)),
                equinox_true_anomaly: Radians::ZERO,
            })
            .seasonal_orbit(SeasonalOrbit {
                eccentricity: e,
                period: Seconds::new(87.969 * SECONDS_PER_DAY),
            })
            .host_flux(flux)
            .mean_surface_temperature(equilibrium(flux, 0.11))
            .build()
            .unwrap()
    }

    /// The 3:2 Mercury of [`mercury_like`]: its equator's warmest less its coldest exceeds 300 K
    /// (research-c's prototype: about 540 K at the semi-major axis).
    #[test]
    fn a_slow_rotator_s_equator_spans_more_than_300_k() {
        let inputs = mercury_like();
        let solution = airless::solve(&inputs);
        assert!(
            solution.climate.converged,
            "{} orbits",
            solution.climate.orbits
        );
        let equator = end_of(&solution, End::Equator);
        let range = equator.moments.warmest_k - equator.moments.coldest_k;
        eprintln!(
            "{} orbits: equator {:.1}–{:.1} K, range {range:.1} K, annual {:.1} K",
            solution.climate.orbits,
            equator.moments.coldest_k,
            equator.moments.warmest_k,
            equator.annual_k
        );
        assert!(range > 300.0, "{range}");
        assert!(
            (brightness_mean(&solution) - inputs.mean_surface_temperature().value()).abs() < 1.0
        );
    }

    /// The reference Moon's equator against Diviner, without tuning: its warmest sample within
    /// 5 K of Williams et al.'s "∼387–397 K at the equator" and its coldest of their "∼95 K just
    /// before sunrise" (2017, Icarus 283, 300, abstract, read), so 382–402 K and 90–100 K; its
    /// range, about 290 K, falls short of 300 K. Its mean in the brightness kind is plan 14's
    /// equilibrium temperature to 0.5 K, by energy.
    #[test]
    fn the_reference_moon_s_equator_matches_diviner() {
        let inputs = reference::moon_like();
        let solution = airless::solve(&inputs);
        assert_eq!(solution.coordinates, Coordinates::Geographic);
        assert!(
            solution.climate.converged,
            "{} orbits",
            solution.climate.orbits
        );
        let equator = end_of(&solution, End::Equator);
        let (noon, predawn) = (equator.moments.warmest_k, equator.moments.coldest_k);
        let brightness = brightness_mean(&solution);
        eprintln!(
            "{} orbits: equator warmest {noon:.1} K, coldest {predawn:.1} K, range {:.1} K; \
             annual {:.1} K; brightness mean {brightness:.2} K against {:.2} K",
            solution.climate.orbits,
            noon - predawn,
            equator.annual_k,
            inputs.mean_surface_temperature().value()
        );
        assert!((382.0..402.0).contains(&noon), "{noon}");
        assert!((90.0..100.0).contains(&predawn), "{predawn}");
        assert!(noon - predawn < 300.0);
        assert!((brightness - inputs.mean_surface_temperature().value()).abs() < 0.5);
        // The poles are colder than the equator in the annual mean, and both at least the floor.
        let floor = solution.lower_bound_k;
        for end in [End::NorthPole, End::SouthPole] {
            let pole = end_of(&solution, end);
            assert!(pole.annual_k < equator.annual_k - 50.0);
            assert!(pole.moments.coldest_k > floor - 0.5, "{end:?}");
        }
    }

    /// The thin air's frost floor on the reference Mars: its bulk carbon dioxide's frost point at
    /// its partial pressure, 605 Pa, lies at about 148 K (Kieffer 2013's 148.0 K at 636 Pa by
    /// James et al.'s curve, as research-c §8 computed it), and holds both poles' polar nights
    /// there for months, where the bare regolith would fall far below; the model's lower bound is
    /// it. The slab's β holds plan 14's 214 K in the brightness kind, to 1 K.
    #[test]
    fn the_thin_air_holds_mars_s_polar_night_at_the_frost_point() {
        let inputs = reference::mars_like();
        let solution = airless::solve(&inputs);
        assert!(
            solution.climate.converged,
            "{} orbits",
            solution.climate.orbits
        );
        let frost = solution.air.frost.expect("Mars's carbon dioxide condenses");
        let point = frost.point_k;
        let brightness = brightness_mean(&solution);
        eprintln!(
            "{} orbits: β {:.3}, frost point {point:.2} K, latent heat {:.0} J kg⁻¹, brightness \
             mean {brightness:.2} K",
            solution.climate.orbits, solution.air.absorptivity, frost.latent_heat_j_per_kg
        );
        assert!((146.0..150.0).contains(&point), "{point}");
        assert!((5.0e5..6.5e5).contains(&frost.latent_heat_j_per_kg));
        assert!((solution.lower_bound_k - point).abs() < 1e-12);
        assert!((0.2..0.3).contains(&solution.air.absorptivity));
        assert!((brightness - 214.0).abs() < 1.0, "{brightness}");
        for end in [End::NorthPole, End::SouthPole] {
            let pole = end_of(&solution, end);
            hyperion_testkit::float::assert_same_bits(pole.moments.coldest_k, point);
            let held = pole.months_k.iter().filter(|m| **m < point + 0.5).count();
            eprintln!("{end:?}: months {:.1?}", pole.months_k);
            assert!(held >= 3, "{end:?}: {held} months at the frost point");
        }
        assert!(
            solution
                .climate
                .months_k
                .iter()
                .flatten()
                .all(|t| *t >= point - 1e-9)
        );
        // With no air, the bare regolith's polar night falls far below the frost point.
        let bare = airless::solve(
            &builder_of(&inputs)
                .surface_pressure(Pascals::ZERO)
                .build()
                .unwrap(),
        );
        assert!(bare.air.frost.is_none());
        let pole = end_of(&bare, End::NorthPole);
        eprintln!("bare: north pole coldest {:.1} K", pole.moments.coldest_k);
        assert!(pole.moments.coldest_k < point - 30.0);
    }

    /// The step writes each climate cell from the radiative model on the reference Moon: its
    /// records are the model's, corrected on its grid by R09.T13.d's (c, a, b), held at its lower
    /// bound, interpolated and held to each stencil's range, its months' anomalies the month's
    /// less the annual, and the lapse rate stays 0.
    #[test]
    fn the_step_writes_each_climate_cell_from_the_radiative_model() {
        let inputs = reference::moon_like();
        assert_eq!(
            inputs.climate().model,
            ClimateModelKind::RadiativeEquilibrium
        );
        let (field, climate_grid) = grids(6);
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
        let mut working = Working::initial(&pass);
        let diagnostics = super::super::run_diagnosed(&pass, &mut working);
        let solution = airless::solve(&inputs);
        let (correction, _) = normalise::radiative(&inputs, &solution);
        assert_same_bits(correction.mean_k, diagnostics.correction.mean_k);
        assert_same_bits(
            correction.equator_pole_k,
            diagnostics.correction.equator_pole_k,
        );
        assert_same_bits(correction.day_night_k, diagnostics.correction.day_night_k);
        let mut climate = solution.climate.clone();
        normalise::apply(
            &mut climate,
            &solution.grid,
            solution.coordinates,
            &correction,
        );
        let _raised = normalise::floor_records(&mut climate, solution.lower_bound_k);
        // What the step writes on an airless world with no ice: the climate cells, every one off
        // the initial state's uniform mean, and a lapse rate of nothing; no ice and no liquid.
        let initial = inputs.mean_surface_temperature().value();
        assert!(
            working
                .climate
                .iter()
                .all(|c| (c.sea_level_temperature.value() - initial).abs() > 1e-6),
            "a climate cell left at the initial {initial} K"
        );
        assert!(
            working
                .cells
                .iter()
                .all(|c| c.ice_entry.is_none() && c.liquid_entry.is_none())
        );
        assert_same_bits(working.lapse_rate_k_per_m, 0.0);
        let floor = solution.lower_bound_k;
        let mut spread: f64 = 0.0;
        for (state, centre) in working.climate.iter().zip(climate_grid.centres()) {
            let annual = ModelGrid::stencil(centre[2]).apply_clamped(&climate.annual_k);
            assert!((state.sea_level_temperature.value() - annual).abs() < 1e-12);
            assert!(state.sea_level_temperature.value() >= floor);
            for (month, &anomaly) in climate.months_k.iter().zip(&state.month_anomaly_k) {
                let value = ModelGrid::stencil(centre[2]).apply_clamped(month);
                assert!((anomaly - (value - annual)).abs() < 1e-12);
                spread = spread.max(anomaly.abs());
            }
        }
        assert!(spread > 0.1, "the months differ: {spread}");
    }

    /// A Venus-like body (the reference Earth's figure under 9.2 MPa of carbon dioxide at 737 K,
    /// Venus's orbit and spin; NASA's Venus fact sheet, from memory), whose regime names the
    /// isothermal surface.
    pub(super) fn venus_like() -> CoarseInputs {
        let earth = reference::earth_like();
        let key = |k: &str| SubstanceKey::new(k).unwrap();
        builder_of(&earth)
            .liquids([])
            .ices([])
            .condensates([])
            .mean_surface_temperature(Kelvin::new(737.0))
            .equator_pole_contrast_k(2.8)
            .day_night_contrast_k(1.0)
            .surface_pressure(Pascals::new(9.2e6))
            .gases([
                GasShare {
                    substance: key("CO2"),
                    mole_fraction: 0.965,
                },
                GasShare {
                    substance: key("N2"),
                    mole_fraction: 0.035,
                },
            ])
            .surface(SurfaceState::RunawayGreenhouse, SurfaceMaterial::Rock)
            .spin(Spin {
                obliquity: Radians::new(2.64_f64.to_radians()),
                rotation_period: Seconds::new(5_832.6 * 3_600.0),
                solar_day: Some(Seconds::new(116.75 * SECONDS_PER_DAY)),
                equinox_true_anomaly: Radians::ZERO,
            })
            .seasonal_orbit(SeasonalOrbit {
                eccentricity: 0.0068,
                period: Seconds::new(224.701 * SECONDS_PER_DAY),
            })
            .host_flux(solar_flux(0.723_32, 0.0068))
            .climate(ClimateRegime {
                thermal: ThermalRegime::Efficient,
                forcing: Forcing::SlowRotator,
                condensable: None,
                model: ClimateModelKind::Isothermal,
            })
            .build()
            .expect("a Venus's inputs are valid")
    }

    /// The plan's test as ruled (decision-r09-t13d-normalisation, plan text (b)): the isothermal
    /// model's own field, before T13.d imposes plan 14's contrasts, varies by under 1 K, over the
    /// climate cells it writes and its twelve months, at plan 14's mean; and a locked world's lies
    /// on the tidally locked grid.
    #[test]
    fn the_isothermal_model_s_own_field_varies_by_under_1_k() {
        let inputs = venus_like();
        assert_eq!(inputs.months(), 12);
        let (field, climate_grid) = grids(6);
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
        let mut working = Working::initial(&pass);
        let (grid, coordinates) = airless::frame(&inputs);
        let own = isothermal::solve(&inputs, &grid);
        let _clamped = super::super::write_climate(&pass, &mut working, &grid, coordinates, &own);
        let (low, high) =
            working
                .climate
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), c| {
                    let t = c.sea_level_temperature.value();
                    let months = c.month_anomaly_k.iter().map(|a| t + a);
                    months.fold((low.min(t), high.max(t)), |(l, h), m| (l.min(m), h.max(m)))
                });
        eprintln!("isothermal field {low:.6}–{high:.6} K");
        assert!(high - low < 1.0);
        assert!((low - 737.0).abs() < 1e-9 && (high - 737.0).abs() < 1e-9);
        let locked = builder_of(&inputs)
            .spin(Spin {
                solar_day: None,
                ..*inputs.spin()
            })
            .build()
            .unwrap();
        let (grid, coordinates) = airless::frame(&locked);
        assert_eq!(coordinates, Coordinates::TidallyLocked);
        let climate = isothermal::solve(&locked, &grid);
        assert_eq!(climate.annual_k.len(), grid.points());
        assert_eq!(climate.months_k.len(), 12);
    }

    /// A locked lava world (research-c §7; Léger et al. 2011, Icarus 213, 1, §5.3, and Kite et
    /// al. 2016, ApJ 828, 80, eq. 1, read by research-c): the reference Moon's crust locked 1:1 to
    /// the Sun at 0.04 au, airless, on a circular orbit with no obliquity, its lid the feldspathic
    /// highlands' 1,533.15 K solidus. Its `melt_area_fraction` is plan 14's eyeball's,
    /// ½[1 − (σ`T_sol`⁴ − `E_n`) ÷ (3/2)(`E_d` − `E_n`)] with Cowan and Agol's day at no
    /// redistribution, `E_d` = (8/3) σ`T_eq`⁴, and the night at its floor (P14.T51.c's
    /// `melt_area`, as its test computes it).
    pub(super) fn lava_world() -> (CoarseInputs, f64) {
        let moon = reference::moon_like();
        let flux = solar_flux(0.04, 0.0);
        let mean = equilibrium(flux, 0.11);
        let floor = night_floor(moon.heat_flow()).value();
        let solidus = moon
            .palette()
            .get(moon.lid_entry())
            .unwrap()
            .transition
            .value();
        let (e_n, e_d) = (
            STEFAN_BOLTZMANN * math::powi(floor, 4),
            8.0 / 3.0 * STEFAN_BOLTZMANN * math::powi(mean.value(), 4),
        );
        let melt = (0.5
            * (1.0 - (STEFAN_BOLTZMANN * math::powi(solidus, 4) - e_n) / (1.5 * (e_d - e_n))))
            .clamp(0.0, 0.5);
        let year = Seconds::new(math::powf(0.04, 1.5) * 365.256 * SECONDS_PER_DAY);
        let inputs = builder_of(&moon)
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
            .host_flux(flux)
            .mean_surface_temperature(mean)
            .climate(ClimateRegime {
                forcing: Forcing::Locked,
                ..*moon.climate()
            })
            .crust(crate::planetary::surface::inputs::CrustInputs {
                melt_area_fraction: melt,
                ..*moon.crust()
            })
            .build()
            .expect("the lava world's inputs are valid");
        (inputs, melt)
    }

    /// The plan's lava-world test: a locked lava world's melt, its cells above the transition of
    /// their own crust's entry by this task's per-cell temperature (here every cell the lid's,
    /// on a stagnant lid), is exactly a dayside region about the substellar point, the cap where
    /// the closed form's εσT⁴ = (1 − A) F cos γ + `E_nf` is above the solidus, and its whole
    /// nightside is solid, at its floor beyond the terminator's stencils. Its area is compared
    /// with the input's `melt_area_fraction` and printed, not imposed (R09's Risks record the
    /// comparison). The field quantises. The world takes plan 14's own contrasts
    /// ([`with_airless_contrasts`]), so that R09.T13.d's corrections are the model's
    /// discretisation, not the reference Moon's placeholders'.
    #[test]
    fn a_locked_lava_world_melts_on_its_dayside_alone() {
        let (placeholders, melt_input) = lava_world();
        let inputs = with_airless_contrasts(&placeholders);
        assert_eq!(inputs.months(), 1);
        let (field, climate_grid) = grids(7);
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
        let mut working = Working::initial(&pass);
        run(&pass, &mut working);
        assert!(
            working
                .cells
                .iter()
                .all(|c| c.crust == hyperion_surface::field::Crust::Lid)
        );
        let solidus = inputs
            .palette()
            .get(inputs.lid_entry())
            .unwrap()
            .transition
            .value();
        let solution = airless::solve(&inputs);
        let normal = (1.0 - 0.11) * inputs.host_flux().value();
        let edge = (solution.emissivity * STEFAN_BOLTZMANN * math::powi(solidus, 4)
            - solution.floor_flux_w_m2)
            / normal;
        let (mut melt_area, mut total) = (0.0, 0.0);
        for ((state, centre), &omega) in working
            .climate
            .iter()
            .zip(climate_grid.centres())
            .zip(climate_grid.solid_angles_sr())
        {
            let cosine = substellar_cosine(*centre);
            let t = state.sea_level_temperature.value();
            let molten = t > solidus;
            total += omega;
            if molten {
                melt_area += omega;
                assert!(cosine > 0.0, "melt on the nightside at cos γ {cosine}");
            }
            if cosine > edge + 0.04 {
                assert!(molten, "solid at cos γ {cosine}, {t} K");
            } else if cosine < edge - 0.04 {
                assert!(!molten, "melt at cos γ {cosine}, {t} K");
            }
            // Beyond the reach of the stencils, which read two bands of 2.5° on.
            if cosine < -0.1 {
                assert!(t < solution.lower_bound_k + 0.5, "{t} at {cosine}");
            }
        }
        let share = melt_area / total;
        eprintln!(
            "pool edge cos γ {edge:.4} ({:.2}°): melt {share:.4} of the surface, the closed \
             form's {:.4}, against the input's {melt_input:.4}",
            math::acos(edge).to_degrees(),
            0.5 * (1.0 - edge)
        );
        assert!((share - 0.5 * (1.0 - edge)).abs() < 0.01);
        let quantised = coarse_pass(reference::SEED, &inputs);
        assert_eq!(
            hyperion_surface::field::FieldView::header(&quantised).months(),
            1
        );
    }
}
