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
//! - [`kepler_arc`](GalaxyRules::kepler_arc), at [`KEPLER_ARC_VERSION`] (plan 09, P09.T28.d.c):
//!   a non-circular exact orbit's state within half a radial period of the epoch by the arc from
//!   the epoch. It is read where a trajectory is prepared (`Trajectory::new`), and reaches none of
//!   the galaxy's memos.
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
//! - [`binary_tables`](GalaxyRules::binary_tables), at [`BINARY_TABLES_VERSION`] (plan 15,
//!   P15.T10.b): the binary classes read plan 15's fitted samplers and share rows. It is read where
//!   the galaxy is built, which installs the share matrix's class shares; `Galaxy::with_rules`
//!   installs them again, and refuses to change it under the field share, which reads them too.
//! - [`class_source`](GalaxyRules::class_source), at [`CLASS_SOURCE_VERSION`] (plan 09,
//!   P09.T37.a): the range query takes the catalogue classes' hosts beside the grid. It is read
//!   where a caller asks for the query's sources (`CatalogueClassSource::registered_in`), and
//!   reaches none of the galaxy's memos.
//! - [`feature_binaries`](GalaxyRules::feature_binaries), at [`FEATURE_BINARIES_VERSION`] (plan
//!   11, P11.T8.f): catalogue features and the galactic centre list plan 11's binary classes, the
//!   catalogue's classes give up the features' share of their hosts, and a feature's binary members
//!   are drawn inside its hard–soft boundary. It is read where a list, a class's process or a
//!   member is built, and reaches none of the galaxy's memos.
//! - [`runaway_deaths`](GalaxyRules::runaway_deaths), at [`RUNAWAY_DEATHS_VERSION`] (plan 09,
//!   P09.T33.b, at 27): the core-collapse class follows the young field's runaways and walkaways,
//!   its field term at the band budget keeping only the deaths its ejection marks leave it. It is
//!   read where the class's process is built and its candidates drawn, and reaches none of the
//!   galaxy's memos.
//! - [`hypervelocity`](GalaxyRules::hypervelocity), at [`HYPERVELOCITY_VERSION`] (plan 09,
//!   P09.T34.b): the hypervelocity survivors' class has its weight in layer D. It is read where the
//!   class table is built, with the galaxy, so `Galaxy::with_rules` refuses to change it.
//! - [`orphan_spray`](GalaxyRules::orphan_spray), at [`ORPHAN_SPRAY_VERSION`] (plan 10, P10.T10.b's
//!   warm-up, at 27): an orphan's tube is sprayed from 500 tracers an arm at ruling 146.3's `τ_p ÷
//!   32`. It is read where a stream's spray is set (`SprayParams::of_stream`), and reaches none of
//!   the galaxy's memos.
//! - [`placement_fates`](GalaxyRules::placement_fates), at [`PLACEMENT_FATES_VERSION`] (plan 08,
//!   P08.T12 at 27): a dead record's marks read its progenitor's fate from the fitted fate table
//!   where it can stand for the track's, and draw its time since death after the table's bracket.
//!   It is read where those marks are drawn, and reaches none of the galaxy's memos.
//! - [`list_source`](GalaxyRules::list_source), at [`LIST_SOURCE_VERSION`] (plan 10, P10.T10.b's
//!   registration member and P10.T3.d.b, with plan 09's P09.T37.b): the range query takes the global
//!   list's streams, dwarf cores and galactic centre beside the grid, and the halo's field gives up
//!   its discrete share. It is read where a caller asks for the query's sources
//!   (`GlobalListSource::is_registered_in`) and where the galaxy is built, which stores the share
//!   in its feature shares before the field's factors are taken from them, so `Galaxy::with_rules`
//!   refuses to change it.
//! - [`stellar`](GalaxyRules::stellar), the stellar stage's holds ([`StellarRules`]), which the
//!   galaxy hands to the stellar stage's entry points as an argument.
//!
//! The next train's bump deletes each galaxy hold with its field, its held arm and its tripwire.
//! The `stellar` field stays through every bump: a bump deletes only a stellar hold's own field
//! inside [`StellarRules`].

use crate::GeneratorVersion;
use crate::galaxy::ages::{FIELD_SHARE_VERSION, FieldShareRules};
use crate::galaxy::catalogue_classes::binary::feature::{
    FEATURE_BINARIES_VERSION, FeatureBinariesRules,
};
use crate::galaxy::catalogue_classes::binary::{
    BINARY_TABLES_VERSION, BinaryTableRules, MERGER_CHANNELS_VERSION, MERGER_CLASS_VERSION,
    MergerChannelRules, MergerClassRules,
};
use crate::galaxy::catalogue_classes::core_collapse::{RUNAWAY_DEATHS_VERSION, RunawayDeathsRules};
use crate::galaxy::catalogue_classes::source::{CLASS_SOURCE_VERSION, ClassSourceRules};
use crate::galaxy::displaced::site::{CARVE_OUT_VERSION, CarveOutRules};
use crate::galaxy::displaced::survivors::{HYPERVELOCITY_VERSION, HypervelocityRules};
use crate::galaxy::displaced::{DISPLACED_VERSION, DisplacedRules};
use crate::galaxy::features::members::recent::{FEATURE_CLASSES_VERSION, FeatureClassesRules};
use crate::galaxy::global_list::orphans::{ORPHAN_STREAMS_VERSION, OrphanStreamsRules};
use crate::galaxy::global_list::spray::{ORPHAN_SPRAY_VERSION, OrphanSprayRules};
use crate::galaxy::global_list::{LIST_SOURCE_VERSION, ListSourceRules};
use crate::galaxy::motion::spherical::{KEPLER_ARC_VERSION, KeplerArcRules};
use crate::galaxy::placement::{PLACEMENT_FATES_VERSION, PlacementFateRules};
use crate::stellar::rules::StellarRules;

/// The galaxy-side holds of the next galaxy train, carried by the `Galaxy` value
/// (decision-galaxy-25-train §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct GalaxyRules {
    /// How an exact orbit takes its state: from the series at every time, or within half a radial
    /// period of the epoch by the arc from the epoch (P09.T28.d.c).
    pub(crate) kepler_arc: KeplerArcRules,
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
    /// Which tables the binary classes read: the scratch ones or plan 15's fitted ones (P15.T10.b).
    pub(crate) binary_tables: BinaryTableRules,
    /// Whether the range query registers the catalogue classes beside the grid (P09.T37.a).
    pub(crate) class_source: ClassSourceRules,
    /// Whether features hold plan 11's binary classes (P11.T8.f).
    pub(crate) feature_binaries: FeatureBinariesRules,
    /// Whether the core-collapse class follows the young field's runaways and walkaways
    /// (P09.T33.b).
    pub(crate) runaway_deaths: RunawayDeathsRules,
    /// Whether the hypervelocity survivors' class has its weight in layer D (P09.T34.b), which the
    /// class table reads where the galaxy is built.
    pub(crate) hypervelocity: HypervelocityRules,
    /// How an orphan's tube is sprayed: as every stream's, or from fewer tracers at a longer step
    /// (P10.T10.b's warm-up).
    pub(crate) orphan_spray: OrphanSprayRules,
    /// How a dead record's marks read its progenitor's fate: the track's, or the fate table's
    /// where it can stand for it (P08.T12 at 27).
    pub(crate) placement_fates: PlacementFateRules,
    /// Whether the range query registers the global list's members, the centre's among them, and
    /// the galaxy stores the halo's discrete share (P10.T10.b's registration member, P10.T3.d.b),
    /// which the build reads.
    pub(crate) list_source: ListSourceRules,
    /// The stellar stage's holds, which `SystemStars`' builders and the galaxy-side readers of the
    /// stellar law read from the galaxy and hand down as an argument.
    pub(crate) stellar: StellarRules,
}

impl GalaxyRules {
    /// Each field's arm at `version`: the new arm from its `*_VERSION`, the held arm below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        Self {
            kepler_arc: if version.get() >= KEPLER_ARC_VERSION.get() {
                KeplerArcRules::Local
            } else {
                KeplerArcRules::Series
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
            binary_tables: if version.get() >= BINARY_TABLES_VERSION.get() {
                BinaryTableRules::Fitted
            } else {
                BinaryTableRules::Scratch
            },
            class_source: if version.get() >= CLASS_SOURCE_VERSION.get() {
                ClassSourceRules::Registered
            } else {
                ClassSourceRules::Unregistered
            },
            feature_binaries: if version.get() >= FEATURE_BINARIES_VERSION.get() {
                FeatureBinariesRules::Listed
            } else {
                FeatureBinariesRules::Unlisted
            },
            runaway_deaths: if version.get() >= RUNAWAY_DEATHS_VERSION.get() {
                RunawayDeathsRules::Ejected
            } else {
                RunawayDeathsRules::FieldOnly
            },
            hypervelocity: if version.get() >= HYPERVELOCITY_VERSION.get() {
                HypervelocityRules::Weighted
            } else {
                HypervelocityRules::Unweighted
            },
            orphan_spray: if version.get() >= ORPHAN_SPRAY_VERSION.get() {
                OrphanSprayRules::Coarse
            } else {
                OrphanSprayRules::Exact
            },
            placement_fates: if version.get() >= PLACEMENT_FATES_VERSION.get() {
                PlacementFateRules::Fitted
            } else {
                PlacementFateRules::Exact
            },
            list_source: if version.get() >= LIST_SOURCE_VERSION.get() {
                ListSourceRules::Registered
            } else {
                ListSourceRules::Unregistered
            },
            stellar: StellarRules::of_version(version),
        }
    }

    /// The version's rules with the holds retargeted from 26 to 27 in force too (train 26,
    /// 2026-11-01: P09.T2.c's `field_share`, P09.T35's `carve_out` (2026-11-02), P09.T36's
    /// `feature_classes`, P11.T8.c–d's `merger_class`, P08.T12's `displaced` and the stellar rules'
    /// `merger_mass`, P11.T4.d; [`StellarRules::every_26_hold_with_its_retargets`]): the rules
    /// their tests, and those of the 27 holds built over them, were measured under while they were
    /// held at 26. The 27 bump deletes it with them.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn every_26_hold_with_its_retargets() -> Self {
        Self {
            field_share: FieldShareRules::Phi,
            carve_out: CarveOutRules::Claimed,
            feature_classes: FeatureClassesRules::Listed,
            merger_class: MergerClassRules::Channels,
            displaced: DisplacedRules::Displaced,
            stellar: StellarRules::every_26_hold_with_its_retargets(),
            ..Self::of_version(crate::GENERATOR_VERSION)
        }
    }

    /// Every galaxy and stellar hold at 27 in force, for a test that asserts a 27 figure: the
    /// version's rules with the galaxy's holds at 27 (`orphan_streams`, P15.T11.b,
    /// `merger_channels`, P11.T8.c's missing channel, `binary_tables`, P15.T10.b, `class_source`,
    /// P09.T37.a, `feature_binaries`, P11.T8.f, `runaway_deaths`, P09.T33.b, `hypervelocity`,
    /// P09.T34.b, `orphan_spray`, P10.T10.b's warm-up, `placement_fates`, P08.T12 at 27,
    /// `list_source`, P10.T10.b's registration with P10.T3.d.b, and `kepler_arc`, P09.T28.d.c, and
    /// those retargeted from 26, `field_share`, P09.T2.c, `carve_out`, P09.T35, `feature_classes`,
    /// P09.T36, `merger_class`, P11.T8.c–d, and `displaced`, P08.T12) and the stellar rules'
    /// `dwarf_limit`, P11.T4 and `merger_mass`, P11.T4.d ([`StellarRules::every_27_hold`]).
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
        let field = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).field_share;
        for v in [23, 24, 25, 26] {
            assert_eq!(field(v), FieldShareRules::None, "{v}");
        }
        assert_eq!(field(27), FieldShareRules::Phi);
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).field_share,
            FieldShareRules::None
        );
        assert_eq!(
            GalaxyRules::every_27_hold().field_share,
            FieldShareRules::Phi
        );
        let carve = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).carve_out;
        for v in [23, 24, 25, 26] {
            assert_eq!(carve(v), CarveOutRules::Unclaimed, "{v}");
        }
        assert_eq!(carve(27), CarveOutRules::Claimed);
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).carve_out,
            CarveOutRules::Unclaimed
        );
        assert_eq!(
            GalaxyRules::every_26_hold_with_its_retargets().carve_out,
            CarveOutRules::Claimed
        );
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

    /// Each galaxy hold at 27 is held below it and new from it on, in every 27 hold.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one block of assertions a 27 hold; the 27 bump deletes each with its field"
    )]
    fn each_27_hold_turns_on_at_27() {
        let orphans = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).orphan_streams;
        for v in [23, 24, 25, 26] {
            assert_eq!(orphans(v), OrphanStreamsRules::Brainstorm, "{v}");
        }
        assert_eq!(orphans(27), OrphanStreamsRules::Fitted);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).orphan_streams,
            OrphanStreamsRules::Brainstorm
        );
        assert_eq!(
            GalaxyRules::every_27_hold().orphan_streams,
            OrphanStreamsRules::Fitted
        );
        let tables = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).binary_tables;
        for v in [23, 24, 25, 26] {
            assert_eq!(tables(v), BinaryTableRules::Scratch, "{v}");
        }
        assert_eq!(tables(27), BinaryTableRules::Fitted);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).binary_tables,
            BinaryTableRules::Scratch
        );
        assert_eq!(
            GalaxyRules::every_27_hold().binary_tables,
            BinaryTableRules::Fitted
        );
        let source = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).class_source;
        for v in [23, 24, 25, 26] {
            assert_eq!(source(v), ClassSourceRules::Unregistered, "{v}");
        }
        assert_eq!(source(27), ClassSourceRules::Registered);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).class_source,
            ClassSourceRules::Unregistered
        );
        assert_eq!(
            GalaxyRules::every_27_hold().class_source,
            ClassSourceRules::Registered
        );
        let binaries = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).feature_binaries;
        for v in [23, 24, 25, 26] {
            assert_eq!(binaries(v), FeatureBinariesRules::Unlisted, "{v}");
        }
        assert_eq!(binaries(27), FeatureBinariesRules::Listed);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).feature_binaries,
            FeatureBinariesRules::Unlisted
        );
        let runaways = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).runaway_deaths;
        for v in [23, 24, 25, 26] {
            assert_eq!(runaways(v), RunawayDeathsRules::FieldOnly, "{v}");
        }
        assert_eq!(runaways(27), RunawayDeathsRules::Ejected);
        assert_eq!(
            GalaxyRules::every_27_hold().runaway_deaths,
            RunawayDeathsRules::Ejected
        );
        let survivors = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).hypervelocity;
        for v in [23, 24, 25, 26] {
            assert_eq!(survivors(v), HypervelocityRules::Unweighted, "{v}");
        }
        assert_eq!(survivors(27), HypervelocityRules::Weighted);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).hypervelocity,
            HypervelocityRules::Unweighted
        );
        assert_eq!(
            GalaxyRules::every_27_hold().hypervelocity,
            HypervelocityRules::Weighted
        );
        let spray = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).orphan_spray;
        for v in [23, 24, 25, 26] {
            assert_eq!(spray(v), OrphanSprayRules::Exact, "{v}");
        }
        assert_eq!(spray(27), OrphanSprayRules::Coarse);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).orphan_spray,
            OrphanSprayRules::Exact
        );
        let fates = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).placement_fates;
        for v in [23, 24, 25, 26] {
            assert_eq!(fates(v), PlacementFateRules::Exact, "{v}");
        }
        assert_eq!(fates(27), PlacementFateRules::Fitted);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).placement_fates,
            PlacementFateRules::Exact
        );
        let list = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).list_source;
        for v in [23, 24, 25, 26] {
            assert_eq!(list(v), ListSourceRules::Unregistered, "{v}");
        }
        assert_eq!(list(27), ListSourceRules::Registered);
        assert_eq!(
            GalaxyRules::of_version(crate::GENERATOR_VERSION).list_source,
            ListSourceRules::Unregistered
        );
        assert_eq!(
            GalaxyRules::every_27_hold().list_source,
            ListSourceRules::Registered
        );
        let arc = |v| GalaxyRules::of_version(GeneratorVersion::new(v)).kepler_arc;
        for v in [23, 24, 25, 26] {
            assert_eq!(arc(v), KeplerArcRules::Series, "{v}");
        }
        assert_eq!(arc(27), KeplerArcRules::Local);
        assert_eq!(
            GalaxyRules::every_26_hold_with_its_retargets().kepler_arc,
            KeplerArcRules::Series
        );
        assert_eq!(
            GalaxyRules::every_27_hold().kepler_arc,
            KeplerArcRules::Local
        );
        assert_eq!(
            GalaxyRules::every_27_hold(),
            GalaxyRules {
                kepler_arc: KeplerArcRules::Local,
                orphan_streams: OrphanStreamsRules::Fitted,
                merger_channels: MergerChannelRules::WithDynamical,
                binary_tables: BinaryTableRules::Fitted,
                class_source: ClassSourceRules::Registered,
                feature_binaries: FeatureBinariesRules::Listed,
                runaway_deaths: RunawayDeathsRules::Ejected,
                hypervelocity: HypervelocityRules::Weighted,
                orphan_spray: OrphanSprayRules::Coarse,
                placement_fates: PlacementFateRules::Fitted,
                list_source: ListSourceRules::Registered,
                stellar: StellarRules::of_version(GeneratorVersion::new(27)),
                ..GalaxyRules::every_26_hold_with_its_retargets()
            }
        );
        // The holds retargeted from 26 (train 26) are every 27 hold's too.
        assert_eq!(
            GalaxyRules::every_27_hold().stellar.merger_mass,
            StellarRules::every_26_hold_with_its_retargets().merger_mass
        );
        assert_eq!(
            GalaxyRules::every_27_hold().stellar,
            StellarRules::every_27_hold()
        );
    }
}
