//! P14.T55.b.a's evaporation in a generated system, under every 28 hold: a belt whose named
//! members evaporate loses their share of its mass and shine, and ends with the last of them; a
//! body's record publishes the mass its rock has left.

use std::sync::OnceLock;

use super::tests::SEED;
use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::rules::GalaxyRules;
use crate::planetary::fate::evaporation::{EvaporationRules, with_evaporation};
use crate::planetary::testing::{
    SampleFilter, sample_records, with_every_27_hold, with_every_28_hold,
};

/// The ordinary census's galaxy (`census_tests`' `tallies_under_every_28_hold`'s) and its first
/// 836 systems, by rank: the census's ranks up to #835.
fn census() -> &'static (Galaxy, Vec<SystemRecord>) {
    static CENSUS: OnceLock<(Galaxy, Vec<SystemRecord>)> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let galaxy = Galaxy::from_params_with_rules(
            SEED,
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way's parameters build a galaxy");
        let records = sample_records(&galaxy, 836, SampleFilter::ALL)
            .expect("the solar circle holds the census's systems");
        (galaxy, records)
    })
}

/// `run` on census system #`rank`, generated under the rules in force, with its context.
fn on_census_system<R>(rank: usize, run: impl FnOnce(&SystemContext, &PlanetarySystem) -> R) -> R {
    let (galaxy, records) = census();
    let ctx = SystemContext::from_record(galaxy, &records[rank]);
    let system = generate(SEED, &ctx);
    run(&ctx, &system)
}

/// The belt of `system` whose members include the body of ID `id`.
fn belt_holding<'s>(system: &'s PlanetarySystem, id: &str) -> &'s Belt {
    system
        .belts
        .iter()
        .find(|belt| {
            belt.members()
                .iter()
                .any(|member| member.index().body_id(system.system).to_string() == id)
        })
        .expect("a belt holds the member")
}

/// The states at `t` that `snapshot` publishes for the members of `belt`.
fn member_states(snapshot: &SystemSnapshot, belt: &Belt) -> Vec<BodyState> {
    belt.members()
        .iter()
        .map(|member| {
            snapshot
                .body(member.index())
                .expect("a snapshot holds every member")
                .identity()
                .state()
        })
        .collect()
}

/// Census #14's inner belt (`41ffecb200000005.e1xx`): under every 28 hold all its eight named
/// members evaporate at 2.31 Gyr, and so the belt ends then, `Destroyed { Evaporated }` at the last
/// of their times; under every 27 hold it is present at the epoch, as every member is.
#[test]
fn a_belt_whose_named_members_all_evaporate_ends_with_the_last_of_them() {
    let (states, belt_state) = with_every_28_hold(|| {
        on_census_system(14, |ctx, system| {
            let belt = belt_holding(system, "41ffecb200000005.e101");
            let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
            let belt_state = snapshot
                .body(belt.index())
                .expect("a snapshot holds its belts")
                .identity()
                .state();
            (member_states(&snapshot, belt), belt_state)
        })
    });
    let times: Vec<UniverseTime> = states
        .iter()
        .map(|state| match state {
            BodyState::Destroyed {
                cause: DestructionCause::Evaporated,
                at,
            } => *at,
            other => panic!("every member of the belt evaporates: {other:?}"),
        })
        .collect();
    let last = *times.iter().max().expect("the belt has members");
    assert_eq!(
        belt_state,
        BodyState::Destroyed {
            cause: DestructionCause::Evaporated,
            at: last
        }
    );
    let kept = with_every_27_hold(|| {
        on_census_system(14, |ctx, system| {
            let belt = belt_holding(system, "41ffecb200000005.e101");
            system
                .snapshot_at(ctx, UniverseTime::EPOCH)
                .body(belt.index())
                .expect("a snapshot holds its belts")
                .identity()
                .state()
        })
    });
    assert_eq!(kept, BodyState::Present);
}

/// The share of `belt`'s named members' mass that `snapshot` publishes gone: all of each one
/// evaporated, and what each present one has lost.
fn share_gone(snapshot: &SystemSnapshot, belt: &Belt) -> f64 {
    let (mut formed, mut lost) = (0.0, 0.0);
    for member in belt.members() {
        let record = snapshot
            .body(member.index())
            .expect("a snapshot holds every member");
        let mass = member.mass().value();
        formed += mass;
        lost += match record.identity().state() {
            BodyState::Destroyed {
                cause: DestructionCause::Evaporated,
                ..
            } => mass,
            BodyState::Present => mass - record.mass().ok().expect("a present mass").value(),
            BodyState::NotYetFormed | BodyState::Destroyed { .. } | BodyState::Unbound { .. } => {
                0.0
            }
        };
    }
    lost / formed
}

/// The mass and the dust's shine of `belt`'s record in `snapshot`, which holds it present.
fn mass_and_shine(snapshot: &SystemSnapshot, belt: &Belt) -> (f64, f64) {
    let record = snapshot
        .body(belt.index())
        .expect("a snapshot holds its belts");
    assert_eq!(record.identity().state(), BodyState::Present);
    let Some(Population::Belt(population)) = record.population().ok() else {
        panic!("a present belt has its population");
    };
    (
        record.mass().ok().expect("a present belt's mass").value(),
        population.fractional_luminosity(),
    )
}

/// Census #14's outer belt (`41ffecb200000005.e2xx`) is partly evaporated at the epoch under
/// every 28 hold, two of its members gone: its mass and its dust's shine are the same belt's with
/// evaporation set back, less the share of its members' mass gone.
#[test]
fn a_belt_partly_evaporated_loses_its_members_share_of_its_mass_and_shine() {
    let ((mass, shine, share), (kept_mass, kept_shine)) = with_every_28_hold(|| {
        on_census_system(14, |ctx, system| {
            let belt = belt_holding(system, "41ffecb200000005.e201");
            let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
            let (mass, shine) = mass_and_shine(&snapshot, belt);
            let share = share_gone(&snapshot, belt);
            let kept = with_evaporation(EvaporationRules::Kept, || {
                mass_and_shine(&system.snapshot_at(ctx, UniverseTime::EPOCH), belt)
            });
            ((mass, shine, share), kept)
        })
    });
    assert!((0.0..1.0).contains(&share) && share > 0.0, "{share}");
    assert!(
        (mass / (kept_mass * (1.0 - share)) - 1.0).abs() < 1e-9,
        "{mass} M⊕ against {kept_mass} less {share}"
    );
    assert!(
        (shine / (kept_shine * (1.0 - share)) - 1.0).abs() < 1e-9,
        "{shine} against {kept_shine} less {share}"
    );
}

/// The record of census #14's member `e101` when its history has taken half its mass, under every
/// 28 hold, some tens of thousands of years before its rock is gone, after its host brightens:
/// present, with the mass its history leaves it, half the mass it formed with.
#[test]
fn a_member_near_the_end_of_its_life_publishes_the_mass_left() {
    let (formed, left, history) = with_every_28_hold(|| {
        on_census_system(14, |ctx, system| {
            let belt = belt_holding(system, "41ffecb200000005.e101");
            let member = belt.members()[0];
            let index = member.index();
            let body = system.body(index).expect("a member is a body");
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let Some(BodyState::Destroyed {
                cause: DestructionCause::Evaporated,
                at: gone,
            }) = fate.ending()
            else {
                panic!("e101 evaporates: {:?}", fate.ending());
            };
            // The time its history has taken half its mass, bisected to the second from its
            // formation to when it is gone.
            let half = member.mass().value() / 2.0;
            let (mut lo, mut hi) = (fate.formed_at().expect("formed"), gone);
            while hi.checked_since(lo).expect("a span").as_seconds_f64() > 1.0 {
                let mid = lo
                    .checked_add(
                        Span::from_seconds_f64(
                            0.5 * hi.checked_since(lo).expect("a span").as_seconds_f64(),
                        )
                        .expect("a span"),
                    )
                    .expect("on the clock");
                if fate.at(mid).mass().value() > half {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let record = system.body_at(ctx, index, lo).unwrap();
            assert_eq!(record.identity().state(), BodyState::Present);
            (
                member.mass(),
                *record.mass().ok().expect("a present mass"),
                fate.at(lo).mass(),
            )
        })
    });
    assert!(
        (left.value() / formed.value() - 0.5).abs() < 0.01,
        "{left:?} against {formed:?}"
    );
    assert!(
        (left.value() / history.value() - 1.0).abs() < 1e-9,
        "{left:?} against its history's {history:?}"
    );
}
