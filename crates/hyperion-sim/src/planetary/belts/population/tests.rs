//! P14.T48.g's population laws: the encounter kernel's limits (decision-p14-t48g-opik §4), the
//! Solar belts' moments, the hold and its tripwire, and the laws' bits.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::belts::tests::solar;
use crate::planetary::belts::{Belt, FIRST_BELT_SLOT};
use crate::planetary::placement::Neighbour;
use crate::planetary::testing::with_every_24_hold;
use crate::units::consts::METRES_PER_AU;
use crate::units::{EarthMasses, SolarMasses};

/// A length of `x` astronomical units.
fn au(x: f64) -> Metres {
    Metres::new(x * METRES_PER_AU)
}

/// Öpik's mean of U² at equal semi-major axes over the part `part`'s untruncated laws, its
/// eccentricities renormalised below 1 as science-r09-craters.md F5.6 takes them, under
/// `weighting`.
fn equal_axis_mean(part: BeltPart, weighting: Weighting) -> f64 {
    let (width, inclination) = widths(part);
    let eccentricities = Eccentricities::Rayleigh {
        width,
        limit: 1.0,
        norm: -math::exp_m1(-1.0 / (2.0 * width * width)),
    };
    let (sum, weight) = crossers_at(
        1.0,
        eccentricities,
        &InclinationNodes::of(inclination),
        weighting,
    );
    sum / weight
}

/// The Solar System input's belts (P14.T21's tests' median solar disc and eight planets).
fn solar_belts() -> Vec<Belt> {
    solar::belts_about(&solar::planets(), &solar::disc(), FIRST_BELT_SLOT).into_belts()
}

/// The planet of mass `mass` M⊕ at `a` au with eccentricity `e`, its chaotic zone about the Sun
/// (P14.T21.a's 1.3 μ^(2⁄7) beyond its pericentre and apocentre), au.
fn zone(mass: f64, a: f64, e: f64) -> (f64, f64) {
    let (lo, hi) = crate::planetary::belts::chaotic_zone(
        &Neighbour::new(EarthMasses::new(mass), au(a), e),
        SolarMasses::new(1.0),
    );
    (lo / METRES_PER_AU, hi / METRES_PER_AU)
}

/// The Solar main belt's population between `floor` and `ceiling` au, uniform in a over Jupiter's
/// 4:1 to 2:1.
fn main_belt(floor: f64, ceiling: f64) -> PopulationLaw {
    PopulationLaw::new(PopulationParts {
        part: BeltPart::Main,
        eccentricity: EccentricityLaw::TruncatedRayleigh,
        inner_edge: au(2.065),
        outer_edge: au(3.279),
        floor: au(floor),
        ceiling: Some(au(ceiling)),
        spread: AxisSpread::Uniform,
    })
    .unwrap()
}

/// decision-p14-t48g-opik §4: at equal semi-major axes and over untruncated laws, the kernel
/// unweighted gives science-r09-craters.md F5.6's 0.0586 and 0.408, and Öpik's weighting 0.0453
/// and 0.3205, each within 10⁻³.
#[test]
fn the_kernel_at_equal_axes_gives_f5_6_s_means_unweighted_and_its_own_weighted() {
    let cases = [
        (BeltPart::Main, Weighting::Orbits, 0.0586),
        (BeltPart::Scattered, Weighting::Orbits, 0.408),
        (BeltPart::Main, Weighting::Collisions, 0.0453),
        (BeltPart::Scattered, Weighting::Collisions, 0.3205),
    ];
    for (part, weighting, expected) in cases {
        let mean = equal_axis_mean(part, weighting);
        eprintln!("{part:?} {weighting:?}: {mean:.6}");
        assert!(
            (mean - expected).abs() <= 1e-3,
            "{part:?} {weighting:?}: {mean:.6} against {expected}"
        );
    }
}

/// P14.T48.g: the Solar asteroid belt's population ⟨e²⟩ lies within 5% of 0.020 between the
/// zones science-r09-craters.md F5.6 takes, Mars's and Jupiter's chaotic zones about their circular
/// orbits (1.55 and 4.28 au): truncation costs the belt proper little. The Solar System input's
/// generated belt, whose zones reach from Mars's aphelion and Jupiter's perihelion (P14.T21.a, 1.69
/// and 4.03 au), and whose a lies as the disc's solids do, loses more, 6.3% (0.0187), which is
/// recorded.
#[test]
fn the_solar_asteroid_belt_loses_little_to_truncation() {
    let (_, mars) = zone(0.107, 1.524, 0.0);
    let (jupiter, _) = zone(317.8, 5.203, 0.0);
    let f5_6 = main_belt(mars, jupiter).mean_square_eccentricity();
    eprintln!("F5.6's zones {mars:.3}–{jupiter:.3} au: ⟨e²⟩ {f5_6:.5}");
    assert!((f5_6 / 0.020 - 1.0).abs() <= 0.05, "⟨e²⟩ {f5_6}");

    let belts = solar_belts();
    let generated = belts[0].main().population();
    let e2 = generated.mean_square_eccentricity();
    let floor = generated.parts().floor.value() / METRES_PER_AU;
    let ceiling = generated.parts().ceiling.unwrap().value() / METRES_PER_AU;
    eprintln!(
        "generated {floor:.3}–{ceiling:.3} au: ⟨e²⟩ {e2:.5}, mean e {:.5}",
        generated.mean_eccentricity()
    );
    assert!((0.017..0.020).contains(&e2), "⟨e²⟩ {e2}");
    let untruncated = 0.1 * (PI / 2.0).sqrt();
    assert!(generated.mean_eccentricity() < untruncated);
}

/// P14.T48.g: a scattered component's population under T21.c's Rayleigh law, truncated at
/// Neptune's chaotic zone, has ⟨e²⟩ below the untruncated law's 0.18. The law adopted beyond the
/// Solar System input's Neptune is perihelion-anchored instead, and its ⟨e²⟩ is the observed
/// scattered disc's: within 10% of 0.191, the mean of JPL's Small-Body Database's 2,032 bodies of
/// the `TNO` class between Neptune's 2:1 and the input disc's outer edge at 90 au (fetched
/// 2026-10-26), above the 0.18 that bound presumed (recorded).
#[test]
fn a_scattered_component_follows_the_observed_disc() {
    let belts = solar_belts();
    let kuiper = &belts[1];
    let scattered = kuiper.scattered().unwrap().population();
    assert_eq!(
        scattered.parts().eccentricity,
        EccentricityLaw::PerihelionAnchored
    );
    let e2 = scattered.mean_square_eccentricity();
    let floor = scattered.parts().floor.value() / METRES_PER_AU;
    let outer = scattered.parts().outer_edge.value() / METRES_PER_AU;
    eprintln!(
        "scattered to {outer:.1} au beyond {floor:.3} au: ⟨e²⟩ {e2:.5}, mean e {:.5}",
        scattered.mean_eccentricity()
    );
    assert!((90.0..90.1).contains(&outer), "{outer}");
    assert!((e2 / 0.191 - 1.0).abs() <= 0.10, "⟨e²⟩ {e2}");
    // The same annulus under T21.c's Rayleigh law, truncated at the same floor.
    let rayleigh = PopulationLaw::new(PopulationParts {
        eccentricity: EccentricityLaw::TruncatedRayleigh,
        ..*scattered.parts()
    })
    .unwrap();
    let rayleigh_e2 = rayleigh.mean_square_eccentricity();
    eprintln!("the Rayleigh alternative's ⟨e²⟩ {rayleigh_e2:.5}");
    assert!(rayleigh_e2 < 0.18, "⟨e²⟩ {rayleigh_e2}");
    // The belt proper beyond Neptune is truncated Rayleigh, with no ceiling.
    let proper = kuiper.main().population();
    assert_eq!(
        proper.parts().eccentricity,
        EccentricityLaw::TruncatedRayleigh
    );
    assert_eq!(proper.parts().ceiling, None);
    assert!(proper.mean_square_eccentricity() < 0.02);
}

/// decision-p14-t48g-opik §4: `U_b`² is finite and positive at both edges of each of the Solar
/// System input's belt parts, and at their mid-radii, and its weighting slows the encounters.
#[test]
fn the_encounter_speed_is_finite_and_positive_at_both_edges() {
    for belt in solar_belts() {
        for component in belt.components() {
            let law = component.population();
            for a in [
                component.inner_edge(),
                component.radius(),
                component.outer_edge(),
            ] {
                let u2 = law.encounter_excitation(a);
                let orbits = law.encounter_mean(a, Weighting::Orbits);
                eprintln!(
                    "{:?} {:?} at {:.3} au: U² {u2:.5} (orbits {orbits:.5})",
                    belt.kind(),
                    component.part(),
                    a.value() / METRES_PER_AU
                );
                assert!(u2.is_finite() && u2 > 0.0, "{u2}");
                assert!(orbits.is_finite() && u2 < orbits, "{u2} {orbits}");
            }
        }
    }
}

/// The encounter quadrature against an independent replica's (the lane's `proto.py`, Python,
/// Gauss–Legendre of 64 nodes in a on each side of the target and 32 in s and i, converged to
/// 3 × 10⁻⁶ against 32 and 24): the Solar main belt between 1.694 and 4.024 au, uniform in a, at
/// Ceres's 2.7675 au and at the mid-radius, within 2 × 10⁻⁴.
#[test]
fn the_encounter_speed_agrees_with_its_replica() {
    let belt = main_belt(1.694, 4.024);
    for (a, expected) in [
        (2.7675, 0.045_189),
        (f64::midpoint(2.065, 3.279), 0.045_280),
    ] {
        let u2 = belt.encounter_excitation(au(a));
        eprintln!("at {a:.4} au: {u2:.6}");
        assert!((u2 - expected).abs() <= 2e-4, "{u2} against {expected}");
    }
}

/// Below the bump the record's moments are version 23's to the bit, whatever the laws, and under
/// every 24 hold the laws'.
#[test]
fn the_record_s_moments_are_23_s_below_the_bump() {
    for belt in solar_belts() {
        for component in belt.components() {
            let part = component.part();
            assert_same_bits(component.mean_eccentricity(), part.mean_eccentricity());
            assert_same_bits(
                component.mean_inclination().value(),
                part.mean_inclination().value(),
            );
            let (e, i) = with_every_24_hold(|| {
                (component.mean_eccentricity(), component.mean_inclination())
            });
            assert_same_bits(e, component.population().mean_eccentricity());
            assert_same_bits(i.value(), part.mean_inclination().value());
        }
    }
}

/// The belt population is held until its bump: below it every belt's moments and every impact
/// velocity are version 23's, and the tests' switch reaches the laws ahead of it.
#[test]
fn the_belt_population_is_held_only_until_its_bump() {
    assert!(
        GENERATOR_VERSION < BELT_POPULATION_VERSION,
        "P14.T48.g's bump (24 with P14.T54.b, or the next) puts every belt's moments and impact \
         velocity on its population's laws. Delete `BELT_POPULATION_VERSION`, `PopulationRules` \
         with its `Held` arms (`BeltComponent::mean_eccentricity`'s, with \
         `BeltPart::mean_eccentricity` if nothing else reads it; \
         `hooks::figures`'s `in_belt_excitation`, `nearest_belt_excitation` with its cube \
         `approach`, `belt_excitation`, `Impactors::lunar`'s and `with_abundance_under`'s \
         `rules`, and `LUNAR_ENCOUNTER_SPEED` for \
         `POPULATION_LUNAR_ENCOUNTER_SPEED`), `with_population_rules` and its thread's rules, its \
         arm in `with_every_24_hold`, and this test; and move T48.f's below-24 tests (Ceres's 5.6 \
         ± 0.1) to the population's"
    );
    assert_eq!(PopulationRules::in_force(), PopulationRules::Held);
    assert_eq!(
        with_population_rules(PopulationRules::Laws, PopulationRules::in_force),
        PopulationRules::Laws
    );
    assert_eq!(PopulationRules::in_force(), PopulationRules::Held);
}

/// The laws' parts out of range are refused, each naming its part.
#[test]
fn a_law_s_parts_out_of_range_are_refused() {
    let good = *main_belt(1.694, 4.024).parts();
    let refused = |parts: PopulationParts| PopulationLaw::new(parts).unwrap_err();
    assert_eq!(
        refused(PopulationParts {
            outer_edge: good.inner_edge,
            ..good
        }),
        BuildPopulationLawError::Edges(good.inner_edge, good.inner_edge)
    );
    let nan = Metres::new(f64::NAN);
    assert!(matches!(
        refused(PopulationParts { floor: nan, ..good }),
        BuildPopulationLawError::Floor(_)
    ));
    // A perihelion-anchored law needs a floor to anchor to; a truncated Rayleigh's may be zero.
    let unanchored = PopulationParts {
        eccentricity: EccentricityLaw::PerihelionAnchored,
        floor: Metres::ZERO,
        ..good
    };
    assert_eq!(
        refused(unanchored),
        BuildPopulationLawError::Floor(Metres::ZERO)
    );
    assert!(
        PopulationLaw::new(PopulationParts {
            floor: Metres::ZERO,
            ..good
        })
        .is_ok()
    );
    assert!(matches!(
        refused(PopulationParts {
            ceiling: Some(Metres::ZERO),
            ..good
        }),
        BuildPopulationLawError::Ceiling(_)
    ));
    let spread = AxisSpread::Solids {
        characteristic_radius: au(30.0),
        snow_line: au(2.7),
        inside_share: 1.5,
    };
    assert_eq!(
        refused(PopulationParts { spread, ..good }),
        BuildPopulationLawError::Spread(spread)
    );
    for message in [
        BuildPopulationLawError::Edges(good.inner_edge, good.inner_edge).to_string(),
        BuildPopulationLawError::Floor(nan).to_string(),
        BuildPopulationLawError::Ceiling(Metres::ZERO).to_string(),
        BuildPopulationLawError::Spread(spread).to_string(),
    ] {
        assert!(!message.chars().next().unwrap().is_uppercase(), "{message}");
        assert!(!message.ends_with(['.', '!', '?']), "{message}");
    }
}

/// The Solar System input's belts' laws and moments, and their encounter speeds across each part,
/// pinned to the bit: the same bits natively and as WebAssembly (P14.T48.g).
#[test]
fn the_solar_belts_populations_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (b, belt) in solar_belts().iter().enumerate() {
        for component in belt.components() {
            let law = component.population();
            let parts = law.parts();
            let name = format!("belt{b}_{:?}", component.part()).to_lowercase();
            w.line(&format!("{name}_law = {:?}", parts.eccentricity));
            w.f64(&format!("{name}_floor_m"), parts.floor.value());
            if let Some(ceiling) = parts.ceiling {
                w.f64(&format!("{name}_ceiling_m"), ceiling.value());
            }
            w.f64(&format!("{name}_mean_e"), law.mean_eccentricity());
            w.f64(&format!("{name}_mean_e2"), law.mean_square_eccentricity());
            w.f64(
                &format!("{name}_mean_i_rad"),
                law.mean_inclination().value(),
            );
            let (lo, hi) = (
                component.inner_edge().value(),
                component.outer_edge().value(),
            );
            for k in 0..=4 {
                let a = lo + (hi - lo) * f64::from(k) / 4.0;
                w.f64(
                    &format!("{name}_u2_{k}"),
                    law.encounter_excitation(Metres::new(a)),
                );
            }
        }
    }
    golden!("planetary/belt_population", w.as_str());
}
