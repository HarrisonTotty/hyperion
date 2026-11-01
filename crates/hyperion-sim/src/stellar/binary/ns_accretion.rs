//! The hold on what a neutron star keeps of a companion's stable Roche-lobe overflow (plan 11,
//! P11.T4's fix, at [`NS_ACCRETION_VERSION`]).
//!
//! As built, a neutron star fed by stable transfer keeps all of it up to its Eddington rate (BSE
//! section 2.6.6, equations 67 and 68; ruling 132.1), and collapses into a black hole once it passes
//! `MAX_NEUTRON_STAR_MASS` (`Engine::accretor_events`). A 1.35 M☉ neutron star beside a 1.5 M☉
//! main-sequence donor takes the whole 0.46 M☉ of the donor's main-sequence transfer, over 3.3 Gyr
//! at about 10⁻¹⁰ M☉ yr⁻¹, passes 2.0 M☉ while the donor crosses its Hertzsprung gap, and ends a
//! 2.6 M☉ black hole. In the slab's census (P11.T4's fix, its
//! research of 2026-11-01) most of the black holes in X-ray binaries were born neutron stars that
//! grew so, from donors of 1.2–3.8 M☉, which made black holes most of the low-mass hosts and put
//! nearly all of them under 5 M☉.
//!
//! Recycled pulsars show that neutron stars keep little of what their donors give them: a
//! millisecond pulsar needs only about 0.1 M☉ to be spun up, and the pulsars' masses are about the
//! slow pulsars' (Tauris, Langer and Kramer 2012, MNRAS 425, 1601; Antoniadis et al. 2016,
//! arXiv:1605.01665). From 27 a neutron star keeps a share [`NS_ACCRETION_EFFICIENCY`] of the
//! transfer, never more than its Eddington rate, and the rest leaves the pair. Its accretion is
//! marked as lost from the accretor, as the super-Eddington excess is; the transfer's step books it
//! so only under the heavy-donor hold, also at 27 ([`heavy_donor`](super::heavy_donor)), and with
//! the donor's specific angular momentum otherwise, as it books every loss.

use crate::GeneratorVersion;
use crate::stellar::rules::StellarRules;

/// The generator version whose stellar rules have a neutron star keep a share of a stable transfer
/// (P11.T4's fix): `Engine::accretion`'s neutron-star accretor takes [`NsAccretionRules::Efficiency`].
///
/// Below it every pair takes the held arm, [`NsAccretionRules::Eddington`], the code of version 24
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: this module with
/// [`NsAccretionRules`], the stellar rules' `ns_accretion` field, the accretion's match on it,
/// the brief's read of [`NsAccretionRules::fitted_tables_hold`], and the tripwire
/// (`the_neutron_star_accretion_is_held_only_until_the_27_bump`); the bump refits the pair-light and
/// reach tables, which the engine's runs fit.
pub(crate) const NS_ACCRETION_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// The share of a stable transfer a neutron star keeps under [`NsAccretionRules::Efficiency`],
/// below its Eddington rate: 0.1.
///
/// It is the share recycled pulsars imply (a lean of 2026-11-01). Spinning a 1.4 M☉ neutron star up
/// to 3 ms takes 0.057 M☉, `ΔM_eq` = 0.22 M☉ (M ÷ M☉)^⅓ ÷ (P ÷ ms)^(4/3) (Tauris, Langer and
/// Kramer 2012, equation 14; `recycling::SPIN_UP_MASS_AT_1_MS`), and the millisecond pulsars'
/// masses lie about 0.1–0.2 M☉ above the slow pulsars' (Antoniadis et al. 2016, arXiv:1605.01665;
/// recalled, not read at source), while a low-mass donor gives its neutron star some 0.7–1 M☉ on
/// its way to a helium white dwarf. Such efficiencies, well under one, are the usual inference
/// (Tauris, Langer and Kramer 2012; recalled).
pub(crate) const NS_ACCRETION_EFFICIENCY: f64 = 0.1;

/// What a neutron star keeps of a companion's stable Roche-lobe overflow (the stellar rules'
/// `ns_accretion` field, the hold of [`NS_ACCRETION_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum NsAccretionRules {
    /// Version 26's and before: all of it, up to the Eddington rate (BSE section 2.6.6, equations
    /// 67 and 68).
    Eddington,
    /// Version 27's: [`NS_ACCRETION_EFFICIENCY`] of it, up to the Eddington rate.
    Efficiency,
}

impl NsAccretionRules {
    /// The share of a transfer a neutron star keeps under this arm, from `eddington`, which gives
    /// for a share s of the transfer the share the Eddington limit leaves of it and whether the
    /// limit cut it; and whether what the star does not keep is marked as lost from it rather than
    /// from the donor: the held arm's where the limit cut it, the new arm's always.
    #[must_use]
    pub(crate) fn accreted(self, eddington: impl Fn(f64) -> (f64, bool)) -> (f64, bool) {
        match self {
            Self::Eddington => eddington(1.0),
            Self::Efficiency => (eddington(NS_ACCRETION_EFFICIENCY).0, true),
        }
    }

    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).ns_accretion
    }
}
