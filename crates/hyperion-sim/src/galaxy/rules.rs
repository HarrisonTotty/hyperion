//! The galaxy-side holds of the next galaxy train, carried by the
//! [`Galaxy`](crate::galaxy::Galaxy) value (decision-galaxy-25-train §1).
//!
//! A change that moves generated output, built before the generator version it belongs to, lands
//! behind a hold: a `*_VERSION` constant beside the code it holds, and a field of [`GalaxyRules`]
//! whose new arm that version turns on. A galaxy carries its rules, set from the generator version
//! in force when it is built
//! ([`Galaxy::from_params`](crate::galaxy::Galaxy::from_params)), so below a hold's version every
//! galaxy takes the held arm, which is the old code verbatim, and every generated value keeps its
//! bits. A test reaches a later version's arm by building a galaxy under its rules, never through
//! a switch on the thread: a galaxy under other rules is another value and compares unequal, so a
//! fixture shared between tests is the same whichever test builds it first. The server and
//! `hyperion-fit` build galaxies through `Galaxy::from_params`, so they take the version's rules.
//!
//! The holds, each a field:
//!
//! - [`kepler_regime`](GalaxyRules::kepler_regime), at [`KEPLER_REGIME_VERSION`] (plan 09,
//!   P09.T28.b.b): the Kepler regime in the drift hook, the grid's loss-cone carve-out and the
//!   walks' pad for the orbits.
//!
//! The next train's bump deletes each hold with its field, its held arm and its tripwire.

use crate::GeneratorVersion;
use crate::galaxy::motion::{KEPLER_REGIME_VERSION, KeplerRegimeRules};

/// The galaxy-side holds of the next galaxy train, carried by the `Galaxy` value
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GalaxyRules {
    /// The law a Kepler-regime grid system moves by, whether the loss cone carves the grid's
    /// plunging orbits out, and the pad the walks choose cells by (P09.T28.b.b).
    pub(crate) kepler_regime: KeplerRegimeRules,
}

impl GalaxyRules {
    /// Each field's arm at `version`: the new arm from its `*_VERSION`, the held arm below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        Self {
            kepler_regime: if version.get() >= KEPLER_REGIME_VERSION.get() {
                KeplerRegimeRules::Kepler
            } else {
                KeplerRegimeRules::Straight
            },
        }
    }

    /// Every galaxy hold at 25 in force, for a test that asserts a 25 figure.
    ///
    /// A later hold at 25 joins it with nothing to edit, and "every 25 hold but one" is
    /// struct-update syntax over it.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_25_hold() -> Self {
        Self::of_version(GeneratorVersion::new(25))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::substellar::SubstellarParams;
    use crate::rng::Seed;

    /// D3: a galaxy built the usual way carries the version's rules, and equals one built under
    /// them explicitly; one built under every 25 hold does not.
    #[test]
    fn a_galaxy_carries_the_version_s_rules() {
        let seed = Seed::new(0x0928_bb00_0000_0001);
        let built = Galaxy::from_params(seed, GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds");
        assert_eq!(built.rules(), GalaxyRules::of_version(GENERATOR_VERSION));
        let explicit = Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::milky_way_like(),
            GalaxyRules::of_version(GENERATOR_VERSION),
        )
        .expect("the Milky Way fixture builds");
        assert_eq!(built, explicit);
        assert_eq!(format!("{built:?}"), format!("{explicit:?}"));
        let at_25 = Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_25_hold(),
        )
        .expect("the Milky Way fixture builds");
        assert_ne!(built, at_25);
        assert_eq!(at_25.rules(), GalaxyRules::every_25_hold());
        // The rules ride the builders that return the same galaxy with something added or
        // changed; `with_full_potential`'s is held by the hold's D4 test, which builds one.
        let substellar = at_25.with_substellar_params(SubstellarParams::generator_default());
        assert_eq!(substellar.rules(), GalaxyRules::every_25_hold());
    }

    /// Each hold's arm follows its version: held below it, new from it on.
    #[test]
    fn each_hold_turns_on_at_its_version() {
        let at = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).kepler_regime;
        assert_eq!(at(23), KeplerRegimeRules::Straight);
        assert_eq!(at(24), KeplerRegimeRules::Straight);
        assert_eq!(at(25), KeplerRegimeRules::Kepler);
        assert_eq!(at(26), KeplerRegimeRules::Kepler);
        assert_eq!(
            GalaxyRules::every_25_hold().kepler_regime,
            KeplerRegimeRules::Kepler
        );
    }
}
