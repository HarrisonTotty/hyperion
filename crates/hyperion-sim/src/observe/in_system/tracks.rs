//! The tracks of a system's bodies and stars through its frame, for [`retarded_in_system`]
//! (rendering plan R03, Design note 8).
//!
//! Positions come from plan 14's [`PlanetarySystem::position_at`] and plan 11's
//! [`star_positions_at`], bit for bit; velocities from [`PlanetarySystem::state_at`] and
//! [`star_states_at`], which share their arithmetic.
//!
//! [`retarded_in_system`]: super::retarded_in_system

use crate::coords::{SystemPosition, SystemVelocity};
use crate::id::BodyId;
use crate::planetary::{BodyIndex, PlanetarySystem, ResolveBodyError, SystemContext};
use crate::stellar::multiplicity::{SystemHierarchy, star_positions_at, star_states_at};
use crate::time::UniverseTime;

use super::SystemTrajectory;

/// A body's track through its system: where plan 14 places it at any time, `None` while it is not
/// present (not yet formed, destroyed or unbound) and for a belt or the halo, which have no single
/// position.
///
/// # Examples
///
/// Where a ship at the barycentre sees a system's first body present at the epoch.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{SystemPosition, SystemVelocity};
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::observe::{BodyTrack, SystemObserver, SystemTrajectory, retarded_in_system};
/// use hyperion_sim::planetary::{self, SystemContext};
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(14));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let now = UniverseTime::EPOCH;
/// let ship = SystemObserver::new(SystemPosition::ORIGIN, SystemVelocity::ZERO, now)?;
/// let seen = cell.iter().find_map(|record| {
///     let ctx = SystemContext::for_system(&galaxy, record.id()).ok()?;
///     let system = planetary::generate(galaxy.seed(), &ctx);
///     let body = system.bodies().first()?;
///     let track = BodyTrack::new(&system, &ctx, body.index()).ok()?;
///     track.position_at(now)?;
///     retarded_in_system(&ship, &track).ok()
/// });
/// let seen = seen.expect("a body is present in the cell's systems");
/// // Its light left it before the ship's present.
/// assert!(seen.emitted() < now);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct BodyTrack<'a> {
    system: &'a PlanetarySystem,
    ctx: &'a SystemContext,
    index: BodyIndex,
}

impl<'a> BodyTrack<'a> {
    /// The track of body `index` of `system`, generated from `ctx`.
    ///
    /// # Errors
    ///
    /// [`ResolveBodyError::NoSuchBody`] if `system` holds no body `index`.
    ///
    /// # Panics
    ///
    /// If `ctx` is not the context `system` was generated from.
    pub fn new(
        system: &'a PlanetarySystem,
        ctx: &'a SystemContext,
        index: BodyIndex,
    ) -> Result<Self, ResolveBodyError> {
        system.position_at(ctx, index, UniverseTime::EPOCH)?;
        Ok(Self { system, ctx, index })
    }

    /// The body's index in its system.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }
}

impl SystemTrajectory for BodyTrack<'_> {
    fn position_at(&self, t: UniverseTime) -> Option<SystemPosition> {
        self.system
            .position_at(self.ctx, self.index, t)
            .expect("the body was resolved when the track was built")
    }

    fn velocity_at(&self, t: UniverseTime) -> Option<SystemVelocity> {
        self.system
            .state_at(self.ctx, self.index, t)
            .expect("the body was resolved when the track was built")
            .map(|(_, velocity)| velocity)
    }
}

/// A star's track through its system: where plan 11's hierarchy places it at any time, before
/// and after the epoch.
///
/// A star is always present: a remnant stays where its progenitor's orbit puts it (plan 11's
/// positions do not model a supernova's kick on the hierarchy).
#[derive(Debug, Clone, Copy)]
pub struct StarTrack<'a> {
    hierarchy: &'a SystemHierarchy,
    star: BodyId,
}

impl<'a> StarTrack<'a> {
    /// The track of `star` in `hierarchy`, or `None` if the hierarchy holds no such star.
    #[must_use]
    pub fn new(hierarchy: &'a SystemHierarchy, star: BodyId) -> Option<Self> {
        hierarchy
            .stars()
            .iter()
            .any(|slot| slot.body() == star)
            .then_some(Self { hierarchy, star })
    }

    /// The star's body ID.
    #[must_use]
    pub const fn star(&self) -> BodyId {
        self.star
    }
}

impl SystemTrajectory for StarTrack<'_> {
    fn position_at(&self, t: UniverseTime) -> Option<SystemPosition> {
        let mut positions = Vec::new();
        star_positions_at(self.hierarchy, t, &mut positions);
        let (_, at) = positions
            .into_iter()
            .find(|(body, _)| *body == self.star)
            .expect("the star was found in the hierarchy when the track was built");
        Some(at)
    }

    fn velocity_at(&self, t: UniverseTime) -> Option<SystemVelocity> {
        let mut states = Vec::new();
        star_states_at(self.hierarchy, t, &mut states);
        let (_, _, velocity) = states
            .into_iter()
            .find(|(body, _, _)| *body == self.star)
            .expect("the star was found in the hierarchy when the track was built");
        Some(velocity)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::planetary::placement::OrbitHost;
    use crate::planetary::system::tests::whole;
    use crate::time::{ClockWindow, Span};

    /// `n` times spread over the clock window by a fixed-seed generator.
    fn random_times(n: usize, seed: u64) -> Vec<UniverseTime> {
        let mut lcg = Lcg::new(seed);
        let span = ClockWindow::END
            .checked_since(ClockWindow::START)
            .expect("the window is on the clock")
            .seconds();
        (0..n)
            .map(|_| {
                let seconds = i64::try_from(lcg.next_below(u64::try_from(span).unwrap())).unwrap();
                let nanos = u32::try_from(lcg.next_below(1_000_000_000)).unwrap();
                ClockWindow::START
                    .checked_add(Span::new(seconds, nanos).unwrap())
                    .unwrap()
            })
            .collect()
    }

    fn assert_same_position(a: Option<SystemPosition>, b: Option<SystemPosition>) {
        assert_eq!(a.is_some(), b.is_some());
        if let (Some(a), Some(b)) = (a, b) {
            for (x, y) in a.metres().into_iter().zip(b.metres()) {
                assert_same_bits(x, y);
            }
        }
    }

    #[test]
    fn a_body_track_is_position_at_bit_for_bit_at_a_thousand_times() {
        let (ctx, system) = whole()
            .iter()
            .find(|(_, system)| system.bodies().len() > 5)
            .expect("the sample holds a system with several bodies");
        let tracks: Vec<BodyTrack<'_>> = system
            .bodies()
            .iter()
            .map(|body| BodyTrack::new(system, ctx, body.index()).unwrap())
            .collect();
        for t in random_times(1_000, 0x0003_0003) {
            for track in &tracks {
                assert_same_position(
                    track.position_at(t),
                    system.position_at(ctx, track.index(), t).unwrap(),
                );
            }
        }
    }

    #[test]
    fn a_moon_s_track_is_its_planet_s_plus_its_offset() {
        let mut moons = 0;
        for (ctx, system) in whole().iter().take(100) {
            for body in system.bodies() {
                let OrbitHost::Body(parent) = body.host() else {
                    continue;
                };
                let moon = BodyTrack::new(system, ctx, body.index()).unwrap();
                let planet = BodyTrack::new(system, ctx, parent).unwrap();
                let t = UniverseTime::EPOCH;
                let (Some(at), Some(centre)) = (moon.position_at(t), planet.position_at(t)) else {
                    continue;
                };
                let record = system.body_at(ctx, body.index(), t).unwrap();
                let Some(orbit) = record.orbit().ok() else {
                    continue;
                };
                let expected = centre.translated(orbit.trajectory().relative_state_at(t).0);
                assert_same_position(Some(at), Some(expected));
                moons += 1;
            }
        }
        assert!(moons > 10, "{moons} moons");
    }

    #[test]
    fn a_star_track_is_star_positions_at_bit_for_bit() {
        let mut stars = 0;
        let mut positions = Vec::new();
        for (ctx, _) in whole().iter().take(100) {
            let h = ctx.hierarchy();
            for t in random_times(10, 0x0003_0004) {
                star_positions_at(h, t, &mut positions);
                for (body, at) in &positions {
                    let track = StarTrack::new(h, *body).unwrap();
                    assert_same_position(track.position_at(t), Some(*at));
                    assert!(track.velocity_at(t).is_some());
                    stars += 1;
                }
            }
        }
        assert!(stars > 1_000, "{stars} star readings");
    }

    #[test]
    fn a_track_of_what_is_not_there_is_refused() {
        let (ctx, system) = &whole()[0];
        // The primary's index is the star's, never a body of the planetary system.
        assert_eq!(
            BodyTrack::new(system, ctx, BodyIndex::PRIMARY).unwrap_err(),
            ResolveBodyError::NoSuchBody
        );
        let (other, _) = whole()
            .iter()
            .find(|(other, _)| other.id() != ctx.id())
            .unwrap();
        let foreign = other.hierarchy().stars()[0].body();
        assert!(StarTrack::new(ctx.hierarchy(), foreign).is_none());
    }
}
