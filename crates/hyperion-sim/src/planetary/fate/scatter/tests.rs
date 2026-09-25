//! The scattering step's tests (ruling 71): a hand-built pair for each outcome, a chain that takes
//! several passes, termination in at most n − 1 passes, and mass conservation.

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;

use super::*;
use crate::orbit::{Eccentricity, Orientation};
use crate::planetary::fate::{FateAt, FateBody, FateHost, death_of};
use crate::planetary::hosts::young::{Formation, FormationDraws};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::system::StarModel;
use crate::units::consts::METRES_PER_AU;
use crate::units::{
    AstronomicalUnits, GravitationalParameter, JupiterMasses, Megayears, Radians, Years,
};

const JUPITER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_326.0);
const EARTH_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(5_513.0);

fn star(mass: f64, age_at_epoch: f64) -> StarModel {
    StarModel::new(
        SolarMasses::new(mass),
        Composition::SOLAR,
        StarDraws::median(),
        Years::new(age_at_epoch),
    )
    .expect("a valid star")
}

/// The 20 M☉ star that collapses to a black hole of 6.81 M☉ from 8.30 M☉ about 20 Myr before the
/// epoch: a circular orbit takes e = 0.219.
fn black_hole_progenitor() -> StarModel {
    star(20.0, 3e7)
}

fn jupiter_mass() -> EarthMasses {
    EarthMasses::from(JupiterMasses::new(1.0))
}

/// A body of `mass` on a primordial orbit of `a_au` and `e` about 20 M☉, at mean anomaly `phase`
/// at the epoch, formed at 0.3 Myr.
fn body(a_au: f64, e: f64, mass: EarthMasses, phase: f64) -> FateBody {
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::from(AstronomicalUnits::new(a_au)),
        GravitationalParameter::from_solar_masses(SolarMasses::new(20.0)),
        Eccentricity::new(e).expect("valid"),
        Orientation::new(Radians::new(0.4), Radians::new(1.3), Radians::new(2.2)).expect("valid"),
        Radians::new(phase),
    )
    .expect("a valid orbit");
    let formation =
        Formation::from_draws(mass, Megayears::new(0.3), &FormationDraws::MEDIAN).expect("valid");
    let density = if mass.value() > 30.0 {
        JUPITER_DENSITY
    } else {
        EARTH_DENSITY
    };
    FateBody::new(formation, orbit, mass, density).expect("valid")
}

/// The fates of `bodies` on `host` followed through its first sudden death at `at` and no
/// further, and the passes the scattering step then takes.
fn through_the_death<'a>(
    bodies: &[&'a FateBody],
    host: &'a FateHost<'a>,
    at: UniverseTime,
) -> (Vec<BodyFate<'a>>, usize) {
    let deaths: Vec<_> = host.stars.iter().map(|star| death_of(star)).collect();
    let mut fates: Vec<BodyFate<'a>> = bodies
        .iter()
        .map(|&body| BodyFate::begin(body, host, deaths.clone()))
        .collect();
    for fate in &mut fates {
        fate.live_to(Some((at, 0)));
    }
    let passes = scatter(&mut fates, at);
    (fates, passes)
}

/// Whether `fate` is present and bound right after the death at `at`.
fn survives(fate: &BodyFate<'_>, at: UniverseTime) -> bool {
    fate.ending.is_none() && fate.segments.last().is_some_and(|s| s.start == at)
}

fn axis_au(at: &FateAt) -> f64 {
    at.orbit().expect("present").semi_major_axis().value() / METRES_PER_AU
}

#[test]
fn the_safronov_number_is_jupiters_10_at_5_au_and_the_earths_0_07_at_1_au() {
    let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
    let sun = SolarMasses::new(1.0);
    let jupiter = safronov_number(jupiter_mass(), JUPITER_DENSITY, au(5.2), sun);
    assert!((jupiter - 10.6).abs() < 0.1, "{jupiter}");
    let earth = safronov_number(EarthMasses::new(1.0), EARTH_DENSITY, au(1.0), sun);
    assert!((earth - 0.0705).abs() < 0.001, "{earth}");
    // Ford and Rasio's (2008) eq. 5 at r = 5 au: θ² = 10 for a Jupiter of Jupiter's radius.
    let radius = math::cbrt(3.0 * Kilograms::from(jupiter_mass()).value() / (4.0 * PI * 1_326.0));
    let theta = safronov_number(jupiter_mass(), JUPITER_DENSITY, au(5.0), sun);
    let eq5 = 10.0 * (crate::units::consts::JUPITER_RADIUS_M / radius);
    assert!((theta / eq5 - 1.0).abs() < 0.03, "{theta} against {eq5}");
}

/// A Jupiter and a 100 M⊕ planet that cross after the black hole's birth: Θ is about 100, so the
/// lighter is ejected, and the heavier takes the pair's binding energy (Ford and Rasio 2008,
/// eq. 2) in its own plane, with the eccentricity of Table 1 at β = 0.24 (ruling 80); a body built
/// by hand takes the law's median, at its pericentre.
#[test]
fn a_heavier_neighbour_with_a_safronov_number_over_1_ejects_the_lighter() {
    let progenitor = black_hole_progenitor();
    let host = FateHost::star(&progenitor);
    let death = death_of(&progenitor).expect("dead by the epoch");
    let (inner, outer) = (
        body(300.0, 0.0, jupiter_mass(), 0.0),
        body(400.0, 0.0, EarthMasses::new(100.0), 2.0),
    );
    // Alone, each is bound on a crossing orbit.
    let alone: Vec<FateAt> = [&inner, &outer]
        .map(|b| BodyFate::resolve(b, &host).at(UniverseTime::EPOCH))
        .into();
    let (inner_orbit, outer_orbit) = (alone[0].orbit().unwrap(), alone[1].orbit().unwrap());
    assert!(
        inner_orbit.apoapsis() > outer_orbit.periapsis(),
        "the orbits cross"
    );
    let theta = safronov_number(
        jupiter_mass(),
        JUPITER_DENSITY,
        inner_orbit.semi_major_axis(),
        death.after,
    );
    assert!(theta > 50.0, "{theta}");

    let fates = BodyFate::resolve_all(&[&inner, &outer], &host);
    assert_eq!(fates[1].ending(), Some(BodyState::Unbound { at: death.at }));
    let survivor = fates[0].at(UniverseTime::EPOCH);
    let recoiled_orbit = survivor.orbit().expect("the survivor is bound");
    // Eq. 2: 1 ÷ a_f = 1 ÷ a₁ + (m₂ ÷ m₁) ÷ a₂.
    let eq2 = 1.0
        / (1.0 / inner_orbit.semi_major_axis().value()
            + (100.0 / jupiter_mass().value()) / outer_orbit.semi_major_axis().value());
    assert!((recoiled_orbit.semi_major_axis().value() / eq2 - 1.0).abs() < 1e-12);
    assert!(
        recoiled_orbit.semi_major_axis() < inner_orbit.semi_major_axis(),
        "the orbit shrinks"
    );
    let beta = 100.0 / (100.0 + jupiter_mass().value());
    let (mean, dispersion) = ejection_eccentricity_law(beta);
    assert!(
        (mean - (0.202 + (beta - 0.2) / 0.05 * 0.061)).abs() < 1e-12,
        "{mean}"
    );
    let e = recoiled_orbit.eccentricity().value();
    assert_same_bits(e, truncated_eccentricity(mean, dispersion, 0.5));
    assert!((0.2..0.3).contains(&e), "e = {e}");
    let (at_death, _) = recoiled_orbit.relative_state_at(death.at);
    assert!((length(at_death.metres()) / recoiled_orbit.periapsis().value() - 1.0).abs() < 1e-9);
    // The plane and the line of apsides are kept.
    for (x, y) in recoiled_orbit
        .orientation()
        .normal()
        .iter()
        .zip(inner_orbit.orientation().normal())
    {
        assert!((x - y).abs() < 1e-12);
    }
    assert_eq!(
        recoiled_orbit.argument_of_periapsis(),
        inner_orbit.argument_of_periapsis()
    );
    // The lighter is present until the death, and ejected from it on.
    let just_before = death
        .at
        .checked_sub(crate::time::Span::new(0, 1).unwrap())
        .unwrap();
    assert_eq!(fates[1].at(just_before).state(), BodyState::Present);
    assert_eq!(
        fates[1].at(just_before).valid_until(),
        None,
        "outside the window"
    );
    assert_eq!(fates[1].at(death.at).mass(), EarthMasses::new(100.0));
    // Slot order does not decide which goes: the lighter is ejected in either order.
    let swapped = BodyFate::resolve_all(&[&outer, &inner], &host);
    assert_eq!(
        swapped[0].ending(),
        Some(BodyState::Unbound { at: death.at })
    );
    assert_eq!(swapped[1].at(UniverseTime::EPOCH), survivor);
}

/// Two Earths that cross after the black hole's birth, some 60 au out: Θ is about 0.6, so they
/// collide, and the heavier takes both masses and the pair's mean angular momentum and energy.
#[test]
fn a_heavier_neighbour_with_a_safronov_number_under_1_merges_with_the_lighter() {
    let progenitor = black_hole_progenitor();
    let host = FateHost::star(&progenitor);
    let death = death_of(&progenitor).expect("dead by the epoch");
    let (heavy, light) = (
        body(20.0, 0.0, EarthMasses::new(1.0), 0.5),
        body(23.0, 0.0, EarthMasses::new(0.5), 3.0),
    );
    let alone: Vec<FateAt> = [&heavy, &light]
        .map(|b| BodyFate::resolve(b, &host).at(death.at))
        .into();
    let (h, l) = (alone[0].orbit().unwrap(), alone[1].orbit().unwrap());
    assert!(h.apoapsis() > l.periapsis(), "the orbits cross");
    let theta = safronov_number(
        EarthMasses::new(1.0),
        EARTH_DENSITY,
        h.semi_major_axis(),
        death.after,
    );
    assert!((0.3..1.0).contains(&theta), "{theta}");

    let fates = BodyFate::resolve_all(&[&heavy, &light], &host);
    assert_eq!(
        fates[1].ending(),
        Some(BodyState::Destroyed {
            cause: DestructionCause::Collided,
            at: death.at
        })
    );
    assert_eq!(
        fates[1].at(UniverseTime::EPOCH).mass(),
        EarthMasses::new(0.5)
    );
    let merged = fates[0].at(death.at);
    assert_eq!(merged.state(), BodyState::Present);
    assert_eq!(merged.mass(), EarthMasses::new(1.5));
    let m = merged.orbit().unwrap();
    // Ford and Rasio's (2008) eq. 1: 1 ÷ a_f = Σ (m_i ÷ m) ÷ a_i, the energy's mean.
    let harmonic = 1.0
        / ((2.0 / 3.0) / h.semi_major_axis().value() + (1.0 / 3.0) / l.semi_major_axis().value());
    assert!((m.semi_major_axis().value() / harmonic - 1.0).abs() < 1e-9);
    // The angular momentum is the mass-weighted mean, as a vector.
    let (hh, hl, hm) = (
        angular_momentum(h),
        angular_momentum(l),
        angular_momentum(m),
    );
    for k in 0..3 {
        let mean = (2.0 * hh[k] + hl[k]) / 3.0;
        assert!(
            (hm[k] - mean).abs() < 1e-9 * length(hm),
            "{k}: {} against {mean}",
            hm[k]
        );
    }
    assert!(m.semi_major_axis() > h.semi_major_axis() && m.semi_major_axis() < l.semi_major_axis());
    // The merged body starts at its pericentre, and nothing else is left to scatter.
    let (r, _) = m.relative_state_at(death.at);
    assert!((length(r.metres()) / m.periapsis().value() - 1.0).abs() < 1e-9);
    assert!(axis_au(&fates[0].at(UniverseTime::EPOCH)) > 40.0);
}

#[test]
fn a_merger_of_near_circular_neighbours_keeps_the_angular_momentum_where_the_energy_cannot() {
    let progenitor = black_hole_progenitor();
    let death = death_of(&progenitor).expect("dead");
    let mu = GravitationalParameter::from_solar_masses(death.after);
    let orbit = |a_au: f64| {
        KeplerElements::from_semi_major_axis(
            Metres::from(AstronomicalUnits::new(a_au)),
            mu,
            Eccentricity::CIRCULAR,
            Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap()
    };
    let segment = |a_au: f64, mass: f64| Segment {
        start: death.at,
        orbit: orbit(a_au),
        reference: death.after,
        circularises: false,
        mass: EarthMasses::new(mass),
    };
    // Coplanar circles at 50 and 55 au: the mean √a is over the square root of the harmonic mean.
    let (heavy, light) = (segment(50.0, 1.0), segment(55.0, 1.0));
    let Merger::Bound(m) = merged(&heavy, &light, EARTH_DENSITY, death.at) else {
        panic!("the merger is bound");
    };
    assert!(m.eccentricity().value() < 1e-6, "{:?}", m.eccentricity());
    let mean_root = f64::midpoint(50.0_f64.sqrt(), 55.0_f64.sqrt());
    assert!((axis_au_of(&m) / (mean_root * mean_root) - 1.0).abs() < 1e-9);

    // Opposed orbits of nearly equal momentum leave a plunge into the remnant.
    let retrograde = Segment {
        orbit: KeplerElements::from_semi_major_axis(
            Metres::from(AstronomicalUnits::new(50.0)),
            mu,
            Eccentricity::CIRCULAR,
            Orientation::new(Radians::new(PI), Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap(),
        mass: EarthMasses::new(0.999_999),
        ..heavy
    };
    assert_eq!(
        merged(&heavy, &retrograde, EARTH_DENSITY, death.at),
        Merger::Disrupted
    );
}

/// Ruling 80's law: Ford and Rasio's (2008) Table 1 at its rows and linearly between, their §4.2
/// fit below β = 0.2 with the dispersion at 0.28 of the mean, and the last row from β = 0.5.
#[test]
fn the_survivors_eccentricity_law_is_ford_and_rasios_table() {
    for (beta, mean, dispersion) in EJECTION_ECCENTRICITY_TABLE {
        assert_eq!(ejection_eccentricity_law(beta), (mean, dispersion));
    }
    let (mean, dispersion) = ejection_eccentricity_law(0.375);
    assert!((mean - 0.467).abs() < 1e-12 && (dispersion - 0.139).abs() < 1e-12);
    let (mean, dispersion) = ejection_eccentricity_law(0.1);
    assert!((mean - 1.44 * crate::math::powf(0.1, 1.23)).abs() < 1e-15);
    assert!((dispersion / mean - 0.28).abs() < 1e-15);
    // The fit meets the table within 2% at β = 0.2.
    let (fit, _) = ejection_eccentricity_law(0.199_999_999);
    assert!((fit / 0.202 - 1.0).abs() < 0.02, "{fit}");
}

/// The inverse distribution of the truncated normal: every rank gives an eccentricity in [0, 1),
/// monotone in the rank, and over an even grid of ranks the mean and dispersion are the table's
/// where the truncation is far off, and the share above 0.8 at β = 0.5 is the normal's tail.
#[test]
fn the_truncated_normal_holds_the_tables_moments_and_its_tail() {
    let grid: Vec<f64> = (0..100_000)
        .map(|k| (f64::from(k) + 0.5) / 100_000.0)
        .collect();
    let draws = |beta: f64| -> Vec<f64> {
        let (mean, dispersion) = ejection_eccentricity_law(beta);
        grid.iter()
            .map(|&u| truncated_eccentricity(mean, dispersion, u))
            .collect()
    };
    for beta in [0.05, 0.2, 0.3, 0.5] {
        let e = draws(beta);
        assert!(e.windows(2).all(|w| w[0] <= w[1]));
        assert!(e.iter().all(|x| (0.0..1.0).contains(x)));
    }
    let e = draws(0.3);
    let n = 100_000.0;
    let mean = e.iter().sum::<f64>() / n;
    let sd = (e.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n).sqrt();
    assert!(
        (mean - 0.333).abs() < 1e-3 && (sd - 0.100).abs() < 1e-3,
        "{mean} {sd}"
    );
    // At β = 0.5, P(e > 0.8 | 0 ≤ e < 1) = (Φ(2.79) − Φ(1.30)) ÷ (Φ(2.79) − Φ(−4.62)) = 9.4%.
    let above =
        f64::from(u32::try_from(draws(0.5).iter().filter(|&&x| x > 0.8).count()).unwrap()) / n;
    let expected = (normal_cdf(0.376 / 0.135) - normal_cdf(0.176 / 0.135))
        / (normal_cdf(0.376 / 0.135) - normal_cdf(-0.624 / 0.135));
    assert!(
        (above - expected).abs() < 1e-3,
        "{above} against {expected}"
    );
    assert!((expected - 0.094).abs() < 0.002, "{expected}");
}

/// A generated body's draws are its own stream's, words 4k and 4k + 1 for its k-th ejection, and
/// differ from ejection to ejection and body to body.
#[test]
fn a_bodys_scatter_draws_are_its_own_stream_words() {
    let system = crate::id::SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let (b, c) = (
        crate::id::BodyId::new(system, 0x0100),
        crate::id::BodyId::new(system, 0x0200),
    );
    let seed = crate::Seed::new(7);
    let draws = ScatterDraws::Stream { seed, body: b };
    let mut stream = Stream::open(seed, tags::PLANET_SCATTER, ObjectKey::from(b));
    stream.seek(4);
    let expected = (stream.uniform_open(), stream.uniform_open());
    assert_eq!(draws.ranks(1), expected);
    assert_eq!(draws.ranks(1), draws.ranks(1));
    assert_ne!(draws.ranks(0), draws.ranks(1));
    assert_ne!(
        draws.ranks(0),
        ScatterDraws::Stream { seed, body: c }.ranks(0)
    );
    assert_eq!(ScatterDraws::Median.ranks(3), (0.5, 0.0));
}

fn axis_au_of(orbit: &KeplerElements) -> f64 {
    orbit.semi_major_axis().value() / METRES_PER_AU
}

/// Six Earths packed from 20 au: every pair crosses after the death, the first mergers grow the
/// heavier body until its Θ passes 1, and each pass removes one body, until the survivors are
/// settled.
#[test]
fn a_packed_chain_takes_several_passes_and_ends_settled() {
    let progenitor = black_hole_progenitor();
    let host = FateHost::star(&progenitor);
    let death = death_of(&progenitor).expect("dead");
    let bodies: Vec<FateBody> = (0..6_u8)
        .map(|k| {
            let k = f64::from(k);
            body(
                20.0 * crate::math::powf(1.08, k),
                0.0,
                EarthMasses::new(1.0 + 0.1 * k),
                1.1 * k,
            )
        })
        .collect();
    let refs: Vec<&FateBody> = bodies.iter().collect();
    let (fates, passes) = through_the_death(&refs, &host, death.at);
    let removed = fates.iter().filter(|f| !survives(f, death.at)).count();
    assert!(passes >= 3, "{passes} passes");
    assert_eq!(passes, removed, "each pass removes one body");
    assert!(passes < bodies.len());
    let survivors: Vec<&Segment> = fates
        .iter()
        .filter(|f| survives(f, death.at))
        .map(|f| f.segments.last().unwrap())
        .collect();
    for (n, first) in survivors.iter().enumerate() {
        for second in &survivors[n + 1..] {
            assert!(!unsettled(first, second));
        }
    }
    let collided = fates
        .iter()
        .filter(|f| {
            matches!(
                f.ending,
                Some(BodyState::Destroyed {
                    cause: DestructionCause::Collided,
                    ..
                })
            )
        })
        .count();
    let ejected = fates
        .iter()
        .filter(|f| matches!(f.ending, Some(BodyState::Unbound { .. })))
        .count();
    assert!(
        collided >= 1 && ejected >= 1,
        "{collided} collisions, {ejected} ejections"
    );
    // The full resolution agrees with the step taken by hand.
    let resolved = BodyFate::resolve_all(&refs, &host);
    for (by_hand, full) in fates.iter().zip(&resolved) {
        assert_eq!(by_hand.at(death.at), full.at(death.at));
    }
}

/// Random crowded hosts: the step ends in at most n − 1 passes, leaves no pair unsettled, and
/// keeps the mass: what is present after it and what it ejected sum to what the death left bound.
#[test]
fn the_step_terminates_settles_every_pair_and_conserves_mass() {
    let progenitor = black_hole_progenitor();
    let host = FateHost::star(&progenitor);
    let death = death_of(&progenitor).expect("dead");
    let mut lcg = Lcg::new(0x71);
    let (mut ejections, mut collisions) = (0, 0);
    for _ in 0..60 {
        let n = 2 + usize::try_from(lcg.next_below(9)).unwrap();
        let mut a = 12.0 + 20.0 * lcg.next_f64();
        let bodies: Vec<FateBody> = (0..n)
            .map(|_| {
                a *= 1.02 + 0.3 * lcg.next_f64();
                let mass = crate::math::exp10(-1.0 + 2.5 * lcg.next_f64());
                body(
                    a,
                    0.1 * lcg.next_f64(),
                    EarthMasses::new(mass),
                    6.3 * lcg.next_f64(),
                )
            })
            .collect();
        let refs: Vec<&FateBody> = bodies.iter().collect();
        let bound_before: f64 = refs
            .iter()
            .map(|b| BodyFate::resolve(b, &host))
            .filter(|f| survives(f, death.at))
            .map(|f| f.at(death.at).mass().value())
            .sum();
        let (fates, passes) = through_the_death(&refs, &host, death.at);
        assert!(passes < n, "{passes} passes for {n} bodies");
        let mut after = 0.0;
        for fate in &fates {
            if !survives(&BodyFate::resolve(fate.body, &host), death.at) {
                continue;
            }
            let now = fate.at(death.at);
            match now.state() {
                BodyState::Unbound { .. } => {
                    after += now.mass().value();
                    ejections += 1;
                }
                BodyState::Destroyed {
                    cause: DestructionCause::Collided,
                    ..
                } => collisions += 1,
                // A merger whose orbit plunges takes both masses with it.
                BodyState::Present | BodyState::Destroyed { .. } => after += now.mass().value(),
                BodyState::NotYetFormed => unreachable!("a survivor has formed"),
            }
        }
        assert!(
            (after / bound_before - 1.0).abs() < 1e-12,
            "{after} M⊕ after against {bound_before} M⊕ before"
        );
        let survivors: Vec<&Segment> = fates
            .iter()
            .filter(|f| survives(f, death.at))
            .map(|f| f.segments.last().unwrap())
            .collect();
        for (k, first) in survivors.iter().enumerate() {
            for second in &survivors[k + 1..] {
                assert!(!unsettled(first, second));
            }
        }
    }
    assert!(
        ejections > 10 && collisions > 10,
        "{ejections} ejections, {collisions} collisions"
    );
}

/// A host without a sudden death couples nothing: every body's history among its neighbours is
/// its history alone, bit for bit, even for neighbours closer than 2√3 Hill radii.
#[test]
fn without_a_supernova_every_body_keeps_its_history_alone() {
    let sun = star(1.0, 13.5e9);
    let host = FateHost::star(&sun);
    let bodies: Vec<FateBody> = [(1.5, 1.0), (1.52, 300.0), (3.0, 10.0), (3.01, 0.5)]
        .into_iter()
        .map(|(a, m)| {
            let b = body(a, 0.0, EarthMasses::new(m), 0.0);
            let orbit = KeplerElements::from_semi_major_axis(
                b.orbit().semi_major_axis(),
                GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
                Eccentricity::CIRCULAR,
                *b.orbit().orientation(),
                Radians::ZERO,
            )
            .unwrap();
            FateBody::new(*b.formation(), orbit, b.mass(), b.density()).unwrap()
        })
        .collect();
    let refs: Vec<&FateBody> = bodies.iter().collect();
    let fates = BodyFate::resolve_all(&refs, &host);
    for (k, (body, fate)) in bodies.iter().zip(&fates).enumerate() {
        let alone = BodyFate::resolve(body, &host);
        assert_eq!(fate, &alone);
        assert_eq!(&BodyFate::resolve_among(&refs, k, &host), fate);
    }
}
