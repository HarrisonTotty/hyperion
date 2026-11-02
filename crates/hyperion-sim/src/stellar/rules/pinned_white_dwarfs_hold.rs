//! The tests of P11.T4's pinned white-dwarf fix, [`PINNED_WHITE_DWARFS_VERSION`]: a pinned
//! primary whose pin is pending is never a white dwarf, but held at its core's last living state
//! until its pin collapses it, only under the rules that turn it on, read by the strip
//! (`Engine::stripped_member_at`) and a merger's product that no star can hold
//! (`Engine::held_product`) through the engine's input's rules.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Three systems of layer E's sample in P11.T10's slow tests (research-leans-26, 2026-11-01),
//!   under every 27 hold and every 27 hold but this one: a primary of 8.05 M☉ that a common
//!   envelope and case-BB transfer make a 1.320 M☉ oxygen–neon white dwarf at 41.58 Myr, pinned
//!   at 42.02 Myr; one of 8.37 M☉ that a common envelope on the thermally pulsing AGB makes a
//!   1.439 M☉ oxygen–neon white dwarf 17.6 kyr before its pin; and one of 9.05 M☉ whose helium
//!   star's common envelope ends in a merger whose product no star can hold, a 1.490 M☉
//!   carbon–oxygen white dwarf 16 kyr before its pin. Under the held arm each is a white dwarf
//!   at its pin and has no collapse there; under the arm each is held at its core's mass and
//!   collapses at its pin to plan 06's neutron star with plan 06's kick, and before the strip or
//!   merger each timeline is the held arm's bit for bit.
//! - Pairs of 8.0–9.6 M☉ primaries under the arm alone: a pinned primary is no white dwarf before
//!   its pin, a held core's radius is its white dwarf's, and every pair that the held arm does not
//!   make a white dwarf before its pin is the held arm's bit for bit.
//! - Slow: `binary_kick_massive_primaries_die_as_plan_06_says` (`stellar/binary/supernova/
//!   agreement.rs`) over layer E's 10⁴ systems under every 27 hold: none of 5,421 pinned primaries
//!   misses plan 06's collapse (`PIN_MISSES`), against 63 without the arm (55 white dwarfs at their
//!   pin with no collapse, 8 collapsing later; 59 systems, the sample drawing some cells twice).
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's fast tests and the engine's slow invariants (2026-11-01):
//! the goldens `stellar/binary_timelines` (pairs 0252, 0438 and 0902 of the pinned thousand) and
//! `sky/census_near_sun` (layer D's and E's stars without photometry, 916 and 571, fall to 914 and
//! 560: held cores in place of white dwarfs), and this module's tests of the version's arm. Nothing
//! moves at 24–26. Under every 27 hold two slow pins are re-pinned with this hold:
//! `engine_channels_hold`'s chi-squares, and `heavy_donor_hold::census_of_the_heavy_donors`'s
//! accreting white dwarfs, 4,116 to 4,106, as ten pinned primaries a pair made white dwarfs
//! collapse to neutron stars at their pins instead.

use std::sync::OnceLock;

use hyperion_testkit::lcg::Lcg;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::galaxy::rules::GalaxyRules;
use crate::id::SystemId;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::{
    BinaryInput, BinaryState, BinaryTimeline, Component, SegmentKind, SupernovaRecord, evolve,
};
use crate::stellar::multiplicity::{StarIndex, stripped_mark_min_mass};
use crate::stellar::remnant::white_dwarf::WhiteDwarfCore;
use crate::stellar::remnant::{RemnantKind, RemnantRecipe};
use crate::stellar::sse::bare_core_light;
use crate::stellar::system::{StarModel, SystemStars};
use crate::units::Years;

/// The seed of the Milky Way fixture, the binaries' agreement tests' (P11.T10), whose layer E
/// holds the three systems.
const SEED: u64 = 0x0b17_0010_5eed_0001;

/// The three systems, each with its primary's mass, M☉, its white dwarf's under the held arm,
/// M☉, and that dwarf's phase (research-leans-26; as measured, 2026-11-01).
const SYSTEMS: [(u64, f64, f64, Phase); 3] = [
    // A common envelope on the early AGB at 41.51 Myr, then case-BB transfer from the helium giant
    // to 1.320 M☉, whose core reaches its mass at 41.58 Myr; pinned at 42.02 Myr.
    (
        0x81f7_b160_0000_0018,
        8.050,
        1.320,
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
    // A common envelope on the early AGB at 32.98 Myr leaves a 2.47 M☉ helium star, whose own
    // envelope at 33.017 Myr merges it with its 2.04 M☉ companion; the product, a thermally pulsing
    // AGB star of its 1.490 M☉ core, has no track; pinned at 33.033 Myr.
    (
        0x8219_b020_0000_001b,
        9.047,
        1.490,
        Phase::CarbonOxygenWhiteDwarf,
    ),
];

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn held_rules() -> StellarRules {
    StellarRules {
        pinned_white_dwarfs: PinnedWhiteDwarfRules::Frozen,
        ..version()
    }
}

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

/// The Milky Way fixture under every 27 hold but this one, built under them, as the agreement
/// tests build theirs.
fn born() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        let rules = GalaxyRules::every_27_hold();
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules {
                stellar: StellarRules {
                    pinned_white_dwarfs: PinnedWhiteDwarfRules::Born,
                    ..rules.stellar
                },
                ..rules
            },
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

/// The version's rules keep the held arm, every version below 27 does and 27 holds a pinned
/// primary until its pin. The tripwire: the bump to 27 deletes this hold.
#[test]
fn the_pinned_white_dwarfs_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < PINNED_WHITE_DWARFS_VERSION,
        "the bump to 27 deletes P11.T4's pinned white-dwarf hold: `PINNED_WHITE_DWARFS_VERSION`, \
         `PinnedWhiteDwarfRules` with its `Born` arm and `holds`, \
         `StellarRules::pinned_white_dwarfs`, its readers' matches in `stripped_member_at` and \
         `Engine::held_product`, this module and its line in `stellar/rules.rs`"
    );
    assert_eq!(version().pinned_white_dwarfs, PinnedWhiteDwarfRules::Born);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).pinned_white_dwarfs;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), PinnedWhiteDwarfRules::Born, "version {v}");
    }
    assert_eq!(at(27), PinnedWhiteDwarfRules::Frozen);
    assert_eq!(
        StellarRules::every_26_hold_with_its_retargets().pinned_white_dwarfs,
        PinnedWhiteDwarfRules::Born
    );
    assert_eq!(
        StellarRules::every_27_hold().pinned_white_dwarfs,
        PinnedWhiteDwarfRules::Frozen
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar.pinned_white_dwarfs,
        PinnedWhiteDwarfRules::Frozen
    );
    assert!(!PinnedWhiteDwarfRules::Born.holds());
    assert!(PinnedWhiteDwarfRules::Frozen.holds());
}

/// Each of the three systems: under every 27 hold but this one its primary is a white dwarf just
/// before its pin and has no collapse there; under every 27 hold it is held there at that dwarf's
/// mass, living, and collapses at its pin to plan 06's remnant, a neutron star, with plan 06's
/// kick; and before its white dwarf is born the two timelines are the same.
#[test]
fn each_system_s_primary_collapses_at_its_pin_under_every_27_hold() {
    for (raw, initial, dwarf, phase) in SYSTEMS {
        let (stars, held) = system(born(), raw);
        let model = stars.primary();
        assert!(
            (model.initial_mass().value() - initial).abs() < 1e-3,
            "{raw:#x}: {:?}",
            model.initial_mass()
        );
        let death = model.death().expect("a massive star dies");
        assert!(death.kind().is_sudden(), "{raw:#x}: {death:?}");
        let pin = death.age().value();
        let shown = held.state_at(before(pin)).stars()[0];
        assert_eq!(shown.phase(), phase, "{raw:#x}: {shown:?}");
        assert!(
            (shown.mass().value() - dwarf).abs() < 1e-3,
            "{raw:#x}: {shown:?}"
        );
        assert!(
            primary_record(&held).is_none_or(|r| r.age().value() > pin * (1.0 + 1e-9)),
            "{raw:#x}: {:?}",
            held.supernovae()
        );

        // The white dwarf's birth under the held arm: the first segment that starts with one.
        let born_at = held
            .segments()
            .iter()
            .map(crate::stellar::binary::Segment::start)
            .find(|&t| white_dwarf(held.state_at(t).stars()[0].phase()))
            .expect("a white dwarf before the pin");
        let born_mass = held.state_at(born_at).stars()[0].mass();

        let (stars, arm) = system(galaxy(), raw);
        assert_eq!(stars.primary().death(), Some(death), "{raw:#x}");
        // Held at the white dwarf's mass at its birth, which the held arm's dwarf then grows from
        // by its companion's wind.
        for k in 1..=8_u32 {
            let t = born_at.value() + (pin - born_at.value()) * f64::from(k) / 8.0;
            let living = arm.state_at(before(t)).stars()[0];
            assert!(living.phase().is_living(), "{raw:#x} at {t} yr: {living:?}");
            assert_eq!(living.mass(), born_mass, "{raw:#x} at {t} yr");
        }
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
        for k in 0..=16_u32 {
            let t = Years::new(born_at.value() * f64::from(k) / 17.0);
            assert_eq!(arm.state_at(t), held.state_at(t), "{raw:#x} at {t:?}");
        }
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

/// What the engine made of a pair: each segment's kind, its span and the pair's state at its
/// start, with the supernovae.
type Made = (
    Vec<(SegmentKind, Years, Years, BinaryState)>,
    Vec<SupernovaRecord>,
);

/// What the engine made of the pair of `timeline`, which the timeline's rules, part of its value
/// (and of its tracks'), do not compare.
fn made(timeline: &BinaryTimeline) -> Made {
    (
        timeline
            .segments()
            .iter()
            .map(|s| (s.kind(), s.start(), s.end(), timeline.state_at(s.start())))
            .collect(),
        timeline.supernovae().to_vec(),
    )
}

/// The pairs of [`massive_pairs`] that move under the arm, as measured (2026-11-01).
const MOVED: u32 = 9;

/// Pairs of 8.0–9.6 M☉ primaries ([`massive_pairs`]) under the arm alone: no pinned primary is a
/// white dwarf at any segment's start before its pin or just before it, and a pair whose primary
/// the held arm does not make a white dwarf before its pin is the held arm's bit for bit. The
/// [`MOVED`] pairs that move are held at their core, living, at no more than
/// `sse::bare_core_light`'s radius, and collapse at their pin with plan 06's remnant and kick.
#[test]
fn only_a_pinned_primary_made_a_white_dwarf_moves() {
    let until = Years::new(6.0e7);
    let mut moved = 0_u32;
    for (i, input) in massive_pairs(240).iter().enumerate() {
        let model = StarModel::new(
            input.masses()[0],
            *input.composition(),
            input.draws()[0].clone(),
            input.age_at_epoch(),
        )
        .expect("a star");
        let held = evolve(input, until);
        let arm = evolve(&input.clone().with_rules(held_rules()), until);
        let pin = model
            .death()
            .filter(|d| {
                d.kind().is_sudden()
                    && model.initial_mass() >= stripped_mark_min_mass(model.composition())
            })
            .map(|d| d.age().value());
        let dwarf_before = |timeline: &BinaryTimeline, pin: f64| {
            timeline
                .segments()
                .iter()
                .map(crate::stellar::binary::Segment::start)
                .chain([before(pin)])
                .filter(|t| t.value() < pin)
                .any(|t| white_dwarf(timeline.state_at(t).stars()[0].phase()))
        };
        let Some(pin) = pin else {
            assert_eq!(made(&arm), made(&held), "pair {i}");
            continue;
        };
        assert!(
            !dwarf_before(&arm, pin),
            "pair {i}: a white dwarf before its pin at {pin} yr"
        );
        if !dwarf_before(&held, pin) {
            assert_eq!(made(&arm), made(&held), "pair {i}");
            continue;
        }
        moved += 1;
        // Held at its core, a star of no more than its bare core's radius, and collapsed from its
        // mass (`held_to`).
        let core = arm.state_at(before(pin)).stars()[0];
        let (_, bare) = bare_core_light(
            RemnantRecipe::default(),
            input.composition().z_fit(),
            WhiteDwarfCore::CarbonOxygen,
            core.mass(),
        );
        assert!(
            core.phase().is_living() && core.radius().value() <= bare.value() * (1.0 + 1e-9),
            "pair {i}: {core:?} against its bare core's {bare:?}"
        );
        let record = primary_record(&arm).unwrap_or_else(|| panic!("pair {i}: no record"));
        assert!(
            (record.age().value() - pin).abs() <= 1e-9 * pin,
            "pair {i}: {record:?} against the pin at {pin} yr"
        );
        let remnant = model.remnant().expect("a remnant");
        assert_eq!(record.remnant().kind(), remnant.kind(), "pair {i}");
        assert_eq!(
            record.remnant().mass().value(),
            remnant.mass().value().min(core.mass().value()),
            "pair {i}"
        );
        assert_eq!(record.kick(), model.natal_kick(), "pair {i}");
    }
    assert_eq!(moved, MOVED, "of 240 pairs");
}

/// The same moved pairs under the arm twice give the same timelines, bit for bit.
#[test]
fn a_held_pair_is_the_same_twice() {
    let until = Years::new(6.0e7);
    let rules = held_rules();
    let mut moved = 0_u32;
    for (i, input) in massive_pairs(240).iter().enumerate() {
        let input = input.clone().with_rules(rules);
        let first = evolve(&input, until);
        if made(&first) == made(&evolve(&input.clone().with_rules(version()), until)) {
            continue;
        }
        moved += 1;
        assert_eq!(evolve(&input, until), first, "pair {i}");
    }
    assert_eq!(moved, MOVED, "of 240 pairs");
}
