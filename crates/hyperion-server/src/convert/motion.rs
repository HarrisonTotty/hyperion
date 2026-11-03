//! Where a system a client names is (plan 07, P07.T10.c): the one place the server places a system
//! it resolved from a client's ID, routed by the record's origin.
//!
//! Plan 03's `position_at` and plan 08's `epoch_velocity` are a grid record's. In a galaxy with
//! kinematic tables, which every production galaxy has, they panic for any other record, the first
//! away from the epoch and the second at every time (plan 03's Risks, "`position_at` and feature
//! members"), so every request that places a system from a client's ID places it here:
//! `resolve_system`, `extinction`'s system targets, `scene_ship`'s frame, and the scene's core
//! after it.
//!
//! From generator version 27 a range query returns three more kinds of system beside the grid's
//! and the features' (plan 10, P10.T10.b's registration member; plan 09, P09.T37), and each is
//! placed here as its row places it, bit for bit: a member of the galactic centre on its path
//! ([`CentrePath`]), a stream's or a dwarf core's member drifting at its own velocity
//! ([`GlobalMemberRecord::position_at`]), and a catalogue class's host on its class's broken line
//! ([`CatalogueSystem::position_at`]). Below 27 no query returns them, and each is refused at every
//! time, as before: a centre member and a list member under the list source's hold
//! ([`GlobalListSource::is_registered_in`]), a host under the class source's
//! ([`CatalogueClassSource::registered_in`]).

use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueClassSource, CatalogueSystem, system_of_record,
};
use hyperion_sim::galaxy::features::centre::{CentreOrbitCache, CentrePath, NoCentreOrbitCache};
use hyperion_sim::galaxy::features::members::NoInteriorCache;
use hyperion_sim::galaxy::features::members::source::FeatureInteriorCache;
use hyperion_sim::galaxy::global_list::GlobalListSource;
use hyperion_sim::galaxy::global_list::members::GlobalMemberRecord;
use hyperion_sim::galaxy::motion::StateAt;
use hyperion_sim::galaxy::placement::{
    ResolveSystemError, SystemOrigin, SystemRecord, resolve_list_member_with,
};
use hyperion_sim::galaxy::query::{SystemHit, epoch_velocity, position_at};
use hyperion_sim::id::{SystemId, SystemIdKind};
use hyperion_sim::observe::{
    Drift, ObservedSystem, Observer, Retardation, TraceMotionError, Trajectory, observe_hit_on,
    summary_observed_on,
};
use hyperion_sim::stellar::system::{SystemStars, SystemSummary};
use hyperion_sim::time::UniverseTime;

use crate::compute::{ListedTubes, Listing};

/// Where a stream's or a dwarf core's member is resolved from (plan 10, P10.T10.b): a served
/// query's list source, which holds its tables, band shares and cores' models, or a request's view
/// of its galaxy's list ([`ListedTubes`]), through plan 10's `resolve_list_member_with`. Either
/// gives the same member.
pub(crate) trait ListMembers {
    /// The member `id` names in `galaxy`.
    ///
    /// # Errors
    ///
    /// The [`ResolveSystemError`] of an ID that names no member of the galaxy's list.
    fn list_member(
        &self,
        galaxy: &Galaxy,
        id: SystemId,
    ) -> Result<GlobalMemberRecord, ResolveSystemError>;
}

impl ListMembers for GlobalListSource<'_> {
    fn list_member(
        &self,
        galaxy: &Galaxy,
        id: SystemId,
    ) -> Result<GlobalMemberRecord, ResolveSystemError> {
        self.member(galaxy, id)
    }
}

impl ListMembers for ListedTubes {
    fn list_member(
        &self,
        galaxy: &Galaxy,
        id: SystemId,
    ) -> Result<GlobalMemberRecord, ResolveSystemError> {
        resolve_list_member_with(galaxy, id, self)
    }
}

/// What [`system_motion`] reads a system's path through: the server's caches, each the same
/// whatever it holds (plan 12, P12.T6; plan 10, P10.T10.b): a catalogue feature's interiors, the
/// galactic centre's cells of members and their orbits, and the galaxy's global list, if the
/// caller has it.
#[derive(Clone, Copy)]
pub(crate) struct MotionReads<'a> {
    interiors: &'a dyn FeatureInteriorCache,
    orbits: &'a dyn CentreOrbitCache,
    list: Option<&'a dyn ListMembers>,
}

impl<'a> MotionReads<'a> {
    /// The reads of a caller that holds the features' interiors in `interiors` and the centre's
    /// orbits in `orbits`, and no list: a list member is then refused.
    #[must_use]
    pub(crate) fn new(
        interiors: &'a dyn FeatureInteriorCache,
        orbits: &'a dyn CentreOrbitCache,
    ) -> Self {
        Self {
            interiors,
            orbits,
            list: None,
        }
    }

    /// The reads of a caller that holds nothing: every path is built afresh.
    #[must_use]
    pub(crate) fn uncached() -> MotionReads<'static> {
        MotionReads::new(&NoInteriorCache, &NoCentreOrbitCache)
    }

    /// The same reads, a list member resolved through `list`.
    #[must_use]
    pub(crate) fn with_list(self, list: &'a dyn ListMembers) -> Self {
        Self {
            list: Some(list),
            ..self
        }
    }

    /// The same reads, a list member resolved through `listing` if it is the galaxy's list
    /// ([`Listing::Listed`]).
    #[must_use]
    pub(crate) fn with_listing(self, listing: &'a Listing) -> Self {
        match listing {
            Listing::Unlisted => self,
            Listing::Listed(tubes) => self.with_list(tubes),
        }
    }

    /// The galaxy's list, if the caller has it.
    #[must_use]
    pub(crate) fn list(&self) -> Option<&'a dyn ListMembers> {
        self.list
    }
}

/// How a system a client names moves: its path, found once by its record's origin, which
/// [`system_motion`] reads at a time and observed mode reads the light on.
///
/// It needs no cache once found, so a caller that places one system at many times, as the scene's
/// core places the ship's frame, keeps it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SystemLine {
    /// A grid system: where a range query finds it, bit for bit, [`position_at`], with plan 08's
    /// [`epoch_velocity`] beside it, as a range row carries it.
    Grid(SystemRecord),
    /// A system on the line [`Drift::of_record`] gives it: a catalogue feature's member, its
    /// cluster's motion and its own, which plan 08's draw is not (plan 09, P09.T10; plan 12,
    /// P12.T6), and an origin the sim adds later, on the sim's own line for it.
    Line(SystemRecord, Drift),
    /// A stream's or a dwarf core's member, drifting at its own velocity, as the list's source
    /// places it ([`GlobalMemberRecord::position_at`]).
    Listed(GlobalMemberRecord),
    /// A member of the galactic centre, on its path ([`CentrePath`]).
    Centre(CentrePath),
    /// A catalogue class's host, on its class's line broken at its explosion
    /// ([`CatalogueSystem::position_at`]), with its velocity there.
    Catalogue(CatalogueSystem),
}

impl SystemLine {
    /// The path of the system `record`, read through `reads` (the module's documentation).
    ///
    /// # Errors
    ///
    /// Below generator version 27, at every time: [`TraceMotionError::CentreOrbitNotBuilt`] for a
    /// member of the galactic centre, [`TraceMotionError::GlobalListMember`] for a stream's or a
    /// dwarf core's member, and [`TraceMotionError::CatalogueMotionNotTraced`] for a catalogue
    /// system. From 27, [`TraceMotionError::GlobalListMember`] for a list member if `reads` holds no
    /// list.
    ///
    /// # Panics
    ///
    /// If `record` is a member, of a feature, the centre or the list, or a catalogue system that
    /// does not resolve in `galaxy`, which only a record of another galaxy can be, or if a cache of
    /// `reads` is of another galaxy, which the server's handles assert.
    pub(crate) fn of(
        galaxy: &Galaxy,
        reads: MotionReads<'_>,
        record: &SystemRecord,
    ) -> Result<Self, TraceMotionError> {
        let served = GlobalListSource::is_registered_in(galaxy);
        match record.origin() {
            SystemOrigin::Grid(_) => Ok(Self::Grid(*record)),
            #[expect(
                clippy::match_same_arms,
                reason = "`SystemOrigin` is `#[non_exhaustive]`, so the wildcard below cannot go; \
                          the feature member is named so that its line is this function's choice, \
                          not the fallback's"
            )]
            SystemOrigin::FeatureMember { .. } => Ok(Self::Line(
                *record,
                Drift::of_record_in(galaxy, reads.interiors, record)?,
            )),
            SystemOrigin::CentreMember { .. } => {
                if !served {
                    return Err(TraceMotionError::CentreOrbitNotBuilt(record.id()));
                }
                let SystemIdKind::Centre(id) = record.id().kind() else {
                    unreachable!("a centre member's record is built with a centre ID")
                };
                Ok(Self::Centre(
                    CentrePath::of(galaxy, reads.orbits, id)
                        .expect("a centre member's record resolves in the galaxy that placed it"),
                ))
            }
            SystemOrigin::GlobalListMember { .. } => {
                let list = reads
                    .list
                    .filter(|_| served)
                    .ok_or(TraceMotionError::GlobalListMember(record.id()))?;
                Ok(Self::Listed(list.list_member(galaxy, record.id()).expect(
                    "a list member's record resolves through the list of the galaxy that placed it",
                )))
            }
            SystemOrigin::CatalogueSystem(_) => {
                if CatalogueClassSource::registered_in(galaxy).is_none() {
                    return Err(TraceMotionError::CatalogueMotionNotTraced(record.id()));
                }
                Ok(Self::Catalogue(system_of_record(galaxy, record)))
            }
            _ => Ok(Self::Line(
                *record,
                Drift::of_record_in(galaxy, reads.interiors, record)?,
            )),
        }
    }

    /// The system's record.
    #[must_use]
    pub(crate) fn record(&self) -> &SystemRecord {
        match self {
            Self::Grid(record) | Self::Line(record, _) => record,
            Self::Listed(member) => member.record(),
            Self::Centre(path) => path.record(),
            Self::Catalogue(system) => system.record(),
        }
    }

    /// Where the system is at `t`, its barycentre in the galactic frame, and its velocity there,
    /// m/s along the galactic axes: its range row's at `t`, bit for bit, but for a catalogue
    /// feature's member, on its own line, whose row plan 03 rounds differently by up to a few
    /// metres.
    ///
    /// # Errors
    ///
    /// [`TraceMotionError::Disrupted`] for a tidal disruption's victim after its disruption, when
    /// it is no system.
    pub(crate) fn at(
        &self,
        galaxy: &Galaxy,
        t: UniverseTime,
    ) -> Result<(GalacticPosition, GalacticVelocity), TraceMotionError> {
        match self {
            Self::Grid(record) => Ok((
                position_at(galaxy, record, t),
                epoch_velocity(galaxy, record),
            )),
            Self::Line(_, line) => Ok((line.position_at(t), line.velocity_at(t))),
            Self::Listed(member) => Ok((member.position_at(t), member.velocity())),
            Self::Centre(path) => match path.state_at(t) {
                StateAt::Exists(state) => Ok((*state.position(), state.velocity())),
                StateAt::NoSuchSystem => Err(TraceMotionError::Disrupted(path.record().id())),
            },
            Self::Catalogue(system) => Ok((system.position_at(t), system.motion().velocity_at(t))),
        }
    }

    /// What `observer` receives from the system `hit`, this path's, whose stars are `stars`, if
    /// the caller gives them: [`observe_hit_on`] on the path.
    #[must_use]
    pub(crate) fn observe(
        &self,
        galaxy: &Galaxy,
        hit: &SystemHit,
        stars: Option<&SystemStars>,
        observer: &Observer,
    ) -> ObservedSystem {
        match self {
            Self::Grid(record) => {
                observe_hit_on(galaxy, hit, &grid_line(galaxy, record), stars, observer)
            }
            Self::Line(_, line) => observe_hit_on(galaxy, hit, line, stars, observer),
            Self::Listed(member) => observe_hit_on(
                galaxy,
                hit,
                &Drift::of_global_member(member),
                stars,
                observer,
            ),
            Self::Centre(path) => observe_hit_on(galaxy, hit, path, stars, observer),
            Self::Catalogue(system) => {
                observe_hit_on(galaxy, hit, system.motion(), stars, observer)
            }
        }
    }

    /// The summary of the system's stars `stars` as `observer` sees it, with the retardation:
    /// [`summary_observed_on`] on the path.
    #[must_use]
    pub(crate) fn summary_observed(
        &self,
        galaxy: &Galaxy,
        stars: &SystemStars,
        observer: &Observer,
    ) -> (SystemSummary, Retardation) {
        match self {
            Self::Grid(record) => summary_observed_on(stars, &grid_line(galaxy, record), observer),
            Self::Line(_, line) => summary_observed_on(stars, line, observer),
            Self::Listed(member) => {
                summary_observed_on(stars, &Drift::of_global_member(member), observer)
            }
            Self::Centre(path) => summary_observed_on(stars, path, observer),
            Self::Catalogue(system) => summary_observed_on(stars, system.motion(), observer),
        }
    }
}

/// A grid system's line: plan 08's velocity from its epoch position.
fn grid_line(galaxy: &Galaxy, record: &SystemRecord) -> Drift {
    Drift::of_record(galaxy, record).expect("a grid system moves on plan 08's line")
}

/// Where the system `record` is at `t`, its barycentre in the galactic frame, and its velocity
/// there, m/s along the galactic axes, by the record's origin ([`SystemLine::of`], then
/// [`SystemLine::at`]): its range row's position and velocity at `t`, bit for bit, for every
/// system a range query returns but a catalogue feature's member, which moves on its own line
/// ([`Drift::of_record_in`]).
///
/// This is the one place the server places a system from a client's ID; [`system_place`] and the
/// range rows read grid records alone. Version 25's wiring of the Kepler regime changes the grid
/// arm: the velocity it reports becomes the orbit's at `t` (P09.T28.b.b's deferred finding (2)).
///
/// [`system_place`]: super::system_place
///
/// # Errors
///
/// As [`SystemLine::of`], at every time, and [`SystemLine::at`].
///
/// # Panics
///
/// As [`SystemLine::of`].
pub(crate) fn system_motion(
    galaxy: &Galaxy,
    reads: MotionReads<'_>,
    record: &SystemRecord,
    t: UniverseTime,
) -> Result<(GalacticPosition, GalacticVelocity), TraceMotionError> {
    SystemLine::of(galaxy, reads, record)?.at(galaxy, t)
}

#[cfg(test)]
pub(crate) mod origins {
    use std::sync::OnceLock;

    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
    use hyperion_sim::galaxy::features::centre::events::CentreVictims;
    use hyperion_sim::galaxy::features::centre::members::CentrePlacement;
    use hyperion_sim::galaxy::features::members::FeatureInterior;
    use hyperion_sim::galaxy::imf::MassBand;
    use hyperion_sim::galaxy::placement::{SystemRecord, resolve};
    use hyperion_sim::id::{CentreMemberId, SystemId};
    use hyperion_sim::units::LightYears;

    use super::GalacticPosition;

    /// The seed of the server tests' universes.
    pub(crate) const SEED: u64 = 0x4d2;

    /// A layer-C system at the solar circle, the extinction test's.
    pub(crate) const GRID: u64 = 0x4200_2cb2_0000_0001;

    /// The galaxy of [`SEED`] as the server builds it, with its kinematic tables, in which plan
    /// 03's `position_at` panics for any record but a grid record's away from the epoch.
    pub(crate) fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)).with_full_potential())
    }

    /// The system `id` names, as `resolve` gives it.
    pub(crate) fn record_of(id: SystemId) -> SystemRecord {
        resolve(galaxy(), id).expect("the system resolves")
    }

    /// The grid system [`GRID`].
    pub(crate) fn a_grid_system() -> SystemRecord {
        record_of(SystemId::from_raw(GRID).expect("a system ID"))
    }

    /// A member of band C of the first feature with members within 3,000 ly of the Sun-like point,
    /// as the integration tests' `a_feature_member` finds it.
    pub(crate) fn a_feature_member() -> SystemRecord {
        let galaxy = galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube");
        let interior =
            FeatureCatalogue::near(galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
                .find_map(|feature| FeatureInterior::of(galaxy, &feature))
                .expect("a feature with members lies within 3,000 ly of the Sun");
        let mut members = Vec::new();
        let member = interior
            .grid()
            .owned_cells()
            .find_map(|cell| {
                interior.members_in_cell(galaxy, MassBand::C, cell, &mut members);
                members.first().map(|(member, _)| *member)
            })
            .expect("the feature has a member in band C");
        record_of(member.record().id())
    }

    /// A core-collapse entry of the class grid in the inner disc (plan 09, P09.T33.a).
    pub(crate) fn a_catalogue_system() -> SystemRecord {
        use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
        use hyperion_sim::galaxy::catalogue_classes::{
            CatalogueCellKey, CatalogueClassCell, ClassId, system_of,
        };

        let galaxy = galaxy();
        let process = CoreCollapseProcess::new(galaxy);
        let entry = (8..28)
            .map(|y| {
                CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, y, 0])
                    .expect("a cell of the root cube")
            })
            .flat_map(|key| CatalogueClassCell::generate(galaxy, &process, key).into_entries())
            .next()
            .expect("the inner disc has recent deaths");
        *system_of(galaxy, entry.id())
            .expect("an entry resolves")
            .record()
    }

    /// The first member of the centre's fullest band and cell, the black hole, and the black hole's
    /// first victim.
    pub(crate) fn the_centre_s_members() -> [SystemRecord; 3] {
        let galaxy = galaxy();
        let placement = CentrePlacement::new(galaxy.centre().expect("the centre builds"));
        let (_, band, cell) = placement.peak_candidates();
        let member = (0..)
            .find_map(|index| placement.candidate(galaxy, band, cell, index))
            .expect("the fullest cell holds a member");
        let victims = CentreVictims::of(galaxy).expect("the victims build");
        let victim = victims
            .victims()
            .first()
            .expect("the black hole has victims");
        [
            member.record().id(),
            SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE),
            SystemId::from(victim.id()),
        ]
        .map(record_of)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::GENERATOR_VERSION;
    use hyperion_sim::coords::LyCell;
    use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord};
    use hyperion_sim::galaxy::features::members::{FeatureInterior, NoInteriorCache};
    use hyperion_sim::galaxy::imf::MassBand;
    use hyperion_sim::id::SystemIdKind;

    use super::origins::{
        SEED, a_catalogue_system, a_feature_member, a_grid_system, galaxy, the_centre_s_members,
    };
    use super::*;
    use crate::compute::{GalaxyKey, SharedClusterModelCache};

    /// The epoch, a century after it and 999 years before it.
    fn times() -> [UniverseTime; 3] {
        [0, 100, -999]
            .map(|years| UniverseTime::from_julian_years(years).expect("inside the clock window"))
    }

    /// A position's cell and its offset's bits, which tell two points apart bit for bit.
    fn bits(position: &GalacticPosition) -> (LyCell, [u64; 3]) {
        (position.cell(), position.offset_metres().map(f64::to_bits))
    }

    fn velocity_bits(velocity: GalacticVelocity) -> [u64; 3] {
        velocity.metres_per_second().map(f64::to_bits)
    }

    #[test]
    fn a_grid_system_is_placed_as_the_range_query_places_it() {
        let galaxy = galaxy();
        let record = a_grid_system();
        assert!(matches!(record.origin(), SystemOrigin::Grid(_)));
        for t in times() {
            let (position, velocity) =
                system_motion(galaxy, MotionReads::uncached(), &record, t).unwrap();
            assert_eq!(
                bits(&position),
                bits(&position_at(galaxy, &record, t)),
                "{t}"
            );
            assert_eq!(
                velocity_bits(velocity),
                velocity_bits(epoch_velocity(galaxy, &record)),
                "{t}"
            );
        }
    }

    /// Two members of the first feature with members near the Sun, and that feature's record.
    fn two_members_of_one_feature() -> (FeatureRecord, [SystemRecord; 2]) {
        let galaxy = galaxy();
        let SystemIdKind::FeatureMember(id) = a_feature_member().id().kind() else {
            panic!("a feature member's ID");
        };
        let feature =
            FeatureCatalogue::resolve(galaxy, id.feature().into()).expect("the member's feature");
        let interior = FeatureInterior::of(galaxy, &feature).expect("the feature has members");
        let (mut members, mut found) = (Vec::new(), Vec::new());
        for cell in interior.grid().owned_cells() {
            interior.members_in_cell(galaxy, MassBand::C, cell, &mut members);
            found.extend(members.iter().map(|(member, _)| *member.record()));
            if found.len() >= 2 {
                break;
            }
        }
        (feature, [found[0], found[1]])
    }

    /// Two members of one feature are placed the same, bit for bit and in either order, whatever
    /// state the server's interior cache is in: holding the feature's grid-reach bound alone, its
    /// interior, through a job's kept view and with no budget, as through no cache (P12.T6).
    #[test]
    fn a_feature_member_is_placed_the_same_through_the_server_s_interior_cache() {
        use hyperion_testkit::order::assert_order_independent;

        let galaxy = galaxy();
        let (feature, members) = two_members_of_one_feature();
        let key = GalaxyKey::new(SEED, GENERATOR_VERSION);
        let placed = |interiors: &dyn FeatureInteriorCache, record: &SystemRecord| {
            times().map(|t| {
                let reads = MotionReads::new(interiors, &NoCentreOrbitCache);
                let (position, velocity) = system_motion(galaxy, reads, record, t).unwrap();
                (bits(&position), velocity_bits(velocity))
            })
        };
        let cache = SharedClusterModelCache::new(64 << 20);
        let (handle, kept) = (cache.handle(key), cache.kept(key));
        assert!(
            handle.grid_reach(galaxy, &feature).is_some(),
            "the bound held"
        );
        let unbudgeted = SharedClusterModelCache::new(0);
        let none = unbudgeted.handle(key);
        for (state, interiors) in [
            ("the bound held", &handle as &dyn FeatureInteriorCache),
            ("the interior held", &handle),
            ("a kept view", &kept),
            ("no budget", &none),
        ] {
            for record in &members {
                assert_eq!(
                    placed(interiors, record),
                    placed(&NoInteriorCache, record),
                    "{state}"
                );
            }
            assert_order_independent(&members, |record| placed(interiors, record));
        }
        assert_eq!(
            cache.counters().entries(),
            1,
            "the feature's interior is kept"
        );
    }

    #[test]
    fn a_feature_member_moves_on_its_own_line() {
        let galaxy = galaxy();
        let record = a_feature_member();
        assert!(matches!(
            record.origin(),
            SystemOrigin::FeatureMember { .. }
        ));
        let line = Drift::of_record(galaxy, &record).unwrap();
        for t in times() {
            let (position, velocity) =
                system_motion(galaxy, MotionReads::uncached(), &record, t).unwrap();
            assert_eq!(bits(&position), bits(&line.position_at(t)), "{t}");
            assert_eq!(
                velocity_bits(velocity),
                velocity_bits(line.velocity()),
                "{t}"
            );
        }
    }

    #[test]
    fn the_centre_s_members_are_refused_at_every_time() {
        let galaxy = galaxy();
        for record in the_centre_s_members() {
            assert!(
                matches!(record.origin(), SystemOrigin::CentreMember { .. }),
                "{:?}",
                record.id()
            );
            for t in times() {
                assert_eq!(
                    system_motion(galaxy, MotionReads::uncached(), &record, t),
                    Err(TraceMotionError::CentreOrbitNotBuilt(record.id())),
                    "{:?} at {t}",
                    record.id()
                );
            }
        }
    }

    /// From generator version 27 the range query serves the catalogue classes' hosts (P09.T37.a),
    /// so a catalogue system is traced at every time, on its class's line.
    #[test]
    fn a_catalogue_system_is_traced_at_every_time() {
        let galaxy = galaxy();
        let record = a_catalogue_system();
        assert!(matches!(record.origin(), SystemOrigin::CatalogueSystem(_)));
        for t in times() {
            assert!(
                system_motion(galaxy, MotionReads::uncached(), &record, t).is_ok(),
                "{t}"
            );
        }
    }
}
