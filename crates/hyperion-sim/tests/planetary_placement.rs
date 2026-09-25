//! Placement tests of plan 14 (P14.T10): no overlapping orbits on sampled hosts (T10.a), the
//! statistics of architecture against their surveys (T10.b), and a golden of placed planets.
//!
//! `SystemContext` (P14.T1.d) is not built, so every host is made as P14.T30.a will make it
//! (`planetary_support`): a record at the Sun-like point of a chosen primary mass, \[Fe/H\] drawn
//! for it by plan 06 or set by the test, its hierarchy drawn by plan 11, the zones of that
//! hierarchy (P14.T9) through `ZoneHierarchy::from(&SystemHierarchy)`, each zone's disc from its
//! components' zero-age `zams` values, and each zone's class and planets. A survey's "per star"
//! counts the planets of every zone that contains the primary.
//!
//! The statistics are asserted on the planets' masses where the survey counts radii, 1–20 M⊕ for
//! 1–4 R⊕, as P14.T10.b allows until the derivation is assembled: the radius rank is drawn on
//! `planet.radius`, which P14.T30 registers (ruling 53).

mod planetary_support;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::planetary::architecture::template::{EARTH_MASSES_PER_JUPITER_MASS, GroupRole};
use hyperion_sim::planetary::architecture::{ArchitectureClass, HostMultiplicity};
use hyperion_sim::planetary::derive::habitable_zone;
use hyperion_sim::planetary::derive::radius::radius_chen_kipping;
use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
use hyperion_sim::planetary::params::{HILL_STABLE_GAP, SPACING_GIANT_MASS};
use hyperion_sim::planetary::placement::classes::orbits::HostPlane;
use hyperion_sim::planetary::placement::classes::tides::{
    GIANT_TIDAL_Q_PRIME, ROCKY_TIDAL_Q_PRIME, circularisation_time, circularise,
};
use hyperion_sim::planetary::placement::{PlacedPlanet, PlacementHost, mutual_hill_radius, place};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::UnitUniform;
use hyperion_sim::stellar::multiplicity::MultiplicityContext;
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::stellar::system::draw_metallicity;
use hyperion_sim::time::CLOCK_WINDOW_H;
use hyperion_sim::units::consts::{
    EARTH_MASS_KG, GRAVITATIONAL_CONSTANT, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_MASS_KG,
};
use hyperion_sim::units::{
    Days, Dex, EarthMasses, HeliumExcess, Kelvin, Megayears, Metres, SolarMasses, Years,
};
use hyperion_sim::{GENERATOR_VERSION, math};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use planetary_support::{Host, System, galaxy, generate, record};

const SEED: Seed = Seed::new(0x5eed_0000_0014_0010);

/// Whether a host's zone contains the primary, alone or with companions.
fn orbits_primary(host: &Host) -> bool {
    host.zone.members().any(|m| m == 0)
}

/// A host's planets in order of semi-major axis.
fn sorted(host: &Host) -> Vec<PlacedPlanet> {
    let mut planets = host.placement.planets().to_vec();
    planets.sort_by(|a, b| {
        a.orbit()
            .semi_major_axis()
            .total_cmp(&b.orbit().semi_major_axis())
    });
    planets
}

/// The planets whose zone contains the primary, as a survey of the primary counts them.
fn primary_planets(system: &System) -> impl Iterator<Item = (&Host, &PlacedPlanet)> {
    system
        .hosts
        .iter()
        .filter(|h| orbits_primary(h))
        .flat_map(|h| h.placement.planets().iter().map(move |p| (h, p)))
}

/// The age every sampled system has at the epoch: 5 Gyr.
const AGE: Years = Years::new(5e9);

fn period_days(p: &PlacedPlanet) -> f64 {
    Days::from(p.orbit().period()).value()
}

fn is_giant(p: &PlacedPlanet) -> bool {
    p.mass() >= EarthMasses::from(SPACING_GIANT_MASS)
}

fn jupiters(p: &PlacedPlanet) -> f64 {
    p.mass().value() / EARTH_MASSES_PER_JUPITER_MASS
}

/// `n` systems from record `first` on, their primaries' masses from `mass` and \[Fe/H\] from
/// `fe_h`, each drawn with its companions.
fn sample(
    galaxy: &Galaxy,
    first: u32,
    n: u32,
    mut mass: impl FnMut(&mut Lcg) -> f64,
    mut fe_h: impl FnMut(&Galaxy, &hyperion_sim::galaxy::placement::SystemRecord, &mut Lcg) -> Dex,
) -> Vec<System> {
    let mut lcg = Lcg::new(0x0014_0010 ^ u64::from(first));
    (first..first + n)
        .map(|i| {
            let r = record(galaxy, i, mass(&mut lcg), AGE);
            let x = fe_h(galaxy, &r, &mut lcg);
            generate(galaxy, &r, x, MultiplicityContext::Free)
        })
        .collect()
}

/// Primaries from the galaxy's mass function over `lo`–`hi` M☉.
fn imf(galaxy: &Galaxy, lo: f64, hi: f64) -> impl FnMut(&mut Lcg) -> f64 + '_ {
    move |lcg| galaxy.mass_function().quantile_in(lo, hi, lcg.next_f64())
}

/// Primaries uniform on `lo`–`hi` M☉.
fn uniform(lo: f64, hi: f64) -> impl FnMut(&mut Lcg) -> f64 {
    move |lcg| lo + (hi - lo) * lcg.next_f64()
}

/// \[Fe/H\] as plan 06 draws it for the record.
fn drawn(galaxy: &Galaxy, r: &hyperion_sim::galaxy::placement::SystemRecord, _: &mut Lcg) -> Dex {
    draw_metallicity(galaxy, r).fe_h()
}

/// The damping time of a planet's eccentricity (P14.T8.e), with Chen and Kipping's median radius
/// at its mass and the Q′ of a giant or a rocky body.
fn damping(p: &PlacedPlanet, host: SolarMasses) -> Years {
    let radius = Metres::from(radius_chen_kipping(p.mass(), UnitUniform::HALF));
    let q = if is_giant(p) {
        GIANT_TIDAL_Q_PRIME
    } else {
        ROCKY_TIDAL_Q_PRIME
    };
    let orbit = p.orbit();
    circularisation_time(
        p.mass(),
        radius,
        host,
        orbit.semi_major_axis(),
        orbit.period(),
        q,
    )
}

/// P14.T10.a on one system: every planet inside its zone, and every pair sharing a host with the
/// inner apocentre at least 2√3 mutual Hill radii below the outer pericentre, at the epoch and
/// at ±H, the orbits circularised to the system's age then (P14.T8.e). Returns the pairs checked.
fn assert_no_overlap(system: &System) -> u64 {
    let window = Years::new(CLOCK_WINDOW_H.as_julian_years_f64());
    let mut pairs = 0;
    for host in &system.hosts {
        let m = host.zone.host_mass();
        let sorted = sorted(host);
        for p in &sorted {
            let orbit = p.orbit();
            if let Some(inner) = host.zone.inner() {
                assert!(
                    orbit.periapsis() >= inner * (1.0 - 1e-12),
                    "{:?}",
                    system.id
                );
            }
            if let Some(outer) = host.zone.outer() {
                assert!(orbit.apoapsis() <= outer * (1.0 + 1e-12), "{:?}", system.id);
            }
        }
        for t in [system.age - window, system.age, system.age + window] {
            let now: Vec<(EarthMasses, Metres, f64)> = sorted
                .iter()
                .map(|p| {
                    let orbit = p.orbit();
                    let (a, e) = circularise(
                        orbit.semi_major_axis(),
                        orbit.eccentricity().value(),
                        damping(p, m),
                        t,
                    );
                    (p.mass(), a, e)
                })
                .collect();
            for pair in now.windows(2) {
                let ((m1, a1, e1), (m2, a2, e2)) = (pair[0], pair[1]);
                let hill = mutual_hill_radius(m1, m2, m, a1, a2).value();
                let gap = a2.value() * (1.0 - e2) - a1.value() * (1.0 + e1);
                assert!(
                    gap >= HILL_STABLE_GAP * hill * (1.0 - 1e-12),
                    "{:?}: {gap} m against {} Hill radii",
                    system.id,
                    HILL_STABLE_GAP
                );
                pairs += 1;
            }
        }
    }
    pairs
}

/// P14.T10.a: no overlapping orbits, over 4,000 systems of the galaxy's mass function from 0.08 to
/// 3 M☉ at the Sun-like point, their \[Fe/H\] drawn and their companions with them.
#[test]
fn no_overlapping_orbits() {
    let galaxy = galaxy(SEED);
    let systems = sample(&galaxy, 0, 4_000, imf(&galaxy, 0.08, 3.0), drawn);
    let pairs: u64 = systems.iter().map(assert_no_overlap).sum();
    let planets: usize = systems
        .iter()
        .flat_map(|s| &s.hosts)
        .map(|h| h.placement.planets().len())
        .sum();
    let multiples = systems
        .iter()
        .filter(|s| s.hierarchy.star_count() > 1)
        .count();
    assert!(
        planets > 4_000 && pairs > 5_000 && multiples > 800,
        "{planets} {pairs} {multiples}"
    );
}

/// P14.T10.a under `just test-slow`: the same over a million systems (design note 20).
#[test]
#[ignore = "slow: places planets about a million sampled hosts"]
fn no_overlapping_orbits_on_a_million_hosts() {
    let galaxy = galaxy(SEED);
    let mut pairs = 0;
    for batch in 0..100 {
        let first = 10_000 + batch * 10_000;
        let systems = sample(&galaxy, first, 10_000, imf(&galaxy, 0.08, 3.0), drawn);
        pairs += systems.iter().map(assert_no_overlap).sum::<u64>();
    }
    assert!(pairs > 1_000_000, "{pairs}");
}

/// Planets per star of the primaries of `systems` that `counts`.
fn per_star(systems: &[System], counts: impl Fn(&Host, &PlacedPlanet) -> bool) -> f64 {
    let n: usize = systems
        .iter()
        .map(|s| primary_planets(s).filter(|(h, p)| counts(h, p)).count())
        .sum();
    ratio(n, systems.len())
}

/// The share of `systems` whose primary has a planet that `counts`.
fn share_with(systems: &[System], counts: impl Fn(&Host, &PlacedPlanet) -> bool) -> f64 {
    let n = systems
        .iter()
        .filter(|s| primary_planets(s).any(|(h, p)| counts(h, p)))
        .count();
    ratio(n, systems.len())
}

fn ratio(a: usize, b: usize) -> f64 {
    let a = u32::try_from(a).expect("a sample's counts fit a u32");
    let b = u32::try_from(b).expect("a sample's size fits a u32");
    f64::from(a) / f64::from(b)
}

/// A small planet by P14.T10.b's mass proxy for 1–4 R⊕: 1–20 M⊕.
fn small(p: &PlacedPlanet) -> bool {
    (1.0..=20.0).contains(&p.mass().value())
}

/// A giant of Cumming et al.'s window: 0.3–10 Jupiter masses.
fn cumming_giant(p: &PlacedPlanet) -> bool {
    (0.3..=10.0).contains(&jupiters(p))
}

/// The radial-velocity semi-amplitude a planet raises on its host, m s⁻¹, seen along galactic
/// north: (2πG ÷ P)^⅓ m sin i ÷ (M★ + m)^⅔ ÷ √(1 − e²).
fn semi_amplitude(p: &PlacedPlanet, host: SolarMasses) -> f64 {
    let orbit = p.orbit();
    let (m, star) = (
        p.mass().value() * EARTH_MASS_KG,
        host.value() * SOLAR_MASS_KG,
    );
    let e = orbit.eccentricity().value();
    let sin_i = math::sin(orbit.inclination().value()).abs();
    let factor =
        math::cbrt(2.0 * core::f64::consts::PI * GRAVITATIONAL_CONSTANT / orbit.period().value());
    factor * m * sin_i / math::powf(star + m, 2.0 / 3.0) / (1.0 - e * e).sqrt()
}

/// The primary's effective temperature at the zero-age main sequence, from its `zams` luminosity
/// and radius.
fn zams_temperature(mass: SolarMasses, composition: &Composition) -> Kelvin {
    let coeffs = ZCoeffs::new(composition.z_fit());
    let (l, r) = (zams::luminosity(mass, &coeffs), zams::radius(mass, &coeffs));
    Kelvin::new(
        SOLAR_EFFECTIVE_TEMPERATURE_K * math::powf(l.value() / (r.value() * r.value()), 0.25),
    )
}

/// A least-squares slope of `ys` against `xs`, each point weighted.
fn weighted_slope(points: &[(f64, f64, f64)]) -> f64 {
    let total: f64 = points.iter().map(|p| p.2).sum();
    let mx = points.iter().map(|p| p.0 * p.2).sum::<f64>() / total;
    let my = points.iter().map(|p| p.1 * p.2).sum::<f64>() / total;
    let sxy: f64 = points.iter().map(|p| p.2 * (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = points.iter().map(|p| p.2 * (p.0 - mx) * (p.0 - mx)).sum();
    sxy / sxx
}

fn pearson(pairs: &[(f64, f64)]) -> f64 {
    let n = f64::from(u32::try_from(pairs.len()).expect("a few thousand pairs"));
    let (mx, my) = pairs
        .iter()
        .fold((0.0, 0.0), |(x, y), &(a, b)| (x + a / n, y + b / n));
    let (sxy, sxx, syy) = pairs.iter().fold((0.0, 0.0, 0.0), |(xy, xx, yy), &(a, b)| {
        let (dx, dy) = (a - mx, b - my);
        (xy + dx * dy, xx + dx * dx, yy + dy * dy)
    });
    sxy / (sxx * syy).sqrt()
}

/// The statistics of one run, each against its source: asserted inside the source's window, or,
/// where no in-source setting of P14.T4.b's weights reaches it, reported as a finding for the
/// orchestrator and pinned at its as-built value, so that a change shows.
#[derive(Default)]
struct Report {
    lines: Vec<String>,
    failures: Vec<String>,
}

impl Report {
    /// `value` must lie in its source's `window`.
    fn check(&mut self, name: &str, value: f64, window: (f64, f64)) {
        let ok = (window.0..=window.1).contains(&value);
        self.lines.push(format!(
            "{name}: {value:.4}, source [{}, {}]",
            window.0, window.1
        ));
        if !ok {
            self.failures.push(format!(
                "{name} = {value} outside its source's [{}, {}]",
                window.0, window.1
            ));
        }
    }

    /// A finding: `value` misses its source's `window`, and is held to its as-built `pin`.
    fn finding(&mut self, name: &str, value: f64, window: (f64, f64), pin: (f64, f64)) {
        let held = (pin.0..=pin.1).contains(&value);
        self.lines.push(format!(
            "FINDING {name}: {value:.4}, source [{}, {}], as built [{}, {}]",
            window.0, window.1, pin.0, pin.1
        ));
        if !held {
            self.failures.push(format!(
                "{name} = {value} moved from its as-built [{}, {}]",
                pin.0, pin.1
            ));
        }
    }

    /// A figure reported with no window.
    fn note(&mut self, line: String) {
        self.lines.push(line);
    }

    fn finish(self) {
        for line in &self.lines {
            eprintln!("{line}");
        }
        assert!(self.failures.is_empty(), "{:#?}", self.failures);
    }
}

/// The share of `systems` with at least `n` planets about the primary that `counts`.
fn share_with_at_least(
    systems: &[System],
    n: usize,
    counts: impl Fn(&PlacedPlanet) -> bool,
) -> f64 {
    let with = systems
        .iter()
        .filter(|s| primary_planets(s).filter(|(_, p)| counts(p)).count() >= n)
        .count();
    ratio(with, systems.len())
}

/// A rank for a planet's Chen and Kipping radius, drawn here: `planet.radius` is P14.T30's.
fn rank(lcg: &mut Lcg) -> UnitUniform {
    UnitUniform::new(lcg.next_f64().clamp(1e-12, 1.0 - 1e-12)).expect("inside (0, 1)")
}

/// Planets per star of 1–4 R⊕ inside `days`, by Chen and Kipping's radius at a drawn rank.
fn small_by_radius(systems: &[System], days: f64, lcg: &mut Lcg) -> f64 {
    let n: usize = systems
        .iter()
        .map(|s| {
            primary_planets(s)
                .filter(|(_, p)| {
                    let r = radius_chen_kipping(p.mass(), rank(lcg)).value();
                    (1.0..=4.0).contains(&r) && period_days(p) < days
                })
                .count()
        })
        .sum();
    ratio(n, systems.len())
}

/// Weiss and Marcy's (2014) mass of a planet of radius `r` R⊕, in M⊕, as Weiss et al. (2018, AJ
/// 155, 48, §5.2, eqs. 6–9) apply it to turn the California–Kepler Survey's radii into masses for
/// their spacings: a density of 2.43 + 3.39 R g cm⁻³ under 1.5 R⊕, 2.69 R^0.93 to 4 R⊕,
/// 0.86 R^1.89 to 9 R⊕, and 100 M⊕ above.
fn weiss_marcy_mass(r: f64) -> EarthMasses {
    let m = if r < 1.5 {
        (2.43 + 3.39 * r) / 5.51 * r * r * r
    } else if r <= 4.0 {
        2.69 * math::powf(r, 0.93)
    } else if r < 9.0 {
        0.86 * math::powf(r, 1.89)
    } else {
        100.0
    };
    EarthMasses::new(m)
}

/// Adjacent pairs of cold chain planets about the primaries of `systems`.
#[derive(Default)]
struct ChainPairs {
    /// Their log radii, at drawn ranks.
    radii: Vec<(f64, f64)>,
    /// Their spacings in mutual Hill radii, from their own masses.
    spacings: Vec<f64>,
    /// Their spacings in mutual Hill radii from the masses Weiss and Marcy's relation gives their
    /// radii, as Weiss et al. (2018) measure them.
    measured_spacings: Vec<f64>,
}

/// Adjacent pairs of cold chain planets about the primaries of `systems`, their radii drawn from
/// `lcg`.
fn chain_pairs(systems: &[System], lcg: &mut Lcg) -> ChainPairs {
    let mut pairs = ChainPairs::default();
    for s in systems {
        for host in s.hosts.iter().filter(|h| orbits_primary(h)) {
            let chain: Vec<PlacedPlanet> = sorted(host)
                .into_iter()
                .filter(|p| p.role() == GroupRole::Chain && !p.hot())
                .collect();
            let radii: Vec<f64> = chain
                .iter()
                .map(|p| radius_chen_kipping(p.mass(), rank(lcg)).value())
                .collect();
            for (i, pair) in chain.windows(2).enumerate() {
                pairs
                    .radii
                    .push((math::log10(radii[i]), math::log10(radii[i + 1])));
                let (a1, a2) = (
                    pair[0].orbit().semi_major_axis(),
                    pair[1].orbit().semi_major_axis(),
                );
                let spacing = |m1: EarthMasses, m2: EarthMasses| {
                    (a2 - a1) / mutual_hill_radius(m1, m2, host.zone.host_mass(), a1, a2)
                };
                pairs.spacings.push(spacing(pair[0].mass(), pair[1].mass()));
                pairs.measured_spacings.push(spacing(
                    weiss_marcy_mass(radii[i]),
                    weiss_marcy_mass(radii[i + 1]),
                ));
            }
        }
    }
    pairs
}

/// η⊕ of `systems`' primaries of 4,800–6,300 K at the zero-age main sequence: planets of 0.5–1.5
/// R⊕, by the median radius 0.08–2.9 M⊕, in the conservative habitable zone of the primary's
/// zero-age luminosity and temperature (P14.T12.b).
fn eta_earth(systems: &[System]) -> f64 {
    let hosts: Vec<&System> = systems
        .iter()
        .filter(|s| {
            (4_800.0..=6_300.0).contains(&zams_temperature(s.star_mass(0), &s.composition).value())
        })
        .collect();
    let n: usize = hosts
        .iter()
        .map(|s| {
            let mass = s.star_mass(0);
            let coeffs = ZCoeffs::new(s.composition.z_fit());
            let zone = habitable_zone(
                zams::luminosity(mass, &coeffs),
                zams_temperature(mass, &s.composition),
            );
            let (inner, outer) = zone.conservative();
            s.hosts
                .iter()
                .filter(|h| h.zone.members().eq([0]))
                .flat_map(|h| h.placement.planets())
                .filter(|p| {
                    let a = p.orbit().semi_major_axis();
                    (0.08..=2.9).contains(&p.mass().value()) && a >= inner && a <= outer
                })
                .count()
        })
        .sum();
    ratio(n, hosts.len())
}

/// P14.T10.b's FGK statistics and ruling 55.1's, on 60,000 FGK primaries of drawn \[Fe/H\].
fn fgk_statistics(galaxy: &Galaxy, report: &mut Report, ranks: &mut Lcg) -> Vec<System> {
    // FGK stars at the Sun-like point, their [Fe/H] drawn.
    let fgk = sample(galaxy, 2_000_000, 60_000, uniform(0.7, 1.3), drawn);
    let small_close = per_star(&fgk, |_, p| small(p) && period_days(p) < 100.0);
    report.check(
        "small planets per FGK star inside 100 days",
        small_close,
        (0.5, 1.2),
    );
    report.note(format!(
        "  by radius at drawn ranks: {:.4}",
        small_by_radius(&fgk, 100.0, ranks)
    ));
    let hot = share_with(&fgk, |_, p| jupiters(p) > 0.1 && period_days(p) < 10.0);
    report.check("hot Jupiters around FGK stars", hot, (0.004, 0.012));
    let cumming = share_with(&fgk, |_, p| cumming_giant(p) && period_days(p) < 2_000.0);
    report.check(
        "giants of 0.3-10 M_J inside 2,000 days",
        cumming,
        (0.07, 0.14),
    );
    report.check(
        "eta-Earth (Bryson et al. 2021)",
        eta_earth(&fgk),
        (0.37, 0.60),
    );
    let mut pairs = chain_pairs(&fgk, ranks);
    report.check(
        "adjacent log radii's correlation (Weiss et al. 2018)",
        pearson(&pairs.radii),
        (0.60, 0.70),
    );
    let larger = pairs
        .radii
        .iter()
        .filter(|(inner, outer)| outer > inner)
        .count();
    report.check(
        "outer planet the larger (Weiss et al. 2018)",
        ratio(larger, pairs.radii.len()),
        (0.650, 0.658),
    );
    let wide = |spacings: &[f64]| {
        ratio(
            spacings.iter().filter(|&&d| d >= 10.0).count(),
            spacings.len(),
        )
    };
    report.finding(
        "small pairs at 10 mutual Hill radii or more, measured as Weiss et al. (2018) measure them",
        wide(&pairs.measured_spacings),
        (0.92, 0.94),
        (0.99, 1.0),
    );
    pairs.spacings.sort_by(f64::total_cmp);
    pairs.measured_spacings.sort_by(f64::total_cmp);
    report.note(format!(
        "  from their own masses: {:.4}, median {:.2} mutual Hill radii; as measured, median {:.2}",
        wide(&pairs.spacings),
        pairs.spacings[pairs.spacings.len() / 2],
        pairs.measured_spacings[pairs.measured_spacings.len() / 2]
    ));

    fgk
}

/// P14.T10.b's M-dwarf statistics, and ruling 48 (b)'s and (e)'s.
fn m_dwarf_statistics(galaxy: &Galaxy, report: &mut Report, ranks: &mut Lcg) {
    // M dwarfs of Dressing and Charbonneau's sample.
    let m = sample(galaxy, 3_000_000, 30_000, uniform(0.35, 0.6), drawn);
    let per_m = per_star(&m, |_, p| small(p) && period_days(p) < 200.0);
    report.finding(
        "small planets per M dwarf inside 200 days (Dressing and Charbonneau 2015)",
        per_m,
        (1.8, 3.2),
        (1.20, 1.26),
    );
    report.note(format!(
        "  by radius at drawn ranks: {:.4}",
        small_by_radius(&m, 200.0, ranks)
    ));

    // Hosts of 0.1-0.5 M☉.
    let low = sample(galaxy, 4_000_000, 30_000, uniform(0.1, 0.5), drawn);
    let multiple = share_with_at_least(&low, 2, |p| period_days(p) < 200.0);
    report.check(
        "0.1-0.5 M_sun hosts with two or more inside 200 days",
        multiple,
        (0.40, 1.0),
    );
    let giant_m = share_with(&low, |_, p| cumming_giant(p) && period_days(p) < 2_000.0);
    report.check("0.1-0.5 M_sun hosts with a giant", giant_m, (0.0, 0.05));

    // Mid-to-late M dwarfs (ruling 48 b).
    let late = sample(galaxy, 5_000_000, 30_000, uniform(0.15, 0.4), drawn);
    let hu_per_star = per_star(&late, |_, p| {
        (0.08..=6.8).contains(&p.mass().value()) && (0.5..10.0).contains(&period_days(p))
    });
    report.finding(
        "M3-M5.5 planets per star inside 10 days (Hardegree-Ullman et al. 2019)",
        hu_per_star,
        (0.70, 1.89),
        (0.30, 0.40),
    );
    let hu_multiple = share_with_at_least(&late, 2, |p| period_days(p) < 10.0);
    report.finding(
        "M3-M5.5 compact multiples (Hardegree-Ullman et al. 2019)",
        hu_multiple,
        (0.11, 0.89),
        (0.05, 0.10),
    );
}

/// P14.T10.b's metallicity statistics: small planets flat to −0.8 and thinned at −2, and the giants' slope.
fn metallicity_statistics(galaxy: &Galaxy, report: &mut Report) {
    // Metallicity: small planets flat to -0.8 and thinned at -2; giants' slope.
    let at = |first: u32, x: f64| {
        let systems = sample(galaxy, first, 20_000, uniform(0.7, 1.3), move |_, _, _| {
            Dex::new(x)
        });
        per_star(&systems, |_, p| small(p) && period_days(p) < 100.0)
    };
    let (solar, poor, halo) = (at(6_000_000, 0.0), at(6_100_000, -0.8), at(6_200_000, -2.0));
    report.finding(
        "small planets at -0.8 against solar",
        poor / solar,
        (0.8, 1.2),
        (0.41, 0.51),
    );
    report.check(
        "small planets at -2 against solar",
        halo / solar,
        (0.0, 0.25),
    );
    let bins: Vec<(f64, f64, f64)> = (0..8)
        .map(|k| {
            let centre = -0.45 + 0.1 * f64::from(k);
            let systems = sample(
                galaxy,
                7_000_000 + 100_000 * k,
                20_000,
                uniform(0.7, 1.3),
                move |_, _, lcg| Dex::new(centre - 0.05 + 0.1 * lcg.next_f64()),
            );
            let detected = share_with(&systems, |h, p| {
                period_days(p) < 4.0 * 365.25 && semi_amplitude(p, h.zone.host_mass()) > 30.0
            });
            (centre, math::log10(detected), detected * 20_000.0)
        })
        .collect();
    report.check(
        "giants' metallicity slope (Fischer and Valenti 2005)",
        weighted_slope(&bins),
        (1.7, 2.3),
    );
}

/// P14.T9.c's close-binary suppression, on the FGK sample.
fn close_binary_statistics(fgk: &[System], report: &mut Report) {
    // Close binaries against single stars of the same mass (P14.T9.c).
    let per_host = |systems: &[System], wanted: HostMultiplicity| {
        let hosts: Vec<&Host> = systems
            .iter()
            .flat_map(|s| &s.hosts)
            .filter(|h| {
                h.zone.host_multiplicity() == wanted
                    && (0.7..1.3).contains(&h.zone.host_mass().value())
                    && h.zone.component_kind().is_some()
            })
            .collect();
        let with = hosts
            .iter()
            .filter(|h| !h.placement.planets().is_empty())
            .count();
        ratio(with, hosts.len())
    };
    let suppression = per_host(fgk, HostMultiplicity::CloseBinary)
        / per_host(fgk, HostMultiplicity::SingleOrWide);
    report.finding(
        "close-binary hosts' planets against single stars' (Kraus et al. 2016)",
        suppression,
        (0.25, 0.5),
        (0.12, 0.17),
    );
}

/// Ruling 55.3's anchors, and ruling 55.1's correlation, on placed single Suns at \[Fe/H\] = 0.
fn anchor_statistics(galaxy: &Galaxy, report: &mut Report, ranks: &mut Lcg) {
    // Ruling 55.3's anchors on placed single Suns at [Fe/H] = 0.
    let sun = sample(galaxy, 8_000_000, 20_000, |_| 1.0, |_, _, _| Dex::ZERO);
    let singles: Vec<System> = sun
        .into_iter()
        .filter(|s| s.hierarchy.star_count() == 1)
        .collect();
    let placed_cumming = share_with(&singles, |_, p| {
        cumming_giant(p) && (2.0..2_000.0).contains(&period_days(p))
    });
    report.check(
        "placed Cumming giants around single Suns (ruling 55.3's fit)",
        placed_cumming,
        (0.103, 0.107),
    );
    let placed_hot = share_with(&singles, |_, p| jupiters(p) > 0.1 && period_days(p) < 10.0);
    report.check(
        "placed hot Jupiters around single Suns (ruling 55.3's fit)",
        placed_hot,
        (0.0078, 0.0086),
    );
    let class_share = |classes: &[ArchitectureClass]| {
        ratio(
            singles
                .iter()
                .filter(|s| classes.contains(&s.hosts[0].placement.class()))
                .count(),
            singles.len(),
        )
    };
    let compact = class_share(&[
        ArchitectureClass::CompactMulti,
        ArchitectureClass::CompactWithColdGiant,
    ]);
    let cold = class_share(&[ArchitectureClass::CompactWithColdGiant]) / compact;
    report.check(
        "placed cold giants in compact systems around single Suns (Zhu and Wu 2018)",
        cold,
        (0.24, 0.40),
    );
    let sun_pairs = chain_pairs(&singles, ranks);
    report.check(
        "adjacent log radii's correlation around single Suns (ruling 55.1)",
        pearson(&sun_pairs.radii),
        (0.60, 0.70),
    );
    report.note(format!("single Suns: compact systems {compact:.4}"));
}

/// P14.T10.b: the statistics of architecture, each against its source's window (see the
/// [module documentation](self) for the hosts and the mass proxy).
///
/// - Small planets, 1–4 R⊕ under 100 days, per FGK star in 0.5–1.2: Fressin et al. (2013, ApJ
///   766, 81, Table 3) 0.557 ± 0.034 inside 85 days (about 0.59 inside 100), Petigura et al.
///   (2018, AJ 155, 89, eq. 17 and Table 6) 0.64.
/// - Per M dwarf, 1–4 R⊕ under 200 days, in 1.8–3.2: Dressing and Charbonneau (2015, ApJ 807,
///   45, abstract) 2.5 ± 0.2, hosts of 2,661–3,999 K and median 0.47 R☉ (§2), taken here as
///   primaries of 0.35–0.60 M☉.
/// - Hot Jupiters, above 0.1 Jupiter masses inside 10 days, around 0.4–1.2% of FGK stars: Howard
///   et al. (2012, ApJS 201, 15, Table 4) 0.4 ± 0.1%, Wright et al. (2012, ApJ 753, 160, abstract)
///   1.2 ± 0.38%.
/// - Giants of 0.3–10 Jupiter masses inside 2,000 days around 7–14% of FGK stars: Cumming et al.
///   (2008, PASP 120, 531, abstract) 10.5%, and 8.5 ± 1.3% from their completeness counts
///   (Tables 1 and 2).
/// - The slope of log occurrence against \[Fe/H\] for giants of Fischer and Valenti's (2005, ApJ
///   622, 1102, §3) window, K > 30 m s⁻¹ inside 4 years around FGK stars, over −0.5 to +0.3:
///   2.0 ± 0.3, their P = 0.03 × 10^(2.0 \[Fe/H\]) with the width of Johnson et al.'s (2010, PASP
///   122, 905, §6.1) β = 1.7 ± 0.3 on Fischer and Valenti's stars.
/// - Among hosts of 0.1–0.5 M☉, at least 40% with two or more planets inside 200 days (Ballard
///   and Johnson 2016, ApJ 816, 66, §3.3: 45 (+12 −23)% of M dwarfs host a coplanar multiple of
///   about five planets), and under 5% with a giant of 0.3–10 Jupiter masses inside 2,000 days
///   (Cumming et al. 2008, §3.4: 1.0%, under 5.4% at 2σ; Johnson et al. 2010: 3.3% inside 2.5 au).
/// - Small planets per star at \[Fe/H\] = −0.8 within 20% of solar, and under a quarter of solar at
///   −2 (the brainstorm, after Buchhave et al. 2012, Nature 486, 375, and Petigura et al. 2018).
///
/// And the checks rulings 48, 52 and 55 name, with P14.T9.c's that waited for this placer:
///
/// - (48 b) mid-to-late M dwarfs, primaries of 0.15–0.40 M☉ (M3 V–M5.5 V; Hardegree-Ullman et al.
///   2019, AJ 158, 75, Tables 1 and 3): 1.19 (+0.70 −0.49) planets of 0.5–2.5 R⊕ per star inside
///   0.5–10 days (by the median radius here, 0.08–6.8 M⊕), and compact multiples, two or more
///   inside 10 days, around 0.44 (+0.45 −0.33) of them;
/// - (48 f) η⊕, 0.37 (+0.48 −0.21) to 0.60 (+0.90 −0.36) per star in the conservative habitable
///   zone of GK dwarfs (Bryson et al. 2021, AJ 161, 36, Table 3; [`eta_earth`]);
/// - (55.1) adjacent planets of compact chains, their log radii correlated at Weiss et al.'s
///   (2018, AJ 155, 48, §3) r = 0.65, window 0.60–0.70, and the outer the larger in 65.4 ± 0.4%
///   of pairs (§5.3);
/// - (52.5) placed spacings of small pairs against Weiss et al.'s (§5.2) 93% at 10 mutual Hill
///   radii or more, peaking near 20, measured as they measure them: each planet's mass from its
///   radius by Weiss and Marcy's (2014) relation (their eqs. 6–9; [`weiss_marcy_mass`]);
/// - (P14.T9.c) hosts in binaries inside 47 au have planets 0.25–0.5 as often as single stars of
///   the same mass (Kraus et al. 2016, AJ 152, 8: `S_bin` = 0.34 (+0.14 −0.15)).
///
/// # Findings, pinned as built
///
/// After ruling 60's calibration (P14.T7's masses and budget, P14.T8's chains and rocky groups,
/// P14.T4.b's compact exponent), η⊕ and both of Weiss et al.'s pair statistics meet their sources.
/// Each of these still misses its source by more than any change inside the sources reaches, and
/// is reported with its dial:
///
/// - M dwarfs have 1.23 small planets inside 200 days, against Dressing and Charbonneau's 2.5
///   (1.15 before ruling 68.2 raised the chains' median to Wu's 7.7 M⊕ × M★, 1.28 before ruling
///   73's budget and truncated masses). Ruling 66's floor of 1 M⊕ × M★ lets their chains' planets
///   under 1 M⊕, where the fixed 1 M⊕ floor had counted them and gave 1.90. By radius the model
///   has half of Dressing and Charbonneau's 1–4 R⊕ planets at every period from 10 to 200 days
///   (their Table 5) and a third inside 10 days, and more 0.5–1 R⊕ planets than they find: 72% of
///   these M dwarfs host a chain, 40% of those the dynamically hot one or two, where Hsu et al.
///   (2020) find the planets consistent with every M dwarf hosting a system and Ballard and
///   Johnson (2016) about five planets inside 200 days in the multiple systems (ruling 73.1);
///
/// - mid-to-late M dwarfs have 0.35 planets per star and 9% compact multiples inside 10 days,
///   against Hardegree-Ullman et al.'s 1.19 and 0.44; the early M dwarfs have 0.28 against Dressing
///   and Charbonneau's (2015, Table 5) 0.63 for the same radii and periods. The chains' first
///   period follows Mulders et al.'s (2018) law, measured about Kepler's FGK hosts, at every host
///   mass (Mulders, Pascucci and Apai 2015, ApJ 798, 112, find the break at one period), so no M
///   dwarf's chain starts closer in than a Sun's;
/// - small planets at \[Fe/H\] = −0.8 are 0.44 of solar (0.46 before ruling 68.2, 0.42 before
///   ruling 73): the compact classes' share there is about 0.72 of solar, as
///   `CompactWithColdGiant`'s weight falls with its giants (the fraction of stars with Kepler-like
///   planets rises by 1.4 between −0.2 and +0.2 in Zhu 2019, which the table follows), metal-poor
///   discs hold fewer planets under the solid budget, and rocky planets, whose masses follow their
///   discs, fall under 1 M⊕;
/// - close-binary hosts have planets 0.14 as often as single stars: `CLOSE_BINARY_SUPPRESSION`'s
///   0.34 compounds with their truncated discs, whose budgets now build no chain at all where they
///   cannot build its first planet (ruling 66), under Kraus et al.'s 1σ (0.19–0.48);
/// - small pairs are at 10 mutual Hill radii or more in 99.4% of pairs measured as Weiss et al.
///   measure them, from masses their mass–radius relation gives each radius, and in all of them by
///   their own masses, the floor of design note 7 by construction, against Weiss et al.'s 93%; the
///   median is 17.1 against a peak near 20 (ruling 52.5).
#[test]
#[ignore = "slow: places planets about a quarter of a million sampled hosts"]
fn the_statistics_of_architecture_meet_their_surveys() {
    let galaxy = galaxy(SEED);
    let mut report = Report::default();
    let mut ranks = Lcg::new(0x00ad_1ace);
    let fgk = fgk_statistics(&galaxy, &mut report, &mut ranks);
    m_dwarf_statistics(&galaxy, &mut report, &mut ranks);
    metallicity_statistics(&galaxy, &mut report);
    close_binary_statistics(&fgk, &mut report);
    anchor_statistics(&galaxy, &mut report, &mut ranks);
    report.finish();
}

/// The zero-age host of `mass` and `fe_h`.
fn zams_host(mass: f64, fe_h: f64) -> DiscHost {
    let mass = SolarMasses::new(mass);
    let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let coeffs = ZCoeffs::new(composition.z_fit());
    DiscHost::new(
        mass,
        composition.fe_h(),
        zams::luminosity(mass, &coeffs),
        zams::radius(mass, &coeffs),
    )
    .unwrap()
}

/// Golden values of P14.T8's placers: every class about a Sun-like host and an M dwarf, in a
/// median disc and a drawn one, every planet's index, mass and orbit by bits.
#[test]
fn placed_planets_are_pinned() {
    let mut out = GoldenWriter::new();
    out.header(GENERATOR_VERSION.get());
    let system_id = |i: u32| {
        hyperion_sim::id::SystemId::from_parts(
            hyperion_sim::id::Layer::A,
            hyperion_sim::coords::GenCell::new(hyperion_sim::coords::CellSize::Ly8, [5, -3, 2])
                .unwrap(),
            i,
        )
        .unwrap()
    };
    for (mass, fe_h) in [(1.0, 0.0), (0.4, 0.2)] {
        let zams = zams_host(mass, fe_h);
        for (d, draws) in [
            DiscDraws::MEDIAN,
            DiscDraws::for_host(SEED, system_id(9), 0),
        ]
        .into_iter()
        .enumerate()
        {
            let disc = disc::derive(&zams, Megayears::new(3.0), &draws, Truncation::NONE);
            for (c, class) in ArchitectureClass::ALL.into_iter().enumerate() {
                let id = system_id(u32::try_from(c).unwrap());
                let host = PlacementHost::new(0, zams, HostPlane::Isotropic);
                let placed = place(SEED, id, &host, Truncation::NONE, &disc, class, 1);
                out.line(&format!(
                    "host {mass} M_sun [Fe/H] {fe_h} disc {d} {} -> {} next {}",
                    class.name(),
                    placed.class().name(),
                    placed.next_slot()
                ));
                for p in placed.planets() {
                    let orbit = p.orbit();
                    let label = format!("planet {:04x}", p.index().get());
                    out.f64(&format!("{label} mass"), p.mass().value());
                    out.f64(&format!("{label} a"), orbit.semi_major_axis().value());
                    out.f64(&format!("{label} e"), orbit.eccentricity().value());
                    out.f64(&format!("{label} i"), orbit.inclination().value());
                    out.f64(&format!("{label} node"), orbit.ascending_node().value());
                    out.f64(
                        &format!("{label} periapsis"),
                        orbit.argument_of_periapsis().value(),
                    );
                    out.f64(
                        &format!("{label} m0"),
                        orbit.mean_anomaly_at_epoch().value(),
                    );
                    out.f64(&format!("{label} formed"), p.formation_distance().value());
                    out.line(&format!(
                        "{label} role {:?} hot {} beyond {} resonance {:?} rescaled {}",
                        p.role(),
                        p.hot(),
                        p.formed_beyond_snow_line(),
                        p.resonance().map(|r| r.commensurability()),
                        p.rescaled()
                    ));
                }
            }
        }
    }
    golden!("planetary/classes", out.as_str());
}
