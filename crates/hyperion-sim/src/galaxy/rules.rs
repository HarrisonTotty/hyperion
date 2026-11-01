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
//! - [`bound_share`](GalaxyRules::bound_share), at [`BOUND_SHARE_VERSION`] (plan 09, P09.T13 after
//!   ruling 145.5): a globular's history takes its own population's μ in place of BM03's 0.70.
//! - [`stream_shell`](GalaxyRules::stream_shell), at [`STREAM_SHELL_VERSION`] (plan 10, P10.T9):
//!   a stream's member outside its bounding shell is none, so the range query may pass by a stream
//!   whose shell its sphere misses.
//! - [`member_remnants`](GalaxyRules::member_remnants), at [`TABULATED_REMNANTS_VERSION`] (plan 09,
//!   P09.T10): a remnant class's member draw asks the cluster's tabulated remnants before it builds
//!   an attempt's star.
//! - [`field_share`](GalaxyRules::field_share), at [`FIELD_SHARE_VERSION`] (plan 09, P09.T2.c):
//!   the field gives up the features' share φ of its systems. It is read where the galaxy is built,
//!   which installs the field's factors, so `Galaxy::with_rules` refuses to change it.
//! - [`carve_out`](GalaxyRules::carve_out), at [`CARVE_OUT_VERSION`] (plan 09, P09.T35): the grid
//!   gives the core-collapse and luminous blue variables' classes their hosts, and layer D gives
//!   up its ancient Type Ia. It is read where the galaxy is built, which installs layer D's
//!   factors, so `Galaxy::with_rules` refuses to change it too.
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
//! - [`orphan_streams`](GalaxyRules::orphan_streams), at [`ORPHAN_STREAMS_VERSION`] (plan 15,
//!   P15.T11.b): the orphans' count and the halo's discrete share take the fitted streams per
//!   globular. It is read where a galaxy's debris is planned (`Debris::plan`, the orphans' draw
//!   and the dwarfs' scale), and reaches none of the galaxy's memos.
//! - [`merger_channels`](GalaxyRules::merger_channels), at [`MERGER_CHANNELS_VERSION`] (plan 11,
//!   P11.T8.c's missing channel, at 27): the stellar merger class adds the engine's dynamical
//!   main-sequence mergers and draws an evolved donor's companion by its donor's kind. It is read
//!   where a merger's scan marks are drawn, and reaches none of the galaxy's memos.
//! - [`stellar`](GalaxyRules::stellar), the stellar stage's holds ([`StellarRules`]), which the
//!   galaxy hands to the stellar stage's entry points as an argument.
//!
//! The next train's bump deletes each galaxy hold with its field, its held arm and its tripwire.
//! The `stellar` field stays through every bump: a bump deletes only a stellar hold's own field
//! inside [`StellarRules`].

use crate::GeneratorVersion;
use crate::galaxy::ages::{FIELD_SHARE_VERSION, FieldShareRules};
use crate::galaxy::catalogue_classes::binary::{
    MERGER_CHANNELS_VERSION, MERGER_CLASS_VERSION, MergerChannelRules, MergerClassRules,
};
use crate::galaxy::displaced::site::{CARVE_OUT_VERSION, CarveOutRules};
use crate::galaxy::displaced::{DISPLACED_VERSION, DisplacedRules};
use crate::galaxy::features::kinds::globular::{BOUND_SHARE_VERSION, BoundShareRules};
use crate::galaxy::features::members::recent::{FEATURE_CLASSES_VERSION, FeatureClassesRules};
use crate::galaxy::features::members::remnants::{MemberRemnantRules, TABULATED_REMNANTS_VERSION};
use crate::galaxy::global_list::orphans::{ORPHAN_STREAMS_VERSION, OrphanStreamsRules};
use crate::galaxy::global_list::{STREAM_SHELL_VERSION, StreamShellRules};
use crate::stellar::rules::StellarRules;

/// The galaxy-side holds of the next galaxy train, carried by the `Galaxy` value
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GalaxyRules {
    /// The share of its birth mass a globular's history says stellar evolution left it (P09.T13,
    /// ruling 145.5).
    pub(crate) bound_share: BoundShareRules,
    /// Whether a stream's bounding shell bounds its members (P10.T9).
    pub(crate) stream_shell: StreamShellRules,
    /// How a remnant class's member draw decides an attempt: its star built, or the cluster's
    /// tabulated remnants first (P09.T10).
    pub(crate) member_remnants: MemberRemnantRules,
    /// Whether the field gives up the features' share φ of its systems (P09.T2.c).
    pub(crate) field_share: FieldShareRules,
    /// Whether the grid gives the catalogue classes their hosts (P09.T35).
    pub(crate) carve_out: CarveOutRules,
    /// Whether features list their recently dead and their luminous blue variables (P09.T36).
    pub(crate) feature_classes: FeatureClassesRules,
    /// How the merger classes draw their entries: every stellar merger a contact pair, or the
    /// engine's channels and a neutron-star remnant less its ejecta (P11.T8.c–d's fix).
    pub(crate) merger_class: MergerClassRules,
    /// Whether layers D and E hold plan 08's displaced classes (P08.T12), which the galaxy's build
    /// reads.
    pub(crate) displaced: DisplacedRules,
    /// The streams per globular the orphans and the discrete share read: the brainstorm's or
    /// P15.T11.b's fit.
    pub(crate) orphan_streams: OrphanStreamsRules,
    /// Which channels the stellar merger class draws under its engine's channels: contact and
    /// evolved donor, or with the dynamical main sequence (P11.T8.c's missing channel).
    pub(crate) merger_channels: MergerChannelRules,
    /// The stellar stage's holds, which `SystemStars`' builders and the galaxy-side readers of the
    /// stellar law read from the galaxy and hand down as an argument.
    pub(crate) stellar: StellarRules,
}

impl GalaxyRules {
    /// Each field's arm at `version`: the new arm from its `*_VERSION`, the held arm below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        Self {
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
            field_share: if version.get() >= FIELD_SHARE_VERSION.get() {
                FieldShareRules::Phi
            } else {
                FieldShareRules::None
            },
            carve_out: if version.get() >= CARVE_OUT_VERSION.get() {
                CarveOutRules::Claimed
            } else {
                CarveOutRules::Unclaimed
            },
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
            orphan_streams: if version.get() >= ORPHAN_STREAMS_VERSION.get() {
                OrphanStreamsRules::Fitted
            } else {
                OrphanStreamsRules::Brainstorm
            },
            merger_channels: if version.get() >= MERGER_CHANNELS_VERSION.get() {
                MergerChannelRules::WithDynamical
            } else {
                MergerChannelRules::ContactAndEvolved
            },
            stellar: StellarRules::of_version(version),
        }
    }

    /// Every galaxy and stellar hold at 26 in force, for a test that asserts a 26 figure: the
    /// version's rules with the holds that turn on at 26 (the galaxy's `bound_share`, P09.T13,
    /// `stream_shell`, P10.T9, `member_remnants`, P09.T10, `field_share`, P09.T2.c, `carve_out`,
    /// P09.T35, `feature_classes`, P09.T36, `merger_class`, P11.T8.c–d, and `displaced`, P08.T12;
    /// the stellar rules' `dwarf_sequence`, P06.T23.a, `helium`, P15.T7.b, `hd_limit`, P06.T42.b,
    /// `v_magnitude`, P06.T43.b, `stripping`, P11.T1.d, `peculiar_classes`, P06.T24.b,
    /// `star_events`, P06.T28.g, and `merger_mass`, P11.T4.d; [`StellarRules::every_26_hold`]).
    ///
    /// A later hold at 26 joins it with nothing to edit, and "every 26 hold but one" is
    /// struct-update syntax over it.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_26_hold() -> Self {
        Self::of_version(GeneratorVersion::new(26))
    }

    /// Every galaxy and stellar hold at 27 in force, for a test that asserts a 27 figure: every 26
    /// hold ([`GalaxyRules::every_26_hold`]) and the galaxy's at 27 (`orphan_streams`, P15.T11.b,
    /// and `merger_channels`, P11.T8.c's missing channel). The planetary stage's holds at 27 are
    /// switched on the thread instead (`planetary::testing::with_every_27_hold`), around a galaxy
    /// built under these.
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
    /// them explicitly; one built under every 26 hold does not.
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
        let at_26 = Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_26_hold(),
        )
        .expect("the Milky Way fixture builds");
        assert_ne!(built, at_26);
        assert_eq!(at_26.rules(), GalaxyRules::every_26_hold());
        // The rules ride the builders that return the same galaxy with something added or
        // changed; `with_full_potential`'s is held by the exact orbit's D4 test, which builds one.
        let substellar = at_26.with_substellar_params(SubstellarParams::generator_default());
        assert_eq!(substellar.rules(), GalaxyRules::every_26_hold());
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
        let bound = galaxy.clone().with_rules(GalaxyRules {
            bound_share: BoundShareRules::OwnMu,
            ..galaxy.rules()
        });
        assert!(bound.centre.0.get().is_some(), "the same stellar rules");
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
        assert_eq!(displaced(26), DisplacedRules::Displaced);
        assert_eq!(displaced(27), DisplacedRules::Displaced);
        assert_eq!(
            GalaxyRules::every_26_hold().displaced,
            DisplacedRules::Displaced
        );
        assert_eq!(
            GalaxyRules::every_26_hold().stellar,
            StellarRules::every_26_hold()
        );
        for v in [23, 24, 25, 26] {
            assert_eq!(
                GalaxyRules::of_version(GeneratorVersion::new(v)).stellar,
                StellarRules::of_version(GeneratorVersion::new(v))
            );
        }
    }

    /// Each galaxy hold at 26 is held below it and new from it on, in every 26 hold.
    #[test]
    fn each_26_hold_turns_on_at_26() {
        let bound = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).bound_share;
        for v in [23, 24, 25] {
            assert_eq!(bound(v), BoundShareRules::Bm03, "{v}");
        }
        assert_eq!(bound(26), BoundShareRules::OwnMu);
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).bound_share,
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
            GalaxyRules::of_version(GENERATOR_VERSION).stream_shell,
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
        let field = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).field_share;
        for v in [23, 24, 25] {
            assert_eq!(field(v), FieldShareRules::None, "{v}");
        }
        assert_eq!(field(26), FieldShareRules::Phi);
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).field_share,
            FieldShareRules::None
        );
        assert_eq!(
            GalaxyRules::every_26_hold().field_share,
            FieldShareRules::Phi
        );
        let carve = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).carve_out;
        for v in [23, 24, 25] {
            assert_eq!(carve(v), CarveOutRules::Unclaimed, "{v}");
        }
        assert_eq!(carve(26), CarveOutRules::Claimed);
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).carve_out,
            CarveOutRules::Unclaimed
        );
        assert_eq!(
            GalaxyRules::every_26_hold().carve_out,
            CarveOutRules::Claimed
        );
        let classes = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).feature_classes;
        for v in [23, 24, 25] {
            assert_eq!(classes(v), FeatureClassesRules::Unlisted, "{v}");
        }
        assert_eq!(classes(26), FeatureClassesRules::Listed);
        assert_eq!(
            GalaxyRules::every_26_hold().feature_classes,
            FeatureClassesRules::Listed
        );
    }

    /// Each galaxy hold at 27 is held below it and new from it on, in every 27 hold.
    #[test]
    fn each_27_hold_turns_on_at_27() {
        let orphans = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).orphan_streams;
        for v in [23, 24, 25, 26] {
            assert_eq!(orphans(v), OrphanStreamsRules::Brainstorm, "{v}");
        }
        assert_eq!(orphans(27), OrphanStreamsRules::Fitted);
        assert_eq!(
            GalaxyRules::every_26_hold().orphan_streams,
            OrphanStreamsRules::Brainstorm
        );
        assert_eq!(
            GalaxyRules::every_27_hold().orphan_streams,
            OrphanStreamsRules::Fitted
        );
    }
}
