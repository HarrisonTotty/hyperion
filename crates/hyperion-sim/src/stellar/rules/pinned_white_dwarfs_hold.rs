//! The tests of P11.T4's pinned white-dwarf fix, [`PINNED_WHITE_DWARFS_VERSION`]: a pinned
//! primary whose pin is pending is never a white dwarf, but held at its core's last living state
//! until its pin collapses it, only under the rules that turn it on, read by the strip
//! (`Engine::stripped_member_at`) through the engine's input's rules.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Two systems of layer E's sample in P11.T10's slow tests (research-leans-26, 2026-11-01),
//!   under every 27 hold and every 27 hold but this one: a primary of 8.05 M☉ that a common
//!   envelope and case-BB transfer make a 1.320 M☉ oxygen–neon white dwarf at 41.58 Myr, pinned
//!   at 42.02 Myr; and one of 8.37 M☉ that a common envelope on the thermally pulsing AGB makes a
//!   1.439 M☉ oxygen–neon white dwarf 17.6 kyr before its pin. Under the held arm each is a white
//!   dwarf at its pin and has no collapse there; under the arm each is held at its core's mass and
//!   collapses at its pin to plan 06's neutron star with plan 06's kick, and before the strip each
//!   timeline is the held arm's bit for bit.
//! - Slow: `binary_kick_massive_primaries_die_as_plan_06_says` (`stellar/binary/supernova/
//!   agreement.rs`) counts, over layer E's 10⁴ systems under every 27 hold, the pinned primaries
//!   that miss plan 06's collapse (`PIN_MISSES`).
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's tests, are listed in plan 11's Risks, "P11.T4's pinned
//! white-dwarf fix" (2026-11-01).

use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::galaxy::rules::GalaxyRules;
use crate::id::SystemId;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::{BinaryTimeline, Component, SupernovaRecord};
use crate::stellar::multiplicity::StarIndex;
use crate::stellar::remnant::RemnantKind;
use crate::stellar::system::SystemStars;
use crate::units::Years;

/// The seed of the Milky Way fixture, the binaries' agreement tests' (P11.T10), whose layer E
/// holds the systems.
const SEED: u64 = 0x0b17_0010_5eed_0001;

/// The systems, each with its primary's mass, M☉, its white dwarf's under the held arm,
/// M☉, and that dwarf's phase (research-leans-26; as measured, 2026-11-01).
const SYSTEMS: [(u64, f64, f64, Phase); 2] = [
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
];

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
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
    assert_eq!(at(27), PinnedWhiteDwarfRules::Held);
    assert_eq!(
        StellarRules::every_26_hold_with_its_retargets().pinned_white_dwarfs,
        PinnedWhiteDwarfRules::Born
    );
    assert_eq!(
        StellarRules::every_27_hold().pinned_white_dwarfs,
        PinnedWhiteDwarfRules::Held
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar.pinned_white_dwarfs,
        PinnedWhiteDwarfRules::Held
    );
    assert!(!PinnedWhiteDwarfRules::Born.holds());
    assert!(PinnedWhiteDwarfRules::Held.holds());
}

/// Each of the systems: under every 27 hold but this one its primary is a white dwarf just
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
