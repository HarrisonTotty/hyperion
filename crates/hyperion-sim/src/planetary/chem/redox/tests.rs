use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::solar::{
    SOLAR_AGE, derive_historic, placed_about_the_sun, solar_disc, table_bodies,
};
use crate::units::consts::{EARTH_MASS_KG, METRES_PER_AU};

/// The zero-age Sun's snow line, from the table's disc.
fn solar_snow_line() -> Metres {
    solar_disc().snow_line()
}

/// A dry body of `mass` M⊕ formed at `au` about the zero-age Sun.
fn dry(au: f64, mass: f64) -> RedoxInputs {
    RedoxInputs {
        formation_distance: Metres::new(au * METRES_PER_AU),
        snow_line: solar_snow_line(),
        water_fraction: 0.0,
        mass: EarthMasses::new(mass),
    }
}

/// O'Neill's (1987; 1988) buffers at 1,400 K, worked by hand from the two abstracts' equations:
/// IW at log f(O₂) = −12.905 and FMQ at −9.312, 3.593 apart.
#[test]
fn the_buffers_are_o_neill_s() {
    let t = BUFFER_REFERENCE_TEMPERATURE;
    assert!((iw_log_fugacity(t).value() + 12.905).abs() < 1e-3);
    assert!((fmq_log_fugacity(t).value() + 9.312).abs() < 1e-3);
    assert!((fmq_above_iw().value() - 3.593).abs() < 1e-3);
    // Plan 14's "about 3.5 at 1,200–1,300 °C", extrapolated beyond the measured ranges.
    for celsius in [1_200.0, 1_300.0] {
        let t = Kelvin::new(celsius + 273.15);
        let offset = (fmq_log_fugacity(t) - iw_log_fugacity(t)).value();
        assert!((3.4..3.6).contains(&offset), "{celsius} °C: {offset}");
    }
    // Both buffers are less oxidising the cooler the melt.
    assert!(iw_log_fugacity(Kelvin::new(1_300.0)) < iw_log_fugacity(t));
    assert!(fmq_log_fugacity(Kelvin::new(1_300.0)) < fmq_log_fugacity(t));
}

/// The soot line lies at (170 ÷ 500)² of the snow line: 0.261 au about the zero-age Sun.
#[test]
fn the_soot_line_is_the_hayashi_profile_s_500_k() {
    let snow = solar_snow_line();
    assert!((soot_line(snow) / snow - 0.1156).abs() < 1e-12);
    assert!((soot_line(snow).value() / METRES_PER_AU - 0.261).abs() < 2e-3);
}

/// P14.T50.a: the table's Mercury, Moon, Mars and Earth, each derived at its own distance about
/// the zero-age Sun with its own water fraction, lie within their cited ΔIW at the median rank.
#[test]
fn the_table_s_mantles_are_within_their_cited_ranges() {
    let disc = solar_disc();
    let fmq = fmq_above_iw().value();
    let cited = [
        // McCubbin et al. 2012's −6.3 to −2.6 and Zolotov et al. 2013's −7.3 to −4.5, both.
        ("Mercury", -6.3, -4.5, RedoxClass::Reduced),
        // Fogel and Rutherford 1995 and Wadhwa 2008.
        ("Moon", -2.5, 0.2, RedoxClass::Intermediate),
        // "close to that of the iron-wüstite buffer" (Wadhwa 2001), held to a unit either side.
        ("Mars", -1.0, 1.0, RedoxClass::Intermediate),
        // Within ±2 log units of FMQ (Frost and McCammon 2008).
        ("Earth", fmq - 2.0, fmq + 2.0, RedoxClass::Oxidised),
    ];
    for (name, low, high, class) in cited {
        let &(_, kg, km, a, e) = table_bodies()
            .find(|(n, ..)| *n == name)
            .expect("the body is in the table");
        let placed = placed_about_the_sun(&disc, (kg, km, a, e));
        let body = derive_historic(&placed, &disc, SOLAR_AGE);
        let inputs = RedoxInputs {
            formation_distance: Metres::new(a * METRES_PER_AU),
            snow_line: disc.snow_line(),
            water_fraction: body.fractions().water(),
            mass: EarthMasses::new(kg / EARTH_MASS_KG),
        };
        let mean = mean_delta_iw(&inputs);
        assert!(
            (low..=high).contains(&mean.value()),
            "{name}: IW{:+.2}",
            mean.value()
        );
        assert_same_bits(delta_iw(&inputs, UnitUniform::HALF).value(), mean.value());
        assert_eq!(RedoxClass::of(mean), class, "{name}");
    }
}

/// P14.T50.a: ΔIW is continuous in each input, across the soot and snow lines, the icy water
/// fraction, both magma-ocean masses and the ceiling: a step of 10⁻⁹ in the input moves it by
/// under 10⁻⁷.
#[test]
fn delta_iw_is_continuous_in_its_inputs() {
    let snow = solar_snow_line();
    let step = |a: RedoxInputs, b: RedoxInputs| {
        let jump = (mean_delta_iw(&a) - mean_delta_iw(&b)).value().abs();
        assert!(jump < 1e-7, "{a:?} → {b:?}: {jump}");
    };
    let nudge = |x: f64| x * (1.0 + 1e-9);
    // The distance, across the soot and snow lines, at every mass and water fraction tried.
    for (mass, water) in [(0.05, 0.0), (0.5, 0.05), (1.0, 0.0), (3.0, 0.3)] {
        for d in [soot_line(snow).value(), snow.value()] {
            let at = |d: f64| RedoxInputs {
                formation_distance: Metres::new(d),
                snow_line: snow,
                water_fraction: water,
                mass: EarthMasses::new(mass),
            };
            step(at(d), at(nudge(d)));
            step(at(d), at(d / (1.0 + 1e-9)));
        }
    }
    // The water fraction, at 0 and at the icy fraction.
    for water in [0.0, ICY_WATER_FRACTION] {
        let at = |w: f64| RedoxInputs {
            water_fraction: w,
            ..dry(5.0, 0.02)
        };
        step(at(water), at(water + 1e-9));
    }
    // The mass, at both ends of the magma ocean's rise.
    for mass in [MAGMA_OCEAN_MASSES.0.value(), MAGMA_OCEAN_MASSES.1.value()] {
        step(dry(1.0, mass), dry(1.0, nudge(mass)));
        step(dry(1.0, mass), dry(1.0, mass / (1.0 + 1e-9)));
    }
    // The ceiling: find, by bisection in mass, where a dry body formed beyond the snow line
    // meets it, and step across.
    let ceiling = fmq_above_iw().value() + OXIDISED_CEILING_ABOVE_FMQ.value();
    let (mut low, mut high) = (MAGMA_OCEAN_MASSES.0.value(), MAGMA_OCEAN_MASSES.1.value());
    for _ in 0..60 {
        let mid = f64::midpoint(low, high);
        if mean_delta_iw(&dry(4.0, mid)).value() < ceiling {
            low = mid;
        } else {
            high = mid;
        }
    }
    step(dry(4.0, low), dry(4.0, high));
    let wet = |w: f64| RedoxInputs {
        water_fraction: w,
        ..dry(4.0, 1.0)
    };
    assert!((mean_delta_iw(&wet(0.3)).value() - ceiling).abs() < 1e-12);
}

/// The scatter is a log-normal term of [`DELTA_IW_SCATTER`] at one sigma, about the mean.
#[test]
fn the_scatter_is_half_a_unit_at_one_sigma() {
    let inputs = dry(1.0, 1.0);
    let sigma = UnitUniform::new(0.841_344_746_068_542_9).expect("inside (0, 1)");
    let shift = (delta_iw(&inputs, sigma) - mean_delta_iw(&inputs)).value();
    assert!((shift - DELTA_IW_SCATTER.value()).abs() < 1e-6, "{shift}");
}

/// The terms as the model states them: the zone term's two ends, the icy term's step and the
/// magma ocean's rise.
#[test]
fn each_term_runs_between_its_cited_ends() {
    let snow = solar_snow_line();
    assert_same_bits(
        zone_term(soot_line(snow) * 0.5, snow),
        SOOT_LINE_DELTA_IW.value(),
    );
    assert!((zone_term(snow * 3.0, snow) - SNOW_LINE_DELTA_IW.value()).abs() < 1e-12);
    assert_same_bits(water_term(0.5), WATER_OXIDATION.value());
    assert_same_bits(water_term(0.0), 0.0);
    assert_same_bits(mass_term(EarthMasses::new(0.01)), 0.0);
    assert_same_bits(
        mass_term(EarthMasses::new(10.0)),
        MAGMA_OCEAN_OXIDATION.value(),
    );
    // More reduced closer in, more oxidised wetter and heavier, never down.
    let mut previous = f64::NEG_INFINITY;
    for au in [0.1, 0.3, 0.5, 1.0, 1.5, 2.0, 3.0, 10.0] {
        let now = mean_delta_iw(&dry(au, 0.3)).value();
        assert!(now >= previous, "{au} au");
        previous = now;
    }
}

/// [`RedoxClass`] bands ΔIW at its two cited edges.
#[test]
fn the_redox_classes_band_delta_iw() {
    let edge = fmq_above_iw().value() - OXIDISED_FROM_BELOW_FMQ.value();
    assert!((edge - 1.593).abs() < 1e-3);
    assert_eq!(RedoxClass::of(Dex::new(-3.000_001)), RedoxClass::Reduced);
    assert_eq!(RedoxClass::of(REDUCED_BELOW), RedoxClass::Intermediate);
    assert_eq!(
        RedoxClass::of(Dex::new(edge - 1e-6)),
        RedoxClass::Intermediate
    );
    assert_eq!(RedoxClass::of(Dex::new(edge)), RedoxClass::Oxidised);
    assert_eq!(
        RedoxClass::ALL,
        [
            RedoxClass::Reduced,
            RedoxClass::Intermediate,
            RedoxClass::Oxidised
        ]
    );
}
