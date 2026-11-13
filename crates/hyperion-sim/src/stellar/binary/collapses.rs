//! The core collapses that act on the orbits the binary engine does not otherwise run (plan 11,
//! P11.T10's fix).
//!
//! As first built, two kinds of collapse left every orbit as drawn:
//!
//! - **A pair the pre-test passes over** ([`can_interact`](super::can_interact)) was one detached
//!   segment of its stars' own models on the drawn orbit, with no supernova record, even where a
//!   star of it collapsed by the age it was run to, though `run_pairs` (`stellar/system.rs`) runs
//!   it because it holds a remnant. So a wide pair kept a neutron star its 265 km/s kick would have
//!   freed bound on its drawn orbit for ever: of 10⁴ massive systems of the binaries' census
//!   sampler (P11.T10's), 575 primary pairs with a companion of 2.5 M☉ or more, all at log P > 3.5
//!   (days). From version 25 P11.T4.l's closed form applied its stars' collapses to its drawn
//!   orbit, with their records, until the fix ran such a pair through the engine.
//! - **An outer orbit**, one of whose members is a pair: `run_pairs` runs no such pair, and the
//!   system showed its other member on its drawn orbit whatever the inner pair lost and however
//!   fast it recoiled (6,289 companions of 2.5 M☉ or more per 10⁴ of those systems).
//!
//! So:
//!
//! - **A pair a star of which dies suddenly by the age it is run to** (a core's collapse, an
//!   electron capture or a pair instability: the pinned primary's at plan 06's age, any other
//!   star's on its own track) is run through the engine, not passed over, whose
//!   `Engine::explode` applies Hurley, Tout and Pols's (2002, "BSE") appendix A1 to it and
//!   unbinds it where the orbit comes out so ([`runs_pair`]). A white dwarf's birth
//!   runs no pair: in an old galaxy it would run most of the low-mass pairs, and it never brings
//!   the periastron in (`light.rs`, item 3).
//! - **Each supernova of a pair the engine ran acts on the orbit of the pair's node** with the
//!   node's other member, the pair a point mass of its stars' masses: its mass lost and its
//!   recoil, the record's barycentre velocity while it stays bound, by A1
//!   ([`outer_orbit_after`]). An orbit it leaves bound recoils in turn on the orbit above it. One
//!   it unbinds releases its other member, which the system no longer shows (`SystemStars`), and
//!   its primary's member goes on alone, a point mass at its velocity after, which may unbind the
//!   orbit above it in turn.
//!
//! **Leans** (2026-11-01, `fix-p11-t10-released`):
//!
//! - A pair the engine now runs for its collapse is stepped from the first star's arrival, as any
//!   other pair it runs, so its orbit before the collapse is the engine's, widened by the stars'
//!   winds, where a run to an age before the collapse is passed over on the drawn orbit: the
//!   pre-collapse orbit depends on the age the pair is run to. `SystemStars` runs every pair to
//!   the system's age at +H, so a system shows one history. P11.T4.l's closed form, the drawn orbit
//!   up to the collapse, was the held arm's; the ruling (p11-supernova-pins §4.3) found the two
//!   routes' statistics the same.
//! - Such a run brings the wide pairs of a pinned primary to the engine, where the rates can
//!   limit a detached step under its floor, max(10⁻⁹ age, 10⁻³ yr). A step held at its floor
//!   stops at the pin inside it (`Engine::step_limit`): passing it, nothing acted on the pin, and
//!   the primary died on its own track with no supernova (P15.T10.b's refit at 27 found one,
//!   system `0x81c9_b51f_c000_0001`; plan 11's records).
//! - The outer orbit's state at the explosion is its drawn elements' at the explosion's universe
//!   time (design note 6), widened for the mass both members have lost since zero age in Jeans
//!   mode, as P11.T4.l's ruling widens a passed-over pair's ([`outer_orbit_after`]: a M is
//!   constant, e and the true anomaly kept, so r × f and v ÷ f with f the node's initial mass over
//!   its mass before): without it, a hierarchy whose winds had taken half its mass would be
//!   unbound by any loss. A common envelope's ejection in the inner pair, over about a year, is
//!   sudden to a wide outer orbit and widens it more than Jeans mode does: a finding.
//! - An outer orbit a supernova leaves bound stays on its drawn elements, as the system shows it,
//!   and the next supernova below it acts on them again.
//! - Where the primary's member escapes an orbit but not the one above it, the outer star that
//!   stays bound to it cannot be shown without a gap in the shown hierarchy's bodies, and is left
//!   out (`SystemStars::shown_at`): the system shows the largest drawn part on the primary's inner
//!   chain that holds only stars still bound.
//! - Only the supernovae of pairs the engine ran act on outer orbits. A star in no such pair (an
//!   outer star itself, or a primary whose companion is a pair) collapses as its own model, and
//!   leaves its orbit as drawn: the range brief's gate checks only the pairs of two stars, which
//!   is where every release starts.

use crate::coords::SystemVelocity;
use crate::orbit::KeplerElements;
use crate::time::UniverseTime;
use crate::units::{GravitationalParameter, SolarMasses};

/// Whether the engine runs a pair the pre-test passes over, given the earliest sudden death of its
/// stars, years (`None` for none), and the age it is run to, `until_years`: where that death falls
/// by then.
#[must_use]
pub(super) fn runs_pair(first_collapse_years: Option<f64>, until_years: f64) -> bool {
    first_collapse_years.is_some_and(|at| at <= until_years)
}

/// What a supernova below it does to an outer orbit ([`outer_orbit_after`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OuterOrbit {
    /// The orbit stays bound, and its barycentre recoils at this velocity, in the frame of its
    /// barycentre before (BSE equation A14).
    Bound(SystemVelocity),
    /// The orbit is unbound, and each member leaves at this velocity, inner member first, in the
    /// same frame.
    Unbound([SystemVelocity; 2]),
}

/// One supernova below an outer orbit, its pair taken as a point mass (BSE appendix A1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PointLoss {
    /// The member whose pair exploded: 0 the orbit's inner member, 1 its outer one.
    pub(crate) member: usize,
    /// Each member's mass just before the explosion, M☉, inner first.
    pub(crate) before_msun: [f64; 2],
    /// The exploding member's mass just after it, M☉.
    pub(crate) after_msun: f64,
    /// The exploding member's change of velocity, the recoil of its pair's barycentre (or the
    /// exploding star's own velocity where it is all its pair has left), in the frame of that
    /// barycentre before.
    pub(crate) kick: SystemVelocity,
}

/// What `loss`, a supernova of one member of the orbit `orbit` at `t`, does to it (BSE appendix
/// A1, each member a point mass): bound, with its barycentre's recoil, or unbound, with each
/// member's velocity.
///
/// The orbit is drawn with the members' initial masses, `initial_msun` in all (the node's
/// [`node_mass`](crate::stellar::multiplicity::SystemHierarchy::node_mass), whose gravitational
/// parameter the elements hold). Its state at the explosion is the drawn elements' at `t` (design
/// note 6), widened in Jeans mode for the mass both members have lost since zero age (r × f,
/// v ÷ f, f the initial mass over the mass before): the adiabatic invariant a M of slow,
/// isotropic loss with e kept (Jeans 1924, MNRAS 85, 2; Hadjidemetriou 1963, Icarus 2, 440;
/// Hurley, Tout and Pols 2002, section 2.1's orbit under a wind it does not accrete). The members'
/// velocities in the barycentre's frame are then split by mass, the kick added to the exploding
/// one's, and the orbit after is bound where the relative velocity is below the escape speed of
/// the mass after at the separation, v² r < 2 G M′. A member left with no mass holds no orbit:
/// the other leaves at its own velocity.
#[must_use]
pub(crate) fn outer_orbit_after(
    orbit: &KeplerElements,
    t: UniverseTime,
    initial_msun: f64,
    loss: PointLoss,
) -> OuterOrbit {
    let PointLoss {
        member,
        before_msun,
        after_msun,
        kick,
    } = loss;
    let total = before_msun[0] + before_msun[1];
    let mut after = before_msun;
    // Not `max`, which is not exact at a signed zero across targets.
    after[member] = if after_msun > 0.0 { after_msun } else { 0.0 };
    let total_after = after[0] + after[1];
    // No barycentre before: the kicked member leaves at its kick.
    if !(total.is_finite() && total > 0.0) {
        let mut speeds = [SystemVelocity::ZERO; 2];
        speeds[member] = kick;
        return OuterOrbit::Unbound(speeds);
    }
    let (r, v) = orbit.relative_state_at(t);
    let f = initial_msun / total;
    let r = r.metres().map(|c| c * f);
    let v = v.metres_per_second().map(|c| c / f);
    // The outer member relative to the inner, split about the barycentre before.
    let mut inner = v.map(|c| -c * before_msun[1] / total);
    let mut outer = v.map(|c| c * before_msun[0] / total);
    let kicked = if member == 0 { &mut inner } else { &mut outer };
    for (c, k) in kicked.iter_mut().zip(kick.metres_per_second()) {
        *c += k;
    }
    let speeds = [SystemVelocity::new(inner), SystemVelocity::new(outer)];
    if !(after[0] > 0.0 && after[1] > 0.0) {
        return OuterOrbit::Unbound(speeds);
    }
    let relative: [f64; 3] = core::array::from_fn(|k| outer[k] - inner[k]);
    let v2: f64 = relative.iter().map(|c| c * c).sum();
    let separation = r.iter().map(|c| c * c).sum::<f64>().sqrt();
    let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(total_after)).value();
    if v2 * separation < 2.0 * mu {
        let centre: [f64; 3] =
            core::array::from_fn(|k| (after[0] * inner[k] + after[1] * outer[k]) / total_after);
        OuterOrbit::Bound(SystemVelocity::new(centre))
    } else {
        OuterOrbit::Unbound(speeds)
    }
}
