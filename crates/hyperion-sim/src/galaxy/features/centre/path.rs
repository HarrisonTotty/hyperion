//! Where a member of the galactic centre is at any time, by its ID, as the centre's source places
//! it (plan 09, P09.T37.b; plan 10, P10.T10.b's registration member).
//!
//! [`CentreMemberSource`](super::CentreMemberSource) places its hits member by member at the
//! sphere's time, each on the law its kind moves by. [`CentrePath`] is one member's, found again
//! from its ID, so that a system a client names is where the row that listed it put it, bit for
//! bit:
//!
//! - **The black hole**, member zero, at rest at the galactic origin.
//! - **A member of the centre's cells**, on its [`Trajectory`] (P09.T28: its Kepler orbit, or
//!   from version 25 its exact orbit, inside the influence radius, a straight line outside), the
//!   very one its cell's [`CentreCellOrbits`] holds, through the caller's [`CentreOrbitCache`].
//! - **A tidal disruption's victim** (P09.T30.b), on its parabola up to its disruption, and no
//!   system after it.
//! - **A recent entry of the centre's list** (P09.T36.b), on the straight line its site lies on,
//!   at its velocity, as a feature's members drift (lean, 2026-11-01): no source places the
//!   centre's recent entries yet, and the lane that places them owns their law.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::features::members::source::drifted_at;
use crate::galaxy::imf::MassBand;
use crate::galaxy::motion::{MotionState, Regime, StateAt, Trajectory};
use crate::galaxy::placement::{ResolveSystemError, SystemRecord};
use crate::id::{CentreMemberId, MemberSlot, SystemId};
use crate::observe::Motion;
use crate::time::UniverseTime;
use crate::units::Seconds;

use super::events::victims::{CentreVictims, TidalDisruptionVictim};
use super::members::CentrePlacement;
use super::recent::recent_member;
use super::{CentreCellOrbits, CentreOrbitCache};

/// How one member of the galactic centre moves, as the centre's source places it (module
/// documentation).
///
/// # Examples
///
/// The black hole, at rest at the origin at every time:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::{CentrePath, NoCentreOrbitCache};
/// use hyperion_sim::galaxy::motion::StateAt;
/// use hyperion_sim::id::CentreMemberId;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let path = CentrePath::of(&galaxy, &NoCentreOrbitCache, CentreMemberId::CENTRAL_BLACK_HOLE)?;
/// let later = UniverseTime::from_julian_years(500).ok_or("in the clock window")?;
/// let StateAt::Exists(state) = path.state_at(later) else {
///     panic!("the black hole is a system at every time");
/// };
/// assert_eq!(state.position(), &GalacticPosition::ORIGIN);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CentrePath {
    record: SystemRecord,
    law: Law,
}

/// The law a [`CentrePath`] moves by.
#[derive(Debug, Clone, PartialEq)]
enum Law {
    /// The black hole: at the origin, at its velocity, at every time.
    AtRest(GalacticVelocity),
    /// A member of the centre's cells, on its trajectory.
    Cell(Box<Trajectory>),
    /// A tidal disruption's victim.
    Victim(Box<TidalDisruptionVictim>),
    /// A recent entry of the list, drifting at its velocity.
    Straight(GalacticVelocity),
}

impl CentrePath {
    /// The path of the centre member `id` in `galaxy`, a cell member's trajectory lent by
    /// `orbits` (module documentation): the same path whatever `orbits` holds.
    ///
    /// # Errors
    ///
    /// [`ResolveSystemError::NoSuchSystem`] where
    /// [`resolve_centre_member`](super::members::resolve_centre_member) gives no member.
    pub fn of(
        galaxy: &Galaxy,
        orbits: &dyn CentreOrbitCache,
        id: CentreMemberId,
    ) -> Result<Self, ResolveSystemError> {
        let model = galaxy
            .centre()
            .map_err(|_| ResolveSystemError::NoSuchSystem)?;
        let placement = CentrePlacement::new(model);
        match id.slot() {
            MemberSlot::FeatureLevel { index: 0 } => {
                let black_hole = placement.black_hole();
                Ok(Self {
                    record: *black_hole.record(),
                    law: Law::AtRest(black_hole.velocity()),
                })
            }
            // The victims first, then the recent entries, as the list numbers them; the
            // disruptions' model refuses only a black hole no galaxy the builder allows has.
            MemberSlot::FeatureLevel { index } => {
                if let Some(victim) = CentreVictims::one(galaxy, model, index).ok().flatten() {
                    return Ok(Self {
                        record: *victim.record(),
                        law: Law::Victim(Box::new(victim)),
                    });
                }
                let member =
                    recent_member(galaxy, model, index).ok_or(ResolveSystemError::NoSuchSystem)?;
                Ok(Self {
                    record: *member.record(),
                    law: Law::Straight(member.velocity()),
                })
            }
            MemberSlot::InCell {
                band, level, cell, ..
            } => {
                let band = MassBand::of_layer(band);
                let cell = placement
                    .grid()
                    .cell(level, cell)
                    .filter(|_| band.is_stellar())
                    .ok_or(ResolveSystemError::NoSuchSystem)?;
                let cell_orbits = orbits.orbits(galaxy, &placement, band, cell);
                let trajectory = trajectory_of(&cell_orbits, SystemId::from(id))
                    .ok_or(ResolveSystemError::NoSuchSystem)?;
                Ok(Self {
                    record: *trajectory.start().record(),
                    law: Law::Cell(Box::new(trajectory.clone())),
                })
            }
        }
    }

    /// The member's record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Where the member is at `t` and how it moves there, as the centre's source places its hit
    /// then, bit for bit; [`StateAt::NoSuchSystem`] for a victim after its disruption. Whether
    /// its star is born yet at `t` is its record's question ([`SystemRecord::existence_at`]).
    ///
    /// # Panics
    ///
    /// As [`Trajectory::state_at`] and
    /// [`TidalDisruptionVictim::state_at`], which never panic, and for a recent entry if its drift
    /// leaves the addressable range, which no member slower than light does within the source
    /// horizon.
    #[must_use]
    pub fn state_at(&self, t: UniverseTime) -> StateAt {
        match &self.law {
            Law::AtRest(velocity) => {
                StateAt::Exists(MotionState::new(GalacticPosition::ORIGIN, *velocity))
            }
            Law::Cell(trajectory) => trajectory.state_at(t),
            Law::Victim(victim) => victim.state_at(t),
            Law::Straight(velocity) => {
                let elapsed = Seconds::new(t.since_epoch().as_seconds_f64());
                StateAt::Exists(MotionState::new(
                    drifted_at(&self.record, *velocity, t, elapsed),
                    *velocity,
                ))
            }
        }
    }

    /// Its state at `t`, a victim's at its disruption from then on: what an observer reads of it,
    /// whose light left it no later than the observer's present.
    #[must_use]
    fn state_or_last(&self, t: UniverseTime) -> MotionState {
        match self.state_at(t) {
            StateAt::Exists(state) => state,
            StateAt::NoSuchSystem => {
                let Law::Victim(victim) = &self.law else {
                    unreachable!("a path is built only for a member that is a system at some time")
                };
                match victim.state_at(victim.time()) {
                    StateAt::Exists(state) => state,
                    StateAt::NoSuchSystem => {
                        unreachable!("a victim is a system at its disruption")
                    }
                }
            }
        }
    }
}

/// `id`'s trajectory among a cell's, if the cell holds it. The loss cone takes no centre member's
/// orbit: the marks drew them clear of it (P09.T28.b.a), so every trajectory is a system's.
fn trajectory_of(orbits: &CentreCellOrbits, id: SystemId) -> Option<&Trajectory> {
    orbits
        .trajectories()
        .iter()
        .find(|trajectory| trajectory.start().record().id() == id)
}

/// A centre member as a source of light (plan 12): its state at each time, a victim's at its
/// disruption from then on, which no light an observer receives while it is a system left it
/// after; an orbit in the Kepler regime and a victim's parabola are followed, so their stated
/// curvature error is zero.
impl crate::observe::Trajectory for CentrePath {
    fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        *self.state_or_last(t).position()
    }

    fn velocity_at(&self, t: UniverseTime) -> GalacticVelocity {
        self.state_or_last(t).velocity()
    }

    fn motion(&self) -> Motion {
        match &self.law {
            Law::Cell(trajectory) if trajectory.regime() == Regime::Kepler => Motion::Followed,
            Law::Victim(_) => Motion::Followed,
            Law::Cell(_) | Law::AtRest(_) | Law::Straight(_) => Motion::Drift,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::centre::events::victims::CentreVictims;
    use crate::galaxy::features::centre::members::resolve_centre_member;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::features::centre::{CentreMemberSource, KeepCentreOrbits};
    use crate::galaxy::query::{LayerSet, QuerySphere, SystemSource};
    use crate::id::Layer;
    use crate::units::LightYears;

    /// Every stellar layer.
    fn stellar() -> LayerSet {
        [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
            .into_iter()
            .fold(LayerSet::EMPTY, LayerSet::with)
    }

    /// The bits of a position, which tell two points apart bit for bit.
    fn bits(position: &GalacticPosition) -> (crate::coords::LyCell, [u64; 3]) {
        (
            position.cell(),
            position.offset_metres().map(hyperion_testkit::float::bits),
        )
    }

    /// The source's hits within 0.05 ly of the black hole at the epoch and a year on are placed by
    /// their IDs where the source put them, bit for bit, through a cache that keeps the cells and
    /// through none; each member's path is its resolved record's.
    #[test]
    fn a_centre_member_s_path_places_it_where_the_source_did() {
        let galaxy = milky_way_galaxy();
        let kept = KeepCentreOrbits::new(galaxy);
        let source = CentreMemberSource::new(galaxy, &kept).expect("the centre builds");
        let mut checked = 0;
        for years in [0, 1] {
            let t = UniverseTime::from_julian_years(years).expect("in the clock window");
            let sphere = QuerySphere::new(
                GalacticPosition::ORIGIN,
                LightYears::new(0.05),
                t,
                LightYears::ZERO,
            )
            .expect("a positive radius");
            let mut hits = Vec::new();
            source.systems_in_sphere(galaxy, &sphere, stellar(), &mut hits);
            assert!(hits.len() > 10, "{} hits", hits.len());
            for hit in hits.iter().take(40) {
                let crate::id::SystemIdKind::Centre(id) = hit.id().kind() else {
                    panic!("a centre ID");
                };
                for orbits in [
                    &kept as &dyn CentreOrbitCache,
                    &super::super::NoCentreOrbitCache,
                ] {
                    let path = CentrePath::of(galaxy, orbits, id).expect("a hit's member");
                    assert_eq!(path.record(), hit.record());
                    assert_eq!(
                        path.record(),
                        resolve_centre_member(galaxy, id)
                            .expect("a hit resolves")
                            .record()
                    );
                    let StateAt::Exists(state) = path.state_at(t) else {
                        panic!("a hit is a system at its time");
                    };
                    assert_eq!(bits(state.position()), bits(hit.position()), "{id:?}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 40);
    }

    /// A victim is on its parabola up to its disruption and no system after it, where an observer
    /// reads it at its disruption; the black hole is at rest at the origin.
    #[test]
    fn a_victim_is_a_system_until_its_disruption() {
        let galaxy = milky_way_galaxy();
        let victims = CentreVictims::of(galaxy).expect("the victims build");
        let victim = victims
            .victims()
            .first()
            .expect("the black hole has victims");
        let crate::id::SystemIdKind::Centre(id) = victim.record().id().kind() else {
            panic!("a centre ID");
        };
        let path = CentrePath::of(galaxy, &super::super::NoCentreOrbitCache, id)
            .expect("a victim's member");
        assert_eq!(path.record(), victim.record());
        let decade = crate::time::Span::from_julian_years(10).expect("a span");
        let before = victim.time().checked_sub(decade).expect("on the clock");
        assert_eq!(path.state_at(before), victim.state_at(before));
        let after = victim.time().checked_add(decade).expect("on the clock");
        assert_eq!(path.state_at(after), StateAt::NoSuchSystem);
        let StateAt::Exists(at_disruption) = victim.state_at(victim.time()) else {
            panic!("a victim is a system at its disruption");
        };
        assert_eq!(
            crate::observe::Trajectory::position_at(&path, after),
            *at_disruption.position()
        );
        assert_eq!(crate::observe::Trajectory::motion(&path), Motion::Followed);
        let hole = CentrePath::of(
            galaxy,
            &super::super::NoCentreOrbitCache,
            CentreMemberId::CENTRAL_BLACK_HOLE,
        )
        .expect("the black hole");
        let StateAt::Exists(state) = hole.state_at(after) else {
            panic!("the black hole is a system at every time");
        };
        assert_eq!(state.position(), &GalacticPosition::ORIGIN);
    }
}
