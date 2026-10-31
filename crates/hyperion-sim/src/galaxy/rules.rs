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
//! - [`kepler_secular`](GalaxyRules::kepler_secular), at [`KEPLER_SECULAR_VERSION`] (plan 09,
//!   P09.T28.d.b): the exact orbit in the centre's potential for a Kepler-regime orbit bound in it.
//! - [`centre_quiescence`](GalaxyRules::centre_quiescence), at [`CENTRE_QUIESCENCE_VERSION`] (plan
//!   09, P09.T30.c): the radiative efficiency of the black hole's disruption tails, falling with
//!   their fallback rate. It is read where a galaxy's victims are drawn (`CentreVictims::new` and
//!   `one`), into each tail, and reaches none of the galaxy's memos, so `Galaxy::with_rules` need
//!   not reset them for it.
//! - [`bound_share`](GalaxyRules::bound_share), at [`BOUND_SHARE_VERSION`] (plan 09, P09.T13 after
//!   ruling 145.5): a globular's history takes its own population's μ in place of BM03's 0.70.
//! - [`stream_shell`](GalaxyRules::stream_shell), at [`STREAM_SHELL_VERSION`] (plan 10, P10.T9):
//!   a stream's member outside its bounding shell is none, so the range query may pass by a stream
//!   whose shell its sphere misses.
//! - [`member_remnants`](GalaxyRules::member_remnants), at [`TABULATED_REMNANTS_VERSION`] (plan 09,
//!   P09.T10): a remnant class's member draw asks the cluster's tabulated remnants before it builds
//!   an attempt's star.
//! - [`stellar`](GalaxyRules::stellar), the stellar stage's holds ([`StellarRules`]), which the
//!   galaxy hands to the stellar stage's entry points as an argument.
//!
//! The next train's bump deletes each galaxy hold with its field, its held arm and its tripwire.
//! The `stellar` field stays through every bump: a bump deletes only a stellar hold's own field
//! inside [`StellarRules`].

use crate::GeneratorVersion;
use crate::galaxy::features::centre::events::luminosity::{
    CENTRE_QUIESCENCE_VERSION, CentreQuiescenceRules,
};
use crate::galaxy::features::kinds::globular::{BOUND_SHARE_VERSION, BoundShareRules};
use crate::galaxy::features::members::remnants::{MemberRemnantRules, TABULATED_REMNANTS_VERSION};
use crate::galaxy::global_list::{STREAM_SHELL_VERSION, StreamShellRules};
use crate::galaxy::motion::{
    KEPLER_REGIME_VERSION, KEPLER_SECULAR_VERSION, KeplerRegimeRules, KeplerSecularRules,
};
use crate::stellar::rules::StellarRules;

/// The galaxy-side holds of the next galaxy train, carried by the `Galaxy` value
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GalaxyRules {
    /// The law a Kepler-regime grid system moves by, whether the loss cone carves the grid's
    /// plunging orbits out, and the pad the walks choose cells by (P09.T28.b.b).
    pub(crate) kepler_regime: KeplerRegimeRules,
    /// The law a Kepler-regime orbit follows: the two-body law, or its exact orbit in the centre's
    /// potential (P09.T28.d.b).
    pub(crate) kepler_secular: KeplerSecularRules,
    /// The efficiency the black hole's disruption tails radiate at (P09.T30.c).
    pub(crate) centre_quiescence: CentreQuiescenceRules,
    /// The share of its birth mass a globular's history says stellar evolution left it (P09.T13,
    /// ruling 145.5).
    pub(crate) bound_share: BoundShareRules,
    /// Whether a stream's bounding shell bounds its members (P10.T9).
    pub(crate) stream_shell: StreamShellRules,
    /// How a remnant class's member draw decides an attempt: its star built, or the cluster's
    /// tabulated remnants first (P09.T10).
    pub(crate) member_remnants: MemberRemnantRules,
    /// The stellar stage's holds, which `SystemStars`' builders and the galaxy-side readers of the
    /// stellar law read from the galaxy and hand down as an argument.
    pub(crate) stellar: StellarRules,
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
            kepler_secular: if version.get() >= KEPLER_SECULAR_VERSION.get() {
                KeplerSecularRules::Spherical
            } else {
                KeplerSecularRules::TwoBody
            },
            centre_quiescence: if version.get() >= CENTRE_QUIESCENCE_VERSION.get() {
                CentreQuiescenceRules::HotFlow
            } else {
                CentreQuiescenceRules::ConstantEfficiency
            },
            bound_share: if version.get() >= BOUND_SHARE_VERSION.get() {
                BoundShareRules::OwnMu
            } else {
                BoundShareRules::Bm03
            },
            stream_shell: if version.get() >= STREAM_SHELL_VERSION.get() {
                StreamShellRules::Shell
            } else {
                StreamShellRules::Unbounded
            },
            member_remnants: if version.get() >= TABULATED_REMNANTS_VERSION.get() {
                MemberRemnantRules::Tabulated
            } else {
                MemberRemnantRules::EveryAttempt
            },
            stellar: StellarRules::of_version(version),
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

    /// Every galaxy and stellar hold at 26 in force, for a test that asserts a 26 figure: 25's
    /// holds and those that turn on at 26 (the galaxy's `bound_share`, P09.T13, `stream_shell`,
    /// P10.T9, and `member_remnants`, P09.T10; the stellar rules' `dwarf_sequence`, P06.T23.a,
    /// `helium`, P15.T7.b, and `hd_limit`, P06.T42.b; [`StellarRules::every_26_hold`]).
    ///
    /// A later hold at 26 joins it with nothing to edit, and "every 26 hold but one" is
    /// struct-update syntax over it.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_26_hold() -> Self {
        Self::of_version(GeneratorVersion::new(26))
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
        let kepler = galaxy.clone().with_rules(GalaxyRules {
            kepler_regime: KeplerRegimeRules::Kepler,
            ..galaxy.rules()
        });
        assert!(kepler.centre.0.get().is_some(), "the same stellar rules");
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
        let at = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).kepler_regime;
        assert_eq!(at(23), KeplerRegimeRules::Straight);
        assert_eq!(at(24), KeplerRegimeRules::Straight);
        assert_eq!(at(25), KeplerRegimeRules::Kepler);
        assert_eq!(at(26), KeplerRegimeRules::Kepler);
        assert_eq!(
            GalaxyRules::every_25_hold().kepler_regime,
            KeplerRegimeRules::Kepler
        );
        let secular = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).kepler_secular;
        assert_eq!(secular(23), KeplerSecularRules::TwoBody);
        assert_eq!(secular(24), KeplerSecularRules::TwoBody);
        assert_eq!(secular(25), KeplerSecularRules::Spherical);
        assert_eq!(secular(26), KeplerSecularRules::Spherical);
        assert_eq!(
            GalaxyRules::every_25_hold().kepler_secular,
            KeplerSecularRules::Spherical
        );
        let quiescence = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).centre_quiescence;
        assert_eq!(quiescence(23), CentreQuiescenceRules::ConstantEfficiency);
        assert_eq!(quiescence(24), CentreQuiescenceRules::ConstantEfficiency);
        assert_eq!(quiescence(25), CentreQuiescenceRules::HotFlow);
        assert_eq!(quiescence(26), CentreQuiescenceRules::HotFlow);
        assert_eq!(
            GalaxyRules::every_25_hold().centre_quiescence,
            CentreQuiescenceRules::HotFlow
        );
        assert_eq!(
            GalaxyRules::every_25_hold().stellar,
            StellarRules::every_25_hold()
        );
        assert_eq!(
            GalaxyRules::every_26_hold().stellar,
            StellarRules::every_26_hold()
        );
        assert_eq!(
            GalaxyRules::every_26_hold().kepler_regime,
            KeplerRegimeRules::Kepler
        );
        let bound = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).bound_share;
        for v in [23, 24, 25] {
            assert_eq!(bound(v), BoundShareRules::Bm03, "{v}");
        }
        assert_eq!(bound(26), BoundShareRules::OwnMu);
        assert_eq!(
            GalaxyRules::every_25_hold().bound_share,
            BoundShareRules::Bm03
        );
        assert_eq!(
            GalaxyRules::every_26_hold().bound_share,
            BoundShareRules::OwnMu
        );
        let shell = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).stream_shell;
        for v in [23, 24, 25] {
            assert_eq!(shell(v), StreamShellRules::Unbounded, "{v}");
        }
        assert_eq!(shell(26), StreamShellRules::Shell);
        assert_eq!(
            GalaxyRules::every_25_hold().stream_shell,
            StreamShellRules::Unbounded
        );
        assert_eq!(
            GalaxyRules::every_26_hold().stream_shell,
            StreamShellRules::Shell
        );
        let remnants = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).member_remnants;
        for v in [23, 24, 25] {
            assert_eq!(remnants(v), MemberRemnantRules::EveryAttempt, "{v}");
        }
        assert_eq!(remnants(26), MemberRemnantRules::Tabulated);
        assert_eq!(
            GalaxyRules::every_26_hold().member_remnants,
            MemberRemnantRules::Tabulated
        );
        for v in [23, 24, 25, 26] {
            assert_eq!(
                GalaxyRules::of_version(GeneratorVersion::new(v)).stellar,
                StellarRules::of_version(GeneratorVersion::new(v))
            );
        }
    }
}
