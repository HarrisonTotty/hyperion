//! Golden values of plan 14's built pieces: the disc's draws and discs, plan 06's disc-lifetime
//! law, the closed forms of the limits and the spacing floor, and the architecture classes'
//! probabilities and draws (P14.T3, T4, T6.a, T15; P06.T15.c); and the golden systems (P14.T32),
//! whole systems of the Milky Way fixture found once by `tests/common`'s `find_system` and pinned
//! by ID, each with its `snapshot_at` at the epoch and at +H.
//!
//! They pin the arithmetic, not only the draws: a reordered sum, a changed power or a moved word
//! changes a line here, which is a generator-version change. CI checks the same file on 64-bit Arm
//! and on wasm32.

#[expect(
    dead_code,
    reason = "the golden systems use the search helper of tests/common alone"
)]
mod common;

use common::{Candidate, find_system};
use hyperion_sim::coords::{CellSize, GenCell};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{SystemRecord, resolve};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::planetary::architecture::CLOSE_BINARY_CUTOFF_AU;
use hyperion_sim::planetary::architecture::template::{
    CountLaw, EARLY_M_DWARF_FIRST_PERIOD_SCALE, EccentricityLaw, Location, MassLaw, PeriodLaw,
    TEMPLATES,
};
use hyperion_sim::planetary::architecture::template::{EARTH_MASSES_PER_JUPITER_MASS, GroupRole};
use hyperion_sim::planetary::architecture::{
    ArchitectureClass, ClassConstraints, ClassDraw, HostMultiplicity, ZoneLimit, class_weights,
    first_period_share,
};
use hyperion_sim::planetary::derive::limits::{TidalPlanet, moon_mass_limit};
use hyperion_sim::planetary::derive::{
    OrbitSense, hill_radius, maximum_surviving_moon_mass, roche_limit_fluid, roche_limit_rigid,
    satellite_stability_limit,
};
use hyperion_sim::planetary::disc::{self, Disc, DiscDraws, DiscHost, Truncation};
use hyperion_sim::planetary::fate::{BodyState, DestructionCause};
use hyperion_sim::planetary::placement::OrbitHost;
use hyperion_sim::planetary::placement::{
    Neighbour, PlacedPlanet, mutual_hill_factor, mutual_hill_radius, next_semi_major_axis,
    satisfies_floor, spacing_floor,
};
use hyperion_sim::planetary::record::{
    BodyKind, BodyRecord, Population as BodyPopulation, Section, SystemSnapshot,
};
use hyperion_sim::planetary::{self, Body, BodyIndex, PlanetarySystem, SystemContext};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::UnitUniform;
use hyperion_sim::stellar::premain::disc_lifetime;
use hyperion_sim::stellar::remnant::DeathKind;
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::stellar::state::Phase;
use hyperion_sim::stellar::state::StarState;
use hyperion_sim::time::ClockWindow;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Days;
use hyperion_sim::units::consts::{
    EARTH_MASS_KG, JUPITER_MASS_KG, METRES_PER_AU, SECONDS_PER_GIGAYEAR, SOLAR_MASS_KG,
};
use hyperion_sim::units::{
    AstronomicalUnits, Dex, EarthMasses, HeliumExcess, Kilograms, KilogramsPerCubicMetre,
    Megayears, Metres, Seconds, SolarMasses,
};
use hyperion_sim::{GENERATOR_VERSION, Seed, math};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

fn au(x: f64) -> Metres {
    Metres::from(AstronomicalUnits::new(x))
}

fn system(cell: [i32; 3], index: u32) -> SystemId {
    SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, cell).unwrap(), index).unwrap()
}

/// A host at the zero-age main sequence of plan 06's fits.
fn zams_host(mass: f64, fe_h: f64) -> DiscHost {
    let m = SolarMasses::new(mass);
    let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let coeffs = ZCoeffs::new(composition.z_fit());
    DiscHost::new(
        m,
        composition.fe_h(),
        zams::luminosity(m, &coeffs),
        zams::radius(m, &coeffs),
    )
    .unwrap()
}

fn write_draws(w: &mut GoldenWriter, d: &DiscDraws) {
    w.f64("gas_fraction", d.gas_fraction.value());
    w.f64("corotation_period", d.corotation_period.value());
    w.f64("characteristic_radius", d.characteristic_radius.value());
    w.f64(
        "circumbinary_lifetime_rank",
        d.circumbinary_lifetime_rank.value(),
    );
}

fn write_disc(w: &mut GoldenWriter, disc: &Disc) {
    let Some(p) = disc.profile() else {
        w.line("none");
        return;
    };
    w.f64("gas_mass_msun", p.gas_mass().value());
    w.f64("solid_mass_mearth", p.solid_mass().value());
    w.f64("lifetime_myr", p.lifetime().value());
    w.f64("snow_line_m", p.snow_line().value());
    w.f64("inner_edge_m", p.inner_edge().value());
    w.f64("outer_edge_m", p.outer_edge().value());
    w.f64("characteristic_radius_m", p.characteristic_radius().value());
    w.f64("corotation_period_s", p.corotation_period().value());
    w.f64(
        "solid_mass_0.5_to_5_au",
        p.solid_mass_between(au(0.5), au(5.0)).value(),
    );
    for a in [0.3, 1.0, 5.0] {
        w.f64(
            &format!("sigma_solids_{a}_au"),
            p.surface_density(au(a)).value(),
        );
        w.f64(
            &format!("sigma_gas_{a}_au"),
            p.gas_surface_density(au(a)).value(),
        );
        w.f64(
            &format!("isolation_mass_{a}_au"),
            p.isolation_mass(au(a)).value(),
        );
    }
}

#[test]
fn discs_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let a = system([652, -4_584, 2_047], 7);
    let b = system([-3, 10, 0], 0);
    let cases = [
        (Seed::new(0x0123_4567_89ab_cdef), a, 0),
        (Seed::new(0x0123_4567_89ab_cdef), a, 1),
        (Seed::new(42), b, 0),
        (Seed::new(42), b, 255),
    ];
    for (seed, id, host) in cases {
        w.line("");
        w.line(&format!("draws seed {seed} system {id} host {host}"));
        write_draws(&mut w, &DiscDraws::for_host(seed, id, host));
    }
    let seed = Seed::new(42);
    let hosts = [
        ("sun", zams_host(1.0, 0.0), Truncation::NONE),
        ("m dwarf", zams_host(0.3, -0.4), Truncation::NONE),
        ("a star", zams_host(2.2, 0.2), Truncation::NONE),
        (
            "truncated sun",
            zams_host(1.0, 0.0),
            Truncation::NONE.with_inner(au(0.2)).with_outer(au(4.0)),
        ),
        (
            "inverted",
            zams_host(1.0, 0.0),
            Truncation::NONE.with_inner(au(3.0)).with_outer(au(2.0)),
        ),
    ];
    for (i, (name, host, truncation)) in (0_u8..).zip(hosts) {
        let draws = DiscDraws::for_host(seed, b, i);
        let lifetime = disc_lifetime(host.mass(), draws.circumbinary_lifetime_rank);
        w.line("");
        w.line(&format!("disc {name}, host {i} of {b}"));
        write_disc(&mut w, &disc::derive(&host, lifetime, &draws, truncation));
    }
    w.line("");
    w.line("median disc of the sun");
    write_disc(
        &mut w,
        &disc::derive(
            &zams_host(1.0, 0.0),
            Megayears::new(2.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        ),
    );
    golden!("planetary/disc", w.as_str());
}

/// P14.T4.c: every class's probability on a 6 × 6 grid of host mass and \[Fe/H\], so that any
/// change to a weight or a scaling is a visible diff; then the class draws of a few hosts, and
/// the figures of every class template (P14.T5), which nothing reads until P14.T8.
#[test]
fn class_probabilities_draws_and_templates_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for m in [0.05, 0.1, 0.3, 1.0, 1.9, 5.0] {
        for x in [-2.5, -1.5, -0.8, -0.3, 0.0, 0.4] {
            let p = class_weights(SolarMasses::new(m), Dex::new(x)).probabilities();
            w.line("");
            w.line(&format!("probabilities m {m} fe_h {x}"));
            for class in ArchitectureClass::ALL {
                w.f64(class.name(), p.get(class));
            }
        }
    }
    let sun = zams_host(1.0, 0.0);
    let disc = disc::derive(
        &sun,
        Megayears::new(2.5),
        &DiscDraws::MEDIAN,
        Truncation::NONE,
    );
    let constraints = [
        ("none", ClassConstraints::NONE),
        (
            "zone inside the snow line, close binary",
            ClassConstraints::new(
                &disc,
                ZoneLimit::Outer(au(2.0)),
                HostMultiplicity::CloseBinary,
            ),
        ),
    ];
    for (name, c) in constraints {
        let p = class_weights(SolarMasses::new(1.0), Dex::new(0.0))
            .constrained(&c)
            .probabilities();
        w.line("");
        w.line(&format!("probabilities m 1 fe_h 0, constrained: {name}"));
        for class in ArchitectureClass::ALL {
            w.f64(class.name(), p.get(class));
        }
    }
    w.line("");
    let weights = class_weights(SolarMasses::new(1.0), Dex::new(0.0));
    let a = system([652, -4_584, 2_047], 7);
    let b = system([-3, 10, 0], 0);
    for (seed, id, host) in [
        (Seed::new(0x0123_4567_89ab_cdef), a, 0),
        (Seed::new(0x0123_4567_89ab_cdef), a, 1),
        (Seed::new(42), b, 0),
        (Seed::new(42), b, 255),
    ] {
        let draw = ClassDraw::for_host(seed, id, host);
        w.u64_hex(
            &format!("mark seed {seed} system {id} host {host}"),
            draw.mark().get(),
        );
        w.line(&format!("class = {}", draw.class(&weights).name()));
    }
    for template in &TEMPLATES {
        for (i, group) in template.groups().iter().enumerate() {
            w.line("");
            let name = format!("{} group {i}", template.class().name());
            w.line(&format!("{name}: {:?}", group.role()));
            write_template_group(&mut w, &name, group);
        }
    }
    golden!("planetary/architecture", w.as_str());
}

fn write_template_group(
    w: &mut GoldenWriter,
    name: &str,
    group: &hyperion_sim::planetary::architecture::template::PlanetGroup,
) {
    w.f64(&format!("{name} presence"), group.presence());
    write_count(w, name, group.count());
    let masses = group.masses();
    w.f64(&format!("{name} mass min"), masses.min().value());
    w.f64(&format!("{name} mass max"), masses.max().value());
    if let MassLaw::PowerLaw { index } = masses.law() {
        w.f64(&format!("{name} mass index"), index);
    }
    match group.location() {
        Location::Period(PeriodLaw::BrokenPowerLaw {
            break_period,
            rising,
            falling,
            min,
            max,
        }) => {
            w.f64(&format!("{name} period break"), break_period.value());
            w.f64(&format!("{name} period rising"), rising);
            w.f64(&format!("{name} period falling"), falling);
            w.f64(&format!("{name} period min"), min.value());
            w.f64(&format!("{name} period max"), max.value());
        }
        Location::Period(PeriodLaw::LogNormal {
            median,
            sigma_dex,
            min,
            max,
        }) => {
            w.f64(&format!("{name} period median"), median.value());
            w.f64(&format!("{name} period sigma"), sigma_dex);
            w.f64(&format!("{name} period min"), min.value());
            w.f64(&format!("{name} period max"), max.value());
        }
        Location::Period(PeriodLaw::LogUniform { min, max }) => {
            w.f64(&format!("{name} period min"), min.value());
            w.f64(&format!("{name} period max"), max.value());
        }
        Location::ScaledAu { inner, outer } => {
            w.f64(&format!("{name} scaled au inner"), inner);
            w.f64(&format!("{name} scaled au outer"), outer);
        }
        Location::SnowLines { inner, outer } => {
            w.f64(&format!("{name} snow lines inner"), inner);
            w.f64(&format!("{name} snow lines outer"), outer);
        }
        Location::Outward => w.line(&format!("{name} location outward")),
        Location::Flanking => w.line(&format!("{name} location flanking")),
    }
    write_eccentricity(w, name, group.eccentricity());
    if let Some(hot) = group.hot_variant() {
        w.f64(&format!("{name} hot probability"), hot.probability());
        write_count(w, &format!("{name} hot"), hot.count());
        for m in [0.1, 0.32, 0.48, 0.65, 1.0] {
            let host = SolarMasses::new(m);
            w.f64(
                &format!("{name} hot probability at {m}"),
                hot.probability_about(host),
            );
            w.f64(
                &format!("{name} first period break factor at {m}"),
                math::powf(EARLY_M_DWARF_FIRST_PERIOD_SCALE, first_period_share(host)),
            );
        }
        write_eccentricity(w, &format!("{name} hot"), hot.eccentricity());
        for n in [1, 2, 3, 5, 7, 10] {
            write_eccentricity(w, &format!("{name} hot of {n}"), hot.eccentricity_for(n));
        }
    }
    w.line(&format!(
        "{name} reach {:?} spacing {:?} origin {:?}",
        group.reach(),
        group.spacing(),
        group.origin()
    ));
}

fn write_count(w: &mut GoldenWriter, name: &str, law: CountLaw) {
    let (least, most) = law.range();
    w.line(&format!("{name} count {least}-{most}"));
    if let CountLaw::ZeroTruncatedPoisson { .. } = law {
        for m in [0.1, 0.48, 1.0, 1.3] {
            let host = SolarMasses::new(m);
            let rate = law.poisson_rate(host).unwrap();
            w.f64(&format!("{name} rate at {m}"), rate);
            w.f64(&format!("{name} mean at {m}"), law.mean(host));
        }
    }
}

fn write_eccentricity(w: &mut GoldenWriter, name: &str, law: EccentricityLaw) {
    match law {
        EccentricityLaw::Rayleigh { sigma } => w.f64(&format!("{name} e rayleigh"), sigma),
        EccentricityLaw::HalfNormal { sigma } => w.f64(&format!("{name} e half-normal"), sigma),
        EccentricityLaw::Beta { a, b } => {
            w.f64(&format!("{name} e beta a"), a);
            w.f64(&format!("{name} e beta b"), b);
        }
        EccentricityLaw::BetaByPeriod {
            split,
            short_a,
            short_b,
            long_a,
            long_b,
        } => {
            w.f64(&format!("{name} e split"), split.value());
            w.f64(&format!("{name} e short beta a"), short_a);
            w.f64(&format!("{name} e short beta b"), short_b);
            w.f64(&format!("{name} e long beta a"), long_a);
            w.f64(&format!("{name} e long beta b"), long_b);
        }
    }
}

#[test]
fn disc_lifetimes_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for m in [0.08, 0.3, 1.0, 2.0, 8.0, 40.0] {
        for u in [1e-9, 0.1, 0.5, 0.9, 0.999_999] {
            let rank = UnitUniform::new(u).unwrap();
            w.f64(
                &format!("m {m} rank {u}"),
                disc_lifetime(SolarMasses::new(m), rank).value(),
            );
        }
    }
    golden!("stellar/disc_lifetime", w.as_str());
}

#[test]
fn limits_and_spacing_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let (radius, density, ice) = (
        Metres::new(58_232e3),
        KilogramsPerCubicMetre::new(687.0),
        KilogramsPerCubicMetre::new(600.0),
    );
    w.f64(
        "roche_fluid_saturn",
        roche_limit_fluid(radius, density, ice).value(),
    );
    w.f64(
        "roche_rigid_saturn",
        roche_limit_rigid(radius, density, ice).value(),
    );
    let (sun, earth) = (Kilograms::new(SOLAR_MASS_KG), Kilograms::new(EARTH_MASS_KG));
    let hill = hill_radius(Metres::new(METRES_PER_AU), 0.0167, earth, sun);
    w.f64("hill_earth_pericentre", hill.value());
    let circular = hill_radius(Metres::new(METRES_PER_AU), 0.0, earth, sun);
    for (sense, name) in [
        (OrbitSense::Prograde, "prograde"),
        (OrbitSense::Retrograde, "retrograde"),
    ] {
        for (e_p, e_s) in [(0.0, 0.0), (0.0167, 0.0549), (0.2, 0.5)] {
            w.f64(
                &format!("stability_{name}_{e_p}_{e_s}"),
                satellite_stability_limit(circular, e_p, e_s, sense).value(),
            );
        }
    }
    let planet = TidalPlanet::new(
        Kilograms::new(0.69 * JUPITER_MASS_KG),
        Metres::new(1.35 * 7.1492e7),
        0.51,
        1e5,
    )
    .unwrap();
    let star = Kilograms::new(1.1 * SOLAR_MASS_KG);
    let age = Seconds::new(5.0 * SECONDS_PER_GIGAYEAR);
    let hd_hill = hill_radius(au(0.0468), 0.0, planet.mass(), star);
    w.f64(
        "moon_limit_hd209458b_0.36",
        moon_mass_limit(hd_hill * 0.36, &planet, age).value(),
    );
    w.f64(
        "surviving_moon_hd209458b",
        maximum_surviving_moon_mass(&planet, star, au(0.0468), 0.0, age).value(),
    );
    let (jupiter, saturn) = (EarthMasses::new(317.83), EarthMasses::new(95.16));
    let one = SolarMasses::new(1.0);
    w.f64(
        "mutual_hill_jupiter_saturn",
        mutual_hill_radius(jupiter, saturn, one, au(5.2029), au(9.5367)).value(),
    );
    let chi = mutual_hill_factor(EarthMasses::new(3.0), EarthMasses::new(5.0), one);
    w.f64("hill_factor_3_5", chi.value());
    let next = next_semi_major_axis(au(0.1), 17.0, chi).unwrap();
    w.f64("next_semi_major_axis_0.1_au_17", next.value());
    for (m1, m2, e1, e2) in [
        (3.0, 5.0, 0.0, 0.0),
        (3.0, 5.0, 0.013, 0.007),
        (317.83, 95.16, 0.05, 0.05),
    ] {
        w.f64(
            &format!("floor_{m1}_{m2}_{e1}_{e2}"),
            spacing_floor(EarthMasses::new(m1), EarthMasses::new(m2), e1, e2),
        );
    }
    let inner = Neighbour::new(jupiter, au(5.2029), 0.0484);
    let outer = Neighbour::new(saturn, au(9.5367), 0.0539);
    w.line(&format!(
        "jupiter_saturn_satisfy_floor = {}",
        satisfies_floor(&inner, &outer, one)
    ));
    golden!("planetary/limits", w.as_str());
}

// --- P14.T32: the golden systems ---

/// The universe the golden systems are found and pinned in: the Milky Way fixture at this seed.
const SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

/// The Milky Way fixture the golden systems live in.
fn fixture() -> Galaxy {
    Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// One of the slice's golden systems (P14.T32): the description it stands for, the layer it is
/// searched in, the most records the search may read, its predicate and the ID found.
struct GoldenSystem {
    name: &'static str,
    layer: Layer,
    budget: u32,
    predicate: fn(&Candidate<'_>) -> bool,
    id: u64,
}

/// A planet's mass in Jupiter masses.
fn jupiters(body: &Body) -> f64 {
    body.mass().value() / EARTH_MASSES_PER_JUPITER_MASS
}

/// The phase of star `n` at the epoch, `None` before it forms.
fn phase_of(c: &Candidate<'_>, n: usize) -> Option<Phase> {
    c.context().stars()[n]
        .state_at(UniverseTime::EPOCH)
        .map(|s| s.phase())
}

/// The states of `c`'s planets at the epoch, in index order.
fn states(c: &Candidate<'_>) -> Vec<BodyState> {
    c.system()
        .snapshot_at(c.context(), UniverseTime::EPOCH)
        .bodies()
        .iter()
        .filter(|r| r.identity().kind() == BodyKind::Planet)
        .map(|r| r.identity().state())
        .collect()
}

/// The planets of `c` present at the epoch.
fn present<'c>(c: &'c Candidate<'_>) -> Vec<&'c Body> {
    let snapshot = c.system().snapshot_at(c.context(), UniverseTime::EPOCH);
    c.system()
        .planets()
        .filter(|b| {
            snapshot
                .body(b.index())
                .is_some_and(|r| r.identity().state() == BodyState::Present)
        })
        .collect()
}

/// A planet as placed.
fn placed(b: &Body) -> &PlacedPlanet {
    b.placed().expect("a planet is placed")
}

/// A single star on the main sequence at the epoch, of initial mass `lo`–`hi` M☉.
fn single_dwarf(c: &Candidate<'_>, lo: f64, hi: f64) -> bool {
    (lo..=hi).contains(&c.record().primary_initial_mass().value())
        && c.context().stars().len() == 1
        && phase_of(c, 0) == Some(Phase::MainSequence)
}

/// A G dwarf: a single main-sequence star of 0.9–1.1 M☉ of 5,300–6,000 K at the epoch.
fn g_dwarf(c: &Candidate<'_>) -> bool {
    single_dwarf(c, 0.9, 1.1)
        && c.context().stars()[0]
            .state_at(UniverseTime::EPOCH)
            .is_some_and(|s| (5_300.0..=6_000.0).contains(&s.effective_temperature().value()))
}

/// Whether every star of `c` is on the main sequence at the epoch.
fn all_dwarfs(c: &Candidate<'_>) -> bool {
    (0..c.context().stars().len()).all(|n| phase_of(c, n) == Some(Phase::MainSequence))
}

/// The pairs of `c`'s hierarchy, by semi-major axis in au.
fn pair_separations_au(c: &Candidate<'_>) -> Vec<f64> {
    c.context()
        .hierarchy()
        .pairs()
        .map(|(_, orbit)| orbit.semi_major_axis().value() / METRES_PER_AU)
        .collect()
}

/// An M dwarf with a resonant chain: a single main-sequence star of 0.08–0.5 M☉ whose chain of
/// three or more planets was marked resonant (P14.T8.a).
fn m_dwarf_with_a_resonant_chain(c: &Candidate<'_>) -> bool {
    single_dwarf(c, 0.08, 0.5) && {
        let chain: Vec<&Body> = c
            .system()
            .planets()
            .filter(|b| placed(b).role() == GroupRole::Chain && !placed(b).hot())
            .collect();
        chain.len() >= 3 && chain.iter().any(|b| placed(b).resonance().is_some())
    }
}

/// A metal-rich G dwarf with a hot Jupiter: \[Fe/H\] of +0.2 or more, and a planet of 0.3
/// Jupiter masses or more inside 10 days, present at the epoch.
fn metal_rich_g_dwarf_with_a_hot_jupiter(c: &Candidate<'_>) -> bool {
    g_dwarf(c)
        && c.context().fe_h().value() >= 0.2
        && present(c)
            .iter()
            .any(|b| jupiters(b) >= 0.3 && Days::from(placed(b).orbit().period()).value() < 10.0)
}

/// A Solar-like system: a G dwarf of the `SolarLike` class with a rocky planet and a giant
/// present at the epoch.
fn solar_like_system(c: &Candidate<'_>) -> bool {
    g_dwarf(c)
        && c.system().architecture(OrbitHost::Star(0)) == Some(ArchitectureClass::SolarLike)
        && {
            let bodies = present(c);
            bodies.iter().any(|b| placed(b).role() == GroupRole::Rocky)
                && bodies.iter().any(|b| placed(b).role() == GroupRole::Giant)
        }
}

/// An eccentric giant: a single FGK dwarf (0.7–1.3 M☉) of the `EccentricGiant` class whose giant
/// has an eccentricity of 0.3 or more.
fn eccentric_giant(c: &Candidate<'_>) -> bool {
    single_dwarf(c, 0.7, 1.3)
        && c.system().architecture(OrbitHost::Star(0)) == Some(ArchitectureClass::EccentricGiant)
        && present(c).iter().any(|b| {
            placed(b).role() == GroupRole::Giant && placed(b).orbit().eccentricity().value() >= 0.3
        })
}

/// A halo star: a single main-sequence star of the stellar halo, with a planet present at the
/// epoch.
fn halo_star(c: &Candidate<'_>) -> bool {
    c.record().population() == Population::Halo
        && c.context().stars().len() == 1
        && all_dwarfs(c)
        && !present(c).is_empty()
}

/// A close binary with a circumbinary planet: two main-sequence stars under 47 au apart (Kraus
/// et al. 2016's cut) with a planet about the pair present at the epoch.
fn close_binary_with_a_circumbinary_planet(c: &Candidate<'_>) -> bool {
    c.context().stars().len() == 2
        && all_dwarfs(c)
        && pair_separations_au(c)
            .iter()
            .all(|&a| a < CLOSE_BINARY_CUTOFF_AU)
        && present(c)
            .iter()
            .any(|b| matches!(b.host(), OrbitHost::Pair(_) | OrbitHost::Barycentre))
}

/// A wide binary with planets about both stars: two main-sequence stars 47 au or more apart,
/// each with a planet of its own present at the epoch.
fn wide_binary_with_planets_about_both(c: &Candidate<'_>) -> bool {
    c.context().stars().len() == 2
        && all_dwarfs(c)
        && pair_separations_au(c)
            .iter()
            .all(|&a| a >= CLOSE_BINARY_CUTOFF_AU)
        && {
            let bodies = present(c);
            [0, 1]
                .iter()
                .all(|&n| bodies.iter().any(|b| b.host() == OrbitHost::Star(n)))
        }
}

/// A hierarchical triple: three main-sequence stars, with a planet present at the epoch.
fn hierarchical_triple(c: &Candidate<'_>) -> bool {
    c.context().stars().len() == 3 && all_dwarfs(c) && !present(c).is_empty()
}

/// A T Tauri star with its disc: a single pre-main-sequence star of 0.1–2 M☉ younger than its
/// disc's lifetime, with bodies to come.
fn t_tauri_star(c: &Candidate<'_>) -> bool {
    (0.1..=2.0).contains(&c.record().primary_initial_mass().value())
        && c.record().age_at_epoch().value() < 5.0e7
        && c.context().stars().len() == 1
        && phase_of(c, 0) == Some(Phase::PreMainSequence)
        && c.system()
            .disc(OrbitHost::Star(0))
            .and_then(|d| d.profile())
            .is_some_and(|d| c.record().age_at_epoch().value() < d.lifetime().value() * 1e6)
        && c.system().planets().next().is_some()
}

/// A subgiant: a single star crossing the Hertzsprung gap at the epoch, with a planet present.
fn subgiant(c: &Candidate<'_>) -> bool {
    c.record().age_at_epoch().value() > 1.0e8
        && c.context().stars().len() == 1
        && phase_of(c, 0) == Some(Phase::HertzsprungGap)
        && !present(c).is_empty()
}

/// A red giant mid engulfment: a single star on the first giant branch at the epoch that has
/// engulfed a planet and still has one.
fn red_giant_mid_engulfment(c: &Candidate<'_>) -> bool {
    c.record().age_at_epoch().value() > 1.0e8
        && c.context().stars().len() == 1
        && phase_of(c, 0) == Some(Phase::FirstGiantBranch)
        && {
            let states = states(c);
            states.contains(&BodyState::Present)
                && states.iter().any(|s| {
                    matches!(
                        s,
                        BodyState::Destroyed {
                            cause: DestructionCause::Engulfed,
                            ..
                        }
                    )
                })
        }
}

/// A fallback black hole with survivors: a single star dead at the epoch by direct collapse, a
/// black hole of complete fallback with no kick (P06.T18), with a planet present.
fn fallback_black_hole_with_survivors(c: &Candidate<'_>) -> bool {
    c.context().stars().len() == 1
        && phase_of(c, 0) == Some(Phase::BlackHole)
        && c.context().stars()[0]
            .death()
            .is_some_and(|d| d.kind() == DeathKind::DirectCollapse)
        && !present(c).is_empty()
}

/// An ordinary filler: a single main-sequence star with a planet present at the epoch.
fn filler(c: &Candidate<'_>) -> bool {
    c.context().stars().len() == 1
        && phase_of(c, 0) == Some(Phase::MainSequence)
        && !present(c).is_empty()
}

/// Fourteen of the fifteen of P14.T32's twenty-four that the slice can make (its _Slice:_ line),
/// in the order the plan lists them; the fifteenth, the T Tauri star, is [`T_TAURI`]. Each was
/// checked by eye from its golden, and its comment says what it holds at the epoch.
const GOLDEN_SYSTEMS: [GoldenSystem; 14] = [
    GoldenSystem {
        // A 0.138 M☉ M dwarf of 3,020 K, [Fe/H] +0.04, `CompactMulti`: three rocky planets of
        // 1.2–2.1 M⊕ and sub-Neptunes of 3.9 and 2.7 M⊕ at 0.057–0.29 au, and an ice giant of
        // 10.4 M⊕ at 0.47 au with seven moons and two rings. An icy belt at 0.62–0.75 au and a
        // cometary halo.
        name: "m_dwarf_resonant_chain",
        layer: Layer::A,
        budget: 200_000,
        predicate: m_dwarf_with_a_resonant_chain,
        id: 0x01ff_fb2c_2000_0000,
    },
    GoldenSystem {
        // A 1.02 M☉ G dwarf of 5,730 K, [Fe/H] +0.42, 9.3 Gyr old, `HotJupiter`: 2.3 Jupiter
        // masses at 0.044 au (3.4 days), circular, with a dust ring, and a cold giant of
        // 1.3 Jupiter masses at 13.9 au (e 0.22) with eight moons and two rings. A Kuiper belt at
        // 19.6–22.1 au and a cometary halo.
        name: "hot_jupiter",
        layer: Layer::C,
        budget: 200_000,
        predicate: metal_rich_g_dwarf_with_a_hot_jupiter,
        id: 0x41fe_acda_0000_0001,
    },
    GoldenSystem {
        // A 0.975 M☉ G dwarf of 5,460 K, [Fe/H] +0.25, `SolarLike`: five rocky planets of
        // 0.74–2.0 M⊕ at 0.16–0.99 au, giants of 5.5, 0.34 and 0.80 Jupiter masses at 2.9, 10.6
        // and 21.5 au, and ice giants of 13.5 and 16.9 M⊕ at 35 and 51 au, each giant with seven
        // moons and a dust ring. A 1.3 M⊕ Kuiper belt at 67–81 au and a cometary halo.
        name: "solar_like",
        layer: Layer::C,
        budget: 200_000,
        predicate: solar_like_system,
        id: 0x4200_aca2_0000_0003,
    },
    GoldenSystem {
        // A 0.902 M☉ dwarf of 5,110 K, [Fe/H] +0.08, `EccentricGiant`: one giant of 0.58 Jupiter
        // masses at 1.17 au, e = 0.45, with three captured moons and a dust ring. A Kuiper belt at
        // 1.9–18.9 au; no halo.
        name: "eccentric_giant",
        layer: Layer::C,
        budget: 200_000,
        predicate: eccentric_giant,
        id: 0x4200_acaa_0000_000a,
    },
    GoldenSystem {
        // A 0.400 M☉ M dwarf of the halo, 3,850 K, [Fe/H] −0.77, 11.7 Gyr old, `CompactMulti`: one
        // rocky planet of 1.58 M⊕ at 0.040 au, circularised, and an icy belt at 0.07–28 au with
        // two dwarf planets; no halo. Re-pinned by ruling 73 (`calib3`): the first halo star with
        // a planet is this one, before `01fdbb3660000000`.
        name: "halo_star",
        layer: Layer::A,
        budget: 200_000,
        predicate: halo_star,
        id: 0x0202_5b2b_e000_0001,
    },
    GoldenSystem {
        // A 0.927 + 0.411 M☉ pair of main-sequence stars 1.97 au apart (e 0.28), [Fe/H] +0.44: the
        // circumbinary zone is `CompactWithColdGiant`, with giants of 5.5 and 0.62 Jupiter masses
        // at 6.5 and 30.3 au (e 0.55) about the pair; neither star has planets of its own (B's
        // `CompactMulti` places none), each has a rocky belt inside 0.45 au, and the pair a Kuiper
        // belt at 51–132 au and a cometary halo.
        name: "close_binary",
        layer: Layer::C,
        budget: 200_000,
        predicate: close_binary_with_a_circumbinary_planet,
        id: 0x41ff_ecae_0000_0001,
    },
    GoldenSystem {
        // A 0.905 + 0.687 M☉ pair of K dwarfs 86 au apart (e 0.69), [Fe/H] +0.03: A has one ice
        // giant of 11.0 M⊕ at 0.126 au (`CompactMulti`) with four moons and two rings, B nine
        // rocky planets of 0.05–0.15 M⊕ at 0.13–0.83 au and an icy one of 1.7 M⊕ at 1.16 au
        // (`TerrestrialOnly`). Each star has a belt beyond its planets; no halo.
        name: "wide_binary",
        layer: Layer::C,
        budget: 200_000,
        predicate: wide_binary_with_planets_about_both,
        id: 0x41ff_ecae_0000_0004,
    },
    GoldenSystem {
        // A 0.870 M☉ K dwarf with a 0.397 + 0.614 M☉ pair of dwarfs 57 au apart (e 0.74), 758 au
        // out, [Fe/H] +0.24: every star `CompactMulti`, with two sub-Neptunes of 4.8 and 6.6 M⊕
        // inside 0.15 au about A, seven rocky planets of 0.41–2.1 M⊕ inside 0.17 au about B, and
        // ten of 0.63–4.5 M⊕ at 0.034–0.85 au about C. A belt beyond each star's planets; no halo.
        name: "hierarchical_triple",
        layer: Layer::C,
        budget: 200_000,
        predicate: hierarchical_triple,
        id: 0x4200_2cb2_0000_0003,
    },
    GoldenSystem {
        // A 1.02 M☉ star crossing the Hertzsprung gap at 2.6 L☉ and 5,270 K, [Fe/H] −0.06,
        // 10.0 Gyr old, drawn `CompactWithColdGiant`, placed `CompactMulti` (its core does not
        // grow in time): an ice giant of 22.5 M⊕ at 0.050 au, circularised, with a dust ring, and
        // a 9.4 M⊕ icy belt at 0.14–89 au.
        name: "subgiant",
        layer: Layer::C,
        budget: 200_000,
        predicate: subgiant,
        id: 0x41ff_2cc2_0000_0007,
    },
    GoldenSystem {
        // A 1.06 M☉ star on the first giant branch at 38 L☉ and 4,510 K, [Fe/H] +0.03,
        // `CompactWithColdGiant`: its chain planets of 2.2 and 3.2 M⊕ engulfed 75 and 37 Myr
        // before the epoch, and giants of 0.81 and 0.99 Jupiter masses at 4.6 and 10.9 au (e 0.38)
        // still present, with six and five moons and a dust ring each. An asteroid belt at
        // 1.8–2.9 au, a Kuiper belt at 16.8–17.2 au and a cometary halo.
        name: "red_giant",
        layer: Layer::C,
        budget: 200_000,
        predicate: red_giant_mid_engulfment,
        id: 0x4202_6cc2_0000_0003,
    },
    GoldenSystem {
        // A 9.94 M☉ black hole of a 32.4 M☉ star that died by direct collapse, [Fe/H] −0.10: seven
        // rocky survivors of 1.3–1.9 M⊕ at 114–912 au, their orbits widened by the progenitor's
        // mass loss, three planets it unbound, and a rocky belt of 10.6 M⊕ at 1,200–1,450 au.
        // Re-pinned after plan 11's ruling 81: the first pin, 0x8200_b2e0_0000_000d, no longer
        // satisfies the predicate.
        name: "fallback_black_hole",
        layer: Layer::E,
        budget: 200_000,
        predicate: fallback_black_hole_with_survivors,
        id: 0x8201_b2e0_0000_0010,
    },
    GoldenSystem {
        // A 0.335 M☉ M dwarf of 3,600 K, [Fe/H] 0.00, `CompactMulti`: one rocky planet of 0.80 M⊕
        // at 0.042 au, circularised, with one moon, and a rocky belt at 0.058–0.070 au.
        name: "filler_a",
        layer: Layer::A,
        budget: 200_000,
        predicate: filler,
        id: 0x01ff_fb2c_6000_0000,
    },
    GoldenSystem {
        // A 0.677 M☉ K dwarf of 4,100 K, [Fe/H] +0.17, `CompactWithColdGiant`: two rocky planets
        // of 0.92 and 0.69 M⊕ at 0.033 and 0.041 au, circularised, and giants of 1.8 and
        // 5.3 Jupiter masses at 1.8 and 14.3 au, each with seven moons and a dust ring. An
        // asteroid belt at 0.73–1.16 au, a Kuiper belt at 19.7–22.7 au and a cometary halo.
        name: "filler_b",
        layer: Layer::B,
        budget: 200_000,
        predicate: filler,
        id: 0x2200_1659_8000_0000,
    },
    GoldenSystem {
        // A 1.14 M☉ F dwarf of 5,980 K, [Fe/H] +0.04, `TerrestrialOnly`: nine planets of
        // 0.15–2.1 M⊕ at 0.43–6.8 au, two of them sub-Neptunes and the outermost icy, and a Kuiper
        // belt at 8.9–10.8 au.
        name: "filler_c",
        layer: Layer::C,
        budget: 200_000,
        predicate: filler,
        id: 0x4200_2cb2_0000_0000,
    },
];

/// The T Tauri star with its disc, the fifteenth of the slice, which no search finds: plan 06's
/// `StarModel` starts every star at the zero-age main sequence, so no star of this generator
/// version is in [`Phase::PreMainSequence`] (P14.T32.a's _Slice:_ line allows a description the
/// grid cannot fill to wait).
const T_TAURI: GoldenSystem = GoldenSystem {
    name: "t_tauri",
    layer: Layer::C,
    budget: 200_000,
    predicate: t_tauri_star,
    id: 0,
};

/// The context and planets of a pinned ID.
fn pinned(galaxy: &Galaxy, id: u64) -> (SystemRecord, SystemContext, PlanetarySystem) {
    let id = SystemId::from_raw(id).expect("a pinned ID is well formed");
    let record = resolve(galaxy, id).expect("a pinned ID names a system");
    let context = SystemContext::for_system(galaxy, id).expect("a pinned ID names a system");
    let system = planetary::generate(galaxy.seed(), &context);
    (record, context, system)
}

/// P14.T32.a: every pinned ID resolves, and its system is still what its name says.
#[test]
fn pinned_ids_satisfy_their_own_predicates() {
    let galaxy = fixture();
    for golden in &GOLDEN_SYSTEMS {
        let id = SystemId::from_raw(golden.id).expect("a pinned ID is well formed");
        assert_eq!(id.layer(), Some(golden.layer), "{}", golden.name);
        let record = resolve(&galaxy, id).expect("a pinned ID names a system");
        assert!(
            (golden.predicate)(&Candidate::new(&galaxy, &record)),
            "{} ({:#018x}) no longer satisfies its predicate",
            golden.name,
            golden.id
        );
    }
}

/// P14.T32.a: the search, run again, finds the pinned IDs, and still finds no T Tauri star.
#[test]
#[ignore = "slow: searches for the golden systems"]
fn the_search_reproduces_the_pinned_ids() {
    let galaxy = fixture();
    let moved: Vec<String> = GOLDEN_SYSTEMS
        .iter()
        .filter_map(|golden| {
            let found = find_system(&galaxy, golden.layer, golden.predicate, golden.budget)
                .map(SystemId::raw);
            (found != Some(golden.id)).then(|| {
                let found = found.map_or_else(|| "none".to_owned(), |id| format!("{id:#018x}"));
                format!("{}: found {found}", golden.name)
            })
        })
        .collect();
    assert!(moved.is_empty(), "{moved:#?}");
    let t_tauri = find_system(&galaxy, T_TAURI.layer, T_TAURI.predicate, T_TAURI.budget);
    assert_eq!(
        t_tauri, None,
        "a T Tauri star is found: pin it, with its golden"
    );
}

/// A section's state, where it has no value to write.
fn write_section_state<T>(w: &mut GoldenWriter, name: &str, section: &Section<T>) {
    w.line(&format!("{name}: {:?}", section.state()));
}

/// A list of bodies, by index, or its section's state.
fn write_indices(w: &mut GoldenWriter, name: &str, section: &Section<Vec<BodyIndex>>) {
    match section {
        Section::Ok(indices) => {
            let listed: Vec<String> = indices.iter().map(|i| format!("{:04x}", i.get())).collect();
            w.line(&format!("{name}: [{}]", listed.join(", ")));
        }
        other => write_section_state(w, name, other),
    }
}

/// A population body's extent (P14.T20–T21).
fn write_population(w: &mut GoldenWriter, population: &BodyPopulation) {
    match population {
        BodyPopulation::Ring(ring) => {
            w.line(&format!("ring: {:?} of {:?}", ring.kind(), ring.material()));
            w.f64("inner_m", ring.inner_edge().value());
            w.f64("outer_m", ring.outer_edge().value());
            w.f64("optical_depth", ring.optical_depth());
            for gap in ring.gaps() {
                w.f64(
                    &format!("gap_{:?}_m", gap.resonance()),
                    gap.radius().value(),
                );
            }
        }
        BodyPopulation::Belt(belt) => {
            w.line(&format!(
                "belt: {:?} about {:?}, {:?}",
                belt.site(),
                belt.host(),
                belt.composition()
            ));
            w.f64("main_inner_m", belt.main().0.value());
            w.f64("main_outer_m", belt.main().1.value());
            if let Some((inner, outer)) = belt.scattered() {
                w.f64("scattered_inner_m", inner.value());
                w.f64("scattered_outer_m", outer.value());
            }
            w.f64("size_slope", belt.size_slope());
            w.f64("largest_diameter_m", belt.largest_diameter().value());
            w.f64("fractional_luminosity", belt.fractional_luminosity());
            write_indices(w, "members", belt.members());
        }
        BodyPopulation::CometaryHalo(halo) => {
            w.line(&format!("halo about {:?}", halo.host()));
            w.f64("inner_m", halo.inner_edge().value());
            w.f64("outer_m", halo.outer_edge().value());
            w.f64("comets", halo.comets());
            w.f64("comet_rate_per_s", halo.comet_rate().value());
        }
    }
}

fn write_record(w: &mut GoldenWriter, record: &BodyRecord) {
    let identity = record.identity();
    let label = identity
        .label()
        .ok()
        .map_or_else(|| "(no label)".to_owned(), ToString::to_string);
    w.line(&format!(
        "body {:04x} {label}: {:?} about {:?}, {:?}",
        record.index().get(),
        identity.kind(),
        identity.parent(),
        identity.state()
    ));
    match record.mass() {
        Section::Ok(mass) => w.f64("mass_mearth", mass.value()),
        other => write_section_state(w, "mass", other),
    }
    match record.orbit() {
        Section::Ok(orbit) => {
            let e = orbit.elements();
            w.f64("a_m", e.semi_major_axis().value());
            w.f64("e", e.eccentricity().value());
            w.f64("i_rad", e.inclination().value());
            w.f64("node_rad", e.ascending_node().value());
            w.f64("periapsis_rad", e.argument_of_periapsis().value());
            w.f64(
                "mean_anomaly_at_epoch_rad",
                e.mean_anomaly_at_epoch().value(),
            );
            w.f64("period_s", e.period().value());
            w.f64("mu_m3_s2", e.gravitational_parameter().value());
            w.line(&format!("valid_until: {:?}", orbit.valid_until()));
        }
        other => write_section_state(w, "orbit", other),
    }
    match record.position() {
        Some(position) => {
            for (axis, x) in ["x_m", "y_m", "z_m"].into_iter().zip(position.metres()) {
                w.f64(axis, x);
            }
        }
        None => w.line("position: none"),
    }
    match record.bulk() {
        Section::Ok(bulk) => {
            w.f64("radius_rearth", bulk.radius().value());
            w.f64("density_kg_m3", bulk.density().value());
            w.f64("gravity_m_s2", bulk.surface_gravity().value());
            w.line(&format!("class: {:?}", bulk.class()));
            let f = bulk.fractions();
            w.f64("iron", f.iron());
            w.f64("rock", f.rock());
            w.f64("water", f.water());
            w.f64("envelope", f.envelope());
            w.f64("t_eq_k", bulk.equilibrium_temperature().value());
        }
        other => write_section_state(w, "bulk", other),
    }
    write_indices(w, "moons", record.moons());
    write_indices(w, "rings", record.rings());
    match record.population() {
        Section::Ok(population) => write_population(w, population),
        other => write_section_state(w, "population", other),
    }
    write_section_state(w, "surface", record.surface());
    write_section_state(w, "hooks", record.hooks());
}

fn write_snapshot(w: &mut GoldenWriter, name: &str, snapshot: &SystemSnapshot) {
    w.line("");
    w.line(&format!(
        "{name}: {} bodies at {:?}",
        snapshot.bodies().len(),
        snapshot.time()
    ));
    write_indices(w, "belts", snapshot.belts());
    match snapshot.halo() {
        Section::Ok(halo) => w.line(&format!(
            "halo: {}",
            halo.map_or_else(|| "none".to_owned(), |index| format!("{:04x}", index.get()))
        )),
        other => write_section_state(w, "halo", other),
    }
    for record in snapshot.bodies() {
        write_record(w, record);
    }
}

/// P14.T32.b: each golden system's whole `snapshot_at` at the epoch and at +H, with the stars it
/// is about. The events of a century wait for P14.T31.
#[test]
fn golden_systems_are_pinned() {
    let galaxy = fixture();
    for golden in &GOLDEN_SYSTEMS {
        let (record, context, system) = pinned(&galaxy, golden.id);
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        w.line(&format!(
            "{} {:#018x}: {:?}, {} stars",
            golden.name,
            golden.id,
            record.population(),
            context.stars().len()
        ));
        w.f64("fe_h", context.fe_h().value());
        w.f64("age_at_epoch_yr", context.age_at_epoch().value());
        for (n, star) in context.stars().iter().enumerate() {
            let state = star.state_at(UniverseTime::EPOCH);
            w.line(&format!(
                "star {n}: {:?} at the epoch",
                state.as_ref().map(StarState::phase)
            ));
            w.f64("initial_mass_msun", star.initial_mass().value());
            if let Some(state) = state {
                w.f64("mass_msun", state.mass().value());
                w.f64("luminosity_lsun", state.luminosity().value());
                w.f64("teff_k", state.effective_temperature().value());
            }
        }
        for (node, orbit) in context.hierarchy().pairs() {
            w.line(&format!("pair {}:", node.get()));
            w.f64("a_au", orbit.semi_major_axis().value() / METRES_PER_AU);
            w.f64("e", orbit.eccentricity().value());
        }
        for host in system.hosts() {
            w.line(&format!(
                "host {:?}: drawn {:?}, placed {:?}",
                host.host(),
                host.drawn_class(),
                host.class()
            ));
        }
        for t in [UniverseTime::EPOCH, ClockWindow::END] {
            let label = if t == UniverseTime::EPOCH {
                "epoch"
            } else {
                "+H"
            };
            write_snapshot(&mut w, label, &system.snapshot_at(&context, t));
        }
        golden!(&format!("planetary/systems/{}", golden.name), w.as_str());
    }
}

/// P14.T30.c on the Solar-like golden: its ten planets are lettered `A b` to `A k` from the inside
/// out, its giants' moons take Roman numerals (`A g II`), and its one belt is `BELT 1`.
#[test]
fn the_solar_like_golden_is_labelled_by_its_layout() {
    let galaxy = fixture();
    let solar = GOLDEN_SYSTEMS
        .iter()
        .find(|golden| golden.name == "solar_like")
        .expect("the Solar-like golden is pinned");
    let (_, _, system) = pinned(&galaxy, solar.id);
    let labels = planetary::label::labels(&system);
    let text = |index: BodyIndex| {
        labels
            .iter()
            .find(|(at, _)| *at == index)
            .map(|(_, label)| label.as_str().to_owned())
            .expect("every body is labelled")
    };
    let mut planets: Vec<&Body> = system.planets().collect();
    planets.sort_by(|a, b| {
        let a = a
            .orbit()
            .expect("a planet orbits")
            .semi_major_axis()
            .value();
        a.total_cmp(
            &b.orbit()
                .expect("a planet orbits")
                .semi_major_axis()
                .value(),
        )
    });
    let letters: Vec<String> = planets.iter().map(|planet| text(planet.index())).collect();
    let expected: Vec<String> = "bcdefghijk".chars().map(|c| format!("A {c}")).collect();
    assert_eq!(letters, expected);
    let all: Vec<&str> = labels.iter().map(|(_, label)| label.as_str()).collect();
    assert!(all.contains(&"A g II"), "{all:?}");
    assert!(all.contains(&"BELT 1"), "{all:?}");
}
