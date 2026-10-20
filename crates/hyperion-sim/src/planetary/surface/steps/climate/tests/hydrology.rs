//! R09.T13.e's tests: the precipitation heuristic and the winds, wired into the step
//! (decision-r09-t13ge §2.13's, with the water cycle's own). The orographic model's own (the ridge,
//! the inland decay, the runoff's share) are `lfpm`'s.

use hyperion_surface::field::{
    FieldView, LogPrecipitation, PaletteRole, PrecipitationSource, Wind, coarse_level,
};
use hyperion_surface::spheroid::Spheroid;

use super::super::coordinates::Coordinates;
use super::super::ebm::EnergyBalance;
use super::super::grid::{LATITUDE_BANDS, ModelGrid};
use super::super::hydrology::{self, Evaporation, MonthCycle};
use super::super::lfpm::{
    self, DEEP_MEASURE, MAX_RESTARTS, Parameters, RATE_FLOOR_SHARE, TOLERANCE,
};
use super::super::wind;
use super::super::{
    Diagnostics, annual_runoff_kg_m2_s, energy_balance, finish, month_shares, normalised,
    run_diagnosed, seas,
};
use super::normalisation::titan_with_basins;
use super::other_regimes::venus_like;
use super::{
    EARTH_HYPSOMETRY_M, SEA_LEVEL_LAND, builder_of, grids, locked_earth, set_ground, synthetic_sea,
};
use crate::math;
use crate::planetary::surface::grid::{CellGraph, parent_index};
use crate::planetary::surface::inputs::{CoarseInputs, SeasonalOrbit, Spin};
use crate::planetary::surface::steps::{Pass, Working};
use crate::planetary::surface::{quantise, reference, steps};
use crate::units::{Metres, Pascals, Radians, Seconds};
use hyperion_testkit::float::assert_same_bits;

/// Seconds in a day, for rates in mm of water a day (1 kg m⁻² of water is 1 mm).
const DAY_S: f64 = 86_400.0;

/// GPCP v2.3's annual zonal-mean precipitation over 1991–2020, mm/day, at each 2.5° band from
/// 88.75° S: the long-term monthly means of NOAA PSL (`precip.mon.ltm.1991-2020.nc`), zonal means
/// of the 2.5° grid, months weighted by their days (the lane's `e2/gpcp-zonal.txt`, read; global
/// mean 2.691 mm/day). Adler, R. F., et al. 2003, J. Hydrometeor. 4, 1147–1167, the dataset's own
/// citation. Printed beside the model, not asserted.
const GPCP_ANNUAL_MM_PER_DAY: [f64; LATITUDE_BANDS] = [
    0.38, 0.37, 0.35, 0.37, 0.49, 0.67, 0.79, 0.89, 1.20, 1.47, 2.03, 2.66, 3.11, 3.20, 3.07, 2.91,
    2.80, 2.89, 2.96, 2.97, 2.91, 2.70, 2.54, 2.38, 2.21, 2.06, 1.91, 1.87, 1.97, 2.26, 2.64, 3.01,
    3.61, 3.97, 4.11, 3.91, 4.10, 5.15, 6.07, 5.42, 3.94, 2.78, 2.22, 1.96, 1.78, 1.70, 1.83, 1.96,
    2.19, 2.42, 2.62, 2.78, 2.76, 2.68, 2.56, 2.50, 2.48, 2.41, 2.41, 2.38, 2.09, 1.90, 1.65, 1.45,
    1.15, 0.97, 0.80, 0.65, 0.59, 0.50, 0.48, 0.50,
];

/// The NCEP/NCAR Reanalysis 1's annual zonal-mean 10 m meridional wind over 1991–2020 in the
/// trades, m/s, positive northward: the 5° bins at 12.5° N, 17.5° N, 12.5° S and 17.5° S, by their
/// centres, all longitudes, computed from NOAA PSL's long-term monthly means (the lane's scratch
/// `e/zonal-winds-ncep-r1.txt`). The dataset's citation: E. Kalnay et al. 1996, "The NCEP/NCAR
/// 40-Year Reanalysis Project", Bull. Amer. Meteor. Soc. 77, 437–471 (PSL's dataset page prints
/// 437–470; the pages here are the publisher's record, through Crossref). Printed beside the
/// model, not asserted (`NOTICE`, "Test data (not shipped)").
const NCEP_TRADES_V_M_S: [(f64, f64); 4] =
    [(12.5, -1.05), (17.5, -1.27), (-12.5, 1.83), (-17.5, 1.77)];

/// The grid a test runs the energy-balance model on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grid {
    /// The step's own: the geographic or the locked model, through [`run_diagnosed`].
    Own,
    /// The zonal model in geographic coordinates, through [`finish`]: on a world with no land–sea
    /// contrast in longitude the geographic model's to 0.01 K (R09.T13.b's reproduction test), at
    /// a seventy-second of its cost.
    Zonal,
}

/// A world the step ran on: its grids, inputs, final state and what the step did.
struct Run {
    field: CellGraph,
    climate: CellGraph,
    inputs: CoarseInputs,
    working: Working,
    diagnostics: Diagnostics,
}

impl Run {
    /// The pass over the run's world.
    fn pass(&self) -> Pass<'_> {
        Pass::new(reference::SEED, &self.inputs, &self.field, &self.climate)
    }

    /// The water cycle's diagnostics.
    fn water(&self) -> &hydrology::Diagnostics {
        self.diagnostics
            .water
            .as_ref()
            .expect("an energy-balance world has a water cycle")
    }

    /// Each climate cell's annual mean precipitation, kg m⁻² s⁻¹: its months' rates weighted by
    /// their shares of the year.
    fn annual_precipitation(&self) -> Vec<f64> {
        let shares = month_shares(&self.pass(), usize::from(self.working.months));
        self.working
            .climate
            .iter()
            .map(|state| {
                shares
                    .iter()
                    .zip(&state.month_precipitation_kg_m2_s)
                    .fold(0.0, |sum, (&s, &p)| sum + s * p)
            })
            .collect()
    }

    /// The solid-angle mean over the climate cells whose centres' latitude lies within `low` to
    /// `high`, degrees, of `values` (one a climate cell).
    fn window(&self, values: &[f64], low: f64, high: f64) -> f64 {
        let (sum, area) = self
            .climate
            .centres()
            .iter()
            .zip(self.climate.solid_angles_sr())
            .zip(values)
            .filter(|((c, _), _)| (low..=high).contains(&latitude_deg(**c)))
            .fold((0.0, 0.0), |(s, a), ((_, &omega), &v)| {
                (s + omega * v, a + omega)
            });
        sum / area
    }

    /// Each 2.5° band's solid-angle mean of `values` (one a climate cell), by the band of each
    /// climate cell's centre.
    fn zonal(&self, values: &[f64]) -> Vec<f64> {
        let (mut sums, mut areas) = (vec![0.0; LATITUDE_BANDS], vec![0.0; LATITUDE_BANDS]);
        for ((centre, &omega), &v) in self
            .climate
            .centres()
            .iter()
            .zip(self.climate.solid_angles_sr())
            .zip(values)
        {
            let band = ModelGrid::band_of(centre[2]);
            sums[band] += omega * v;
            areas[band] += omega;
        }
        sums.iter().zip(&areas).map(|(s, a)| s / a).collect()
    }

    /// The annual energy-flux equator's latitude, degrees.
    fn annual_efe_deg(&self) -> f64 {
        degrees(self.water().annual_efe_sine)
    }

    /// Each climate cell's annual-mean 10 m wind towards the north and the east, m/s, from its
    /// records: each month's decoded speed along its azimuth, weighted by the month's share.
    fn annual_winds(&self) -> (Vec<f64>, Vec<f64>) {
        let shares = month_shares(&self.pass(), usize::from(self.working.months));
        self.working
            .climate
            .iter()
            .map(|state| {
                shares.iter().zip(&state.month_wind).fold(
                    (0.0, 0.0),
                    |(north, east), (&s, sample)| {
                        let (sin, cos) = math::sin_cos(sample.azimuth.value());
                        let speed = sample.speed.value();
                        (north + s * speed * cos, east + s * speed * sin)
                    },
                )
            })
            .unzip()
    }
}

/// The latitude of the body-frame unit vector `d`, degrees.
fn latitude_deg(d: [f64; 3]) -> f64 {
    degrees(d[2])
}

/// The latitude of sine `x`, degrees.
fn degrees(x: f64) -> f64 {
    math::asin(x.clamp(-1.0, 1.0)).to_degrees()
}

/// The step on the world of `inputs` at `level` from [`Working::initial`] with its ground set by
/// `set`, its energy-balance model on `grid`.
fn run_world(
    inputs: CoarseInputs,
    level: u8,
    grid: Grid,
    set: impl FnOnce(&Pass<'_>, &mut Working),
) -> Run {
    let (field, climate) = grids(level);
    let (working, diagnostics) = {
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
        let mut working = Working::initial(&pass);
        set(&pass, &mut working);
        let diagnostics = match grid {
            Grid::Own => run_diagnosed(&pass, &mut working),
            Grid::Zonal => {
                let model =
                    energy_balance(&pass, &working, ModelGrid::zonal(), Coordinates::Geographic);
                finish(&pass, &mut working, model)
            }
        };
        (working, diagnostics)
    };
    Run {
        field,
        climate,
        inputs,
        working,
        diagnostics,
    }
}

/// The reference Earth with Earth's land by 2.5° × 5° box as a synthetic sea, at Earth's own
/// longitudes, its land at Earth2014's hypsometry by band placed by each box's Earth2014 median
/// land height (R09.T13.g's Earth; [`synthetic_sea`], decision-r09-t13e-asbuilt).
fn synthetic_earth(pass: &Pass<'_>, working: &mut Working) {
    synthetic_sea(pass, working, &EARTH_HYPSOMETRY_M);
}

/// Every cell of `working` flooded 100 m deep with the main liquid: an aquaplanet.
fn flood_all(pass: &Pass<'_>, working: &mut Working) {
    let flood = pass.inputs().main_liquid();
    for cell in &mut working.cells {
        set_ground(cell, Metres::new(-100.0), Metres::ZERO, flood);
    }
    working.sea_level = Metres::ZERO;
}

/// The reference Earth at obliquity `obliquity_deg` and its rotation rate times `rate`, its solar
/// day from its sidereal one on its year, under 1 bar, GK19's Earth-like atmosphere (so that its
/// cap is decision-r09-t13ge §2.10's figure; the reference Earth's 985.5 hPa moves it by 0.2%).
fn earth_at(obliquity_deg: f64, rate: f64) -> CoarseInputs {
    let earth = reference::earth_like();
    let year = earth.seasonal_orbit().unwrap().period.value();
    let rotation = earth.spin().rotation_period.value() / rate;
    builder_of(&earth)
        .spin(Spin {
            obliquity: Radians::new(obliquity_deg.to_radians()),
            rotation_period: Seconds::new(rotation),
            solar_day: Some(Seconds::new(1.0 / (1.0 / rotation - 1.0 / year))),
            ..*earth.spin()
        })
        .surface_pressure(Pascals::new(1.0e5))
        .build()
        .expect("an Earth at another obliquity and rotation is valid")
}

/// The reference Earth's inputs on a sphere of 1,500 km, whose coarse level is 6, so that its step
/// at level 6 quantises (decision-r09-t13-test-cost's second radius).
fn small(inputs: &CoarseInputs) -> CoarseInputs {
    let inputs = builder_of(inputs)
        .figure(Spheroid::sphere(1.5e6))
        .build()
        .expect("the reference Earth's inputs hold on a smaller sphere");
    assert_eq!(coarse_level(inputs.radius()).get(), 6);
    inputs
}

/// The reference Earth with no seasonal forcing: no obliquity on a circular orbit, one month, on
/// a 1,500 km sphere ([`small`]).
fn unseasonal_earth() -> CoarseInputs {
    let earth = reference::earth_like();
    let inputs = builder_of(&earth)
        .spin(Spin {
            obliquity: Radians::ZERO,
            ..*earth.spin()
        })
        .seasonal_orbit(SeasonalOrbit {
            eccentricity: 0.0,
            ..*earth.seasonal_orbit().unwrap()
        })
        .build()
        .expect("an unseasonal Earth is valid");
    assert_eq!(inputs.months(), 1);
    small(&inputs)
}

/// The reference Earth with no liquid: its condensable, water, is not among its surface liquids,
/// so the energy-balance model is dry ([`EnergyBalance::for_inputs`]).
fn dry_earth() -> CoarseInputs {
    builder_of(&reference::earth_like())
        .liquids([])
        .condensates([])
        .build()
        .expect("a dry Earth is valid")
}

/// The decoded wind's meridional part, m/s, towards local north.
fn northward(sample: &steps::WindSample) -> f64 {
    sample.speed.value() * math::cos(sample.azimuth.value())
}

/// The count of sign changes round the year of `values`, read as a cycle.
fn sign_changes(values: &[f64]) -> usize {
    (0..values.len())
        .filter(|&m| (values[m] > 0.0) != (values[(m + 1) % values.len()] > 0.0))
        .count()
}

/// Each band's annual mean of a month's band value `of` (the month's cycle and the band), each
/// month weighted by its share of the year.
fn annual_bands(run: &Run, of: fn(&MonthCycle, usize) -> f64) -> Vec<f64> {
    let water = run.water();
    let shares = month_shares(&run.pass(), water.months.len());
    (0..LATITUDE_BANDS)
        .map(|band| {
            water
                .months
                .iter()
                .zip(&shares)
                .fold(0.0, |sum, (cycle, &s)| sum + s * of(cycle, band))
        })
        .collect()
}

/// The zonal budget's annual E − P at each band, kg m⁻² s⁻¹: each month's band E − P, the
/// divergence of the latent heat's flux over L, weighted by the month's share.
fn annual_budget(run: &Run) -> Vec<f64> {
    annual_bands(run, |c, k| {
        c.budget.evaporation_less_precipitation_kg_m2_s[k]
    })
}

/// The solid-angle mean of `bands` (one a band of the zonal grid) over the bands whose centres'
/// latitude lies within `low` to `high`, degrees.
fn band_window(bands: &[f64], low: f64, high: f64) -> f64 {
    let grid = ModelGrid::zonal();
    let (mut sum, mut area) = (0.0, 0.0);
    for (band, &value) in bands.iter().enumerate() {
        if (low..=high).contains(&degrees(grid.centre_sine(band))) {
            sum += grid.weight(band) * value;
            area += grid.weight(band);
        }
    }
    sum / area
}

/// Asserts the zonal budget's pattern (decision-r09-t13e-asbuilt §1.5): in each hemisphere the
/// annual E − P ([`annual_budget`]) has a negative solid-angle mean over 40–60° and a positive one
/// over 15–35°, the midlatitudes converging the vapour the subtropics export, as ERA-Interim's
/// does (Siler, Roe and Armour 2018, J. Climate 31, 7481, Fig. 1, read by
/// decision-r09-t13e-asbuilt: E > P in the subtropics, P > E in the midlatitudes). Prints the four
/// means.
fn assert_the_budget_s_pattern(run: &Run) {
    let budget = annual_budget(run);
    for (hemisphere, sign) in [("north", 1.0_f64), ("south", -1.0)] {
        let window = |a: f64, b: f64| {
            let (low, high) = if sign > 0.0 { (a, b) } else { (-b, -a) };
            band_window(&budget, low, high)
        };
        let (tracks, subtropics) = (window(40.0, 60.0), window(15.0, 35.0));
        eprintln!(
            "  {hemisphere}: the zonal budget's annual E − P {:+.3} mm/day over 40–60°, {:+.3} \
             over 15–35°",
            tracks * DAY_S,
            subtropics * DAY_S
        );
        assert!(tracks < 0.0, "{hemisphere}: E − P {tracks} over 40–60°");
        assert!(
            subtropics > 0.0,
            "{hemisphere}: E − P {subtropics} over 15–35°"
        );
    }
}

/// Asserts the water balance (decision-r09-t13ge §2.13, e8): in every month global precipitation
/// equals global evaporation, the open liquid's plus the land's ε P, to 1%. Prints each month's.
fn assert_the_water_balances(run: &Run) {
    for (m, cycle) in run.water().months.iter().enumerate() {
        let evaporation = cycle.liquid_evaporation_kg_m2_s + cycle.recycled_kg_m2_s;
        let precipitation = cycle.precipitation_kg_m2_s;
        eprintln!(
            "  month {m:2}: P {:.4} mm/day, E {:.4} (liquid {:.4}, land {:.4}), k {:.4}",
            precipitation * DAY_S,
            evaporation * DAY_S,
            cycle.liquid_evaporation_kg_m2_s * DAY_S,
            cycle.recycled_kg_m2_s * DAY_S,
            cycle.scale
        );
        assert!(evaporation > 0.0, "month {m}");
        assert!(
            (precipitation / evaporation - 1.0).abs() < 0.01,
            "month {m}: P {precipitation} against E {evaporation}"
        );
    }
}

/// Prints a run's water cycle beside GPCP and NCEP: the annual zonal precipitation by band, the
/// windows of decision-r09-t13ge §2.13, the global means, the energy-flux equators and centres, the
/// balance's factors, the orographic solves' convergence, and the trades and westerlies.
fn report(name: &str, run: &Run) {
    report_bands(name, run);
    report_summary(run);
    report_months(run);
    report_winds(run);
}

/// [`report`]'s bands: the annual zonal precipitation, its parts and GPCP's.
fn report_bands(name: &str, run: &Run) {
    let annual = run.annual_precipitation();
    let zonal = run.zonal(&annual);
    let grid = ModelGrid::zonal();
    // Each climate cell's cover: its four field cells all sea, all land, or mixed.
    let seas_of: Vec<u8> = (0..run.climate.len())
        .map(|c| {
            (0..4)
                .map(|k| {
                    u8::from(seas::under_sea(
                        &run.working.cells[4 * usize::try_from(c).unwrap() + k],
                    ))
                })
                .sum()
        })
        .collect();
    let part = |wanted: u8| -> Vec<f64> {
        let (mut sums, mut areas) = (vec![0.0; LATITUDE_BANDS], vec![0.0; LATITUDE_BANDS]);
        for (((centre, &omega), &p), &s) in run
            .climate
            .centres()
            .iter()
            .zip(run.climate.solid_angles_sr())
            .zip(&annual)
            .zip(&seas_of)
        {
            if s == wanted {
                let band = ModelGrid::band_of(centre[2]);
                sums[band] += omega * p;
                areas[band] += omega;
            }
        }
        sums.iter()
            .zip(&areas)
            .map(|(s, a)| if *a > 0.0 { s / a } else { f64::NAN })
            .collect()
    };
    let (sea, land) = (part(4), part(0));
    let rates = annual_bands(run, |c, k| c.scale * c.large_scale_kg_m2_s[k]);
    let evaporation = annual_bands(run, |c, k| c.band_evaporation_kg_m2_s[k]);
    let budget = annual_budget(run);
    eprintln!(
        "{name}: annual zonal precipitation, mm/day, against GPCP v2.3 (1991–2020); the all-sea \
         and all-land climate cells' means, k P_ls, the bands' Ē and E − P"
    );
    for (band, (&p, &gpcp)) in zonal.iter().zip(&GPCP_ANNUAL_MM_PER_DAY).enumerate() {
        eprintln!(
            "  {:7.2}°  {:5.2}  GPCP {gpcp:5.2}  sea {:5.2} land {:5.2}  k P_ls {:5.2}  Ē {:5.2}  \
             E − P {:+5.2}",
            degrees(grid.centre_sine(band)),
            p * DAY_S,
            sea[band] * DAY_S,
            land[band] * DAY_S,
            rates[band] * DAY_S,
            evaporation[band] * DAY_S,
            budget[band] * DAY_S
        );
    }
}

/// [`report`]'s summary: the windows against GPCP's, the global means, and the tropical maximum
/// against the annual energy-flux equator.
fn report_summary(run: &Run) {
    let water = run.water();
    let annual = run.annual_precipitation();
    let zonal = run.zonal(&annual);
    let shares = month_shares(&run.pass(), water.months.len());
    let grid = ModelGrid::zonal();
    let large_scale = annual_bands(run, |c, k| c.large_scale_kg_m2_s[k]);
    let evaporation = annual_bands(run, |c, k| c.band_evaporation_kg_m2_s[k]);
    let budget = annual_budget(run);
    for (low, high) in [(40.0, 60.0), (15.0, 35.0), (-60.0, -40.0), (-35.0, -15.0)] {
        eprintln!(
            "  window {low:5.1}° to {high:5.1}°: P {:.2} mm/day, GPCP {:.2}; the zonal budget's \
             P_ls {:.2}, Ē {:.2}, E − P {:+.2}",
            run.window(&annual, low, high) * DAY_S,
            band_window(&GPCP_ANNUAL_MM_PER_DAY, low, high),
            band_window(&large_scale, low, high) * DAY_S,
            band_window(&evaporation, low, high) * DAY_S,
            band_window(&budget, low, high) * DAY_S
        );
    }
    let annual_of = |part: fn(&MonthCycle) -> f64| -> f64 {
        water
            .months
            .iter()
            .zip(&shares)
            .fold(0.0, |sum, (cycle, &s)| sum + s * part(cycle))
    };
    eprintln!(
        "  global: P {:.3} mm/day, E {:.3} (the liquid's {:.3}, the land's ε P {:.3}); GPCP 2.691",
        annual_of(|c| c.precipitation_kg_m2_s) * DAY_S,
        annual_of(|c| c.liquid_evaporation_kg_m2_s + c.recycled_kg_m2_s) * DAY_S,
        annual_of(|c| c.liquid_evaporation_kg_m2_s) * DAY_S,
        annual_of(|c| c.recycled_kg_m2_s) * DAY_S
    );
    let tropical = (0..LATITUDE_BANDS)
        .filter(|&k| degrees(grid.centre_sine(k)).abs() < 30.0)
        .fold((0, f64::NEG_INFINITY), |(best, most), k| {
            if zonal[k] > most {
                (k, zonal[k])
            } else {
                (best, most)
            }
        });
    eprintln!(
        "  annual energy-flux equator {:+.2}°; tropical maximum {:+.2}° ({:.2} mm/day); Hadley \
         edge {:.1}°, width {:.3}",
        run.annual_efe_deg(),
        degrees(grid.centre_sine(tropical.0)),
        tropical.1 * DAY_S,
        water.circulation.edge_deg(),
        water.circulation.width()
    );
}

/// [`report`]'s months: each month's energy-flux equator, centre, factor and rate, and its
/// orographic solve's iteration, components and budget.
fn report_months(run: &Run) {
    for (m, cycle) in run.water().months.iter().enumerate() {
        let solve = cycle.orographic.as_ref().map_or_else(String::new, |d| {
            let c = &d.components;
            format!(
                "; land {} cells, {} restarts, {} sweeps, measure {:.1e}{} (residual {:.1e}); \
                 components of 1: {}, 2: {}, 3–16: {}, > 16: {} ({} cells, the largest {}); {} \
                 below zero; the budget's closure {:.1e}",
                d.land_cells,
                d.restarts,
                d.sweeps,
                d.measure,
                if d.converged { "" } else { " (the cap)" },
                d.relative_residual,
                c.single,
                c.pairs,
                c.small,
                c.large,
                c.large_cells,
                c.largest,
                d.negative_cells,
                closure(d)
            )
        });
        eprintln!(
            "  month {m:2}: EFE {:+6.2}°, centre {:+6.2}°, k {:.4}, P {:.3} mm/day{}{solve}",
            degrees(cycle.budget.efe_sine),
            degrees(cycle.budget.centre_sine),
            cycle.scale,
            cycle.precipitation_kg_m2_s * DAY_S,
            if cycle.calm { ", wholly calm" } else { "" }
        );
    }
}

/// The orographic solve's land budget's closure, (inflow − runoff − outflow) ÷ inflow.
fn closure(d: &lfpm::Diagnostics) -> f64 {
    (d.inflow_kg_s - d.runoff_kg_s - d.outflow_kg_s) / d.inflow_kg_s
}

/// The largest error of `values` against `reference` (one a climate cell) over the climate cells
/// `land` marks, |P − P*| ÷ max(P*, `floor`), with its cell, and the share of those cells above
/// 10⁻³.
fn compare(values: &[f64], reference: &[f64], floor: f64, land: &[bool]) -> (f64, usize, f64) {
    let (mut worst, mut at, mut above, mut count) = (0.0_f64, 0, 0_u32, 0_u32);
    for (c, ((&p, &q), &is_land)) in values.iter().zip(reference).zip(land).enumerate() {
        if !is_land {
            continue;
        }
        let error = (p - q).abs() / q.max(floor);
        if error > worst {
            (worst, at) = (error, c);
        }
        above += u32::from(error > 1e-3);
        count += 1;
    }
    (worst, at, f64::from(above) / f64::from(count))
}

/// Asserts the ruled convergence on the coarse pass's Earth (decision-r09-t13e-lfpm-convergence
/// §3.5): every month's orographic solve converges before the cap and closes the land's budget
/// within 10⁻³ of its inflow, the deep solve meets the stop test at [`DEEP_MEASURE`], and every
/// land climate cell's (any of its four field cells land) monthly and annual rate lies within 1%
/// of the deep solve's, |P − P*| ≤ 0.01 max(P*, 10⁻³ k* `P̄_sea`), `deep` the same water cycle at
/// the deep tolerance on a clone of the finished state ([`Parameters::deep`]) with its records
/// `deep_climate`. Prints each month's comparison.
fn assert_the_solve_converges(
    run: &Run,
    deep: &hydrology::Diagnostics,
    deep_climate: &[steps::ClimateState],
) {
    let water = run.water();
    let shares = month_shares(&run.pass(), water.months.len());
    let land: Vec<bool> = run
        .working
        .cells
        .as_chunks::<4>()
        .0
        .iter()
        .map(|cells| cells.iter().any(|cell| !seas::under_sea(cell)))
        .collect();
    let at = |c: usize| {
        let centre = run.climate.centre(u32::try_from(c).unwrap());
        (
            latitude_deg(centre),
            math::atan2(centre[1], centre[0]).to_degrees(),
        )
    };
    let (mut annual, mut annual_deep, mut annual_floor) =
        (vec![0.0; land.len()], vec![0.0; land.len()], 0.0);
    for (m, (cycle, deep_cycle)) in water.months.iter().zip(&deep.months).enumerate() {
        let d = cycle
            .orographic
            .as_ref()
            .expect("each month of the coarse pass's Earth solves its orography");
        let dd = deep_cycle.orographic.as_ref().unwrap();
        let values: Vec<f64> = run
            .working
            .climate
            .iter()
            .map(|s| s.month_precipitation_kg_m2_s[m])
            .collect();
        let reference: Vec<f64> = deep_climate
            .iter()
            .map(|s| s.month_precipitation_kg_m2_s[m])
            .collect();
        let floor = RATE_FLOOR_SHARE * deep_cycle.scale * dd.sea_mean_kg_m2_s;
        let (worst, cell, above) = compare(&values, &reference, floor, &land);
        let (latitude, longitude) = at(cell);
        eprintln!(
            "  month {m:2}: {} restarts, {} sweeps, measure {:.1e}; deep {} restarts, {} sweeps, \
             measure {:.1e} (residual {:.1e}); the worst land climate cell {worst:.2e} at \
             {latitude:.2}°, {longitude:.2}° ({:.3} against {:.3} mm/day), {:.2}% above 0.1%",
            d.restarts,
            d.sweeps,
            d.measure,
            dd.restarts,
            dd.sweeps,
            dd.measure,
            dd.relative_residual,
            values[cell] * DAY_S,
            reference[cell] * DAY_S,
            100.0 * above
        );
        assert!(
            d.converged && d.restarts <= MAX_RESTARTS && d.measure <= TOLERANCE,
            "month {m}: {d:?}"
        );
        assert!(closure(d).abs() <= 1e-3, "month {m}: {d:?}");
        assert!(
            dd.measure <= DEEP_MEASURE,
            "month {m}: the deep solve's {dd:?}"
        );
        assert!(
            worst <= 0.01,
            "month {m}: the worst land climate cell {worst}"
        );
        for (sum, &v) in annual.iter_mut().zip(&values) {
            *sum += shares[m] * v;
        }
        for (sum, &v) in annual_deep.iter_mut().zip(&reference) {
            *sum += shares[m] * v;
        }
        annual_floor += shares[m] * floor;
    }
    let (worst, cell, above) = compare(&annual, &annual_deep, annual_floor, &land);
    let (latitude, longitude) = at(cell);
    eprintln!(
        "  annual: the worst land climate cell {worst:.2e} at {latitude:.2}°, {longitude:.2}° \
         ({:.3} against {:.3} mm/day), {:.2}% above 0.1%, of {} land climate cells",
        annual[cell] * DAY_S,
        annual_deep[cell] * DAY_S,
        100.0 * above,
        land.iter().filter(|&&l| l).count()
    );
    assert!(worst <= 0.01, "annual: the worst land climate cell {worst}");
}

/// [`report`]'s winds: the annual zonal-mean trades and westerlies against NCEP/NCAR R1's.
fn report_winds(run: &Run) {
    let (north, east) = run.annual_winds();
    let bin = |values: &[f64], centre: f64| run.window(values, centre - 2.5, centre + 2.5);
    for (centre, ncep) in NCEP_TRADES_V_M_S {
        eprintln!(
            "  annual v at {centre:+5.1}°: {:+.2} m/s, NCEP {ncep:+.2}; u {:+.2} m/s",
            bin(&north, centre),
            bin(&east, centre)
        );
    }
    eprintln!(
        "  annual u at 52.5° S {:+.2} m/s (NCEP +6.92), at 47.5° N {:+.2} (NCEP +1.71); v at \
         52.5° S {:+.2} (NCEP −1.88)",
        bin(&east, -52.5),
        bin(&east, 47.5),
        bin(&north, -52.5)
    );
}

/// The ruled Earth test (decision-r09-t13ge §2.13, e9, as decision-r09-t13e-asbuilt §1.5 rules
/// it) on the reference Earth: R09.T13.g's synthetic sea, Natural Earth's land by 2.5° × 5° box,
/// its heights R09.T13.g's band eighths placed by each box's Earth2014 median land height
/// ([`synthetic_earth`]), the whole step at level 7 on its geographic model, printed against
/// GPCP v2.3 and NCEP/NCAR R1. In the southern hemisphere the annual zonal precipitation's
/// solid-angle mean over 40–60° exceeds that over 15–35°; in each hemisphere the zonal budget's
/// annual E − P has a negative solid-angle mean over 40–60° and a positive one over 15–35°
/// ([`assert_the_budget_s_pattern`]); and the precipitation's maximum within 30° of the equator
/// lies within 10° of the annual energy-flux equator.
///
/// The northern hemisphere's two windows are printed beside GPCP's, not asserted: on Earth's
/// geography the storm track's over the subtropics' reads 0.99–1.04 as the relief's placement
/// moves it (1.00 as built), inside what that placement alone moves it by, against GPCP's 1.27
/// (decision-r09-t13e-asbuilt §1.2, §1.5; plan 09's deferred finding E-P1, whose revision restores
/// the assertion).
#[test]
fn the_reference_earth_s_southern_storm_track_is_wetter_than_its_subtropics_its_midlatitudes_converge_vapour_and_its_rain_band_is_on_its_energy_flux_equator()
 {
    let run = run_world(reference::earth_like(), 7, Grid::Own, synthetic_earth);
    report("The reference Earth (synthetic sea)", &run);
    let annual = run.annual_precipitation();
    let (north_tracks, north_subtropics) = (
        run.window(&annual, 40.0, 60.0),
        run.window(&annual, 15.0, 35.0),
    );
    eprintln!(
        "  the northern windows, printed, not asserted: 40–60° N {:.2} mm/day against 15–35° N \
         {:.2} ({:.2}); GPCP {:.2} against {:.2} ({:.2})",
        north_tracks * DAY_S,
        north_subtropics * DAY_S,
        north_tracks / north_subtropics,
        band_window(&GPCP_ANNUAL_MM_PER_DAY, 40.0, 60.0),
        band_window(&GPCP_ANNUAL_MM_PER_DAY, 15.0, 35.0),
        band_window(&GPCP_ANNUAL_MM_PER_DAY, 40.0, 60.0)
            / band_window(&GPCP_ANNUAL_MM_PER_DAY, 15.0, 35.0)
    );
    let (tracks, subtropics) = (
        run.window(&annual, -60.0, -40.0),
        run.window(&annual, -35.0, -15.0),
    );
    eprintln!(
        "  the southern windows: 40–60° S {:.2} mm/day against 15–35° S {:.2} ({:.2})",
        tracks * DAY_S,
        subtropics * DAY_S,
        tracks / subtropics
    );
    assert!(
        tracks > subtropics,
        "south: 40–60° {} mm/day against 15–35° {}",
        tracks * DAY_S,
        subtropics * DAY_S
    );
    assert_the_budget_s_pattern(&run);
    let zonal = run.zonal(&annual);
    let grid = ModelGrid::zonal();
    let tropical = (0..LATITUDE_BANDS)
        .filter(|&k| degrees(grid.centre_sine(k)).abs() < 30.0)
        .fold((0, f64::NEG_INFINITY), |(best, most), k| {
            if zonal[k] > most {
                (k, zonal[k])
            } else {
                (best, most)
            }
        })
        .0;
    let (maximum, efe) = (degrees(grid.centre_sine(tropical)), run.annual_efe_deg());
    assert!(
        (maximum - efe).abs() <= 10.0,
        "the tropical maximum at {maximum}° against the energy-flux equator at {efe}°"
    );
}

/// The ruled balance (decision-r09-t13ge §2.13, e8): global precipitation equals global
/// evaporation, the open liquid's Priestley–Taylor evaporation plus the land's ε(z) P, to 1%, in
/// every month, on the reference Earth (its synthetic sea, level 6); and the climate records'
/// solid-angle mean is the field cells'.
#[test]
fn global_precipitation_equals_global_evaporation() {
    let run = run_world(reference::earth_like(), 6, Grid::Own, synthetic_earth);
    let water = run.water();
    assert_eq!(water.months.len(), 12);
    assert_the_water_balances(&run);
    for (m, cycle) in water.months.iter().enumerate() {
        let precipitation = cycle.precipitation_kg_m2_s;
        let (sum, area) = run
            .working
            .climate
            .iter()
            .zip(run.climate.solid_angles_sr())
            .fold((0.0, 0.0), |(s, a), (state, &omega)| {
                (s + omega * state.month_precipitation_kg_m2_s[m], a + omega)
            });
        assert!(
            ((sum / area) / precipitation - 1.0).abs() < 1e-12,
            "month {m}"
        );
    }
}

/// The ruled wind test (decision-r09-t13ge §2.13): each month's quantised wind is the wind that
/// month's orographic step used. The step writes each climate cell's month as the record's code
/// decoded, and the orographic step reads each field cell's wind from that state
/// ([`wind::cell_wind`]); so the quantised field's codes are the ones the step recorded as read,
/// they decode to the state's samples bit for bit, and re-quantise to themselves. On the
/// reference Earth's synthetic sea at level 6, every month has a wind somewhere.
#[test]
fn each_month_s_quantised_wind_is_the_wind_that_month_s_orographic_step_used() {
    let run = run_world(
        small(&reference::earth_like()),
        6,
        Grid::Own,
        synthetic_earth,
    );
    let water = run.water();
    let field = quantise(&run.inputs, run.working.clone()).unwrap();
    assert_eq!(field.climate_layer().len(), run.working.climate.len());
    for (c, (record, state)) in field
        .climate_layer()
        .iter()
        .zip(&run.working.climate)
        .enumerate()
    {
        for m in 0..12 {
            let code = record.wind[m];
            assert_eq!(code, water.winds[m][c], "climate cell {c}, month {m}");
            assert_same_bits(code.speed().value(), state.month_wind[m].speed.value());
            assert_same_bits(
                code.azimuth_angle().value(),
                state.month_wind[m].azimuth.value(),
            );
            assert_eq!(
                Wind::from_velocity(code.speed(), code.azimuth_angle()).unwrap(),
                code
            );
        }
    }
    for (m, codes) in water.winds.iter().enumerate() {
        let moving = codes.iter().filter(|&&w| w != Wind::CALM).count();
        eprintln!(
            "month {m:2}: {moving} of {} climate cells' winds not calm",
            codes.len()
        );
        assert!(moving * 2 > codes.len(), "month {m}: {moving}");
    }
    // The field cells' winds the orographic step read: each its climate cell's sample at its own
    // centre, so a field cell's direction is its record's azimuth from its own local north.
    let n = 1_000_u32;
    let up = run.field.centre(n);
    let sample = &run.working.climate[usize::try_from(parent_index(n)).unwrap()].month_wind[0];
    if let lfpm::CellWind::Towards(towards) = wind::cell_wind(sample, up) {
        let (east, north) = wind::east_north(up);
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let angle = math::atan2(dot(towards, east), dot(towards, north));
        let off = (angle - sample.azimuth.value()).rem_euclid(core::f64::consts::TAU);
        assert!(
            off.min(core::f64::consts::TAU - off) < 1e-12,
            "{angle} against {sample:?}"
        );
    }
}

/// The ruled aquaplanet test (decision-r09-t13ge §2.13, e10): on a synthetic aquaplanet at 60°
/// obliquity and Earth's rotation (every cell flooded, level 6, the zonal model), no month's
/// rain-band centre lies poleward of the ascending-branch cap, 37.4° (GK19's Eq. 20 at 60°, 1 bar,
/// Ω⊕ and a year); an equatorial cell's meridional wind changes sign twice a year, each month's
/// speed at least 0.05 m/s.
#[test]
fn on_a_60_degree_aquaplanet_the_centre_holds_at_the_cap_and_the_equatorial_wind_reverses_twice() {
    let run = run_world(earth_at(60.0, 1.0), 6, Grid::Zonal, flood_all);
    let water = run.water();
    let cap = water
        .circulation
        .cap_deg()
        .expect("a rotating world has a cap");
    eprintln!(
        "60° aquaplanet: cap {cap:.3}°, Hadley edge {:.1}°",
        water.circulation.edge_deg()
    );
    assert!((cap - 37.4).abs() < 0.05, "{cap}");
    let equatorial = run
        .climate
        .centres()
        .iter()
        .enumerate()
        .fold((0, f64::INFINITY), |(best, least), (c, centre)| {
            if centre[2].abs() < least {
                (c, centre[2].abs())
            } else {
                (best, least)
            }
        })
        .0;
    let winds = &run.working.climate[equatorial].month_wind;
    let v: Vec<f64> = winds.iter().map(northward).collect();
    for (m, cycle) in water.months.iter().enumerate() {
        let centre = degrees(cycle.budget.centre_sine);
        eprintln!(
            "  month {m:2}: EFE {:+6.2}°, centre {centre:+6.2}°, equatorial v {:+.3} m/s ({:.3} \
             m/s), P {:.3} mm/day",
            degrees(cycle.budget.efe_sine),
            v[m],
            winds[m].speed.value(),
            cycle.precipitation_kg_m2_s * DAY_S
        );
        assert!(
            centre.abs() <= cap + 1e-9,
            "month {m}: {centre}° beyond {cap}°"
        );
        assert!(winds[m].speed.value() >= 0.05, "month {m}: {:?}", winds[m]);
    }
    assert_eq!(sign_changes(&v), 2, "{v:?}");
}

/// The ruled slow twin (decision-r09-t13ge §2.13, §2.12): the same aquaplanet at Ω⊕ ÷ 8 has a cap
/// of 90°, and its centre is its energy-flux equator in every month.
#[test]
fn the_aquaplanet_at_an_eighth_of_earth_s_rotation_centres_on_its_energy_flux_equator() {
    let run = run_world(earth_at(60.0, 0.125), 6, Grid::Zonal, flood_all);
    let water = run.water();
    let cap = water
        .circulation
        .cap_deg()
        .expect("a rotating world has a cap");
    eprintln!(
        "60° aquaplanet at Ω⊕ ÷ 8: cap {cap}°, Hadley edge {:.1}° (width {:.3})",
        water.circulation.edge_deg(),
        water.circulation.width()
    );
    assert_same_bits(cap, 90.0);
    for (m, cycle) in water.months.iter().enumerate() {
        eprintln!(
            "  month {m:2}: EFE {:+6.2}°, P {:.3} mm/day",
            degrees(cycle.budget.efe_sine),
            cycle.precipitation_kg_m2_s * DAY_S
        );
        assert_same_bits(cycle.budget.centre_sine, cycle.budget.efe_sine);
    }
}

/// The ruled Titan test (decision-r09-t13ge §2.13, §2.12): a Titan-like world (R09.T13.d's: methane
/// and ethane seas in four basins, one month) evaporates only over its methane seas, every
/// methane cell evaporating and no ethane or dry cell, and its global precipitation equals its
/// evaporation to 1%.
#[test]
fn a_titan_like_world_evaporates_only_over_its_methane_seas_and_rains_what_it_evaporates() {
    let (field, climate_grid, inputs, mut working) = titan_with_basins(6);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let model = normalised(&pass, &working);
    let own = model
        .water
        .clone()
        .expect("the Titan-like world runs the energy balance");
    let (grid, coordinates) = (model.grid.clone(), model.coordinates);
    let diagnostics = finish(&pass, &mut working, model);
    let water = diagnostics.water.as_ref().unwrap();
    let ebm = EnergyBalance::for_inputs(&inputs);
    let evaporation = Evaporation::new(&pass, &working, &ebm, &own).expect("methane evaporates");
    let palette = inputs.palette();
    let key = |k: &str| crate::substance::SubstanceKey::new(k).unwrap();
    let methane = palette.find(PaletteRole::Liquid, key("CH4")).unwrap();
    let mut rates = [0.0; 1];
    let (mut methane_cells, mut wet) = (0_u32, 0_u32);
    for (n, cell) in (0_u32..).zip(&working.cells) {
        let open = evaporation.rates(&pass, &working, &grid, coordinates, n, &mut rates);
        if cell.liquid_entry == Some(methane) {
            assert!(open && rates[0] > 0.0, "methane cell {n}: {}", rates[0]);
            methane_cells += 1;
            wet += 1;
        } else {
            assert!(!open, "cell {n}: {:?}", cell.liquid_entry);
            assert_same_bits(rates[0], 0.0);
        }
    }
    assert!(methane_cells > 0 && wet == methane_cells);
    let cycle = &water.months[0];
    let evaporated = cycle.liquid_evaporation_kg_m2_s + cycle.recycled_kg_m2_s;
    let year = 365.25 * DAY_S;
    eprintln!(
        "Titan-like: {methane_cells} methane cells; E {:.3} kg m⁻² a⁻¹ (the liquid's {:.3}, the \
         land's ε P {:.3}), P {:.3}, k {:.4}; EFE {:+.2}°, cap {:?}",
        evaporated * year,
        cycle.liquid_evaporation_kg_m2_s * year,
        cycle.recycled_kg_m2_s * year,
        cycle.precipitation_kg_m2_s * year,
        cycle.scale,
        degrees(cycle.budget.efe_sine),
        water.circulation.cap_deg()
    );
    assert!(evaporated > 0.0);
    assert!((cycle.precipitation_kg_m2_s / evaporated - 1.0).abs() < 0.01);
}

/// The ruled dry world (decision-r09-t13ge §2.13, §2.12): a dry world under the energy-balance
/// model, the reference Earth with no liquid (level 6, the zonal model), has no precipitation and
/// winds that are not calm, ψ standing on its sensible heat's flux; and its annual runoff is zero.
#[test]
fn a_dry_world_under_the_energy_balance_model_has_no_precipitation_and_winds_that_are_not_calm() {
    let run = run_world(dry_earth(), 6, Grid::Zonal, |_, _| {});
    assert!(EnergyBalance::for_inputs(&run.inputs).moisture.is_none());
    let water = run.water();
    for (m, cycle) in water.months.iter().enumerate() {
        assert_same_bits(cycle.precipitation_kg_m2_s, 0.0);
        assert_same_bits(cycle.liquid_evaporation_kg_m2_s, 0.0);
        assert!(cycle.orographic.is_none(), "month {m}");
        let moving = water.winds[m].iter().filter(|&&w| w != Wind::CALM).count();
        eprintln!("dry world, month {m:2}: {moving} climate cells' winds not calm");
        assert!(moving > 0, "month {m}");
    }
    for state in &run.working.climate {
        for &p in &state.month_precipitation_kg_m2_s {
            assert_same_bits(p, 0.0);
        }
    }
    let pass = run.pass();
    for n in 0..run.field.len() {
        assert_same_bits(annual_runoff_kg_m2_s(&pass, &run.working, n), 0.0);
    }
}

/// The ruled other models (decision-r09-t13ge §2.13, §2.7): the reference Mars (the radiative
/// model) and the Venus-like body (the isothermal one) have no precipitation and calm winds.
#[test]
fn the_reference_mars_and_the_venus_like_body_have_no_precipitation_and_calm_winds() {
    for (name, inputs) in [
        ("Mars", reference::mars_like()),
        ("Venus-like", venus_like()),
    ] {
        let run = run_world(inputs, 5, Grid::Own, |_, _| {});
        assert!(run.diagnostics.water.is_none(), "{name}");
        for state in &run.working.climate {
            for m in 0..12 {
                assert_same_bits(state.month_precipitation_kg_m2_s[m], 0.0);
                assert_eq!(state.month_wind[m], steps::WindSample::CALM, "{name}");
            }
        }
    }
}

/// The ruled one-month test (decision-r09-t13ge §2.13; Design note 8's one month): a world at
/// e = 0 with no obliquity to its seasonal orbit (the reference Earth so, its synthetic sea at
/// level 6) has one wind: its records' first month holds a wind and a rate, and the other eleven
/// are calm and dry.
#[test]
fn a_world_at_no_eccentricity_and_no_obliquity_has_one_wind() {
    let run = run_world(unseasonal_earth(), 6, Grid::Own, synthetic_earth);
    let field = quantise(&run.inputs, run.working.clone()).unwrap();
    assert_eq!(field.header().months(), 1);
    let (mut winds, mut rains) = (0_u32, 0_u32);
    for record in field.climate_layer() {
        winds += u32::from(record.wind[0] != Wind::CALM);
        rains += u32::from(record.month_precipitation[0] != LogPrecipitation::NONE);
        assert!(record.wind[1..].iter().all(|&w| w == Wind::CALM));
        assert!(
            record.month_precipitation[1..]
                .iter()
                .all(|&p| p == LogPrecipitation::NONE)
        );
    }
    eprintln!(
        "one month: {winds} winds and {rains} rates of {}",
        field.climate_layer().len()
    );
    assert!(winds > 0 && rains > 0);
}

/// The ruled header test (decision-r09-t13ge §2.13): the header says heuristic, on a world whose
/// step wrote precipitation.
#[test]
fn the_header_says_heuristic() {
    let run = run_world(unseasonal_earth(), 6, Grid::Own, synthetic_earth);
    assert!(
        run.working
            .climate
            .iter()
            .any(|state| state.month_precipitation_kg_m2_s[0] > 0.0)
    );
    let field = quantise(&run.inputs, run.working.clone()).unwrap();
    assert_eq!(
        field.header().precipitation(),
        PrecipitationSource::Heuristic
    );
}

/// The ruled runoff test (decision-r09-t13ge §2.13, e8; R09.T14's): the annual runoff function is
/// zero over liquid and on a dry world ([`a_dry_world_under_the_energy_balance_model_has_no_precipitation_and_winds_that_are_not_calm`]),
/// and over a flat sea-level continent is 1 − ε₀ of P, its climate cell's annual rate: the
/// unseasonal reference Earth's synthetic sea with its land at the sea level, level 6.
#[test]
fn the_annual_runoff_is_none_over_liquid_and_the_unrecycled_share_over_a_sea_level_continent() {
    let run = run_world(unseasonal_earth(), 6, Grid::Own, |pass, working| {
        synthetic_sea(pass, working, &SEA_LEVEL_LAND);
    });
    let pass = run.pass();
    let annual = run.annual_precipitation();
    let (mut land, mut wet) = (0_u32, 0_u32);
    for (n, cell) in (0_u32..).zip(&run.working.cells) {
        let runoff = annual_runoff_kg_m2_s(&pass, &run.working, n);
        if seas::under_sea(cell) {
            assert_same_bits(runoff, 0.0);
        } else {
            let p = annual[usize::try_from(parent_index(n)).unwrap()];
            assert_same_bits(runoff, (1.0 - lfpm::SEA_LEVEL_RECYCLED_SHARE) * p);
            land += 1;
            wet += u32::from(p > 0.0);
        }
    }
    eprintln!("runoff: {land} land cells, {wet} with rain");
    assert!(land > 0 && wet > 0);
}

/// A locked world's circulation converges on its substellar point, uncapped (decision-r09-t13ge
/// §2.10, §2.12; Design note 8): the reference Earth locked 1:1 to its star on a circular orbit
/// with no obliquity (one month), all sea, at level 5. Its energy-flux equator is the tidally locked
/// pole, the substellar point, and its centre that equator; every moving wind on the day side
/// blows towards the substellar point; and the rain is heaviest near it.
#[test]
fn a_locked_aquaplanet_s_winds_converge_on_its_substellar_point() {
    let run = run_world(locked_earth(0.0, 0.0), 5, Grid::Own, flood_all);
    let water = run.water();
    assert_eq!(water.circulation.cap_deg(), None);
    let cycle = &water.months[0];
    assert_same_bits(cycle.budget.centre_sine, cycle.budget.efe_sine);
    assert!(cycle.budget.efe_sine > 0.99, "{}", cycle.budget.efe_sine);
    let (mut towards, mut day, mut wettest) = (0_u32, 0_u32, (0.0_f64, 0.0_f64));
    for (state, &up) in run.working.climate.iter().zip(run.climate.centres()) {
        let gamma_cosine = up[0];
        let p = state.month_precipitation_kg_m2_s[0];
        if p > wettest.0 {
            wettest = (p, gamma_cosine);
        }
        let sample = &state.month_wind[0];
        if !(0.05..0.95).contains(&gamma_cosine) || sample.speed.value() <= 0.0 {
            continue;
        }
        day += 1;
        let (east, north) = wind::east_north(up);
        let (sin, cos) = math::sin_cos(sample.azimuth.value());
        let blowing = [0, 1, 2].map(|k| cos * north[k] + sin * east[k]);
        // The substellar direction along the surface, +x less its part along up.
        let substellar = [1.0 - up[0] * up[0], -up[0] * up[1], -up[0] * up[2]];
        let along =
            blowing[0] * substellar[0] + blowing[1] * substellar[1] + blowing[2] * substellar[2];
        towards += u32::from(along > 0.0);
    }
    eprintln!(
        "locked aquaplanet: EFE sine {:.6}, Hadley edge {:.1}°; {towards} of {day} dayside winds \
         towards the substellar point; the wettest cell {:.2} mm/day at {:.1}° from it",
        cycle.budget.efe_sine,
        water.circulation.edge_deg(),
        wettest.0 * DAY_S,
        math::acos(wettest.1).to_degrees()
    );
    assert!(day > 0 && towards == day, "{towards} of {day}");
    assert!(wettest.1 > math::cos(30_f64.to_radians()), "{}", wettest.1);
}

/// The coarse pass's reference Earth (its generated continents and R09.T12.e's sea, level 8)
/// through the climate step, printed beside GPCP and NCEP/NCAR R1 as the synthetic Earth is, with
/// the step's cost, the model and the water cycle apart, one thread. A recorded measurement of the
/// shipped generator's own Earth (decision-r09-t13e-asbuilt §1.7): it asserts the water balance in
/// every month ([`assert_the_water_balances`]) and the zonal budget's E − P pattern in each
/// hemisphere ([`assert_the_budget_s_pattern`]), but never its precipitation windows, since its
/// generated midlatitudes are mostly sea and its southern margin a few per cent. It is also the
/// ruled slow test of the orographic solve (decision-r09-t13e-lfpm-convergence §3.5): the water
/// cycle run again at the deep tolerance on a clone of the finished state, every month converges
/// before the cap and closes its budget, and every land climate cell's monthly and annual rate
/// lies within 1% of the deep solve's ([`assert_the_solve_converges`]).
#[test]
#[ignore = "slow: the reference Earth's coarse pass to the climate at level 8, and its water cycle \
            again at the deep tolerance, about a minute"]
fn the_coarse_pass_s_reference_earth_s_water_cycle_against_gpcp() {
    let inputs = reference::earth_like();
    let level = hyperion_surface::field::coarse_level(inputs.radius());
    let (field, climate) = (CellGraph::for_field(level), CellGraph::for_climate(level));
    let (working, diagnostics, deep) = {
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
        let mut working = Working::initial(&pass);
        steps::plates::run(&pass, &mut working);
        steps::relief::run(&pass, &mut working);
        steps::craters::run(&pass, &mut working);
        steps::relief::run_after_craters(&pass, &mut working);
        let start = std::time::Instant::now();
        let model = normalised(&pass, &working);
        let modelled = start.elapsed().as_secs_f64();
        let kept = model.clone();
        let start = std::time::Instant::now();
        let diagnostics = finish(&pass, &mut working, model);
        let finished = start.elapsed().as_secs_f64();
        let start = std::time::Instant::now();
        let again = hydrology::run(&pass, &mut working, &kept);
        let cycle = start.elapsed().as_secs_f64();
        assert_eq!(again.as_ref(), diagnostics.water.as_ref());
        let mut deep_working = working.clone();
        let start = std::time::Instant::now();
        let deep = hydrology::run_with(&pass, &mut deep_working, &kept, &Parameters::deep())
            .expect("the Earth's model keeps its own records");
        let deep_s = start.elapsed().as_secs_f64();
        let land = working.cells.iter().filter(|c| !seas::under_sea(c)).count();
        eprintln!(
            "level {}: the model {modelled:.2} s, the rest {finished:.2} s, of which the water \
             cycle {cycle:.2} s; the step without it {:.2} s and with it {:.2} s; {land} of {} \
             cells land; τ {TOLERANCE:e}; the deep water cycle {deep_s:.2} s",
            level.get(),
            modelled + finished - cycle,
            modelled + finished,
            working.cells.len()
        );
        (working, diagnostics, (deep, deep_working.climate))
    };
    let run = Run {
        field,
        climate,
        inputs,
        working,
        diagnostics,
    };
    report("The reference Earth (the coarse pass)", &run);
    assert_the_water_balances(&run);
    assert_the_budget_s_pattern(&run);
    assert_the_solve_converges(&run, &deep.0, &deep.1);
}
