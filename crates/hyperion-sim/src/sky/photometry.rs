//! A star's absolute V magnitude as the sky reads it, with the interims of galaxy plan 06's asks
//! A3 and A4 in one place (rendering plan R06, Design note 7).
//!
//! - **A3, protostars dark in V.** A Class 0 protostar sits inside an envelope of A<sub>V</sub>
//!   ≳ 100 (André, Ward-Thompson and Barsony 1993; Whitney et al. 2003b, ApJ 598, 1079, §3), a
//!   Class I one inside A<sub>V</sub> 32–225 towards most inclinations (Whitney et al. 2003a, ApJ
//!   591, 1049, §2 and Fig. 3), so neither has a V magnitude here. A Class I source seen pole-on,
//!   down its outflow cavity (A<sub>V</sub> about 1.5), would show; the interim treats every one as
//!   dark, a few per cent of them wrongly (R06's Risks). Plan 06's protostar phase,
//!   [`Phase::Protostar`], is exactly the accretion that
//!   [`protostar_class`](crate::stellar::premain::protostar_class) calls Class 0 or I (it ends at
//!   [`PROTOSTAR_DURATION`](crate::stellar::premain::PROTOSTAR_DURATION)), so the phase alone
//!   decides. A Class II star, on the pre-main sequence, takes no circumstellar term: its birth
//!   cloud's extinction is plan 07's.
//! - **A4, white dwarfs.** Plan 06's photometry gives no white dwarf a V magnitude
//!   ([`absolute_magnitude_v`]), so a white dwarf is dark here until the Montreal grids (Bédard et
//!   al. 2020) give it one; the census counts it in its tallies. Neutron stars and black holes are
//!   dark too. M giants are as bright as plan 06 says (up to 1.7 mag too bright at M6 III, its own
//!   note).
//!
//! When plan 06 answers A3 and A4, these two functions switch to its photometry and nothing else
//! here changes. Two of the census's bounds rest on A4's interim, and must change with it: the
//! phase envelope's bins beyond a star's lifetime hold only remnants, which it keeps dark
//! ([`PhaseEnvelope`](super::phase::PhaseEnvelope), R06.T8.m), and a pair that plan 11 answers
//! `Remnants` gives the census nothing ([`StarBounds`](super::census::StarBounds), R06.T8.g).
//! Both then take white-dwarf rows.

use super::colour::{StarColour, star_colour, surface_gravity};
use super::disc::grid_of;
use crate::stellar::photometry::absolute_magnitude_v;
use crate::stellar::{Phase, StarState};
use crate::units::Magnitudes;

/// Whether the sky treats `state` as dark in V: a protostar (A3's interim), a white dwarf (A4's
/// interim), a neutron star, a black hole, nothing left, or any state plan 06's photometry gives
/// no V magnitude (an object cooler than its table's 1,710 K, or one of no luminosity).
#[must_use]
pub fn is_dark_in_v(state: &StarState) -> bool {
    absolute_v_of_state(state).is_none()
}

/// The absolute V magnitude the sky gives `state`, or `None` where it is dark in V
/// ([`is_dark_in_v`]): plan 06's [`absolute_magnitude_v`] for every other state.
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::photometry::{absolute_v_of_state, is_dark_in_v};
/// use hyperion_sim::stellar::sse::Track;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let sun = Track::full(SolarMasses::new(1.0), &Composition::SOLAR, &StarDraws::median());
/// // Deep in its envelope at 0.1 Myr, a solar-mass protostar shows nothing in V.
/// assert!(is_dark_in_v(&sun.state_at(Years::new(1e5))));
/// let today = absolute_v_of_state(&sun.state_at(Years::new(4.6e9))).ok_or("visible")?;
/// assert!((today.value() - 4.8).abs() < 0.3);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn absolute_v_of_state(state: &StarState) -> Option<Magnitudes> {
    match state.phase() {
        Phase::Protostar
        | Phase::HeliumWhiteDwarf
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant => None,
        Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::Substellar => absolute_magnitude_v(state),
    }
}

/// The colour of `state` from the spectral table ([`star_colour`]): its effective temperature, its
/// gravity from its own mass and radius ([`surface_gravity`], Design note 6) and the grid of its
/// phase, as the host discs read them.
#[must_use]
pub fn colour_of_state(state: &StarState) -> StarColour {
    star_colour(
        state.effective_temperature(),
        surface_gravity(state.mass(), state.radius()),
        grid_of(state.phase()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::Track;
    use crate::units::{SolarMasses, Years};

    #[test]
    fn protostars_and_white_dwarfs_are_dark_and_main_sequence_stars_are_not() {
        let sun = Track::full(
            SolarMasses::new(1.0),
            &Composition::SOLAR,
            &StarDraws::median(),
        );
        for age in [1.0, 1e4, 1e5, 4.9e5] {
            let state = sun.state_at(Years::new(age));
            assert_eq!(state.phase(), Phase::Protostar, "age {age}");
            assert!(is_dark_in_v(&state));
        }
        let main_sequence = sun.state_at(Years::new(4.6e9));
        assert_eq!(main_sequence.phase(), Phase::MainSequence);
        assert!(!is_dark_in_v(&main_sequence));
        let white_dwarf = sun.state_at(Years::new(1.5e10));
        assert_eq!(white_dwarf.phase(), Phase::CarbonOxygenWhiteDwarf);
        assert!(is_dark_in_v(&white_dwarf));
    }
}
