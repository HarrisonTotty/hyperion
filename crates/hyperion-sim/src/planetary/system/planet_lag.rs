//! An airless planet over a hot ice bulk takes P14.T56.g's rock lag (research-census-25 §3), behind
//! its hold at generator version 26 ([`PLANET_LAG_VERSION`]).
//!
//! decision-p14-t56g §4 gave planets no lag, since under an air a planet's water is a sea, a cover
//! or vapour, and left "an `Airless` `Icy` planet at T0 above 273.15 K" to main as the one case
//! that would change the design. P14.T33.c's census found three in 10⁶ bodies (0.04–0.12 M⊕ at
//! 1.2–1.6 S⊕, kept at 205–215 K by their bright ice). Ice in vacuum retreats there (Hertz–Knudsen
//! at the vapour pressure of Murphy and Koop 2005, QJRMS 131, 1539): free sublimation of 15–60 m a
//! year at 205–215 K, held near a metre a year by the light it absorbs, so over Gyr its surface is
//! the lag, §4's own condition of vacuum or an air far below saturation. From 26 a planet with no
//! given bulk whose bare derivation publishes `Airless` or `Tenuous` over an ice bulk at a T0 above
//! the melting point is derived once more under [`SolvedLag::AboveMeltingOverIce`], by one test
//! ([`planet_takes_the_lag`]) at each site that derives it, its record's and its rings' parent's,
//! and every other planet keeps its bits. The lag takes only what its ice crust held, so the
//! lagged derivation cannot gain an air: there is no feedback.
//!
//! [`SolvedLag::AboveMeltingOverIce`]: crate::planetary::derive::SolvedLag::AboveMeltingOverIce

use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::hooks::figures::cover::UNDER_ICE_MELTING_POINT;
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which an airless planet over a hot ice bulk takes the rock lag: 26,
/// the planetary bump after 25's (research-census-25 §3, ruled 2026-10-31).
///
/// It is this rule's one hold. Below it [`PlanetLagRules::in_force`] is [`PlanetLagRules::Held`]:
/// no planet is lagged, and every figure is version 24's and 25's bit for bit. 26's bump deletes
/// this hold, [`PlanetLagRules`] with its `Held` arm, the tests' switch and its tripwire
/// (`the_planet_lag_is_held_only_until_the_26_bump`).
pub(crate) const PLANET_LAG_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Whether an airless planet over a hot ice bulk takes the lag (its hold, [`PLANET_LAG_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PlanetLagRules {
    /// Version 25's and before: no planet takes the lag.
    Held,
    /// Version 26's: an airless or tenuous planet over a hot ice bulk does.
    Airless,
}

impl PlanetLagRules {
    /// The rules this generator version gives: the airless planets' lag from
    /// [`PLANET_LAG_VERSION`], none below it; in the tests, the rules a test's switch sets on its
    /// thread ([`with_planet_lag`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= PLANET_LAG_VERSION {
            Self::Airless
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The planet lag's rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<PlanetLagRules>> = const { core::cell::Cell::new(None) };
}

/// `run`'s result with the planet lag's rules `rules` on this thread, for the tests to reach
/// version 26's ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_planet_lag<R>(rules: PlanetLagRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<PlanetLagRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Whether an airless planet over a hot ice bulk takes the lag ([`PlanetLagRules::Airless`]); never
/// under its hold.
#[must_use]
pub(crate) fn planet_lag_in_force() -> bool {
    PlanetLagRules::in_force() == PlanetLagRules::Airless
}

/// Whether the planet derived bare as `derived` is derived once more under
/// [`SolvedLag::AboveMeltingOverIce`](crate::planetary::derive::SolvedLag::AboveMeltingOverIce):
/// from 26 where it publishes `Airless` or `Tenuous`, its bulk
/// is ice and its zero-albedo temperature at its flux exceeds [`UNDER_ICE_MELTING_POINT`], the
/// lag's own test to the bit; never under the hold.
#[must_use]
pub(crate) fn planet_takes_the_lag(derived: &DerivedBody) -> bool {
    planet_lag_in_force()
        && matches!(
            derived.atmosphere().published_state(),
            SurfaceState::Airless | SurfaceState::Tenuous
        )
        && SurfaceMaterial::of(&derived.fractions()) == SurfaceMaterial::Ice
        && equilibrium_temperature(derived.flux(), BondAlbedo::from_fraction(0.0))
            > UNDER_ICE_MELTING_POINT
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The planet lag is held until the 25 → 26 bump: the rules in force are version 25's.
    #[test]
    fn the_planet_lag_is_held_only_until_the_26_bump() {
        assert!(
            GENERATOR_VERSION < PLANET_LAG_VERSION,
            "the 25 → 26 bump gives an airless planet over a hot ice bulk the rock lag \
             (research-census-25 §3): delete `PLANET_LAG_VERSION`, `PlanetLagRules` with its \
             `Held` arm and its arm in `planet_takes_the_lag`, the tests' `with_planet_lag`, its \
             switch in `with_every_26_hold` and this test"
        );
        assert_eq!(PlanetLagRules::in_force(), PlanetLagRules::Held);
        assert_eq!(
            crate::planetary::testing::with_every_26_hold(PlanetLagRules::in_force),
            PlanetLagRules::Airless
        );
        assert_eq!(
            with_planet_lag(PlanetLagRules::Airless, PlanetLagRules::in_force),
            PlanetLagRules::Airless
        );
        assert_eq!(PlanetLagRules::in_force(), PlanetLagRules::Held);
    }
}
