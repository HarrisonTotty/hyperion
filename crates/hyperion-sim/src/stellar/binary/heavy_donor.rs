//! Stable transfer from a main-sequence donor heavier than its white dwarf, neutron star or black
//! hole, and the transfer's angular momentum (plan 11, P11.T4's fix).
//!
//! As first built, a 1.4 M☉ main-sequence donor at its lobe beside a 0.6 M☉ white dwarf
//! transferred at about 10⁻¹⁰ M☉ yr⁻¹ for 3.4 Gyr, a nuclear rate, while its orbit widened from 12
//! to 18 h; such a donor's thermal rate, M ÷ `τ_KH`, is 4.4 × 10⁻⁷. In the slab's census (P11.T4's
//! fix, its research of 2026-11-01) 57% of the hydrogen-donor cataclysmic variables had a donor
//! heavier than the white dwarf, and 60% lay above 6 h. Two departures from BSE's book of the
//! transfer's angular momentum widened the orbit:
//!
//! - what is not accreted left with the donor's specific orbital angular momentum always, though
//!   the accretion marks novae and super-Eddington loss as leaving from the accretor
//!   (`Accretion::from_accretor`), as the module documentation of `rlof` says the published code
//!   has it: never read, so a nova's ejecta from a 0.6 M☉ white dwarf beside a 1.4 M☉ donor took
//!   0.18 of what they would, (0.3 ÷ 0.7)²;
//! - the donor's transferred spin was returned to the orbit (BSE's `jorb + djt`) but not taken
//!   from the donor, whose corotation was then restored with nothing drawn from the orbit, so each
//!   step added about x `R_L`² ω of angular momentum to the pair (BSE's code keeps the pair's total
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
//! So:
//!
//! - a main-sequence donor (BSE's type 1) heavier than its white dwarf, neutron star or black hole
//!   is held to its thermal rate, and transfers at it wherever a step at it still leaves the donor
//!   overfilling its lobe ([`thermal_rate`]);
//! - what is not accreted leaves with the accretor's specific orbital angular momentum where the
//!   accretion marks it so (`Accretion::from_accretor`);
//! - onto a white dwarf, a neutron star or a black hole, the transfer's step returns no spin to the
//!   orbit, and the donor's corotation after it draws its spin from the orbit, the pair's total
//!   kept ([`corotation_from_orbit`]), its envelope's spin at its radius held to
//!   its Roche lobe, as BSE's transfer reads a donor that overfills it (`evolv2.f`'s `radx`;
//!   P11.T10's kicks at version 25: at the radius it would have alone, a case BB helium giant
//!   stripped toward its core, which HPT's helium-giant radius swells far past its lobe, drained
//!   its orbit to nothing and merged its neutron stars).
//!
//! Onto any other accretor the transfer keeps the first book of its spin (a lean of 2026-11-01):
//! forced on at version 24, the corotation drawn from the orbit moves the paper's
//! Algol, a W Ursae Majoris contact and the double neutron stars' channel, none of which this fix
//! is for, and the general departure is left for its own fix.

use super::star::Kind;

/// Whether a donor of type `donor` and `donor_mass` M☉ beside an accretor of type `accretor` and
/// `accretor_mass` M☉ is held to its thermal rate, and transfers at it wherever a step at it leaves
/// the donor overfilling: a main-sequence donor of BSE's type 1 heavier than a white dwarf, a
/// neutron star or a black hole.
#[must_use]
pub(super) fn thermal_rate(
    donor: Kind,
    accretor: Kind,
    donor_mass: f64,
    accretor_mass: f64,
) -> bool {
    donor == Kind::MainSequence && compact(accretor) && donor_mass > accretor_mass
}

/// Whether the donor's corotation after a step of transfer onto an accretor of type `accretor`
/// draws its spin from the orbit, keeping the pair's total angular momentum, its envelope's spin at
/// its radius held to its Roche lobe, where otherwise the step returns the transferred spin to the
/// orbit and the corotation is restored with nothing drawn (`Engine::corotate_donor`): onto a white
/// dwarf, a neutron star or a black hole.
#[must_use]
pub(super) fn corotation_from_orbit(accretor: Kind) -> bool {
    compact(accretor)
}

/// Whether a star of type `kind` is a white dwarf, a neutron star or a black hole.
#[must_use]
fn compact(kind: Kind) -> bool {
    kind.is_remnant() && kind != Kind::Massless
}
