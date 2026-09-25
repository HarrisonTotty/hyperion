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
use hyperion_sim::stellar::multiplicity::{HierarchyNode, MultiplicityContext};
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::stellar::system::draw_metallicity;
use hyperion_sim::time::CLOCK_WINDOW_H;
use hyperion_sim::units::consts::{
    EARTH_MASS_KG, GRAVITATIONAL_CONSTANT, SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_MASS_KG,
};
use hyperion_sim::units::{
    AstronomicalUnits, Days, Dex, EarthMasses, HeliumExcess, Kelvin, Megayears, Metres,
    SolarMasses, Years,
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

/// Ruling 87.1's finding: Weiss et al.'s correlation in linear radius, for all pairs and those
/// above 1 R⊕, with and without the pairs holding a planet above 16 R⊕.
fn linear_radius_notes(all: &[(f64, f64)], above: &[(f64, f64)], report: &mut Report) {
    let linear = |pairs: &[(f64, f64)]| -> Vec<(f64, f64)> {
        pairs
            .iter()
            .map(|&(a, b)| (math::exp10(a), math::exp10(b)))
            .collect()
    };
    let under_16 = |pairs: &[(f64, f64)]| -> Vec<(f64, f64)> {
        let top = math::log10(16.0);
        pairs
            .iter()
            .copied()
            .filter(|&(a, b)| a <= top && b <= top)
            .collect()
    };
    let (all_16, above_16) = (under_16(all), under_16(above));
    report.note(format!(
        "  in linear radius: all pairs {:.4}, above 1 R_earth {:.4} ({} of {} pairs)",
        pearson(&linear(all)),
        pearson(&linear(above)),
        above.len(),
        all.len()
    ));
    report.note(format!(
        "  in linear radius without planets above 16 R_earth: all pairs {:.4}, above 1 R_earth \
         {:.4} ({} and {} pairs); in log radius {:.4} and {:.4}",
        pearson(&linear(&all_16)),
        pearson(&linear(&above_16)),
        all_16.len(),
        above_16.len(),
        pearson(&all_16),
        pearson(&above_16)
    ));
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
    let above: Vec<(f64, f64)> = pairs
        .radii
        .iter()
        .copied()
        .filter(|&(a, b)| a > 0.0 && b > 0.0)
        .collect();
    report.check(
        "adjacent log radii's correlation above 1 R_earth (Weiss et al. 2018)",
        pearson(&above),
        (0.45, 0.62),
    );
    linear_radius_notes(&pairs.radii, &above, report);
    let larger = pairs
        .radii
        .iter()
        .filter(|(inner, outer)| outer > inner)
        .count();
    report.finding(
        "outer planet the larger (Weiss et al. 2018)",
        ratio(larger, pairs.radii.len()),
        (0.633, 0.675),
        (0.630, 0.645),
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

/// P14.T10.b's early M dwarfs against Dressing and Charbonneau, and ruling 87.2's single stars;
/// returns their small planets per primary inside 200 days.
fn m_dwarf_statistics(galaxy: &Galaxy, report: &mut Report, ranks: &mut Lcg) -> f64 {
    // M dwarfs of Dressing and Charbonneau's sample.
    let m = sample(galaxy, 3_000_000, 30_000, uniform(0.35, 0.6), drawn);
    let per_m = per_star(&m, |_, p| small(p) && period_days(p) < 200.0);
    report.check(
        "small planets per M dwarf inside 200 days, every primary (Dressing and Charbonneau 2015)",
        per_m,
        (1.8, 3.2),
    );
    report.note(format!(
        "  by radius at drawn ranks: {:.4}",
        small_by_radius(&m, 200.0, ranks)
    ));
    let cumulative: Vec<String> = [10.0, 50.0, 100.0, 150.0, 200.0]
        .iter()
        .map(|&days| {
            format!(
                "{:.3}",
                per_star(&m, |_, p| small(p) && period_days(p) < days)
            )
        })
        .collect();
    report.note(format!(
        "  inside 10, 50, 100, 150 and 200 days: {} (Table 5: 0.47, 1.60, 2.03, 2.36, 2.47)",
        cumulative.join(", ")
    ));
    let inner = per_star(&m, |_, p| small(p) && (0.5..10.0).contains(&period_days(p)));
    report.check(
        "small planets per M dwarf at 0.5-10 days (Dressing and Charbonneau 2015, Table 5)",
        inner,
        (0.37, 0.57),
    );

    // Ruling 87.2: single stars and binaries wider than 200 au, against 2.47 ÷ 0.68.
    let total = m.len();
    let (alone, close): (Vec<System>, Vec<System>) = m
        .into_iter()
        .partition(|s| nearest_companion_au(s).is_none_or(|a| a > 200.0));
    report.check(
        "small planets per single M dwarf or one wider than 200 au inside 200 days (ruling 87.2)",
        per_star(&alone, |_, p| small(p) && period_days(p) < 200.0),
        (2.9, 4.4),
    );
    report.note(format!(
        "  {} of {total} primaries single or wider than 200 au",
        alone.len()
    ));
    early_m_dwarf_diagnosis(&alone, report);
    let hsu: usize = alone
        .iter()
        .chain(&close)
        .map(|s| {
            primary_planets(s)
                .filter(|(_, p)| {
                    let r = radius_chen_kipping(p.mass(), rank(ranks)).value();
                    (0.5..=4.0).contains(&r) && (0.5..256.0).contains(&period_days(p))
                })
                .count()
        })
        .sum();
    report.note(format!(
        "  every primary's planets of 0.5-4 R_earth at 0.5-256 days by radius: {:.4} (Hsu et al. \
         2020: 4.2-8.4)",
        ratio(hsu, total)
    ));

    per_m
}

/// Ruling 85.3's and 87.5's late M dwarfs, P14.T10.b's hosts of 0.1–0.5 M☉, and ruling 48 (b)'s
/// mid-to-late M dwarfs.
fn late_m_dwarf_statistics(galaxy: &Galaxy, report: &mut Report) {
    // Late M dwarfs of Sabotta et al.'s (2021) CARMENES sample, under 0.34 M☉: planets of
    // 1-10 M⊕ in minimum mass, m sin i with i seen along galactic north.
    let late_rv = sample(galaxy, 4_500_000, 30_000, uniform(0.1, 0.34), drawn);
    let rv_small = |p: &PlacedPlanet| {
        let m_sin_i = p.mass().value() * math::sin(p.orbit().inclination().value()).abs();
        (1.0..=10.0).contains(&m_sin_i)
    };
    report.finding(
        "late M dwarfs' 1-10 M_earth (m sin i) planets at 1-10 days (Sabotta et al. 2021)",
        per_star(&late_rv, |_, p| {
            rv_small(p) && (1.0..10.0).contains(&period_days(p))
        }),
        (0.78, 1.41),
        (0.10, 0.15),
    );
    report.finding(
        "late M dwarfs' 1-10 M_earth (m sin i) planets at 10-100 days (Sabotta et al. 2021)",
        per_star(&late_rv, |_, p| {
            rv_small(p) && (10.0..100.0).contains(&period_days(p))
        }),
        (0.29, 0.95),
        (0.69, 0.75),
    );
    let rv_inner = per_star(&late_rv, |_, p| {
        rv_small(p) && (1.0..10.0).contains(&period_days(p))
    });
    let rv_all = per_star(&late_rv, |_, p| {
        rv_small(p) && (1.0..100.0).contains(&period_days(p))
    });
    let mut first_periods: Vec<f64> = late_rv
        .iter()
        .flat_map(|s| s.hosts.iter().filter(|h| orbits_primary(h)))
        .filter_map(|h| {
            sorted(h)
                .iter()
                .find(|p| p.role() == GroupRole::Chain)
                .map(period_days)
        })
        .collect();
    first_periods.sort_by(f64::total_cmp);
    report.note(format!(
        "  planets of any mass at 1-10 days {:.4} and 10-100 days {:.4}; of 1-10 M_earth (true) \
         at 1-10 days {:.4}; chains' median first period {:.2} days",
        per_star(&late_rv, |_, p| (1.0..10.0).contains(&period_days(p))),
        per_star(&late_rv, |_, p| (10.0..100.0).contains(&period_days(p))),
        per_star(&late_rv, |_, p| {
            (1.0..=10.0).contains(&p.mass().value()) && (1.0..10.0).contains(&period_days(p))
        }),
        first_periods[first_periods.len() / 2]
    ));
    report.note(format!(
        "  their share at 1-10 days of 1-100 days: {:.4}, against Sabotta et al.'s 0.66",
        rv_inner / rv_all
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
        (0.64, 0.70),
    );
    let hu_multiple = share_with_at_least(&late, 2, |p| period_days(p) < 10.0);
    report.finding(
        "M3-M5.5 compact multiples (Hardegree-Ullman et al. 2019)",
        hu_multiple,
        (0.11, 0.89),
        (0.17, 0.20),
    );
}

/// The semi-major axis of the primary's innermost orbit about a companion, au, or `None` for a
/// single star.
fn nearest_companion_au(system: &System) -> Option<f64> {
    let nodes = system.hierarchy.nodes();
    let mut under = vec![0_u32; nodes.len()];
    for (i, node) in nodes.iter().enumerate().rev() {
        under[i] = match *node {
            HierarchyNode::Star(star) => 1 << star.get(),
            HierarchyNode::Pair { inner, outer, .. } => {
                under[usize::from(inner.get())] | under[usize::from(outer.get())]
            }
        };
    }
    system
        .hierarchy
        .pairs()
        .filter(|(index, _)| under[usize::from(index.get())] & 1 == 1)
        .map(|(_, orbit)| AstronomicalUnits::from(orbit.semi_major_axis()).value())
        .min_by(f64::total_cmp)
}

/// What the early M dwarfs' planets inside 200 days are made of, printed for review.
fn early_m_dwarf_diagnosis(systems: &[System], report: &mut Report) {
    let inside = |p: &PlacedPlanet| small(p) && period_days(p) < 200.0;
    let by = |f: &dyn Fn(&PlacedPlanet) -> bool| per_star(systems, |_, p| inside(p) && f(p));
    let hosts = |f: &dyn Fn(&Host) -> bool| {
        ratio(
            systems
                .iter()
                .filter(|s| s.hosts.iter().any(|h| orbits_primary(h) && f(h)))
                .count(),
            systems.len(),
        )
    };
    let chain = |h: &Host, hot: bool| {
        h.placement
            .planets()
            .iter()
            .any(|p| p.role() == GroupRole::Chain && p.hot() == hot)
    };
    report.note(format!(
        "  of them: cold chain {:.4}, hot chain {:.4}, no planet inside 200 days {:.4}; small \
         planets inside 200 days from cold chains {:.4}, hot chains {:.4}, others {:.4}; every \
         planet under 1 M_earth inside 200 days {:.4}",
        hosts(&|h| chain(h, false)),
        hosts(&|h| chain(h, true)),
        1.0 - share_with(systems, |_, p| period_days(p) < 200.0),
        by(&|p| p.role() == GroupRole::Chain && !p.hot()),
        by(&|p| p.role() == GroupRole::Chain && p.hot()),
        by(&|p| p.role() != GroupRole::Chain),
        per_star(systems, |_, p| p.mass().value() < 1.0
            && period_days(p) < 200.0),
    ));
    let hot_chain = |_: &Host, p: &PlacedPlanet| p.role() == GroupRole::Chain && p.hot();
    report.note(format!(
        "  hot chain planets whose eccentricity is scaled down to the spacing floor: {:.4}",
        per_star(systems, |h, p| hot_chain(h, p) && p.rescaled()) / per_star(systems, hot_chain)
    ));
}

/// Ruling 87.3's finding: small planets per star inside 200 days about primaries of 0.65–0.75 M☉,
/// where ruling 85.4's blend reaches the late K dwarfs, lie between the early M dwarfs' and the
/// FGK stars'.
fn blend_statistics(galaxy: &Galaxy, fgk: &[System], early_m: f64, report: &mut Report) {
    let inside = |_: &Host, p: &PlacedPlanet| small(p) && period_days(p) < 200.0;
    let edge = sample(galaxy, 3_500_000, 20_000, uniform(0.65, 0.75), drawn);
    let (blend, fgk) = (per_star(&edge, inside), per_star(fgk, inside));
    report.note(format!(
        "  0.65-0.75 M_sun {blend:.4} against early M {early_m:.4} and FGK {fgk:.4}: between {}",
        (fgk.min(early_m)..=fgk.max(early_m)).contains(&blend)
    ));
    report.finding(
        "small planets per star of 0.65-0.75 M_sun inside 200 days (ruling 87.3)",
        blend,
        (fgk.min(early_m), fgk.max(early_m)),
        (1.20, 1.27),
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
///   every primary of 0.35–0.60 M☉ with its companions: their 2.47 per Kepler target already holds
///   the close binaries' suppression, and a volume-limited sample of primaries has about
///   2.8 ± 0.4 (ruling 87.2).
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
/// - (55.1, 85.1) adjacent planets of compact chains, compared like with like with Weiss et al.'s
///   (2018, AJ 155, 48, §3) pairs: all pairs' log radii correlated at r = 0.65, window 0.60–0.70
///   (two standard errors of their 504 pairs), and pairs of planets both above 1 R⊕ at r = 0.53,
///   window 0.45–0.62 (ruling 87.1: the sampling spread is about 0.04 and the method ambiguous).
///   Weiss et al. do not say whether r is of log or linear radii; Fig. 2 is on log axes, and a
///   re-run of their cuts on the CKS II radii gives 0.617 in log and 0.587 in linear, so the log's
///   is asserted and the linear's reported, with and without the pairs holding a planet above
///   16 R⊕. Their SNR swap test needs each star's photometric noise, which the placed sample does
///   not carry, so it is not applied;
/// - (87.2) small planets per early M dwarf that is single or whose nearest companion is over
///   200 au away, inside 200 days, against 3.6 (Dressing and Charbonneau's 2.47 per Kepler target
///   ÷ 0.68, the share of Moe and Kratter's (2021, MNRAS 507, 3593, §4 and Fig. 6) magnitude-limited
///   M dwarfs whose companions leave their planets), window 2.9–4.4; beyond 200 au Moe and Kratter
///   suppress no planet;
/// - (P14.T10.b, ruling 85.4) Dressing and Charbonneau's (2015, Table 5) 0.47 small planets per
///   early M dwarf at 0.5–10 days, window 0.37–0.57: their rows' errors in quadrature give about
///   +0.065 −0.05, and the window takes ±0.10, about two of them, on the mass proxy;
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
/// Each of these still misses its source by more than any change inside the sources reaches, or
/// is a finding a ruling records rather than tunes, and is reported with its dial:
///
/// - the outer planet of a pair is the larger in 0.640 of pairs, against Weiss et al.'s 65.4%
///   with the ±2.1% sampling spread of their 504 pairs (ruling 87.4; a re-run of their cuts gives
///   64.5%), at the 0.54 dex width ruling 85.1 restores; it is recorded, not tuned;
/// - (87.3) small planets per star inside 200 days about primaries of 0.65–0.75 M☉, which ruling
///   85.4's blend reaches, are 1.23, between the FGK stars' 0.81 and the early M dwarfs' 2.83;
/// - late M dwarfs under 0.34 M☉ have 0.12 planets of 1–10 M⊕ in m sin i at 1–10 days, against
///   Sabotta et al.'s (2021, A&A 653, A114, Table 4) 1.06 (+0.35 −0.28), and 0.72 at 10–100 days,
///   inside their 0.55 (+0.40 −0.26) (rulings 85.3 and 87.5): below the blend of ruling 85.4 these
///   hosts' chains start at the FGK law's first period, a median of 15 days, so 0.43 planets of
///   any mass lie at 1–10 days, and their masses, 7.7 M⊕ × M★, put most of those under 1 M⊕;
/// - mid-to-late M dwarfs have 0.67 planets per star and 18% compact multiples inside 10 days,
///   against Hardegree-Ullman et al.'s 1.19 and 0.44 (the multiples inside their interval), their
///   hosts above 0.30 M☉ in ruling 85.4's blend;
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
    let early_m = m_dwarf_statistics(&galaxy, &mut report, &mut ranks);
    late_m_dwarf_statistics(&galaxy, &mut report);
    blend_statistics(&galaxy, &fgk, early_m, &mut report);
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
