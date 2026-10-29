//! Where a system a client names is (plan 07, P07.T10.c): the one place the server places a system
//! it resolved from a client's ID, routed by the record's origin.
//!
//! Plan 03's `position_at` and plan 08's `epoch_velocity` are a grid record's. In a galaxy with
//! kinematic tables, which every production galaxy has, they panic for any other record, the first
//! away from the epoch and the second at every time (plan 03's Risks, "`position_at` and feature
//! members"), so every request that places a system from a client's ID places it here:
//! `resolve_system`, `extinction`'s system targets, `scene_ship`'s frame, and the scene's core
//! after it.

use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::galaxy::query::{epoch_velocity, position_at};
use hyperion_sim::observe::{Drift, TraceMotionError, Trajectory};
use hyperion_sim::time::UniverseTime;

/// Where the system `record` is at `t`, its barycentre in the galactic frame, and its velocity
/// there, m/s along the galactic axes, by the record's origin.
///
/// - A grid record is where a range query finds it, bit for bit: [`position_at`], with plan 08's
///   [`epoch_velocity`] beside it, as a range row carries it.
/// - A feature member moves on its own line, its cluster's motion and its own, which plan 08's draw
///   is not: [`Drift::of_record`] (plan 09, P09.T10; plan 12, P12.T6).
/// - A member of the galactic centre (the black hole, a member of the centre's grid or a tidal
///   disruption's victim) is refused at every time, the epoch included, until plan 09's P09.T37 and
///   P09.T40 place them: they turn this arm on for every request together. Refused at the epoch
///   too, no answer depends on the time, which the scene's core relies on; and a victim is no
///   system at all after its disruption, which can precede the epoch (seed 0x4d2's nine were
///   disrupted 43,000–227,000 years before it), so its epoch position may be only formal.
/// - An origin the sim adds later is placed on the sim's own line for it, or refused by its error:
///   [`SystemOrigin`] is `#[non_exhaustive]`, so this `match` cannot name it, but
///   [`Drift::of_record`]'s own `match` does, and it never reaches a grid record's draw.
///
/// This is the one place the server places a system from a client's ID; [`system_place`] and the
/// range rows read grid records alone. Version 25's wiring of the Kepler regime changes the grid
/// arm: the velocity it reports becomes the orbit's at `t` (P09.T28.b.b's deferred finding (2)).
///
/// [`system_place`]: super::system_place
///
/// # Errors
///
/// [`TraceMotionError::CentreOrbitNotBuilt`] for a member of the galactic centre, at every time.
///
/// # Panics
///
/// If `record` is a feature member that does not resolve in `galaxy`, which only a record of
/// another galaxy can be ([`Drift::of_record`]).
pub(crate) fn system_motion(
    galaxy: &Galaxy,
    record: &SystemRecord,
    t: UniverseTime,
) -> Result<(GalacticPosition, GalacticVelocity), TraceMotionError> {
    match record.origin() {
        SystemOrigin::Grid(_) => Ok((
            position_at(galaxy, record, t),
            epoch_velocity(galaxy, record),
        )),
        #[expect(
            clippy::match_same_arms,
            reason = "`SystemOrigin` is `#[non_exhaustive]`, so the wildcard below cannot go; the \
                      feature member is named so that its line is this function's choice, not the \
                      fallback's"
        )]
        SystemOrigin::FeatureMember { .. } => on_its_line(galaxy, record, t),
        SystemOrigin::CentreMember { .. } => {
            Err(TraceMotionError::CentreOrbitNotBuilt(record.id()))
        }
        _ => on_its_line(galaxy, record, t),
    }
}

/// `record` on the line [`Drift::of_record`] gives it, at `t`.
fn on_its_line(
    galaxy: &Galaxy,
    record: &SystemRecord,
    t: UniverseTime,
) -> Result<(GalacticPosition, GalacticVelocity), TraceMotionError> {
    let line = Drift::of_record(galaxy, record)?;
    Ok((line.position_at(t), line.velocity()))
}

/// The systems of each origin in the server tests' galaxy, for the tests of what places them: this
/// module's, and the scene core's, which frames a ship on the feature member.
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
    use hyperion_sim::coords::LyCell;

    use super::origins::{a_feature_member, a_grid_system, galaxy, the_centre_s_members};
    use super::*;

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
            let (position, velocity) = system_motion(galaxy, &record, t).unwrap();
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
            let (position, velocity) = system_motion(galaxy, &record, t).unwrap();
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
                    system_motion(galaxy, &record, t),
                    Err(TraceMotionError::CentreOrbitNotBuilt(record.id())),
                    "{:?} at {t}",
                    record.id()
                );
            }
        }
    }
}
