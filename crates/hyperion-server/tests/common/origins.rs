//! A system of each origin in a server galaxy, shared by the integration tests that place a system
//! from a client's ID (plan 07, P07.T10.c; plan 12, P12.T6): a grid system, a catalogue feature's
//! member, and the galactic centre's members.

use std::sync::OnceLock;

use hyperion_protocol::UniverseTime;
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::centre::events::CentreVictims;
use hyperion_sim::galaxy::features::centre::members::CentrePlacement;
use hyperion_sim::galaxy::features::members::MemberRecord;
use hyperion_sim::id::{CentreMemberId, SystemId};
use hyperion_sim::units::LightYears;

/// The seed of every universe the origins' tests create, the other server tests' own.
pub const SEED: u64 = 0x4d2;

/// Seconds in a Julian year, the sim's year.
pub const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// A layer-C system at the solar circle, one of `system_summary`'s pinned systems.
pub const GRID_SYSTEM: u64 = 0x4200_2cb2_0000_0001;

/// The galaxy of [`SEED`] as the server builds it, with its kinematic tables, built once for the
/// binary: without them no feature has members, and plan 03's `position_at` panics for any record
/// but a grid record's away from the epoch.
pub fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)).with_full_potential())
}

/// The instant `years` Julian years from the epoch, as the wire carries it.
pub fn at_years(years: i64) -> UniverseTime {
    UniverseTime {
        seconds: years * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    }
}

/// A member of band C of the first feature with members within 3,000 ly of the Sun-like point.
pub fn a_feature_member(galaxy: &Galaxy) -> MemberRecord {
    use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
    use hyperion_sim::galaxy::features::members::FeatureInterior;
    use hyperion_sim::galaxy::imf::MassBand;

    let sun = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
        .expect("in the root cube");
    let interior = FeatureCatalogue::near(galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
        .find_map(|feature| FeatureInterior::of(galaxy, &feature))
        .expect("a feature with members lies within 3,000 ly of the Sun");
    let mut members = Vec::new();
    interior
        .grid()
        .owned_cells()
        .find_map(|cell| {
            interior.members_in_cell(galaxy, MassBand::C, cell, &mut members);
            members.first().map(|(member, _)| *member)
        })
        .expect("the feature has a member in band C")
}

/// The galactic centre's three kinds of member: the first member of its fullest band and cell,
/// the black hole, and the black hole's first tidal disruption victim.
pub fn the_centre_s_members(galaxy: &Galaxy) -> [SystemId; 3] {
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
}
