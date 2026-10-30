//! The geographic model against observed monthly climatologies (R09.T13.l, which replaces R09.T13's
//! offline `ExoPlaSim` check): the relief Earth's monthly fields over Earth's land boxes against GPCP
//! v2.3's precipitation ([`gpcp_boxes`](super::gpcp_boxes)) and the NCEP/NCAR Reanalysis 1's
//! near-surface air temperature at the sea level ([`ncep_air_boxes`](super::ncep_air_boxes)), both
//! 1991–2020 on the climate model's 2.5° × 5° boxes. The zonal fields are held to observations by
//! R09.T13.a's and R09.T13.i's tests; this checks the two-dimensional, monthly structure: the
//! pattern of each month over the land, the rain shadows, the west coasts' deserts and the Indian
//! monsoon.

use super::super::grid::{GEOGRAPHIC_COLUMNS, LATITUDE_BANDS, ModelGrid, index};
use super::gpcp_boxes::GPCP_LAND_BOXES_HUNDREDTH_MM_PER_DAY;
use super::ncep_air::MONTH_DAYS;
use super::ncep_air_boxes::NCEP_LAND_BOXES_HUNDREDTH_K;
use super::{EARTH_HYPSOMETRY_M, EARTH_LAND_BOXES, grids, synthetic_sea};
use crate::math;
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::{Pass, Working};

/// Seconds in a day, for rates in mm of water a day (1 kg m⁻² of water is 1 mm).
const DAY_S: f64 = 86_400.0;

/// The least Natural Earth land share of a land box, thousandths of its solid angle: half
/// ([`EARTH_LAND_BOXES`]).
const LAND_BOX_SHARE: u16 = 500;

/// The calendar's winter months in the north, December, January and February (indices from
/// January). The reference Earth's months are equal spans of eccentric anomaly from periapsis, about
/// 3 January, so that its month m lies within about two days of calendar month m (`hydrology`'s
/// `DJF_MONTHS`).
const DJF: [usize; 3] = [11, 0, 1];

/// The calendar's summer months in the north, June, July and August.
const JJA: [usize; 3] = [5, 6, 7];

/// One land box's monthly fields, the model's and the observed, January first.
#[derive(Debug, Clone, PartialEq)]
struct LandBox {
    /// The box's band, from the south pole.
    band: usize,
    /// The box's column, of 5° from −180° of longitude.
    column: usize,
    /// The model's sea-level air temperature, K: the solid-angle mean over the climate cells whose
    /// centres lie in the box of each one's annual sea-level temperature plus its month's anomaly.
    model_k: [f64; 12],
    /// NCEP R1's σ = 0.995 air temperature reduced to the sea level over land, K
    /// ([`ncep_air_boxes`](super::ncep_air_boxes)).
    observed_k: [f64; 12],
    /// The model's precipitation, mm/day: the solid-angle mean over the same climate cells.
    model_mm_per_day: [f64; 12],
    /// GPCP v2.3's precipitation, mm/day ([`gpcp_boxes`](super::gpcp_boxes)).
    observed_mm_per_day: [f64; 12],
}

/// The latitude and longitude of the centre of the box of band `band` and column `column`, degrees.
fn centre_deg(band: usize, column: usize) -> (f64, f64) {
    (-88.75 + 2.5 * index(band), -177.5 + 5.0 * index(column))
}

/// The months' days' share of the year, as the observed tables' annual means weigh them
/// ([`MONTH_DAYS`]); the model's month m is weighed as calendar month m.
fn annual(months: &[f64; 12]) -> f64 {
    let year: f64 = MONTH_DAYS.iter().sum();
    months
        .iter()
        .zip(&MONTH_DAYS)
        .fold(0.0, |sum, (&x, &days)| sum + x * days / year)
}

/// The mean of `months` over the season `season`, its months weighted equally.
fn seasonal(months: &[f64; 12], season: [usize; 3]) -> f64 {
    season.iter().fold(0.0, |sum, &m| sum + months[m]) / 3.0
}

/// The first harmonic of a year's twelve monthly means `months` (January first), each month's at
/// its midpoint, k + ½ twelfths of the year: its amplitude, in the values' unit, and the phase of
/// its maximum, days from 1 January of a 365.25-day year.
fn first_harmonic(months: &[f64]) -> (f64, f64) {
    let (mut cosine, mut sine) = (0.0, 0.0);
    for (k, &x) in months.iter().enumerate() {
        let (s, c) = math::sin_cos(core::f64::consts::TAU * (index(k) + 0.5) / 12.0);
        cosine += x * c / 6.0;
        sine += x * s / 6.0;
    }
    let phase = math::atan2(sine, cosine).rem_euclid(core::f64::consts::TAU);
    (
        (cosine * cosine + sine * sine).sqrt(),
        phase / core::f64::consts::TAU * 365.25,
    )
}

/// The land boxes, each with the model's fields from `working` and the observed ones, and the count
/// of land boxes that hold no climate cell's centre, which are left out (all of them near the
/// poles, where a box is smaller than a climate cell at level 7).
///
/// # Panics
///
/// If the observed tables do not hold Earth's land boxes, in the same order.
fn land_boxes(pass: &Pass<'_>, working: &Working) -> (Vec<LandBox>, usize) {
    let points = ModelGrid::geographic();
    let climate = pass.climate_grid();
    let (mut area, mut temperature, mut precipitation) = (
        vec![0.0; points.points()],
        vec![[0.0; 12]; points.points()],
        vec![[0.0; 12]; points.points()],
    );
    for ((centre, &omega), state) in climate
        .centres()
        .iter()
        .zip(climate.solid_angles_sr())
        .zip(&working.climate)
    {
        let point = points.point_of(*centre);
        area[point] += omega;
        for m in 0..12 {
            temperature[point][m] +=
                omega * (state.sea_level_temperature.value() + state.month_anomaly_k[m]);
            precipitation[point][m] += omega * state.month_precipitation_kg_m2_s[m] * DAY_S;
        }
    }
    let mut boxes = Vec::with_capacity(GPCP_LAND_BOXES_HUNDREDTH_MM_PER_DAY.len());
    let mut empty = 0;
    for (&(band, column, rain), &(t_band, t_column, air)) in GPCP_LAND_BOXES_HUNDREDTH_MM_PER_DAY
        .iter()
        .zip(NCEP_LAND_BOXES_HUNDREDTH_K)
    {
        assert_eq!(
            (band, column),
            (t_band, t_column),
            "the tables' boxes differ"
        );
        let (band, column) = (usize::from(band), usize::from(column));
        let point = points.point(band, column);
        if area[point] <= 0.0 {
            empty += 1;
            continue;
        }
        boxes.push(LandBox {
            band,
            column,
            model_k: temperature[point].map(|t| t / area[point]),
            observed_k: air.map(|t| f64::from(t) / 100.0),
            model_mm_per_day: precipitation[point].map(|p| p / area[point]),
            observed_mm_per_day: rain.map(|p| f64::from(p) / 100.0),
        });
    }
    (boxes, empty)
}

/// How a model field compares with an observed one over a set of boxes, each weighted by its solid
/// angle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Skill {
    /// The centred pattern correlation (Taylor 2001, J. Geophys. Res. 106, 7183, eq. 1): the
    /// weighted covariance of the two fields over the product of their weighted standard deviations.
    correlation: f64,
    /// The centred pattern correlation of each field's departures from its own mean over the set's
    /// boxes in each band: the pattern in longitude, which the zonal fields do not hold.
    departure_correlation: f64,
    /// The weighted RMS of the model less the observed, the bias included.
    rms: f64,
    /// The weighted mean of the model less the observed.
    bias: f64,
}

/// The weighted centred correlation of `x` and `y` with weights `w`.
fn correlation(w: &[f64], x: &[f64], y: &[f64]) -> f64 {
    let total: f64 = w.iter().sum();
    let mean = |v: &[f64]| w.iter().zip(v).fold(0.0, |s, (&w, &v)| s + w * v) / total;
    let (mx, my) = (mean(x), mean(y));
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for ((&w, &x), &y) in w.iter().zip(x).zip(y) {
        sxy += w * (x - mx) * (y - my);
        sxx += w * (x - mx) * (x - mx);
        syy += w * (y - my) * (y - my);
    }
    sxy / (sxx * syy).sqrt()
}

/// Each value of `values` (one a box of `boxes`) less the mean over the boxes of its band; the boxes
/// of a band share one solid angle, so the mean is plain.
fn departures(boxes: &[LandBox], values: &[f64]) -> Vec<f64> {
    let (mut sums, mut counts) = ([0.0; LATITUDE_BANDS], [0.0; LATITUDE_BANDS]);
    for (b, &v) in boxes.iter().zip(values) {
        sums[b.band] += v;
        counts[b.band] += 1.0;
    }
    boxes
        .iter()
        .zip(values)
        .map(|(b, &v)| v - sums[b.band] / counts[b.band])
        .collect()
}

/// The skill over `boxes` of the model's values `model` against the observed `observed`, one a
/// box.
fn skill(boxes: &[LandBox], model: &[f64], observed: &[f64]) -> Skill {
    let grid = ModelGrid::geographic();
    let w: Vec<f64> = boxes.iter().map(|b| grid.weight(b.band)).collect();
    let total: f64 = w.iter().sum();
    let (squares, bias) = w
        .iter()
        .zip(model.iter().zip(observed))
        .fold((0.0, 0.0), |(sq, bias), (&w, (&m, &o))| {
            (sq + w * (m - o) * (m - o), bias + w * (m - o))
        });
    Skill {
        correlation: correlation(&w, model, observed),
        departure_correlation: correlation(
            &w,
            &departures(boxes, model),
            &departures(boxes, observed),
        ),
        rms: (squares / total).sqrt(),
        bias: bias / total,
    }
}

/// A region of the land boxes: those whose centres lie within latitudes `south` to `north` and
/// longitudes `west` to `east`, degrees, the bounds included.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Region {
    /// Its name, as printed.
    name: &'static str,
    /// The southern bound of its boxes' centres, degrees of latitude.
    south: f64,
    /// The northern bound, degrees of latitude.
    north: f64,
    /// The western bound, degrees of longitude.
    west: f64,
    /// The eastern bound, degrees of longitude.
    east: f64,
}

impl Region {
    /// Whether it holds land box `b`'s centre.
    fn holds(&self, b: &LandBox) -> bool {
        let (latitude, longitude) = centre_deg(b.band, b.column);
        (self.south..=self.north).contains(&latitude)
            && (self.west..=self.east).contains(&longitude)
    }

    /// The solid-angle means over its land boxes of `of`'s model and observed values, and their
    /// count.
    ///
    /// # Panics
    ///
    /// If it holds no land box.
    fn means(&self, boxes: &[LandBox], of: impl Fn(&LandBox) -> (f64, f64)) -> (f64, f64, usize) {
        let grid = ModelGrid::geographic();
        let (mut model, mut observed, mut area, mut count) = (0.0, 0.0, 0.0, 0);
        for b in boxes.iter().filter(|b| self.holds(b)) {
            let (m, o) = of(b);
            let w = grid.weight(b.band);
            model += w * m;
            observed += w * o;
            area += w;
            count += 1;
        }
        assert!(count > 0, "{} holds no land box", self.name);
        (model / area, observed / area, count)
    }
}

/// A rain shadow or a west coast's desert: the annual precipitation of `dry`, the leeward side or
/// the west coast, over that of `wet`, the windward side or the east coast at the same latitudes.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Contrast {
    /// The side observed to be dry.
    dry: Region,
    /// The side observed to be wet.
    wet: Region,
}

/// The model's and the observed dry-over-wet ratios of the annual precipitation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ratio {
    /// The model's ratio.
    model: f64,
    /// The observed ratio.
    observed: f64,
}

impl Contrast {
    /// Prints both sides' annual means and the ratios, and returns the ratios.
    fn report(&self, boxes: &[LandBox]) -> Ratio {
        let rate = |b: &LandBox| (annual(&b.model_mm_per_day), annual(&b.observed_mm_per_day));
        let (dry_model, dry_observed, dry_boxes) = self.dry.means(boxes, rate);
        let (wet_model, wet_observed, wet_boxes) = self.wet.means(boxes, rate);
        let ratio = Ratio {
            model: dry_model / wet_model,
            observed: dry_observed / wet_observed,
        };
        eprintln!(
            "  {} ({dry_boxes} boxes) over {} ({wet_boxes}): the model {dry_model:.2} over \
             {wet_model:.2} mm/day, {:.2}; GPCP {dry_observed:.2} over {wet_observed:.2}, {:.2}",
            self.dry.name, self.wet.name, ratio.model, ratio.observed
        );
        ratio
    }
}

/// The rain shadows of the Andes, the Sierra Nevada and the Himalaya, each its leeward side over its
/// windward side: the southern Andes under the westerlies (37.5–50° S, the 5° column west of
/// 70° W over the one east of it), the Sierra Nevada (35–42.5° N, west of 120° W over east of it)
/// and the Himalaya under the monsoon (the Tibetan Plateau, 30–37.5° N, over the Ganges's plain,
/// 22.5–27.5° N, both 75–90° E). The bounds are those of the boxes' centres.
const RAIN_SHADOWS: [Contrast; 3] = [
    Contrast {
        dry: Region {
            name: "the Andes' lee, Patagonia (37.5–50° S, 70–65° W)",
            south: -50.0,
            north: -37.5,
            west: -70.0,
            east: -65.0,
        },
        wet: Region {
            name: "their windward side, Chile (75–70° W)",
            south: -50.0,
            north: -37.5,
            west: -75.0,
            east: -70.0,
        },
    },
    Contrast {
        dry: Region {
            name: "the Sierra Nevada's lee, the Great Basin (35–42.5° N, 120–115° W)",
            south: 35.0,
            north: 42.5,
            west: -120.0,
            east: -115.0,
        },
        wet: Region {
            name: "its windward side, California (125–120° W)",
            south: 35.0,
            north: 42.5,
            west: -125.0,
            east: -120.0,
        },
    },
    Contrast {
        dry: Region {
            name: "the Himalaya's lee, the Tibetan Plateau (30–37.5° N, 75–90° E)",
            south: 30.0,
            north: 37.5,
            west: 75.0,
            east: 90.0,
        },
        wet: Region {
            name: "its windward side, the Ganges's plain (22.5–27.5° N)",
            south: 22.5,
            north: 27.5,
            west: 75.0,
            east: 90.0,
        },
    },
];

/// The west coasts' deserts, each a continent's west coast over its east coast at the same
/// latitudes: the Atacama (15–30° S, 75–65° W, over Brazil's coast, 50–40° W), the Namib (17.5–30°
/// S, 10–20° E, over Mozambique's coast, 30–40° E), the Sonoran and Baja California (22.5–32.5° N,
/// 117.5–107.5° W, over the Gulf of Mexico's coast, 100–80° W) and Western Australia (20–32.5° S,
/// 112.5–122.5° E, over the east coast, 145–155° E). The bounds are those of the boxes' centres.
const WEST_COASTS: [Contrast; 4] = [
    Contrast {
        dry: Region {
            name: "the Atacama (15–30° S, 75–65° W)",
            south: -30.0,
            north: -15.0,
            west: -75.0,
            east: -65.0,
        },
        wet: Region {
            name: "Brazil's coast (50–40° W)",
            south: -30.0,
            north: -15.0,
            west: -50.0,
            east: -40.0,
        },
    },
    Contrast {
        dry: Region {
            name: "the Namib (17.5–30° S, 10–20° E)",
            south: -30.0,
            north: -17.5,
            west: 10.0,
            east: 20.0,
        },
        wet: Region {
            name: "Mozambique's coast (30–40° E)",
            south: -30.0,
            north: -17.5,
            west: 30.0,
            east: 40.0,
        },
    },
    Contrast {
        dry: Region {
            name: "the Sonoran and Baja California (22.5–32.5° N, 117.5–107.5° W)",
            south: 22.5,
            north: 32.5,
            west: -117.5,
            east: -107.5,
        },
        wet: Region {
            name: "the Gulf of Mexico's coast (100–80° W)",
            south: 22.5,
            north: 32.5,
            west: -100.0,
            east: -80.0,
        },
    },
    Contrast {
        dry: Region {
            name: "Western Australia (20–32.5° S, 112.5–122.5° E)",
            south: -32.5,
            north: -20.0,
            west: 112.5,
            east: 122.5,
        },
        wet: Region {
            name: "Australia's east coast (145–155° E)",
            south: -32.5,
            north: -20.0,
            west: 145.0,
            east: 155.0,
        },
    },
];

/// India, whose monsoon's JJA-against-DJF shift is compared: 10–25° N, 70–87.5° E, the bounds those
/// of the boxes' centres.
const INDIA: Region = Region {
    name: "India (10–25° N, 70–87.5° E)",
    south: 10.0,
    north: 25.0,
    west: 70.0,
    east: 87.5,
};

/// The latitudes within which NCEP R1's σ = 0.995 air temperature stands for the 2 m one to about
/// 0.3 K in the zonal means (its band means against [`ncep_air`](super::ncep_air)'s, 0.34 K RMS from
/// 58.75° S to 58.75° N): the temperature's skill is also printed over the land boxes in them.
const TEMPERATE_DEG: f64 = 60.0;

/// The northern extratropics' land, whose seasonal cycle of temperature is printed: 30–70° N, the
/// bounds those of the boxes' centres.
const NORTHERN_LAND: Region = Region {
    name: "the northern land (30–70° N)",
    south: 30.0,
    north: 70.0,
    west: -180.0,
    east: 180.0,
};

/// The least pattern correlation of the sea-level air temperature over the land boxes asserted in
/// each month: 0.75, below the first run's least, 0.824 in April (R09.T13.l's record).
const LEAST_TEMPERATURE_CORRELATION: f64 = 0.75;

/// The least pattern correlation of the precipitation over the land boxes asserted in each month:
/// 0.25, below the first run's least, 0.356 in June (R09.T13.l's record). Its departures from the
/// bands' means are held positive, the first run's least 0.238 in May.
const LEAST_PRECIPITATION_CORRELATION: f64 = 0.25;

/// Prints each month's and the annual skill of `model` against `observed` (each of a box's months)
/// over `boxes`, under the heading `name`, and returns the monthly skills.
fn report_skill(
    name: &str,
    unit: &str,
    boxes: &[LandBox],
    model: impl Fn(&LandBox) -> [f64; 12],
    observed: impl Fn(&LandBox) -> [f64; 12],
) -> Vec<Skill> {
    eprintln!(
        "  {name} over {} land boxes: pattern correlation, its departures from the bands' means, \
         RMS and bias ({unit})",
        boxes.len()
    );
    let model: Vec<[f64; 12]> = boxes.iter().map(model).collect();
    let observed: Vec<[f64; 12]> = boxes.iter().map(observed).collect();
    let mut skills = Vec::with_capacity(12);
    for m in 0..12 {
        let s = skill(
            boxes,
            &model.iter().map(|v| v[m]).collect::<Vec<_>>(),
            &observed.iter().map(|v| v[m]).collect::<Vec<_>>(),
        );
        eprintln!(
            "    month {:2}: r {:.3}, departures r {:+.3}, RMS {:.2}, bias {:+.2}",
            m + 1,
            s.correlation,
            s.departure_correlation,
            s.rms,
            s.bias
        );
        skills.push(s);
    }
    let s = skill(
        boxes,
        &model.iter().map(annual).collect::<Vec<_>>(),
        &observed.iter().map(annual).collect::<Vec<_>>(),
    );
    eprintln!(
        "    annual  : r {:.3}, departures r {:+.3}, RMS {:.2}, bias {:+.2}",
        s.correlation, s.departure_correlation, s.rms, s.bias
    );
    skills
}

/// Prints [`NORTHERN_LAND`]'s mean sea-level air temperature each month, the model's and NCEP's,
/// with their seasonal ranges and their first harmonics' amplitudes and phases ([`first_harmonic`]).
fn report_cycle(boxes: &[LandBox]) {
    let cycle: Vec<(f64, f64)> = (0..12)
        .map(|m| {
            let (model, observed, _) =
                NORTHERN_LAND.means(boxes, |b| (b.model_k[m], b.observed_k[m]));
            (model, observed)
        })
        .collect();
    let printed: Vec<String> = cycle
        .iter()
        .map(|(model, observed)| format!("{model:.1}/{observed:.1}"))
        .collect();
    eprintln!(
        "  {}'s mean sea-level air temperature by month, the model's over NCEP's, K: {}",
        NORTHERN_LAND.name,
        printed.join(" ")
    );
    let (model_cycle, observed_cycle): (Vec<f64>, Vec<f64>) = cycle.into_iter().unzip();
    let range = |v: &[f64]| {
        let (low, high) = v
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| {
                (l.min(x), h.max(x))
            });
        high - low
    };
    let (model_amplitude, model_phase) = first_harmonic(&model_cycle);
    let (observed_amplitude, observed_phase) = first_harmonic(&observed_cycle);
    eprintln!(
        "    its seasonal range: the model's {:.1} K, NCEP's {:.1} K; its first harmonic's \
         amplitude {model_amplitude:.2} K against {observed_amplitude:.2} K, its maximum on day \
         {model_phase:.1} against {observed_phase:.1}, {:+.1} days",
        range(&model_cycle),
        range(&observed_cycle),
        model_phase - observed_phase
    );
}

/// Prints the Himalaya's rain shadow ([`RAIN_SHADOWS`]'s third) in JJA, the monsoon's season.
fn report_himalaya_summer(boxes: &[LandBox]) {
    let himalaya = RAIN_SHADOWS[2];
    let summer = |b: &LandBox| {
        (
            seasonal(&b.model_mm_per_day, JJA),
            seasonal(&b.observed_mm_per_day, JJA),
        )
    };
    let (lee_model, lee_observed, _) = himalaya.dry.means(boxes, summer);
    let (windward_model, windward_observed, _) = himalaya.wet.means(boxes, summer);
    eprintln!(
        "    the Himalaya in JJA: the model {lee_model:.2} over {windward_model:.2} mm/day, {:.2}; \
         GPCP {lee_observed:.2} over {windward_observed:.2}, {:.2}",
        lee_model / windward_model,
        lee_observed / windward_observed
    );
}

/// Prints [`INDIA`]'s JJA and DJF precipitation, the model's and GPCP's, and the wettest month of
/// each, and returns the model's JJA and DJF means, mm/day.
fn report_monsoon(boxes: &[LandBox]) -> (f64, f64) {
    let season = |months: [usize; 3]| {
        move |b: &LandBox| {
            (
                seasonal(&b.model_mm_per_day, months),
                seasonal(&b.observed_mm_per_day, months),
            )
        }
    };
    let (jja_model, jja_observed, india_boxes) = INDIA.means(boxes, season(JJA));
    let (djf_model, djf_observed, _) = INDIA.means(boxes, season(DJF));
    eprintln!(
        "  the Indian monsoon, {} ({india_boxes} boxes): the model's JJA {jja_model:.2} against DJF \
         {djf_model:.2} mm/day, {:.2} times; GPCP's {jja_observed:.2} against {djf_observed:.2}, \
         {:.2} times",
        INDIA.name,
        jja_model / djf_model,
        jja_observed / djf_observed
    );
    let wettest = |of: fn(&LandBox) -> [f64; 12]| {
        let mut sums = [0.0; 12];
        for b in boxes.iter().filter(|b| INDIA.holds(b)) {
            for (s, v) in sums.iter_mut().zip(of(b)) {
                *s += v;
            }
        }
        (0..12).fold(0, |best, m| if sums[m] > sums[best] { m } else { best }) + 1
    };
    eprintln!(
        "    its wettest month: the model's {}, GPCP's {}",
        wettest(|b| b.model_mm_per_day),
        wettest(|b| b.observed_mm_per_day)
    );
    (jja_model, djf_model)
}

/// Asserts that on the model each contrast's dry side is drier than its wet side: each of `ratios`,
/// one a contrast of `contrasts`, under 1.
fn assert_each_dry_side_is_drier(contrasts: &[Contrast], ratios: &[Ratio]) {
    for (contrast, ratio) in contrasts.iter().zip(ratios) {
        assert!(
            ratio.model < 1.0,
            "{} is no drier than {}: {}",
            contrast.dry.name,
            contrast.wet.name,
            ratio.model
        );
    }
}

/// The observed tables hold Earth's land boxes, the climate model's 2.5° × 5° boxes whose Natural
/// Earth land share is at least half ([`EARTH_LAND_BOXES`], [`LAND_BOX_SHARE`]), each once, in band
/// order from the south and each band's in column order, the two tables' boxes the same; and every
/// value is within its field's range (precipitation at most 30 mm/day, temperature within 220–320 K).
#[test]
fn the_observed_tables_hold_earth_s_land_boxes_in_order() {
    let expected: Vec<(u8, u8)> = (0..LATITUDE_BANDS)
        .flat_map(|band| (0..GEOGRAPHIC_COLUMNS).map(move |column| (band, column)))
        .filter(|&(band, column)| EARTH_LAND_BOXES[band][column] >= LAND_BOX_SHARE)
        .map(|(band, column)| {
            (
                u8::try_from(band).expect("a band fits a u8"),
                u8::try_from(column).expect("a column fits a u8"),
            )
        })
        .collect();
    let rain: Vec<(u8, u8)> = GPCP_LAND_BOXES_HUNDREDTH_MM_PER_DAY
        .iter()
        .map(|&(b, c, _)| (b, c))
        .collect();
    let air: Vec<(u8, u8)> = NCEP_LAND_BOXES_HUNDREDTH_K
        .iter()
        .map(|&(b, c, _)| (b, c))
        .collect();
    assert_eq!(rain, expected, "GPCP's boxes");
    assert_eq!(air, expected, "NCEP's boxes");
    for &(band, column, months) in GPCP_LAND_BOXES_HUNDREDTH_MM_PER_DAY {
        assert!(
            months.iter().all(|&p| p <= 3_000),
            "({band}, {column}): {months:?}"
        );
    }
    for &(band, column, months) in NCEP_LAND_BOXES_HUNDREDTH_K {
        assert!(
            months.iter().all(|&t| (22_000..=32_000).contains(&t)),
            "({band}, {column}): {months:?}"
        );
    }
    eprintln!("{} land boxes", expected.len());
}

/// R09.T13.l, which replaces R09.T13's offline `ExoPlaSim` check: the reference Earth on R09.T13.g's
/// relief Earth (the synthetic sea of Natural Earth's land by 2.5° × 5° box, its heights Earth2014's
/// band eighths placed by each box's median land height; [`synthetic_sea`]), the whole step at level
/// 7 on its geographic model, against GPCP v2.3 and NCEP/NCAR R1 over 1991–2020 on the land boxes.
/// A model box is the solid-angle mean of the climate cells whose centres lie in it, land and sea,
/// as an observed box is the mean of its whole area; the model's month m stands for calendar month
/// m.
///
/// It prints, each month and for the annual means, the pattern correlation, the pattern correlation
/// of the departures from each band's mean over its land boxes (the structure in longitude), the RMS
/// and the bias, of the sea-level temperature (over every land box, and over those within 60° of the
/// equator, where the reanalysis's σ = 0.995 field stands for its 2 m one) and of the precipitation;
/// and the named signatures of the precipitation heuristic: the rain shadows of the Andes, the
/// Sierra Nevada and the Himalaya ([`RAIN_SHADOWS`]), the west coasts' deserts ([`WEST_COASTS`])
/// and the Indian monsoon's JJA-against-DJF shift ([`INDIA`]).
#[test]
#[ignore = "slow: the reference Earth's step at level 7 on its geographic model, about 55 s"]
fn the_relief_earth_s_monthly_fields_against_gpcp_and_ncep_r1() {
    let inputs = reference::earth_like();
    let (field, climate) = grids(7);
    let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
    let mut working = Working::initial(&pass);
    synthetic_sea(&pass, &mut working, &EARTH_HYPSOMETRY_M);
    super::super::run(&pass, &mut working);
    let (boxes, empty) = land_boxes(&pass, &working);
    eprintln!(
        "The relief Earth against GPCP v2.3 and NCEP R1, 1991–2020, over Earth's land boxes \
         ({} of them; {empty} hold no climate cell and are left out)",
        boxes.len()
    );
    let temperature = report_skill(
        "the sea-level air temperature",
        "K",
        &boxes,
        |b| b.model_k,
        |b| b.observed_k,
    );
    let temperate: Vec<LandBox> = boxes
        .iter()
        .filter(|b| centre_deg(b.band, b.column).0.abs() < TEMPERATE_DEG)
        .cloned()
        .collect();
    report_skill(
        "the sea-level air temperature within 60° of the equator",
        "K",
        &temperate,
        |b| b.model_k,
        |b| b.observed_k,
    );
    report_cycle(&boxes);
    let precipitation = report_skill(
        "the precipitation",
        "mm/day",
        &boxes,
        |b| b.model_mm_per_day,
        |b| b.observed_mm_per_day,
    );
    eprintln!("  the rain shadows, the lee's annual precipitation over the windward side's:");
    let shadows: Vec<Ratio> = RAIN_SHADOWS.iter().map(|c| c.report(&boxes)).collect();
    report_himalaya_summer(&boxes);
    eprintln!("  the west coasts' deserts, the west coast's annual precipitation over the east's:");
    let deserts: Vec<Ratio> = WEST_COASTS.iter().map(|c| c.report(&boxes)).collect();
    let (summer, winter) = report_monsoon(&boxes);
    for (m, s) in temperature.iter().enumerate() {
        assert!(
            s.correlation >= LEAST_TEMPERATURE_CORRELATION,
            "month {}: the temperature's pattern correlation {}",
            m + 1,
            s.correlation
        );
    }
    for (m, s) in precipitation.iter().enumerate() {
        assert!(
            s.correlation >= LEAST_PRECIPITATION_CORRELATION,
            "month {}: the precipitation's pattern correlation {}",
            m + 1,
            s.correlation
        );
        assert!(
            s.departure_correlation > 0.0,
            "month {}: the precipitation's departures' pattern correlation {}",
            m + 1,
            s.departure_correlation
        );
    }
    assert_each_dry_side_is_drier(&RAIN_SHADOWS, &shadows);
    assert_each_dry_side_is_drier(&WEST_COASTS, &deserts);
    assert!(
        summer > winter,
        "India's JJA {summer} mm/day against its DJF {winter}"
    );
}
