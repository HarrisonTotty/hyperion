//! Placing a cluster's members in its nested grid, band by band (plan 09, P09.T21; brainstorm,
//! "Dense features: clusters and the galactic centre").
//!
//! A feature with members ([`FeatureInterior`]) carries its [`ClusterModel`], its
//! [`MemberClassTable`] and a [`NestedGrid`] of 16 cells and eight levels whose width the table
//! chooses ([`MemberClassTable::grid_width`]): the finest that reaches the tidal radius, doubled
//! while no cell and band on the axes would expect more than 2,000 candidates, as far as the tails
//! run. The tails are then cut at the grid's reach. Every owned cell is placed by the same thinning
//! as the field, once per mass band:
//!
//! - **Count.** The bound is [`MemberClassTable::cell_bound`] over the cell, the profiled classes
//!   at its corner nearest the centre and the tail over its bounding sphere, and the candidates
//!   are a Poisson draw of mean bound × volume on `member.cell`, keyed by `ObjectKey::cell(word)`
//!   with the word the member ID of the cell's candidate 0 in the band (Design note 23). In the
//!   eight cells with a corner at the centre of a core-collapsed cluster the candidates are
//!   proposed radially under the cusp's envelope instead ([`CellProposal`]), since the nearest
//!   corner's bound there is the cusp's softened peak. The count is clamped at the 13-bit index's
//!   8,192 with a debug assertion, as plan 03 clamps a grid cell's.
//! - **Candidate.** Candidate `i`'s streams are keyed by its [`FeatureMemberId`]: three uniforms on
//!   `member.position` put it in its cell ([`CellProposal::place`]), one mark on `member.accept`
//!   picks its class under the bound there or rejects it ([`MemberClassTable::pick`]), and an
//!   accepted candidate is drawn by [`draw_member`].
//!
//! [`resolve_member`] reruns one candidate: "no such system" for an index at or past its cell's
//! count, a rejected candidate, a feature that does not resolve or has no members, or a band the
//! table has no classes in.

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::ResolveSystemError;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{FeatureMemberId, FeatureRef, MemberSlot, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::units::consts::METRES_PER_LIGHT_YEAR;

use super::super::catalogue::{FeatureCatalogue, FeatureRecord, bulk_velocity};
use super::super::cluster::ClusterModel;
use super::super::interior::{CellProposal, LocalCell, MemberClassTable};
use super::super::nested::{NestedCell, NestedGrid};
use super::level_list::FeatureLevelList;
use super::source::FeatureInteriorCache;
use super::{MemberRecord, draw_member};

/// A feature's interior: what placing its members needs, built once per feature and kept by the
/// caller (P09.T21).
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureInterior {
    feature: FeatureRecord,
    model: ClusterModel,
    table: MemberClassTable,
    grid: NestedGrid,
    bulk: GalacticVelocity,
}

impl FeatureInterior {
    /// The interior of `feature`, or `None` for a feature with no members at the epoch (a cloud,
    /// an association, an unbound embedded region or a dissolved cluster;
    /// [`ClusterModel::from_record`]) or in a galaxy built without its kinematic tables
    /// ([`Galaxy::with_full_potential`]).
    ///
    /// Members are defined only in a galaxy with its kinematic tables: the bulk velocity
    /// ([`bulk_velocity`]) sets the tails' axis and so which candidates are accepted, and a zero
    /// stand-in would give the same ID another member. The tails run along it from the tidal
    /// radius to the feature's [`reach`](FeatureRecord::reach), cut at the grid's.
    #[must_use]
    pub fn of(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Self> {
        let bulk = bulk_velocity(galaxy, feature)?;
        let model = ClusterModel::from_record(galaxy, feature)?;
        let table =
            MemberClassTable::new(galaxy, &model, feature.reach(), bulk.metres_per_second());
        let grid = table.grid();
        let table = table.with_tails_to(grid.reach());
        Some(Self {
            feature: *feature,
            model,
            table,
            grid,
            bulk,
        })
    }

    /// The feature.
    #[must_use]
    pub const fn feature(&self) -> &FeatureRecord {
        &self.feature
    }

    /// Its cluster model.
    #[must_use]
    pub const fn model(&self) -> &ClusterModel {
        &self.model
    }

    /// Its class table.
    #[must_use]
    pub const fn table(&self) -> &MemberClassTable {
        &self.table
    }

    /// Its nested grid.
    #[must_use]
    pub const fn grid(&self) -> &NestedGrid {
        &self.grid
    }

    /// Its bulk velocity, galactic.
    #[must_use]
    pub const fn bulk_velocity(&self) -> GalacticVelocity {
        self.bulk
    }

    /// The feature-level list of its members (P09.T22).
    #[must_use]
    pub fn feature_level_list(&self, galaxy: &Galaxy) -> FeatureLevelList {
        FeatureLevelList::of(galaxy, &self.feature)
    }

    /// The feature's reference, for member IDs.
    #[must_use]
    fn feature_ref(&self) -> FeatureRef {
        self.feature.id().feature_ref()
    }

    /// The ID of candidate `index` of `band` in `cell`.
    ///
    /// # Panics
    ///
    /// If `cell` is not an owned cell of a catalogue feature's grid or `index` is 8,192 or more,
    /// which [`NestedGrid`]'s cells and the clamp rule out.
    #[must_use]
    pub fn member_id(&self, band: MassBand, cell: NestedCell, index: u16) -> FeatureMemberId {
        FeatureMemberId::new(
            self.feature_ref(),
            MemberSlot::InCell {
                band: band.layer(),
                level: cell.level(),
                cell: cell.cell(),
                index,
            },
        )
        .expect("an owned cell of a 16-cell, 8-level grid and an index below 8,192 make an ID")
    }

    /// How `band`'s candidates are proposed in `cell`, and the cell's box.
    #[must_use]
    fn proposal(&self, band: MassBand, cell: NestedCell) -> (CellProposal, LocalCell) {
        let local = self.grid.local_cell(cell);
        (self.table.proposal(band, &local), local)
    }

    /// The expected candidates of `band` in `cell`: its bound times its volume, or its cusp
    /// envelope's integral, what the census sums (P09.T23).
    #[must_use]
    pub fn expected_candidates(&self, band: MassBand, cell: NestedCell) -> f64 {
        let (proposal, local) = self.proposal(band, cell);
        proposal.expected(&local)
    }

    /// The candidate count of `band` in `cell`: a Poisson draw on the cell's stream, clamped at
    /// [`MemberSlot::INDEX_LIMIT`] (module documentation).
    ///
    /// # Panics
    ///
    /// In debug builds, if the draw exceeds the index.
    #[must_use]
    pub fn candidate_count(&self, galaxy: &Galaxy, band: MassBand, cell: NestedCell) -> u16 {
        let mean = self.expected_candidates(band, cell);
        count_from_mean(galaxy, self.member_id(band, cell, 0), mean)
    }

    /// Candidate `index` of `band` in `cell`, if the thinning keeps it, with its local position
    /// (ly from the feature's centre); `None` if it is rejected or its draw exhausts its attempts.
    /// The index is not checked against the count.
    #[must_use]
    pub fn candidate(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        index: u16,
    ) -> Option<(MemberRecord, PointLy)> {
        let (proposal, local) = self.proposal(band, cell);
        self.candidate_under(galaxy, band, cell, index, &proposal, &local)
    }

    /// [`candidate`](Self::candidate) with the cell's bound and box given.
    fn candidate_under(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        index: u16,
        proposal: &CellProposal,
        local: &LocalCell,
    ) -> Option<(MemberRecord, PointLy)> {
        let id = self.member_id(band, cell, index);
        let key = ObjectKey::from(SystemId::from(id));
        let mut position = Stream::open(galaxy.seed(), tags::MEMBER_POSITION, key);
        let u = [0; 3].map(|_| position.uniform());
        let (p, bound) = proposal.place(local, u)?;
        let mark = Stream::open(galaxy.seed(), tags::MEMBER_ACCEPT, key).mark();
        let class = self.table.pick(band, &p, mark, bound)?;
        let at = self.galactic(&p);
        draw_member(
            galaxy,
            &self.feature,
            &self.model,
            &class,
            &at,
            id,
            self.bulk,
        )
        .map(|member| (member, p))
    }

    /// The galactic position of the local point `p` of a member.
    ///
    /// # Panics
    ///
    /// If `p` is so far out that the position leaves the addressable range, which no member's
    /// can: members lie within the feature's reach, at most 4,096 ly from a centre in the root
    /// cube.
    #[must_use]
    pub(crate) fn galactic(&self, p: &PointLy) -> GalacticPosition {
        let metres = [p.x, p.y, p.z].map(|c| c * METRES_PER_LIGHT_YEAR);
        self.feature
            .position()
            .translated(GalacticDisplacement::new(metres))
            .expect("a member within 4,096 ly of a feature in the root cube is addressable")
    }

    /// Every accepted member of `band` in `cell`, in index order, with its local position, into
    /// `out` (cleared first).
    pub fn members_in_cell(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        out: &mut Vec<(MemberRecord, PointLy)>,
    ) {
        out.clear();
        let (proposal, local) = self.proposal(band, cell);
        let n = count_from_mean(
            galaxy,
            self.member_id(band, cell, 0),
            proposal.expected(&local),
        );
        out.extend(
            (0..n).filter_map(|i| self.candidate_under(galaxy, band, cell, i, &proposal, &local)),
        );
    }
}

/// The count of a cell whose candidate 0 is `first` and whose mean is `mean`.
#[must_use]
fn count_from_mean(galaxy: &Galaxy, first: FeatureMemberId, mean: f64) -> u16 {
    let word = SystemId::from(first).raw();
    let drawn = Stream::open(galaxy.seed(), tags::MEMBER_CELL, ObjectKey::cell(word)).poisson(mean);
    let limit = MemberSlot::INDEX_LIMIT;
    debug_assert!(
        drawn <= u64::from(limit),
        "a nested cell of {first:?} drew {drawn} candidates from a mean of {mean}, past the index's \
         {limit}"
    );
    u16::try_from(drawn.min(u64::from(limit))).expect("a count clamped at 8,192 fits in u16")
}

/// The member `id` names in `galaxy`, or [`ResolveSystemError::NoSuchSystem`] (module
/// documentation), its feature's interior from `interiors` (ruling 139.6: a caller resolving
/// members of one feature keeps it). A feature-level member resolves through its list (P09.T22).
///
/// # Errors
///
/// [`ResolveSystemError::NoSuchSystem`] if the feature does not resolve or has no members (in a
/// galaxy without its kinematic tables no feature has), the index is at or past its cell's count,
/// or the candidate was rejected.
pub fn resolve_member(
    galaxy: &Galaxy,
    interiors: &dyn FeatureInteriorCache,
    id: FeatureMemberId,
) -> Result<MemberRecord, ResolveSystemError> {
    let feature = FeatureCatalogue::resolve(galaxy, id.feature().into())
        .ok_or(ResolveSystemError::NoSuchSystem)?;
    match id.slot() {
        MemberSlot::FeatureLevel { index } => FeatureLevelList::of(galaxy, &feature)
            .member(index)
            .ok_or(ResolveSystemError::NoSuchSystem),
        MemberSlot::InCell {
            band,
            level,
            cell,
            index,
        } => {
            let band = MassBand::of_layer(band);
            if !band.is_stellar() {
                return Err(ResolveSystemError::NoSuchSystem);
            }
            let interior = interiors
                .interior(galaxy, &feature)
                .ok_or(ResolveSystemError::NoSuchSystem)?;
            let cell = interior
                .grid
                .cell(level, cell)
                .ok_or(ResolveSystemError::NoSuchSystem)?;
            let (proposal, local) = interior.proposal(band, cell);
            let mean = proposal.expected(&local);
            if index >= count_from_mean(galaxy, interior.member_id(band, cell, 0), mean) {
                return Err(ResolveSystemError::NoSuchSystem);
            }
            interior
                .candidate_under(galaxy, band, cell, index, &proposal, &local)
                .map(|(member, _)| member)
                .ok_or(ResolveSystemError::NoSuchSystem)
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use std::sync::OnceLock;

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::FeatureMarks;
    use crate::galaxy::features::members::NoInteriorCache;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::resolve;
    use crate::id::{FeatureCell, SystemIdKind};
    use crate::math;
    use crate::rng::Seed;

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x0921_0000), GalaxyParams::milky_way_like())
                .unwrap()
                .with_full_potential()
        })
    }

    /// An old open cluster of a few thousand solar masses at birth, shared by the tests: an
    /// interior costs seconds, nearly all of it the cluster model's retention quadrature.
    fn open_cluster() -> &'static FeatureInterior {
        static OPEN: OnceLock<FeatureInterior> = OnceLock::new();
        OPEN.get_or_init(|| {
            let galaxy = galaxy();
            [[0, 6, 0], [1, 6, 0]]
                .iter()
                .flat_map(|&c| {
                    FeatureCatalogue::cell(galaxy, FeatureCell::new(c).unwrap())
                        .features()
                        .to_vec()
                })
                .filter(|f| match f.marks() {
                    FeatureMarks::OpenCluster(m) => (3e3..3e4).contains(&m.initial_mass().value()),
                    _ => false,
                })
                .find_map(|f| FeatureInterior::of(galaxy, &f))
                .expect("the fixture has such an open cluster")
        })
    }

    /// A globular of 5,000–20,000 M☉, core-collapsed or not, shared by the tests.
    fn globular(collapsed: bool) -> &'static FeatureInterior {
        static GLOBULARS: [OnceLock<FeatureInterior>; 2] = [OnceLock::new(), OnceLock::new()];
        GLOBULARS[usize::from(collapsed)].get_or_init(|| {
            let galaxy = galaxy();
            FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
                .filter(|f| match f.marks() {
                    FeatureMarks::Globular(m) => (5e3..2e4).contains(&m.mass().value()),
                    _ => false,
                })
                .filter_map(|f| FeatureInterior::of(galaxy, &f))
                .find(|i| i.model().is_core_collapsed() == collapsed)
                .expect("the fixture has such a globular")
        })
    }

    /// The owned cell where `band` expects the most candidates.
    fn busiest(interior: &FeatureInterior, band: MassBand) -> NestedCell {
        interior
            .grid()
            .owned_cells()
            .max_by(|a, b| {
                interior
                    .expected_candidates(band, *a)
                    .total_cmp(&interior.expected_candidates(band, *b))
            })
            .expect("a grid has cells")
    }

    /// The busiest owned cell of `band` that holds a member.
    fn populated(
        galaxy: &Galaxy,
        interior: &FeatureInterior,
        band: MassBand,
    ) -> Option<NestedCell> {
        let mut cells: Vec<(f64, NestedCell)> = interior
            .grid()
            .owned_cells()
            .map(|c| (interior.expected_candidates(band, c), c))
            .collect();
        cells.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut out = Vec::new();
        cells.into_iter().map(|(_, c)| c).find(|&c| {
            interior.members_in_cell(galaxy, band, c, &mut out);
            !out.is_empty()
        })
    }

    fn total(interior: &FeatureInterior) -> f64 {
        MassBand::ALL
            .iter()
            .map(|&b| interior.table().expected(b))
            .sum()
    }

    /// Every member of `band` over the whole grid, with its local position.
    fn all_members(
        galaxy: &Galaxy,
        interior: &FeatureInterior,
        band: MassBand,
    ) -> Vec<(MemberRecord, PointLy)> {
        let mut out = Vec::new();
        let mut cell_members = Vec::new();
        for cell in interior.grid().owned_cells() {
            interior.members_in_cell(galaxy, band, cell, &mut cell_members);
            out.extend_from_slice(&cell_members);
        }
        out
    }

    #[test]
    fn every_member_s_id_round_trips_and_resolves_to_it() {
        let galaxy = galaxy();
        let interior = open_cluster();
        let mut n = 0;
        for band in MassBand::ALL {
            let members = all_members(galaxy, interior, band);
            for (member, _) in &members {
                let id = member.record().id();
                assert_eq!(SystemId::from_raw(id.raw()), Ok(id));
                n += 1;
            }
            // Resolving rebuilds the cluster's model, seconds each, so a few members a band.
            for (member, _) in members.iter().step_by(members.len() / 2 + 1) {
                assert_eq!(resolve(galaxy, member.record().id()), Ok(*member.record()));
            }
        }
        let expected = total(interior);
        assert!(
            (f64::from(n) - expected).abs() < 5.0 * expected.sqrt(),
            "{n} members against {expected} expected"
        );
    }

    #[test]
    fn a_galaxy_without_its_kinematic_tables_places_no_member() {
        let bare =
            Galaxy::from_params(Seed::new(0x0921_0000), GalaxyParams::milky_way_like()).unwrap();
        let interior = open_cluster();
        assert_eq!(FeatureInterior::of(&bare, interior.feature()), None);
        let cell = populated(galaxy(), interior, MassBand::A).unwrap();
        let mut out = Vec::new();
        interior.members_in_cell(galaxy(), MassBand::A, cell, &mut out);
        let SystemIdKind::FeatureMember(id) = out[0].0.record().id().kind() else {
            panic!("a member's ID is a feature member's")
        };
        assert_eq!(
            resolve_member(&bare, &NoInteriorCache, id),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }

    /// Members of one feature resolved through one shared [`KeepInteriors`], in any order, are
    /// the members resolved cold.
    #[test]
    fn resolving_through_a_kept_interior_does_not_depend_on_order() {
        use crate::galaxy::features::members::KeepInteriors;
        use hyperion_testkit::order::assert_order_independent;
        let galaxy = galaxy();
        let interior = open_cluster();
        let mut ids = Vec::new();
        for band in [MassBand::A, MassBand::B, MassBand::C] {
            if let Some(cell) = populated(galaxy, interior, band) {
                let mut out = Vec::new();
                interior.members_in_cell(galaxy, band, cell, &mut out);
                ids.extend(
                    out.iter()
                        .take(2)
                        .filter_map(|(m, _)| match m.record().id().kind() {
                            SystemIdKind::FeatureMember(id) => Some(id),
                            _ => None,
                        }),
                );
            }
        }
        assert!(ids.len() >= 4, "{ids:?}");
        let kept = KeepInteriors::new(galaxy);
        assert_order_independent(&ids, |&id| resolve_member(galaxy, &kept, id));
        for &id in &ids {
            assert_eq!(
                resolve_member(galaxy, &kept, id),
                resolve_member(galaxy, &NoInteriorCache, id)
            );
        }
    }

    #[test]
    fn an_index_past_the_count_or_a_rejected_candidate_is_no_such_system() {
        let galaxy = galaxy();
        let interior = open_cluster();
        let band = MassBand::A;
        let cell = busiest(interior, band);
        let count = interior.candidate_count(galaxy, band, cell);
        let past = interior.member_id(band, cell, count);
        assert_eq!(
            resolve_member(galaxy, &NoInteriorCache, past),
            Err(ResolveSystemError::NoSuchSystem)
        );
        let rejected = (0..count)
            .find(|&i| interior.candidate(galaxy, band, cell, i).is_none())
            .expect("the nearest-corner bound rejects some candidates");
        assert_eq!(
            resolve_member(
                galaxy,
                &NoInteriorCache,
                interior.member_id(band, cell, rejected)
            ),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }

    /// The radii of every class of `band` with enough members against its profile's
    /// distribution; returns how many classes were tested.
    fn assert_profiles(galaxy: &Galaxy, interior: &FeatureInterior, band: MassBand) -> usize {
        let table = interior.table();
        let members = all_members(galaxy, interior, band);
        let mut tested = 0;
        for (index, (class, _)) in table.classes(band).enumerate() {
            let profile = table.profile(band, index).unwrap();
            let mut radii: Vec<f64> = members
                .iter()
                .filter(|(m, _)| m.class() == class)
                .map(|(_, p)| (p.x * p.x + p.y * p.y + p.z * p.z).sqrt())
                .collect();
            if radii.len() < 300 {
                continue;
            }
            let ks = ks_one_sample(&mut radii, |r| profile.cumulative(r));
            assert_p_value(&format!("{class:?}'s radii"), ks.p_value, ALPHA);
            tested += 1;
        }
        tested
    }

    #[test]
    fn a_globular_s_members_follow_their_class_profiles() {
        let galaxy = galaxy();
        let interior = globular(false);
        let tested: usize = [MassBand::A, MassBand::B, MassBand::C]
            .into_iter()
            .map(|band| assert_profiles(galaxy, interior, band))
            .sum();
        assert!(tested >= 3, "{tested} classes tested");
    }

    #[test]
    fn a_collapsed_cluster_s_cusp_is_placed_by_its_envelope() {
        let galaxy = galaxy();
        let interior = globular(true);
        let tested: usize = [MassBand::A, MassBand::B, MassBand::C]
            .into_iter()
            .map(|band| assert_profiles(galaxy, interior, band))
            .sum();
        assert!(tested >= 2, "{tested} classes tested");
        // Its eight central cells expect few candidates, not the millions the cusp's peak would
        // give a uniform bound.
        let grid = interior.grid();
        for cell in grid.owned_cells().filter(|c| c.level() == 0) {
            let local = grid.local_cell(cell);
            if local.nearest_radius() <= 0.0 {
                for band in MassBand::ALL {
                    let n = interior.expected_candidates(band, cell);
                    assert!(n < CELL_TARGET, "{band:?} {cell:?}: {n}");
                }
            }
        }
    }

    const CELL_TARGET: f64 = 2_000.0;

    #[test]
    fn a_member_does_not_depend_on_what_else_was_placed() {
        let galaxy = galaxy();
        let interior = open_cluster();
        let grid = interior.grid();
        let mut keys: Vec<(MassBand, NestedCell)> = [
            (MassBand::C, [7, 8, 8], 0),
            (MassBand::B, [3, 8, 8], 2),
            (MassBand::A, [0, 15, 3], 5),
        ]
        .into_iter()
        .map(|(b, c, l)| (b, grid.cell(l, c).unwrap()))
        .collect();
        keys.insert(
            0,
            (
                MassBand::A,
                populated(galaxy, interior, MassBand::A).expect("band A has members"),
            ),
        );
        assert_order_independent(&keys, |&(band, cell)| {
            let mut out = Vec::new();
            interior.members_in_cell(galaxy, band, cell, &mut out);
            out
        });
        // A candidate drawn alone is the cell's.
        let (band, cell) = keys[0];
        let mut out = Vec::new();
        interior.members_in_cell(galaxy, band, cell, &mut out);
        let first = out.first().expect("the cell has members").0;
        let SystemIdKind::FeatureMember(id) = first.record().id().kind() else {
            panic!("a member's ID is a feature member's")
        };
        let MemberSlot::InCell { index, .. } = id.slot() else {
            panic!("a nested member is in a cell")
        };
        assert_eq!(
            interior.candidate(galaxy, band, cell, index).map(|c| c.0),
            Some(first)
        );
    }

    /// The fullest cell and band over every owned cell is the one on the axes, and under the
    /// target, for the shared open cluster and the two globulars.
    #[test]
    fn the_axes_hold_the_fullest_cell_and_it_is_far_from_the_index() {
        for interior in [open_cluster(), globular(false), globular(true)] {
            let grid = interior.grid();
            let table = interior.table();
            let all = grid
                .owned_cells()
                .flat_map(|c| MassBand::ALL.map(|b| table.cell_candidates(b, grid, c)))
                .fold(0.0, f64::max);
            let axes = table.peak_candidates(grid);
            assert!(
                all <= axes * (1.0 + 1e-12),
                "{all} over all cells, {axes} on the axes"
            );
            assert!(all <= CELL_TARGET, "{all}");
        }
    }

    /// Every cluster of three feature cells and every other globular of the first hundred in the
    /// walk (734 at version 15): their fullest cell and band on the axes (where the bound peaks) against
    /// the 8,192 index with eight standard deviations to spare; the plan's 2,000 is a target, and
    /// a dense core the finest grid that reaches its tidal radius cannot bring under it is counted.
    #[test]
    #[ignore = "slow: some hundreds of cluster models, seconds each"]
    fn no_cell_and_band_of_the_fixture_s_clusters_nears_the_index() {
        let galaxy = galaxy();
        let mut features: Vec<FeatureRecord> = [[0, 6, 0], [1, 6, 0], [0, 1, 0]]
            .iter()
            .flat_map(|&c| {
                FeatureCatalogue::cell(galaxy, FeatureCell::new(c).unwrap())
                    .features()
                    .to_vec()
            })
            .collect();
        features.extend(
            FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
                .take(100)
                .step_by(2),
        );
        let (mut clusters, mut over_target, mut worst) = (0, 0, 0.0_f64);
        for f in &features {
            let Some(interior) = FeatureInterior::of(galaxy, f) else {
                continue;
            };
            clusters += 1;
            let peak = interior.table().peak_candidates(interior.grid());
            worst = worst.max(peak);
            if peak > CELL_TARGET {
                over_target += 1;
            }
            // The index holds the fullest cell's Poisson count with eight standard deviations to
            // spare, as the feature-level list's does.
            assert!(
                peak + 8.0 * peak.sqrt() < f64::from(MemberSlot::INDEX_LIMIT),
                "{:?}: {peak}",
                interior.feature().id()
            );
        }
        println!("{clusters} clusters, {over_target} over the target, fullest {worst:.0}");
        assert!(clusters > 50, "{clusters} clusters");
    }

    /// A globular of `members` expected members whose centre holds `core` members per cubic
    /// light-year, from 47 Tucanae's catalogue parameters rescaled in mass and core radius.
    fn synthetic_globular(
        galaxy: &Galaxy,
        members: f64,
        core: f64,
    ) -> (ClusterModel, MemberClassTable) {
        use crate::galaxy::features::testing::{TUC_47, catalogue_parameters, named_cluster};
        use crate::units::{LightYears, SolarMasses};
        let mut p = catalogue_parameters(named_cluster(TUC_47));
        let build = |p: &crate::galaxy::features::cluster::ClusterParameters| {
            let model = ClusterModel::new(galaxy, p);
            let reach = LightYears::new(4.0 * model.tidal_radius().value());
            let table = MemberClassTable::new(galaxy, &model, reach, [1.0, 0.0, 0.0]);
            (model, table)
        };
        for _ in 0..4 {
            let (_, table) = build(&p);
            let n: f64 = MassBand::ALL.iter().map(|&b| table.expected(b)).sum();
            let centre = table.density(&PointLy::new(0.0, 0.0, 0.0));
            let k = members / n;
            p.mass = SolarMasses::new(p.mass.value() * k);
            p.initial_mass = SolarMasses::new(p.initial_mass.value() * k);
            p.mass_loss_rate *= k;
            let r_c = p.core_radius.expect("a globular's core is given").value();
            p.core_radius = Some(LightYears::new(r_c * math::cbrt(k * centre / core)));
        }
        build(&p)
    }

    #[test]
    fn a_globular_of_a_million_systems_peaks_near_600_a_cell_and_its_light_counts_rise_gently() {
        let galaxy = galaxy();
        let (model, table) = synthetic_globular(galaxy, 1e6, 240.0);
        let n: f64 = MassBand::ALL.iter().map(|&b| table.expected(b)).sum();
        let centre = table.density(&PointLy::new(0.0, 0.0, 0.0));
        assert!(
            (n / 1e6 - 1.0).abs() < 0.05 && (centre / 240.0 - 1.0).abs() < 0.05,
            "{n} {centre}"
        );
        let grid = table.grid();
        let table = table.with_tails_to(grid.reach());
        let per_level = |band: MassBand| -> Vec<f64> {
            (0..grid.levels())
                .map(|level| {
                    grid.owned_cells()
                        .filter(|c| c.level() == level)
                        .map(|c| table.cell_candidates(band, &grid, c))
                        .fold(0.0, f64::max)
                })
                .collect()
        };
        let mut peak = 0.0_f64;
        for band in MassBand::ALL {
            let levels = per_level(band);
            println!("{band:?}: {levels:.0?}");
            peak = peak.max(levels.iter().copied().fold(0.0, f64::max));
        }
        println!(
            "width 2^{} ly, r_c {:.2} ly, r_t {:.0} ly, peak {peak:.0}",
            grid.width_log2(),
            model.core_radius().value(),
            model.tidal_radius().value()
        );
        assert!(
            (300.0..1_200.0).contains(&peak),
            "the fullest cell expects {peak}"
        );
        // Ruling 139.3: outside the core the heavy bands' counts per cell stay flat or fall level
        // by level; the light bands', falling as r⁻² to r⁻²·⁵ (Design note 9, ruling 139.1), rise
        // by at most 2^1.1 a level, a level's inner edge (four cells out) standing for its radius.
        // The rise is asserted from twice the half-mass radius, the range of ruling 139.1's slope
        // test: between r_h and 2 r_h the ruled outer factor has reached only 0.8 of r⁻², and band
        // A rises ×2.164 (2^1.11) from the level at 16 ly to the next, which plan 09 records.
        for band in [MassBand::D, MassBand::E] {
            let levels = per_level(band);
            let top = levels
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(0, |(i, _)| i);
            for pair in levels[top..].windows(2) {
                assert!(pair[1] <= pair[0] * 1.05, "{band:?}: {levels:.1?}");
            }
        }
        let (r_h, r_t) = (
            model.half_mass_radius().value(),
            model.tidal_radius().value(),
        );
        let inside: Vec<usize> = grid
            .all_levels()
            .filter(|l| l.level() > 0)
            .filter(|l| (2.0 * r_h..=0.5 * r_t).contains(&(4.0 * l.edge().value())))
            .map(|l| usize::from(l.level()))
            .collect();
        assert!(
            inside.len() >= 2,
            "levels {inside:?} between r_h and r_t ÷ 2"
        );
        for band in [MassBand::A, MassBand::B, MassBand::C] {
            let levels = per_level(band);
            for pair in inside.windows(2) {
                let rise = levels[pair[1]] / levels[pair[0]];
                println!("{band:?} levels {pair:?}: ×{rise:.3}");
                assert!(
                    rise <= math::exp2(1.1) * (1.0 + 1e-9),
                    "{band:?}: ×{rise} from level {} ({levels:.1?})",
                    pair[0]
                );
            }
        }
    }

    #[test]
    fn members_golden() {
        let galaxy = galaxy();
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let open = open_cluster();
        let glob = globular(false);
        for (name, interior) in [("open cluster", &open), ("globular", &glob)] {
            let grid = interior.grid();
            w.line(&format!(
                "{name} {} width 2^{} ly",
                interior.feature().id().designation(),
                grid.width_log2()
            ));
            // Each band's busiest cell that holds a member, and its first three members.
            for band in MassBand::ALL {
                let Some(cell) = populated(galaxy, interior, band) else {
                    w.line(&format!("band {band:?} no members"));
                    continue;
                };
                let n = interior.candidate_count(galaxy, band, cell);
                w.line(&format!(
                    "band {band:?} level {} cell {:?} candidates {n}",
                    cell.level(),
                    cell.cell()
                ));
                let mut members = Vec::new();
                interior.members_in_cell(galaxy, band, cell, &mut members);
                for (m, p) in members.iter().take(3) {
                    let label = format!("{:x}", m.record().id());
                    w.line(&format!("{label} {:?}", m.class().kind));
                    w.f64(&format!("{label}.x"), p.x);
                    w.f64(&format!("{label}.y"), p.y);
                    w.f64(&format!("{label}.z"), p.z);
                    w.f64(
                        &format!("{label}.mass"),
                        m.record().primary_initial_mass().value(),
                    );
                    w.f64(&format!("{label}.age"), m.record().age_at_epoch().value());
                    let [vx, vy, vz] = m.velocity().metres_per_second();
                    w.f64(&format!("{label}.vx"), vx);
                    w.f64(&format!("{label}.vy"), vy);
                    w.f64(&format!("{label}.vz"), vz);
                }
            }
        }
        // A collapsed globular's eight central cells, proposed under the cusp's envelope.
        let collapsed = globular(true);
        w.line(&format!(
            "collapsed globular {}",
            collapsed.feature().id().designation()
        ));
        for c in [[7, 7, 7], [8, 8, 8], [7, 8, 7], [8, 7, 8]] {
            let cell = collapsed.grid().cell(0, c).unwrap();
            for band in MassBand::ALL {
                let n = collapsed.candidate_count(galaxy, band, cell);
                w.f64(
                    &format!("cell {c:?} band {band:?} expected"),
                    collapsed.expected_candidates(band, cell),
                );
                w.line(&format!("cell {c:?} band {band:?} candidates {n}"));
                let mut members = Vec::new();
                collapsed.members_in_cell(galaxy, band, cell, &mut members);
                for (m, p) in members.iter().take(2) {
                    let label = format!("{:x}", m.record().id());
                    w.line(&format!("{label} {:?}", m.class().kind));
                    w.f64(&format!("{label}.x"), p.x);
                    w.f64(&format!("{label}.y"), p.y);
                    w.f64(&format!("{label}.z"), p.z);
                    w.f64(
                        &format!("{label}.mass"),
                        m.record().primary_initial_mass().value(),
                    );
                }
            }
        }
        golden!("galaxy/features/members", w.as_str());
    }
}
