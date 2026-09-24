//! Plan 14's property tests of derivation on placed planets (P14.T16.b): no planet hotter than its
//! star, and a body's radius, temperature and envelope continuous in time across ±H.
//!
//! The hosts are `planetary_support`'s, made as P14.T30.a will make them (`SystemContext` is not
//! built). Plan 06's `StarModel` (P06.T29.a) has not merged here, so each star's state comes from
//! HPT's formulae, plan 06's track integrator, at the ages sampled, and a star under 0.1 M☉ from
//! the substellar cooling fit (`sse::evolve`). The derivation's assembly (P14.T16.a's
//! `derive_body`) is not in this tree either, so each body is derived here from the same pieces,
//! in its order: the flux of the stars its zone orbits (P14.T12.a, companions outside the zone
//! left out), the Bond albedo of 0.3 that holds until P14.T13, the composition solved at the
//! flux of the host's zero-age luminosity (P14.T11.c; ruling 53) at a rank drawn here, since
//! `planet.radius` is P14.T30's, and above 0.3 Jupiter masses plan 13's cooling fit with its
//! inflation and internal heat (P14.T11.d).

mod planetary_support;

use hyperion_sim::Seed;
use hyperion_sim::planetary::architecture::template::EARTH_MASSES_PER_JUPITER_MASS;
use hyperion_sim::planetary::derive::irradiation::with_internal_heat;
use hyperion_sim::planetary::derive::radius::radius_giant;
use hyperion_sim::planetary::derive::{
    BondAlbedo, HostLight, Illumination, composition, equilibrium_temperature, radius_chen_kipping,
};
use hyperion_sim::planetary::placement::PlacedPlanet;
use hyperion_sim::stellar::draws::{StarDraws, UnitUniform};
use hyperion_sim::stellar::multiplicity::MultiplicityContext;
use hyperion_sim::stellar::sse::{MIN_INITIAL_MASS, Track, ZCoeffs, evolve, zams};
use hyperion_sim::stellar::substellar::giant_cooling;
use hyperion_sim::stellar::system::draw_metallicity;
use hyperion_sim::stellar::{Phase, StarState};
use hyperion_sim::time::CLOCK_WINDOW_H;
use hyperion_sim::units::{
    EarthFluxes, EarthMasses, EarthRadii, JupiterMasses, Kelvin, Metres, SolarLuminosities, Watts,
    Years,
};
use hyperion_testkit::lcg::Lcg;
use planetary_support::{Host, System, galaxy, generate, record};

const SEED: Seed = Seed::new(0x5eed_0000_0014_0016);

/// The half-width of the clock window, H, in years: 1,000.
fn window() -> f64 {
    CLOCK_WINDOW_H.as_julian_years_f64()
}

/// One star's state at any age: its track, or for a star under 0.1 M☉ its cooling fit.
struct Star {
    mass: hyperion_sim::units::SolarMasses,
    composition: hyperion_sim::stellar::Composition,
    draws: StarDraws,
    track: Option<Box<Track>>,
}

impl Star {
    fn state_at(&self, age: Years) -> StarState {
        match &self.track {
            Some(track) => track.state_at(age),
            None => evolve(self.mass, &self.composition, &self.draws, age),
        }
    }
}

/// Every star of `system`, built to its age plus H.
fn stars(system: &System, seed: Seed) -> Vec<Star> {
    let until = Years::new(system.age.value() + window() + 1.0);
    system
        .hierarchy
        .stars()
        .iter()
        .map(|slot| {
            let draws = StarDraws::for_star(seed, slot.body());
            let mass = slot.initial_mass();
            let track = (mass >= MIN_INITIAL_MASS)
                .then(|| Box::new(Track::to_age(mass, &system.composition, &draws, until)));
            Star {
                mass,
                composition: system.composition,
                draws,
                track,
            }
        })
        .collect()
}

/// A body as derived at one time: its temperature, radius and envelope fraction.
#[derive(Debug, Clone, Copy)]
struct Derived {
    temperature: Kelvin,
    radius: EarthRadii,
    envelope: f64,
}

/// The formation of a small body: its composition solved at the flux of its host's zero-age
/// luminosity at its primordial orbit, at a drawn rank (ruling 53).
struct Formed {
    radius: EarthRadii,
    envelope: f64,
}

fn formed(system: &System, host: &Host, p: &PlacedPlanet, rank: UnitUniform) -> Formed {
    let coeffs = ZCoeffs::new(system.composition.z_fit());
    let luminosity: f64 = host
        .zone
        .members()
        .map(|m| zams::luminosity(system.star_mass(m), &coeffs).value())
        .sum();
    let orbit = p.orbit();
    // The flux reads the luminosity alone; a solar surface stands in for the host's.
    let light = HostLight::new(
        SolarLuminosities::new(luminosity),
        Kelvin::new(5_772.0),
        hyperion_sim::units::SolarRadii::new(1.0),
    )
    .expect("a zero-age luminosity is positive");
    let flux = Illumination::new(light, orbit.semi_major_axis(), orbit.eccentricity().value())
        .expect("a Kepler orbit's axis is positive")
        .flux();
    let drawn = radius_chen_kipping(p.mass(), rank);
    match composition(p.mass(), drawn, p.formed(), flux) {
        Ok(solved) => Formed {
            radius: solved.radius(),
            envelope: solved.envelope_fraction(),
        },
        // From 0.414 Jupiter masses the giants' own path gives the radius.
        Err(_) => Formed {
            radius: drawn,
            envelope: 1.0,
        },
    }
}

/// The body `p` of `host` at `age`, given its formation and the zone's stars' states then; and
/// the hottest of those stars' effective temperatures. `None` about a dark host.
fn derive_at(
    system: &System,
    host: &Host,
    p: &PlacedPlanet,
    formed: &Formed,
    states: &[StarState],
    age: Years,
) -> Option<(Derived, Kelvin)> {
    let orbit = p.orbit();
    let mut flux = EarthFluxes::ZERO;
    let mut hottest = Kelvin::ZERO;
    for m in host.zone.members() {
        let state = states[usize::from(m)];
        if state.phase() == Phase::BlackHole {
            return None;
        }
        let light = HostLight::new(
            state.luminosity(),
            state.effective_temperature(),
            state.radius(),
        )
        .expect("a star's state is finite and not negative");
        flux = flux
            + Illumination::new(light, orbit.semi_major_axis(), orbit.eccentricity().value())
                .expect("a Kepler orbit's axis is positive")
                .flux();
        if state.effective_temperature() > hottest {
            hottest = state.effective_temperature();
        }
    }
    let irradiated = equilibrium_temperature(flux, BondAlbedo::BEFORE_ATMOSPHERES);
    let jupiters = p.mass().value() / EARTH_MASSES_PER_JUPITER_MASS;
    let derived = if (0.3..=13.0).contains(&jupiters) {
        let interior = giant_cooling(JupiterMasses::new(jupiters), age, &system.composition)
            .expect("a giant of 0.3-13 Jupiter masses at a positive age is inside the fit");
        let giant = radius_giant(p.mass(), &interior, flux).expect("a giant's mass and flux");
        let radius = if jupiters >= 0.414 {
            giant.radius()
        } else {
            giant.blended(formed.radius)
        };
        let heat = Watts::from(giant.internal_luminosity());
        Derived {
            temperature: with_internal_heat(irradiated, heat, Metres::from(radius)),
            radius,
            envelope: formed.envelope,
        }
    } else {
        Derived {
            temperature: irradiated,
            radius: formed.radius,
            envelope: formed.envelope,
        }
    };
    Some((derived, hottest))
}

/// `n` systems of the galaxy's mass function over 0.08–3 M☉, their [Fe/H] drawn, at `age`.
fn systems(galaxy: &hyperion_sim::galaxy::Galaxy, first: u32, n: u32, age: Years) -> Vec<System> {
    let mut lcg = Lcg::new(0x0014_0016 ^ u64::from(first));
    (first..first + n)
        .map(|i| {
            let mass = galaxy
                .mass_function()
                .quantile_in(0.08, 3.0, lcg.next_f64());
            let r = record(galaxy, i, mass, age);
            let fe_h = draw_metallicity(galaxy, &r).fe_h();
            generate(galaxy, &r, fe_h, MultiplicityContext::Free)
        })
        .collect()
}

/// A rank for a body's radius, drawn here: `planet.radius` is P14.T30's (ruling 53).
fn radius_rank(lcg: &mut Lcg) -> UnitUniform {
    UnitUniform::new(lcg.next_f64().clamp(1e-12, 1.0 - 1e-12)).expect("inside (0, 1)")
}

/// P14.T16.b: no planet hotter than its star. For every sampled body and time in ±H about ages of
/// 0.1, 1, 5 and 12 Gyr, its temperature, internal heat included, lies below the hottest
/// effective temperature of the stars it orbits at the same age; hosts that are black holes are
/// left out.
#[test]
fn no_planet_hotter_than_its_star() {
    let galaxy = galaxy(SEED);
    let mut ranks = Lcg::new(0x0016_b0d1);
    let (mut bodies, mut giants) = (0_u32, 0_u32);
    for (k, age) in [1e8, 1e9, 5e9, 1.2e10].into_iter().enumerate() {
        let first = 100_000 * u32::try_from(k).expect("four ages");
        for system in systems(&galaxy, first, 600, Years::new(age)) {
            let stars = stars(&system, galaxy.seed());
            for host in &system.hosts {
                for p in host.placement.planets() {
                    let formation = formed(&system, host, p, radius_rank(&mut ranks));
                    for t in [-window(), 0.0, window()] {
                        let at = Years::new(age + t);
                        let states: Vec<StarState> = stars.iter().map(|s| s.state_at(at)).collect();
                        let Some((body, hottest)) =
                            derive_at(&system, host, p, &formation, &states, at)
                        else {
                            continue;
                        };
                        assert!(
                            body.temperature < hottest,
                            "{:?} {:?}: {:?} against {:?}",
                            system.id,
                            p.index(),
                            body.temperature,
                            hottest
                        );
                    }
                    bodies += 1;
                    if p.mass() >= EarthMasses::new(0.3 * EARTH_MASSES_PER_JUPITER_MASS) {
                        giants += 1;
                    }
                }
            }
        }
    }
    assert!(
        bodies > 2_000 && giants > 100,
        "{bodies} bodies, {giants} giants"
    );
}

/// P14.T16.b: a body's radius, temperature and envelope fraction are continuous in time across
/// ±H: at steps of a year, no relative jump reaches 10⁻³ except where a star it orbits changes
/// phase in that step.
#[test]
fn radius_temperature_and_envelope_are_continuous_in_time() {
    let galaxy = galaxy(SEED);
    let mut ranks = Lcg::new(0x0016_b0d2);
    let mut steps = 0_u64;
    for (k, age) in [1e9, 5e9].into_iter().enumerate() {
        let first = 500_000 + 100_000 * u32::try_from(k).expect("two ages");
        for system in systems(&galaxy, first, 60, Years::new(age)) {
            let stars = stars(&system, galaxy.seed());
            for host in &system.hosts {
                let planets = host.placement.planets();
                if planets.is_empty() {
                    continue;
                }
                let formations: Vec<Formed> = planets
                    .iter()
                    .map(|p| formed(&system, host, p, radius_rank(&mut ranks)))
                    .collect();
                let mut previous: Option<(Vec<StarState>, Vec<Option<Derived>>)> = None;
                for step in 0..=2_000_u32 {
                    let at = Years::new(age - window() + f64::from(step));
                    let states: Vec<StarState> = stars.iter().map(|s| s.state_at(at)).collect();
                    let now: Vec<Option<Derived>> = planets
                        .iter()
                        .zip(&formations)
                        .map(|(p, f)| derive_at(&system, host, p, f, &states, at).map(|d| d.0))
                        .collect();
                    if let Some((before_states, before)) = &previous {
                        let changed = host.zone.members().any(|m| {
                            before_states[usize::from(m)].phase() != states[usize::from(m)].phase()
                        });
                        for (b, n) in before.iter().zip(&now) {
                            let (Some(b), Some(n)) = (b, n) else {
                                continue;
                            };
                            steps += 1;
                            if changed {
                                continue;
                            }
                            let jump = |x: f64, y: f64| ((y - x) / x).abs();
                            assert!(jump(b.temperature.value(), n.temperature.value()) < 1e-3);
                            assert!(jump(b.radius.value(), n.radius.value()) < 1e-3);
                            assert!((n.envelope - b.envelope).abs() < 1e-3 * b.envelope.max(1e-3));
                        }
                    }
                    previous = Some((states, now));
                }
            }
        }
    }
    assert!(steps > 50_000, "{steps}");
}
