//! The months of a body's year, and a time's place between them (plan R09, Design note 8;
//! `decision-r09-t2.md` item 1).
//!
//! A year has one month, on a world with no seasonal forcing, or twelve. The twelve are equal
//! spans of the seasonal orbit's eccentric anomaly E, counted from periapsis: month k spans E from
//! k · 30° to (k + 1) · 30°. That orbit is the one that sets the sun's declination and distance,
//! the body's own about its star or stars, or its planet's for a moon, and the header carries its
//! eccentricity e ([`FieldHeader::season_eccentricity`]). Equal spans of eccentric anomaly rather
//! than of time hold an eccentric orbit's brief periapsis season in twelve samples, which twelve
//! equal-time months miss above e ≈ 0.3 (the ruling's evidence).
//!
//! The time along the orbit is its mean anomaly M, which grows uniformly with time from 0 at
//! periapsis. Kepler's equation, M = E − e sin E (Murray and Dermott 1999, _Solar System
//! Dynamics_, §2.4, eq. 2.52), turns each month's edges into mean anomalies directly,
//! Mₖ = k · 30° − e sin(k · 30°), so month k's share of the period is
//! [ΔE − e (sin Eₖ₊₁ − sin Eₖ)] ÷ 2π: a twelfth on a circular orbit, 29.95 to 30.92 days on
//! Earth's (e = 0.0167), and 0.43 to 1.57 twelfths at e = 0.6, the shortest at periapsis.
//!
//! Since the edges are mean anomalies already, nothing here solves Kepler's equation: a month and
//! a blend are comparisons and the four operators on fixed sines of multiples of 30° (exact but
//! for √3 ÷ 2, a constant), so they are the same bits natively and as WebAssembly. The one
//! remainder, [`math::fmod`], is exact. A golden file pins them on both targets.

use core::f64::consts::{FRAC_PI_6, TAU};

use hyperion_base::math;
use hyperion_base::units::Radians;

use super::FieldHeader;

/// The months of a seasonal year.
const MONTHS: usize = 12;

/// sin 60° = √3 ÷ 2, the nearest `f64` (a test holds it to `3.0_f64.sqrt() / 2.0`, which is
/// correctly rounded, since `sqrt` is and halving is exact).
const HALF_ROOT_3: f64 = 0.866_025_403_784_438_6;

/// sin(k · 30°) for k from 0 to 12, exact where the sine is 0, ±½ or ±1.
const EDGE_SINES: [f64; MONTHS + 1] = [
    0.0,
    0.5,
    HALF_ROOT_3,
    1.0,
    HALF_ROOT_3,
    0.5,
    0.0,
    -0.5,
    -HALF_ROOT_3,
    -1.0,
    -HALF_ROOT_3,
    -0.5,
    0.0,
];

/// A time's place between the two nearest months (Design note 8): month [`from`](Self::from)
/// with weight 1 − w and month [`to`](Self::to) with weight w ([`weight`](Self::weight)), so
/// that a monthly quantity x read at that time is (1 − w) x(from) + w x(to).
///
/// The weight is linear in mean anomaly, that is in time, between the two months' centres, each
/// the mean-anomaly midpoint of its span; it is 0 at `from`'s centre and rises towards 1 at
/// `to`'s, so the seasons turn without a step at a month's edge, and faster near periapsis on an
/// eccentric orbit, as the climate does. In a one-month year it is month 0 alone, which is also
/// the default.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MonthBlend {
    from: u8,
    to: u8,
    weight: f64,
}

impl MonthBlend {
    /// The earlier month, from 0, whose centre is at or before the time.
    #[must_use]
    pub fn from(self) -> u8 {
        self.from
    }

    /// The later month, from 0: the next after [`from`](Self::from), or month 0 after month 11,
    /// across periapsis; month 0 in a one-month year.
    #[must_use]
    pub fn to(self) -> u8 {
        self.to
    }

    /// The weight of [`to`](Self::to), 0 to 1: 0 at `from`'s centre.
    #[must_use]
    pub fn weight(self) -> f64 {
        self.weight
    }
}

/// The mean anomalies of the twelve months' edges on a seasonal orbit of eccentricity
/// `season_eccentricity`, radians from periapsis, or `None` unless 0 ≤ e < 1.
///
/// Edge k is Mₖ = k · 30° − e sin(k · 30°) for k from 0 to 11, and edge 12 is 2π, the year's end.
/// They increase strictly, since dM ÷ dE = 1 − e cos E is positive for e below 1. Month k is the
/// mean anomalies from edge k up to, not including, edge k + 1. The coarse pass bins its steps
/// into months by them (R09.T13.a), and the classes count months by their durations from them
/// (R09.T15).
#[must_use]
pub fn month_edges(season_eccentricity: f64) -> Option<[Radians; MONTHS + 1]> {
    let e = season_eccentricity;
    if !(0.0..1.0).contains(&e) {
        return None;
    }
    let mut edges = [Radians::new(TAU); MONTHS + 1];
    for ((k, edge), sine) in (0_u8..).zip(&mut edges[..MONTHS]).zip(EDGE_SINES) {
        *edge = Radians::new(f64::from(k) * FRAC_PI_6 - e * sine);
    }
    Some(edges)
}

/// The share of the period that month `month` (from 0) of `header`'s year spans: 1 for a
/// one-month year's month, and (Mₖ₊₁ − Mₖ) ÷ 2π for month k of twelve ([`month_edges`]); `None`
/// past the header's months.
#[must_use]
pub fn month_share(header: &FieldHeader, month: u8) -> Option<f64> {
    if month >= header.months() {
        return None;
    }
    if header.months() == 1 {
        return Some(1.0);
    }
    let edges = header_edges(header);
    let k = usize::from(month);
    Some((edges[k + 1].value() - edges[k].value()) / TAU)
}

/// The month (from 0) of `header`'s year that holds the time at the seasonal orbit's mean anomaly
/// `mean_anomaly` (radians, any finite angle, 0 at periapsis): the k whose edges hold it
/// ([`month_edges`]), and 0 in a one-month year.
///
/// # Panics
///
/// If `mean_anomaly` is not finite.
#[must_use]
pub fn month_at(header: &FieldHeader, mean_anomaly: Radians) -> u8 {
    let m = reduce(mean_anomaly);
    if header.months() == 1 {
        return 0;
    }
    let edges = header_edges(header);
    (0_u8..12)
        .rev()
        .find(|&k| edges[usize::from(k)].value() <= m)
        .expect("the first edge is 0, at or before every reduced mean anomaly")
}

/// The blend of the two months of `header`'s year nearest the time at the seasonal orbit's mean
/// anomaly `mean_anomaly` (radians, any finite angle, 0 at periapsis), linear in mean anomaly
/// between their centres ([`MonthBlend`]); month 0 alone in a one-month year.
///
/// It is continuous in time, across every month's edge and across periapsis, where month 11
/// blends into month 0, and gives a month alone at its centre. A reader finds the mean anomaly
/// at the scene's time from the seasonal orbit and blends each monthly field with this (R11's
/// coverage, wind and sea state).
///
/// # Panics
///
/// If `mean_anomaly` is not finite.
#[must_use]
pub fn month_blend(header: &FieldHeader, mean_anomaly: Radians) -> MonthBlend {
    let m = reduce(mean_anomaly);
    if header.months() == 1 {
        return MonthBlend {
            from: 0,
            to: 0,
            weight: 0.0,
        };
    }
    let edges = header_edges(header);
    // `f64::midpoint` of operands this size is (a + b) ÷ 2, two of the four operators.
    let centres: [f64; MONTHS] =
        core::array::from_fn(|k| f64::midpoint(edges[k].value(), edges[k + 1].value()));
    // Month 11's centre lies in [23π ÷ 12, 2π) for every e below 1, so it and every mean anomaly
    // past it lose 2π exactly (Sterbenz), and both sides of periapsis read the one span alike.
    let last_before_periapsis = centres[MONTHS - 1] - TAU;
    let blend = |from: u8, to: u8, start: f64, end: f64, at: f64| MonthBlend {
        from,
        to,
        weight: (at - start) / (end - start),
    };
    match (0_u8..12).rev().find(|&k| centres[usize::from(k)] <= m) {
        Some(11) => blend(11, 0, last_before_periapsis, centres[0], m - TAU),
        Some(k) => {
            let n = usize::from(k);
            blend(k, k + 1, centres[n], centres[n + 1], m)
        }
        None => blend(11, 0, last_before_periapsis, centres[0], m),
    }
}

/// The edges of `header`'s twelve months.
fn header_edges(header: &FieldHeader) -> [Radians; MONTHS + 1] {
    month_edges(header.season_eccentricity())
        .expect("a header's season eccentricity is at least 0 and below 1")
}

/// `mean_anomaly` reduced to [0, 2π): its exact remainder modulo 2π, lifted by 2π if negative, and
/// +0 for a lift that rounds to 2π or a remainder of −0.
fn reduce(mean_anomaly: Radians) -> f64 {
    let m = mean_anomaly.value();
    assert!(m.is_finite(), "a mean anomaly must be finite, got {m}");
    let turn = math::fmod(m, TAU);
    if turn < 0.0 {
        let lifted = turn + TAU;
        if lifted < TAU { lifted } else { 0.0 }
    } else {
        // −0 + 0 is +0, so a whole number of turns below zero reduces to +0 as one above does.
        turn + 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldView;
    use crate::testing::{SyntheticWorld, synthetic_field};
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// A twelve-month header of season eccentricity `e`, from the Ceres-like world's.
    fn header(e: f64) -> FieldHeader {
        let mut parts = synthetic_field(SyntheticWorld::CeresLike)
            .header()
            .parts()
            .clone();
        parts.months = 12;
        parts.season_eccentricity = e;
        FieldHeader::new(parts).unwrap()
    }

    /// The value of the monthly quantity `values` that `blend` reads.
    fn blended(blend: MonthBlend, values: &[f64; 12]) -> f64 {
        let w = blend.weight();
        (1.0 - w) * values[usize::from(blend.from())] + w * values[usize::from(blend.to())]
    }

    #[test]
    fn the_half_root_of_three_is_the_correctly_rounded_one() {
        assert_same_bits(HALF_ROOT_3, 3.0_f64.sqrt() / 2.0);
    }

    /// On a circular orbit every month is a twelfth of the period.
    #[test]
    fn on_a_circular_orbit_every_month_is_a_twelfth() {
        let h = header(0.0);
        for month in 0..12 {
            let share = month_share(&h, month).unwrap();
            assert!((share - 1.0 / 12.0).abs() < 1e-15, "month {month}: {share}");
        }
        assert_eq!(month_share(&h, 12), None);
    }

    /// At e = 0.6 the shares sum to 1 to 10⁻¹², the periapsis month is (π ÷ 6 − 0.6 sin 30°) ÷ 2π
    /// of the period and the apoapsis month (π ÷ 6 + 0.6 sin 30°) ÷ 2π, 0.43 and 1.57 twelfths.
    #[test]
    fn an_eccentric_orbit_s_months_share_its_period_by_kepler_s_equation() {
        let h = header(0.6);
        let shares: Vec<f64> = (0..12).map(|k| month_share(&h, k).unwrap()).collect();
        let total: f64 = shares.iter().sum();
        assert!((total - 1.0).abs() < 1e-12, "{total}");
        let half_month = core::f64::consts::PI / 6.0;
        let periapsis = (half_month - 0.6 * 0.5) / TAU;
        let apoapsis = (half_month + 0.6 * 0.5) / TAU;
        assert!((shares[0] - periapsis).abs() < 1e-15, "{}", shares[0]);
        assert!((shares[6] - apoapsis).abs() < 1e-15, "{}", shares[6]);
        assert!((shares[0] * 12.0 - 0.427).abs() < 1e-3);
        assert!((shares[6] * 12.0 - 1.573).abs() < 1e-3);
        // Earth's months, of a sidereal year of 365.256 days, run 29.95 to 30.92 days.
        let earth = header(0.0167);
        let days: Vec<f64> = (0..12)
            .map(|k| month_share(&earth, k).unwrap() * 365.256)
            .collect();
        assert!((days[0] - 29.953).abs() < 1e-3, "{days:?}");
        assert!((days[6] - 30.923).abs() < 1e-3, "{days:?}");
    }

    /// Each month's edges hold the mean anomalies `month_at` gives it, an edge itself belonging
    /// to the month it opens; any angle is reduced first.
    #[test]
    fn month_at_finds_the_month_whose_edges_hold_the_time() {
        for e in [0.0, 0.0167, 0.6, 0.95] {
            let h = header(e);
            let edges = month_edges(e).unwrap();
            for k in 0..12_u8 {
                let start = edges[usize::from(k)].value();
                let end = edges[usize::from(k) + 1].value();
                assert_eq!(month_at(&h, Radians::new(start)), k, "e {e}");
                assert_eq!(
                    month_at(&h, Radians::new(f64::midpoint(start, end))),
                    k,
                    "e {e}"
                );
                assert_eq!(month_at(&h, Radians::new(end.next_down())), k, "e {e}");
            }
            assert_eq!(month_at(&h, Radians::new(TAU)), 0);
            assert_eq!(month_at(&h, Radians::new(-1e-9)), 11);
            // A lift to 2π that rounds is periapsis, month 0.
            assert_eq!(month_at(&h, Radians::new(-1e-300)), 0);
            let mid_april = f64::midpoint(edges[3].value(), edges[4].value());
            assert_eq!(month_at(&h, Radians::new(5.0 * TAU + mid_april)), 3);
            assert_eq!(month_at(&h, Radians::new(mid_april - 3.0 * TAU)), 3);
        }
        assert_eq!(month_edges(1.0), None);
        assert_eq!(month_edges(f64::NAN), None);
    }

    /// The blend gives each month alone at its centre, moves without a step across every month's
    /// edge, centre and periapsis, and its weight stays in [0, 1].
    #[test]
    fn month_blend_is_continuous_and_gives_each_month_alone_at_its_centre() {
        let values = [
            3.0, -1.0, 7.5, 0.25, 3.1, -0.9, 7.6, 0.35, 3.2, -0.8, 7.7, 0.45,
        ];
        for e in [0.0, 0.0934, 0.6, 0.9] {
            let h = header(e);
            let edges = month_edges(e).unwrap();
            for k in 0..12_u8 {
                let n = usize::from(k);
                let centre = f64::midpoint(edges[n].value(), edges[n + 1].value());
                let at_centre = month_blend(&h, Radians::new(centre));
                assert_eq!(at_centre.from(), k, "e {e}");
                assert_same_bits(at_centre.weight(), 0.0);
                assert_same_bits(blended(at_centre, &values), values[n]);
                for point in [edges[n].value(), centre] {
                    let below = month_blend(&h, Radians::new(point - 1e-9));
                    let above = month_blend(&h, Radians::new(point + 1e-9));
                    let step = (blended(above, &values) - blended(below, &values)).abs();
                    assert!(step < 1e-6, "e {e}, month {k}: a step of {step} at {point}");
                }
            }
            let mut m = -0.3;
            while m < TAU + 0.3 {
                let w = month_blend(&h, Radians::new(m)).weight();
                assert!((0.0..=1.0).contains(&w), "e {e}: weight {w} at {m}");
                m += 1e-3;
            }
            // Across periapsis month 11 blends into month 0 on one span, whose weight the last
            // `f64` before 2π and 0 itself give within their separation, 8.9 × 10⁻¹⁶ rad over a
            // span of at least 0.07 rad.
            let before = month_blend(&h, Radians::new(TAU.next_down()));
            let after = month_blend(&h, Radians::new(0.0));
            assert_eq!((before.from(), before.to()), (11, 0));
            assert_eq!((after.from(), after.to()), (11, 0));
            let rise = after.weight() - before.weight();
            assert!((0.0..1e-13).contains(&rise), "e {e}: {rise}");
        }
    }

    /// A one-month year has month 0 alone, the whole period long, at every time.
    #[test]
    fn a_one_month_year_is_month_zero_alone() {
        let flat = synthetic_field(SyntheticWorld::Flat);
        let h = flat.header();
        assert_eq!(h.months(), 1);
        for m in [-7.0, 0.0, 1.0, 100.0] {
            let blend = month_blend(h, Radians::new(m));
            assert_eq!((blend.from(), blend.to()), (0, 0));
            assert_same_bits(blend.weight(), 0.0);
            assert_eq!(month_at(h, Radians::new(m)), 0);
        }
        assert_eq!(month_share(h, 0), Some(1.0));
        assert_eq!(month_share(h, 1), None);
    }

    /// The golden file `tests/golden/field/months.golden`: the edges, shares, months and blends
    /// of five orbits to e just below 1, at times that include both zeros, a lift that rounds to
    /// 2π, the last `f64` below it, many turns either way and a large angle, to the bit, so that
    /// the server and the client's WebAssembly read the same months. The months are generated
    /// output (Design note 8): a change to them fails here, and is a generator-version change.
    #[test]
    fn months_match_their_golden() {
        let mut w = GoldenWriter::new();
        w.header(crate::generator_version());
        let anomalies = [
            -0.0,
            0.0,
            -1e-300,
            -1e-9,
            0.3,
            1.0,
            core::f64::consts::PI,
            5.0,
            TAU.next_down(),
            TAU,
            7.5 * TAU + 0.2,
            -3.0 * TAU - 0.2,
            1e6,
        ];
        for e in [0.0, 0.016_711_23, 0.6, 0.95, 1.0_f64.next_down()] {
            let h = header(e);
            for (k, edge) in month_edges(e).unwrap().iter().enumerate() {
                w.f64(&format!("e {e:e} edge[{k}]"), edge.value());
            }
            for k in 0..12 {
                w.f64(&format!("e {e:e} share[{k}]"), month_share(&h, k).unwrap());
            }
            for m in anomalies {
                let blend = month_blend(&h, Radians::new(m));
                w.line(&format!(
                    "e {e:e} m {m:e}: month {}, blend {} to {}",
                    month_at(&h, Radians::new(m)),
                    blend.from(),
                    blend.to()
                ));
                w.f64(&format!("e {e:e} m {m:e} weight"), blend.weight());
            }
        }
        golden!("field/months", w.as_str());
    }

    /// Tests that read files, which the browser target cannot.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    mod native_only {
        /// Every golden file under `tests/golden/field/` carries `GENERATOR_VERSION`, not R05's
        /// `TEST_PLANET_VERSION`, which heads the files directly under `tests/golden/`.
        #[test]
        fn every_field_golden_carries_the_generator_version() {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("golden")
                .join("field");
            let expected = format!("# generator_version = {}", crate::generator_version());
            let mut seen = 0;
            for entry in std::fs::read_dir(&dir).expect("the field golden directory is readable") {
                let path = entry.expect("a directory entry is readable").path();
                if path.extension().is_some_and(|e| e == "golden") {
                    let text = std::fs::read_to_string(&path).expect("a golden file is readable");
                    assert_eq!(
                        text.lines().next(),
                        Some(expected.as_str()),
                        "{}",
                        path.display()
                    );
                    seen += 1;
                }
            }
            assert_eq!(seen, 1, "the field goldens in {}", dir.display());
        }
    }
}
