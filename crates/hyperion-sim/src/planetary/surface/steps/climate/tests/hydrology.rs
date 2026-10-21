//! R09.T13.e's tests: the precipitation heuristic and the winds, wired into the step
//! (decision-r09-t13ge §2.13's, with the water cycle's own), as R09.T13.j's revision for the land's
//! water amends them (decision-r09-t13e-revision §6.3 and its plan text (b)). The orographic
//! model's own (the ridge, the inland decay, the land's vapour source, the runoff's share) are
//! `lfpm`'s.

use hyperion_surface::field::{
    FieldView, LogPrecipitation, PaletteRole, PrecipitationSource, Wind, coarse_level,
};
use hyperion_surface::spheroid::Spheroid;

use super::super::coordinates::Coordinates;
use super::super::ebm::{EnergyBalance, Freezing, ICE_FREE_ALBEDO};
use super::super::grid::{BandStencil, LATITUDE_BANDS, ModelGrid};
use super::super::hydrology::{self, BandWinds, Evaporation, MonthCycle, SeaBands};
use super::super::lfpm::{
    self, CellWind, Cover, DOWNWIND_TOLERANCE, EARTH_DEEP_TOLERANCE, GroundState, MAX_RESTARTS,
    Mesh, Month, Parameters, RATE_FLOOR_SHARE, TOLERANCE,
};
use super::super::partition;
use super::super::wind;
use super::super::{
    Diagnostics, annual_runoff_kg_m2_s, energy_balance, finish, month_shares, normalised,
    run_diagnosed, seas,
};
use super::normalisation::titan_with_basins;
use super::other_regimes::venus_like;
use super::{
    EARTH_HYPSOMETRY_M, SEA_LEVEL_LAND, builder_of, direction, grids, locked_earth, set_ground,
    synthetic_sea,
};
use crate::math;
use crate::planetary::surface::grid::{CellGraph, parent_index};
use crate::planetary::surface::inputs::{CoarseInputs, SeasonalOrbit, Spin};
use crate::planetary::surface::steps::{Pass, Working};
use crate::planetary::surface::{quantise, reference, steps};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Metres, Pascals, Radians, Seconds};
use hyperion_testkit::float::assert_same_bits;

/// Seconds in a day, for rates in mm of water a day (1 kg m⁻² of water is 1 mm).
const DAY_S: f64 = 86_400.0;

/// Köppen–Geiger's arid climates' bound on the mean annual precipitation as a multiple of its
/// threshold, "MAP < 10 × `P_threshold`" (Peel, M. C., Finlayson, B. L., and McMahon, T. A. 2007,
/// Hydrol. Earth Syst. Sci. 11, 1633, Table 1; "Peel et al. 2007" below), as
/// `hyperion_surface::material::classify` codes its "Arid" regime (plan 10's T1.b rule), whose
/// constants are crate-private.
const KOPPEN_ARID_MULTIPLE: f64 = 10.0;

/// Köppen–Geiger's threshold's offset where the summer half-year holds 70% of the precipitation,
/// mm a year: `P_threshold` = 2 MAT + 28 (Peel et al. 2007, Table 1).
const KOPPEN_SUMMER_OFFSET_MM: f64 = 28.0;

/// Köppen–Geiger's threshold's offset where neither half-year holds 70% of the precipitation, mm a
/// year: `P_threshold` = 2 MAT + 14 (Peel et al. 2007, Table 1). Where the winter
/// half holds it the threshold is 2 MAT, no offset.
const KOPPEN_EVEN_OFFSET_MM: f64 = 14.0;

/// The share of the year's precipitation in one half-year at which Köppen–Geiger's threshold takes
/// that half's offset: 70% (Peel et al. 2007, Table 1; the classifier holds it at or above).
const KOPPEN_SEASONAL_SHARE: f64 = 0.7;

/// The kelvin of 0 °C, which Köppen–Geiger's temperatures are in.
const ZERO_CELSIUS_K: f64 = 273.15;

/// GPCP v2.3's annual zonal-mean precipitation over 1991–2020, mm/day, at each 2.5° band from
/// 88.75° S: the long-term monthly means of NOAA PSL (`precip.mon.ltm.1991-2020.nc`), zonal means
/// of the 2.5° grid, months weighted by their days (the lane's `e2/gpcp-zonal.txt`, read; global
/// mean 2.691 mm/day). GPCP's papers: Adler, R. F., et al. 2003, J. Hydrometeor. 4, 1147–1167
/// (version 2), and Adler, R. F., et al. 2018, Atmosphere 9, 138, doi:10.3390/atmos9040138 (the
/// version 2.3 paper). Printed beside the model, not asserted.
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

/// A box of the sphere that holds the climate cells whose centres lie in it, its bounds included:
/// latitude `south` to `north` and longitude `west` to `east`, degrees, the longitude east of the
/// +x meridian, atan2(y, x), in (−180°, 180°].
#[derive(Debug, Clone, Copy, PartialEq)]
struct Region {
    /// The southern bound, degrees of latitude.
    south: f64,
    /// The northern bound, degrees of latitude.
    north: f64,
    /// The western bound, degrees of longitude.
    west: f64,
    /// The eastern bound, degrees of longitude.
    east: f64,
}

impl Region {
    /// The band from latitude `south` to `north`, degrees, at every longitude.
    const fn band(south: f64, north: f64) -> Self {
        Self {
            south,
            north,
            west: -180.0,
            east: 180.0,
        }
    }

    /// Whether it holds the centre at latitude `latitude` and longitude `longitude`, degrees.
    fn holds(&self, latitude: f64, longitude: f64) -> bool {
        (self.south..=self.north).contains(&latitude)
            && (self.west..=self.east).contains(&longitude)
    }
}

/// Eurasia's interior (decision-r09-t13e-revision §6.3, new assertion 1): 50–60° N, 40–100° E,
/// its bounds the ruling's estimate script `model1d.py`'s (§3.1).
const EURASIA_INTERIOR: Region = Region {
    south: 50.0,
    north: 60.0,
    west: 40.0,
    east: 100.0,
};

/// Eurasia's Atlantic margin, which its interior is measured against (decision-r09-t13e-revision
/// §6.3): 50–60° N, 10° W–20° E, its bounds the ruling's estimate script `model1d.py`'s (§3.1).
const EURASIA_COAST: Region = Region {
    south: 50.0,
    north: 60.0,
    west: -10.0,
    east: 20.0,
};

/// The latitudes whose arid share R10 needs held (decision-r09-t13e-revision §6.3, new assertion
/// 2): 40–70° N, its bounds the ruling's estimate script `model1d.py`'s (§3.1).
const ARID_NORTH: Region = Region::band(40.0, 70.0);

/// The latitudes whose arid share is printed beside [`ARID_NORTH`]'s: 60° S–70° N, its bounds the
/// ruling's estimate script `model1d.py`'s (decision-r09-t13e-revision §3.1).
const ARID_WIDE: Region = Region::band(-60.0, 70.0);

/// The Antarctic plateau's latitudes (decision-r09-t13e-revision §6.3, printed): 75° S and south,
/// its climate cells taken where their four field cells' mean height exceeds
/// [`PLATEAU_HEIGHT_M`]; its bounds the ruling's estimate script `model1d.py`'s (§3.1).
const ANTARCTIC: Region = Region::band(-90.0, -75.0);

/// The height above the sea level above which an Antarctic climate cell is on the plateau, m:
/// 2 km, §6.3's "land cells south of 75° S above 2 km" (decision-r09-t13e-revision).
const PLATEAU_HEIGHT_M: f64 = 2_000.0;

/// The Amazon's box (decision-r09-t13e-revision §6.3, printed): 10° S–2° N, 72°–50° W, its
/// bounds the ruling's estimate script `model1d.py`'s (§3.1).
const AMAZON: Region = Region {
    south: -10.0,
    north: 2.0,
    west: -72.0,
    east: -50.0,
};

/// Central Asia's box (decision-r09-t13e-revision §6.3 and E-P9, printed): 43–50° N, 55–80° E,
/// its bounds the ruling's estimate script `model1d.py`'s (§3.1).
const CENTRAL_ASIA: Region = Region {
    south: 43.0,
    north: 50.0,
    west: 55.0,
    east: 80.0,
};

/// The E-L7 patch (decision-r09-t13e-lfpm-convergence §5, decision-r09-t13e-revision §5.1): the
/// coarse pass's reference Earth's land convergence line near 1.11° N, 54.26° E, degrees of
/// latitude and longitude.
const E_L7_PATCH_DEG: (f64, f64) = (1.11, 54.26);

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

    /// Each climate cell's count of land field cells, those not under liquid
    /// ([`seas::under_sea`]): 4 where it is all land, 0 where it is all sea.
    fn land_counts(&self) -> Vec<u8> {
        self.working
            .cells
            .as_chunks::<4>()
            .0
            .iter()
            .map(|cells| {
                cells
                    .iter()
                    .map(|cell| u8::from(!seas::under_sea(cell)))
                    .sum()
            })
            .collect()
    }

    /// Each climate cell's four field cells' mean height above the sea level, m: the solid-angle
    /// mean of their water surfaces less the sea level (the ground's, where dry), as its rate is
    /// theirs.
    fn mean_heights_m(&self) -> Vec<f64> {
        let sea_level = self.working.sea_level.value();
        self.working
            .cells
            .as_chunks::<4>()
            .0
            .iter()
            .zip(self.field.solid_angles_sr().as_chunks::<4>().0)
            .map(|(cells, omegas)| {
                let (sum, area) =
                    cells
                        .iter()
                        .zip(omegas)
                        .fold((0.0, 0.0), |(sum, area), (cell, &omega)| {
                            (
                                sum + omega * (cell.water_surface.value() - sea_level),
                                area + omega,
                            )
                        });
                sum / area
            })
            .collect()
    }

    /// The solid-angle mean of `values` (one a climate cell) over the climate cells `keep` admits
    /// by their index and their centre's latitude and longitude ([`latitude_longitude_deg`]), and
    /// their count; NaN where it admits none.
    fn mean_where(&self, values: &[f64], keep: impl Fn(usize, f64, f64) -> bool) -> (f64, u32) {
        let (mut sum, mut area, mut cells) = (0.0, 0.0, 0_u32);
        for (c, ((centre, &omega), &v)) in self
            .climate
            .centres()
            .iter()
            .zip(self.climate.solid_angles_sr())
            .zip(values)
            .enumerate()
        {
            let (latitude, longitude) = latitude_longitude_deg(*centre);
            if keep(c, latitude, longitude) {
                sum += omega * v;
                area += omega;
                cells += 1;
            }
        }
        (if area > 0.0 { sum / area } else { f64::NAN }, cells)
    }

    /// Each climate cell's Köppen–Geiger inputs: its mean precipitation in mm a Julian year,
    /// Σ sₘ Pₘ × 31,557,600 s (1 kg m⁻² of water is 1 mm); its annual mean temperature at its own
    /// height, its sea-level temperature less Γ times its four field cells' mean height
    /// ([`Run::mean_heights_m`]); and the share of its precipitation, each month's sₘ Pₘ, in its
    /// six warmest months by its month temperature `T_sl` + aₘ, ties to the earlier month, summed
    /// warmest first, as the classifier's `summer_share` does (none in a one-month year).
    fn koppen(&self) -> Vec<Koppen> {
        let months = usize::from(self.working.months);
        let shares = month_shares(&self.pass(), months);
        let lapse = self.working.lapse_rate_k_per_m;
        self.working
            .climate
            .iter()
            .zip(self.mean_heights_m())
            .map(|(state, height_m)| {
                let fallen = |m: usize| shares[m] * state.month_precipitation_kg_m2_s[m];
                let precipitation_mm =
                    (0..months).fold(0.0, |sum, m| sum + fallen(m)) * SECONDS_PER_JULIAN_YEAR;
                let sea_level_k = state.sea_level_temperature.value();
                let celsius = sea_level_k - lapse * height_m - ZERO_CELSIUS_K;
                let summer_share = (months == 12 && precipitation_mm > 0.0).then(|| {
                    let mut warmest: [(f64, usize); 12] =
                        core::array::from_fn(|m| (sea_level_k + state.month_anomaly_k[m], m));
                    warmest.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                    warmest[..6]
                        .iter()
                        .fold(0.0, |sum, &(_, m)| sum + fallen(m))
                        * SECONDS_PER_JULIAN_YEAR
                        / precipitation_mm
                });
                Koppen {
                    precipitation_mm,
                    celsius,
                    summer_share,
                }
            })
            .collect()
    }

    /// Whether each climate cell is arid by Köppen–Geiger's bound at its own temperature
    /// ([`is_arid`]) on its inputs ([`Run::koppen`]).
    fn arid(&self) -> Vec<bool> {
        self.koppen()
            .iter()
            .map(|k| is_arid(k.precipitation_mm, k.celsius, k.summer_share))
            .collect()
    }
}

/// A climate cell's Köppen–Geiger inputs ([`Run::koppen`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Koppen {
    /// Its mean precipitation, mm a Julian year.
    precipitation_mm: f64,
    /// Its annual mean temperature at its own height, °C.
    celsius: f64,
    /// The share of its precipitation in its six warmest months; `None` in a one-month year or
    /// where nothing falls.
    summer_share: Option<f64>,
}

/// Which half-year holds a climate cell's precipitation, by Köppen–Geiger's 70% (Peel et al. 2007,
/// Table 1; [`KOPPEN_SEASONAL_SHARE`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WarmHalf {
    /// Its six warmest months hold at least 70% of the year's precipitation: summer-heavy.
    Summer,
    /// Its six coldest do: winter-heavy.
    Winter,
    /// Neither does, or the year has one month.
    Neither,
}

impl WarmHalf {
    /// The half-year of a cell whose six warmest months hold `summer_share` of its precipitation
    /// (`None` in a one-month year or where nothing falls), the summer tested first, as the
    /// classifier tests it.
    fn of(summer_share: Option<f64>) -> Self {
        match summer_share {
            Some(summer) if summer >= KOPPEN_SEASONAL_SHARE => Self::Summer,
            Some(summer) if 1.0 - summer >= KOPPEN_SEASONAL_SHARE => Self::Winter,
            _ => Self::Neither,
        }
    }
}

/// Whether a climate cell whose mean precipitation is `precipitation_mm` (a Julian year) and whose
/// annual mean temperature at its own height is `celsius` (°C) is arid, `summer_share` the share of
/// its precipitation in its six warmest months (`None` in a one-month year): coded exactly as
/// `hyperion_surface::material::classify` documents its "Arid" regime (plan 10's T1.b rule; the
/// classifier's predicate, `is_arid`, is crate-private). It is arid where nothing falls, or where
/// MAP < 10 (2T + offset) mm a year, the offset 28 mm where the summer half holds at least 70% of
/// the precipitation, none where the winter half does, and 14 mm otherwise or in a one-month year
/// ([`WarmHalf::of`]; Peel et al. 2007, Hydrol. Earth Syst. Sci. 11, 1633, Table 1;
/// [`KOPPEN_ARID_MULTIPLE`]).
fn is_arid(precipitation_mm: f64, celsius: f64, summer_share: Option<f64>) -> bool {
    if precipitation_mm <= 0.0 {
        return true;
    }
    let offset = match WarmHalf::of(summer_share) {
        WarmHalf::Summer => KOPPEN_SUMMER_OFFSET_MM,
        WarmHalf::Winter => 0.0,
        WarmHalf::Neither => KOPPEN_EVEN_OFFSET_MM,
    };
    precipitation_mm < KOPPEN_ARID_MULTIPLE * (2.0 * celsius + offset)
}

/// The latitude and longitude of the body-frame unit vector `d`, degrees: the longitude
/// atan2(y, x), east of the +x meridian, as [`assert_the_solve_converges`] reads it.
fn latitude_longitude_deg(d: [f64; 3]) -> (f64, f64) {
    (latitude_deg(d), math::atan2(d[1], d[0]).to_degrees())
}

/// a · b.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// max(0, `x`), as the water cycle clamps a rate: `x` where it is positive, else zero.
fn positive(x: f64) -> f64 {
    if x > 0.0 { x } else { 0.0 }
}

/// The index into a vector of cell `cell`.
fn slot(cell: u32) -> usize {
    usize::try_from(cell).expect("a cell index fits a usize")
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
/// day from its sidereal one on its year, under 1 bar, Guendelman and Kaspi 2019's (ApJ 881, 67)
/// Earth-like atmosphere (so that its cap is decision-r09-t13ge §2.10's figure; the reference
/// Earth's 985.5 hPa moves it by 0.2%).
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
    if let Some(means) = &run.diagnostics.surface_means {
        eprintln!(
            "{name}: the energy balance's steady surface mean {:.3} K ({:?}), its seasonal one \
             {:.3} K after {} orbits; the mean's correction {:?}; T13.d's constant c {:+.3} K",
            means.steady_k,
            means.optical_depth,
            means.seasonal_k,
            means.orbits,
            means.correction,
            run.diagnostics.correction.mean_k
        );
        if let super::super::ebm::MeanCorrection::Searched {
            search,
            answer,
            g1,
            g1_probe,
            defect,
            g2_start_k,
            g2,
            kept,
        } = &means.correction
        {
            let mean = run.inputs.mean_surface_temperature().value();
            for (i, record) in search.iter().enumerate() {
                eprintln!("{name}:   Z{i} {record:?}");
            }
            eprintln!(
                "{name}: the answer {answer:?}; G1 {g1:?}; d and S_Z (and its pair) {defect:?}; G2 \
                 {g2:?}, from a start of surface mean {g2_start_k:?} K, its miss {:+.4} K; kept \
                 {kept:?}",
                g2.mean_k - mean
            );
            // G1's probe stop (decision-r09-t13i's §2.4 follow-ups 8 §1 and 9 §1).
            eprintln!(
                "{name}: G1 {} orbits, its probe ρ_n and ρ_n−1 {:?}, r* {:?}, R̄ {:?} K, T̂ {:?} K, \
                 d {:?} K; G2 from {g2_start_k:?} K, {} orbits, {:.4} K",
                g1.orbits,
                g1_probe.as_ref().and_then(|probe| probe.mean_ratios),
                g1_probe.as_ref().map(|probe| probe.ratio),
                g1_probe.as_ref().map(|probe| probe.remainder_k),
                g1_probe.as_ref().map(|probe| probe.limit_k),
                defect.map(|(d, _)| d),
                g2.orbits,
                g2.mean_k
            );
        }
        if let Some((annual, _)) = &means.transports {
            let radius = run.inputs.radius().value();
            let scale = 2.0 * core::f64::consts::PI * radius * radius / 1e15;
            let peak = |values: &[f64]| {
                values
                    .iter()
                    .fold((0.0_f64, 0.0_f64), |(north, south), &v| {
                        (north.max(v * scale), south.min(v * scale))
                    })
            };
            let ocean: Vec<f64> = annual
                .ekman_w_m2
                .iter()
                .zip(&annual.gyres_w_m2)
                .map(|(e, g)| e + g)
                .collect();
            eprintln!(
                "{name}: the annual transports' peaks, PW: the atmosphere's {:+.2?}, the ocean's \
                 {:+.2?} (the Ekman cells' {:+.2?})",
                peak(&annual.atmosphere_w_m2),
                peak(&ocean),
                peak(&annual.ekman_w_m2)
            );
        }
    }
    report_bands(name, run);
    report_summary(run);
    report_months(run);
    report_winds(run);
}

/// [`report`]'s bands: the annual zonal precipitation, its parts and GPCP's, and the zonal
/// budget's sea and source (decision-r09-t13e-revision §4.1, §4.2): k `P_sea`, `e_b`, `s_b` and
/// max(0, C), each month's band value weighted by its share.
fn report_bands(name: &str, run: &Run) {
    let annual = run.annual_precipitation();
    let zonal = run.zonal(&annual);
    let grid = ModelGrid::zonal();
    // Each climate cell's cover: its four field cells all sea, all land, or mixed.
    let land_of = run.land_counts();
    let part = |wanted: u8| -> Vec<f64> {
        let (mut sums, mut areas) = (vec![0.0; LATITUDE_BANDS], vec![0.0; LATITUDE_BANDS]);
        for (((centre, &omega), &p), &l) in run
            .climate
            .centres()
            .iter()
            .zip(run.climate.solid_angles_sr())
            .zip(&annual)
            .zip(&land_of)
        {
            if l == wanted {
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
    let (sea, land) = (part(0), part(4));
    let rates = annual_bands(run, |c, k| c.scale * c.sea.rates_kg_m2_s[k]);
    let sea_evaporation = annual_bands(run, |c, k| c.sea.evaporation_kg_m2_s[k]);
    let sea_shares = annual_bands(run, |c, k| c.sea.sea_shares[k]);
    let convergence = annual_bands(run, |c, k| c.sea.convergence_kg_m2_s[k]);
    let evaporation = annual_bands(run, |c, k| c.band_evaporation_kg_m2_s[k]);
    let budget = annual_budget(run);
    eprintln!(
        "{name}: annual zonal precipitation, mm/day, against GPCP v2.3 (1991–2020); the all-sea \
         and all-land climate cells' means, k P_sea, the bands' e_b, s_b, max(0, C), Ē and E − P"
    );
    for (band, (&p, &gpcp)) in zonal.iter().zip(&GPCP_ANNUAL_MM_PER_DAY).enumerate() {
        eprintln!(
            "  {:7.2}°  {:5.2}  GPCP {gpcp:5.2}  sea {:5.2} land {:5.2}  k P_sea {:5.2}  e_b {:5.2}  \
             s_b {:5.3}  max(0, C) {:5.2}  Ē {:5.2}  E − P {:+5.2}",
            degrees(grid.centre_sine(band)),
            p * DAY_S,
            sea[band] * DAY_S,
            land[band] * DAY_S,
            rates[band] * DAY_S,
            sea_evaporation[band] * DAY_S,
            sea_shares[band],
            convergence[band] * DAY_S,
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
    let sea_rates = annual_bands(run, |c, k| c.sea.rates_kg_m2_s[k]);
    let sea_evaporation = annual_bands(run, |c, k| c.sea.evaporation_kg_m2_s[k]);
    let convergence = annual_bands(run, |c, k| c.sea.convergence_kg_m2_s[k]);
    let evaporation = annual_bands(run, |c, k| c.band_evaporation_kg_m2_s[k]);
    let budget = annual_budget(run);
    for (low, high) in [(40.0, 60.0), (15.0, 35.0), (-60.0, -40.0), (-35.0, -15.0)] {
        eprintln!(
            "  window {low:5.1}° to {high:5.1}°: P {:.2} mm/day, GPCP {:.2}; the zonal budget's \
             P_sea {:.2}, e_b {:.2}, max(0, C) {:.2}, Ē {:.2}, E − P {:+.2}",
            run.window(&annual, low, high) * DAY_S,
            band_window(&GPCP_ANNUAL_MM_PER_DAY, low, high),
            band_window(&sea_rates, low, high) * DAY_S,
            band_window(&sea_evaporation, low, high) * DAY_S,
            band_window(&convergence, low, high) * DAY_S,
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

/// [`report`]'s months: each month's energy-flux equator, centre, factor and rate, its land's
/// frozen share, its field cells' orographic winds along the model's bands, and its orographic
/// solve's iteration, components, source and budget.
fn report_months(run: &Run) {
    for (m, cycle) in run.water().months.iter().enumerate() {
        let solve = cycle.orographic.as_ref().map_or_else(String::new, |d| {
            let c = &d.components;
            format!(
                "; land {} cells, {} restarts, {} sweeps, measure {:.1e}{} (residual {:.1e}); \
                 components of 1: {}, 2: {}, 3–16: {}, > 16: {} ({} cells, the largest {}), the \
                 flow order's backward share {:.4} (the largest's {:.4}); {} below zero; the \
                 source {:.4e} kg/s ({:.3} of inflow + source); the budget's closure {:.1e}",
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
                d.backward_share,
                d.largest_backward_share,
                d.negative_cells,
                d.source_kg_s,
                d.source_kg_s / (d.inflow_kg_s + d.source_kg_s),
                closure(d)
            )
        });
        let winds = &cycle.band_winds;
        eprintln!(
            "  month {m:2}: EFE {:+6.2}°, centre {:+6.2}°, k {:.4}, P {:.3} mm/day, frozen land \
             {:.4}; the field cells' winds along the bands east {} west {} calm {}{}{solve}",
            degrees(cycle.budget.efe_sine),
            degrees(cycle.budget.centre_sine),
            cycle.scale,
            cycle.precipitation_kg_m2_s * DAY_S,
            cycle.frozen_land_share,
            winds.eastward,
            winds.westward,
            winds.calm,
            if cycle.calm { ", wholly calm" } else { "" }
        );
    }
}

/// The orographic solve's land budget's closure, (inflow + source − runoff − outflow) ÷ (inflow +
/// source).
fn closure(d: &lfpm::Diagnostics) -> f64 {
    let given = d.inflow_kg_s + d.source_kg_s;
    (given - d.runoff_kg_s - d.outflow_kg_s) / given
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
/// within 10⁻³ of its inflow, the deep solve meets the stop test at [`EARTH_DEEP_TOLERANCE`] (the
/// ruling's E-L8 §5), and every land climate cell's (any of its four field cells land) monthly and
/// annual rate lies within 1% of the deep solve's, |P − P*| ≤ 0.01 max(P*, 10⁻³ k* `P̄_sea`),
/// `deep` the same water cycle at that tolerance on a clone of the finished state with its records
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
            dd.converged && dd.measure <= EARTH_DEEP_TOLERANCE,
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

/// The regional figures the ruled Earth test asserts (decision-r09-t13e-revision §6.3): annual
/// means over the all-land climate cells, kg m⁻² s⁻¹, or a share of their solid angle; NaN where a
/// region holds none.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Regions {
    /// Eurasia's interior's mean ([`EURASIA_INTERIOR`]).
    interior: f64,
    /// Eurasia's Atlantic margin's mean ([`EURASIA_COAST`]).
    coast: f64,
    /// The arid share of 40–70° N ([`ARID_NORTH`]; [`Run::arid`]).
    arid_north: f64,
}

/// Prints a run's regional figures (decision-r09-t13e-revision §6.3, printed and recorded) and
/// returns those the ruled Earth test asserts: each window's (40–60° and 15–35° in each
/// hemisphere) all-land and all-sea climate cells' annual means beside its whole mean; Eurasia's
/// interior over its Atlantic margin, with their means; the arid shares of 40–70° N and 60° S–70° N
/// ([`Run::arid`]); and the annual means of the Antarctic plateau, the Amazon and Central Asia,
/// each over its all-land climate cells, mm/day. A region with no all-land climate cell prints NaN.
fn report_regions(run: &Run) -> Regions {
    let annual = run.annual_precipitation();
    let land = run.land_counts();
    let mean_of = |values: &[f64], region: Region, cover: u8| {
        run.mean_where(values, |c, latitude, longitude| {
            land[c] == cover && region.holds(latitude, longitude)
        })
    };
    for (low, high) in [(40.0, 60.0), (15.0, 35.0), (-60.0, -40.0), (-35.0, -15.0)] {
        let window = Region::band(low, high);
        let ((on_land, land_cells), (on_sea, sea_cells)) =
            (mean_of(&annual, window, 4), mean_of(&annual, window, 0));
        eprintln!(
            "  window {low:+5.1}° to {high:+5.1}°: P {:.2} mm/day; its all-land climate cells \
             {:.2} ({land_cells} cells), its all-sea {:.2} ({sea_cells})",
            run.window(&annual, low, high) * DAY_S,
            on_land * DAY_S,
            on_sea * DAY_S
        );
    }
    let ((interior, inland), (coast, coastal)) = (
        mean_of(&annual, EURASIA_INTERIOR, 4),
        mean_of(&annual, EURASIA_COAST, 4),
    );
    eprintln!(
        "  Eurasia's interior (50–60° N, 40–100° E; {inland} all-land climate cells) {:.3} mm/day \
         over its Atlantic margin (10° W–20° E; {coastal}) {:.3}: {:.3}",
        interior * DAY_S,
        coast * DAY_S,
        interior / coast
    );
    let arid: Vec<f64> = run
        .arid()
        .into_iter()
        .map(|arid| f64::from(u8::from(arid)))
        .collect();
    let ((arid_north, north_cells), (arid_wide, wide_cells)) =
        (mean_of(&arid, ARID_NORTH, 4), mean_of(&arid, ARID_WIDE, 4));
    eprintln!(
        "  arid by Köppen–Geiger's bound at their own temperatures: {arid_north:.3} of the all-land \
         climate cells' solid angle at 40–70° N ({north_cells} cells), {arid_wide:.3} at 60° S–70° \
         N ({wide_cells})"
    );
    let heights = run.mean_heights_m();
    let (plateau, plateau_cells) = run.mean_where(&annual, |c, latitude, longitude| {
        land[c] == 4 && heights[c] > PLATEAU_HEIGHT_M && ANTARCTIC.holds(latitude, longitude)
    });
    let ((amazon, amazon_cells), (asia, asia_cells)) = (
        mean_of(&annual, AMAZON, 4),
        mean_of(&annual, CENTRAL_ASIA, 4),
    );
    eprintln!(
        "  annual means, mm/day, of the all-land climate cells: the Antarctic plateau (75° S and \
         south, above 2 km; {plateau_cells} cells) {:.3}; the Amazon (10° S–2° N, 72°–50° W; \
         {amazon_cells}) {:.3}; Central Asia (43–50° N, 55–80° E; {asia_cells}) {:.3}",
        plateau * DAY_S,
        amazon * DAY_S,
        asia * DAY_S
    );
    Regions {
        interior,
        coast,
        arid_north,
    }
}

/// Prints a run's balance factor k by month (decision-r09-t13ge §2.8, e8), and returns them.
fn report_balance(run: &Run) -> Vec<f64> {
    let factors: Vec<f64> = run.water().months.iter().map(|c| c.scale).collect();
    let printed: Vec<String> = factors.iter().map(|k| format!("{k:.4}")).collect();
    eprintln!("  the balance's factor k by month: {}", printed.join(", "));
    factors
}

/// The reference Earth's months, from 0, whose spans' midpoints fall in December–February: months
/// 11, 0 and 1, whose midpoints lie 352.8, 17.5 and 47.5 days after 1 January. Its months are
/// equal spans of eccentric anomaly from periapsis ([`month_shares`]; e = 0.0167, a sidereal year),
/// and periapsis falls about 3 January (Earth's perihelion, from memory): 2.5 days after 1 January,
/// which puts the reference Earth's March equinox, at true anomaly 77.05°, on 20 March.
const DJF_MONTHS: [usize; 3] = [11, 0, 1];

/// The reference Earth's months whose spans' midpoints fall in June–August, as [`DJF_MONTHS`]:
/// months 5, 6 and 7, at 169.7, 200.6 and 231.4 days after 1 January.
const JJA_MONTHS: [usize; 3] = [5, 6, 7];

/// Prints E-P11's seasons on a run of the reference Earth (decision-r09-t13e-revision, "E-P11
/// (follow-up)" §4, printed and not asserted), each over the all-land climate cells:
///
/// - the 40–60° N land's DJF and JJA means ([`DJF_MONTHS`], [`JJA_MONTHS`]), each month weighted
///   equally, as GPCP's seasonal means are, beside GPCP v2.3's 1.51 and 2.27 mm/day (the ruling's
///   `obs_gpcp.py`, land by Natural Earth's box share, as its §4 quotes them);
/// - the 40–70° N land's warm-half classes by solid angle ([`WarmHalf::of`] on [`Run::koppen`]'s
///   summer share, the arid test's month ranking): summer-heavy, winter-heavy and neither, beside
///   GPCP's 0.28, 0.02 and 0.70 (the ruling's `arid_seasonality.py`, its §3);
/// - the 40–70° N arid share with Peel's summer offset, 2T + 28, imposed on every cell ([`is_arid`]
///   with a summer share of 1), beside the ruling's estimate of 0.25 (its §3, the lane-like case's
///   0.251), and beside the share on the model's own months.
///
/// Returns the 40–60° N land's DJF and JJA means, kg m⁻² s⁻¹ (decision-r09-t13i §3.4, 2).
fn report_seasons(run: &Run) -> (f64, f64) {
    let land = run.land_counts();
    let mean_of = |values: &[f64], region: Region| {
        run.mean_where(values, |c, latitude, longitude| {
            land[c] == 4 && region.holds(latitude, longitude)
        })
    };
    let season = |months: [usize; 3]| -> Vec<f64> {
        run.working
            .climate
            .iter()
            .map(|state| {
                months
                    .iter()
                    .fold(0.0, |sum, &m| sum + state.month_precipitation_kg_m2_s[m])
                    / 3.0
            })
            .collect()
    };
    let storm_tracks = Region::band(40.0, 60.0);
    let ((winter, cells), (summer, _)) = (
        mean_of(&season(DJF_MONTHS), storm_tracks),
        mean_of(&season(JJA_MONTHS), storm_tracks),
    );
    eprintln!(
        "  E-P11: the 40–60° N all-land climate cells ({cells}) DJF {:.2} mm/day, JJA {:.2}; GPCP \
         1.51 and 2.27",
        winter * DAY_S,
        summer * DAY_S
    );
    let koppen = run.koppen();
    let share_of = |half: WarmHalf| {
        let indicator: Vec<f64> = koppen
            .iter()
            .map(|k| f64::from(u8::from(WarmHalf::of(k.summer_share) == half)))
            .collect();
        mean_of(&indicator, ARID_NORTH)
    };
    let ((summer_heavy, north_cells), (winter_heavy, _), (neither, _)) = (
        share_of(WarmHalf::Summer),
        share_of(WarmHalf::Winter),
        share_of(WarmHalf::Neither),
    );
    eprintln!(
        "  E-P11: the 40–70° N all-land climate cells' ({north_cells}) warm halves by solid angle: \
         summer-heavy {summer_heavy:.3}, winter-heavy {winter_heavy:.3}, neither {neither:.3}; \
         GPCP 0.28, 0.02 and 0.70"
    );
    let arid = |summer_share: fn(&Koppen) -> Option<f64>| {
        let indicator: Vec<f64> = koppen
            .iter()
            .map(|k| {
                f64::from(u8::from(is_arid(
                    k.precipitation_mm,
                    k.celsius,
                    summer_share(k),
                )))
            })
            .collect();
        mean_of(&indicator, ARID_NORTH).0
    };
    eprintln!(
        "  E-P11: arid at 40–70° N with Peel's summer offset (2T + 28) on every cell {:.3}, the \
         ruling's estimate 0.25; on the model's own months {:.3}",
        arid(|_| Some(1.0)),
        arid(|k| k.summer_share)
    );
    (winter, summer)
}

/// The NCEP/NCAR R1 1991–2020 sea's evaporation per unit of the sea's area over the four windows
/// 40–60° N, 15–35° N, 40–60° S and 15–35° S, mm/day (decision-r09-t13i §3.4, 3, E-P8's windows,
/// as decision-r09-t13e-revision's `ncep_evap.txt` gives them): 1.98, 4.69, 1.57 and 4.38. NCEP
/// R1's ocean latent heat flux runs high, by 18–20 W m⁻² (over 15%) against more than 100 flux
/// buoys (Yu, L. 2007, J. Climate 20, 5376, p. 5378, as the lane's science check read it), so the
/// real sea's rates lie lower still.
const NCEP_SEA_EVAPORATION_MM_PER_DAY: [((f64, f64), f64); 4] = [
    ((40.0, 60.0), 1.98),
    ((15.0, 35.0), 4.69),
    ((-60.0, -40.0), 1.57),
    ((-35.0, -15.0), 4.38),
];

/// The observed annual E − P over the same four windows, mm/day, GPCP v2.3 less NCEP/NCAR R1's
/// evaporation and, in brackets, GPCP less NCEP's evaporation scaled by 0.944 to close its global
/// budget on GPCP's (decision-r09-t13i §3.1's table; decision-r09-t13e-revision §1.2): −0.73
/// (−0.83), +1.42 (+1.23), −1.41 (−1.50) and +1.65 (+1.44). The 15–35° N +1.42 is GPCP's 2.005
/// less NCEP's 3.425 (the ruling's `gpcp.windows.txt` and `ncep_evap.txt`; its text prints +1.43).
const OBSERVED_E_MINUS_P_MM_PER_DAY: [(f64, f64); 4] =
    [(-0.73, -0.83), (1.42, 1.23), (-1.41, -1.50), (1.65, 1.44)];

/// The greatest 40–60° N sea's DJF ÷ JJA evaporation per sea area of the ruled Earth
/// (decision-r09-t13i §3.4, 2; NCEP R1 3.75, T13.j 8.9, the ruling's stand-ins 4.2–6.6): 6, a
/// ratio from 6 to [`SEA_SEASON_RATIO_BLOCK`] reported to main.
const SEA_SEASON_RATIO_BOUND: f64 = 6.0;

/// The 40–60° N sea's DJF ÷ JJA above which the ruled Earth's test fails (decision-r09-t13i §3.4,
/// 2): 7.5.
const SEA_SEASON_RATIO_BLOCK: f64 = 7.5;

/// The least 40–60° N all-land JJA mean over its DJF mean of the ruled Earth (decision-r09-t13i
/// §3.4, 2; T13.j 0.32, the ruling's estimate 0.69 at `n_c` = 6 and 0.37 at `n_c` = 4, GPCP 1.50):
/// 0.45, a ratio from [`LAND_SEASON_RATIO_BLOCK`] to 0.45 reported to main.
const LAND_SEASON_RATIO_BOUND: f64 = 0.45;

/// The 40–60° N land's JJA ÷ DJF below which the ruled Earth's test fails (decision-r09-t13i
/// §3.4, 2): 0.35.
const LAND_SEASON_RATIO_BLOCK: f64 = 0.35;

/// The mean over `months` of each month's window over `low` to `high` degrees of a band field
/// `of` the month's cycle ([`band_window`]), each month weighted equally, as GPCP's and NCEP's
/// seasonal means are.
fn season_window(
    run: &Run,
    months: [usize; 3],
    low: f64,
    high: f64,
    of: fn(&MonthCycle) -> &[f64],
) -> f64 {
    let water = run.water();
    months.iter().fold(0.0, |sum, &m| {
        sum + band_window(of(&water.months[m]), low, high)
    }) / 3.0
}

/// Prints the 40–60° N water cycle's months (decision-r09-t13i §3.4, 3) and returns its sea's DJF
/// ÷ JJA evaporation per sea area (§3.4, 2), the band-solid-angle window of `e_b` as T13.j's 8.9
/// was measured (decision-r09-t13e-revision §2), printed beside the same weighted by each band's
/// sea as the ruling's stand-ins weight it: each month's band windows over 40–60° N of the
/// sea's evaporation per sea area `e_b`, the bands' mean evaporation Ē, the sea share `s_b`, the
/// zonal budget's E − P, its convergence C⁺ = max(0, C), the sea's rate `P_sea` before the balance,
/// the balance's k, the land's frozen share and the all-land climate cells' P (as
/// decision-r09-t13e-revision §2's table gives them); the band-window convergence C = −(E − P) in
/// JJA and DJF beside the ruling's stand-in (0.45 and 0.93) and NCEP's own (0.35 and 0.84); and
/// E-P8's windows, the sea's annual evaporation per sea area and E − P in each hemisphere at
/// 40–60° and 15–35°, beside NCEP's and GPCP − NCEP's.
fn report_storm_track_months(run: &Run) -> f64 {
    fn sea(c: &MonthCycle) -> &[f64] {
        &c.sea.evaporation_kg_m2_s
    }
    fn budget(c: &MonthCycle) -> &[f64] {
        &c.budget.evaporation_less_precipitation_kg_m2_s
    }
    let water = run.water();
    let land = run.land_counts();
    let storm_tracks = Region::band(40.0, 60.0);
    eprintln!(
        "  the 40–60° N months, mm/day: e_b, Ē, s_b, E − P, C⁺, P_sea, k, the frozen land share, the \
         all-land climate cells' P"
    );
    for (m, cycle) in water.months.iter().enumerate() {
        let precipitation: Vec<f64> = run
            .working
            .climate
            .iter()
            .map(|state| state.month_precipitation_kg_m2_s[m])
            .collect();
        let (all_land, _) = run.mean_where(&precipitation, |c, latitude, longitude| {
            land[c] == 4 && storm_tracks.holds(latitude, longitude)
        });
        eprintln!(
            "    month {m:2}: e_b {:.2}, Ē {:.2}, s_b {:.3}, E − P {:+.2}, C⁺ {:.2}, P_sea {:.2}, k \
             {:.3}, frozen {:.3}, all-land P {:.2}",
            band_window(&cycle.sea.evaporation_kg_m2_s, 40.0, 60.0) * DAY_S,
            band_window(&cycle.band_evaporation_kg_m2_s, 40.0, 60.0) * DAY_S,
            band_window(&cycle.sea.sea_shares, 40.0, 60.0),
            band_window(
                &cycle.budget.evaporation_less_precipitation_kg_m2_s,
                40.0,
                60.0
            ) * DAY_S,
            band_window(&cycle.sea.convergence_kg_m2_s, 40.0, 60.0) * DAY_S,
            band_window(&cycle.sea.rates_kg_m2_s, 40.0, 60.0) * DAY_S,
            cycle.scale,
            cycle.frozen_land_share,
            all_land * DAY_S
        );
    }
    let (winter_sea, summer_sea) = (
        season_window(run, DJF_MONTHS, 40.0, 60.0, sea),
        season_window(run, JJA_MONTHS, 40.0, 60.0, sea),
    );
    let ratio = winter_sea / summer_sea;
    // The same weighted by each band's sea, Σ w s e ÷ Σ w s, as the ruling's stand-ins weight it.
    let by_sea = |months: [usize; 3]| {
        let grid = ModelGrid::zonal();
        months.iter().fold(0.0, |sum, &m| {
            let sea = &water.months[m].sea;
            let (mut evaporated, mut area) = (0.0, 0.0);
            for band in 0..LATITUDE_BANDS {
                if (40.0..=60.0).contains(&degrees(grid.centre_sine(band))) {
                    let w = grid.weight(band) * sea.sea_shares[band];
                    evaporated += w * sea.evaporation_kg_m2_s[band];
                    area += w;
                }
            }
            sum + evaporated / area
        }) / 3.0
    };
    eprintln!(
        "  the 40–60° N sea's evaporation per sea area: DJF {:.2} mm/day, JJA {:.2}, DJF ÷ JJA \
         {ratio:.2} (at most {SEA_SEASON_RATIO_BOUND}, reported to {SEA_SEASON_RATIO_BLOCK}; NCEP \
         R1 3.75, T13.j 8.9, the ruling's stand-ins 4.2–6.6); weighted by each band's sea {:.2}",
        winter_sea * DAY_S,
        summer_sea * DAY_S,
        by_sea(DJF_MONTHS) / by_sea(JJA_MONTHS)
    );
    eprintln!(
        "  the 40–60° N band-window convergence: JJA {:.2} mm/day, DJF {:.2} (the ruling's stand-in \
         0.45 and 0.93; NCEP's own 0.35 and 0.84)",
        -season_window(run, JJA_MONTHS, 40.0, 60.0, budget) * DAY_S,
        -season_window(run, DJF_MONTHS, 40.0, 60.0, budget) * DAY_S
    );
    let sea_evaporation = annual_bands(run, |c, k| c.sea.evaporation_kg_m2_s[k]);
    let annual = annual_budget(run);
    for (((low, high), ncep), (gpcp_ncep, scaled)) in NCEP_SEA_EVAPORATION_MM_PER_DAY
        .iter()
        .zip(OBSERVED_E_MINUS_P_MM_PER_DAY)
    {
        eprintln!(
            "  E-P8, {low:+.0}° to {high:+.0}°: the sea's evaporation per sea area {:.2} mm/day \
             (NCEP R1 {ncep:.2}); E − P {:+.2} (GPCP − NCEP {gpcp_ncep:+.2}, GPCP − 0.944 NCEP \
             {scaled:+.2})",
            band_window(&sea_evaporation, *low, *high) * DAY_S,
            band_window(&annual, *low, *high) * DAY_S
        );
    }
    ratio
}

/// The ruled Earth test (decision-r09-t13ge §2.13, e9, as decision-r09-t13e-asbuilt §1.5 set it up
/// and decision-r09-t13e-revision §6.3 rules it) on the reference Earth: R09.T13.g's synthetic
/// sea, Natural Earth's land by 2.5° × 5° box, its heights R09.T13.g's band eighths placed by each
/// box's Earth2014 median land height ([`synthetic_earth`]), the whole step at level 7 on its
/// geographic model, printed against GPCP v2.3 and NCEP/NCAR R1. It asserts:
///
/// - in each hemisphere the annual zonal precipitation's solid-angle mean over 40–60° exceeds that
///   over 15–35° (decision-r09-t13ge §2.13; the northern window, printed and not asserted under
///   decision-r09-t13e-asbuilt, plan 09's deferred finding E-P1, is restored by
///   decision-r09-t13e-revision, whose landing acceptance is a northern ratio of at least 1.10;
///   GPCP's ratio, printed beside it, is 1.27 in the north);
/// - in each hemisphere the zonal budget's annual E − P has a negative solid-angle mean over 40–60°
///   and a positive one over 15–35° ([`assert_the_budget_s_pattern`]);
/// - the precipitation's maximum within 30° of the equator lies within 10° of the annual
///   energy-flux equator;
/// - Eurasia's interior is watered: the solid-angle mean of the annual precipitation over the
///   all-land climate cells (all four field cells land) whose centres lie between 50° and 60° N and
///   40° and 100° E is at least 0.4 of that between 10° W and 20° E (GPCP v2.3 1991–2020: 0.59;
///   the ruling's estimate 0.56, the as-built model's 0.21 on it; decision-r09-t13e-revision §6.3);
/// - at most a quarter of the solid angle of the all-land climate cells whose centres lie between
///   40° and 70° N is arid by Köppen–Geiger's bound at their own temperatures ([`Run::arid`],
///   [`is_arid`]), R10's arid ground (Peel et al. 2007, Hydrol. Earth Syst. Sci. 11, 1633, Table
///   1, as plan 10's T1.b rule reads it; GPCP with NCEP R1's temperatures 0.08, the ruling's
///   estimate 0.11, the as-built model's 0.43 on it; decision-r09-t13e-revision §6.3);
/// - every month's balance factor k lies within [0.85, 1.15] (the as-built model's 1.17–1.35,
///   measured 1.1685–1.3492, the ruling's estimate 0.97–1.00; decision-r09-t13e-revision §6.3).
///
/// It prints the windows' all-land and all-sea parts, k by month, the interior ratio and its two
/// means, the arid shares of 40–70° N and 60° S–70° N, the Antarctic plateau, the Amazon and
/// Central Asia ([`report_regions`]), whose observed figures are recorded in plan 09, not here;
/// and E-P11's seasons ([`report_seasons`]; decision-r09-t13e-revision, "E-P11 (follow-up)" §4):
/// the 40–60° N land's DJF and JJA means, the 40–70° N land's warm-half classes, and its arid
/// share with Peel's summer offset imposed on every cell, printed beside GPCP's and the ruling's
/// estimate.
///
/// R09.T13.i's water cycle (decision-r09-t13i §3.4, 2–3) adds:
///
/// - the 40–60° N sea's evaporation per sea area has a DJF ÷ JJA of at most
///   [`SEA_SEASON_RATIO_BOUND`], 6 (NCEP R1 3.75; T13.j 8.9): from 6 to
///   [`SEA_SEASON_RATIO_BLOCK`], 7.5, it is reported, and above it the test fails;
/// - the 40–60° N all-land climate cells' JJA mean is at least [`LAND_SEASON_RATIO_BOUND`], 0.45,
///   of their DJF mean (T13.j 0.32; GPCP 1.50): from [`LAND_SEASON_RATIO_BLOCK`], 0.35, to 0.45 it
///   is reported, and below it the test fails (E-P11's "JJA above DJF" withdrawn with the land
///   store, decision-r09-t13i §3.4, 5, and plan 09's deferred finding I1);
/// - printed ([`report_storm_track_months`]): the 40–60° N months, its band-window JJA and DJF
///   convergence, and E-P8's windows beside NCEP's and GPCP − NCEP's.
#[test]
fn the_reference_earth_s_storm_tracks_are_wetter_than_its_subtropics_its_interiors_are_watered_and_its_rain_band_is_on_its_energy_flux_equator()
 {
    let run = run_world(reference::earth_like(), 7, Grid::Own, synthetic_earth);
    report("The reference Earth (synthetic sea)", &run);
    let regions = report_regions(&run);
    let (land_winter, land_summer) = report_seasons(&run);
    let sea_ratio = report_storm_track_months(&run);
    let land_ratio = land_summer / land_winter;
    eprintln!(
        "  the 40–60° N all-land climate cells' JJA ÷ DJF {land_ratio:.3} (at least \
         {LAND_SEASON_RATIO_BOUND}, reported from {LAND_SEASON_RATIO_BLOCK}; T13.j 0.32, GPCP 1.50)"
    );
    if sea_ratio > SEA_SEASON_RATIO_BOUND {
        eprintln!(
            "  REPORT to main: the 40–60° N sea's DJF ÷ JJA {sea_ratio:.2} exceeds \
             {SEA_SEASON_RATIO_BOUND} (decision-r09-t13i §3.4, 2)"
        );
    }
    if land_ratio < LAND_SEASON_RATIO_BOUND {
        eprintln!(
            "  REPORT to main: the 40–60° N land's JJA ÷ DJF {land_ratio:.3} is under \
             {LAND_SEASON_RATIO_BOUND} (decision-r09-t13i §3.4, 2)"
        );
    }
    let factors = report_balance(&run);
    let annual = run.annual_precipitation();
    let mut windows = Vec::with_capacity(2);
    for (hemisphere, sign) in [("north", 1.0_f64), ("south", -1.0)] {
        let bounds = |low: f64, high: f64| {
            if sign > 0.0 {
                (low, high)
            } else {
                (-high, -low)
            }
        };
        let (at_tracks, at_subtropics) = (bounds(40.0, 60.0), bounds(15.0, 35.0));
        let model = |(low, high): (f64, f64)| run.window(&annual, low, high);
        let gpcp = |(low, high): (f64, f64)| band_window(&GPCP_ANNUAL_MM_PER_DAY, low, high);
        let (tracks, subtropics) = (model(at_tracks), model(at_subtropics));
        let (gpcp_tracks, gpcp_subtropics) = (gpcp(at_tracks), gpcp(at_subtropics));
        eprintln!(
            "  the {hemisphere}ern windows: 40–60° {:.2} mm/day against 15–35° {:.2} ({:.3}); \
             GPCP {gpcp_tracks:.2} against {gpcp_subtropics:.2} ({:.2})",
            tracks * DAY_S,
            subtropics * DAY_S,
            tracks / subtropics,
            gpcp_tracks / gpcp_subtropics
        );
        windows.push((hemisphere, tracks, subtropics));
    }
    for (hemisphere, tracks, subtropics) in windows {
        assert!(
            tracks > subtropics,
            "{hemisphere}: 40–60° {} mm/day against 15–35° {}",
            tracks * DAY_S,
            subtropics * DAY_S
        );
    }
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
    assert!(
        regions.interior >= 0.4 * regions.coast,
        "Eurasia's interior {} mm/day against its Atlantic margin's {}",
        regions.interior * DAY_S,
        regions.coast * DAY_S
    );
    assert!(
        regions.arid_north <= 0.25,
        "{} of 40–70° N's land arid",
        regions.arid_north
    );
    for (m, k) in factors.iter().enumerate() {
        assert!((0.85..=1.15).contains(k), "month {m}: k {k}");
    }
    assert!(
        sea_ratio <= SEA_SEASON_RATIO_BLOCK,
        "the 40–60° N sea's DJF ÷ JJA {sea_ratio}"
    );
    assert!(
        land_ratio >= LAND_SEASON_RATIO_BLOCK,
        "the 40–60° N land's JJA ÷ DJF {land_ratio}"
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

/// Frozen ground on the ruled Earth's inputs (decision-r09-t13e-revision §4.4 and §6.3's unit test
/// 7): the reference Earth's synthetic sea at level 6 ([`synthetic_earth`]), as
/// [`global_precipitation_equals_global_evaporation`] runs it. For every month and land field
/// cell the test computes its month temperature at its height, `T_sl` − Γ (`z_w` − `z_sea`) + aₘ in
/// that order, against the model's freezing point `T_f` (`EnergyBalance::for_inputs`'s
/// `Freezing::At`): [`hydrology::ground_of`] agrees with it; the orographic model's recycled share
/// on that ground is none below `T_f`, and above it bit for bit `recycled_share(H)`, which is
/// ε₀ e^(−H/H₀) to 10⁻¹², H its height above the sea level read as 0 where negative; and each
/// month's frozen share of the land's solid angle is the step's `frozen_land_share` to the bit,
/// both summing Ω over the frozen land cells and over the land cells in cell order, then dividing.
/// Some month holds both frozen and thawed land.
///
/// The twelve-month runoff (§4.5): every land field cell's [`annual_runoff_kg_m2_s`] is, to the
/// bit, the test's own Σₘ sₘ (1 − εₘ) Pₘ in month order, sₘ the month's share of the year
/// ([`month_shares`]), Pₘ its climate cell's month rate and εₘ the test's own, `recycled_share(H)`
/// in a thawed month and none in a frozen one; and some land cell has both frozen and thawed
/// months.
/// On a world where no ice forms (`Freezing::Never`), the coldest land cell's coldest month, far
/// below `T_f`, is thawed. Prints each month's frozen share and the runoff's counts.
#[test]
fn on_the_reference_earth_frozen_ground_recycles_nothing_and_thawed_ground_its_share_at_its_height()
{
    let run = run_world(reference::earth_like(), 6, Grid::Own, synthetic_earth);
    let water = run.water();
    assert_eq!(water.months.len(), 12);
    let freezing = EnergyBalance::for_inputs(&run.inputs).freezing;
    let Freezing::At { point_k } = freezing else {
        panic!("water freezes on the reference Earth: {freezing:?}");
    };
    let parameters = Parameters::HERGARTEN_ROBL;
    let (lapse, sea_level) = (
        run.working.lapse_rate_k_per_m,
        run.working.sea_level.value(),
    );
    let pass = run.pass();
    let shares = month_shares(&pass, water.months.len());
    let cells = run.working.cells.len();
    // Each field cell's Σₘ sₘ (1 − εₘ) Pₘ so far, and whether it has had a frozen month and a
    // thawed one.
    let (mut runoff, mut kinds) = (vec![0.0; cells], vec![(false, false); cells]);
    let mut coldest: Option<(u32, usize, f64)> = None;
    let mut both = false;
    for (m, cycle) in water.months.iter().enumerate() {
        let (mut land_area, mut frozen_area) = (0.0, 0.0);
        for (n, (cell, &omega)) in
            (0_u32..).zip(run.working.cells.iter().zip(run.field.solid_angles_sr()))
        {
            if seas::under_sea(cell) {
                continue;
            }
            let state = &run.working.climate[slot(parent_index(n))];
            let height_m = cell.water_surface.value() - sea_level;
            let surface_k =
                state.sea_level_temperature.value() - lapse * height_m + state.month_anomaly_k[m];
            let frozen = surface_k < point_k;
            let ground = hydrology::ground_of(&run.working, n, m, freezing);
            let expected = if frozen {
                GroundState::Frozen
            } else {
                GroundState::Thawed
            };
            assert_eq!(ground, expected, "cell {n}, month {m}: {surface_k} K");
            let share = parameters.recycled_share_on(height_m, ground);
            let epsilon = if frozen {
                assert_same_bits(share, 0.0);
                frozen_area += omega;
                0.0
            } else {
                let epsilon = parameters.recycled_share(height_m);
                assert_same_bits(share, epsilon);
                let by_hand = lfpm::SEA_LEVEL_RECYCLED_SHARE
                    * math::exp(-height_m.max(0.0) / lfpm::HEIGHT_SCALE_M);
                assert!(
                    (share - by_hand).abs() <= 1e-12,
                    "cell {n}, month {m}: ε {share} against {by_hand} at {height_m} m"
                );
                epsilon
            };
            land_area += omega;
            let i = slot(n);
            runoff[i] += shares[m] * ((1.0 - epsilon) * state.month_precipitation_kg_m2_s[m]);
            if frozen {
                kinds[i].0 = true;
            } else {
                kinds[i].1 = true;
            }
            if coldest.is_none_or(|(_, _, k)| surface_k < k) {
                coldest = Some((n, m, surface_k));
            }
        }
        let frozen_share = frozen_area / land_area;
        eprintln!(
            "  month {m:2}: frozen ground on {frozen_share:.4} of the land's solid angle (the \
             step's {:.4})",
            cycle.frozen_land_share
        );
        assert_same_bits(cycle.frozen_land_share, frozen_share);
        both |= frozen_area > 0.0 && frozen_area < land_area;
    }
    assert!(both, "no month holds both frozen and thawed land");
    let coldest = coldest.expect("the reference Earth has land");
    assert_the_annual_runoff(&run, &runoff, &kinds, coldest);
}

/// Asserts the frozen-ground test's twelve-month runoff on `run` (decision-r09-t13e-revision §4.5):
/// every land field cell's [`annual_runoff_kg_m2_s`] is its `runoff` (one a field cell, the test's
/// own Σₘ sₘ (1 − εₘ) Pₘ in month order) to the bit, and some land cell's `kinds` (whether it had a
/// frozen month, and a thawed one) are both; and the coldest land cell's coldest month, `coldest`
/// (its cell, month and temperature at its surface, K), is thawed on a world where no ice forms
/// (`Freezing::Never`). Prints the counts.
fn assert_the_annual_runoff(
    run: &Run,
    runoff: &[f64],
    kinds: &[(bool, bool)],
    coldest: (u32, usize, f64),
) {
    let pass = run.pass();
    let (mut land_cells, mut mixed) = (0_u32, 0_u32);
    for (n, cell) in (0_u32..).zip(&run.working.cells) {
        if seas::under_sea(cell) {
            continue;
        }
        assert_same_bits(
            annual_runoff_kg_m2_s(&pass, &run.working, n),
            runoff[slot(n)],
        );
        land_cells += 1;
        mixed += u32::from(kinds[slot(n)] == (true, true));
    }
    let (n, m, surface_k) = coldest;
    eprintln!(
        "  the annual runoff of {land_cells} land field cells is Σ sₘ (1 − εₘ) Pₘ to the bit; \
         {mixed} of them have both frozen and thawed months; the coldest, cell {n} in month {m}, \
         at {surface_k:.2} K"
    );
    assert!(mixed > 0, "no land cell has both frozen and thawed months");
    assert_eq!(
        hydrology::ground_of(
            &run.working,
            n,
            m,
            Freezing::Never {
                albedo: ICE_FREE_ALBEDO
            }
        ),
        GroundState::Thawed,
        "cell {n}, month {m}, at {surface_k} K where no ice forms"
    );
}

/// Asserts that `wind` blows towards `expected`, to the bit.
#[track_caller]
fn assert_towards(wind: CellWind, expected: [f64; 3]) {
    let CellWind::Towards(towards) = wind else {
        panic!("a calm wind, not one towards {expected:?}");
    };
    for (&a, &e) in towards.iter().zip(&expected) {
        assert_same_bits(a, e);
    }
}

/// Asserts that `wind` is still air.
#[track_caller]
fn assert_calm(wind: CellWind) {
    assert!(matches!(wind, CellWind::Calm), "{wind:?} is not calm");
}

/// The ruled wind test (decision-r09-t13ge §2.13), as decision-r09-t13e-revision §4.3 and §6.3
/// reword it: each month's orographic step reads that month's quantised wind, carrying vapour
/// along its component along the model's bands, east, west or calm by its sign. The step writes
/// each climate cell's month as the record's code decoded, so the quantised field's codes are the
/// ones the step recorded, they decode to the state's samples bit for bit and re-quantise to
/// themselves, and in every month more than half the climate cells' winds move. For every month
/// and field cell, its climate cell's decoded record read at its centre ([`wind::cell_wind`]), ŵ
/// as a unit direction, has an along-band component ŵ · ê, ê the cell's [`wind::model_east`]
/// about the geographic pole +z; the orographic wind [`wind::along_bands`] takes from it is
/// towards ê where that component exceeds [`DOWNWIND_TOLERANCE`], towards −ê where it is below its
/// negative, and calm otherwise or where the record is calm; and each month's
/// [`MonthCycle::band_winds`] counts those senses as the test does over the field cells, which ties
/// the test to the winds the step read. Both eastward and westward winds occur. On the reference
/// Earth's synthetic sea at level 6, on a 1,500 km sphere ([`small`]).
#[test]
fn each_month_s_orographic_step_reads_its_quantised_wind_along_the_model_s_bands() {
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
    let pole = [0.0, 0.0, 1.0];
    let (mut eastward, mut westward) = (0_u32, 0_u32);
    for (m, (codes, cycle)) in water.winds.iter().zip(&water.months).enumerate() {
        let moving = codes.iter().filter(|&&code| code != Wind::CALM).count();
        let mut counts = BandWinds::default();
        for (n, &up) in (0_u32..).zip(run.field.centres()) {
            let sample = &run.working.climate[slot(parent_index(n))].month_wind[m];
            let read = wind::cell_wind(sample, up);
            let east = wind::model_east(pole, up);
            let orographic = wind::along_bands(read, east);
            let along = match (read, east) {
                (CellWind::Towards(towards), Some(ahead)) => {
                    dot(towards, ahead) / dot(towards, towards).sqrt()
                }
                _ => 0.0,
            };
            match east {
                Some(ahead) if along > DOWNWIND_TOLERANCE => {
                    assert_towards(orographic, ahead);
                    counts.eastward += 1;
                }
                Some(ahead) if along < -DOWNWIND_TOLERANCE => {
                    assert_towards(orographic, [-ahead[0], -ahead[1], -ahead[2]]);
                    counts.westward += 1;
                }
                _ => {
                    assert_calm(orographic);
                    counts.calm += 1;
                }
            }
        }
        eprintln!(
            "month {m:2}: {moving} of {} climate cells' winds not calm; the field cells' winds \
             along the bands east {} west {} calm {}",
            codes.len(),
            counts.eastward,
            counts.westward,
            counts.calm
        );
        assert!(moving * 2 > codes.len(), "month {m}: {moving}");
        assert_eq!(cycle.band_winds, counts, "month {m}");
        eastward += counts.eastward;
        westward += counts.westward;
    }
    assert!(
        eastward > 0 && westward > 0,
        "east {eastward}, west {westward}"
    );
    // A field cell's direction is its record's azimuth from its own local north.
    let n = 1_000_u32;
    let up = run.field.centre(n);
    let sample = &run.working.climate[slot(parent_index(n))].month_wind[0];
    if let CellWind::Towards(towards) = wind::cell_wind(sample, up) {
        let (east, north) = wind::east_north(up);
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
/// rain-band centre lies poleward of the ascending-branch cap, 37.4° (Guendelman and Kaspi 2019,
/// ApJ 881, 67, doi:10.3847/1538-4357/ab2a06 (arXiv:1906.05748), §4.4, Eq. 20, at 60°, 1 bar, Ω⊕
/// and a year); an equatorial cell's meridional wind changes sign twice a year, each month's
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

/// The bands of the sea's-rate test that have cells but no sea, each with the band whose sea it
/// takes, its nearest that has one, equatorward first (T13.a's rule): in the south band 10 takes
/// its equatorward neighbour 11, band 20 its poleward one 19 (its equatorward 21 has none) and
/// band 21 its equatorward 22; in the north band 60 takes its equatorward 59.
const SEALESS_BANDS: [(usize, usize); 4] = [(10, 11), (20, 19), (21, 22), (60, 59)];

/// The sea's-rate test's converging band, E − P < 0.
const CONVERGING_BAND: usize = 40;

/// The sea's-rate test's diverging band, whose sea still rains: E − P > 0 and `e_b` + `C_b` ÷
/// `s_b` > 0.
const DIVERGING_BAND: usize = 30;

/// The sea's-rate test's strongly diverging band, whose export outweighs its sea's evaporation:
/// `e_b` + `C_b` ÷ `s_b` < 0.
const EXPORTING_BAND: usize = 25;

/// The sea's rate (decision-r09-t13e-revision §4.1 and §6.3's unit test 8, its synthetic case):
/// [`SeaBands::of`] on 72 bands, each with cells, their solid angles, seas, evaporation and E − P
/// set by hand. A band's `e_b` is its evaporation over its sea's solid angle and `s_b` its sea's
/// share of its own; a converging band's sea rains `e_b` + `C_b`, `C_b` = −(E − P)
/// ([`CONVERGING_BAND`]); a diverging band's `e_b` + `C_b` ÷ `s_b`, its export falling on its sea
/// alone ([`DIVERGING_BAND`]); a strongly diverging band's nothing ([`EXPORTING_BAND`]); a band
/// with cells but no sea takes its nearest band's `e_b` and `s_b`, equatorward first, with its own
/// `C_b` ([`SEALESS_BANDS`]); and with no sea anywhere `e_b` and `s_b` are 0, a diverging band
/// rains nothing and a converging band its `C_b`. The land's source is max(0, `C_b`) throughout.
/// Each is the formula computed here, to the bit.
#[test]
fn the_sea_rains_its_evaporation_and_its_band_s_convergence_and_a_divergent_band_s_export_falls_on_its_sea()
 {
    let real = |band: usize| f64::from(u32::try_from(band).expect("a band index fits a u32"));
    let source_of = |band: usize| {
        SEALESS_BANDS
            .iter()
            .find(|&&(sealess, _)| sealess == band)
            .map_or(band, |&(_, source)| source)
    };
    let areas: Vec<f64> = (0..LATITUDE_BANDS)
        .map(|band| 1e-3 * (1.0 + real(band) / 72.0))
        .collect();
    let sea_areas: Vec<f64> = areas
        .iter()
        .enumerate()
        .map(|(band, &area)| {
            if source_of(band) == band {
                area * (0.3 + real(band) / 144.0)
            } else {
                0.0
            }
        })
        .collect();
    let evaporated: Vec<f64> = sea_areas
        .iter()
        .enumerate()
        .map(|(band, &sea)| sea * 3e-5 * (1.0 + real(band) / 144.0))
        .collect();
    // E − P, kg m⁻² s⁻¹: by hand where the test names a branch, else alternating.
    let budget: Vec<f64> = (0..LATITUDE_BANDS)
        .map(|band| match band {
            CONVERGING_BAND => -1.2e-5,
            DIVERGING_BAND => 6e-6,
            EXPORTING_BAND => 3e-5,
            10 => 4e-6,
            20 => 2e-6,
            21 => -3e-6,
            60 => -5e-6,
            _ if band % 3 == 0 => -8e-6,
            _ => 5e-6,
        })
        .collect();
    let sea = SeaBands::of(&evaporated, &areas, &sea_areas, &budget);
    let divergent_rate = |band: usize| {
        let source = source_of(band);
        evaporated[source] / sea_areas[source] - budget[band] / (sea_areas[source] / areas[source])
    };
    assert!(budget[CONVERGING_BAND] < 0.0);
    assert!(budget[DIVERGING_BAND] > 0.0 && divergent_rate(DIVERGING_BAND) > 0.0);
    assert!(budget[EXPORTING_BAND] > 0.0 && divergent_rate(EXPORTING_BAND) < 0.0);
    assert_same_bits(sea.rates_kg_m2_s[EXPORTING_BAND], 0.0);
    for (band, &emp) in budget.iter().enumerate() {
        let source = source_of(band);
        let evaporation = evaporated[source] / sea_areas[source];
        let share = sea_areas[source] / areas[source];
        let convergence = -emp;
        let rate = if convergence >= 0.0 {
            evaporation + convergence
        } else {
            evaporation + convergence / share
        };
        assert_same_bits(sea.evaporation_kg_m2_s[band], evaporation);
        assert_same_bits(sea.sea_shares[band], share);
        assert_same_bits(sea.rates_kg_m2_s[band], positive(rate));
        assert_same_bits(sea.convergence_kg_m2_s[band], positive(convergence));
    }
    let none = vec![0.0; LATITUDE_BANDS];
    let dry = SeaBands::of(&none, &areas, &none, &budget);
    for (band, &emp) in budget.iter().enumerate() {
        let convergence = -emp;
        assert_same_bits(dry.evaporation_kg_m2_s[band], 0.0);
        assert_same_bits(dry.sea_shares[band], 0.0);
        assert_same_bits(
            dry.rates_kg_m2_s[band],
            if convergence >= 0.0 { convergence } else { 0.0 },
        );
        assert_same_bits(dry.convergence_kg_m2_s[band], positive(convergence));
    }
}

/// The sea's rate on an aquaplanet (decision-r09-t13e-revision §4.1 and §6.3's unit test 8): on
/// the 60° aquaplanet of
/// [`on_a_60_degree_aquaplanet_the_centre_holds_at_the_cap_and_the_equatorial_wind_reverses_twice`],
/// every cell sea, each month's every band's `e_b` is its mean evaporation Ē over all its cells
/// bit for bit, its `s_b` is 1, and its sea's rate `P_sea` is max(0, Ē − (E − P)) bit for bit, the
/// as-built `P_ls`.
#[test]
fn on_a_60_degree_aquaplanet_the_sea_rains_its_band_s_mean_evaporation_less_its_e_minus_p() {
    let run = run_world(earth_at(60.0, 1.0), 6, Grid::Zonal, flood_all);
    let water = run.water();
    assert_eq!(water.months.len(), 12);
    assert!(run.working.cells.iter().all(seas::under_sea));
    for cycle in &water.months {
        for (band, (&mean, &emp)) in cycle
            .band_evaporation_kg_m2_s
            .iter()
            .zip(&cycle.budget.evaporation_less_precipitation_kg_m2_s)
            .enumerate()
        {
            assert_same_bits(cycle.sea.evaporation_kg_m2_s[band], mean);
            assert_same_bits(cycle.sea.sea_shares[band], 1.0);
            assert_same_bits(cycle.sea.rates_kg_m2_s[band], positive(mean - emp));
        }
    }
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
    let mut methane_cells = 0_u32;
    for (n, cell) in (0_u32..).zip(&working.cells) {
        let open = evaporation.rates(&pass, &working, &grid, coordinates, n, &mut rates);
        if cell.liquid_entry == Some(methane) {
            assert!(open && rates[0] > 0.0, "methane cell {n}: {}", rates[0]);
            methane_cells += 1;
        } else {
            assert!(!open, "cell {n}: {:?}", cell.liquid_entry);
            assert_same_bits(rates[0], 0.0);
        }
    }
    assert!(methane_cells > 0, "the Titan-like world has methane seas");
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

/// decision-r09-t13i §3.2's `Q_O`, "the point's over its sea share": the convergence of a point
/// with no sea is none, of a half-sea point twice its own, of an all-sea point itself, each to the
/// bit, month by month.
#[test]
fn the_ocean_s_heat_is_the_point_s_convergence_over_its_sea_share() {
    let convergence = vec![vec![2.0, 3.0, -4.0], vec![-1.0, 0.25, 7.0]];
    let per_sea = super::super::per_sea_area(&convergence, &[0.0, 0.5, 1.0]);
    for (month, expected) in per_sea.iter().zip([[0.0, 6.0, -4.0], [0.0, 0.5, 7.0]]) {
        for (&q, e) in month.iter().zip(expected) {
            assert_same_bits(q, e);
        }
    }
}

/// decision-r09-t13i §3.2: the sea's evaporation reads the ocean's heat, `Q_O`, in its energy. On
/// the reference Earth's synthetic sea at level 5 (the zonal model, whose kept run records its
/// ocean): with no record, and with a record of zeros, every field cell's rates are the same to the
/// bit; and with the record, each sea cell whose point the ocean warms in every month evaporates
/// at least as much in its year as without it, and each it cools in every month at most as much.
#[test]
fn the_sea_s_evaporation_reads_the_ocean_s_heat() {
    let inputs = reference::earth_like();
    let (field, climate_grid) = grids(5);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate_grid);
    let mut working = Working::initial(&pass);
    synthetic_earth(&pass, &mut working);
    let model = energy_balance(&pass, &working, ModelGrid::zonal(), Coordinates::Geographic);
    let own = model
        .water
        .clone()
        .expect("the Earth keeps its own records");
    let ocean = own
        .ocean_w_m2
        .clone()
        .expect("the synthetic sea carries the ocean's heat");
    let (grid, coordinates) = (model.grid.clone(), model.coordinates);
    let _ = finish(&pass, &mut working, model);
    let ebm = EnergyBalance::for_inputs(&inputs);
    let evaporation = |records: &hydrology::OwnRecords| {
        Evaporation::new(&pass, &working, &ebm, records).expect("the Earth evaporates")
    };
    let with = evaporation(&own);
    let without = evaporation(&hydrology::OwnRecords {
        ocean_w_m2: None,
        ..own.clone()
    });
    let zeroed = evaporation(&hydrology::OwnRecords {
        ocean_w_m2: Some(vec![vec![0.0; ocean[0].len()]; ocean.len()]),
        ..own.clone()
    });
    let shares = month_shares(&pass, 12);
    let year = |rates: &[f64]| -> f64 { rates.iter().zip(&shares).map(|(e, s)| s * e).sum() };
    let (mut warmed, mut cooled) = (0_u32, 0_u32);
    let (mut a, mut b, mut c) = ([0.0; 12], [0.0; 12], [0.0; 12]);
    for n in 0..u32::try_from(working.cells.len()).expect("the cells fit a u32") {
        let open = with.rates(&pass, &working, &grid, coordinates, n, &mut a);
        without.rates(&pass, &working, &grid, coordinates, n, &mut b);
        zeroed.rates(&pass, &working, &grid, coordinates, n, &mut c);
        for (&no, &zero) in b.iter().zip(&c) {
            assert_same_bits(no, zero);
        }
        if !open {
            continue;
        }
        let point = grid.point_of(coordinates.to_model(field.centre(n)));
        let heat: Vec<f64> = ocean.iter().map(|month| month[point]).collect();
        if heat.iter().all(|&q| q > 0.0) {
            assert!(
                year(&a) >= year(&b),
                "cell {n}: {} against {}",
                year(&a),
                year(&b)
            );
            warmed += 1;
        } else if heat.iter().all(|&q| q < 0.0) {
            assert!(
                year(&a) <= year(&b),
                "cell {n}: {} against {}",
                year(&a),
                year(&b)
            );
            cooled += 1;
        }
    }
    eprintln!("sea cells warmed by the ocean every month {warmed}, cooled every month {cooled}");
    assert!(
        warmed + cooled > 0,
        "no sea cell's point is warmed or cooled every month"
    );
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

/// The ruled runoff test (decision-r09-t13ge §2.13, e8; R09.T14's), as decision-r09-t13e-revision
/// §4.5 and §6.3 reword it: the annual runoff function is zero over liquid and on a dry world
/// ([`a_dry_world_under_the_energy_balance_model_has_no_precipitation_and_winds_that_are_not_calm`]),
/// and over a flat sea-level continent is, month by month, 1 − ε₀ of P, its climate cell's rate,
/// on ground at or above the condensable's freezing point, and all of P on frozen ground: the
/// unseasonal reference Earth's synthetic sea with its land at the sea level, level 6, one month.
/// The test reads each land cell's temperature at its surface as its climate cell's sea-level
/// temperature less Γ times its height above the sea level (one month, so no anomaly), against the
/// model's freezing point (`EnergyBalance::for_inputs`'s `Freezing::At`). Both kinds of ground
/// occur, each with rain somewhere.
#[test]
fn the_annual_runoff_is_none_over_liquid_the_unrecycled_share_on_thawed_ground_and_all_the_rain_on_frozen_ground()
 {
    let run = run_world(unseasonal_earth(), 6, Grid::Own, |pass, working| {
        synthetic_sea(pass, working, &SEA_LEVEL_LAND);
    });
    let pass = run.pass();
    let annual = run.annual_precipitation();
    let freezing = EnergyBalance::for_inputs(&run.inputs).freezing;
    let Freezing::At { point_k } = freezing else {
        panic!("water freezes on the reference Earth: {freezing:?}");
    };
    let (lapse, sea_level) = (
        run.working.lapse_rate_k_per_m,
        run.working.sea_level.value(),
    );
    let (mut thawed, mut frozen) = ([0_u32; 2], [0_u32; 2]);
    for (n, cell) in (0_u32..).zip(&run.working.cells) {
        let runoff = annual_runoff_kg_m2_s(&pass, &run.working, n);
        if seas::under_sea(cell) {
            assert_same_bits(runoff, 0.0);
            continue;
        }
        let parent = slot(parent_index(n));
        let p = annual[parent];
        let surface_k = run.working.climate[parent].sea_level_temperature.value()
            - lapse * (cell.water_surface.value() - sea_level);
        let counts = if surface_k >= point_k {
            assert_same_bits(runoff, (1.0 - lfpm::SEA_LEVEL_RECYCLED_SHARE) * p);
            &mut thawed
        } else {
            assert_same_bits(runoff, p);
            &mut frozen
        };
        counts[0] += 1;
        counts[1] += u32::from(p > 0.0);
    }
    eprintln!(
        "runoff: {} thawed land cells ({} with rain) and {} frozen ({} with rain)",
        thawed[0], thawed[1], frozen[0], frozen[1]
    );
    assert!(
        thawed[1] > 0 && frozen[1] > 0,
        "thawed {thawed:?}, frozen {frozen:?}"
    );
}

/// A locked world's circulation converges on its substellar point, uncapped (decision-r09-t13ge
/// §2.10, §2.12; Design note 8): the reference Earth locked 1:1 to its star on a circular orbit
/// with no obliquity (one month), all sea, at level 5. Its energy-flux equator is the tidally locked
/// pole, the substellar point, and its centre that equator; its Hadley share at every inner band
/// edge is the as-built one to the bit, no edge lying beyond its centre (decision-r09-t13i §3.5,
/// 1); every moving wind on the day side blows towards the substellar point; and the rain is
/// heaviest near it.
#[test]
fn a_locked_aquaplanet_s_winds_converge_on_its_substellar_point() {
    let run = run_world(locked_earth(0.0, 0.0), 5, Grid::Own, flood_all);
    let water = run.water();
    assert_eq!(water.circulation.cap_deg(), None);
    let cycle = &water.months[0];
    assert_same_bits(cycle.budget.centre_sine, cycle.budget.efe_sine);
    assert!(cycle.budget.efe_sine > 0.99, "{}", cycle.budget.efe_sine);
    let grid = ModelGrid::locked();
    for edge in 1..LATITUDE_BANDS {
        let x = grid.edge_sine(edge);
        assert_same_bits(
            water.circulation.share(x, cycle.budget.centre_sine),
            partition::as_built_share(x, cycle.budget.centre_sine, water.circulation.width()),
        );
    }
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

/// Each month's field-cell precipitation rates after the balance, kg m⁻² s⁻¹, solved again on
/// `run`'s finished state in its model's `coordinates` at Hergarten and Robl's set: the water
/// cycle's stages 5 and 6 as [`hydrology::run`] takes them, each month's bands and k its
/// [`MonthCycle`]'s. Each sea cell's rate is its band's `P_sea` read by the band stencil, and each
/// land cell's the orographic solve's, with its band's convergence, its month's ground
/// ([`hydrology::ground_of`]) and its climate cell's decoded wind along the model's bands; each
/// times the month's k. The water cycle keeps no field cell's rate; [`replication_error`] holds
/// these to its records.
fn field_rates(run: &Run, coordinates: Coordinates) -> Vec<Vec<f64>> {
    let pass = run.pass();
    let (inputs, field, working) = (pass.inputs(), pass.grid(), &run.working);
    let sea_level = working.sea_level.value();
    let heights_m: Vec<f64> = working
        .cells
        .iter()
        .map(|cell| cell.water_surface.value() - sea_level)
        .collect();
    let cover: Vec<Cover> = working
        .cells
        .iter()
        .map(|cell| {
            if seas::under_sea(cell) {
                Cover::Sea
            } else {
                Cover::Land
            }
        })
        .collect();
    let pole = coordinates.to_body([0.0, 0.0, 1.0]);
    let stencils: Vec<BandStencil> = field
        .centres()
        .iter()
        .map(|&centre| BandStencil::at(coordinates.to_model(centre)[2]))
        .collect();
    let freezing = EnergyBalance::for_inputs(inputs).freezing;
    let mesh = Mesh::of_graph(field, inputs.radius());
    let mut months = Vec::with_capacity(run.water().months.len());
    for (m, cycle) in run.water().months.iter().enumerate() {
        let mut winds = Vec::with_capacity(cover.len());
        let mut sea_rates = vec![0.0; cover.len()];
        let mut convergence = vec![0.0; cover.len()];
        let mut ground = vec![GroundState::Thawed; cover.len()];
        for (n, &up) in (0_u32..).zip(field.centres()) {
            let (i, sample) = (
                slot(n),
                &working.climate[slot(parent_index(n))].month_wind[m],
            );
            winds.push(wind::along_bands(
                wind::cell_wind(sample, up),
                wind::model_east(pole, up),
            ));
            match cover[i] {
                Cover::Sea => sea_rates[i] = stencils[i].clamped(&cycle.sea.rates_kg_m2_s),
                Cover::Land => {
                    convergence[i] = stencils[i].clamped(&cycle.sea.convergence_kg_m2_s);
                    ground[i] = hydrology::ground_of(working, n, m, freezing);
                }
            }
        }
        let land = if cycle.orographic.is_some() {
            let month = Month {
                heights_m: &heights_m,
                cover: &cover,
                ground: &ground,
                winds: &winds,
                sea_rates_kg_m2_s: &sea_rates,
                convergence_kg_m2_s: &convergence,
            };
            lfpm::solve(&mesh, &month, &Parameters::HERGARTEN_ROBL).precipitation_kg_m2_s
        } else {
            vec![0.0; cover.len()]
        };
        months.push(
            cover
                .iter()
                .zip(land.iter().zip(&sea_rates))
                .map(|(&c, (&on_land, &on_sea))| {
                    cycle.scale
                        * match c {
                            Cover::Sea => on_sea,
                            Cover::Land => on_land,
                        }
                })
                .collect(),
        );
    }
    months
}

/// The largest relative difference, over every month and climate cell of `run`, of the
/// solid-angle mean of its four field cells' `rates` ([`field_rates`]) from its record: zero where
/// the replication is the water cycle's, the same sums in the same order.
fn replication_error(run: &Run, rates: &[Vec<f64>]) -> f64 {
    let omegas = run.field.solid_angles_sr().as_chunks::<4>().0;
    let mut worst = 0.0_f64;
    for (m, month) in rates.iter().enumerate() {
        for ((state, omega), rate) in run
            .working
            .climate
            .iter()
            .zip(omegas)
            .zip(month.as_chunks::<4>().0)
        {
            let (sum, area) = omega
                .iter()
                .zip(rate)
                .fold((0.0, 0.0), |(sum, area), (&o, &p)| (sum + o * p, area + o));
            let record = state.month_precipitation_kg_m2_s[m];
            let difference = (sum / area - record).abs() / record.max(f64::MIN_POSITIVE);
            worst = worst.max(difference);
        }
    }
    worst
}

/// Prints the E-L7 patch (decision-r09-t13e-revision §5.1; decision-r09-t13e-lfpm-convergence §5):
/// the field cell nearest [`E_L7_PATCH_DEG`], its cover, its annual rate from `rates`
/// ([`field_rates`]) and its climate cell's record's, mm/day.
fn report_patch(run: &Run, rates: &[Vec<f64>]) {
    let shares = month_shares(&run.pass(), rates.len());
    let target = direction(E_L7_PATCH_DEG.0, E_L7_PATCH_DEG.1);
    let nearest = run
        .field
        .centres()
        .iter()
        .enumerate()
        .fold((0, f64::NEG_INFINITY), |(best, most), (n, &centre)| {
            let along = dot(centre, target);
            if along > most {
                (n, along)
            } else {
                (best, most)
            }
        })
        .0;
    let annual = rates
        .iter()
        .zip(&shares)
        .fold(0.0, |sum, (month, &s)| sum + s * month[nearest]);
    let parent = slot(parent_index(
        u32::try_from(nearest).expect("a cell index fits a u32"),
    ));
    let (latitude, longitude) = latitude_longitude_deg(run.field.centres()[nearest]);
    eprintln!(
        "  the E-L7 patch: the field cell {nearest} nearest {:.2}° N, {:.2}° E, at {latitude:.3}°, \
         {longitude:.3}° ({}), annual {:.3} mm/day; its climate cell {:.3}",
        E_L7_PATCH_DEG.0,
        E_L7_PATCH_DEG.1,
        if seas::under_sea(&run.working.cells[nearest]) {
            "sea"
        } else {
            "land"
        },
        annual * DAY_S,
        run.annual_precipitation()[parent] * DAY_S
    );
}

/// The coarse Earth's d with G1 converged at the climate's stop, K: −0.335, against which
/// decision-r09-t13i's §2.4 follow-up 8 §1 holds its probe's d. Re-measured at version 23's air
/// (P14.T54.a, the second to land re-measuring by decision-r09-t13i's row 8: T49.b's `c_p`(T),
/// the greenhouse's batch arm and `reference::earth_air`'s water at 0.77 of `p_sat` move the
/// climate) by the follow-up 7 build's procedure, G1 under the climate's stop alone and d from its
/// mean alone: G1 287.665 K in 8 orbits against Z₃'s 287.999 K. At 22's air it was −0.438 (plan
/// 09's R09.T13.i as-built record: G1 287.561 K in 7 orbits against Z₃'s 287.998 K).
const CONVERGED_G1_DEFECT_K: f64 = -0.335;

/// The coarse Earth's kept G2 from a converged G1, K: 287.976, against which follow-up 8 §1 holds
/// G2 from the probe. Re-measured at 23's air with [`CONVERGED_G1_DEFECT_K`] (the same run: G2
/// predicted, from 287.599 K in 3 orbits, 0.024 K from plan 14's mean); 287.969 at 22's air.
const CONVERGED_G1_G2_MEAN_K: f64 = 287.976;

/// How near G1's probe stop must leave d to the converged G1's, K: 0.05, the probe's own precision
/// (decision-r09-t13i's §2.4 follow-up 8 §1 tests, kept by its follow-up 9 §2).
const PROBE_DEFECT_AGREEMENT_K: f64 = 0.05;

/// How near the kept G2 from G1's probe must lie to the one from the converged G1, K: 0.1, the
/// climate stop's (decision-r09-t13i's §2.4 follow-up 9 §2, corrected from follow-up 8's 0.05 K:
/// d's error, moving G2 by at most about 0.035 K, and the two G2 runs' own stops together).
const PROBE_G2_AGREEMENT_K: f64 = 0.1;

/// Asserts G1's probe stop on the coarse Earth (decision-r09-t13i's §2.4 follow-up 8 §1 tests, as
/// its follow-up 9 §2 corrects them): its d within [`PROBE_DEFECT_AGREEMENT_K`] of the converged
/// G1's [`CONVERGED_G1_DEFECT_K`], and G2, kept, within [`PROBE_G2_AGREEMENT_K`] of
/// [`CONVERGED_G1_G2_MEAN_K`].
fn assert_the_probe_measures_g1(run: &Run) {
    let means = run
        .diagnostics
        .surface_means
        .as_ref()
        .expect("the energy-balance model's surface means");
    let super::super::ebm::MeanCorrection::Searched {
        defect, g2, kept, ..
    } = &means.correction
    else {
        panic!("the geographic model searches: {:?}", means.correction);
    };
    let (d, _) = defect.expect("G1's probe measured d");
    eprintln!(
        "  the probe's d {d:+.4} K against the converged G1's {CONVERGED_G1_DEFECT_K:+.3}; G2 {:.4} \
         K against {CONVERGED_G1_G2_MEAN_K:.3}",
        g2.mean_k
    );
    assert!(
        (d - CONVERGED_G1_DEFECT_K).abs() <= PROBE_DEFECT_AGREEMENT_K,
        "d {d} K"
    );
    assert_eq!(*kept, super::super::ebm::KeptRun::G2);
    assert!(
        (g2.mean_k - CONVERGED_G1_G2_MEAN_K).abs() <= PROBE_G2_AGREEMENT_K,
        "G2 {} K",
        g2.mean_k
    );
}

/// The coarse pass's reference Earth (its generated continents and R09.T12.e's sea, level 8)
/// through the climate step, printed beside GPCP and NCEP/NCAR R1 as the synthetic Earth is, its
/// water cycle run again on the finished state to the same diagnostics. It reads no clock: its
/// cost is the test runner's own timing, recorded in plan 09's Risks. A recorded measurement of the
/// shipped generator's own Earth (decision-r09-t13e-asbuilt §1.7): it asserts the water balance in
/// every month ([`assert_the_water_balances`]) and the zonal budget's E − P pattern in each
/// hemisphere ([`assert_the_budget_s_pattern`]), but never its precipitation windows, since its
/// generated midlatitudes are mostly sea and its southern margin a few per cent. It is also the
/// ruled slow test of the orographic solve (decision-r09-t13e-lfpm-convergence §3.5): the water
/// cycle run again on a clone of the finished state at the deep tolerance its E-L8 §5 rules,
/// [`EARTH_DEEP_TOLERANCE`], every month converges before the cap and closes its budget, and every
/// land climate cell's monthly and annual rate lies within 1% of the deep solve's
/// ([`assert_the_solve_converges`]).
///
/// It prints, never asserts, the windows' land and sea parts, Eurasia's interior ratio and the
/// other regions of the ruled Earth's ([`report_regions`], NaN where the generated Earth has no
/// all-land climate cell in a box), k by month, and the E-L7 patch (decision-r09-t13e-revision
/// §5.1): the field cell nearest 1.11° N, 54.26° E, its annual rate and its climate cell's, the
/// field cells' rates solved again from the finished state ([`field_rates`], held to the records
/// to the bit by [`replication_error`]). As built under decision-r09-t13e-lfpm-convergence it
/// read 17.06 mm/day and its climate cell 12.24, a land convergence line under the trades
/// (plan 09's E-L7).
#[test]
#[ignore = "slow: the reference Earth's coarse pass to the climate at level 8, its water cycle \
            again at the deep tolerance, and its field cells' rates solved again, about 50 s"]
fn the_coarse_pass_s_reference_earth_s_water_cycle_against_gpcp() {
    let inputs = reference::earth_like();
    let level = hyperion_surface::field::coarse_level(inputs.radius());
    let (field, climate) = (CellGraph::for_field(level), CellGraph::for_climate(level));
    let (working, diagnostics, deep, coordinates) = {
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
        let mut working = Working::initial(&pass);
        steps::plates::run(&pass, &mut working);
        steps::relief::run(&pass, &mut working);
        steps::craters::run(&pass, &mut working);
        steps::relief::run_after_craters(&pass, &mut working);
        let model = normalised(&pass, &working);
        let kept = model.clone();
        let diagnostics = finish(&pass, &mut working, model);
        let again = hydrology::run(&pass, &mut working, &kept);
        assert_eq!(again.as_ref(), diagnostics.water.as_ref());
        let mut deep_working = working.clone();
        let deep = hydrology::run_with(
            &pass,
            &mut deep_working,
            &kept,
            &Parameters::deep().with_tolerance(EARTH_DEEP_TOLERANCE),
        )
        .expect("the Earth's model keeps its own records");
        let land = working.cells.iter().filter(|c| !seas::under_sea(c)).count();
        eprintln!(
            "level {}: {land} of {} cells land; τ {TOLERANCE:e}; the deep water cycle at τ \
             {EARTH_DEEP_TOLERANCE:e}",
            level.get(),
            working.cells.len()
        );
        (
            working,
            diagnostics,
            (deep, deep_working.climate),
            kept.coordinates,
        )
    };
    let run = Run {
        field,
        climate,
        inputs,
        working,
        diagnostics,
    };
    report("The reference Earth (the coarse pass)", &run);
    assert_the_probe_measures_g1(&run);
    report_regions(&run);
    report_balance(&run);
    let rates = field_rates(&run, coordinates);
    let replication = replication_error(&run, &rates);
    report_patch(&run, &rates);
    eprintln!("  the field cells' rates solved again: the records within {replication:.1e}");
    assert_same_bits(replication, 0.0);
    assert_the_water_balances(&run);
    assert_the_budget_s_pattern(&run);
    assert_the_solve_converges(&run, &deep.0, &deep.1);
}
