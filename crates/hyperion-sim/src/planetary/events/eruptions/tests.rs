//! Tests of `body.eruption` (P14.T31.a): the law and its threshold, the bins' limit, the counts
//! against the rate, the durations, what runs at an instant, and the listings' independence of
//! partition and order.

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{
    ALPHA, assert_p_value, assert_poisson_count, ks_one_sample, normal_cdf,
};

use super::super::tests::{SEED, YEAR, body, earth_eruptions, year, years};
use super::*;
use crate::events::testing::{assert_partition_independent, partition};
use crate::events::{EventBin, event_id};
use crate::id::event_tags;
use crate::planetary::hooks::figures::interior::GENERATED_EARTH_HEAT_FLOW;
use crate::planetary::params::SURFACE_MELT_HEAT_FLOW;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

fn model(inputs: &EruptionInputs) -> EruptionModel {
    EruptionModel::new(SEED, body(0x0103), inputs).unwrap()
}

fn list(model: &EruptionModel, window: TimeWindow) -> Vec<Eruption> {
    let mut out = Vec::new();
    eruptions(model, window, &mut out);
    out
}

fn per_year(rate: EventsPerSecond) -> f64 {
    rate.value() * SECONDS_PER_JULIAN_YEAR
}

/// A body of heat flow `flow` W m⁻² and radius `radius_km`.
fn hot(flow: f64, radius_km: f64) -> EruptionInputs {
    EruptionInputs {
        heat_flow: WattsPerSquareMetre::new(flow),
        radius: Metres::new(radius_km * 1e3),
    }
}

#[test]
fn a_model_refuses_its_inputs_out_of_range() {
    for flow in [-1e-6, f64::NAN, f64::INFINITY] {
        let error = EruptionModel::new(SEED, body(1), &hot(flow, 6_371.0)).unwrap_err();
        assert!(
            matches!(error, BuildEruptionModelError::HeatFlow(_)),
            "{flow}"
        );
    }
    for radius in [0.0, -5.0, f64::NAN] {
        let error = EruptionModel::new(SEED, body(1), &hot(0.09, radius)).unwrap_err();
        assert!(
            matches!(error, BuildEruptionModelError::Radius(_)),
            "{radius}"
        );
    }
    assert_eq!(
        BuildEruptionModelError::HeatFlow(WattsPerSquareMetre::new(-1.0)).to_string(),
        "heat flow -1 W m⁻² is not finite and non-negative"
    );
}

/// Earth's rate is Γ⊕, nothing erupts at or below `F_c`, and the rate rises with the heat flow and
/// with the area.
#[test]
fn the_rate_rises_with_heat_flow_from_the_threshold() {
    let earth = model(&earth_eruptions());
    assert!((per_year(earth.listed_rate()) / EARTH_ERUPTIONS_PER_YEAR - 1.0).abs() < 1e-12);
    assert_eq!(earth.outgassing(), 1.0);
    for flow in [0.0, 0.012, 0.022, SURFACE_MELT_HEAT_FLOW.value()] {
        let quiet = model(&hot(flow, 3_389.5));
        assert_eq!(quiet.listed_rate(), EventsPerSecond::ZERO, "{flow} W m⁻²");
        assert_eq!(list(&quiet, years(0, 100)), Vec::<Eruption>::new());
    }
    let mut last = 0.0;
    for flow in [0.026, 0.05, 0.0925, 0.5, 2.24] {
        let rate = model(&hot(flow, 6_371.0)).listed_rate().value();
        assert!(rate > last, "{flow} W m⁻²");
        last = rate;
    }
    let small = model(&EruptionInputs {
        radius: Metres::new(6.371e6 / 2.0),
        ..earth_eruptions()
    });
    assert!((small.listed_rate().value() * 4.0 / earth.listed_rate().value() - 1.0).abs() < 1e-12);
    // An Io erupts some three Earths' worth: 35.7 times Earth's outgassing over 8.2% of its area.
    let io = model(&hot(2.24, 1_821.6));
    let ratio = io.listed_rate().value() / earth.listed_rate().value();
    assert!((2.5..3.5).contains(&ratio), "{ratio}");
}

/// A lava world's onsets are held at the bins' limit, and say so.
#[test]
fn a_lava_world_is_held_at_the_bins_limit() {
    let earth = model(&earth_eruptions());
    assert!(!earth.is_saturated());
    let lava = model(&hot(100.0, 6_371.0));
    assert!(lava.is_saturated());
    let mean = lava.listed_rate().value() * BIN_SECONDS_F64;
    assert!((mean / PoissonBins::MAX_MEAN_PER_BIN - 1.0).abs() < 1e-12);
    assert!(lava.law_rate() > lava.listed_rate());
    let listed = list(&lava, years(0, 1));
    assert!(listed.len() > 3_000, "{}", listed.len());
}

/// Test (a): over 10⁴ years the counts match the rate, an Earth's, a Venus-like world's and a
/// Mars's, which has none.
#[test]
fn counts_over_ten_thousand_years_match_the_rate() {
    let window = years(-5_000, 5_000);
    for (name, inputs) in [
        ("earth", earth_eruptions()),
        ("venus", hot(0.0496, 6_051.8)),
        ("mars", hot(0.022, 3_389.5)),
    ] {
        let model = model(&inputs);
        let count = u64::try_from(list(&model, window).len()).unwrap();
        if name == "mars" {
            assert_eq!(count, 0);
        } else {
            assert_poisson_count(name, count, model.expected_count(window), ALPHA);
        }
    }
}

/// Durations are log-normal about 7 weeks, never above 10 years, and each eruption ends its
/// duration after its onset.
#[test]
fn durations_are_log_normal_about_seven_weeks() {
    let earth = model(&earth_eruptions());
    let listed = list(&earth, years(0, 300));
    assert!(listed.len() > 9_000, "{}", listed.len());
    let max = MAX_DURATION.as_seconds_f64();
    let mut z: Vec<f64> = listed
        .iter()
        .map(|eruption| {
            let seconds = eruption.duration().as_seconds_f64();
            assert!(seconds > 0.0 && seconds <= max);
            assert_eq!(
                eruption.onset().checked_add(eruption.duration()),
                Some(eruption.end())
            );
            math::ln(seconds / MEDIAN_DURATION_SECONDS) / DURATION_LOG_SIGMA
        })
        .collect();
    // The cut at 10 years takes some 2% of the draws; below it they are the normal's, cut there.
    let cut = math::ln(max / MEDIAN_DURATION_SECONDS) / DURATION_LOG_SIGMA;
    z.retain(|&x| x < cut - 1e-9);
    let below = normal_cdf(cut);
    assert_p_value(
        "durations' logarithms",
        ks_one_sample(&mut z, |x| (normal_cdf(x) / below).clamp(0.0, 1.0)).p_value,
        ALPHA,
    );
}

/// What runs at an instant is exactly the long listing's eruptions running then, with none missed
/// by the look-back.
#[test]
fn the_eruptions_running_at_an_instant_are_the_listing_s() {
    let earth = model(&earth_eruptions());
    let long = list(&earth, years(-12, 3));
    for t in [
        year(-1),
        UniverseTime::EPOCH,
        UniverseTime::new(YEAR / 3 + 7, 11).unwrap(),
        year(2),
    ] {
        let mut running = Vec::new();
        eruptions_active_at(&earth, t, &mut running);
        let expected: Vec<Eruption> = long.iter().filter(|e| e.is_active_at(t)).copied().collect();
        assert_eq!(running, expected, "at {t}");
    }
    let mut running = Vec::new();
    eruptions_active_at(&earth, UniverseTime::EPOCH, &mut running);
    assert_ne!(running, Vec::<Eruption>::new());
}

/// Test (c): a window's listing is its pieces' in turn, and the same in any order of asking;
/// every eruption comes back from its ID alone, whose text form round-trips; another body's or
/// another tag's ID names none.
#[test]
fn listings_are_independent_of_partition_and_order() {
    let earth = model(&earth_eruptions());
    let whole = years(-1, 2);
    let cuts = [
        UniverseTime::new(-YEAR / 2, 0).unwrap(),
        UniverseTime::EPOCH,
        UniverseTime::new(BIN_SECONDS * 10 + 1, 999).unwrap(),
    ];
    assert_partition_independent(whole, &cuts, |w| list(&earth, w));
    assert_order_independent(&partition(whole, &cuts), |w| list(&earth, *w));
    let listed = list(&earth, whole);
    for event in &listed {
        assert_eq!(eruption(&earth, event.id()), Some(*event));
        assert_eq!(event.id().to_string().parse::<EventId>(), Ok(event.id()));
    }
    let word = listed[0].id().word();
    let other = event_id(
        body(0x0104).into(),
        BODY_ERUPTION,
        word.bin(),
        word.number(),
    );
    assert_eq!(eruption(&earth, other), None);
    let tag = event_id(
        body(0x0103).into(),
        event_tags::BODY_IMPACT,
        word.bin(),
        word.number(),
    );
    assert_eq!(eruption(&earth, tag), None);
    let beyond = event_id(
        body(0x0103).into(),
        BODY_ERUPTION,
        EventBin::new(0).unwrap(),
        255,
    );
    assert_eq!(eruption(&earth, beyond), None);
    let again = EruptionModel::new(SEED, body(0x0103), &earth_eruptions()).unwrap();
    assert_eq!(list(&again, whole), listed);
}

/// The generator's Earth is the outgassing's unit, so a body at its heat flow erupts as Earth does.
#[test]
fn the_generated_earth_is_the_unit() {
    assert_eq!(outgassing(GENERATED_EARTH_HEAT_FLOW), 1.0);
}

/// The deviation from decision-p14-t31a-events lean 2, against the Global Volcanism Program
/// database's figures: 16.8% of eruptions last over a year and 1.9% over ten, and with the cut at
/// ten years about 27 run at once (the database's 37–44 hold persistent activity besides), by
/// Little's law and over 400 instants of the listing.
#[test]
fn the_durations_match_the_global_volcanism_program() {
    let z = |seconds: f64| math::ln(seconds / MEDIAN_DURATION_SECONDS) / DURATION_LOG_SIGMA;
    let a_year = SECONDS_PER_JULIAN_YEAR;
    let over_a_year = 1.0 - normal_cdf(z(a_year));
    assert!(
        (over_a_year - 0.168).abs() < 0.005,
        "{over_a_year} over a year"
    );
    let over_ten = 1.0 - normal_cdf(z(10.0 * a_year));
    assert!(
        (over_ten - 0.019).abs() < 0.002,
        "{over_ten} over ten years"
    );
    // E[min(D, 10 yr)] for the log-normal: its mean below the cut plus the cut times the share
    // above it.
    let cut = MAX_DURATION.as_seconds_f64();
    let sigma = DURATION_LOG_SIGMA;
    let mean = MEDIAN_DURATION_SECONDS * math::exp(sigma * sigma / 2.0);
    let held = mean * normal_cdf(z(cut) - sigma) + cut * (1.0 - normal_cdf(z(cut)));
    let at_once = EARTH_ERUPTIONS_PER_YEAR * held / a_year;
    assert!((26.0..28.5).contains(&at_once), "{at_once} at once");
    let earth = model(&earth_eruptions());
    let mut total = 0;
    for k in 0..400 {
        let mut running = Vec::new();
        eruptions_active_at(&earth, year(k - 200), &mut running);
        total += running.len();
    }
    #[expect(clippy::cast_precision_loss, reason = "a count of some ten thousand")]
    let sampled = total as f64 / 400.0;
    assert!(
        (24.0..30.0).contains(&sampled),
        "{sampled} at once over 400 instants"
    );
}
