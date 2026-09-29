//! Resolving a system ID: from 64 bits to the system they name, or to "no such system" (plan 03,
//! P03.T7).
//!
//! The brainstorm's "Identifiers": "A well-formed ID does not always name a system." An ID is a
//! candidate's address — layer, cell and candidate index — and a candidate is only a system if the
//! thinning accepted it, so the checks here can fail for an ID that decodes perfectly: its kind may
//! not be one this generator version places, its index may be at or above its cell's candidate
//! count, and its candidate may have been thinned or claimed by a catalogue class. Every layer is
//! placed since plan 13 added the brown dwarfs and the rogue planets (P13.T3.c).
//!
//! Resolving costs one bound, one Poisson draw and one candidate, whatever the layer and however
//! full the cell: no cell is generated, and nothing is cached, so the answer cannot depend on what
//! has been generated before.

use super::cell::{candidate_count_from_bound, layer_bound};
use super::{
    CandidateOutcome, CellKey, ResolveSystemError, SystemRecord, evaluate_candidate_from_bound,
};
use crate::galaxy::Galaxy;
use crate::galaxy::features::centre::members::resolve_centre_member;
use crate::galaxy::features::members::{NoInteriorCache, resolve_member};
use crate::galaxy::global_list::TubeLookup;
use crate::id::{SystemId, SystemIdKind};

/// The system `id` names in `galaxy`.
///
/// Every ID that arrives from outside the sim — a save, the protocol, a designation a player typed
/// — goes through this function, because nothing outside can promise that an ID names a system.
/// The canonical decode is plan 01's ([`SystemId::from_raw`]); this adds the questions only the
/// galaxy can answer.
///
/// # Errors
///
/// - [`ResolveSystemError::KindNotGenerated`] for an ID under the reserved layer value — a
///   stream, a dwarf core, pinned content or a catalogue system — until plans 09 and 10 generate
///   them. Streams and dwarf cores stay so here for good, since their members need the caller's
///   tube tables: they resolve through [`resolve_with`]. A catalogue feature's member resolves
///   through plan 09's [`resolve_member`] (P09.T21), and the galactic centre's through
///   [`resolve_centre_member`] (P09.T27), which builds the centre's model.
/// - [`ResolveSystemError::NoSuchSystem`] if the index is at or above the cell's
///   [`candidate_count`](super::candidate_count), or its candidate was thinned, or a catalogue
///   class claimed it; for a feature member, if [`resolve_member`] finds none.
///
/// # Examples
///
/// An ID read back from a save, resolved and its record read:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell, resolve};
/// use hyperion_sim::id::{Layer, SystemId};
///
/// let galaxy = Galaxy::new(Seed::new(31));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// // What a save holds: the ID's 64 bits, and nothing else about the system.
/// let saved: u64 = cell[0].id().raw();
///
/// let id = SystemId::from_raw(saved)?;
/// let system = resolve(&galaxy, id)?;
/// assert_eq!(system.id(), id);
/// assert_eq!(system.layer(), Layer::C);
/// assert!((0.75..=2.5).contains(&system.primary_initial_mass().value()));
/// // The record is the same one generating the whole cell produces.
/// assert_eq!(system, cell[0]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn resolve(galaxy: &Galaxy, id: SystemId) -> Result<SystemRecord, ResolveSystemError> {
    match id.kind() {
        // A grid ID's cell fields span the root cube, so there is nothing more to check about the
        // cell; every layer, the substellar two included, is placed.
        SystemIdKind::Grid(grid) => resolve_candidate(galaxy, CellKey::of(id)?, grid.index()),
        // A catalogue feature's member reruns its one candidate (plan 09, P09.T21).
        SystemIdKind::FeatureMember(member) => {
            resolve_member(galaxy, &NoInteriorCache, member).map(|m| *m.record())
        }
        // The galactic centre's members and its black hole (plan 09, P09.T27).
        SystemIdKind::Centre(member) => resolve_centre_member(galaxy, member).map(|m| *m.record()),
        SystemIdKind::Stream(_)
        | SystemIdKind::DwarfCore(_)
        | SystemIdKind::Pinned(_)
        | SystemIdKind::Catalogue(_) => Err(ResolveSystemError::KindNotGenerated),
    }
}

/// The system `id` names in `galaxy`, reading streams and dwarf cores through the caller's
/// cache of the global list and its tube tables (plan 10, P10.T1).
///
/// [`resolve`] takes no context, and a stream's member cannot be resolved without its tube table,
/// nor a dwarf core's without the list's specification of the core, so this is the resolver for a
/// caller that holds the list. Every kind but those two is forwarded to [`resolve`] and answers
/// exactly as it does.
///
/// # Errors
///
/// As [`resolve`]. A stream's or a dwarf core's member answers
/// [`ResolveSystemError::KindNotGenerated`] until plan 10 places them (P10.T8.b and P10.T7.b).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::{GlobalList, StreamNumber, TubeLookup, TubeTable};
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell, resolve, resolve_with};
/// use hyperion_sim::id::Layer;
///
/// struct NoTubes(GlobalList);
///
/// impl TubeLookup for NoTubes {
///     fn global_list(&self) -> &GlobalList {
///         &self.0
///     }
///
///     fn tube(&self, _: StreamNumber) -> Arc<TubeTable> {
///         unreachable!("a grid ID reads no tube")
///     }
/// }
///
/// let galaxy = Galaxy::new(Seed::new(31));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let id = cell[0].id();
/// let lookup = NoTubes(GlobalList::default());
/// assert_eq!(resolve_with(&galaxy, id, &lookup), resolve(&galaxy, id));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn resolve_with(
    galaxy: &Galaxy,
    id: SystemId,
    _lookup: &dyn TubeLookup,
) -> Result<SystemRecord, ResolveSystemError> {
    match id.kind() {
        SystemIdKind::Stream(_) | SystemIdKind::DwarfCore(_) => {
            Err(ResolveSystemError::KindNotGenerated)
        }
        SystemIdKind::Grid(_)
        | SystemIdKind::FeatureMember(_)
        | SystemIdKind::Centre(_)
        | SystemIdKind::Pinned(_)
        | SystemIdKind::Catalogue(_) => resolve(galaxy, id),
    }
}

/// The record of candidate `index` of `key`, if the cell has that candidate and the thinning kept
/// it.
///
/// The cell's bound is evaluated once and handed to both the candidate count and the thinning,
/// which is what makes this bit for bit what [`generate_cell`](super::generate_cell) would have
/// produced for the same candidate.
fn resolve_candidate(
    galaxy: &Galaxy,
    key: CellKey,
    index: u32,
) -> Result<SystemRecord, ResolveSystemError> {
    let bound = layer_bound(galaxy, key);
    if index >= candidate_count_from_bound(galaxy.seed(), key, bound) {
        return Err(ResolveSystemError::NoSuchSystem);
    }
    match evaluate_candidate_from_bound(galaxy, key, index, bound) {
        CandidateOutcome::Accepted(record) => Ok(record),
        CandidateOutcome::Thinned | CandidateOutcome::ClaimedByCatalogue => {
            Err(ResolveSystemError::NoSuchSystem)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{
        STELLAR_LAYERS, candidate_count, evaluate_candidate, generate_cell,
    };
    use std::sync::Arc;

    use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureMarks};
    use crate::galaxy::features::members::FeatureInterior;
    use crate::galaxy::global_list::{GlobalList, StreamNumber, TubeTable};
    use crate::galaxy::imf::MassBand;
    use crate::id::{
        CentreMemberId, DwarfCoreMemberId, FeatureCell, Layer, MemberSlot, StreamMemberId,
    };
    use crate::rng::Seed;

    /// The seed of the galaxy these tests resolve IDs in.
    const SEED: u64 = 0x0300_4e50_0000_0000;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn sunlike() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap()
    }

    /// 1,000 cells spread over the five layers around the Sun-like point.
    fn cells() -> Vec<CellKey> {
        let mut keys = Vec::new();
        for spec in STELLAR_LAYERS {
            let first = CellKey::containing(spec.layer(), &sunlike()).unwrap();
            let [x, y, z] = first.gen_cell().to_array();
            for i in 0..200_i32 {
                let (dx, dy, dz) = (i % 6, i / 6 % 6, i / 36 % 6);
                keys.push(CellKey::new(spec.layer(), [x + dx, y + dy, z - dz]).unwrap());
            }
        }
        keys
    }

    #[test]
    fn resolve_returns_the_record_generating_the_cell_produces() {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        let mut systems = 0;
        for key in cells() {
            generate_cell(&galaxy, key, &mut cell);
            for record in &cell {
                assert_eq!(resolve(&galaxy, record.id()), Ok(*record));
                systems += 1;
            }
        }
        assert!(systems > 1_000, "only {systems} systems over 1,000 cells");
    }

    #[test]
    fn resolve_does_not_depend_on_whether_the_cell_was_generated_before() {
        let (galaxy, untouched) = (galaxy(), galaxy());
        let key = CellKey::containing(Layer::C, &sunlike()).unwrap();
        let mut cell = Vec::new();
        generate_cell(&galaxy, key, &mut cell);
        let id = cell[0].id();
        // Cold, on a galaxy nothing has been generated from, then again after its cell.
        let cold = resolve(&untouched, id);
        assert_eq!(cold, Ok(cell[0]));
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [5, 812, 0]).unwrap(),
            &mut cell,
        );
        assert_eq!(resolve(&galaxy, id), cold);
    }

    #[test]
    fn resolve_refuses_an_index_the_cell_has_no_candidate_for() {
        let galaxy = galaxy();
        for spec in STELLAR_LAYERS {
            let key = CellKey::containing(spec.layer(), &sunlike()).unwrap();
            let count = candidate_count(&galaxy, key);
            assert!(count < key.index_capacity());
            let id = key.candidate_id(count).unwrap();
            assert_eq!(
                resolve(&galaxy, id),
                Err(ResolveSystemError::NoSuchSystem),
                "layer {} at its candidate count of {count}",
                spec.layer().letter()
            );
            // `u32::MAX` masked to the layer's index width: the widest index an ID can carry.
            let widest = key.index_capacity() - 1;
            assert_eq!(
                resolve(&galaxy, key.candidate_id(widest).unwrap()),
                Err(ResolveSystemError::NoSuchSystem),
                "layer {} at index {widest}",
                spec.layer().letter()
            );
        }
    }

    #[test]
    fn resolve_refuses_a_thinned_candidate() {
        let galaxy = galaxy();
        // A layer-E cell at the Sun-like point thins about one candidate in fifteen, some 1.2 a
        // cell, so a single cell has none about three times in ten: walk along x until a dozen
        // have been seen, which a sound fixture reaches within a few tens of cells.
        let first = CellKey::containing(Layer::E, &sunlike()).unwrap();
        let [x, y, z] = first.gen_cell().to_array();
        let mut thinned = 0;
        for step in 0..64 {
            let key = CellKey::new(Layer::E, [x + step, y, z]).unwrap();
            for index in 0..candidate_count(&galaxy, key) {
                if evaluate_candidate(&galaxy, key, index) == CandidateOutcome::Thinned {
                    let id = key.candidate_id(index).unwrap();
                    assert_eq!(resolve(&galaxy, id), Err(ResolveSystemError::NoSuchSystem));
                    thinned += 1;
                }
            }
            if thinned >= 12 {
                return;
            }
        }
        panic!("only {thinned} thinned candidates in 64 layer-E cells at the Sun-like point");
    }

    /// Since plan 13 (P13.T3.c) every layer resolves: a substellar ID names an object or no such
    /// system, never an ungenerated layer. The reserved kinds are still refused.
    #[test]
    fn resolve_places_every_layer_and_refuses_the_kinds_this_version_does_not_place() {
        let galaxy = galaxy();
        for (layer, y) in [(Layer::BrownDwarf, 1_625), (Layer::RoguePlanet, 6_500)] {
            let key = CellKey::new(layer, [0, y, 0]).unwrap();
            for index in 0..=candidate_count(&galaxy, key) {
                let id = key.candidate_id(index).unwrap();
                match resolve(&galaxy, id) {
                    Ok(record) => assert_eq!(record.layer(), layer),
                    Err(e) => assert_eq!(e, ResolveSystemError::NoSuchSystem),
                }
            }
        }
        // The galactic centre's black hole resolves since P09.T27.
        let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
        assert_eq!(black_hole.raw(), 0xf000_0007_0000_0000);
        let record = resolve(&galaxy, black_hole).unwrap();
        assert_eq!(record.id(), black_hole);
        assert_eq!(record.layer(), Layer::E);
    }

    /// A lookup whose tube tables must not be read: no ID resolved so far needs one.
    struct NoTubes(GlobalList);

    impl TubeLookup for NoTubes {
        fn global_list(&self) -> &GlobalList {
            &self.0
        }

        fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
            panic!("stream {} was read before P10.T8.b", stream.get())
        }
    }

    /// Plan 10, P10.T1: a stream's and a dwarf core's members are not resolved by either
    /// resolver yet, and `resolve` never will resolve them.
    #[test]
    fn stream_and_dwarf_core_members_are_not_generated_yet() {
        let galaxy = galaxy();
        let lookup = NoTubes(GlobalList::default());
        let stream = SystemId::from(StreamMemberId::new(3, Layer::D, 900, [31, 32], 0).unwrap());
        let core = SystemId::from(
            DwarfCoreMemberId::new(
                1,
                MemberSlot::InCell {
                    band: Layer::C,
                    level: 0,
                    cell: [7, 8, 9],
                    index: 2,
                },
            )
            .unwrap(),
        );
        let listed = SystemId::from(
            DwarfCoreMemberId::new(0, MemberSlot::FeatureLevel { index: 0 }).unwrap(),
        );
        for id in [stream, core, listed] {
            assert_eq!(
                resolve(&galaxy, id),
                Err(ResolveSystemError::KindNotGenerated)
            );
            assert_eq!(
                resolve_with(&galaxy, id, &lookup),
                Err(ResolveSystemError::KindNotGenerated)
            );
        }
    }

    /// Plan 10, P10.T1: every kind but a stream's and a dwarf core's is forwarded, so
    /// `resolve_with` answers as `resolve` does on grid IDs (systems, empty indices and thinned
    /// candidates) and on the centre's black hole. Feature and centre members are the slow test
    /// below.
    #[test]
    fn resolve_with_agrees_with_resolve_on_grid_and_centre_ids() {
        let lookup = NoTubes(GlobalList::default());
        let galaxy = galaxy();
        let mut cell = Vec::new();
        let mut grid = Vec::new();
        for key in cells().into_iter().step_by(40) {
            generate_cell(&galaxy, key, &mut cell);
            grid.extend(cell.iter().map(SystemRecord::id));
            let count = candidate_count(&galaxy, key);
            grid.extend((0..count + 2).map(|i| key.candidate_id(i).unwrap()));
        }
        assert!(grid.len() > 100, "{} grid IDs", grid.len());
        // An in-cell centre member builds the centre's model, so it is the slow test's.
        let centre = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
        for id in grid.into_iter().chain([centre]) {
            assert_eq!(
                resolve_with(&galaxy, id, &lookup),
                resolve(&galaxy, id),
                "{id:?}"
            );
        }
    }

    /// Plan 10, P10.T1: `resolve_with` answers as `resolve` does on a feature's members, accepted
    /// and past the cell's count, and on a member of the centre. Slow: feature members need the
    /// kinematic tables, and each resolve rebuilds its feature's interior or the centre's model,
    /// seconds apiece.
    #[test]
    #[ignore = "slow: builds the kinematic tables and resolves feature members, seconds each"]
    fn resolve_with_agrees_with_resolve_on_feature_and_centre_members() {
        let lookup = NoTubes(GlobalList::default());
        let galaxy = galaxy().with_full_potential();
        let interior = FeatureCatalogue::cell(&galaxy, FeatureCell::new([0, 6, 0]).unwrap())
            .features()
            .iter()
            .filter(|f| matches!(f.marks(), FeatureMarks::OpenCluster(_)))
            .find_map(|f| FeatureInterior::of(&galaxy, f))
            .expect("the fixture has an open cluster there");
        let mut out = Vec::new();
        let (band, nested) = MassBand::ALL
            .iter()
            .flat_map(|&b| interior.grid().owned_cells().map(move |c| (b, c)))
            .find(|&(b, c)| {
                interior.members_in_cell(&galaxy, b, c, &mut out);
                out.len() >= 2
            })
            .expect("some cell holds two members");
        let count = interior.candidate_count(&galaxy, band, nested);
        let mut ids: Vec<SystemId> = out.iter().take(2).map(|(m, _)| m.record().id()).collect();
        ids.push(SystemId::from(interior.member_id(band, nested, count)));
        // A centre member in a cell, which builds the centre's model (P09.T27).
        ids.push(SystemId::from(
            CentreMemberId::new(MemberSlot::InCell {
                band: Layer::A,
                level: 9,
                cell: [2, 17, 18],
                index: 12,
            })
            .unwrap(),
        ));
        for id in ids {
            let direct = resolve(&galaxy, id);
            assert_eq!(resolve_with(&galaxy, id, &lookup), direct, "{id:?}");
        }
        assert!(
            resolve(
                &galaxy,
                SystemId::from(interior.member_id(band, nested, count))
            )
            .is_err()
        );
    }
}
