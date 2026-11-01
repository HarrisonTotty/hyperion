//! The hold on stable transfer from a main-sequence donor heavier than its white dwarf, neutron
//! star or black hole, and on the transfer's angular momentum (plan 11, P11.T4's fix, at
//! [`HEAVY_DONOR_VERSION`]).
//!
//! As built, a 1.4 M☉ main-sequence donor at its lobe beside a 0.6 M☉ white dwarf transfers at
//! about 10⁻¹⁰ M☉ yr⁻¹ for 3.4 Gyr, a nuclear rate, while its orbit widens from 12 to 18 h; such a
//! donor's thermal rate, M ÷ `τ_KH`, is 4.4 × 10⁻⁷. In the slab's census (P11.T4's fix, its research
//! of 2026-11-01) 57% of the hydrogen-donor cataclysmic variables had a donor heavier than the
//! white dwarf, and 60% lay above 6 h. Two departures from BSE's book of the transfer's angular
//! momentum widen the orbit:
//!
//! - what is not accreted leaves with the donor's specific orbital angular momentum always, though
//!   the accretion marks novae and super-Eddington loss as leaving from the accretor
//!   (`Accretion::from_accretor`), as the module documentation of `rlof` says the published code
//!   has it: never read, so a nova's ejecta from a 0.6 M☉ white dwarf beside a 1.4 M☉ donor take
//!   0.18 of what they would, (0.3 ÷ 0.7)²;
//! - the donor's transferred spin is returned to the orbit (BSE's `jorb + djt`) but not taken from
//!   the donor, whose corotation is then restored with nothing drawn from the orbit, so each step
//!   adds about x `R_L`² ω of angular momentum to the pair (BSE's code keeps the pair's total
//!   through its tides; recalled).
//!
//! A donor whose overfill a transfer only deepens cannot shrink inside its lobe on its own
//! thermal timescale, and transfers on it (Schenker et al. 2002, MNRAS 337, 1105, on the
//! thermal-timescale mass transfer of cataclysmic variables with donors heavier than their white
//! dwarf; recalled, not read at source). BSE holds a giant-like donor to that rate, M ÷ `τ_KH`
//! (Hurley, Tout and Pols 2002, equations 60 and 61), and a main-sequence one only to its dynamical
//! rate; the engine's main sequence follows its track's radius at once, so it never overfills
//! its lobe by the ln(R ÷ `R_L`) of 0.42 at which BSE's equation 58 would reach a 1.4 M☉ donor's
//! thermal rate.
//!
//! From 27 ([`HeavyDonorRules::Thermal`]):
//!
//! - a main-sequence donor (BSE's type 1) heavier than its white dwarf, neutron star or black hole
//!   is held to its thermal rate, and transfers at it wherever a step at it still leaves the donor
//!   overfilling its lobe ([`HeavyDonorRules::thermal_rate`]);
//! - what is not accreted leaves with the accretor's specific orbital angular momentum where the
//!   accretion marks it so ([`HeavyDonorRules::loss_from_accretor`]);
//! - onto a white dwarf, a neutron star or a black hole, the transfer's step returns no spin to the
//!   orbit, and the donor's corotation after it draws its spin from the orbit, the pair's total
//!   kept ([`HeavyDonorRules::corotation_from_orbit`]).
//!
//! Onto any other accretor the transfer keeps the held arm's book of its spin (a lean of
//! 2026-11-01): forced on at version 24, the corotation drawn from the orbit moves the paper's
//! Algol, a W Ursae Majoris contact and the double neutron stars' channel, none of which this fix
//! is for, and the general departure is left for its own fix.

use crate::GeneratorVersion;
use crate::stellar::rules::StellarRules;

use super::star::Kind;

/// The generator version whose stellar rules transfer from a main-sequence donor heavier than its
/// compact accretor at its thermal rate and keep the transfer's angular momentum (P11.T4's fix):
/// `Engine::transfer_step`, `Engine::transfer_trial` and `Engine::transfer_phase` take
/// [`HeavyDonorRules::Thermal`].
///
/// Below it every pair takes the held arm, [`HeavyDonorRules::Nuclear`], the code of version 24
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: this module with
/// [`HeavyDonorRules`], the stellar rules' `heavy_donor` field, the transfer's three reads of it,
/// `Engine::corotate_donor` (which only the held arm calls), `Accretion::from_accretor`'s place in
/// the held arm, the brief's read of [`HeavyDonorRules::fitted_tables_hold`], and the tripwire
/// (`the_heavy_donor_rules_are_held_only_until_the_27_bump`); the bump refits the pair-light and
/// reach tables, which the engine's runs fit.
pub(crate) const HEAVY_DONOR_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// How stable transfer treats a main-sequence donor heavier than its compact accretor, and how it
/// books its angular momentum (the stellar rules' `heavy_donor` field, the hold of
/// [`HEAVY_DONOR_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum HeavyDonorRules {
    /// Version 26's and before: the overflow law's rate up to the dynamical rate for every
    /// main-sequence donor; every loss with the donor's specific angular momentum; the donor's
    /// transferred spin returned to the orbit and its corotation restored with nothing drawn.
    Nuclear,
    /// Version 27's: the thermal rate for a main-sequence donor heavier than its compact accretor
    /// that a step at it leaves overfilling; the accretor's specific angular momentum where the
    /// accretion marks the loss as its; and, onto a compact accretor, the pair's total angular
    /// momentum kept through the donor's corotation.
    Thermal,
}

impl HeavyDonorRules {
    /// Whether a donor of type `donor` and `donor_mass` M☉ beside an accretor of type `accretor`
    /// and `accretor_mass` M☉ is held to its thermal rate, and transfers at it wherever a step at it
    /// leaves the donor overfilling: under [`HeavyDonorRules::Thermal`], a main-sequence donor of
    /// BSE's type 1 heavier than a white dwarf, a neutron star or a black hole.
    #[must_use]
    pub(super) fn thermal_rate(
        self,
        donor: Kind,
        accretor: Kind,
        donor_mass: f64,
        accretor_mass: f64,
    ) -> bool {
        match self {
            Self::Nuclear => false,
            Self::Thermal => {
                donor == Kind::MainSequence && compact(accretor) && donor_mass > accretor_mass
            }
        }
    }

    /// Whether what a transfer does not accrete leaves with the accretor's specific orbital angular
    /// momentum rather than the donor's, for an accretion that marks its loss as the accretor's
    /// (`from_accretor`): never under [`HeavyDonorRules::Nuclear`], which reads no mark.
    #[must_use]
    pub(super) const fn loss_from_accretor(self, from_accretor: bool) -> bool {
        match self {
            Self::Nuclear => false,
            Self::Thermal => from_accretor,
        }
    }

    /// Whether the donor's corotation after a step of transfer onto an accretor of type `accretor`
    /// draws its spin from the orbit, keeping the pair's total angular momentum, where the held
    /// arm returns the transferred spin to the orbit in the step and restores the corotation with
    /// nothing drawn: under [`HeavyDonorRules::Thermal`], onto a white dwarf, a neutron star or a
    /// black hole.
    #[must_use]
    pub(super) fn corotation_from_orbit(self, accretor: Kind) -> bool {
        match self {
            Self::Nuclear => false,
            Self::Thermal => compact(accretor),
        }
    }

    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).heavy_donor
    }
}

/// Whether a star of type `kind` is a white dwarf, a neutron star or a black hole.
#[must_use]
fn compact(kind: Kind) -> bool {
    kind.is_remnant() && kind != Kind::Massless
}
