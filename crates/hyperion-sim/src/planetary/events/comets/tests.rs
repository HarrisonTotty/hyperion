//! Tests of `system.comet` (P14.T31.a): the Solar System's rate inside 5 au, the marks'
//! distributions, the orbits at perihelion and at the listing radius, the reach, plunging comets,
//! the floor, and the listings' independence of partition and order.

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, ks_one_sample};

use super::super::tests::{SEED, YEAR, solar_comets, system, year, years};
use super::*;
use crate::events::testing::{assert_partition_independent, partition};
use crate::events::{EventBin, event_id};
use crate::id::event_tags;
use crate::orbit::NEAR_PARABOLIC_BAND;
use crate::planetary::halo::{HaloHost, Scatterer, halo};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{AstronomicalUnits, EarthMasses, SolarMasses};

fn model(inputs: &CometInputs) -> CometModel {
    CometModel::new(SEED, system(), inputs).unwrap()
}

fn list(model: &CometModel, window: TimeWindow) -> Vec<Comet> {
    let mut out = Vec::new();
    comets(model, window, &mut out);
    out
}

fn au(metres: f64) -> f64 {
    metres / METRES_PER_AU
}

#[test]
fn a_model_refuses_its_inputs_out_of_range() {
    let good = solar_comets();
    let refuse = |inputs: CometInputs| CometModel::new(SEED, system(), &inputs).unwrap_err();
    for rate in [-1.0, f64::NAN, f64::INFINITY] {
        let inputs = CometInputs {
            rate: EventsPerSecond::new(rate),
            ..good
        };
        assert!(
            matches!(refuse(inputs), BuildCometModelError::Rate(_)),
            "{rate}"
        );
    }
    for l in [0.0, -1.0, f64::NAN] {
        let inputs = CometInputs {
            luminosity: SolarLuminosities::new(l),
            ..good
        };
        assert!(
            matches!(refuse(inputs), BuildCometModelError::Luminosity(_)),
            "{l}"
        );
    }
    let inputs = CometInputs {
        host: GravitationalParameter::new(0.0),
        ..good
    };
    assert!(matches!(
        refuse(inputs),
        BuildCometModelError::HostParameter(_)
    ));
    let inputs = CometInputs {
        host_radius: Metres::new(-1.0),
        ..good
    };
    assert!(matches!(
        refuse(inputs),
        BuildCometModelError::HostRadius(_)
    ));
    let inputs = CometInputs {
        host: GravitationalParameter::new(1e-300),
        ..good
    };
    assert!(matches!(refuse(inputs), BuildCometModelError::Reach { .. }));
}

/// Test (a): the Solar System's halo of 7.5 × 10¹¹ comets (P14.T21.d's rate, ruling 84.5) sends
/// 5–20 comets a year inside 5 au, and the counts over 10⁴ years match the rate.
#[test]
fn the_solar_system_sends_five_to_twenty_comets_a_year_inside_five_au() {
    let strip = Metres::from(AstronomicalUnits::new(1.39e5));
    let sun = HaloHost::new(SolarMasses::new(1.0), EarthMasses::new(32.2), strip, None);
    let halo = halo(SEED, system(), &sun, Scatterer::Present).unwrap();
    let rate = halo.comet_rate(SolarLuminosities::new(1.0)).value() * 7.5e11 / halo.comets();
    let inputs = CometInputs {
        rate: EventsPerSecond::new(rate),
        ..solar_comets()
    };
    let model = model(&inputs);
    let per_year = model.listed_rate().value() * SECONDS_PER_JULIAN_YEAR;
    assert!((5.0..20.0).contains(&per_year), "{per_year} a year");
    assert_eq!(model.floor_diameter(), NUCLEUS_REFERENCE_DIAMETER);
    let window = years(-5_000, 5_000);
    let listed = list(&model, window);
    assert!(listed.iter().all(|c| au(c.perihelion().value()) <= 5.0));
    let count = u64::try_from(listed.len()).unwrap();
    assert_poisson_count(
        "comets inside 5 au",
        count,
        model.expected_count(window),
        ALPHA,
    );
}

/// The marks: perihelia uniform out to 5 au, 1 − e uniform below its bound, orientations
/// isotropic, and nuclei on the size law from 1 km to 150 km.
#[test]
fn the_marks_follow_their_laws() {
    let model = model(&solar_comets());
    let listed = list(&model, years(0, 600));
    assert!(listed.len() > 5_000, "{}", listed.len());
    let limit = model.perihelion_limit().value();
    let a_min = model.min_semi_major_axis().value();
    let law = nuclei();
    let mut columns: [Vec<f64>; 6] = Default::default();
    for comet in &listed {
        let e = comet.eccentricity();
        assert!(e > MIN_ECCENTRICITY && e <= 1.0, "{e}");
        let o = comet.orientation();
        columns[0].push(comet.perihelion().value() / limit);
        let span = ECCENTRICITY_SPAN.min(comet.perihelion().value() / a_min);
        columns[1].push((1.0 - e) / span);
        columns[2].push((1.0 - math::cos(o.inclination().value())) / 2.0);
        columns[3].push(o.ascending_node().value() / TAU);
        columns[4].push(o.argument_of_periapsis().value() / TAU);
        columns[5].push(1.0 - law.share_above(comet.nucleus_diameter()));
    }
    for (name, column) in [
        "perihelion",
        "eccentricity",
        "cosine of the inclination",
        "node",
        "periapsis",
        "nucleus",
    ]
    .into_iter()
    .zip(columns.iter_mut())
    {
        assert_p_value(
            name,
            ks_one_sample(column, |x| x.clamp(0.0, 1.0)).p_value,
            ALPHA,
        );
    }
}

/// At its perihelion time a comet is at its perihelion distance, moving across the radius; and at
/// the ends of its listing it is at the listing radius, inside its aphelion. Comets below the open
/// orbits' floor are `Orbit::Bound`.
#[test]
fn comets_are_at_perihelion_on_time_and_listed_inside_the_radius() {
    let model = model(&solar_comets());
    let radius = model.listing_radius().value();
    let (mut open, mut bound) = (0, 0);
    for comet in list(&model, years(0, 200)).iter().filter(|c| !c.plunges()) {
        match comet.orbit() {
            Orbit::Open(_) => {
                open += 1;
                assert!(comet.eccentricity() >= OpenOrbit::MIN_ECCENTRICITY);
            }
            Orbit::Bound(_) => {
                bound += 1;
                assert!(comet.eccentricity() < OpenOrbit::MIN_ECCENTRICITY);
            }
        }
        let q = comet.perihelion().value();
        let (r, v) = comet.relative_state_at(comet.perihelion_time());
        let r_m = r.metres();
        assert!(
            (r.length().value() / q - 1.0).abs() < 1e-7,
            "{} at perihelion",
            comet.id()
        );
        let radial = (r_m[0] * v.metres_per_second()[0]
            + r_m[1] * v.metres_per_second()[1]
            + r_m[2] * v.metres_per_second()[2])
            / (r.length().value() * v.speed().value());
        assert!(
            radial.abs() < 1e-6,
            "{} moves across the radius at perihelion",
            comet.id()
        );
        let aphelion = q * (1.0 + comet.eccentricity()) / (1.0 - comet.eccentricity());
        assert!(
            aphelion > radius,
            "{} turns back inside the radius",
            comet.id()
        );
        for t in [comet.listed().start(), comet.listed().end()] {
            let at = comet.position_at(t).length().value();
            assert!(
                (at / radius - 1.0).abs() < 1e-5,
                "{} at {t}: {} au",
                comet.id(),
                au(at)
            );
        }
        assert!(comet.listed().contains(comet.perihelion_time()));
    }
    assert!(open > 0 && bound > 20 * open, "{open} open, {bound} bound");
}

/// No comet stays listed longer than the model's reach either side of its perihelion, which the
/// slowest orbits come close to.
#[test]
fn no_comet_outstays_the_reach() {
    let model = model(&solar_comets());
    let reach = to_nanos(UniverseTime::EPOCH.checked_add(model.reach()).unwrap());
    let mut longest = 0;
    for comet in list(&model, years(0, 2_000)) {
        let at = to_nanos(comet.perihelion_time());
        let before = at - to_nanos(comet.listed().start());
        let after = to_nanos(comet.listed().end()) - at;
        assert!(before <= reach && after <= reach, "{}", comet.id());
        longest = longest.max(before);
    }
    // The Sun's reach is about 15.2 years (decision-p14-t31a-events); the slowest drawn come close.
    let years_f = |nanos: i128| {
        #[expect(clippy::cast_precision_loss, reason = "a test's display of decades")]
        let x = nanos as f64 / 1e9 / SECONDS_PER_JULIAN_YEAR;
        x
    };
    assert!(
        (15.0..15.4).contains(&years_f(reach)),
        "reach {}",
        years_f(reach)
    );
    assert!(years_f(longest) > 14.0, "longest {}", years_f(longest));
}

/// A comet whose perihelion lies inside its host is listed until it reaches the host's surface.
#[test]
fn a_comet_that_plunges_ends_at_its_host_s_surface() {
    let giant = CometInputs {
        host_radius: Metres::new(0.5 * METRES_PER_AU),
        ..solar_comets()
    };
    let model = model(&giant);
    let listed = list(&model, years(0, 100));
    let plungers: Vec<&Comet> = listed.iter().filter(|c| c.plunges()).collect();
    assert!(plungers.len() > 30, "{}", plungers.len());
    for comet in plungers {
        assert!(comet.perihelion() <= giant.host_radius);
        assert!(comet.listed().end() <= comet.perihelion_time());
        let at = comet.position_at(comet.listed().end()).length().value();
        assert!(
            (au(at) / 0.5 - 1.0).abs() < 1e-5,
            "{}: {} au",
            comet.id(),
            au(at)
        );
    }
}

/// A great halo's comets are listed above a raised floor, at the tighter of the two limits (here
/// the listing's, 1,024 at once, about 34 a year about the Sun), and the rate of its large comets
/// is as the size law has it.
#[test]
fn a_great_halo_raises_the_floor() {
    let busy = CometInputs {
        rate: EventsPerSecond::per_julian_year(5_000.0),
        ..solar_comets()
    };
    let model = model(&busy);
    assert!(model.floor_diameter() > NUCLEUS_REFERENCE_DIAMETER);
    let at_once = model.listed_rate().value() * 2.0 * model.reach().as_seconds_f64();
    assert!(
        (at_once / MAX_LISTED_COMETS - 1.0).abs() < 1e-9,
        "{at_once}"
    );
    assert!(model.listed_rate().value() * BIN_SECONDS_F64 < PoissonBins::MAX_MEAN_PER_BIN);
    let large = Metres::new(20e3);
    let expected = 5_000.0 * nuclei().share_above(large);
    let per_year = model.rate_above(large).value() * SECONDS_PER_JULIAN_YEAR;
    assert!((per_year / expected - 1.0).abs() < 1e-12);
    assert!(
        list(&model, years(0, 10))
            .iter()
            .all(|c| c.nucleus_diameter() >= model.floor_diameter())
    );
}

/// The comets listed in a window or at an instant are exactly the long listing's comets inside the
/// radius then.
#[test]
fn the_comets_listed_are_the_long_listing_s_inside_the_radius() {
    let model = model(&solar_comets());
    let long = list(&model, years(-40, 45));
    for t in [
        UniverseTime::EPOCH,
        UniverseTime::new(3 * YEAR + 12_345, 6).unwrap(),
    ] {
        let mut listed = Vec::new();
        comets_listed_at(&model, t, &mut listed);
        let expected: Vec<Comet> = long.iter().filter(|c| c.is_listed_at(t)).copied().collect();
        assert_eq!(listed, expected, "at {t}");
        // The ruling's test 5: some 300 at once about the Sun.
        assert!(
            (200..400).contains(&listed.len()),
            "{} at {t}",
            listed.len()
        );
        for comet in &listed {
            let r = comet.position_at(t).length().value();
            assert!(r <= model.listing_radius().value() * (1.0 + 1e-6));
        }
    }
    let window = years(1, 3);
    let mut listed = Vec::new();
    comets_listed_in(&model, window, &mut listed);
    let expected: Vec<Comet> = long
        .iter()
        .filter(|c| c.listed().start() < window.end() && window.start() < c.listed().end())
        .copied()
        .collect();
    assert_eq!(listed, expected);
    let mut none = Vec::new();
    comets_listed_in(
        &model,
        TimeWindow::new(year(1), year(1)).unwrap(),
        &mut none,
    );
    assert_eq!(none, Vec::<Comet>::new());
}

/// Test (c): a window's listing is its pieces' in turn, and the same in any order of asking; every
/// comet comes back from its ID alone, whose text form round-trips; another system's or another
/// tag's ID names none.
#[test]
fn listings_are_independent_of_partition_and_order() {
    let model = model(&solar_comets());
    let whole = years(-2, 4);
    let cuts = [
        UniverseTime::new(-YEAR + 3, 0).unwrap(),
        year(0),
        year(0),
        year(3),
    ];
    assert_partition_independent(whole, &cuts, |w| list(&model, w));
    assert_order_independent(&partition(whole, &cuts), |w| list(&model, *w));
    let instants = [year(-1), UniverseTime::EPOCH, year(5)];
    assert_order_independent(&instants, |t| {
        let mut out = Vec::new();
        comets_listed_at(&model, *t, &mut out);
        out
    });
    let listed = list(&model, whole);
    for event in &listed {
        assert_eq!(comet(&model, event.id()), Some(*event));
        assert_eq!(event.id().to_string().parse::<EventId>(), Ok(event.id()));
    }
    let word = listed[0].id().word();
    let elsewhere = SystemId::from_raw(0x0200_0800_2000_0001).unwrap();
    let other = event_id(elsewhere.into(), SYSTEM_COMET, word.bin(), word.number());
    assert_eq!(comet(&model, other), None);
    let tag = event_id(
        system().into(),
        event_tags::BODY_IMPACT,
        word.bin(),
        word.number(),
    );
    assert_eq!(comet(&model, tag), None);
    // The same seed gives the same comets from a second model, and another system others.
    let again = CometModel::new(SEED, system(), &solar_comets()).unwrap();
    assert_eq!(list(&again, whole), listed);
    let neighbour = CometModel::new(SEED, elsewhere, &solar_comets()).unwrap();
    assert_ne!(list(&neighbour, whole), listed);
    let beyond = event_id(
        system().into(),
        SYSTEM_COMET,
        EventBin::new(0).unwrap(),
        255,
    );
    assert_eq!(comet(&model, beyond), None);
}

/// The time out to the radius runs continuously into Barker's at e = 1, agrees with the orbit's
/// own distance on both sides of the near-parabolic band, and is about 15 years for a parabola of
/// q = 5 au about the Sun.
#[test]
fn the_time_to_the_radius_is_continuous_at_the_parabola() {
    let mu = solar_comets().host;
    let r = 30.0 * METRES_PER_AU;
    let orientation =
        Orientation::new(Radians::new(0.3), Radians::new(1.0), Radians::new(2.0)).unwrap();
    for q_au in [1e-4, 0.01, 1.0, 5.0] {
        let q = q_au * METRES_PER_AU;
        let parabola = time_from_perihelion(q, 1.0, mu.value(), r);
        let near = time_from_perihelion(q, 1.0 - 1e-14, mu.value(), r);
        assert!(
            (near / parabola - 1.0).abs() < 1e-8,
            "{q_au} au: {parabola} s, {near} s"
        );
        for e in [
            1.0 - 0.5 * NEAR_PARABOLIC_BAND,
            1.0 - 2.0 * NEAR_PARABOLIC_BAND,
            0.995,
        ] {
            if q * (1.0 + e) / (1.0 - e) <= r {
                continue;
            }
            let t = time_from_perihelion(q, e, mu.value(), r);
            let at = UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64(t).unwrap())
                .unwrap();
            let orbit = orbit_of(q, e, orientation, UniverseTime::EPOCH, mu);
            let distance = orbit.relative_state_at(at).0.length().value();
            assert!(
                (distance / r - 1.0).abs() < 1e-7,
                "{q_au} au, e {e}: {} au",
                au(distance)
            );
        }
    }
    let parabola =
        time_from_perihelion(5.0 * METRES_PER_AU, 1.0, mu.value(), r) / SECONDS_PER_JULIAN_YEAR;
    assert!((14.9..15.1).contains(&parabola), "{parabola} years");
    assert_eq!(
        time_from_perihelion(METRES_PER_AU, 0.995, mu.value(), 0.5 * METRES_PER_AU),
        0.0
    );
}

/// The ruling's amendment (c): whatever the perihelion, down to 10⁻⁶ of its limit (deep inside the
/// host, which a comet plunges into below about 10⁻³), and whatever
/// the eccentricity's uniform, a comet's period is at least 200 years and its aphelion lies beyond
/// the listing radius, for the Sun and for hosts of 10, 100 and 10⁴ L☉.
#[test]
fn no_comet_is_short_period_or_turns_back_inside_the_radius() {
    for (l, m) in [(1.0, 1.0), (10.0, 1.6), (100.0, 2.5), (1e4, 1.0)] {
        let inputs = CometInputs {
            luminosity: SolarLuminosities::new(l),
            host: GravitationalParameter::from_solar_masses(SolarMasses::new(m)),
            ..solar_comets()
        };
        let model = model(&inputs);
        let (limit, a_min) = (
            model.perihelion_limit().value(),
            model.min_semi_major_axis().value(),
        );
        let mu = inputs.host.value();
        for step in 0..=600 {
            let q = limit * math::exp10(-f64::from(step) / 100.0);
            for u in [0.0, 0.25, 0.5, 0.999, 1.0 - f64::EPSILON / 2.0] {
                let e = eccentricity_of(q, a_min, u);
                if e >= 1.0 {
                    continue;
                }
                let a = q / (1.0 - e);
                let period = TAU * a * (a / mu).sqrt() / SECONDS_PER_JULIAN_YEAR;
                // An f64 near 1 holds 1 − e to about a part in 10⁹ at the smallest perihelia, so
                // the period is checked to 10⁻⁸.
                assert!(
                    period >= MIN_PERIOD_YEARS * (1.0 - 1e-8),
                    "{l} L☉, q {q}: P {period}"
                );
                assert!(
                    a * (1.0 + e) > model.listing_radius().value(),
                    "{l} L☉, q {q}"
                );
            }
        }
    }
}

/// The ruling's test 5: about hosts of 10, 100 and 10⁴ L☉ no more than 1,024 comets are listed at
/// once, and the floor is never below 1 km.
#[test]
fn a_luminous_host_lists_at_most_1024_at_once() {
    for (l, m) in [(10.0_f64, 1.6), (100.0, 2.5), (1e4, 1.0)] {
        let inputs = CometInputs {
            rate: EventsPerSecond::per_julian_year(10.9 * l.sqrt()),
            luminosity: SolarLuminosities::new(l),
            host: GravitationalParameter::from_solar_masses(SolarMasses::new(m)),
            ..solar_comets()
        };
        let model = model(&inputs);
        assert!(model.floor_diameter() >= NUCLEUS_REFERENCE_DIAMETER);
        let at_once = model.listed_rate().value() * 2.0 * model.reach().as_seconds_f64();
        assert!(
            at_once <= MAX_LISTED_COMETS * (1.0 + 1e-9),
            "{l} L☉: {at_once}"
        );
        let mut listed = Vec::new();
        comets_listed_at(&model, UniverseTime::EPOCH, &mut listed);
        assert!(listed.len() <= 1_024, "{l} L☉: {} listed", listed.len());
        assert!(!listed.is_empty(), "{l} L☉");
    }
}
