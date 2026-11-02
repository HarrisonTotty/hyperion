//! A grid system's placement on the wire (plan 08, P08.T15.a): how its layer's grid placed it
//! beside its birth population, from the sim's placement class.
//!
//! A row carries it only from a galaxy that holds plan 08's displaced classes, every galaxy from
//! generator version 27 (P08.T12's hold, `DISPLACED_VERSION`). One that holds none places every
//! system in the field, which its rows leave unsaid, so that below 27 every range answer keeps the
//! bytes the server's goldens pin, as P09.T40.a's gate keeps the rows' origins (lean, 2026-11-01).

use hyperion_protocol::PlacementKind;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::displaced::{DisplacedKind, PlacementClass};
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};

/// How `record`'s layer's grid placed it, for a grid system's row of a galaxy that holds plan 08's
/// displaced classes; `None` for a feature's or the centre's member and a catalogue system, which
/// no population's grid placed, and for every row of a galaxy that holds no displaced class.
#[must_use]
pub(super) fn grid_placement(galaxy: &Galaxy, record: &SystemRecord) -> Option<PlacementKind> {
    // Every origin but the grid's, those the sim adds later among them (`SystemOrigin` is
    // `#[non_exhaustive]`), is a feature's, the centre's or a list's: none is placed by a population.
    let placed = matches!(record.origin(), SystemOrigin::Grid(_));
    (placed && galaxy.fields().displaced().is_some())
        .then(|| wire_placement(record.placement_class()))
}

/// The wire's placement: the class without its index or its kick's bin.
#[must_use]
fn wire_placement(class: PlacementClass) -> PlacementKind {
    match class {
        PlacementClass::Alive => PlacementKind::Field,
        PlacementClass::Retained { .. } => PlacementKind::Retained,
        PlacementClass::Displaced { kind, .. } => match kind {
            DisplacedKind::Remnant => PlacementKind::DisplacedRemnant,
            DisplacedKind::Runaway => PlacementKind::Runaway,
            DisplacedKind::Walkaway => PlacementKind::Walkaway,
            DisplacedKind::HypervelocitySurvivor => PlacementKind::HypervelocitySurvivor,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    use hyperion_protocol::{MassLayer, QueryModeDto, SystemsInRange, SystemsInRangeRequest};
    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::displaced::{DisplacedClassId, SpeedBin};
    use hyperion_sim::galaxy::params::GalaxyParams;
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::range_query;
    use hyperion_sim::{GeneratorVersion, Seed};

    use super::*;
    use crate::convert::origins::{a_catalogue_system, a_feature_member, a_grid_system};
    use crate::convert::{RangeRequest, RowOrigins, systems_in_range};

    /// The Milky Way fixture under version 27's rules, every hold at 27 in force, so that it holds
    /// plan 08's displaced classes.
    fn milky_way_27() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params_at_version(
                Seed::new(0x4d2),
                GalaxyParams::milky_way_like(),
                GeneratorVersion::new(27),
            )
            .expect("the Milky Way fixture builds")
        })
    }

    /// The fixture as the server builds it today, at the generator version in force.
    fn milky_way() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x4d2), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture builds")
        })
    }

    /// A request for layer E alone, and nothing lighter, within `radius_ly` of the light-year cell
    /// `cell_ly`, at the epoch.
    fn layer_e_within(cell_ly: [i32; 3], radius_ly: f64) -> SystemsInRangeRequest {
        SystemsInRangeRequest {
            universe: hyperion_protocol::UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            centre: hyperion_protocol::GalacticPosition {
                cell_ly,
                offset_m: [0.0; 3],
            },
            radius_ly,
            time: hyperion_protocol::UniverseTime {
                seconds: 0,
                nanos: 0,
            },
            min_layer: MassLayer::E,
            limit: 20_000,
            include_stellar: false,
            mode: QueryModeDto::Now,
        }
    }

    /// `request`'s answer from `galaxy` as the server sends it, beside the sim's hits, in order.
    fn served(
        galaxy: &Galaxy,
        request: &SystemsInRangeRequest,
    ) -> (SystemsInRange, Vec<PlacementClass>) {
        let query = RangeRequest::try_from(request)
            .expect("a valid request")
            .into_query();
        let result = range_query(galaxy, &mut NoCache::new(), &[], &query);
        let classes = result
            .systems()
            .iter()
            .map(|hit| hit.record().placement_class())
            .collect();
        let answer = systems_in_range(
            galaxy,
            request.clone(),
            &query,
            &result,
            None,
            None,
            RowOrigins::Unsent,
        );
        (answer, classes)
    }

    #[test]
    fn each_placement_class_has_its_wire_kind() {
        let class = DisplacedClassId::new(3);
        let displaced = |kind| PlacementClass::Displaced { class, kind };
        let speed = SpeedBin::new(2).expect("a speed bin");
        for (placement, wire) in [
            (PlacementClass::Alive, PlacementKind::Field),
            (PlacementClass::Retained { speed }, PlacementKind::Retained),
            (
                displaced(DisplacedKind::Remnant),
                PlacementKind::DisplacedRemnant,
            ),
            (displaced(DisplacedKind::Runaway), PlacementKind::Runaway),
            (displaced(DisplacedKind::Walkaway), PlacementKind::Walkaway),
            (
                displaced(DisplacedKind::HypervelocitySurvivor),
                PlacementKind::HypervelocitySurvivor,
            ),
        ] {
            assert_eq!(wire_placement(placement), wire, "{placement:?}");
        }
    }

    #[test]
    fn in_a_galaxy_with_the_classes_only_a_grid_system_carries_a_placement() {
        // The rule reads the galaxy's classes and the record's origin alone, so the server tests'
        // records of each origin stand for those of a galaxy at 27.
        let galaxy = milky_way_27();
        assert_eq!(
            grid_placement(galaxy, &a_grid_system()),
            Some(PlacementKind::Field)
        );
        assert_eq!(grid_placement(galaxy, &a_feature_member()), None);
        assert_eq!(grid_placement(galaxy, &a_catalogue_system()), None);
    }

    /// Each row's placement, checked against the sim's class of the hit it was built from.
    fn placements(answer: &SystemsInRange, classes: &[PlacementClass]) -> Vec<PlacementKind> {
        assert_eq!(answer.systems.len(), classes.len(), "one row per hit");
        answer
            .systems
            .iter()
            .zip(classes)
            .map(|(row, &class)| {
                assert_eq!(
                    row.placement,
                    Some(wire_placement(class)),
                    "{}",
                    row.designation
                );
                row.placement.expect("a grid row carries its placement")
            })
            .collect()
    }

    /// How many rows of each placement.
    fn counted(placements: &[PlacementKind]) -> BTreeMap<PlacementKind, usize> {
        let mut counts = BTreeMap::new();
        for &placement in placements {
            *counts.entry(placement).or_default() += 1;
        }
        counts
    }

    #[test]
    fn below_27_every_grid_system_is_in_the_field_and_no_row_says_so() {
        // The 27 bump, which installs the classes in every galaxy, retires this test.
        assert!(
            milky_way().fields().displaced().is_none(),
            "the version in force holds plan 08's displaced classes"
        );
        // Layer E within 300 ly of the Sun-like point, and every layer within 20 ly of it.
        for request in [
            layer_e_within([0, 26_000, 0], 300.0),
            SystemsInRangeRequest {
                min_layer: MassLayer::A,
                ..layer_e_within([0, 26_000, 0], 20.0)
            },
        ] {
            let (answer, classes) = served(milky_way(), &request);
            assert!(!answer.systems.is_empty(), "{:?}", request.min_layer);
            assert_eq!(answer.systems.len(), classes.len(), "one row per hit");
            for (row, &class) in answer.systems.iter().zip(&classes) {
                // As the sim says: a galaxy that holds no displaced class places every system in
                // the field, none retained.
                assert_eq!(class, PlacementClass::Alive, "{}", row.designation);
                // And the row is plan 04's, as the server's goldens pin it.
                assert_eq!(row.placement, None, "{}", row.designation);
            }
        }
    }

    #[test]
    fn under_every_27_hold_a_layer_e_row_of_each_kind_is_served_with_its_placement() {
        // Layer E within 250 ly of a point in the plane at 16,000 ly, where the young disc's
        // living massive stars are dense enough for some of each kind: 4,049 rows at this seed,
        // four in the field, five runaways and five walkaways among them (2026-11-01).
        let (answer, classes) = served(milky_way_27(), &layer_e_within([0, 16_000, 0], 250.0));
        let counts = counted(&placements(&answer, &classes));
        for kind in [
            PlacementKind::Field,
            PlacementKind::Retained,
            PlacementKind::DisplacedRemnant,
            PlacementKind::Runaway,
            PlacementKind::Walkaway,
        ] {
            assert!(
                counts.contains_key(&kind),
                "no {kind:?} row among {counts:?}"
            );
        }
        // The survivors have no weight until P09.T34.b.
        assert!(
            !counts.contains_key(&PlacementKind::HypervelocitySurvivor),
            "{counts:?}"
        );
    }
}
