//! The tests of P11.T4's swollen-dwarf fix, [`SWOLLEN_DWARF_VERSION`]: from it a helium white
//! dwarf fed hydrogen swells into a giant whose core stays degenerate, only under the rules that
//! turn it on, read by `Engine::swell` through the engine's input's rules. The engine's own cases
//! are `stellar::binary::tests`' (P11.T4's swollen dwarf there).
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.

use super::*;
use crate::GENERATOR_VERSION;
use crate::stellar::binary::swollen_dwarf::{SWOLLEN_DWARF_VERSION, SwollenDwarfRules};

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules keep the held arm, every version below 29 does and 29 keeps a swollen
/// helium dwarf's core degenerate; the fitted tables hold only under the version's arm. The
/// tripwire: the bump to 29 deletes this hold.
#[test]
fn the_swollen_dwarf_is_held_only_until_the_29_bump() {
    assert!(
        GENERATOR_VERSION < SWOLLEN_DWARF_VERSION,
        "the bump to 29 deletes P11.T4's swollen-dwarf hold, not its rule: \
         `SWOLLEN_DWARF_VERSION`, `SwollenDwarfRules` with its `AtBase` arm, \
         `StellarRules::swollen_dwarf` and the swell's two reads of it, the brief's read of \
         `SwollenDwarfRules::fitted_tables_hold`, this module and its line in \
         `stellar/rules.rs`; it keeps `Engine::degenerate_giant`, `sse::degenerate_giant_mass`, \
         `Engine::feeds_a_swollen_dwarf`, `Engine::dwarf_cores` at every swell and \
         `SwollenDeath` applied always, and refits the pair-light and reach tables"
    );
    assert_eq!(version().swollen_dwarf, SwollenDwarfRules::AtBase);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).swollen_dwarf;
    for v in [GENERATOR_VERSION.get(), 25, 26, 27] {
        assert_eq!(at(v), SwollenDwarfRules::AtBase, "version {v}");
    }
    assert_eq!(at(29), SwollenDwarfRules::Degenerate);
    assert_eq!(
        StellarRules::every_29_hold().swollen_dwarf,
        SwollenDwarfRules::Degenerate
    );
    assert_eq!(
        StellarRules::of_version(GENERATOR_VERSION).swollen_dwarf,
        SwollenDwarfRules::AtBase
    );
    assert!(version().swollen_dwarf.fitted_tables_hold());
    assert!(!SwollenDwarfRules::Degenerate.fitted_tables_hold());
}
