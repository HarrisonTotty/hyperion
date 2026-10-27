//! Tests of `body.storm` (P14.T31.b): the refusal, the count over 10⁴ years against the cycles the
//! phase crosses (test (a, b)), the Saturn-like rate and scatter, the marks, the storms running at
//! an instant, and test (c), the listings' independence of partition and order.

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof, ks_one_sample};

use super::super::tests::{SATURN_PERIOD, SEED, YEAR, body, origin, saturn_storms, year, years};
use super::*;
use crate::events::testing::{assert_partition_independent, partition};
use crate::events::{EventBin, event_id};
use crate::id::event_tags;

fn model() -> StormModel {
    StormModel::new(SEED, body(0x0106), &saturn_storms()).unwrap()
}

fn list(model: &StormModel, window: TimeWindow) -> Vec<Storm> {
    let mut out = Vec::new();
    storms(model, window, &mut out);
    out
}

/// Saturn's period, Julian years.
fn saturn_years() -> f64 {
    SATURN_PERIOD.as_julian_years_f64()
}

#[test]
fn a_model_refuses_a_period_under_the_clock_s_minimum() {
    for seconds in [-1, 0, 1, 15] {
        let period = Span::from_seconds(seconds);
        let inputs = StormInputs {
            orbital_period: period,
            origin: origin(),
        };
        assert_eq!(
            StormModel::new(SEED, body(0x0106), &inputs),
            Err(BuildStormModelError::PeriodTooShort(period)),
            "{seconds} s"
        );
    }
    let shortest = StormInputs {
        orbital_period: LinearClock::MIN_PERIOD,
        origin: origin(),
    };
    assert!(StormModel::new(SEED, body(0x0106), &shortest).is_ok());
    assert_eq!(
        BuildStormModelError::PeriodTooShort(Span::from_seconds(15)).to_string(),
        "a storm period of +15.000000000 s is shorter than the +16.000000000 s minimum"
    );
}

/// Every giant has great storms, and nothing else does.
#[test]
fn the_giants_have_great_storms() {
    for (class, giant) in [
        (PlanetClass::Rocky, false),
        (PlanetClass::Icy, false),
        (PlanetClass::SubNeptune, false),
        (PlanetClass::IceGiant, true),
        (PlanetClass::GasGiant, true),
    ] {
        assert_eq!(has_great_storms(class), giant, "{class:?}");
    }
}

/// Saturn's noise is J = 15 octaves, as the source horizon asks at 29.447 years, with a bound of
/// 37.8 cycles (decision-p14-t31b-phases §2).
#[test]
fn saturn_s_noise_spans_the_source_horizon() {
    let model = model();
    assert_eq!(model.noise().octaves(), 15);
    assert_eq!(model.noise().lattice_cycles(), 1);
    assert_eq!(model.noise().skip(), None);
    let bound = model.noise().noise_bound_cycles();
    assert!((bound - 37.8).abs() < 0.05, "{bound}");
    assert_eq!(model.clock().period(), SATURN_PERIOD);
    assert_eq!(model.clock().origin(), origin());
}

/// Test (a, b) for the storms: the count over 10⁴ years is exactly the whole cycles Φ crosses
/// there, read from `phase_at` at the window's ends, and within 2 × the noise's bound + 1 of
/// 10⁴ ÷ P (decision-p14-t31b-phases §3; the plan's "Poisson interval" does not fit a phase with
/// no skip).
#[test]
fn the_count_is_the_cycles_the_phase_crosses() {
    let model = model();
    let window = years(0, 10_000);
    let listed = list(&model, window);
    let first = model.phase_at(window.start()).cycles();
    let last = model.phase_at(window.end()).cycles();
    let cycles: Vec<i64> = listed.iter().map(Storm::cycle).collect();
    assert_eq!(cycles, ((first + 1)..=last).collect::<Vec<_>>());
    let expected = 10_000.0 / saturn_years();
    let reach = 2.0 * model.noise().noise_bound_cycles() + 1.0;
    let count = f64::from(u32::try_from(listed.len()).unwrap());
    assert!(
        (count - expected).abs() <= reach,
        "{count} storms against {expected} ± {reach}"
    );
}

/// The storms are Saturn-like: about one an orbit, the intervals' coefficient of variation between
/// Saturn's five intervals' without 2010 (0.06) and with it (0.15), and every interval between
/// P ÷ 1.9 and P ÷ 0.1, the slope check's bounds.
#[test]
fn storms_come_about_once_an_orbit_with_saturn_s_scatter() {
    let model = model();
    let listed = list(&model, years(0, 10_000));
    let intervals: Vec<f64> = listed
        .windows(2)
        .map(|pair| {
            pair[1]
                .onset()
                .checked_since(pair[0].onset())
                .unwrap()
                .as_julian_years_f64()
                / saturn_years()
        })
        .collect();
    let n = f64::from(u32::try_from(intervals.len()).unwrap());
    let mean = intervals.iter().sum::<f64>() / n;
    let variance = intervals
        .iter()
        .map(|x| (x - mean) * (x - mean))
        .sum::<f64>()
        / (n - 1.0);
    let cv = variance.sqrt() / mean;
    assert!((mean - 1.0).abs() < 0.05, "mean interval {mean} P");
    assert!((0.06..=0.15).contains(&cv), "interval CV {cv}");
    for interval in &intervals {
        assert!(
            (1.0 / 1.9..=1.0 / 0.1).contains(interval),
            "an interval of {interval} P"
        );
    }
}

/// Each storm's duration share is uniform between its bounds, and its band takes the shares of
/// Saturn's six storms, 3 : 2 : 1.
#[test]
fn the_marks_follow_their_laws() {
    let model = model();
    let listed = list(&model, years(-20_000, 20_000));
    let period = SATURN_PERIOD.as_seconds_f64();
    let mut shares: Vec<f64> = listed
        .iter()
        .map(|storm| {
            assert_eq!(
                storm.end(),
                storm.onset().checked_add(storm.duration()).unwrap()
            );
            storm.duration().as_seconds_f64() / period
        })
        .collect();
    let width = MAX_DURATION_SHARE - MIN_DURATION_SHARE;
    assert_p_value(
        "duration shares",
        ks_one_sample(&mut shares, |x| {
            ((x - MIN_DURATION_SHARE) / width).clamp(0.0, 1.0)
        })
        .p_value,
        ALPHA,
    );
    let mut counts = [0_u64; 3];
    for storm in &listed {
        counts[match storm.band() {
            StormBand::Equatorial => 0,
            StormBand::MidLatitude => 1,
            StormBand::Polar => 2,
        }] += 1;
    }
    let total = f64::from(u32::try_from(listed.len()).unwrap());
    let expected = BAND_WEIGHTS.map(|weight| total * weight / BAND_TOTAL);
    assert_p_value(
        "latitude bands",
        chi_square_gof(&counts, &expected).p_value,
        ALPHA,
    );
}

/// `storms_active_at` gives the storms a scan of the listing finds running, at instants about
/// every onset and end and on a grid of a hundredth of a period: each storm runs from its onset to
/// the nanosecond before its end, and the grid, at a storm's 1% of the period on average, finds
/// about ten more. The answers are the same in any order of asking.
#[test]
fn the_storms_running_at_an_instant_match_a_scan() {
    let model = model();
    let listed = list(&model, years(-100, 400));
    let nanosecond = Span::new(0, 1).unwrap();
    let mut instants = Vec::new();
    for storm in list(&model, years(0, 300)) {
        for t in [storm.onset(), storm.end()] {
            instants.extend([t.checked_sub(nanosecond).unwrap(), t]);
        }
    }
    let step = SATURN_PERIOD.as_seconds_f64() / 100.0;
    for k in 0..1_000_i32 {
        let span = Span::from_seconds_f64(f64::from(k) * step).unwrap();
        instants.push(year(0).checked_add(span).unwrap());
    }
    let mut running = 0;
    assert_order_independent(&instants, |t| {
        let mut active = Vec::new();
        storms_active_at(&model, *t, &mut active);
        active
    });
    for t in instants {
        let mut active = Vec::new();
        storms_active_at(&model, t, &mut active);
        let scanned: Vec<Storm> = listed
            .iter()
            .copied()
            .filter(|storm| storm.is_active_at(t))
            .collect();
        assert_eq!(active, scanned, "at {t}");
        assert!(active.len() <= 1);
        running += active.len();
    }
    let ten = list(&model, years(0, 300)).len();
    assert!(running > 2 * ten, "{running} instants with a storm running");
}

/// Test (c): a window's listing is its pieces' in turn, whatever the cuts, and the same in any
/// order of asking; every storm comes back from its ID alone, whose text form round-trips.
#[test]
fn listings_are_independent_of_partition_and_order() {
    let model = model();
    let whole = years(-150, 250);
    let cuts = [
        year(-150),
        UniverseTime::new(-40 * YEAR - 17, 5).unwrap(),
        year(30),
        UniverseTime::new(30 * YEAR + 1, 0).unwrap(),
        year(250),
    ];
    assert_partition_independent(whole, &cuts, |w| list(&model, w));
    let pieces = partition(whole, &cuts[1..4]);
    assert_order_independent(&pieces, |w| list(&model, *w));
    let listed = list(&model, whole);
    assert!(listed.len() > 8, "{} storms", listed.len());
    for event in &listed {
        assert_eq!(storm(&model, event.id()).as_ref(), Some(event));
        assert_eq!(event.id().to_string().parse::<EventId>(), Ok(event.id()));
    }
}

/// Another body's, another tag's or a non-zero number's ID names no storm; one body's storms are
/// the same from two models, and two bodies' differ.
#[test]
fn a_storm_is_its_own_body_s() {
    let model = model();
    let first = list(&model, years(0, 100))[0];
    let bin = first.id().word().bin();
    assert_eq!(
        storm(&model, event_id(body(0x0105).into(), BODY_STORM, bin, 0)),
        None
    );
    assert_eq!(
        storm(
            &model,
            event_id(body(0x0106).into(), event_tags::BODY_DUSTSTORM, bin, 0)
        ),
        None
    );
    assert_eq!(
        storm(&model, event_id(body(0x0106).into(), BODY_STORM, bin, 1)),
        None
    );
    let any_cycle = event_id(
        body(0x0106).into(),
        BODY_STORM,
        EventBin::new(12_345).unwrap(),
        0,
    );
    assert_eq!(storm(&model, any_cycle).map(|s| s.cycle()), Some(12_345));
    let again = StormModel::new(SEED, body(0x0106), &saturn_storms()).unwrap();
    assert_eq!(list(&again, years(0, 300)), list(&model, years(0, 300)));
    let neighbour = StormModel::new(SEED, body(0x0105), &saturn_storms()).unwrap();
    assert_ne!(list(&neighbour, years(0, 300)), list(&model, years(0, 300)));
}
