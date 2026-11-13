//! P14.T55.b.a's evaporation in a generated system, from generator version 28: a belt whose named
//! members evaporate loses their share of its mass and shine, and ends with the last of them; a
//! body's record publishes the mass its rock has left; and from 29 a moon's bulk and a belt's
//! largest body read it.

use std::sync::OnceLock;

use super::tests::SEED;
use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::rules::GalaxyRules;
use crate::planetary::testing::{SampleFilter, sample_records};

/// The ordinary census's galaxy (`census_tests`' `tallies_under_every_28_hold`'s) and its first
/// 1,257 systems, by rank: the census's ranks up to #1,256.
fn census() -> &'static (Galaxy, Vec<SystemRecord>) {
    static CENSUS: OnceLock<(Galaxy, Vec<SystemRecord>)> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let galaxy = Galaxy::from_params_with_rules(
            SEED,
            GalaxyParams::milky_way_like(),
            GalaxyRules::of_version(crate::GENERATOR_VERSION),
        )
        .expect("the Milky Way's parameters build a galaxy");
        let records = sample_records(&galaxy, 1_257, SampleFilter::ALL)
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
/// of their times.
#[test]
fn a_belt_whose_named_members_all_evaporate_ends_with_the_last_of_them() {
    let (states, belt_state) = {
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
    };
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
/// every 28 hold, two of its members gone: its mass and its dust's shine are the belt's own at the
/// epoch (its collisional mass and its dust's shine by its host's light then), less the share of
/// its members' mass gone.
#[test]
fn a_belt_partly_evaporated_loses_its_members_share_of_its_mass_and_shine() {
    let ((mass, shine, share), (kept_mass, kept_shine)) = {
        on_census_system(14, |ctx, system| {
            let belt = belt_holding(system, "41ffecb200000005.e201");
            let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
            let (mass, shine) = mass_and_shine(&snapshot, belt);
            let share = share_gone(&snapshot, belt);
            let age = ctx.age_at(UniverseTime::EPOCH);
            let zone = system
                .zone(belt.host())
                .expect("a belt's host is one of its system's zones");
            let light = Epoch::new(system, ctx, UniverseTime::EPOCH)
                .luminosity(zone)
                .expect("the belt's host shines at the epoch");
            let kept = (
                belt.mass_at(age).value(),
                belt.fractional_luminosity(age, light),
            );
            ((mass, shine, share), kept)
        })
    };
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
    let (formed, left, history) = {
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
    };
    assert!(
        (left.value() / formed.value() - 0.5).abs() < 0.01,
        "{left:?} against {formed:?}"
    );
    assert!(
        (left.value() / history.value() - 1.0).abs() < 1e-9,
        "{left:?} against its history's {history:?}"
    );
}

/// The record at the epoch of the moon of ID `id` of census #`rank`, and the mass it formed with.
fn moon_at_the_epoch(rank: usize, id: &str) -> (BodyRecord, EarthMasses) {
    on_census_system(rank, |ctx, system| {
        let body = system
            .bodies()
            .iter()
            .find(|body| body.index.body_id(system.system).to_string() == id)
            .expect("the census system holds the moon");
        assert!(matches!(body.part, Part::Moon(_)), "{id} is a moon");
        let record = system
            .snapshot_at(ctx, UniverseTime::EPOCH)
            .body(body.index)
            .expect("a snapshot holds every moon")
            .clone();
        assert_eq!(record.identity().state(), BodyState::Present);
        (record, body.mass())
    })
}

/// A record's mass over the volume of its bulk's radius.
fn mass_over_volume(record: &BodyRecord) -> KilogramsPerCubicMetre {
    let mass = record.mass().ok().expect("a present mass");
    let bulk = record.bulk().ok().expect("a present moon's bulk");
    let r = Metres::from(bulk.radius()).value();
    KilogramsPerCubicMetre::new(
        Kilograms::from(*mass).value() / (4.0 / 3.0 * core::f64::consts::PI * r * r * r),
    )
}

/// A partly evaporated moon's density is its mass over its volume: census #341's capture `0303`,
/// three quarters of its rock gone at the epoch, census #6's giant-impact moon `0101` and census
/// #1,256's regular moon `0101`, each a little under 1% gone, publish their bulk at their mass
/// left, its surface gravity and class its mass left's and its figure on its bulk's radius.
#[test]
fn a_partly_evaporated_moon_s_density_is_its_mass_over_its_volume() {
    for (rank, id, least) in [
        (341, "42002cadffc00003.0303", 0.7),
        (6, "61ffd9680000000c.0101", 0.003),
        (1_256, "42002cb5ffc00004.0101", 0.009),
    ] {
        let (left, formed) = moon_at_the_epoch(rank, id);
        let mass = left.mass().ok().expect("a present mass").value();
        let share = mass / formed.value();
        assert!(1.0 - share > least, "{id}: {share} of its mass left");
        let bulk = left.bulk().ok().expect("a present moon's bulk");
        let density = bulk.density().value();
        assert!(
            (density / mass_over_volume(&left).value() - 1.0).abs() < 1e-9,
            "{id}: {density} kg/m³ against its mass over its volume, {:?}",
            mass_over_volume(&left)
        );
        let r = Metres::from(bulk.radius()).value();
        let gravity = GM_EARTH * mass / (r * r);
        assert!(
            (bulk.surface_gravity().value() / gravity - 1.0).abs() < 1e-9,
            "{id}: its gravity is its mass left's, {:?} against {gravity}",
            bulk.surface_gravity()
        );
        assert_eq!(
            bulk.class(),
            PlanetClass::of(EarthMasses::new(mass), &bulk.fractions()),
            "{id}"
        );
        // Its figure is on its bulk's radius, the one at its mass left.
        let figure = left.figure().ok().expect("a present moon's figure");
        let mean = figure.spheroid().volumetric_radius_m();
        assert!(
            (mean / r - 1.0).abs() < 1e-12,
            "{id}: {mean} m against {r} m"
        );
    }
}

/// A partly evaporated moon's every reader reads it at its mass left, census #341's moons: its
/// surface hooks' bulk is its record's, its record's figure is its body's figure at its published
/// bulk and mass, and its rotation law at [`parent_time`] is the one its rounds hold from its
/// derivation at its mass left then, as `moon_laws_are_held_after_at_most_two_rounds` checks it
/// (round 1's record is round 1's derivation and gives its law, to the bit).
#[test]
fn a_partly_evaporated_moon_s_readers_read_its_mass_left() {
    on_census_system(341, |ctx, system| {
        let t = parent_time(ctx);
        assert_eq!(t, UniverseTime::EPOCH);
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        let mut lost = 0;
        for body in system.bodies() {
            let (Part::Moon(moon), OrbitHost::Body(parent)) = (&body.part, body.host) else {
                continue;
            };
            let index = body.index();
            let record = system.body_at(ctx, index, t).unwrap();
            let Some(mass) = record.mass().ok().copied() else {
                continue;
            };
            let Section::Ok(bulk) = record.bulk() else {
                continue;
            };
            if mass.value() < body.mass().value() {
                lost += 1;
            }
            let hooks = system
                .hooks_at(ctx, index, t)
                .unwrap()
                .expect("a moon's hooks");
            let Section::Ok(composition) = hooks.bulk() else {
                panic!("{index:?}: a present moon's hooks hold its bulk");
            };
            assert_eq!(composition.fractions(), bulk.fractions(), "{index:?}");
            assert_eq!(
                record.figure(),
                &body.figure_section(Some(bulk), mass, t),
                "{index:?}"
            );
            let primary = system.body(parent).unwrap();
            let host = fate_host(ctx, system.zone_of(primary));
            let fate = system.fate_of(primary, &host);
            let now = system.parent_now(&epoch, &cosines, parent, &fate);
            let Some(held) = system.moon_law(&epoch, &cosines, body, moon, (&now, &fate)) else {
                continue;
            };
            assert_eq!(system.rotation_of(ctx, index).unwrap(), Some(held.law));
            let left = moon.at_mass(mass);
            let derived = system.record_moon(&epoch, body, (&now, &fate)).unwrap();
            assert_eq!(derived.body().mass(), mass, "{index:?}");
            let orbit = moon.satellite.orbit_at(&moon.parent, ctx.age_at(t));
            let spin_bulk = SpinBulk::from(&PlanetarySystem::moon_bulk_of(&left, &derived, &now));
            let record_law = system
                .rotation_from(ctx, body, &orbit, spin_bulk, mass, derived.spin_tides())
                .unwrap();
            if held.round == MoonRound::First {
                let case = MoonNow {
                    body,
                    moon: &left,
                    orbit,
                    parent: &now,
                    fate: &fate,
                };
                let round_one = system
                    .moon_derived_with(
                        &epoch,
                        &cosines,
                        &case,
                        Some(held.locked),
                        PassDriver::Solve,
                    )
                    .unwrap();
                assert_eq!(derived, round_one, "{index:?}");
                assert_eq!(record_law, held.law, "{index:?}");
            }
        }
        assert!(lost >= 3, "{lost} of census #341's moons lost mass");
    });
}

/// The largest body census #14's third belt (`41ffecb200000005.e3xx`) publishes at the epoch,
/// beside its largest member left as its members' records state them (each present one's diameter
/// shrunk by the cube root of its published mass's share of its formed one, each gone one's none)
/// and its largest as formed.
fn largest_of_the_third_belt() -> (Metres, Metres, Metres) {
    on_census_system(14, |ctx, system| {
        let belt = belt_holding(system, "41ffecb200000005.e301");
        let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
        let record = snapshot
            .body(belt.index())
            .expect("a snapshot holds its belts");
        let Some(Population::Belt(population)) = record.population().ok() else {
            panic!("a present belt has its population");
        };
        let left = belt
            .members()
            .iter()
            .map(|member| {
                let record = snapshot
                    .body(member.index())
                    .expect("a snapshot holds every member");
                match record.identity().state() {
                    BodyState::Present => {
                        let mass = record.mass().ok().expect("a present mass").value();
                        member.diameter().value() * math::cbrt(mass / member.mass().value())
                    }
                    BodyState::Destroyed {
                        cause: DestructionCause::Evaporated,
                        ..
                    } => 0.0,
                    BodyState::NotYetFormed
                    | BodyState::Destroyed { .. }
                    | BodyState::Unbound { .. } => member.diameter().value(),
                }
            })
            .fold(0.0, f64::max);
        (
            population.largest_diameter(),
            Metres::new(left),
            belt.largest_diameter(),
        )
    })
}

/// A partly evaporated belt's largest body is its largest member left: census #14's third belt,
/// each of its five members partly evaporated at the epoch, its largest by about 1%, publishes its
/// largest member's diameter left, as that member's own mass states it, below the diameter its
/// largest member formed with.
#[test]
fn a_partly_evaporated_belt_s_largest_body_is_its_largest_member_left() {
    let (largest, left, formed) = largest_of_the_third_belt();
    assert!(
        (largest.value() / left.value() - 1.0).abs() < 1e-12,
        "{largest:?} against its largest member left, {left:?}"
    );
    let shrunk = largest.value() / formed.value();
    assert!((0.99..1.0).contains(&shrunk), "{shrunk}");
}
