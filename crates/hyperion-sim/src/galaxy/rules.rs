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
//! - [`feature_classes`](GalaxyRules::feature_classes), at [`FEATURE_CLASSES_VERSION`] (plan 09,
//!   P09.T36): catalogue features and the galactic centre list their recently dead and their
//!   luminous blue variables, and their band-E cells give them up. It is read where a list or an
//!   interior is built, and reaches none of the galaxy's memos.
//! - [`merger_class`](GalaxyRules::merger_class), at [`MERGER_CLASS_VERSION`] (plan 11,
//!   P11.T8.c–d's fix): the stellar merger class draws the engine's channels, a contact pair at
//!   the tidal limit or an evolved donor whose common envelope merges it, and a neutron-star
//!   merger's remnant keeps the pair's mass less its ejecta. It is read where a merger's scan
//!   marks are drawn, and reaches none of the galaxy's memos.
//! - [`displaced`](GalaxyRules::displaced), at [`DISPLACED_VERSION`] (plan 08, P08.T12): the
//!   displaced classes in layers D and E. It is read when the galaxy is built, after the field share
//!   and the carve-out, by the build's one `match` (`Galaxy::from_params`), which installs the
//!   classes and the stay shares that every reader of the two layers then reads; so
//!   `Galaxy::with_rules` refuses to change it as well.
//! - [`stellar`](GalaxyRules::stellar), the stellar stage's holds ([`StellarRules`]), which the
//!   galaxy hands to the stellar stage's entry points as an argument.
//!
//! The next train's bump deletes each galaxy hold with its field, its held arm and its tripwire.
//! The `stellar` field stays through every bump: a bump deletes only a stellar hold's own field
//! inside [`StellarRules`].

use crate::GeneratorVersion;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::catalogue_classes::binary::{MERGER_CLASS_VERSION, MergerClassRules};
use crate::galaxy::displaced::site::CarveOutRules;
use crate::galaxy::displaced::{DISPLACED_VERSION, DisplacedRules};
use crate::galaxy::features::members::recent::{FEATURE_CLASSES_VERSION, FeatureClassesRules};
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
    /// Whether features list their recently dead and their luminous blue variables (P09.T36).
    pub(crate) feature_classes: FeatureClassesRules,
    /// How the merger classes draw their entries: every stellar merger a contact pair, or the
    /// engine's channels and a neutron-star remnant less its ejecta (P11.T8.c–d's fix).
    pub(crate) merger_class: MergerClassRules,
    /// Whether layers D and E hold plan 08's displaced classes (P08.T12), which the galaxy's build
    /// reads.
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
            feature_classes: if version.get() >= FEATURE_CLASSES_VERSION.get() {
                FeatureClassesRules::Listed
            } else {
                FeatureClassesRules::Unlisted
            },
            merger_class: if version.get() >= MERGER_CLASS_VERSION.get() {
                MergerClassRules::Channels
            } else {
                MergerClassRules::ContactOnly
            },
            displaced: if version.get() >= DISPLACED_VERSION.get() {
                DisplacedRules::Displaced
            } else {
                DisplacedRules::FieldOnly
            },
            stellar: StellarRules::of_version(version),
        }
    }

    /// The version's rules with the holds retargeted from 26 to 27 in force too (train 26,
    /// 2026-11-01: P09.T2.c's `field_share`, P09.T35's `carve_out` (2026-11-02), P09.T36's
    /// `feature_classes`, P11.T8.c–d's `merger_class` and P08.T12's `displaced`; the stellar
    /// rules', [`StellarRules::every_26_hold_with_its_retargets`], hold none since P11.T4.d's
    /// `merger_mass` was folded): the rules their tests, and those of the 27 holds built over them,
    /// were measured under while they were held at 26. The 27 bump deletes it with them.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_26_hold_with_its_retargets() -> Self {
        Self {
            feature_classes: FeatureClassesRules::Listed,
            merger_class: MergerClassRules::Channels,
            displaced: DisplacedRules::Displaced,
            stellar: StellarRules::every_26_hold_with_its_retargets(),
            ..Self::of_version(crate::GENERATOR_VERSION)
        }
    }

    /// Every galaxy and stellar hold at 27 in force, for a test that asserts a 27 figure: the
    /// version's rules with the galaxy's holds retargeted from 26 (`feature_classes`, P09.T36,
    /// `merger_class`, P11.T8.c–d, and `displaced`, P08.T12); the stellar rules hold none at 27
    /// ([`StellarRules::every_27_hold`]).
    /// The planetary stage's holds at 27 are switched on the thread instead
    /// (`planetary::testing::with_every_27_hold`), around a galaxy built under these; a galaxy
    /// hold has no switch and joins here.
    ///
    /// A later hold at 27 joins it with nothing to edit, and "every 27 hold but one" is
    /// struct-update syntax over it.
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
    /// them explicitly; one built under every 27 hold does not.
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
        assert_ne!(built, at_27);
        assert_eq!(at_27.rules(), GalaxyRules::every_27_hold());
        // The rules ride the builders that return the same galaxy with something added or
        // changed; `with_full_potential`'s is held by the exact orbit's D4 test, which builds one.
        let substellar = at_27.with_substellar_params(SubstellarParams::generator_default());
        assert_eq!(substellar.rules(), GalaxyRules::every_27_hold());
    }

    /// `Galaxy::with_rules` keeps the centre's model while the stellar rules stay as they were, and
    /// empties it when they change, so that the next read builds it under them. It keeps the
    /// centre's motion, which reads no stellar law. Built again, the model is the same, since its
    /// classes read only the kick law's parameters, which no stellar arm changes.
    #[test]
    fn with_rules_resets_the_centre_s_model_when_the_stellar_rules_change() {
        let galaxy = Galaxy::from_params(
            Seed::new(0x0928_bb00_0000_0002),
            GalaxyParams::milky_way_like(),
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
            ..GalaxyRules::of_version(GENERATOR_VERSION)
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
        let displaced = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).displaced;
        assert_eq!(displaced(24), DisplacedRules::FieldOnly);
        assert_eq!(displaced(25), DisplacedRules::FieldOnly);
        assert_eq!(displaced(26), DisplacedRules::FieldOnly);
        assert_eq!(displaced(27), DisplacedRules::Displaced);
        assert_eq!(
            GalaxyRules::every_27_hold().displaced,
            DisplacedRules::Displaced
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
    fn each_retargeted_hold_turns_on_at_27() {
        let classes = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).feature_classes;
        for v in [23, 24, 25, 26] {
            assert_eq!(classes(v), FeatureClassesRules::Unlisted, "{v}");
        }
        assert_eq!(classes(27), FeatureClassesRules::Listed);
        assert_eq!(
            GalaxyRules::every_27_hold().feature_classes,
            FeatureClassesRules::Listed
        );
    }

    /// Every 27 hold is every 26 hold with its retargets and the stellar rules' holds at 27: the
    /// galaxy holds none of its own at 27.
    #[test]
    fn each_27_hold_turns_on_at_27() {
        assert_eq!(
            GalaxyRules::every_27_hold(),
            GalaxyRules {
                stellar: StellarRules::of_version(GeneratorVersion::new(27)),
                ..GalaxyRules::every_26_hold_with_its_retargets()
            }
        );
        assert_eq!(
            GalaxyRules::every_27_hold().stellar,
            StellarRules::every_27_hold()
        );
    }
}
