//! The tests of P11.T4's kept-core endings, [`CORE_ENDINGS_VERSION`]: from it a white dwarf's core
//! that a giant keeps ends as its track does but for what is left, and every oxygen–neon core
//! collapses at the capture mass under the default recipe, only under the rules that turn it on,
//! read by the binary engine through its input's rules and by the track's builder through its
//! options. The engine's own cases are `stellar::binary::tests`' (P11.T4's kept-core endings
//! there); the census's figures are `dwarf_limit_hold.rs`'s, under every 29 hold.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//!
//! The movers, found by turning the hold on at the version (and back off) over the sim's tests
//! with its `testing` feature and the fitting crate's (2026-11-04; nothing moves at 24–28, the
//! held arm being version 28's code): the golden `sky/census_near_sun`
//! (`sky::census::query::tests::the_census_near_the_sun_is_pinned`), the binary engine's pins
//! `stellar::binary::tests::{a_thousand_timelines_are_pinned,
//! the_last_capped_sample_ends_as_it_does_with_the_cap_raised,
//! the_dwarf_limit_leaves_a_pinned_primary_to_its_pin}`, the sky's phase fit
//! (`sky::phase::tests::a_handful_of_cells_are_the_fit_of_todays_tracks`), and the fitted tables
//! `kick_defaults` and `kick_rank`, whose fingerprint probe `death_age(7.7 M☉, stripped mark set)`
//! moves (`hyperion-fit`'s `the_committed_tables_are_fresh`): a companion-stripped star in the
//! wider electron-capture window, whose thermal pulses the cap now ends at the capture mass. The
//! bump re-blesses them by name and refits the two tables. The server's tests were not surveyed.

use super::*;
use crate::GENERATOR_VERSION;
use crate::stellar::Phase;
use crate::stellar::binary::core_endings::{CORE_ENDINGS_VERSION, CoreEndingRules};
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::collapse::OXYGEN_NEON_CAPTURE_MASS;
use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules keep the held arm, every version from 24 to 28 does and 29 unifies the kept
/// cores' endings; the fitted tables hold only under the version's arm. The tripwire: the bump to
/// 29 deletes this hold.
#[test]
fn the_core_endings_are_held_only_until_the_29_bump() {
    assert!(
        GENERATOR_VERSION < CORE_ENDINGS_VERSION,
        "the bump to 29 deletes P11.T4's kept-core endings' hold, not its rule: \
         `CORE_ENDINGS_VERSION`, `CoreEndingRules` with its `Held` arm, \
         `StellarRules::core_endings` and its reads (`swollen_dwarf::SwollenDeath::of`, \
         `Engine::core_limit_age`, `Builder::pulses_cap`, `helium_star_end`, the dwarfs' \
         collapse masses and `DwarfLimitRules::unplaced`'s helium giant), the brief's read of \
         `CoreEndingRules::fitted_tables_hold`, this module and its line in `stellar/rules.rs`; \
         it keeps the rules of `CoreEndingRules::collapse_mass` and \
         `CoreEndingRules::below_the_chandrasekhar_mass` applied always, re-blesses the movers \
         listed here and refits the pair-light, reach and kick tables"
    );
    assert_eq!(version().core_endings, CoreEndingRules::Held);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).core_endings;
    for v in [24, 25, 26, 27, 28] {
        assert_eq!(at(v), CoreEndingRules::Held, "version {v}");
    }
    assert_eq!(at(29), CoreEndingRules::Unified);
    assert_eq!(
        StellarRules::every_29_hold().core_endings,
        CoreEndingRules::Unified
    );
    assert_eq!(
        StellarRules::of_version(crate::GENERATOR_VERSION).core_endings,
        CoreEndingRules::Held
    );
    assert!(version().core_endings.fitted_tables_hold());
    assert!(!CoreEndingRules::Unified.fitted_tables_hold());
}

/// Under the unified arm and the default recipe an oxygen–neon dwarf collapses at the capture
/// mass, 1.37 M☉; under the held arm, under `Hurley2000`, and for a carbon–oxygen or helium dwarf,
/// at the Chandrasekhar mass.
#[test]
fn an_oxygen_neon_dwarf_collapses_at_the_capture_mass_from_29() {
    let (one, co, he) = (
        Phase::OxygenNeonWhiteDwarf,
        Phase::CarbonOxygenWhiteDwarf,
        Phase::HeliumWhiteDwarf,
    );
    let (mm20, h00) = (RemnantRecipe::MandelMuller2020, RemnantRecipe::Hurley2000);
    for (rules, phase, recipe, mass) in [
        (
            CoreEndingRules::Unified,
            one,
            mm20,
            OXYGEN_NEON_CAPTURE_MASS,
        ),
        (CoreEndingRules::Unified, one, h00, CHANDRASEKHAR_MASS),
        (CoreEndingRules::Unified, co, mm20, CHANDRASEKHAR_MASS),
        (CoreEndingRules::Unified, he, mm20, CHANDRASEKHAR_MASS),
        (CoreEndingRules::Held, one, mm20, CHANDRASEKHAR_MASS),
    ] {
        let at = rules.collapse_mass(phase, recipe);
        assert!(
            (at.value() - mass.value()).abs() < 1e-12,
            "{rules:?}, {phase:?}, {recipe:?}: {at:?}"
        );
    }
}
