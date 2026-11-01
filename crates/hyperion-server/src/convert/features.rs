//! Plan 09's features as the wire carries them (P09.T40): what placed a range row, a member's
//! class, and a catalogue class.
//!
//! The wire's types are plan 09's P09.T39's (`hyperion-protocol`'s `features`). Three of their enums
//! grow, by plan 04's reserved list: [`SystemOriginWire`], [`CatalogueClassWire`] and
//! [`MemberClassWire`], so a variant the sim adds later takes a new string without a bump. This
//! module sends only the variants that exist.

use hyperion_protocol::{
    CatalogueClassWire, CentreTracerWire, ClassKindWire, FeatureIdHex, GenerationWire,
    MemberClassWire, MultiplicityWire, SystemOriginWire,
};
use hyperion_sim::galaxy::catalogue_classes::ClassId;
use hyperion_sim::galaxy::features::FeatureId;
use hyperion_sim::galaxy::features::centre::members::CentreMemberRecord;
use hyperion_sim::galaxy::features::centre::{CentreClass, CentreClassKind, CentreTracer};
use hyperion_sim::galaxy::features::interior::{ClassKind, Generation, MemberClass, Multiplicity};
use hyperion_sim::galaxy::global_list::StreamNumber;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::id::{CentreMemberId, MemberSlot, SystemIdKind};

use super::mass_layer;

/// A feature's ID as the wire carries it: the word its members share with their member bits
/// zeroed ([`FeatureId::object_word`]), as P09.T39 fixed it.
#[must_use]
pub(crate) fn feature_id_hex(feature: FeatureId) -> FeatureIdHex {
    FeatureIdHex::from_u64(feature.object_word())
}

/// What placed `record`'s system, as a range row carries it once P09.T40.a's gate is open.
///
/// A grid system is `grid`, and a catalogue feature's member `feature_member` with its feature. A
/// member of the galactic centre is `centre_member`, and a member of the global list `stream` with
/// its stream's number or `dwarf_core` with its core's, which its ID carries (plan 10, P10.T10.b):
/// rows carry them from generator version 27, whose range query registers the list's source with
/// the centre's (P10.T10.b's registration member and P09.T37.b). A catalogue class's host is
/// `catalogue_system` with its class (P09.T33.a), from 27 too (P09.T37.a).
///
/// # Panics
///
/// For an origin the sim adds later, which [`SystemOrigin`]'s `#[non_exhaustive]` lets it add
/// without this crate knowing: the task that registers its source gives it a wire variant here,
/// since the enum grows. For a global-list member whose ID is neither a stream's nor a dwarf
/// core's, which the sim never makes; and for a catalogue class the registry allocates after plan
/// 09's nine, which has no wire string ([`catalogue_class`]).
#[must_use]
pub(crate) fn system_origin(record: &SystemRecord) -> SystemOriginWire {
    match record.origin() {
        SystemOrigin::Grid(_) => SystemOriginWire::Grid,
        SystemOrigin::FeatureMember { feature, .. } => SystemOriginWire::FeatureMember {
            feature: feature_id_hex(feature.feature()),
        },
        SystemOrigin::CentreMember { .. } => SystemOriginWire::CentreMember,
        SystemOrigin::CatalogueSystem(class) => SystemOriginWire::CatalogueSystem {
            class: catalogue_class(class)
                .expect("every class the sim places has a wire string (its registry's test)"),
        },
        SystemOrigin::GlobalListMember { .. } => match record.id().kind() {
            SystemIdKind::Stream(member) => SystemOriginWire::Stream {
                stream: StreamNumber::from(member).get(),
            },
            SystemIdKind::DwarfCore(member) => SystemOriginWire::DwarfCore {
                core: u16::from(member.number()),
            },
            other @ (SystemIdKind::Grid(_)
            | SystemIdKind::FeatureMember(_)
            | SystemIdKind::Centre(_)
            | SystemIdKind::Pinned(_)
            | SystemIdKind::Catalogue(_)) => {
                unreachable!("a global-list member's ID is a stream's or a core's: {other:?}")
            }
        },
        other => unreachable!(
            "no source the server registers places a system of origin {other:?}: the task that \
             registers one gives it a wire variant (plan 04's reserved list: the origin grows)"
        ),
    }
}

/// A galactic-centre member's class as the wire carries it (plan 09, P09.T37.b): a member of the
/// centre's grid its centre class's ([`centre_class`]); a member of the centre's feature-level list
/// after the black hole the list's class, which for the members the centre's source places, the
/// tidal disruptions' victims (P09.T30.b), is theirs; and the black hole, member zero, none.
///
/// The recently dead and luminous blue variables of P09.T36.b's list follow the victims on it, and
/// the centre's source places none of them yet; the task that has it place them names their
/// classes here.
#[must_use]
pub(crate) fn centre_member_class(
    id: CentreMemberId,
    member: &CentreMemberRecord,
) -> Option<MemberClassWire> {
    match id.slot() {
        MemberSlot::InCell { .. } => member.class().map(centre_class),
        MemberSlot::FeatureLevel { index: 0 } => None,
        MemberSlot::FeatureLevel { .. } => Some(MemberClassWire::Listed {
            class: CatalogueClassWire::TidalDisruptionVictim,
        }),
    }
}

/// One of the centre's classes as the wire carries it: its kind, its band's layer, its episode and
/// the distribution function its orbits are drawn from.
#[must_use]
pub(crate) fn centre_class(class: &CentreClass) -> MemberClassWire {
    MemberClassWire::Centre {
        kind: match class.kind {
            CentreClassKind::Living => class_kind(ClassKind::Living),
            CentreClassKind::WhiteDwarf => class_kind(ClassKind::WhiteDwarf),
            CentreClassKind::NeutronStar => class_kind(ClassKind::NeutronStar),
            CentreClassKind::BlackHole => class_kind(ClassKind::BlackHole),
        },
        band: mass_layer(class.band.layer()),
        episode: class.component,
        tracer: match class.tracer {
            CentreTracer::Stars => CentreTracerWire::Stars,
            CentreTracer::BlackHoles => CentreTracerWire::BlackHoles,
            CentreTracer::YoungDisc => CentreTracerWire::YoungDisc,
            CentreTracer::YoungIsotropic => CentreTracerWire::YoungIsotropic,
        },
    }
}

/// A feature member's class as the wire carries it: its kind, its band's layer, its population and
/// its multiplicity (plan 09's [`MemberClass`]). The initial-mass range and the mean mass travel
/// in a feature's detail (`ClassCountWire`), not on a row.
#[must_use]
pub(crate) fn member_class(class: &MemberClass) -> MemberClassWire {
    MemberClassWire::Cluster {
        kind: class_kind(class.kind),
        band: mass_layer(class.band.layer()),
        generation: generation(class.generation),
        multiplicity: multiplicity(class.multiplicity),
    }
}

/// The wire's class kind.
#[must_use]
pub(crate) fn class_kind(kind: ClassKind) -> ClassKindWire {
    match kind {
        ClassKind::Living => ClassKindWire::Living,
        ClassKind::WhiteDwarf => ClassKindWire::WhiteDwarf,
        ClassKind::NeutronStar => ClassKindWire::NeutronStar,
        ClassKind::BlackHole => ClassKindWire::BlackHole,
        ClassKind::Tail => ClassKindWire::Tail,
    }
}

/// The wire's population of a massive globular.
#[must_use]
fn generation(generation: Generation) -> GenerationWire {
    match generation {
        Generation::First => GenerationWire::First,
        Generation::Second => GenerationWire::Second,
    }
}

/// The wire's multiplicity.
#[must_use]
fn multiplicity(multiplicity: Multiplicity) -> MultiplicityWire {
    match multiplicity {
        Multiplicity::Single => MultiplicityWire::Single,
        Multiplicity::Binary => MultiplicityWire::Binary,
    }
}

/// A catalogue class as the wire carries it, by the registry's constants in value order (plan 09's
/// `ClassId`, P09.T1), or `None` for a class the registry allocates after plan 09's nine.
///
/// [`ClassId`] is a struct, not an enum, so a class registered later does not fail to compile
/// here: it is `None`, and `every_registered_class_has_a_distinct_wire_string` fails until it is
/// given one (the enum grows, by plan 04's reserved list, so that needs no bump).
#[must_use]
pub(crate) fn catalogue_class(class: ClassId) -> Option<CatalogueClassWire> {
    let wire = match class {
        ClassId::CORE_COLLAPSE => CatalogueClassWire::CoreCollapse,
        ClassId::TYPE_IA => CatalogueClassWire::TypeIa,
        ClassId::STELLAR_MERGER => CatalogueClassWire::StellarMerger,
        ClassId::NEUTRON_STAR_MERGER => CatalogueClassWire::NeutronStarMerger,
        ClassId::LUMINOUS_BLUE_VARIABLE => CatalogueClassWire::LuminousBlueVariable,
        ClassId::XRAY_BINARY => CatalogueClassWire::XrayBinary,
        ClassId::ACCRETING_WHITE_DWARF => CatalogueClassWire::AccretingWhiteDwarf,
        ClassId::TIDAL_DISRUPTION_VICTIM => CatalogueClassWire::TidalDisruptionVictim,
        ClassId::ACCRETING_WHITE_DWARF_SLOW => CatalogueClassWire::AccretingWhiteDwarfSlow,
        _ => return None,
    };
    Some(wire)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use hyperion_protocol::MassLayer;
    use hyperion_sim::galaxy::catalogue_classes::ALL_CLASSES;
    use hyperion_sim::galaxy::imf::MassBand;
    use hyperion_sim::id::{
        CentreMemberId, DwarfCoreMemberId, FeatureCell, FeatureMemberId, Layer, MemberSlot,
        StreamMemberId, SystemId,
    };
    use hyperion_sim::units::SolarMasses;

    use super::*;

    /// Every class the registry allocates converts to a wire string of its own, in value order: the
    /// counterpart of the protocol's wire-strings test, so that a class registered without a wire
    /// string fails here rather than reaching the conversion's `None` (P09.T40.a).
    #[test]
    fn every_registered_class_has_a_distinct_wire_string() {
        let strings: Vec<String> = ALL_CLASSES
            .iter()
            .map(|&class| {
                let wire =
                    catalogue_class(class).unwrap_or_else(|| panic!("{class} has no wire string"));
                serde_json::to_value(wire)
                    .expect("a string enum serialises")
                    .as_str()
                    .expect("a string")
                    .to_owned()
            })
            .collect();
        assert_eq!(
            strings,
            [
                "core_collapse",
                "type_ia",
                "stellar_merger",
                "neutron_star_merger",
                "luminous_blue_variable",
                "xray_binary",
                "accreting_white_dwarf",
                "tidal_disruption_victim",
                "accreting_white_dwarf_slow",
            ]
        );
        assert_eq!(
            strings.iter().collect::<BTreeSet<_>>().len(),
            ALL_CLASSES.len(),
            "each class its own string"
        );
        let values: Vec<u8> = ALL_CLASSES.iter().map(|class| class.value()).collect();
        assert!(values.is_sorted(), "in value order: {values:?}");
        // A free value is no class at all.
        assert_eq!(ClassId::from_value(9), None);
    }

    /// A grid system's origin is `grid`, whatever its component; a feature member's names its
    /// feature by its object word; the centre's is `centre_member`; and a global-list member's is
    /// `stream` or `dwarf_core`, with the number its ID carries (P10.T10.b).
    #[test]
    fn each_origin_is_the_wire_s() {
        let grid = crate::convert::origins::a_grid_system();
        assert_eq!(system_origin(&grid), SystemOriginWire::Grid);
        // A record of `origin` with `id`, at the grid system's place and primary.
        let of = |id: SystemId, origin: SystemOrigin| {
            SystemRecord::from_parts(
                id,
                *grid.epoch_position(),
                origin,
                grid.population(),
                grid.primary_initial_mass(),
                grid.age_at_epoch(),
            )
        };
        let feature =
            FeatureId::of(FeatureCell::new([1, -9, 3]).expect("a cell"), 212).expect("an ID");
        let slot = MemberSlot::InCell {
            band: Layer::C,
            level: 2,
            cell: [3, 4, 5],
            index: 6,
        };
        let member = FeatureMemberId::new(feature.feature_ref(), slot).expect("a member ID");
        let wire = system_origin(&of(
            SystemId::from(member),
            SystemOrigin::FeatureMember {
                feature: feature.into(),
                attempt: 3,
            },
        ));
        assert_eq!(
            wire,
            SystemOriginWire::FeatureMember {
                feature: FeatureIdHex::from_u64(feature.object_word()),
            }
        );
        let centre = CentreMemberId::new(slot).expect("a centre member ID");
        assert_eq!(
            system_origin(&of(
                SystemId::from(centre),
                SystemOrigin::CentreMember { attempt: 0 }
            )),
            SystemOriginWire::CentreMember
        );
        let listed = SystemOrigin::GlobalListMember { attempt: 1 };
        let stream = StreamMemberId::new(141, Layer::B, 9, [3, 60], 7).expect("a stream member ID");
        assert_eq!(
            system_origin(&of(SystemId::from(stream), listed)),
            SystemOriginWire::Stream { stream: 141 }
        );
        let core = DwarfCoreMemberId::new(2, slot).expect("a dwarf core member ID");
        assert_eq!(
            system_origin(&of(SystemId::from(core), listed)),
            SystemOriginWire::DwarfCore { core: 2 }
        );
    }

    /// A member class keeps its kind, band, population and multiplicity.
    #[test]
    fn a_member_class_is_the_wire_s() {
        let class = MemberClass {
            kind: ClassKind::WhiteDwarf,
            band: MassBand::C,
            generation: Generation::Second,
            multiplicity: Multiplicity::Binary,
            initial_mass_range: (SolarMasses::new(0.9), SolarMasses::new(1.2)),
            mean_mass: SolarMasses::new(0.6),
        };
        assert_eq!(
            member_class(&class),
            MemberClassWire::Cluster {
                kind: ClassKindWire::WhiteDwarf,
                band: MassLayer::C,
                generation: GenerationWire::Second,
                multiplicity: MultiplicityWire::Binary,
            }
        );
        for (kind, wire) in [
            (ClassKind::Living, ClassKindWire::Living),
            (ClassKind::NeutronStar, ClassKindWire::NeutronStar),
            (ClassKind::BlackHole, ClassKindWire::BlackHole),
            (ClassKind::Tail, ClassKindWire::Tail),
        ] {
            assert_eq!(class_kind(kind), wire);
        }
    }
}
