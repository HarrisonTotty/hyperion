//! The hold on the mass a dynamical main-sequence merger keeps (plan 11, P11.T4.d's fix, at
//! [`MERGER_MASS_VERSION`]).
//!
//! A low-mass (BSE's k = 0) main-sequence donor over its critical mass ratio transfers
//! dynamically and the pair merges (`Engine::merge_dynamically`, BSE section 2.6.4). As built, a
//! main-sequence or Hertzsprung-gap accretor keeps only what its thermal timescale allows of the
//! donor, √(`τ_KH` `τ_dyn`) ÷ `τ_KH,2` of it, a small fraction, and the rest leaves the pair, so
//! the product is the accretor's mass: a 0.454 + 0.138 M☉ pair leaves 0.1375 M☉.
//!
//! BSE's own code, read as COSMIC carries it (research-leans-16; v3.4.0's `evolv2.f`, the original
//! branch), restricts a main-sequence accretor so ("Restrict accretion to thermal timescale of
//! secondary", `dm2 = taum/tkh(j2)*dm1`), but gives an accretor of k 2–6, the gap among them, the
//! whole donor ("Add all the material to the giant's envelope", `dm2 = dm1`). Its collision rule,
//! `mix.f`, merges two main-sequence stars with M₁ + M₂, and COSMIC changed the dynamical branch's
//! main-sequence accretor to full accretion on 2023-07-20 ("This will lead to a merger of the two
//! MS stars"). P11.T4.d's rule as main reads it (the brief of 2026-10-31) is that collision rule.
//! From 26 the dynamical merger takes it: a main-sequence accretor and the donor make a
//! main-sequence star of M₁ + M₂ at the fractional age 0.1 Σ Mᵢ τᵢ ÷ M (their equation 80) of
//! both stars, and a gap accretor takes the whole donor, at its place in the gap on its new mass's
//! track, each with no mass lost.

use crate::GeneratorVersion;
use crate::stellar::rules::StellarRules;

/// The generator version whose stellar rules merge a dynamically transferring main-sequence pair
/// with both stars' masses (P11.T4.d's fix): `Engine::merge_dynamically`'s main-sequence and
/// Hertzsprung-gap accretors take [`MergerMassRules::Pair`].
///
/// Below it every pair takes the held arm, [`MergerMassRules::ThermalLimit`], the code of version
/// 24 verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_26_hold()`. The bump to 26 deletes this hold: this module with
/// [`MergerMassRules`], the stellar rules' `merger_mass` field, `merge_dynamically`'s match on it,
/// the brief's read of [`MergerMassRules::fitted_tables_hold`], and the tripwire
/// (`the_merger_mass_is_held_only_until_the_26_bump`); the bump refits the pair-light and reach
/// tables, which the engine's runs fit.
pub(crate) const MERGER_MASS_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// What a dynamically transferring pair's merger keeps when its accretor is on the main sequence
/// or in the Hertzsprung gap (the stellar rules' `merger_mass` field, the hold of
/// [`MERGER_MASS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MergerMassRules {
    /// Version 25's and before: the accretor and what its thermal timescale allows of the donor,
    /// ΔM, a main-sequence accretor at a tenth of its fractional age, 0.1 `M_a` `τ_a` ÷ (`M_a` +
    /// ΔM) (equation 80 with the donor's term dropped; BSE's dynamical branch, section 2.6.4,
    /// places it otherwise), a gap accretor at its own place in the gap.
    ThermalLimit,
    /// Version 26's: both stars' masses, at both stars' fractional age (BSE's collision of two
    /// main-sequence stars, `mix.f`, section 2.7.3 and equation 80, and COSMIC's dynamical branch
    /// since 2023-07-20); a gap accretor the whole donor (BSE's `evolv2.f` for k 2–6).
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
