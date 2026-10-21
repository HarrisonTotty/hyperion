//! Köppen–Geiger's climate classes for a cell's twelve months, by Peel, Finlayson and McMahon
//! 2007's Table 1 (Hydrol. Earth Syst. Sci. 11, 1633, p. 1636) with Beck et al. 2018's rules (Sci.
//! Data 5, 180214, Table 2 p. 5), applied to rates (R09.T15, Design note 11).
//!
//! The table's figures are a station's: T in °C, a month's precipitation in millimetres and the
//! year's (MAP) in millimetres a year. A generated world's months are not Earth's, so the table is
//! read in rates: a month's precipitation is its rate over a twelfth of a Julian year, Design note
//! 11's 30.4375 d (365.25 ÷ 12, so that MAP is the months' sum, as Beck et al.'s `KoppenGeiger.m`
//! sums them), and MAP its mean rate, each month weighted by its share of the year, over 365.25 d.
//! Every count of months, `T_mon10` the months above 10 °C, counts their durations in twelfths of
//! the year, since an eccentric orbit's months differ in length (Design note 8: they are equal in
//! eccentric anomaly); the temperature thresholds stay. Each month's share is its span between
//! `month_edges` over 2π, the same as `month_share`'s, which reads a header the pass has not built.
//!
//! The summer is the cell's warmest consecutive half-year (Design note 11): HYPERION's adaptation
//! of the table's summer, the warmer of ONDJFM and AMJJAS, to a calendar with neither. The
//! candidates are the twelve windows of half the year's duration that open at a month's edge, the
//! warmest the one of highest duration-weighted mean anomaly (ties to the earliest), so that the
//! choice is the climate cell's alone, whatever a field cell's height; the winter is its
//! complement, six months each on a circular orbit. The halves' totals (the 70% rule, the s-and-w
//! tie) weight each month by its duration inside the half; the driest and wettest months of a half
//! are of the months at least half inside it.
//!
//! The order of decision is the table's: B first, taking precedence over the rest (Peel p. 1637,
//! "the B climates must be identified first"; Beck et al. p. 2), then A, E, and C or D. E is
//! `T_hot` ≤ 10 °C (Beck et al.'s Table 2; Peel's `T_hot` < 10 leaves 10 °C in no group). The
//! 70% rule is strict (Peel's "over 70%"), a tie of s and w gives s (Beck et al.'s
//! `KoppenGeiger.m`), and C's c is "not a or b" without Peel's 1 ≤ `T_mon10`, which months counted
//! in twelfths could miss (decision-r09-t15-classes §4 and §8). Where Beck et al.'s code and their
//! Table 2 differ (the code's `Pw*2.333>Ps` tests a 30% share), the table governs.

use core::f64::consts::{PI, TAU};

use hyperion_surface::field::KoppenGeiger;
use hyperion_surface::num;

use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{Kelvin, Radians};

/// The months of a seasonal year.
pub(crate) const MONTHS: usize = 12;

/// 0 °C, kelvin: the table's temperatures are Celsius.
const ZERO_CELSIUS_K: f64 = 273.15;

/// The month the table's monthly precipitation is counted over, s: a twelfth of a Julian year,
/// Design note 11's 30.4375 d, so that the year's total is the months' sum.
const SECONDS_PER_TABLE_MONTH: f64 = SECONDS_PER_JULIAN_YEAR / 12.0;

/// The share of the year's precipitation one half-year must exceed for the arid threshold to take
/// that half's offset: 70% (Peel et al. 2007, Table 1, "over 70%").
const SEASONAL_SHARE: f64 = 0.7;

/// The months a b class needs above 10 °C, twelfths of the year (Peel et al. 2007, Table 1).
const WARM_MONTHS: f64 = 4.0;

/// How far below a count of months, in twelfths of the year, a duration still meets it: 10⁻⁹,
/// about 0.03 s of an Earth year, so that whole months on a circular orbit, whose shares are a
/// twelfth each only to rounding, count whole.
const COUNT_TOLERANCE_TWELFTHS: f64 = 1e-9;

/// The arid threshold's offset where over 70% of the precipitation falls in the summer half, mm a
/// year (Peel et al. 2007, Table 1).
const SUMMER_OFFSET_MM: f64 = 28.0;

/// The arid threshold's offset where neither half holds over 70%, mm a year (Peel et al. 2007,
/// Table 1).
const EVEN_OFFSET_MM: f64 = 14.0;

/// One month of a cell's climatology, as the classifier reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Month {
    /// The month's mean temperature less the annual mean, kelvin: the climate cell's anomaly.
    pub anomaly_k: f64,
    /// The month's mean precipitation rate, kg m⁻² s⁻¹ (a millimetre of water a second),
    /// non-negative.
    pub precipitation_kg_m2_s: f64,
}

/// Which half-year's offset the arid threshold took (Peel et al. 2007, Table 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum AridOffset {
    /// Over 70% of the year's precipitation falls in the winter half: `P_th` = 2 MAT.
    Winter,
    /// Over 70% falls in the summer half: `P_th` = 2 MAT + 28.
    Summer,
    /// Neither, or nothing falls: `P_th` = 2 MAT + 14.
    Even,
}

/// The second letter of a C or D class: the dry season.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrySeason {
    /// s, a dry summer.
    Summer,
    /// w, a dry winter.
    Winter,
    /// f, no dry season.
    None,
}

/// The third letter of a C or D class: the summer's heat or the winter's cold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Heat {
    /// a, a hot summer.
    Hot,
    /// b, a warm summer.
    Warm,
    /// c, a cold summer.
    Cold,
    /// d, a very cold winter (D alone).
    VeryCold,
}

/// A cell's class with the figures that decided it, which the reference worlds' tests print.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reading {
    /// The class.
    pub class: KoppenGeiger,
    /// The arid threshold's offset, by the halves' shares of the precipitation.
    pub offset: AridOffset,
    /// The year's mean precipitation, MAP, mm a Julian year.
    pub annual_precipitation_mm: f64,
    /// `T_mon10`: the months above 10 °C, counted by their durations in twelfths of the year.
    pub warm_twelfths: f64,
}

/// The year's two halves for one cell: each month's duration inside the summer, and whether the
/// month belongs to the summer (at least half of it inside).
#[derive(Debug, Clone, Copy)]
struct Halves {
    /// Each month's span inside the summer, radians of mean anomaly.
    summer_span_rad: [f64; MONTHS],
    in_summer: [bool; MONTHS],
}

/// Köppen–Geiger's class of a cell whose twelve months are `months` about its annual mean
/// temperature `annual_mean`, their edges in mean anomaly `edges` (`month_edges`').
///
/// # Panics
///
/// If an anomaly, a rate or the annual mean is not finite, a rate is negative, or the edges do not
/// increase from 0 to 2π (a fault of the caller).
#[must_use]
pub(crate) fn classify(
    months: &[Month; MONTHS],
    edges: &[Radians; MONTHS + 1],
    annual_mean: Kelvin,
) -> Reading {
    let year = Year::of(months, edges, annual_mean);
    let class = year
        .arid()
        .or_else(|| year.tropical())
        .or_else(|| year.polar())
        .unwrap_or_else(|| year.temperate_or_continental());
    Reading {
        class,
        offset: year.offset,
        annual_precipitation_mm: year.map_mm,
        warm_twelfths: year.warm_twelfths,
    }
}

/// The table's figures of one cell's year.
#[derive(Debug, Clone, Copy)]
struct Year {
    /// Each month's precipitation over a table month, mm.
    monthly_mm: [f64; MONTHS],
    /// MAT, °C.
    mat_c: f64,
    /// MAP, mm a Julian year.
    map_mm: f64,
    /// `T_hot`, the warmest month's mean, °C.
    hot_c: f64,
    /// `T_cold`, the coldest month's mean, °C.
    cold_c: f64,
    /// `T_mon10`, twelfths of the year.
    warm_twelfths: f64,
    /// The summer and the winter.
    halves: Halves,
    /// The summer's precipitation, each month's rate times its span inside the half, kg m⁻² s⁻¹
    /// rad.
    summer_kg_m2_s_rad: f64,
    /// The winter's, alike.
    winter_kg_m2_s_rad: f64,
    /// The arid threshold's offset.
    offset: AridOffset,
}

impl Year {
    /// The figures of `months` on `edges` about the annual mean `annual_mean`.
    #[must_use]
    fn of(months: &[Month; MONTHS], edges: &[Radians; MONTHS + 1], annual_mean: Kelvin) -> Self {
        let span = |m: usize| edges[m + 1].value() - edges[m].value();
        for (m, month) in months.iter().enumerate() {
            let (a, rate) = (month.anomaly_k, month.precipitation_kg_m2_s);
            assert!(a.is_finite(), "month {m}'s anomaly is not finite: {a}");
            assert!(
                rate.is_finite() && rate >= 0.0,
                "month {m}'s precipitation is not a finite rate of at least zero: {rate}"
            );
            assert!(span(m) > 0.0, "month {m}'s edges do not increase");
        }
        assert!(
            annual_mean.value().is_finite(),
            "the annual mean is not finite: {}",
            annual_mean.value()
        );
        let mat_c = annual_mean.value() - ZERO_CELSIUS_K;
        let celsius: [f64; MONTHS] = core::array::from_fn(|m| mat_c + months[m].anomaly_k);
        let (hot_c, cold_c) = celsius
            .iter()
            .fold((f64::NEG_INFINITY, f64::INFINITY), |(hot, cold), &t| {
                (num::max(hot, t), num::min(cold, t))
            });
        let year_kg_m2_s_rad: f64 = (0..MONTHS)
            .map(|m| span(m) * months[m].precipitation_kg_m2_s)
            .sum();
        let warm_twelfths = 12.0
            * (0..MONTHS)
                .filter(|&m| celsius[m] > 10.0)
                .map(|m| span(m) / TAU)
                .sum::<f64>();
        let halves = halves(months, edges);
        let summer_kg_m2_s_rad: f64 = (0..MONTHS)
            .map(|m| halves.summer_span_rad[m] * months[m].precipitation_kg_m2_s)
            .sum();
        let winter_kg_m2_s_rad = year_kg_m2_s_rad - summer_kg_m2_s_rad;
        let share = |half: f64| year_kg_m2_s_rad > 0.0 && half / year_kg_m2_s_rad > SEASONAL_SHARE;
        let offset = if share(summer_kg_m2_s_rad) {
            AridOffset::Summer
        } else if share(winter_kg_m2_s_rad) {
            AridOffset::Winter
        } else {
            AridOffset::Even
        };
        Self {
            monthly_mm: core::array::from_fn(|m| {
                months[m].precipitation_kg_m2_s * SECONDS_PER_TABLE_MONTH
            }),
            mat_c,
            map_mm: year_kg_m2_s_rad / TAU * SECONDS_PER_JULIAN_YEAR,
            hot_c,
            cold_c,
            warm_twelfths,
            halves,
            summer_kg_m2_s_rad,
            winter_kg_m2_s_rad,
            offset,
        }
    }

    /// The B class, tested first: MAP under ten times the arid threshold `P_th`, W under five
    /// times it, h at an annual mean of 18 °C or more.
    #[must_use]
    fn arid(&self) -> Option<KoppenGeiger> {
        let threshold = 2.0 * self.mat_c
            + match self.offset {
                AridOffset::Winter => 0.0,
                AridOffset::Summer => SUMMER_OFFSET_MM,
                AridOffset::Even => EVEN_OFFSET_MM,
            };
        let class = match (self.map_mm < 5.0 * threshold, self.mat_c >= 18.0) {
            (true, true) => KoppenGeiger::BWh,
            (true, false) => KoppenGeiger::BWk,
            (false, true) => KoppenGeiger::BSh,
            (false, false) => KoppenGeiger::BSk,
        };
        (self.map_mm < 10.0 * threshold).then_some(class)
    }

    /// The A class: the coldest month at least 18 °C, by its driest month against 60 mm and
    /// 100 − MAP ÷ 25 mm.
    #[must_use]
    fn tropical(&self) -> Option<KoppenGeiger> {
        (self.cold_c >= 18.0).then(|| {
            let dry = self
                .monthly_mm
                .iter()
                .copied()
                .fold(f64::INFINITY, num::min);
            if dry >= 60.0 {
                KoppenGeiger::Af
            } else if dry >= 100.0 - self.map_mm / 25.0 {
                KoppenGeiger::Am
            } else {
                KoppenGeiger::Aw
            }
        })
    }

    /// The E class: the warmest month at or below 10 °C, ET above 0 °C.
    #[must_use]
    fn polar(&self) -> Option<KoppenGeiger> {
        let class = if self.hot_c > 0.0 {
            KoppenGeiger::ET
        } else {
            KoppenGeiger::EF
        };
        (self.hot_c <= 10.0).then_some(class)
    }

    /// The C class, the coldest month above 0 °C, or the D class, at or below it: by the dry
    /// season and the summer's heat or the winter's cold.
    #[must_use]
    fn temperate_or_continental(&self) -> KoppenGeiger {
        let half = |in_summer: bool| {
            (0..MONTHS)
                .filter(move |&m| self.halves.in_summer[m] == in_summer)
                .map(|m| self.monthly_mm[m])
        };
        let driest = |in_summer: bool| half(in_summer).fold(f64::INFINITY, num::min);
        let wettest = |in_summer: bool| half(in_summer).fold(f64::NEG_INFINITY, num::max);
        let summer_dry = driest(true);
        let dry_summer = summer_dry < 40.0 && summer_dry < wettest(false) / 3.0;
        let dry_winter = driest(false) < wettest(true) / 10.0;
        let season = match (dry_summer, dry_winter) {
            (true, true) if self.summer_kg_m2_s_rad > self.winter_kg_m2_s_rad => DrySeason::Winter,
            (true, _) => DrySeason::Summer,
            (false, true) => DrySeason::Winter,
            (false, false) => DrySeason::None,
        };
        let temperate = self.cold_c > 0.0;
        let heat = if self.hot_c >= 22.0 {
            Heat::Hot
        } else if self.warm_twelfths + COUNT_TOLERANCE_TWELFTHS >= WARM_MONTHS {
            Heat::Warm
        } else if !temperate && self.cold_c < -38.0 {
            Heat::VeryCold
        } else {
            Heat::Cold
        };
        if temperate {
            temperate_class(season, heat)
        } else {
            continental_class(season, heat)
        }
    }
}

/// The C class of `season` and `heat`, which is never very cold.
#[must_use]
const fn temperate_class(season: DrySeason, heat: Heat) -> KoppenGeiger {
    match (season, heat) {
        (DrySeason::Summer, Heat::Hot) => KoppenGeiger::Csa,
        (DrySeason::Summer, Heat::Warm) => KoppenGeiger::Csb,
        (DrySeason::Summer, Heat::Cold | Heat::VeryCold) => KoppenGeiger::Csc,
        (DrySeason::Winter, Heat::Hot) => KoppenGeiger::Cwa,
        (DrySeason::Winter, Heat::Warm) => KoppenGeiger::Cwb,
        (DrySeason::Winter, Heat::Cold | Heat::VeryCold) => KoppenGeiger::Cwc,
        (DrySeason::None, Heat::Hot) => KoppenGeiger::Cfa,
        (DrySeason::None, Heat::Warm) => KoppenGeiger::Cfb,
        (DrySeason::None, Heat::Cold | Heat::VeryCold) => KoppenGeiger::Cfc,
    }
}

/// The D class of `season` and `heat`.
#[must_use]
const fn continental_class(season: DrySeason, heat: Heat) -> KoppenGeiger {
    match (season, heat) {
        (DrySeason::Summer, Heat::Hot) => KoppenGeiger::Dsa,
        (DrySeason::Summer, Heat::Warm) => KoppenGeiger::Dsb,
        (DrySeason::Summer, Heat::Cold) => KoppenGeiger::Dsc,
        (DrySeason::Summer, Heat::VeryCold) => KoppenGeiger::Dsd,
        (DrySeason::Winter, Heat::Hot) => KoppenGeiger::Dwa,
        (DrySeason::Winter, Heat::Warm) => KoppenGeiger::Dwb,
        (DrySeason::Winter, Heat::Cold) => KoppenGeiger::Dwc,
        (DrySeason::Winter, Heat::VeryCold) => KoppenGeiger::Dwd,
        (DrySeason::None, Heat::Hot) => KoppenGeiger::Dfa,
        (DrySeason::None, Heat::Warm) => KoppenGeiger::Dfb,
        (DrySeason::None, Heat::Cold) => KoppenGeiger::Dfc,
        (DrySeason::None, Heat::VeryCold) => KoppenGeiger::Dfd,
    }
}

/// The summer and winter of a cell whose months are `months` on `edges`: the warmest of the twelve
/// windows of half the year (π of mean anomaly) that open at a month's edge, by the
/// duration-weighted mean anomaly inside, ties to the earliest.
#[must_use]
fn halves(months: &[Month; MONTHS], edges: &[Radians; MONTHS + 1]) -> Halves {
    let spans = |open_rad: f64| -> [f64; MONTHS] {
        core::array::from_fn(|m| overlap(edges[m].value(), edges[m + 1].value(), open_rad))
    };
    let (mut best, mut warmest) = (spans(edges[0].value()), f64::NEG_INFINITY);
    for edge in &edges[..MONTHS] {
        let inside = spans(edge.value());
        let heat: f64 = (0..MONTHS).map(|m| inside[m] * months[m].anomaly_k).sum();
        if heat > warmest {
            (best, warmest) = (inside, heat);
        }
    }
    Halves {
        summer_span_rad: best,
        in_summer: core::array::from_fn(|m| {
            2.0 * best[m] >= edges[m + 1].value() - edges[m].value()
        }),
    }
}

/// The length of the month [`start_rad`, `end_rad`) inside the half-year that opens at
/// `open_rad`, radians: [`open_rad`, `open_rad` + π) taken modulo 2π, with 0 ≤ `start_rad` <
/// `end_rad` ≤ 2π and 0 ≤ `open_rad` < 2π.
#[must_use]
fn overlap(start_rad: f64, end_rad: f64, open_rad: f64) -> f64 {
    let close = open_rad + PI;
    let inside =
        |from: f64, to: f64| num::max(0.0, num::min(end_rad, to) - num::max(start_rad, from));
    if close > TAU {
        inside(open_rad, TAU) + inside(0.0, close - TAU)
    } else {
        inside(open_rad, close)
    }
}
