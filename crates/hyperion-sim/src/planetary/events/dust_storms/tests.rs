//! Tests of `body.duststorm` (P14.T31.b): the refusal, the season and the noise against it, the
//! count over 10⁴ years against the skip's binomial (test (a, b)), the season of every onset at
//! three eccentricities (test (b)), the marks, the storms running at an instant, and test (c),
//! the listings' independence of partition and order.

use core::f64::consts::PI;

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof, ks_one_sample};

use super::super::tests::{
    MARS_ECCENTRICITY, MARS_PERIOD, SEED, YEAR, body, mars_dust_storms, origin, year, years,
};
use super::*;
use crate::events::testing::{assert_partition_independent, partition};
use crate::events::{EventBin, event_id};
use crate::id::event_tags;

fn model(e: f64) -> DustStormModel {
    DustStormModel::new(SEED, body(0x0104), &mars_dust_storms(e)).unwrap()
}

fn list(model: &DustStormModel, window: TimeWindow) -> Vec<DustStorm> {
    let mut out = Vec::new();
    dust_storms(model, window, &mut out);
    out
}

fn e(value: f64) -> Eccentricity {
    Eccentricity::new(value).unwrap()
}

#[test]
fn a_model_refuses_a_period_under_the_clock_s_minimum() {
    for seconds in [-1, 0, 1, 15] {
        let period = Span::from_seconds(seconds);
        let inputs = DustStormInputs {
            orbital_period: period,
            ..mars_dust_storms(MARS_ECCENTRICITY)
        };
        assert_eq!(
            DustStormModel::new(SEED, body(0x0104), &inputs),
            Err(BuildDustStormModelError::PeriodTooShort(period)),
            "{seconds} s"
        );
    }
    let shortest = DustStormInputs {
        orbital_period: LinearClock::MIN_PERIOD,
        ..mars_dust_storms(MARS_ECCENTRICITY)
    };
    assert!(DustStormModel::new(SEED, body(0x0104), &shortest).is_ok());
    assert_eq!(
        BuildDustStormModelError::PeriodTooShort(Span::from_seconds(15)).to_string(),
        "a dust storm period of +15.000000000 s is shorter than the +16.000000000 s minimum"
    );
}

/// The planets whose background dust is above 0 have dust storms: Mars's τ of 0.5, and any trace;
/// none for no dust, an Earth's 0 among them, or a NaN.
///
/// The τ are plain values, not `dust_optical_depth`'s: P14.T54.b's tripwire,
/// `nothing_on_the_record_calls_the_inventory_yet`, refuses code outside the aerosols module that
/// names it before the 24 bump, and `has_dust_storms`'s doctest reads `MARS_DUST_OPTICAL_DEPTH`
/// itself.
#[test]
fn the_dusty_planets_have_dust_storms() {
    // MARS_DUST_OPTICAL_DEPTH, Mars's background τ(550).
    let mars = 0.5;
    assert!(has_dust_storms(mars));
    assert!(has_dust_storms(1e-12));
    for none in [0.0, -0.0, f64::NAN] {
        assert!(!has_dust_storms(none), "{none}");
    }
}

/// h(e) and a(e) at the ruling's eccentricities (decision-p14-t31b-phases §1): h 0.25, 0.2203 and
/// 0.156; Mars's a 0.0821 and its bound 0.198 cycles; the cap 0.087 binding below e ≈ 0.05.
#[test]
fn the_season_and_the_noise_are_the_ruling_s() {
    assert!((season_half_width_cycles(e(0.0)) - 0.25).abs() < 1e-15);
    assert!((season_half_width_cycles(e(MARS_ECCENTRICITY)) - 0.2203).abs() < 5e-5);
    assert!((season_half_width_cycles(e(0.3)) - 0.156).abs() < 5e-4);
    let mars = model(MARS_ECCENTRICITY);
    assert_eq!(
        mars.noise().amplitude_cycles(),
        noise_amplitude_cycles(e(MARS_ECCENTRICITY))
    );
    assert!((mars.noise().amplitude_cycles() - 0.0821).abs() < 5e-5);
    assert!((mars.noise().noise_bound_cycles() - 0.198).abs() < 5e-4);
    assert_eq!(mars.noise().octaves(), NOISE_OCTAVES);
    assert_eq!(
        mars.noise().skip(),
        Some(SkipMark::from_probability(SKIP_PROBABILITY))
    );
    // The cap is returned as it is, so these compare bits.
    assert_eq!(noise_amplitude_cycles(e(0.0)), MAX_NOISE_AMPLITUDE_CYCLES);
    assert_eq!(noise_amplitude_cycles(e(0.045)), MAX_NOISE_AMPLITUDE_CYCLES);
    assert!(noise_amplitude_cycles(e(0.055)) < MAX_NOISE_AMPLITUDE_CYCLES);
    // The bound stays inside 0.9 h(e), and so inside the season, at every eccentricity.
    for k in 0..1_000_u32 {
        let value = f64::from(k) / 1_000.0;
        let bound = MonotonePhase::new(noise_amplitude_cycles(e(value)), 1, NOISE_OCTAVES)
            .unwrap()
            .noise_bound_cycles();
        let half_width = season_half_width_cycles(e(value));
        assert!(
            bound <= SEASON_MARGIN * half_width * (1.0 + 1e-15),
            "at e = {value}: {bound} against {half_width}"
        );
    }
    // At the largest eccentricity below 1, where h(e) is a difference of two nearly equal terms,
    // the season and the noise are not negative, and a model builds.
    let nearly_parabolic = e(1.0 - f64::EPSILON / 2.0);
    assert!(season_half_width_cycles(nearly_parabolic) >= 0.0);
    assert!(noise_amplitude_cycles(nearly_parabolic) >= 0.0);
    let inputs = DustStormInputs {
        eccentricity: nearly_parabolic,
        ..mars_dust_storms(MARS_ECCENTRICITY)
    };
    assert!(DustStormModel::new(SEED, body(0x0104), &inputs).is_ok());
    assert!((perihelion_contrast(e(MARS_ECCENTRICITY)) - 0.455).abs() < 5e-4);
    assert!((perihelion_contrast(e(0.0167)) - 0.069).abs() < 5e-4);
    assert_eq!(perihelion_contrast(e(0.0)), 0.0);
}

/// For Mars the season is Ls 160.9–340.9°, about 0.44 of the year: h(e) is the mean anomaly at
/// ν = ±90°, and the model's true anomaly is ±90° there.
#[test]
fn the_season_is_the_half_orbit_nearer_the_star() {
    let mars = model(MARS_ECCENTRICITY);
    let h = season_half_width_cycles(e(MARS_ECCENTRICITY));
    assert!((2.0 * h - 0.44).abs() < 0.005, "{}", 2.0 * h);
    let period = MARS_PERIOD.as_seconds_f64();
    for (share, nu) in [(h, PI / 2.0), (-h, -PI / 2.0), (0.0, 0.0)] {
        let t = origin()
            .checked_add(Span::from_seconds_f64(share * period).unwrap())
            .unwrap();
        let found = mars.true_anomaly_at(t).value();
        assert!((found - nu).abs() < 1e-6, "at {share} P: {found} rad");
    }
}

/// Test (a, b) for the dust storms: over 10⁴ years the cycles the phase crosses are K, and the
/// storm years among them are Binomial(K, 1/3) (decision-p14-t31b-phases §3: 1,772 ± 113 at
/// α = 10⁻³ for K = 5,317, against a Poisson interval's ± 138). Every listed storm's cycle is one
/// of the K, and every unskipped one of them is listed.
#[test]
fn the_storm_years_are_binomial_at_one_in_three() {
    let mars = model(MARS_ECCENTRICITY);
    let window = years(0, 10_000);
    let listed = list(&mars, window);
    let first = mars.phase_at(window.start()).cycles();
    let last = mars.phase_at(window.end()).cycles();
    let unskipped: Vec<i64> = ((first + 1)..=last)
        .filter(|&cycle| {
            mars.noise()
                .event(mars.series(), mars.clock(), cycle)
                .is_some()
        })
        .collect();
    assert_eq!(
        listed.iter().map(DustStorm::cycle).collect::<Vec<_>>(),
        unskipped
    );
    let cycles = u64::try_from(last - first).unwrap();
    assert!((5_316..=5_318).contains(&cycles), "{cycles} Mars years");
    let storms = u64::try_from(listed.len()).unwrap();
    let k = f64::from(u32::try_from(cycles).unwrap());
    assert_p_value(
        "Mars years with a global dust storm",
        chi_square_gof(&[storms, cycles - storms], &[k / 3.0, 2.0 * k / 3.0]).p_value,
        ALPHA,
    );
}

/// Test (b): over 10⁴ years every storm breaks out in the perihelion season, within 90° of true
/// anomaly of perihelion, at e = 0, at Mars's e and at e = 0.3, and its true anomaly is the
/// model's at its onset.
#[test]
fn every_storm_breaks_out_in_the_perihelion_season() {
    for value in [0.0, MARS_ECCENTRICITY, 0.3] {
        let model = model(value);
        let listed = list(&model, years(0, 10_000));
        assert!(
            listed.len() > 1_500,
            "{} storms at e = {value}",
            listed.len()
        );
        for storm in &listed {
            let nu = storm.onset_true_anomaly();
            assert_eq!(nu, model.true_anomaly_at(storm.onset()));
            assert!(
                nu.value().abs() <= SEASON_HALF_WIDTH.value(),
                "at e = {value}, {} broke out at {} rad",
                storm.id(),
                nu.value()
            );
        }
    }
}

/// The onsets' spread in true anomaly at Mars's e: a standard deviation of about 32°, a little
/// tighter than the ten catalogued storms' 45° (a deferred finding), centred on perihelion.
#[test]
fn the_onsets_spread_about_perihelion() {
    let listed = list(&model(MARS_ECCENTRICITY), years(0, 10_000));
    let degrees: Vec<f64> = listed
        .iter()
        .map(|storm| storm.onset_true_anomaly().value().to_degrees())
        .collect();
    let n = f64::from(u32::try_from(degrees.len()).unwrap());
    let mean = degrees.iter().sum::<f64>() / n;
    let sd = (degrees.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0)).sqrt();
    assert!(mean.abs() < 3.0, "mean onset at {mean}°");
    assert!((28.0..=36.0).contains(&sd), "onsets spread by {sd}°");
}

/// Each storm's duration share is uniform between its bounds, and its end is its onset plus it.
#[test]
fn the_durations_are_uniform_shares_of_the_period() {
    let listed = list(&model(MARS_ECCENTRICITY), years(0, 10_000));
    let period = MARS_PERIOD.as_seconds_f64();
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
}

/// `dust_storms_active_at` gives the storms a scan of the listing finds running, at instants about
/// every onset and end and on a grid of a fiftieth of a period, the same in any order of asking.
#[test]
fn the_storms_running_at_an_instant_match_a_scan() {
    let mars = model(MARS_ECCENTRICITY);
    let listed = list(&mars, years(-10, 110));
    let nanosecond = Span::new(0, 1).unwrap();
    let mut instants = Vec::new();
    for storm in list(&mars, years(0, 100)) {
        for t in [storm.onset(), storm.end()] {
            instants.extend([t.checked_sub(nanosecond).unwrap(), t]);
        }
    }
    let step = MARS_PERIOD.as_seconds_f64() / 50.0;
    for k in 0..2_500_i32 {
        let span = Span::from_seconds_f64(f64::from(k) * step).unwrap();
        instants.push(year(0).checked_add(span).unwrap());
    }
    let mut running = 0;
    assert_order_independent(&instants, |t| {
        let mut active = Vec::new();
        dust_storms_active_at(&mars, *t, &mut active);
        active
    });
    for t in instants {
        let mut active = Vec::new();
        dust_storms_active_at(&mars, t, &mut active);
        let scanned: Vec<DustStorm> = listed
            .iter()
            .copied()
            .filter(|storm| storm.is_active_at(t))
            .collect();
        assert_eq!(active, scanned, "at {t}");
        assert!(active.len() <= 1);
        running += active.len();
    }
    assert!(running > 100, "{running} instants with a storm running");
}

/// Test (c): a window's listing is its pieces' in turn, whatever the cuts, and the same in any
/// order of asking; every storm comes back from its ID alone, whose text form round-trips.
#[test]
fn listings_are_independent_of_partition_and_order() {
    let mars = model(MARS_ECCENTRICITY);
    let whole = years(-20, 30);
    let cuts = [
        year(-20),
        UniverseTime::new(-5 * YEAR - 17, 5).unwrap(),
        year(4),
        UniverseTime::new(4 * YEAR + 1, 0).unwrap(),
        year(30),
    ];
    assert_partition_independent(whole, &cuts, |w| list(&mars, w));
    let pieces = partition(whole, &cuts[1..4]);
    assert_order_independent(&pieces, |w| list(&mars, *w));
    let listed = list(&mars, whole);
    assert!(listed.len() > 4, "{} storms", listed.len());
    for event in &listed {
        assert_eq!(dust_storm(&mars, event.id()).as_ref(), Some(event));
        assert_eq!(event.id().to_string().parse::<EventId>(), Ok(event.id()));
    }
}

/// Another body's, another tag's, a non-zero number's or a skipped cycle's ID names no storm; one
/// body's storms are the same from two models, and two bodies' differ.
#[test]
fn a_dust_storm_is_its_own_body_s() {
    let mars = model(MARS_ECCENTRICITY);
    let listed = list(&mars, years(0, 100));
    let bin = listed[0].id().word().bin();
    assert_eq!(
        dust_storm(&mars, event_id(body(0x0103).into(), BODY_DUSTSTORM, bin, 0)),
        None
    );
    assert_eq!(
        dust_storm(
            &mars,
            event_id(body(0x0104).into(), event_tags::BODY_STORM, bin, 0)
        ),
        None
    );
    assert_eq!(
        dust_storm(&mars, event_id(body(0x0104).into(), BODY_DUSTSTORM, bin, 1)),
        None
    );
    let cycles: Vec<i64> = listed.iter().map(DustStorm::cycle).collect();
    let skipped = (cycles[0]..=cycles[cycles.len() - 1])
        .find(|c| !cycles.contains(c))
        .unwrap();
    let skipped = event_id(
        body(0x0104).into(),
        BODY_DUSTSTORM,
        EventBin::new(skipped).unwrap(),
        0,
    );
    assert_eq!(dust_storm(&mars, skipped), None);
    let again =
        DustStormModel::new(SEED, body(0x0104), &mars_dust_storms(MARS_ECCENTRICITY)).unwrap();
    assert_eq!(list(&again, years(0, 100)), listed);
    let neighbour =
        DustStormModel::new(SEED, body(0x0103), &mars_dust_storms(MARS_ECCENTRICITY)).unwrap();
    assert_ne!(list(&neighbour, years(0, 100)), listed);
}
