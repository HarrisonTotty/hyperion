//! The hold on the mass a dynamical main-sequence merger keeps (plan 11, P11.T4.d's fix, at
//! [`MERGER_MASS_VERSION`]).
//!
//! A fully convective main-sequence donor over its critical mass ratio transfers dynamically and
//! the pair merges (`Engine::merge_dynamically`, BSE sections 2.6.4 and 2.6.5). As built, a
//! main-sequence accretor keeps only what its thermal timescale allows of the donor,
//! √(`τ_KH` `τ_dyn`) ÷ `τ_KH,2` of it, a few per cent at most, and the rest leaves the pair, so the
//! product is the accretor's mass: a 0.454 + 0.138 M☉ pair leaves 0.1375 M☉. That is BSE's
//! dynamical branch, built for transfer that goes on; for a pair that merges, P11.T4.d's rule is
//! the collision matrix's (their table 2, section 2.7.3), whose two main-sequence stars merge with
//! their masses, as the engine's own collisions and contacts already merge them
//! (`Engine::main_sequence_merger`). From 26 the dynamical merger takes that rule: a
//! main-sequence star of M₁ + M₂ at the fractional age 0.1 Σ Mᵢ τᵢ ÷ M (their equation 80) of both
//! stars, with no mass lost.

use crate::GeneratorVersion;
use crate::stellar::rules::StellarRules;

/// The generator version whose stellar rules merge a dynamically transferring main-sequence pair
/// with both stars' masses (P11.T4.d's fix): `Engine::merge_dynamically`'s main-sequence accretor
/// takes [`MergerMassRules::Pair`].
///
/// Below it every pair takes the held arm, [`MergerMassRules::ThermalLimit`], the code of version
/// 24 verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_26_hold()`. The bump to 26 deletes this hold: this module with
/// [`MergerMassRules`], the stellar rules' `merger_mass` field, `merge_dynamically`'s match on it,
/// the brief's read of [`MergerMassRules::fitted_tables_hold`], and the tripwire
/// (`the_merger_mass_is_held_only_until_the_26_bump`); the bump refits the pair-light and reach
/// tables, which the engine's runs fit.
pub(crate) const MERGER_MASS_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// What a dynamically transferring pair's main-sequence merger keeps (the stellar rules'
/// `merger_mass` field, the hold of [`MERGER_MASS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MergerMassRules {
    /// Version 25's and before: the accretor and what its thermal timescale allows of the donor,
    /// at the accretor's own fractional age (BSE's dynamical branch, section 2.6.5).
    ThermalLimit,
    /// Version 26's: both stars' masses, at both stars' fractional age (BSE's collision of two
    /// main-sequence stars, section 2.7.3 and equation 80).
    Pair,
}

impl MergerMassRules {
    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).merger_mass
    }
}
