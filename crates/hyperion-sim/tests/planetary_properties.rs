//! Plan 14's property tests of derivation on placed planets (P14.T16.b): no planet hotter than its
//! star, and a body's radius, temperatures and envelope continuous in time across ±H.
//!
//! The hosts are `planetary_support`'s, made as P14.T30.a makes them. Each star is plan 06's
//! `StarModel` at the system's age, and each body goes through P14.T16.a's `derive_body` as the
//! generator derives it: about the stars its zone orbits (companions outside the zone left out),
//! each with its X-ray and ultraviolet history (P14.T1.a) and its largest past luminosity, with
//! its radius rank and volatile ranks drawn here from a fixed generator.

#[expect(dead_code, reason = "the property tests read only part of the support")]
mod planetary_support;

use hyperion_sim::planetary::architecture::template::EARTH_MASSES_PER_JUPITER_MASS;
use hyperion_sim::planetary::context::XuvHistory;
use hyperion_sim::planetary::derive::atmosphere::{Retention, SurfaceState, VolatileDraws};
use hyperion_sim::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
use hyperion_sim::planetary::placement::PlacedPlanet;
use hyperion_sim::stellar::draws::{StarDraws, UnitUniform};
use hyperion_sim::stellar::multiplicity::MultiplicityContext;
use hyperion_sim::stellar::sse::{MIN_INITIAL_MASS, ZCoeffs, zams};
use hyperion_sim::stellar::substellar;
use hyperion_sim::stellar::system::{StarModel, draw_metallicity};
use hyperion_sim::stellar::{Phase, StarState};
use hyperion_sim::time::{CLOCK_WINDOW_H, UniverseTime};
use hyperion_sim::units::{EarthMasses, EarthRadii, Kelvin, Kilograms, Years};
use hyperion_sim::{Seed, math};
use hyperion_testkit::lcg::Lcg;
use planetary_support::{Host, System, galaxy, generate, record};

const SEED: Seed = Seed::new(0x5eed_0000_0014_0016);

/// The half-width of the clock window, H, in whole years: 1,000.
fn window() -> i64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "H is a whole thousand years, exact in both types"
    )]
    let years = CLOCK_WINDOW_H.as_julian_years_f64() as i64;
    years
}

/// The instant `years` whole years from the epoch.
fn at(years: i64) -> UniverseTime {
    UniverseTime::from_julian_years(years).expect("inside the clock window")
}

/// Every star of `system` as plan 06's model at its age, and its X-ray and ultraviolet history
/// from its zero-age luminosity, as `SystemContext::xuv_histories` gives it.
fn stars(system: &System, seed: Seed) -> Vec<(StarModel, XuvHistory)> {
    let coeffs = ZCoeffs::new(system.composition.z_fit());
    system
        .hierarchy
        .stars()
        .iter()
        .map(|slot| {
            let mass = slot.initial_mass();
            let draws = StarDraws::for_star(seed, slot.body());
            let model = StarModel::new(mass, system.composition, draws, system.age)
                .expect("a sampled star's mass and age are in range");
            let zams_luminosity = if mass >= MIN_INITIAL_MASS {
                zams::luminosity(mass, &coeffs)
            } else {
                substellar::cooling(mass, Years::new(5e9), &system.composition)
                    .expect("a star below 0.1 M_sun lies inside the cooling fits")
                    .luminosity()
            };
            (model, XuvHistory::new(mass, zams_luminosity))
        })
        .collect()
}

/// A body as derived at one time: its temperatures, radius, envelope fraction and surface state.
#[derive(Debug, Clone, Copy)]
struct Derived {
    equilibrium: Kelvin,
    surface: Kelvin,
    radius: EarthRadii,
    envelope: f64,
    state: SurfaceState,
    retention: Retention,
}

/// The body `p` of `host`, with radius rank `rank` and volatile ranks `volatiles`, at `t`, about
/// its zone's stars in `stars`, whose states then are `states`; and the hottest of those stars'
/// effective temperatures. `None` about a black hole, or before the body forms.
fn derive_at(
    system: &System,
    host: &Host,
    p: &PlacedPlanet,
    (rank, volatiles): (UnitUniform, VolatileDraws),
    stars: &[(StarModel, XuvHistory)],
    states: &[Option<StarState>],
    t: UniverseTime,
) -> Option<(Derived, Kelvin)> {
    let mut lights = Vec::new();
    let mut hottest = Kelvin::ZERO;
    for m in host.zone.members() {
        let state = states[usize::from(m)]?;
        if state.phase() == Phase::BlackHole {
            return None;
        }
        let (model, xuv) = &stars[usize::from(m)];
        lights.push(
            HostLight::new(
                state.luminosity(),
                state.effective_temperature(),
                state.radius(),
            )
            .expect("a star's state is finite and not negative")
            .with_history(*xuv, model.max_luminosity_until(t)),
        );
        if state.effective_temperature() > hottest {
            hottest = state.effective_temperature();
        }
    }
    let hosts = BodyHosts::new(
        Kilograms::from(host.zone.host_mass()),
        system.composition,
        &lights,
        &[],
    )
    .expect("a zone's mass is positive and it orbits a star");
    let placed = PlacedBody::new(p.mass(), *p.orbit(), p.formation_distance(), rank)
        .expect("a placed planet's mass and formation distance are positive")
        .with_volatiles(volatiles);
    let disc = host.disc.profile()?;
    let body = derive_body(&placed, &hosts, disc, system.age, t).ok()?;
    let air = body.atmosphere();
    Some((
        Derived {
            // What the body radiates at: a giant's effective temperature, its internal heat
            // included (ruling 112.7), and any other body's equilibrium one.
            equilibrium: body
                .effective_temperature()
                .unwrap_or_else(|| body.equilibrium_temperature()),
            surface: air.surface_temperature(),
            radius: body.radius(),
            envelope: body.fractions().envelope(),
            state: air.state(),
            retention: air.retention(),
        },
        hottest,
    ))
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

/// The stars' states and the bodies' derivations at one step of the continuity test.
type Step = (Vec<Option<StarState>>, Vec<Option<Derived>>);

/// A rank drawn from `lcg`.
fn rank(lcg: &mut Lcg) -> UnitUniform {
    UnitUniform::new(lcg.next_f64().clamp(1e-12, 1.0 - 1e-12)).expect("inside (0, 1)")
}

/// A body's radius rank and volatile ranks, drawn here: `planet.radius` and `planet.volatiles`
/// are the generator's.
fn ranks(lcg: &mut Lcg) -> (UnitUniform, VolatileDraws) {
    let radius = rank(lcg);
    let volatiles = VolatileDraws {
        water: rank(lcg),
        carbon: rank(lcg),
        nitrogen: rank(lcg),
    };
    (radius, volatiles)
}

/// P14.T16.b: no planet hotter than its star. For every sampled body and time in ±H about ages of
/// 0.1, 1, 5 and 12 Gyr, its equilibrium temperature, a giant's effective temperature (internal
/// heat included), lies below the hottest effective temperature of the stars it orbits at the same
/// time, and its surface temperature at most at it, where `atmosphere` saturates it (ruling
/// 133.5; the saturated share is printed as a finding and held to at most 10⁻³); hosts that are
/// black holes are left out.
#[test]
fn no_planet_hotter_than_its_star() {
    let galaxy = galaxy(SEED);
    let mut lcg = Lcg::new(0x0016_b0d1);
    let (mut bodies, mut giants, mut airless, mut aired) = (0_u32, 0_u32, 0_u32, 0_u32);
    let (mut samples, mut saturated) = (0_u32, 0_u32);
    for (k, age) in [1e8, 1e9, 5e9, 1.2e10].into_iter().enumerate() {
        let first = 100_000 * u32::try_from(k).expect("four ages");
        for system in systems(&galaxy, first, 600, Years::new(age)) {
            let stars = stars(&system, galaxy.seed());
            for host in &system.hosts {
                for p in host.placement.planets() {
                    let drawn = ranks(&mut lcg);
                    for t in [-window(), 0, window()] {
                        let t = at(t);
                        let states: Vec<Option<StarState>> =
                            stars.iter().map(|(s, _)| s.state_at(t)).collect();
                        let Some((body, hottest)) =
                            derive_at(&system, host, p, drawn, &stars, &states, t)
                        else {
                            continue;
                        };
                        // The surface saturates at the hottest host's effective temperature
                        // (`atmosphere`'s cap), so it may reach it but never pass it.
                        assert!(body.equilibrium < hottest, "{body:?} against {hottest:?}");
                        for temperature in [body.equilibrium, body.surface] {
                            assert!(
                                temperature <= hottest,
                                "{:?} {:?}: {body:?} against {hottest:?}",
                                system.id,
                                p.index(),
                            );
                        }
                        samples += 1;
                        saturated += u32::from(body.surface >= hottest);
                        if body.state == SurfaceState::Airless {
                            airless += 1;
                        } else {
                            aired += 1;
                        }
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
        bodies > 2_000 && giants > 100 && airless > 100 && aired > 100,
        "{bodies} bodies, {giants} giants, {airless} airless, {aired} with air"
    );
    // Ruling 133.5: the host's temperature bounds the grey greenhouse, and a surface held at it
    // is the model saturating, not a climate (the missing physics is volatiles dissolving into a
    // melt, a later P14.T13 task). A finding, held to at most 10⁻³ of the samples.
    let share = f64::from(saturated) / f64::from(samples);
    eprintln!(
        "FINDING surfaces saturated at their host's temperature: {saturated} of {samples} ({share:.2e})"
    );
    assert!(share <= 1e-3, "{saturated} of {samples} surfaces saturated");
}

/// P14.T16.b: a body's radius, temperatures and envelope fraction are continuous in time across
/// ±H: at steps of a year, no relative jump reaches 10⁻³ except where a star it orbits changes
/// phase in that step, or the body's surface state or the gases it retains change (a recorded
/// state change).
#[test]
fn radius_temperature_and_envelope_are_continuous_in_time() {
    let galaxy = galaxy(SEED);
    let mut lcg = Lcg::new(0x0016_b0d2);
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
                let drawn: Vec<_> = planets.iter().map(|_| ranks(&mut lcg)).collect();
                let mut previous: Option<Step> = None;
                for step in -window()..=window() {
                    let t = at(step);
                    let states: Vec<Option<StarState>> =
                        stars.iter().map(|(s, _)| s.state_at(t)).collect();
                    let now: Vec<Option<Derived>> = planets
                        .iter()
                        .zip(&drawn)
                        .map(|(p, &d)| {
                            derive_at(&system, host, p, d, &stars, &states, t).map(|d| d.0)
                        })
                        .collect();
                    if let Some((before_states, before)) = &previous {
                        let changed = host.zone.members().any(|m| {
                            let phase =
                                |s: &[Option<StarState>]| s[usize::from(m)].map(|s| s.phase());
                            phase(before_states) != phase(&states)
                        });
                        for (b, n) in before.iter().zip(&now) {
                            let (Some(b), Some(n)) = (b, n) else {
                                continue;
                            };
                            steps += 1;
                            if changed || b.state != n.state || b.retention != n.retention {
                                continue;
                            }
                            let jump = |x: f64, y: f64| ((y - x) / x).abs();
                            assert!(jump(b.equilibrium.value(), n.equilibrium.value()) < 1e-3);
                            assert!(jump(b.surface.value(), n.surface.value()) < 1e-3);
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

/// The ages the small planets of [`small_planets`] are derived at.
#[derive(Debug, Clone, Copy)]
enum SampleAge {
    /// Each system's own age, log-uniform over 1–10 Gyr.
    Own,
    /// A tenth of it, log-uniform over 0.1–1 Gyr: the same planets, younger.
    Tenth,
    /// One age for every system.
    Fixed(Years),
}

/// A planet of [`small_planets`]: which one it is (the sampled system, its host and its place
/// there, the same at every [`SampleAge`]), its radius, R⊕, and whether it holds an envelope.
#[derive(Debug, Clone, Copy)]
struct SmallPlanet {
    key: (u32, usize, usize),
    radius: f64,
    enveloped: bool,
}

/// The small planets (1–6 R⊕, ruling 131.2) inside 100 days of the FGK single stars (0.6–1.4 M☉)
/// of `n` sampled systems of ages log-uniform over 1–10 Gyr, each derived at the age `age` says.
/// The same planet has the same key at every age, so two samples pair.
fn small_planets(n: u32, age: SampleAge) -> Vec<SmallPlanet> {
    let galaxy = galaxy(Seed::new(0x5eed_0000_0014_016c));
    let mut lcg = Lcg::new(0x0016_c0de);
    let mut planets = Vec::new();
    for i in 0..n {
        let mass = 0.6 + 0.8 * lcg.next_f64();
        let own = Years::new(1e9 * math::exp10(lcg.next_f64()));
        let r = record(&galaxy, 900_000 + i, mass, own);
        let fe_h = draw_metallicity(&galaxy, &r).fe_h();
        let mut system = generate(&galaxy, &r, fe_h, MultiplicityContext::Free);
        if system.hierarchy.star_count() != 1 {
            continue;
        }
        system.age = match age {
            SampleAge::Own => own,
            SampleAge::Tenth => Years::new(own.value() / 10.0),
            SampleAge::Fixed(years) => years,
        };
        let stars = stars(&system, galaxy.seed());
        let t = at(0);
        let states: Vec<Option<StarState>> = stars.iter().map(|(s, _)| s.state_at(t)).collect();
        for (h, host) in system.hosts.iter().enumerate() {
            for (k, p) in host.placement.planets().iter().enumerate() {
                let drawn = ranks(&mut lcg);
                let days = p.orbit().period().value() / 86_400.0;
                if days >= 100.0 {
                    continue;
                }
                let Some((body, _)) = derive_at(&system, host, p, drawn, &stars, &states, t) else {
                    continue;
                };
                let radius = body.radius.value();
                if (1.0..6.0).contains(&radius) {
                    planets.push(SmallPlanet {
                        key: (i, h, k),
                        radius,
                        enveloped: body.envelope > 0.0,
                    });
                }
            }
        }
    }
    planets
}

/// The planets of `planets` of 1–4 R⊕, as radius and envelope, which the histogram, the valley
/// and Berger et al.'s ratio read (ruling 131.2: `bins` folds every larger one into its last bin).
fn up_to_four(planets: &[SmallPlanet]) -> Vec<(f64, bool)> {
    planets
        .iter()
        .filter(|p| p.radius < 4.0)
        .map(|p| (p.radius, p.enveloped))
        .collect()
}

/// Rogers and Owen's (2021) ratio of planets up to 1.8 R⊕ to those of 1.8–6 R⊕ among `planets`
/// (ruling 131.2), which contraction alone can only raise.
fn rogers_owen_ratio(planets: &[SmallPlanet]) -> f64 {
    let below = planets.iter().filter(|p| p.radius < 1.8).count();
    let above = planets.len() - below;
    f64::from(u32::try_from(below).expect("a sample fits a u32"))
        / f64::from(u32::try_from(above).expect("a sample fits a u32"))
}

/// Ruling 131.2's diagnostics of the same planets at a tenth of their age (`middle`) and at their
/// age (`old`), and of `young` at 10 Myr: the share of the pairs enveloped then and bare now, the
/// pairs that cross 3.5 R⊕ downward, and the bare share of 1–1.8 R⊕ planets at 10 Myr.
fn age_diagnostics(middle: &[SmallPlanet], old: &[SmallPlanet], young: &[SmallPlanet]) {
    let then: std::collections::BTreeMap<_, _> = middle.iter().map(|p| (p.key, *p)).collect();
    let pairs: Vec<(SmallPlanet, SmallPlanet)> = old
        .iter()
        .filter_map(|now| then.get(&now.key).map(|before| (*before, *now)))
        .collect();
    let count = |n: usize| f64::from(u32::try_from(n).expect("a sample fits a u32"));
    let stripped = pairs
        .iter()
        .filter(|(before, now)| before.enveloped && !now.enveloped)
        .count();
    let crossing = pairs
        .iter()
        .filter(|(before, now)| before.radius >= 3.5 && now.radius < 3.5)
        .count();
    let small_young: Vec<&SmallPlanet> = young.iter().filter(|p| p.radius < 1.8).collect();
    let bare_young = small_young.iter().filter(|p| !p.enveloped).count();
    eprintln!(
        "paired planets (1-6 R_earth at both ages): {}; enveloped at a tenth of their age and bare \
         at it: {stripped} ({:.4}); crossing 3.5 R_earth downward: {crossing} ({:.4}); bare share \
         of 1-1.8 R_earth at 10 Myr: {bare_young} of {} ({:.4})",
        pairs.len(),
        count(stripped) / count(pairs.len()),
        count(crossing) / count(pairs.len()),
        small_young.len(),
        count(bare_young) / count(small_young.len()),
    );
}

/// The histogram of `planets`' radii in bins of 0.05 dex from 1 R⊕ (Fulton et al. 2017, AJ 154,
/// 109, bin in the logarithm too).
fn bins(planets: &[(f64, bool)]) -> [u32; 12] {
    let mut bins = [0_u32; 12];
    for &(radius, _) in planets {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a bin of a logarithm in [0, 0.6), 0-11"
        )]
        let bin = (math::log10(radius) / 0.05) as usize;
        bins[bin.min(11)] += 1;
    }
    bins
}

/// The ratio of super-Earths (1–1.8 R⊕) to sub-Neptunes (1.8–3.5 R⊕) among `planets`.
fn super_earths_per_sub_neptune(planets: &[(f64, bool)]) -> f64 {
    let count = |range: core::ops::Range<f64>| {
        f64::from(
            u32::try_from(planets.iter().filter(|(r, _)| range.contains(r)).count())
                .expect("a sample fits a u32"),
        )
    };
    count(1.0..1.8) / count(1.8..3.5)
}

/// The median radius of the planets of `planets` that hold an envelope, R⊕.
fn enveloped_median(planets: &[(f64, bool)]) -> f64 {
    let mut radii: Vec<f64> = planets
        .iter()
        .filter(|(_, e)| *e)
        .map(|(r, _)| *r)
        .collect();
    radii.sort_by(f64::total_cmp);
    radii[radii.len() / 2]
}

/// The radius histogram's depth at its valley: the least bin between 1.5 and 2.0 R⊕ (0.176–0.301
/// dex, bins 3–5) against the peaks either side, the largest bin below 1.5 R⊕ and the largest from
/// 2.0 R⊕ up; `(valley, lower peak, upper peak)`.
fn valley_depth(bins: &[u32; 12]) -> (u32, u32, u32) {
    let valley = bins[3..=5].iter().copied().min().unwrap_or(0);
    let lower = bins[..=3].iter().copied().max().unwrap_or(0);
    let upper = bins[6..].iter().copied().max().unwrap_or(0);
    (valley, lower, upper)
}

/// P14.T16.c: the radius valley emerges. Small planets inside 100 days of FGK hosts of 1–10 Gyr,
/// read after escape at the system's age (ruling 119.2), have a bimodal radius distribution with
/// its minimum between 1.5 and 2.0 R⊕ at under two thirds of either peak (Fulton et al. 2017),
/// from escape alone. The envelopes start from ruling 119.1's formation law.
///
/// Ruling 122.1 withdraws the plan's 10 Myr check: under the law's 1% floor 10 Myr already parts
/// bare cores from puffed envelopes, so the young gap is deeper and at 2.0–2.2 R⊕, which is pinned
/// as a finding (the young minimum above 2.0 R⊕, the old one inside it). Two age checks replace
/// it: the super-Earth : sub-Neptune ratio (1–1.8 : 1.8–3.5 R⊕) is larger at 1–10 Gyr than at
/// 0.1–1 Gyr (reported against Berger et al. 2020's 0.61 ± 0.09 → 1.00 ± 0.10 and Rogers and
/// Owen 2021's 0.77 → 0.95), and enveloped planets' median radius is larger at 10 Myr than at
/// 1–10 Gyr (Fernandes et al. 2025). The first does not hold and is pinned as a finding: the
/// valley's drift over gigayears (David et al. 2021) needs a loss channel on that timescale,
/// which the model has not (ruling 131.1 keeps the pin, the old ratio at 0.9–1.0 of the young).
/// Ruling 131.2 adds the like-for-like check: the sample reaches 6 R⊕, and Rogers and Owen's
/// split, up to 1.8 : 1.8–6 R⊕, is larger at 1–10 Gyr than at 0.1–1 Gyr; the histogram, the
/// valley and Berger's ratio still read the planets of 1–4 R⊕.
#[test]
#[ignore = "slow: derives the small planets of 40,000 sampled systems three times"]
fn radius_valley_emerges() {
    let n = 40_000;
    let (old_sample, middle_sample, young_sample) = (
        small_planets(n, SampleAge::Own),
        small_planets(n, SampleAge::Tenth),
        small_planets(n, SampleAge::Fixed(Years::new(1e7))),
    );
    let old_planets = up_to_four(&old_sample);
    let middle_planets = up_to_four(&middle_sample);
    let young_planets = up_to_four(&young_sample);
    let (old, young) = (bins(&old_planets), bins(&young_planets));
    let (ratio_old, ratio_middle) = (
        super_earths_per_sub_neptune(&old_planets),
        super_earths_per_sub_neptune(&middle_planets),
    );
    let (median_old, median_young) = (
        enveloped_median(&old_planets),
        enveloped_median(&young_planets),
    );
    eprintln!("1-10 Gyr: {old:?}\n10 Myr:   {young:?}");
    eprintln!(
        "super-Earths per sub-Neptune: {ratio_middle:.3} at 0.1-1 Gyr, {ratio_old:.3} at 1-10 Gyr \
         (Berger et al. 2020: 0.61 +- 0.09 to 1.00 +- 0.10; Rogers and Owen 2021: 0.77 to 0.95)"
    );
    eprintln!(
        "enveloped planets' median radius: {median_young:.3} R_earth at 10 Myr, {median_old:.3} at \
         1-10 Gyr"
    );
    assert!(old.iter().sum::<u32>() > 2_000, "{old:?}");
    let (valley, lower, upper) = valley_depth(&old);
    assert!(
        3 * valley < 2 * lower && 3 * valley < 2 * upper,
        "no valley at 1.5-2.0 R_earth: {old:?}"
    );
    // A finding (`atmo14`, round 9, for the orchestrator): the ratio does not rise with age, as
    // Berger et al.'s does. Escape here is done within the saturation time, so between 0.1-1 and
    // 1-10 Gyr only the envelopes' contraction moves radii, and it moves sub-Neptunes into the
    // 1.8-3.5 R_earth bin; the valley's gigayear drift needs a loss channel the model has not
    // (ruling 122.1). Pinned as built: the old ratio within 0.9-1.0 of the younger one.
    let drift = ratio_old / ratio_middle;
    assert!(
        (0.9..1.0).contains(&drift),
        "{ratio_middle} then {ratio_old}"
    );
    assert!(
        median_young > median_old,
        "{median_young} then {median_old}"
    );
    // Ruling 131.2: like for like, Rogers and Owen's split (up to 1.8 : 1.8-6 R_earth), which
    // contraction can only raise, is larger at 1-10 Gyr than at 0.1-1 Gyr (their 0.77 to 0.95,
    // x1.23). A failure would be the photoevaporation tail's, a bug: no window is widened.
    let (split_old, split_middle) = (
        rogers_owen_ratio(&old_sample),
        rogers_owen_ratio(&middle_sample),
    );
    eprintln!(
        "up to 1.8 : 1.8-6 R_earth: {split_middle:.3} at 0.1-1 Gyr, {split_old:.3} at 1-10 Gyr, \
         x{:.3} (Rogers and Owen 2021: x1.23); Berger's split x{drift:.3} (Berger x1.64, Sandoval \
         x1.24, Rogers and Owen x1.23)",
        split_old / split_middle
    );
    age_diagnostics(&middle_sample, &old_sample, &young_sample);
    assert!(
        split_old > split_middle,
        "Rogers and Owen's split fell with age: {split_middle} then {split_old}"
    );
    // The pinned finding: the young gap lies above 2.0 R_earth, the old one inside it.
    let deepest = |bins: &[u32; 12]| {
        (3..9)
            .min_by_key(|&i| bins[i])
            .expect("a non-empty range of bins")
    };
    assert!(
        deepest(&young) >= 6,
        "the young gap below 2.0 R_earth: {young:?}"
    );
    assert!(
        deepest(&old) < 6,
        "the old gap not moved inside 2.0 R_earth: {old:?}"
    );
}

// --- P14.T22.b: moons and rings of whole generated systems (ruling 83.8) ---

mod satellites {
    //! Moons and rings of whole systems as `planetary::generate` makes them, about the real
    //! hosts of layer C cells near the solar circle (P14.T22.b, as ruling 83.8 amends it).

    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::Galaxy;
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

    /// What of `moon`'s orbit `orbit` its stability limit holds: a capture's semi-major axis, the
    /// variable Domingos et al.'s limit is fitted on (ruling 133.2), and another moon's apocentre,
    /// as it is placed.
    fn limit_reach(moon: &Satellite, orbit: &KeplerElements) -> Metres {
        match moon.moon() {
            SatelliteMoon::Captured(_) => orbit.semi_major_axis(),
            SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => orbit.apoapsis(),
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
            let reach = limit_reach(moon, &orbit);
            assert!(
                reach < parent.stability_limit(e, sense(moon)),
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
        check_shares(&galaxy, &plan)
    }

    /// Checks the systems of the `k`th of `shares` shares of `plan`, every `shares`th cell.
    fn check_share(galaxy: &Galaxy, plan: &[(CellKey, usize)], k: usize, shares: usize) -> Counts {
        let mut counts = Counts::default();
        let mut records: Vec<SystemRecord> = Vec::new();
        for &(key, take) in plan.iter().skip(k).step_by(shares) {
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
    }

    /// The cells are shared out among threads, each counting its own; the checks are per system
    /// and the counts are summed, so the result is the same on any number of threads. At most
    /// four, the slots `.config/nextest.toml` gives the slow run.
    #[cfg(not(target_family = "wasm"))]
    fn check_shares(galaxy: &Galaxy, plan: &[(CellKey, usize)]) -> Counts {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(4));
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..threads)
                .map(|k| scope.spawn(move || check_share(galaxy, plan, k, threads)))
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

    /// wasm32-wasip1 has no threads, so the whole plan is one share on this thread.
    #[cfg(target_family = "wasm")]
    fn check_shares(galaxy: &Galaxy, plan: &[(CellKey, usize)]) -> Counts {
        check_share(galaxy, plan, 0, 1)
    }

    /// The Solar-like golden system of `planetary_golden` (P14.T32), in its own universe.
    const SOLAR_LIKE: u64 = 0x4200_6cba_0000_0009;

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
