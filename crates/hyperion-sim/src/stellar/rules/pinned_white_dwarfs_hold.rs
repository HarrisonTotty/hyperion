//! The tests of P11.T4's pinned white-dwarf fix: a pinned primary whose pin is pending is never a
//! white dwarf, but held at its core's last living state until its pin collapses it, read by the
//! strip (`Engine::stripped_member_at`) and a merger's product that no star can hold
//! (`Engine::held_product`). Its hold is deleted (the 27 fold); these are its tests of the new arm.
//!
//! - Three systems of layer E's sample in P11.T10's slow tests (research-leans-26, 2026-11-01, two
//!   of them replaced at generator version 26), under every 27 hold: primaries of 8.17, 8.37 and
//!   8.85 M☉ that their binaries strip to oxygen–neon or carbon–oxygen cores of 1.333, 1.439 and
//!   1.457 M☉ before their pins. Each is held at its core's mass, living, and collapses at its pin
//!   to plan 06's neutron star with plan 06's kick (under the held arm each was a white dwarf at
//!   its pin with no collapse there).
//! - Pairs of 8.0–9.6 M☉ primaries: a pinned primary is no white dwarf before its pin, and a pair
//!   is the same twice.
//! - Slow: `binary_kick_massive_primaries_die_as_plan_06_says` (`stellar/binary/supernova/
//!   agreement.rs`) over layer E's 10⁴ systems under every 27 hold: none of 5,421 pinned primaries
//!   misses plan 06's collapse (`PIN_MISSES`), against 63 without the fix (55 white dwarfs at their
//!   pin with no collapse, 8 collapsing later; 59 systems, the sample drawing some cells twice).

use std::sync::OnceLock;

use hyperion_testkit::lcg::Lcg;

use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::galaxy::rules::GalaxyRules;
use crate::id::SystemId;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::{BinaryInput, BinaryTimeline, Component, SupernovaRecord, evolve};
use crate::stellar::multiplicity::{StarIndex, stripped_mark_min_mass};
use crate::stellar::remnant::RemnantKind;
use crate::stellar::system::{StarModel, SystemStars};
use crate::units::Years;

/// The seed of the Milky Way fixture, the binaries' agreement tests' (P11.T10), whose layer E
/// holds the three systems.
const SEED: u64 = 0x0b17_0010_5eed_0001;

/// The three systems, each with its primary's mass, M☉, the white dwarf's it made under the held
/// arm, M☉, and that dwarf's phase (research-leans-26's census of layer E's 10⁴ records, as measured,
/// 2026-11-01; at generator version 26 the grid keeps two of its three no more, and the same
/// census's first oxygen–neon and carbon–oxygen dwarfs of the kind take their places).
const SYSTEMS: [(u64, f64, f64, Phase); 3] = [
    // An oxygen–neon dwarf of 1.333 M☉ before its pin at 40.79 Myr (from 26; at 24 and 25
    // `0x81f7_b160_0000_0018`, a common envelope and case-BB transfer to 1.320 M☉).
    (
        0x820b_b2a0_0000_0016,
        8.171,
        1.333,
        Phase::OxygenNeonWhiteDwarf,
    ),
    // A common envelope on the thermally pulsing AGB at 38.556 Myr, which leaves its 1.439 M☉ core;
    // pinned at 38.573 Myr.
    (
        0x8205_b4e0_0000_0006,
        8.368,
        1.439,
        Phase::OxygenNeonWhiteDwarf,
    ),
    // A carbon–oxygen dwarf of 1.457 M☉ before its pin at 34.45 Myr (from 26; at 24 and 25
    // `0x8219_b020_0000_001b`, a merger's product of a 1.490 M☉ core).
    (
        0x81eb_afe0_0000_001a,
        8.851,
        1.457,
        Phase::CarbonOxygenWhiteDwarf,
    ),
];

/// The Milky Way fixture under every 27 hold.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds under every 27 hold")
    })
}

/// The stars of system `raw` in `galaxy`, and its primary's pair's timeline.
fn system(galaxy: &Galaxy, raw: u64) -> (SystemStars, BinaryTimeline) {
    let id = SystemId::from_raw(raw).expect("a system id");
    let record = placement::resolve(galaxy, id).expect("the system resolves");
    let stars = SystemStars::generate(galaxy, &record);
    let timeline = stars
        .pairs()
        .iter()
        .find(|p| p.stars()[0] == StarIndex::PRIMARY)
        .expect("the primary's pair is run")
        .timeline()
        .clone();
    (stars, timeline)
}

/// The primary's supernova record in `timeline`, if it has one.
fn primary_record(timeline: &BinaryTimeline) -> Option<SupernovaRecord> {
    timeline
        .supernovae()
        .iter()
        .copied()
        .find(|s| s.component() == Component::Primary)
}

/// Whether `phase` is a white dwarf's.
fn white_dwarf(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
    )
}

/// The age, years, just before `pin`, a pinned death's: the engine's last living instant.
fn before(pin: f64) -> Years {
    Years::new(pin - 1.0e-9 * pin)
}

/// Each of the three systems, under every 27 hold: its primary is no white dwarf before its pin,
/// but held, living, at its core's mass, that of the white dwarf it made under the held arm, and
/// collapses at its pin to plan 06's remnant, a neutron star, with plan 06's kick.
#[test]
fn each_system_s_primary_collapses_at_its_pin_under_every_27_hold() {
    for (raw, initial, dwarf, _) in SYSTEMS {
        let (stars, arm) = system(galaxy(), raw);
        let model = stars.primary();
        assert!(
            (model.initial_mass().value() - initial).abs() < 1e-3,
            "{raw:#x}: {:?}",
            model.initial_mass()
        );
        let death = model.death().expect("a massive star dies");
        assert!(death.kind().is_sudden(), "{raw:#x}: {death:?}");
        let pin = death.age().value();
        for t in arm
            .segments()
            .iter()
            .map(crate::stellar::binary::Segment::start)
            .chain([before(pin)])
            .filter(|t| t.value() < pin)
        {
            let primary = arm.state_at(t).stars()[0];
            assert!(
                !white_dwarf(primary.phase()),
                "{raw:#x} at {t:?}: {primary:?}"
            );
        }
        let living = arm.state_at(before(pin)).stars()[0];
        assert!(living.phase().is_living(), "{raw:#x}: {living:?}");
        assert!(
            (living.mass().value() - dwarf).abs() < 1e-3,
            "{raw:#x}: {living:?}"
        );
        let record = primary_record(&arm).unwrap_or_else(|| panic!("{raw:#x}: no record"));
        assert!(
            (record.age().value() - pin).abs() <= 1e-9 * pin,
            "{raw:#x}: {:?} against the pin at {pin} yr",
            record.age()
        );
        let remnant = model.remnant().expect("a remnant");
        assert_eq!(remnant.kind(), RemnantKind::NeutronStar, "{raw:#x}");
        assert_eq!(record.remnant(), remnant, "{raw:#x}");
        assert_eq!(record.kick(), model.natal_kick(), "{raw:#x}");
    }
}

/// `n` pairs of 8.0–9.6 M☉ primaries, log-uniform, beside companions of 0.15–0.9 of their mass,
/// at 20–4,000 d (log-uniform, eccentric to 0.5 above 100 d) and \[Fe/H\] 0, −0.4 or −0.7 in turn,
/// with each star's own draws, run to 60 Myr, past every one's death: the range whose primaries the
/// held arm makes white dwarfs before their pin.
fn massive_pairs(n: u32) -> Vec<BinaryInput> {
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer};
    use crate::orbit::{Eccentricity, KeplerElements, Orientation};
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::units::{Dex, GravitationalParameter, HeliumExcess, Radians, Seconds, SolarMasses};

    let mut lcg = Lcg::new(0x0b1e_d0a7);
    let mut next = move || lcg.next_f64();
    let log_uniform = |lo: f64, hi: f64, u: f64| {
        crate::math::exp(crate::math::ln(lo) + u * crate::math::ln(hi / lo))
    };
    (0..n)
        .map(|i| {
            let m1 = log_uniform(8.0, 9.6, next());
            let m2 = m1 * (0.15 + 0.75 * next());
            let period = log_uniform(20.0, 4_000.0, next());
            let e = if period > 100.0 { 0.5 * next() } else { 0.0 };
            let fe_h = [0.0, -0.4, -0.7][usize::try_from(i % 3).expect("a small index")];
            let orbit = KeplerElements::from_period(
                Seconds::new(period * 86_400.0),
                GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
                Eccentricity::new(e).expect("an eccentricity in [0, 1)"),
                Orientation::new(Radians::new(0.3), Radians::new(0.1), Radians::new(0.2))
                    .expect("an orientation"),
                Radians::new(6.0 * next()),
            )
            .expect("an orbit");
            let cell = GenCell::new(
                CellSize::Ly8,
                [i32::try_from(i).expect("a small index") - 500, 11, 0],
            )
            .expect("a cell");
            let system = SystemId::from_parts(Layer::A, cell, 0).expect("a system");
            let draws =
                [0, 1].map(|k| StarDraws::for_star(Seed::new(0x0b1e_d0a7), BodyId::new(system, k)));
            BinaryInput::new(
                SolarMasses::new(m1),
                SolarMasses::new(m2),
                Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
                orbit,
                draws,
                Years::new(6.0e7),
            )
            .expect("a pair")
        })
        .collect()
}

/// Pairs of 8.0–9.6 M☉ primaries ([`massive_pairs`]), the range whose primaries the held arm made
/// white dwarfs before their pin: no pinned primary is a white dwarf at any segment's start before
/// its pin or just before it.
#[test]
fn no_pinned_primary_is_a_white_dwarf_before_its_pin() {
    let until = Years::new(6.0e7);
    let mut pinned = 0_u32;
    for (i, input) in massive_pairs(240).iter().enumerate() {
        let model = StarModel::new(
            input.masses()[0],
            *input.composition(),
            input.draws()[0].clone(),
            input.age_at_epoch(),
        )
        .expect("a star");
        let Some(pin) = model
            .death()
            .filter(|d| {
                d.kind().is_sudden()
                    && model.initial_mass() >= stripped_mark_min_mass(model.composition())
            })
            .map(|d| d.age().value())
        else {
            continue;
        };
        pinned += 1;
        let timeline = evolve(input, until);
        let dwarf_before = timeline
            .segments()
            .iter()
            .map(crate::stellar::binary::Segment::start)
            .chain([before(pin)])
            .filter(|t| t.value() < pin)
            .any(|t| white_dwarf(timeline.state_at(t).stars()[0].phase()));
        assert!(
            !dwarf_before,
            "pair {i}: a white dwarf before its pin at {pin} yr"
        );
    }
    assert!(pinned > 100, "{pinned} of 240 pairs pinned");
}

/// The pairs twice give the same timelines, bit for bit.
#[test]
fn a_held_pair_is_the_same_twice() {
    let until = Years::new(6.0e7);
    for (i, input) in massive_pairs(240).iter().enumerate() {
        assert_eq!(evolve(input, until), evolve(input, until), "pair {i}");
    }
}
