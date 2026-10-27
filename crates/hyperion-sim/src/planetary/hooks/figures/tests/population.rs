//! P14.T48.g's impactors under every hold at 24: Öpik's mean `U_b`² of the belts' populations,
//! rule 3's refitted approach and the refitted lunar encounter factor (decision-p14-t48g-opik), on
//! the Solar System as P14.T48.f's tests take it.

use super::*;
use crate::planetary::SystemContext;
use crate::planetary::index::BodyIndex;
use crate::planetary::record::Section;
use crate::planetary::testing::with_every_24_hold;

/// The Solar System bodies rule 3's approach is fitted on, by their impactors under `sources`:
/// Mercury, Venus, Earth, the Moon and Mars.
fn approach_bodies(sources: &ImpactorSources) -> [(&'static str, Impactors); 5] {
    [
        ("Mercury", planet_impactors("Mercury", sources)),
        ("Venus", planet_impactors("Venus", sources)),
        ("Earth", planet_impactors("Earth", sources)),
        ("the Moon", the_moon(sources)),
        ("Mars", planet_impactors("Mars", sources)),
    ]
}

/// The five bodies' velocities, km/s, with rule 3's approach c xⁿ from the nearest belt proper's
/// mid-radius `a_b` on its `U_b`² `own`, as [`speeds`] assembles them.
fn approach_velocities(a_b: Metres, own: f64, c: f64, n: f64) -> [f64; 5] {
    let host = sun_parameter();
    let velocity = |name: &str, moon: Option<Metres>, escape_squared: f64| {
        let parent = planet(name);
        let x = fractional_distance(parent.orbit.semi_major_axis, a_b);
        let source = ImpactorSource::NearestBelt {
            excitation: own + c * math::powf(x, n),
            composition: BeltComposition::Rocky,
        };
        speeds(source, &parent, moon, host, escape_squared)
            .velocity_squared_m2_s2
            .sqrt()
            / 1e3
    };
    let escape = |name: &str| {
        let &(.., mass, radius) = PLANETS.iter().find(|p| p.0 == name).unwrap();
        2.0 * GRAVITATIONAL_CONSTANT * mass * 1e24 / (radius * 1e3)
    };
    let (a_km, gm, radius_km) = MOON;
    let moon_escape = 2.0 * gm * 1e9 / (radius_km * 1e3);
    [
        velocity("Mercury", None, escape("Mercury")),
        velocity("Venus", None, escape("Venus")),
        velocity("Earth", None, escape("Earth")),
        velocity("Earth", Some(Metres::new(a_km * 1e3)), moon_escape),
        velocity("Mars", None, escape("Mars")),
    ]
}

/// P14.T48.f's Solar-System figures as built, km/s, on JPL's elements (its record's, science-r09-
/// craters.md F5.5): Mercury, Venus, Earth, the Moon and Mars, which version 23 gives within
/// 0.005 km/s and which rule 3's refit at 24 is held to (decision-p14-t48g-opik §4, as main read
/// it, 2026-10-26: the rounded 41.4, …, 10.6 admit no c xⁿ within 0.1 km/s).
const T48F_AS_BUILT_KMS: [f64; 5] = [41.43, 25.68, 19.87, 16.68, 10.55];

/// decision-p14-t48g-opik §2 and §4: under every hold at 24, Mercury, Venus, Earth, the Moon and
/// Mars stay within 0.1 km/s of P14.T48.f's figures as built ([`T48F_AS_BUILT_KMS`]), the Moon
/// within 100 m/s of R09's `LUNAR_IMPACT_VELOCITY`, and Mars ÷ the Moon in 0.53–0.65; Ceres lies
/// within 10% of Bottke et al. 1994's main-belt mean, about 5.3 km/s (from memory, ruling §1;
/// T48.f's 5.6 ± 0.1 holds below 24 only); Pluto within 20% of Singer et al. 2019's 2.2 km/s; and
/// the 15 regular moons within 10% of Brasser et al. 2025's Table 5, from the giants' comets.
#[test]
fn under_every_24_hold_the_solar_system_keeps_t48f_s_velocities() {
    let sun = solar_sources();
    let names = approach_bodies(sun).map(|(name, i)| (name, kms(&i)));
    for ((name, at_23), figure) in names.iter().zip(T48F_AS_BUILT_KMS) {
        assert!((at_23 - figure).abs() <= 0.005, "{name}: {at_23} at 23");
    }
    let as_built: Vec<(&str, f64)> = names
        .iter()
        .map(|&(name, _)| name)
        .zip(T48F_AS_BUILT_KMS)
        .collect();
    let (at_24, ceres_24, pluto_24, moon_24) = with_every_24_hold(|| {
        (
            approach_bodies(sun).map(|(_, i)| kms(&i)),
            kms(&ceres(163.0, 0.0, sun)),
            kms(&pluto(sun)),
            the_moon(sun).velocity().value(),
        )
    });
    for ((name, before), after) in as_built.iter().zip(at_24) {
        eprintln!(
            "{name}: {before:.4} → {after:.4} km/s ({:+.4})",
            after - before
        );
        assert!(
            (after - before).abs() <= 0.1,
            "{name}: {after:.4} against {before:.4}"
        );
    }
    assert!(
        (moon_24 - LUNAR_IMPACT_VELOCITY.value()).abs() <= 100.0,
        "the Moon's {moon_24} m/s against the map's reference"
    );
    let ratio = at_24[4] / at_24[3];
    assert!((0.53..=0.65).contains(&ratio), "Mars ÷ the Moon {ratio:.4}");
    eprintln!("Ceres {ceres_24:.4} km/s, Pluto {pluto_24:.4} km/s, Mars ÷ the Moon {ratio:.4}");
    assert!(
        (ceres_24 / 5.3 - 1.0).abs() <= 0.10,
        "Ceres: {ceres_24:.4} km/s"
    );
    assert!(
        (pluto_24 / 2.2 - 1.0).abs() <= 0.20,
        "Pluto: {pluto_24:.4} km/s"
    );
    with_every_24_hold(|| {
        for (name, parent, a_km, gm, radius_km, expected) in MOONS {
            let moon = moon_impactors(
                parent,
                (a_km, gm, radius_km),
                SnowLineSide::Beyond,
                (100.0, 0.0),
                sun,
            );
            let v = kms(&moon);
            assert!(
                (v / expected - 1.0).abs() <= 0.10,
                "{name}: {v:.3} km/s against {expected}"
            );
            assert_eq!(moon.source(), ImpactorSource::Comets, "{name}");
        }
    });
}

/// decision-p14-t48g-opik §3: rule 3's approach at 24, c xⁿ, is the minimax fit on the five bodies
/// against their figures below 24 ([`ENCOUNTER_APPROACH_COEFFICIENT`] and
/// [`ENCOUNTER_APPROACH_EXPONENT`]), on a grid of 0.0005 in c and 0.001 in n, which this refits;
/// its greatest miss lies within 0.1 km/s, where the cube's is 0.31–0.38 km/s.
#[test]
fn under_every_24_hold_rule_three_s_approach_is_its_minimax_fit() {
    let sun = solar_sources();
    let targets = approach_bodies(sun).map(|(_, i)| kms(&i));
    let a_b = solar_belts()[0].main.mid_radius();
    let own = with_every_24_hold(|| {
        solar_belts()[0]
            .population_of(BeltPart::Main)
            .unwrap()
            .encounter_excitation(a_b)
    });
    let miss = |c: f64, n: f64| {
        approach_velocities(a_b, own, c, n)
            .iter()
            .zip(targets)
            .map(|(v, t)| (v - t).abs())
            .fold(0.0, f64::max)
    };
    let mut best = (f64::INFINITY, 0.0, 0.0);
    for j in 0..=300 {
        let n = 2.7 + 0.001 * f64::from(j);
        for k in 0..=200 {
            let c = 0.95 + 0.0005 * f64::from(k);
            let m = miss(c, n);
            if m < best.0 {
                best = (m, c, n);
            }
        }
    }
    let (m, c, n) = best;
    eprintln!(
        "U_b² at the mid-radius {own:.6}; minimax c {c:.4}, n {n:.3}, greatest miss {m:.4} km/s; \
         the cube's {:.4}",
        miss(1.0, 3.0)
    );
    assert!((ENCOUNTER_APPROACH_COEFFICIENT - c).abs() < 1e-9, "c {c}");
    assert!((ENCOUNTER_APPROACH_EXPONENT - n).abs() < 1e-9, "n {n}");
    assert!(m <= 0.1, "greatest miss {m}");
    assert!(miss(1.0, 3.0) > 0.3, "the cube's miss {}", miss(1.0, 3.0));
    // The fit is what the impactors take at 24.
    let at_24 = with_every_24_hold(|| approach_bodies(sun).map(|(_, i)| kms(&i)));
    for (v, fitted) in at_24.iter().zip(approach_velocities(a_b, own, c, n)) {
        assert!((v - fitted).abs() < 1e-9, "{v} against {fitted}");
    }
}

/// decision-p14-t48g-opik §3: under every hold at 24 the reference Moon's `R_b` is 1 to 10⁻¹², its
/// encounter factor being [`POPULATION_LUNAR_ENCOUNTER_SPEED`], which this refits.
#[test]
fn under_every_24_hold_the_moon_s_rate_is_one() {
    let moon = with_every_24_hold(reference_moon);
    eprintln!(
        "the reference Moon at 24: V {:.17e} m/s, Ψ {}, R_b {}",
        moon.encounter.value(),
        moon.abundance,
        moon.rate()
    );
    assert!(
        (moon.encounter / POPULATION_LUNAR_ENCOUNTER_SPEED - 1.0).abs() < 1e-12,
        "{:?}",
        moon.encounter
    );
    assert_same_bits(moon.abundance, 1.0);
    assert!((moon.rate() - 1.0).abs() < 1e-12, "{}", moon.rate());
    // Below 24 the Moon's rate is still taken over the version's factor.
    assert_eq!(reference_moon().lunar, LUNAR_ENCOUNTER_SPEED);
}

/// Under every hold at 24 the Solar System's rates keep P14.T48.c's ruled ranges
/// ([`check_the_solar_system_s_rates`]).
#[test]
fn under_every_24_hold_the_solar_system_s_rates_follow_the_ruling() {
    with_every_24_hold(|| check_the_solar_system_s_rates(POPULATION_LUNAR_ENCOUNTER_SPEED));
}

/// Below 24 every source's excitation is P14.T48.f's whatever the belts' populations: the Solar
/// System's sources with their populations give the same bits as without them.
#[test]
fn below_24_the_populations_move_no_impactor() {
    let without: Vec<SourceBelt> = solar_belts()
        .into_iter()
        .map(|belt| SourceBelt {
            population: None,
            ..belt
        })
        .collect();
    let bare = ImpactorSources::new(sun_parameter(), without, solar_planets()).unwrap();
    let sun = solar_sources();
    let pairs = [
        (
            planet_impactors("Mars", sun),
            planet_impactors("Mars", &bare),
        ),
        (the_moon(sun), the_moon(&bare)),
        (ceres(163.0, 0.0, sun), ceres(163.0, 0.0, &bare)),
        (pluto(sun), pluto(&bare)),
    ];
    for (with, without) in pairs {
        assert_eq!(with, without);
    }
}

/// The Solar System's impactors under every hold at 24, pinned to the bit as
/// [`the_solar_system_s_impactors_are_pinned`] pins version 23's.
#[test]
fn under_every_24_hold_the_solar_system_s_impactors_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let sun = solar_sources();
    let text = with_every_24_hold(|| {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let pin = |w: &mut GoldenWriter, name: &str, i: &Impactors| {
            w.line(&format!("{name}_rule = {}", rule(i.source())));
            w.f64(&format!("{name}_excitation"), i.source().excitation());
            w.f64(&format!("{name}_velocity"), i.velocity().value());
            w.f64(&format!("{name}_rate"), i.rate());
        };
        for (name, ..) in PLANETS {
            pin(&mut w, &name.to_lowercase(), &planet_impactors(name, sun));
        }
        pin(&mut w, "moon", &the_moon(sun));
        pin(&mut w, "ceres", &ceres(163.0, 0.17 * CERES.3 * 1e24, sun));
        pin(&mut w, "pluto", &pluto(sun));
        pin(&mut w, "reference_moon", &reference_moon());
        w.as_str().to_owned()
    });
    golden!("planetary/impactors_population", &text);
}

/// One body's impact figures: its rule, velocity (m/s), rate and crater densities at 1 km, the
/// contract's N☾ and the mapped one the surface crate derives (per km²).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImpactFigures {
    rule: &'static str,
    velocity: f64,
    rate: f64,
    lunar_density: f64,
    mapped_density: f64,
}

/// The impact figures of every body of `system` with figures at the epoch, by index, under the
/// rules in force.
fn impact_figures(
    ctx: &SystemContext,
    system: &crate::planetary::PlanetarySystem,
) -> Vec<(BodyIndex, ImpactFigures)> {
    use crate::time::UniverseTime;
    system
        .indices()
        .into_iter()
        .filter_map(|index| {
            let hooks = system.hooks_at(ctx, index, UniverseTime::EPOCH).ok()??;
            let Section::Ok(figures) = hooks.figures() else {
                return None;
            };
            let impactors = figures.impactors();
            Some((
                index,
                ImpactFigures {
                    rule: rule(impactors.source()),
                    velocity: impactors.velocity().value(),
                    rate: impactors.rate(),
                    lunar_density: figures.crater_density().value(),
                    mapped_density: craters::cumulative_density(
                        figures.craters(),
                        Metres::new(1_000.0),
                    )
                    .value(),
                },
            ))
        })
        .collect()
}

/// The belts' recorded mean eccentricities of `system` at the epoch, by index, under the rules in
/// force.
fn belt_moments(
    ctx: &SystemContext,
    system: &crate::planetary::PlanetarySystem,
) -> Vec<(BodyIndex, f64, f64)> {
    use crate::planetary::record::Population;
    use crate::time::UniverseTime;
    system
        .snapshot_at(ctx, UniverseTime::EPOCH)
        .bodies()
        .iter()
        .filter_map(|record| match record.population().ok() {
            Some(Population::Belt(belt)) => Some((
                record.index(),
                belt.mean_eccentricity(),
                belt.mean_inclination().value(),
            )),
            _ => None,
        })
        .collect()
}

/// The class of a body struck by `rule` in `system`, for the census: a belt member's rule with its
/// part and that part's eccentricity law, and the rule alone for any other body.
fn census_class(
    system: &crate::planetary::PlanetarySystem,
    index: BodyIndex,
    rule: &'static str,
) -> String {
    let member = system
        .belts()
        .iter()
        .flat_map(|belt| belt.members().iter().map(move |m| (belt, m)))
        .find(|(_, m)| m.index() == index);
    match member {
        Some((belt, m)) => {
            let law = belt
                .population()
                .part(m.part())
                .map(|law| law.parts().eccentricity);
            format!("{rule} member {:?} {law:?}", m.part())
        }
        None => rule.to_owned(),
    }
}

/// A census class's relative changes from 23's rules to 24's, one entry per body.
#[derive(Debug, Default)]
struct ClassChanges {
    velocity: Vec<f64>,
    rate: Vec<f64>,
    mapped: Vec<f64>,
}

/// The 5th, 50th and 95th percentiles and the extremes of `values`, sorted in place.
fn quantiles(values: &mut [f64]) -> [f64; 5] {
    values.sort_by(f64::total_cmp);
    let at = |q: f64| {
        let n = values.len().saturating_sub(1);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a census's index of a few thousand, rounded"
        )]
        let k = (q * n as f64).round() as usize;
        values[k]
    };
    [at(0.0), at(0.05), at(0.5), at(0.95), at(1.0)]
}

/// P14.T48.g's census, by name: over the first 200 systems of the sample, generated whole under
/// every hold at 24, each belt's recorded mean eccentricity and inclination and each body's impact
/// velocity, rate and crater densities at 1 km with the belt population's hold set back and in
/// force, so that T48.g's own movers are stated apart from the other holds'. Every figure is
/// finite, the inclinations do not move, and only bodies struck from belts move but for the lunar
/// factor's 0.49% in every rate.
#[test]
#[ignore = "a measurement for P14.T48.g's census: 200 whole systems under every hold at 24, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "a census's one pass over the sample and its report, which read best together"
)]
fn under_every_24_hold_the_sample_s_belt_moments_and_impacts_are_stated() {
    use std::collections::BTreeMap;

    use crate::planetary::belts::population::{PopulationRules, with_population_rules};
    use crate::planetary::system::tests::{SEED, sample};

    let mut ratios = Vec::new();
    let mut coldest: Vec<(f64, String)> = Vec::new();
    let mut classes: BTreeMap<String, ClassChanges> = BTreeMap::new();
    let mut extremes: Vec<(f64, String)> = Vec::new();
    for ctx in sample().iter().take(200) {
        let (system, held, laws) = with_every_24_hold(|| {
            let system = crate::planetary::generate(SEED, ctx);
            let held = with_population_rules(PopulationRules::Held, || {
                (belt_moments(ctx, &system), impact_figures(ctx, &system))
            });
            let laws = (belt_moments(ctx, &system), impact_figures(ctx, &system));
            (system, held, laws)
        });
        for ((index, e_held, i_held), (_, e, i)) in held.0.iter().zip(&laws.0) {
            assert!(e.is_finite() && i.is_finite());
            assert_same_bits(*i_held, *i);
            ratios.push(e / e_held);
            let belt = system.belts().iter().find(|b| b.index() == *index).unwrap();
            let law = belt.main().population().parts();
            let au = |m: Metres| m.value() / METRES_PER_AU;
            coldest.push((
                e / e_held,
                format!(
                    "{:016x}.{:04x} {:?} {:?}: {:.3}–{:.3} au, floor {:.3}, ceiling {:?}, mean e {e:.5}",
                    ctx.id().raw(),
                    index.get(),
                    belt.kind(),
                    belt.site(),
                    au(law.inner_edge),
                    au(law.outer_edge),
                    au(law.floor),
                    law.ceiling.map(au)
                ),
            ));
        }
        for ((index, before), (_, after)) in held.1.iter().zip(&laws.1) {
            assert!(
                after.velocity.is_finite()
                    && after.rate.is_finite()
                    && after.lunar_density.is_finite()
                    && after.mapped_density.is_finite(),
                "{index:?}: {after:?}"
            );
            assert_eq!(before.rule, after.rule, "{index:?}");
            // A body with no crater of 1 km under its air keeps none.
            let change = |a: f64, b: f64| if b > 0.0 { a / b - 1.0 } else { 0.0 };
            let dv = change(after.velocity, before.velocity);
            let dr = change(after.rate, before.rate);
            let dm = change(after.mapped_density, before.mapped_density);
            if matches!(after.rule, "comets" | "inner_comets") {
                assert_same_bits(after.velocity, before.velocity);
            }
            let class = census_class(&system, *index, after.rule);
            let entry = classes.entry(class.clone()).or_default();
            entry.velocity.push(dv);
            entry.rate.push(dr);
            entry.mapped.push(dm);
            extremes.push((
                dv,
                format!(
                    "{:016x}.{:04x} {class}: v {:.1} → {:.1} m/s, R_b {:.4} → {:.4}, mapped N(1 \
                     km) {:.4e} → {:.4e}",
                    ctx.id().raw(),
                    index.get(),
                    before.velocity,
                    after.velocity,
                    before.rate,
                    after.rate,
                    before.mapped_density,
                    after.mapped_density
                ),
            ));
        }
    }
    let [lo, p5, p50, p95, hi] = quantiles(&mut ratios);
    eprintln!(
        "belts {}: mean e at 24 over 23's min {lo:.4}, p5 {p5:.4}, median {p50:.4}, p95 {p95:.4}, \
         max {hi:.4}",
        ratios.len()
    );
    coldest.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (ratio, line) in coldest.iter().take(4) {
        eprintln!("  coldest ×{ratio:.4}: {line}");
    }
    for (class, changes) in &mut classes {
        let [v0, v5, v50, v95, v1] = quantiles(&mut changes.velocity);
        let [r0, _, r50, _, r1] = quantiles(&mut changes.rate);
        let [m0, _, m50, _, m1] = quantiles(&mut changes.mapped);
        eprintln!(
            "  {class}: {} bodies; Δv min {:+.2}%, p5 {:+.2}%, median {:+.2}%, p95 {:+.2}%, max \
             {:+.2}%; ΔR_b {:+.2}% / {:+.2}% / {:+.2}%; Δ mapped N {:+.2}% / {:+.2}% / {:+.2}%",
            changes.velocity.len(),
            100.0 * v0,
            100.0 * v5,
            100.0 * v50,
            100.0 * v95,
            100.0 * v1,
            100.0 * r0,
            100.0 * r50,
            100.0 * r1,
            100.0 * m0,
            100.0 * m50,
            100.0 * m1
        );
    }
    extremes.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, line) in extremes.iter().take(3).chain(extremes.iter().rev().take(3)) {
        eprintln!("  {line}");
    }
}

/// A source belt whose population is not its own components', part for part and annulus for
/// annulus, is refused, naming its belt proper's law's edges.
#[test]
fn a_population_that_is_not_its_belt_s_is_refused() {
    let [main, kuiper] = <[SourceBelt; 2]>::try_from(solar_belts()).unwrap();
    let swapped = SourceBelt {
        population: kuiper.population,
        ..main
    };
    let law = kuiper.population.unwrap().main;
    assert_eq!(
        ImpactorSources::new(sun_parameter(), vec![swapped], solar_planets()),
        Err(BuildImpactorSourcesError::BeltPopulation(
            law.parts().inner_edge,
            law.parts().outer_edge
        ))
    );
    let unscattered = SourceBelt {
        scattered: None,
        ..kuiper
    };
    assert!(matches!(
        ImpactorSources::new(sun_parameter(), vec![unscattered], solar_planets()),
        Err(BuildImpactorSourcesError::BeltPopulation(..))
    ));
    let message = BuildImpactorSourcesError::BeltPopulation(Metres::ZERO, Metres::ZERO).to_string();
    assert!(!message.chars().next().unwrap().is_uppercase(), "{message}");
    assert!(!message.ends_with(['.', '!', '?']), "{message}");
}

/// decision-p14-t48g-opik §4 and the Rust review: under every hold at 24 a source belt given with
/// no population reads its parts' laws untruncated over each annulus, uniform in a, and a body in
/// its belt proper strikes at that law's Öpik mean.
#[test]
fn under_every_24_hold_a_belt_with_no_population_reads_untruncated_laws() {
    let bare = SourceBelt {
        population: None,
        ..solar_belts()[0]
    };
    let law = bare.population_of(BeltPart::Main).unwrap();
    assert_eq!(
        *law.parts(),
        PopulationParts {
            part: BeltPart::Main,
            eccentricity: EccentricityLaw::TruncatedRayleigh,
            inner_edge: bare.main.inner_edge,
            outer_edge: bare.main.outer_edge,
            floor: Metres::ZERO,
            ceiling: None,
            spread: AxisSpread::Uniform,
        }
    );
    assert_eq!(bare.population_of(BeltPart::Scattered), None);
    let sources = ImpactorSources::new(sun_parameter(), vec![bare], solar_planets()).unwrap();
    let ceres = with_every_24_hold(|| ceres(163.0, 0.0, &sources));
    let a = au(CERES.0);
    let ImpactorSource::InBelt { excitation, .. } = ceres.source() else {
        panic!("{:?}", ceres.source());
    };
    assert_same_bits(excitation, law.encounter_excitation(a));
}

/// decision-p14-t48g-opik §4.6: `U_b`² stays finite and positive where the truncation limit L
/// meets the least crossing eccentricity `e_min`, at the edges of a belt bounded by its own zones
/// (floor at its inner edge, ceiling at its outer), and is continuous there with its value just
/// inside.
#[test]
fn the_encounter_speed_is_finite_where_the_limit_meets_the_crossing_eccentricity() {
    let (inner, outer) = (au(2.0), au(3.0));
    let law = PopulationLaw::new(PopulationParts {
        part: BeltPart::Main,
        eccentricity: EccentricityLaw::TruncatedRayleigh,
        inner_edge: inner,
        outer_edge: outer,
        floor: inner,
        ceiling: Some(outer),
        spread: AxisSpread::Uniform,
    })
    .unwrap();
    for (edge, inside) in [(inner, au(2.0 + 1e-6)), (outer, au(3.0 - 1e-6))] {
        let (at, near) = (
            law.encounter_excitation(edge),
            law.encounter_excitation(inside),
        );
        eprintln!(
            "at {:.6} au: {at:.6}, just inside {near:.6}",
            edge.value() / METRES_PER_AU
        );
        assert!(at.is_finite() && at > 0.0, "{at}");
        assert!((at / near - 1.0).abs() < 0.05, "{at} against {near}");
    }
}

/// Under every hold at 24 the Solar System input's generated belts (P14.T21's, on [`PLANETS`] and
/// Neptune) give Earth and Mars within 0.1 km/s of their figures below 24: the refit, made on the
/// hand-built belt uniform in a, carries to the belts the generator makes, whose a follows the
/// disc's solids.
#[test]
fn under_every_24_hold_the_generators_belts_keep_earth_s_and_mars_s_velocities() {
    let disc = solar_disc();
    let neighbours: Vec<Neighbour> = PLANETS
        .iter()
        .map(|&(_, a, e, _, mass, _)| Neighbour::new(earths(mass * 1e24), au(a), e))
        .chain([Neighbour::new(
            EarthMasses::new(17.15),
            au(NEPTUNE_AU),
            0.009,
        )])
        .collect();
    let plane = SystemPlane::new(Radians::ZERO, Radians::ZERO).unwrap();
    let host = BeltHost::new(OrbitHost::Star(0), &disc, &neighbours, plane);
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let belts = host_belts(Seed::new(0x48f), system, &host, FIRST_BELT_SLOT);
    let generated: Vec<SourceBelt> = belts.belts().iter().map(SourceBelt::from).collect();
    let sources = ImpactorSources::new(sun_parameter(), generated, solar_planets()).unwrap();
    for name in ["Earth", "Mars"] {
        let at_23 = kms(&planet_impactors(name, &sources));
        let at_24 = with_every_24_hold(|| kms(&planet_impactors(name, &sources)));
        eprintln!("{name}: {at_23:.4} → {at_24:.4} km/s");
        assert!(
            (at_24 - at_23).abs() <= 0.1,
            "{name}: {at_24:.4} against {at_23:.4}"
        );
    }
}
