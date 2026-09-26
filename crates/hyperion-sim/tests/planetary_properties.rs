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

#[expect(dead_code, reason = "the property tests read no host's disc")]
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

// --- P14.T22.b: moons and rings of whole generated systems (ruling 83.8) ---

mod satellites {
    //! Moons and rings of whole systems as `planetary::generate` makes them, about the real
    //! hosts of layer C cells near the solar circle (P14.T22.b, as ruling 83.8 amends it).

    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
    use hyperion_sim::id::Layer;
    use hyperion_sim::orbit::KeplerElements;
    use hyperion_sim::planetary::derive::OrbitSense;
    use hyperion_sim::planetary::fate::BodyState;
    use hyperion_sim::planetary::moons::irregular::KOZAI_GAP;
    use hyperion_sim::planetary::moons::{CaptureKind, MoonParent};
    use hyperion_sim::planetary::placement::mutual_hill_radius;
    use hyperion_sim::planetary::record::{BodyRecord, Population, Section};
    use hyperion_sim::planetary::satellites::{Satellite, SatelliteMoon, Satellites};
    use hyperion_sim::planetary::{PlanetarySystem, SystemContext, generate};
    use hyperion_sim::time::{ClockWindow, UniverseTime};
    use hyperion_sim::units::{Metres, SolarMasses, Years};

    use super::planetary_support::galaxy;

    const SEED: Seed = Seed::new(0x5eed_0000_0014_0022);

    /// 2√3, design note 7's gap between one moon's apocentre and the next one's pericentre.
    const GAP: f64 = 3.464_101_615_137_754_6;

    /// The cells of the walk: rows along x at the solar circle, then the rows beside them, then
    /// the planes above and below, a fixed order that stays in the disc near 26,000 ly.
    fn walk() -> impl Iterator<Item = CellKey> {
        let planes = (0_i32..).flat_map(|k| if k == 0 { vec![0] } else { vec![k, -k] });
        planes.take(41).flat_map(|z| {
            (780..=844).flat_map(move |y| {
                (-100..=100).map(move |x| {
                    CellKey::new(Layer::C, [x, y, z]).expect("a cell near the solar circle")
                })
            })
        })
    }

    /// The first `count` systems of [`walk`]'s cells, as each cell and how many of its records
    /// to take.
    fn plan(galaxy: &hyperion_sim::galaxy::Galaxy, count: usize) -> Vec<(CellKey, usize)> {
        let mut records: Vec<SystemRecord> = Vec::new();
        let mut left = count;
        let mut plan = Vec::new();
        for key in walk() {
            generate_cell(galaxy, key, &mut records);
            let take = records.len().min(left);
            if take > 0 {
                plan.push((key, take));
                left -= take;
            }
            if left == 0 {
                return plan;
            }
        }
        panic!(
            "the walk holds {} systems, fewer than {count}",
            count - left
        );
    }

    /// Calls `visit` on each of the first `count` systems of [`walk`], whole, one at a time, so
    /// that a large run holds one system at once.
    pub(super) fn each_system(
        seed: Seed,
        count: usize,
        mut visit: impl FnMut(&SystemContext, &PlanetarySystem),
    ) {
        let galaxy = galaxy(seed);
        let mut records: Vec<SystemRecord> = Vec::new();
        for (key, take) in plan(&galaxy, count) {
            generate_cell(&galaxy, key, &mut records);
            for record in records.iter().take(take) {
                let ctx = SystemContext::from_record(&galaxy, record);
                let system = generate(galaxy.seed(), &ctx);
                visit(&ctx, &system);
            }
        }
    }

    /// What the checks counted.
    #[derive(Debug, Default)]
    pub(super) struct Counts {
        pub(super) moons: usize,
        pub(super) irregulars: usize,
        pub(super) rings: usize,
        pub(super) planets_with_moons: usize,
    }

    /// The times the properties hold at: −H, the epoch and +H.
    fn window() -> [UniverseTime; 3] {
        [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END]
    }

    /// The sense a moon goes round its parent in.
    fn sense(moon: &Satellite) -> OrbitSense {
        match moon.moon() {
            SatelliteMoon::Captured(captured) => captured.sense(),
            SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => OrbitSense::Prograde,
        }
    }

    /// Whether a moon is exempt from the non-crossing rule: an irregular or a rocky planet's small
    /// capture (ruling 83.8).
    fn irregular(moon: &Satellite) -> bool {
        match moon.moon() {
            SatelliteMoon::Captured(captured) => captured.kind() != CaptureKind::Large,
            SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => false,
        }
    }

    /// Asserts P14.T22.b's properties for one planet's satellites `found` in `system` at `t`.
    fn check(
        ctx: &SystemContext,
        system: &PlanetarySystem,
        found: &Satellites,
        t: UniverseTime,
        counts: &mut Counts,
    ) {
        let Some(parent) = found.parent() else {
            return;
        };
        let record = |index| {
            system
                .body_at(ctx, index, t)
                .expect("a generated body resolves")
        };
        let present = |r: &BodyRecord| r.identity().state() == BodyState::Present;
        let mut ordered: Vec<(KeplerElements, &Satellite)> = Vec::new();
        for moon in found.moons() {
            let r = record(moon.index());
            if !present(&r) {
                continue;
            }
            let orbit = *r
                .orbit()
                .ok()
                .expect("a present moon has an orbit")
                .elements();
            let density = r.bulk().ok().expect("a present moon has a bulk").density();
            let e = orbit.eccentricity().value();
            let (apo, peri) = (orbit.apoapsis(), orbit.periapsis());
            let id = moon.index();
            assert!(
                apo < parent.hill_radius_at_pericentre(),
                "{id:?} beyond the Hill radius"
            );
            assert!(
                apo < parent.stability_limit(e, sense(moon)),
                "{id:?} beyond its limit"
            );
            assert!(
                peri > parent.roche_limit_fluid(density),
                "{id:?} inside the Roche limit"
            );
            assert!(peri > parent.radius(), "{id:?} inside its planet");
            counts.moons += 1;
            ordered.push((orbit, moon));
        }
        if !ordered.is_empty() {
            counts.planets_with_moons += 1;
        }
        // Regular, giant-impact and Triton-like moons never cross (T10.a's rule about the planet).
        let mut kept: Vec<&(KeplerElements, &Satellite)> = ordered
            .iter()
            .filter(|(_, moon)| !irregular(moon))
            .collect();
        kept.sort_by(|a, b| {
            a.0.semi_major_axis()
                .value()
                .total_cmp(&b.0.semi_major_axis().value())
        });
        let host = SolarMasses::from(parent.mass());
        for pair in kept.windows(2) {
            let ((inner, mi), (outer, mo)) = (pair[0], pair[1]);
            let hill = mutual_hill_radius(
                mi.mass(),
                mo.mass(),
                host,
                inner.semi_major_axis(),
                outer.semi_major_axis(),
            );
            assert!(
                outer.periapsis() - inner.apoapsis() >= hill * GAP * (1.0 - 1e-9),
                "{:?} crosses {:?} in {:?} at {t:?}: {inner:?} {outer:?} {mi:?} {mo:?} {parent:?}",
                mi.index(),
                mo.index(),
                system.system()
            );
        }
        // Ruling 83.8: each irregular clears every other moon and avoids the Kozai gap.
        let clear_of = kept
            .iter()
            .map(|(orbit, moon)| match moon.moon() {
                SatelliteMoon::GiantImpact(impact) => {
                    let end = Years::new(ctx.age_at(ClockWindow::END).value());
                    impact.orbit_at(parent, end).apoapsis()
                }
                SatelliteMoon::Regular(_) | SatelliteMoon::Captured(_) => orbit.apoapsis(),
            })
            .fold(Metres::new(0.0), |far, a| if a > far { a } else { far });
        let (lo, hi) = (KOZAI_GAP.0.value(), KOZAI_GAP.1.value());
        for (orbit, moon) in ordered.iter().filter(|(_, moon)| irregular(moon)) {
            assert!(
                orbit.periapsis() > clear_of,
                "{:?} reaches the regular moons",
                moon.index()
            );
            let local = moon.local_orbit_at(parent, ctx.age_at(t));
            let degrees = local.inclination().value().to_degrees();
            assert!(
                !(lo < degrees && degrees < hi),
                "{:?} at {degrees}°",
                moon.index()
            );
            counts.irregulars += 1;
        }
        check_rings(ctx, system, found, parent, t, counts);
    }

    /// Asserts that `found`'s rings about `parent` lie inside its Roche limits at `t` (P14.T20).
    fn check_rings(
        ctx: &SystemContext,
        system: &PlanetarySystem,
        found: &Satellites,
        parent: &MoonParent,
        t: UniverseTime,
        counts: &mut Counts,
    ) {
        let present = |r: &BodyRecord| r.identity().state() == BodyState::Present;
        // Rings inside Roche limits (P14.T20).
        for ring in found.rings() {
            let r = system
                .body_at(ctx, ring.index(), t)
                .expect("a generated body resolves");
            if !present(&r) {
                continue;
            }
            let Section::Ok(Population::Ring(ring)) = r.population() else {
                panic!("a present ring carries its extent");
            };
            assert!(ring.outer_edge() <= parent.roche_limit_fluid(ring.material().density()));
            assert!(ring.inner_edge() >= parent.radius());
            counts.rings += 1;
        }
    }

    /// Checks every planet's satellites of the first `count` systems of [`each_system`] at −H,
    /// the epoch and +H.
    pub(super) fn check_all(seed: Seed, count: usize) -> Counts {
        let galaxy = galaxy(seed);
        let plan = plan(&galaxy, count);
        // The cells are shared out among threads, each counting its own; the checks are per
        // system and the counts are summed, so the result is the same on any number of threads.
        let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
        let (galaxy, plan) = (&galaxy, &plan);
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..threads)
                .map(|k| {
                    scope.spawn(move || {
                        let mut counts = Counts::default();
                        let mut records: Vec<SystemRecord> = Vec::new();
                        for &(key, take) in plan.iter().skip(k).step_by(threads) {
                            generate_cell(galaxy, key, &mut records);
                            for record in records.iter().take(take) {
                                let ctx = SystemContext::from_record(galaxy, record);
                                let system = generate(galaxy.seed(), &ctx);
                                for found in system.satellites() {
                                    for t in window() {
                                        check(&ctx, &system, found, t, &mut counts);
                                    }
                                }
                            }
                        }
                        counts
                    })
                })
                .collect();
            workers.into_iter().fold(Counts::default(), |sum, worker| {
                let part = worker.join().expect("a worker's checks hold");
                Counts {
                    moons: sum.moons + part.moons,
                    irregulars: sum.irregulars + part.irregulars,
                    rings: sum.rings + part.rings,
                    planets_with_moons: sum.planets_with_moons + part.planets_with_moons,
                }
            })
        })
    }

    /// The Solar-like golden system of `planetary_golden` (P14.T32), in its own universe.
    const SOLAR_LIKE: u64 = 0x4200_aca2_0000_0003;

    /// P14.T22's statistics for the report, printed, not asserted beyond their presence: moons
    /// per giant, the share of cold and hot giants with a massive ring (0.15 and 0.03, P14.T20),
    /// the share of single FGK main-sequence hosts of 1–10 Gyr with a detected cold belt
    /// (0.15–0.30, P14.T21.b), and the Solar-like golden's moons and belts.
    #[test]
    #[ignore = "slow: counts the satellites and belts of 100,000 whole systems"]
    fn moons_and_rings_statistics_slow() {
        use hyperion_sim::planetary::belts::DETECTION_THRESHOLD;
        use hyperion_sim::planetary::derive::PlanetClass;
        use hyperion_sim::planetary::record::BeltKind;
        use hyperion_sim::planetary::rings::{ICY_RING_TEMPERATURE, RingKind};
        use hyperion_sim::stellar::Phase;

        let count = 100_000;
        let (mut gas, mut gas_moons, mut ice, mut ice_moons) = (0_u32, 0_u32, 0_u32, 0_u32);
        let (mut cold, mut cold_massive, mut hot, mut hot_massive) = (0_u32, 0_u32, 0_u32, 0_u32);
        let (mut fgk, mut fgk_cold_belt) = (0_u32, 0_u32);
        each_system(Seed::new(0x5eed_0000_0014_2023), count, |ctx, system| {
            for found in system.satellites() {
                let Some(parent) = found.parent() else {
                    continue;
                };
                let n = u32::try_from(found.moons().len()).unwrap();
                match parent.class() {
                    PlanetClass::GasGiant => (gas, gas_moons) = (gas + 1, gas_moons + n),
                    PlanetClass::IceGiant => (ice, ice_moons) = (ice + 1, ice_moons + n),
                    PlanetClass::Rocky | PlanetClass::Icy | PlanetClass::SubNeptune => continue,
                }
                let record = system
                    .body_at(ctx, found.parent_index(), UniverseTime::EPOCH)
                    .unwrap();
                let Some(bulk) = record.bulk().ok() else {
                    continue;
                };
                let massive = found.rings().iter().any(|r| r.kind() == RingKind::Massive);
                if bulk.equilibrium_temperature() < ICY_RING_TEMPERATURE {
                    (cold, cold_massive) = (cold + 1, cold_massive + u32::from(massive));
                } else {
                    (hot, hot_massive) = (hot + 1, hot_massive + u32::from(massive));
                }
            }
            let star = &ctx.stars()[0];
            let age = ctx.age_at_epoch();
            let Some(state) = star.state_at(UniverseTime::EPOCH) else {
                return;
            };
            if ctx.stars().len() == 1
                && (0.6..=1.5).contains(&star.initial_mass().value())
                && (1e9..=1e10).contains(&age.value())
                && state.phase() == Phase::MainSequence
            {
                fgk += 1;
                let seen = system.belts().iter().any(|belt| {
                    belt.kind() == BeltKind::Kuiper
                        && belt.detectable_luminosity(Years::new(age.value()), state.luminosity())
                            >= DETECTION_THRESHOLD
                });
                fgk_cold_belt += u32::from(seen);
            }
        });
        let ratio = |a: u32, b: u32| f64::from(a) / f64::from(b.max(1));
        eprintln!(
            "{} systems: {gas} gas giants with {:.2} moons each, {ice} ice giants with {:.2}; \
             massive rings on {cold_massive} of {cold} cold giants ({:.3}) and {hot_massive} of \
             {hot} hot ({:.3}); detected cold belts about {fgk_cold_belt} of {fgk} FGK hosts ({:.3})",
            count,
            ratio(gas_moons, gas),
            ratio(ice_moons, ice),
            ratio(cold_massive, cold),
            ratio(hot_massive, hot),
            ratio(fgk_cold_belt, fgk),
        );
        let galaxy = galaxy(Seed::new(0x5eed_0000_0014_0032));
        let id = hyperion_sim::id::SystemId::from_raw(SOLAR_LIKE).unwrap();
        let ctx = SystemContext::for_system(&galaxy, id).unwrap();
        let system = generate(galaxy.seed(), &ctx);
        for record in system.snapshot_at(&ctx, UniverseTime::EPOCH).bodies() {
            let label = record
                .identity()
                .label()
                .ok()
                .map(|l| l.as_str().to_owned());
            let extent = match record.population().ok() {
                Some(Population::Belt(belt)) => format!(
                    "{:.3}-{:.3} au",
                    belt.inner_edge().value() / 1.495_978_707e11,
                    belt.outer_edge().value() / 1.495_978_707e11
                ),
                Some(Population::CometaryHalo(halo)) => {
                    format!("{:.3e} comets", halo.comets())
                }
                _ => String::new(),
            };
            eprintln!(
                "solar-like {:?} {:?} {label:?} {:?} {extent}",
                record.index(),
                record.identity().kind(),
                record.mass().ok()
            );
        }
        assert!(gas + ice > 500, "{gas} + {ice} giants");
    }

    #[test]
    fn moons_inside_hill_spheres_and_rings_inside_roche_limits() {
        let counts = check_all(SEED, 1_500);
        assert!(
            counts.moons > 300 && counts.irregulars > 30 && counts.rings > 100,
            "{counts:?}"
        );
    }

    #[test]
    #[ignore = "slow: checks the satellites of a million whole systems at ±H"]
    fn moons_inside_hill_spheres_and_rings_inside_roche_limits_slow() {
        let counts = check_all(Seed::new(0x5eed_0000_0014_2022), 1_000_000);
        eprintln!("{counts:?}");
        assert!(counts.moons > 100_000, "{counts:?}");
    }
}
