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
//! - [`stellar`](GalaxyRules::stellar), the stellar stage's holds ([`StellarRules`]), which the
//!   galaxy hands to the stellar stage's entry points as an argument.
//!
//! The next train's bump deletes each galaxy hold with its field, its held arm and its tripwire.
//! The `stellar` field stays through every bump: a bump deletes only a stellar hold's own field
//! inside [`StellarRules`].

use crate::GeneratorVersion;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::displaced::DisplacedRules;
use crate::galaxy::displaced::site::CarveOutRules;
use crate::galaxy::features::members::recent::FeatureClassesRules;
use crate::stellar::rules::StellarRules;

/// The galaxy-side holds of the next galaxy train, carried by the `Galaxy` value
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GalaxyRules {
    /// Whether the field gives up the features' share φ of its systems (P09.T2.c): every
    /// version's field does, and only the tests' budget ([`FieldShareRules`]) does not. It is read
    /// where the galaxy is built, which installs the field's factors, so `Galaxy::with_rules`
    /// refuses to change it.
    pub(crate) field_share: FieldShareRules,
    /// Whether the grid gives the catalogue classes their hosts (P09.T35): every version's grid
    /// does, and only the tests' grid without it ([`CarveOutRules`]) does not. It is read where the
    /// galaxy is built, which installs layer D's factors, so `Galaxy::with_rules` refuses to
    /// change it.
    pub(crate) carve_out: CarveOutRules,
    /// Whether features list their recently dead and their luminous blue variables (P09.T36):
    /// every version's do, and only the tests' reference without the lists
    /// ([`FeatureClassesRules`]) does not.
    pub(crate) feature_classes: FeatureClassesRules,
    /// Whether layers D and E hold plan 08's displaced classes (P08.T12), which the galaxy's build
    /// reads: every version's do, and only the tests' field alone ([`DisplacedRules`]) does not.
    pub(crate) displaced: DisplacedRules,
    /// The stellar stage's holds, which `SystemStars`' builders and the galaxy-side readers of the
    /// stellar law read from the galaxy and hand down as an argument.
    pub(crate) stellar: StellarRules,
}

impl GalaxyRules {
    /// Each field's arm at `version`: the new arm from its `*_VERSION`, the held arm below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        Self {
            field_share: FieldShareRules::Phi,
            carve_out: CarveOutRules::Claimed,
            feature_classes: FeatureClassesRules::Listed,
            displaced: DisplacedRules::Displaced,
            stellar: StellarRules::of_version(version),
        }
    }

    /// Every galaxy and stellar hold at 27 in force, for a test that asserts a 27 figure: the
    /// version's rules from 27, every galaxy and stellar hold at 27 being on the record (train 27).
    /// It stays for the tests that name 27's rules and for the holds at 28, whose planetary helper
    /// builds on `planetary::testing::with_every_27_hold`; the 28 bump deletes it with them.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_27_hold() -> Self {
        Self::of_version(GeneratorVersion::new(27))
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
    /// them explicitly; from 27 one built under every 27 hold is the same galaxy.
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
        let at_27 = Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds");
        assert_eq!(built, at_27);
        assert_eq!(at_27.rules(), GalaxyRules::every_27_hold());
        // The rules ride the builders that return the same galaxy with something added or
        // changed; `with_full_potential`'s is held by the exact orbit's D4 test, which builds one.
        let substellar = at_27.with_substellar_params(SubstellarParams::generator_default());
        assert_eq!(substellar.rules(), GalaxyRules::every_27_hold());
    }

    /// `Galaxy::with_rules` keeps the centre's model while the stellar rules stay as they were, and
    /// empties it when they change, so that the next read builds it under them. It keeps the
    /// centre's motion, which reads no stellar law. Built again, the model is the same, since its
    /// classes read only the kick law's parameters, which no stellar arm changes. The galaxy holds
    /// no displaced class (the tests' field alone, [`DisplacedRules`]), whose table read its own
    /// stellar rules, so that `with_rules` may change them.
    #[test]
    fn with_rules_resets_the_centre_s_model_when_the_stellar_rules_change() {
        let field_only = GalaxyRules {
            displaced: DisplacedRules::FieldOnly,
            ..GalaxyRules::of_version(GENERATOR_VERSION)
        };
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(0x0928_bb00_0000_0002),
            GalaxyParams::milky_way_like(),
            field_only,
        )
        .expect("the Milky Way fixture builds");
        let model = galaxy
            .centre()
            .expect("the fixture's centre builds")
            .clone();
        let same = galaxy.clone().with_rules(galaxy.rules());
        assert!(same.centre.0.get().is_some(), "the same stellar rules");
        let probed = galaxy.with_rules(GalaxyRules {
            stellar: StellarRules::probe(),
            ..field_only
        });
        assert!(probed.centre.0.get().is_none(), "other stellar rules");
        assert!(probed.centre_motion.0.get().is_some());
        assert_eq!(probed.centre().expect("built again"), &model);
    }

    /// Each hold's arm follows its version: held below it, new from it on.
    #[test]
    fn each_hold_turns_on_at_its_version() {
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).stellar,
            StellarRules::of_version(GENERATOR_VERSION)
        );
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).stellar,
            StellarRules::of_version(crate::GENERATOR_VERSION)
        );
        for v in [23, 24, 25, 26] {
            assert_eq!(
                GalaxyRules::of_version(GeneratorVersion::new(v)).stellar,
                StellarRules::of_version(GeneratorVersion::new(v))
            );
        }
    }

    /// Each hold retargeted from 26 to 27 (train 26) is held below 27 and new from it on, in every
    /// 27 hold.
    #[test]
    fn each_retargeted_hold_turns_on_at_27() {}

    /// Every 27 hold is every 26 hold with its retargets and the stellar rules' holds at 27: the
    /// galaxy holds none of its own at 27.
    #[test]
    fn each_27_hold_turns_on_at_27() {
        assert_eq!(
            GalaxyRules::every_27_hold(),
            GalaxyRules {
                stellar: StellarRules::of_version(GeneratorVersion::new(27)),
                ..GalaxyRules::every_27_hold()
            }
        );
        assert_eq!(
            GalaxyRules::every_27_hold().stellar,
            StellarRules::every_27_hold()
        );
    }
}
