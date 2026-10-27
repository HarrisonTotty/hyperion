//! Tests of `body.impact` (P14.T31.a): the law and its anchors, the Solar System's Earth, Moon and
//! Jupiter (decision-p14-t31a-events's tests 1–4), the floor, the counts against the rate, the
//! marks, and the listings' independence of partition and order.

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, ks_one_sample};

use super::super::tests::{
    EARTH, JUPITER, JUPITER_RADIUS, MOON, SEED, YEAR, body, earth_inputs, giant_inputs,
    moon_impactors, planet_impactors, radius_of, year, years,
};
use super::*;
use crate::events::testing::{assert_partition_independent, partition};
use crate::events::{EventBin, event_id};
use crate::id::event_tags;
use crate::planetary::hooks::figures::ICY_PROJECTILE_DENSITY;
use crate::planetary::testing::with_every_24_hold;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

fn list(model: &ImpactModel, window: TimeWindow) -> Vec<Impact> {
    let mut out = Vec::new();
    impacts(model, window, &mut out);
    out
}

fn model(inputs: &ImpactInputs) -> ImpactModel {
    ImpactModel::new(SEED, body(0x0105), inputs).unwrap()
}

/// The mean interval between impacts at `rate`, Julian years.
fn interval(rate: EventsPerSecond) -> f64 {
    1.0 / (rate.value() * SECONDS_PER_JULIAN_YEAR)
}

/// One megaton of TNT, J.
const MEGATON: f64 = 4.184e15;

#[test]
fn a_model_refuses_its_inputs_out_of_range() {
    let good = giant_inputs();
    let refuse = |inputs: ImpactInputs| ImpactModel::new(SEED, body(1), &inputs).unwrap_err();
    for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let inputs = ImpactInputs {
            radius: Metres::new(radius),
            ..good
        };
        assert!(
            matches!(refuse(inputs), BuildImpactModelError::Radius(_)),
            "{radius}"
        );
    }
    for rate in [-1e-9, f64::NAN, f64::INFINITY] {
        let inputs = ImpactInputs {
            rate_over_moon: rate,
            ..good
        };
        assert!(
            matches!(refuse(inputs), BuildImpactModelError::RateOverMoon(_)),
            "{rate}"
        );
    }
    for v in [0.0, f64::NAN] {
        let inputs = ImpactInputs {
            velocity: MetresPerSecond::new(v),
            ..good
        };
        assert!(
            matches!(refuse(inputs), BuildImpactModelError::Velocity(_)),
            "{v}"
        );
    }
    let inputs = ImpactInputs {
        projectile_density: KilogramsPerCubicMetre::new(0.0),
        ..good
    };
    assert!(matches!(
        refuse(inputs),
        BuildImpactModelError::ProjectileDensity(_)
    ));
    for age in [-0.1, f64::NAN] {
        let inputs = ImpactInputs {
            system_age: Gigayears::new(age),
            ..good
        };
        assert!(
            matches!(refuse(inputs), BuildImpactModelError::SystemAge(_)),
            "{age}"
        );
    }
    assert_eq!(
        BuildImpactModelError::Radius(Metres::new(-1.0)).to_string(),
        "radius -1 m is not finite and positive"
    );
}

/// The ruling's test 4: B is 1 at the Solar System's age and 1 ± 10⁻³ at 10 Gyr, within 1% of the
/// chronology's derivative at 0.01, 0.1, 1 and 2 Gyr, and never rises with age.
#[test]
fn the_bombardment_is_the_chronology_s_rate_and_falls_with_age() {
    assert!((bombardment(SOLAR_SYSTEM_AGE) - 1.0).abs() < 1e-15);
    assert!((bombardment(Gigayears::new(10.0)) - 1.0).abs() < 1e-3);
    // dN/dS at S = 0 of Neukum, Ivanov and Hartmann's curve counted from the system's formation.
    let derivative = |age: f64| 5.44e-14 * 6.93 * math::exp(6.93 * (4.567 - age)) + 8.38e-4;
    for age in [0.01, 0.1, 1.0, 2.0] {
        let expected = derivative(age) / derivative(4.567);
        let b = bombardment(Gigayears::new(age));
        assert!(
            (b / expected - 1.0).abs() < 0.01,
            "B({age}) = {b} against {expected}"
        );
    }
    let mut last = f64::INFINITY;
    for age in [0.0, 0.01, 0.1, 0.5, 1.0, 2.0, 4.0, 4.567, 5.0, 8.0, 13.0] {
        let b = bombardment(Gigayears::new(age));
        assert!(b.is_finite() && b <= last, "B({age}) = {b} after {last}");
        last = b;
    }
}

/// The ruling's test 1, on its plain Earth: above 1 Mt once in 30–1,000 years (133.5 expected),
/// above 10 Mt once in 300–3,000 (751), and above 1 km once in 0.6 Myr to 1%, the anchor.
#[test]
fn an_earth_is_struck_as_the_ruling_has_it() {
    let earth = model(&earth_inputs(4.567));
    let megaton = interval(earth.rate_above_joules(MEGATON));
    assert!(
        (30.0..1_000.0).contains(&megaton),
        "1 Mt every {megaton} years"
    );
    assert!(
        (megaton / 133.5 - 1.0).abs() < 0.02,
        "1 Mt every {megaton} years"
    );
    let ten = interval(earth.rate_above_joules(10.0 * MEGATON));
    assert!((300.0..3_000.0).contains(&ten), "10 Mt every {ten} years");
    let kilometre = interval(earth.rate_above(ROCKY_REFERENCE_DIAMETER));
    assert!(
        (kilometre / 0.6e6 - 1.0).abs() < 0.01,
        "1 km every {kilometre} years"
    );
    assert_eq!(
        earth.rate_above(ROCKY_REFERENCE_DIAMETER),
        earth.reference_rate()
    );
    // A 1 km stony body at 19.87 km/s brings some 3.1 × 10²⁰ J, about 74,000 Mt; the megaton's
    // diameter is 23.8 m.
    let joules = earth.energy_of(ROCKY_REFERENCE_DIAMETER);
    assert!((3.0e20..3.2e20).contains(&joules), "{joules} J");
    assert!((earth.diameter_of(MEGATON).value() - 23.8).abs() < 0.1);
    for d in [3.0, 24.0, 1_000.0, 2e5] {
        let back = earth.diameter_of(earth.energy_of(Metres::new(d))).value();
        assert!(
            (back / d - 1.0).abs() < 1e-14,
            "{d} m came back as {back} m"
        );
    }
}

/// The ruling's tests 1–3 on the Solar System of P14.T48.f's tests, under the rules at 23 and with
/// every hold at 24 in force: Earth above 1 Mt once in 30–1,000 years; the Moon once in
/// 1,000–10,000; Jupiter, struck by the giants' comets, meets a Shoemaker–Levy 9 (a comet over
/// 1.5 km; its parent's mass gives 1.5 km at 500–600 kg m⁻³) once in 50–500 years, the plan's own
/// bound, and 10–65 impacts a year above 1 Mt (Hueso et al. 2018, A&A 617, A68: 10–65 a year of
/// Chelyabinsk class). Over 10⁴ years the Shoemaker–Levy-class count matches its rate.
#[test]
fn the_solar_system_s_bodies_are_struck_as_observed() {
    let check = || {
        let earth = planet_impactors(EARTH);
        let earth = ImpactInputs::new(radius_of(EARTH), &earth, SOLAR_SYSTEM_AGE);
        assert_eq!(earth.size_law, rocky_projectiles());
        let earth = ImpactModel::new(SEED, body(0x0103), &earth).unwrap();
        let earth_megaton = interval(earth.rate_above_joules(MEGATON));
        assert!(
            (30.0..1_000.0).contains(&earth_megaton),
            "Earth: 1 Mt every {earth_megaton} yr"
        );

        let moon = moon_impactors();
        let moon = ImpactInputs::new(Metres::new(MOON.2 * 1e3), &moon, SOLAR_SYSTEM_AGE);
        let moon = ImpactModel::new(SEED, body(0x0113), &moon).unwrap();
        let moon_megaton = interval(moon.rate_above_joules(MEGATON));
        assert!(
            (1_000.0..10_000.0).contains(&moon_megaton),
            "the Moon: 1 Mt every {moon_megaton} yr"
        );

        let jupiter = planet_impactors(JUPITER);
        assert!(matches!(jupiter.source(), ImpactorSource::Comets));
        assert_eq!(jupiter.projectile_density(), ICY_PROJECTILE_DENSITY);
        let inputs = ImpactInputs::new(JUPITER_RADIUS, &jupiter, SOLAR_SYSTEM_AGE);
        assert_eq!(inputs.size_law, icy_projectiles());
        let jupiter = ImpactModel::new(SEED, body(0x0105), &inputs).unwrap();
        let sl9 = Metres::new(1_500.0);
        let sl9_interval = interval(jupiter.rate_above(sl9));
        assert!(
            (50.0..500.0).contains(&sl9_interval),
            "SL9 every {sl9_interval} yr"
        );
        let per_year = 1.0 / interval(jupiter.rate_above_joules(MEGATON));
        assert!(
            (10.0..65.0).contains(&per_year),
            "Jupiter: {per_year} a year above 1 Mt"
        );
        let window = years(-5_000, 5_000);
        let count = list(&jupiter, window)
            .iter()
            .filter(|impact| impact.diameter() >= sl9)
            .count();
        let expected = jupiter.rate_above(sl9).value() * 1e4 * SECONDS_PER_JULIAN_YEAR;
        assert_poisson_count("SL9-class", u64::try_from(count).unwrap(), expected, ALPHA);
        (earth_megaton, moon_megaton, sl9_interval)
    };
    let at_23 = check();
    let at_24 = with_every_24_hold(check);
    assert!(at_23.0.is_finite() && at_24.0.is_finite());
}

/// An Earth lists its impacts from the stated megaton up; a giant 100 Myr old, struck some 10⁴
/// times as often, lists only its largest, at the bins' limit; and the floor removes small impacts
/// without moving the large ones' rate.
#[test]
fn the_floor_is_the_stated_energy_unless_the_bins_would_overflow() {
    let now = model(&earth_inputs(4.567));
    assert!((now.floor_joules() / FIXED_FLOOR_JOULES - 1.0).abs() < 1e-12);
    assert!(now.listed_rate().value() * BIN_SECONDS_F64 < PoissonBins::MAX_MEAN_PER_BIN);
    let giant = model(&giant_inputs());
    assert!((giant.floor_joules() / FIXED_FLOOR_JOULES - 1.0).abs() < 1e-12);
    let young = model(&ImpactInputs {
        system_age: Gigayears::new(0.1),
        ..giant_inputs()
    });
    assert!(young.floor_joules() > 10.0 * FIXED_FLOOR_JOULES);
    let mean = young.listed_rate().value() * BIN_SECONDS_F64;
    assert!(
        (mean / PoissonBins::MAX_MEAN_PER_BIN - 1.0).abs() < 1e-9,
        "the raised floor holds a bin at the limit, {mean}"
    );
    let large = Metres::new(10e3);
    let ratio = young.rate_above(large).value() / giant.rate_above(large).value();
    assert!((ratio / young.bombardment() - 1.0).abs() < 1e-12);
    for impact in list(&young, years(0, 3)) {
        assert!(impact.energy_joules() >= young.floor_joules() * (1.0 - 1e-12));
    }
}

/// No impacts with no impactors, nor where the stated energy needs a projectile beyond the law's
/// largest.
#[test]
fn a_body_with_nothing_above_its_floor_lists_nothing() {
    let none = model(&ImpactInputs {
        rate_over_moon: 0.0,
        ..giant_inputs()
    });
    assert_eq!(none.listed_rate(), EventsPerSecond::ZERO);
    assert_eq!(list(&none, years(0, 100)), Vec::<Impact>::new());
    // At 1 cm/s the megaton needs a comet of some 680 km, beyond the law's 150 km.
    let slow = model(&ImpactInputs {
        velocity: MetresPerSecond::new(0.01),
        ..giant_inputs()
    });
    assert_eq!(slow.listed_rate(), EventsPerSecond::ZERO);
    assert_eq!(slow.floor_diameter(), icy_projectiles().largest());
    assert_eq!(list(&slow, years(0, 100)), Vec::<Impact>::new());
}

/// Test (a): over 10⁴ years the counts match the rate, a giant's and an Earth's.
#[test]
fn counts_over_ten_thousand_years_match_the_rate() {
    for (name, inputs) in [("giant", giant_inputs()), ("earth", earth_inputs(4.567))] {
        let model = model(&inputs);
        let window = years(-5_000, 5_000);
        let count = u64::try_from(list(&model, window).len()).unwrap();
        assert_poisson_count(name, count, model.expected_count(window), ALPHA);
    }
}

/// Every impact brings at least the floor's energy, its diameter follows the size law above the
/// floor, and its velocity is the body's.
#[test]
fn the_marks_follow_the_size_law_above_the_floor() {
    let model = model(&giant_inputs());
    let listed = list(&model, years(0, 200));
    assert!(listed.len() > 2_000, "{} impacts", listed.len());
    let law = giant_inputs().size_law;
    let floor = model.floor_diameter();
    let mut cdf_values: Vec<f64> = listed
        .iter()
        .map(|impact| {
            assert!(impact.energy_joules() >= model.floor_joules() * (1.0 - 1e-12));
            assert!(impact.diameter() >= floor && impact.diameter() <= law.largest());
            assert_eq!(impact.velocity(), giant_inputs().velocity);
            1.0 - law.share_above(impact.diameter()) / law.share_above(floor)
        })
        .collect();
    assert_p_value(
        "diameters against the size law",
        ks_one_sample(&mut cdf_values, |x| x.clamp(0.0, 1.0)).p_value,
        ALPHA,
    );
}

/// Test (c): a window's listing is its pieces' in turn, whatever the cuts, and the same in any
/// order of asking; every impact comes back from its ID alone, whose text form round-trips.
#[test]
fn listings_are_independent_of_partition_and_order() {
    let model = model(&giant_inputs());
    let whole = years(-3, 7);
    let cuts = [
        year(-3),
        UniverseTime::new(-YEAR - 17, 5).unwrap(),
        year(2),
        UniverseTime::new(2 * YEAR + 1, 0).unwrap(),
        year(7),
    ];
    assert_partition_independent(whole, &cuts, |w| list(&model, w));
    let pieces = partition(whole, &cuts[1..4]);
    assert_order_independent(&pieces, |w| list(&model, *w));
    let listed = list(&model, whole);
    assert_ne!(listed, Vec::<Impact>::new());
    for event in &listed {
        assert_eq!(impact(&model, event.id()).as_ref(), Some(event));
        assert_eq!(event.id().to_string().parse::<EventId>(), Ok(event.id()));
    }
}

/// Another body's, another tag's or a missing number's ID names no impact; one body's impacts are
/// the same from two models, and two bodies' differ.
#[test]
fn an_impact_is_its_own_body_s() {
    let model = model(&giant_inputs());
    let first = list(&model, years(0, 5))[0];
    let word = first.id().word();
    let other = event_id(body(0x0104).into(), BODY_IMPACT, word.bin(), word.number());
    assert_eq!(impact(&model, other), None);
    let tag = event_id(
        body(0x0105).into(),
        event_tags::BODY_ERUPTION,
        word.bin(),
        0,
    );
    assert_eq!(impact(&model, tag), None);
    let beyond = event_id(
        body(0x0105).into(),
        BODY_IMPACT,
        EventBin::new(0).unwrap(),
        255,
    );
    assert_eq!(impact(&model, beyond), None);
    let again = ImpactModel::new(SEED, body(0x0105), &giant_inputs()).unwrap();
    assert_eq!(list(&again, years(0, 5)), list(&model, years(0, 5)));
    let neighbour = ImpactModel::new(SEED, body(0x0104), &giant_inputs()).unwrap();
    assert_ne!(list(&neighbour, years(0, 5)), list(&model, years(0, 5)));
}

/// The size law follows the projectiles' composition: rocky from a rocky belt, icy from an icy
/// belt or the comets.
#[test]
fn the_size_law_follows_the_projectiles_composition() {
    use crate::planetary::belts::BeltComposition;
    let belt = |composition| ImpactorSource::InBelt {
        excitation: 0.06,
        composition,
    };
    let near = |composition| ImpactorSource::NearestBelt {
        excitation: 0.06,
        composition,
    };
    assert_eq!(
        size_law_of(belt(BeltComposition::Rocky)),
        rocky_projectiles()
    );
    assert_eq!(
        size_law_of(near(BeltComposition::Rocky)),
        rocky_projectiles()
    );
    assert_eq!(size_law_of(belt(BeltComposition::Icy)), icy_projectiles());
    assert_eq!(size_law_of(near(BeltComposition::Icy)), icy_projectiles());
    assert_eq!(size_law_of(ImpactorSource::Comets), icy_projectiles());
    assert_eq!(
        size_law_of(ImpactorSource::InnerComets { excitation: 0.1 }),
        icy_projectiles()
    );
}
