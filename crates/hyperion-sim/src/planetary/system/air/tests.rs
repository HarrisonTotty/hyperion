//! The air's sections on the record (P14.T54.b): held at 23, so that every record is 23's; under
//! the batch's rules, the structure of every surface with an air, and none for every airless one,
//! on the Solar System table and over the plan-14 sample (`system::tests::whole`).

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
use crate::planetary::hooks::figures::{VolatileHistory, table_figures};
use crate::planetary::hooks::structure::{
    DRY_LAPSE_FACTOR, METHANE_LAPSE_FACTOR, WATER_LAPSE_FACTOR, effective_temperature,
    internal_temperature, skin_temperature,
};
use crate::planetary::hooks::surface::regime::{ClimateInputs, surface_reservoirs};
use crate::planetary::hooks::surface::tests::table;
use crate::planetary::record::{BodyRecord, Section, Surface, SystemSnapshot};
use crate::planetary::system::RecordRules;
use crate::planetary::system::tests::{SEED, golden_systems, whole};
use crate::planetary::{PlanetarySystem, SystemContext, generate};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::Pascals;

/// How many of the sample's whole systems the record tests build under the batch's rules.
const AIR_SAMPLE: usize = 40;

/// The rules in force, with the air's sections under the batch's: the cold traps stay held, so
/// that only the air's hold is lifted.
fn air_batch() -> RecordRules {
    RecordRules {
        air: AirRules::Batch,
        ..RecordRules::in_force()
    }
}

/// The snapshot at the epoch of the system `system` of context `ctx` under the air's batch.
fn batch_snapshot(ctx: &SystemContext, system: &PlanetarySystem) -> SystemSnapshot {
    system
        .clone()
        .with_record_rules(air_batch())
        .snapshot_at(ctx, UniverseTime::EPOCH)
}

/// The first [`AIR_SAMPLE`] whole systems' snapshots at the epoch under the air's batch, built
/// once.
fn batch_snapshots() -> &'static [SystemSnapshot] {
    static SNAPSHOTS: OnceLock<Vec<SystemSnapshot>> = OnceLock::new();
    SNAPSHOTS.get_or_init(|| {
        whole()
            .iter()
            .take(AIR_SAMPLE)
            .map(|(ctx, system)| batch_snapshot(ctx, system))
            .collect()
    })
}

/// Whether `surface`'s conditions hold an air: a surface pressure above zero over some gas.
fn has_air(surface: &Surface) -> bool {
    let conditions = surface.conditions();
    conditions.surface_pressure().value() > 0.0 && !conditions.gases().is_empty()
}

/// The radiative-convective structure of `surface`, which must hold one.
fn profile_of(surface: &Surface) -> &RadiativeConvective {
    match surface.vertical_structure() {
        Modelled::Ok(Some(VerticalStructure::RadiativeConvective(profile))) => profile,
        other => panic!("no structure: {other:?}"),
    }
}

/// Asserts that `profile` starts at `record`'s surface conditions, that its skin is the one of its
/// bulk section's `T_eq` and its figures' heat flow (held at most `T_s`), and that its adiabat
/// cools aloft, every node's pressure and temperature below the last's.
fn check_profile(record: &BodyRecord, surface: &Surface, profile: &RadiativeConvective) {
    let id = record.identity().id();
    let conditions = surface.conditions();
    let t_s = conditions.mean_surface_temperature();
    assert_same_bits(profile.surface_temperature().value(), t_s.value());
    assert_same_bits(
        profile.surface_pressure().value(),
        conditions.surface_pressure().value(),
    );
    let Section::Ok(bulk) = record.bulk() else {
        panic!("{id}: a surface with no bulk section");
    };
    let skin = skin_temperature(effective_temperature(
        bulk.equilibrium_temperature(),
        internal_temperature(surface.figures().heat_flow()),
    ));
    let held = if skin > t_s { t_s } else { skin };
    assert_same_bits(profile.skin_temperature().value(), held.value());
    let nodes = profile.nodes();
    assert!(!nodes.is_empty(), "{id}");
    for pair in nodes.windows(2) {
        assert!(
            pair[1].pressure() < pair[0].pressure()
                && pair[1].temperature() < pair[0].temperature(),
            "{id}: {pair:?}"
        );
    }
    for node in nodes {
        assert!(
            node.pressure().value().is_finite() && node.temperature().value().is_finite(),
            "{id}: {node:?}"
        );
    }
}

/// The air's sections are held until P14.T54.b's bump (P14.T54.b.l; decision-p14-t54b-open §3.3):
/// below it the rules in force are the held ones, and under them every golden system's record, at
/// both times its golden pins, holds its vertical structure not modelled. The server's
/// `the_members_of_later_tasks_are_absent_and_a_moon_s_ocean_is_computed` holds the wire's member
/// absent on every golden body. P14.T54.b.b–d and .i add the aerosols, the modifiers and the
/// envelope to the same check.
#[test]
fn the_air_sections_are_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < AIR_SECTIONS_VERSION,
        "the 23 → 24 bump (P14.T54.b.l) puts the air's sections on the record: every surface's \
         vertical structure (P14.T24.e), `null` for one with no air, with the members P14.T54.b's \
         later subtasks hold here. Delete `AIR_SECTIONS_VERSION`, `AirRules` with its `Held` arm, \
         `record_structure`'s rules, `RecordRules::air` (and `RecordRules` with \
         `PlanetarySystem::with_record_rules` once the cold traps' hold goes too); the four source \
         scans with their `HELD` lists (`nothing_on_the_record_calls_the_inventory_yet`, \
         `nothing_on_the_record_calls_the_condensation_yet`, \
         `substance_aerosol_nothing_on_the_record_reads_the_extinction_yet`, \
         `modifiers_nothing_on_the_record_reads_the_modifiers_yet`); \
         `the_venus_test_is_rechecked_at_the_23_to_24_bump`, \
         `the_held_record_carries_no_structure` and this test. The server's \
         `the_members_of_later_tasks_are_absent_and_a_moon_s_ocean_is_computed` then takes the \
         structure as present, `null` on an airless surface (decision-p14-t54b-open §1.3, §3.3)"
    );
    assert_eq!(AirRules::in_force(), AirRules::Held);
    assert_eq!(RecordRules::in_force().air, AirRules::Held);
    let mut surfaces = 0_usize;
    for (name, ctx, system) in golden_systems() {
        for t in [UniverseTime::EPOCH, ClockWindow::END] {
            for record in system.snapshot_at(ctx, t).bodies() {
                let Section::Ok(surface) = record.surface() else {
                    continue;
                };
                assert_eq!(
                    surface.vertical_structure(),
                    Modelled::NotModelled,
                    "{name} {}",
                    record.identity().id()
                );
                surfaces += 1;
            }
        }
    }
    assert!(surfaces > 20, "{surfaces} golden surfaces");
}

/// A body of the Solar System table, with what its record's structure reads and its structure
/// under the air's switch (the batch's rules).
struct TableAir {
    name: &'static str,
    /// Its conditions with their climate regime, as a record's surface holds them.
    conditions: SurfaceConditions,
    /// Its equilibrium temperature.
    equilibrium: Kelvin,
    /// Its figures' heat flow.
    heat_flow: WattsPerSquareMetre,
    /// Its structure under [`AirRules::Batch`].
    structure: Modelled<Option<VerticalStructure>>,
}

/// Every body of the Solar System table under the air's switch, its conditions given their regime
/// as `hooks::structure`'s tests give them.
fn table_under_the_switch() -> Vec<TableAir> {
    table()
        .into_iter()
        .map(|(name, body, conditions)| {
            let figures = table_figures(&body).expect("the table's bodies have figures");
            let published = figures
                .clone()
                .with_surface(&conditions, VolatileHistory::NONE);
            let reservoirs = surface_reservoirs(body.atmosphere());
            let conditions = conditions.with_climate(&ClimateInputs {
                mass: body.mass(),
                reservoirs: &reservoirs,
                from_colder: false,
                ice_fraction: published.ice_fraction(),
                ice_threshold: published.ice_threshold(),
            });
            let (equilibrium, heat_flow) = (body.equilibrium_temperature(), figures.heat_flow());
            let structure = record_structure(AirRules::Batch, &conditions, equilibrium, heat_flow);
            TableAir {
                name,
                conditions,
                equilibrium,
                heat_flow,
                structure,
            }
        })
        .collect()
}

/// On the Solar System table, under the air's switch, the record's structure is the derivation's
/// for each body with an air and none, `Ok(None)` (the wire's `null`), for each without, the Moon
/// among them; held, it is not modelled; a skin that cannot be (no light and no internal heat), and
/// conditions without their regime, are not modelled.
#[test]
fn the_record_s_structure_is_the_derivation_s_or_none_without_air() {
    let (mut airs, mut airless) = (Vec::new(), Vec::new());
    for air in table_under_the_switch() {
        let TableAir {
            name,
            ref conditions,
            equilibrium,
            heat_flow,
            ref structure,
        } = air;
        if conditions.surface_pressure().value() > 0.0 {
            let inputs = StructureInputs::of_surface(conditions, equilibrium, heat_flow).unwrap();
            let derived = RadiativeConvective::derive(&inputs).unwrap();
            assert_eq!(
                structure,
                &Modelled::Ok(Some(VerticalStructure::RadiativeConvective(derived))),
                "{name}"
            );
            // No light and no internal heat: a skin of 0 K, refused.
            let dark = record_structure(
                AirRules::Batch,
                conditions,
                Kelvin::new(0.0),
                WattsPerSquareMetre::new(0.0),
            );
            assert_eq!(dark, Modelled::NotModelled, "{name}");
            airs.push(name);
        } else {
            assert!(conditions.gases().is_empty(), "{name}");
            assert_eq!(structure, &Modelled::Ok(None), "{name}");
            airless.push(name);
        }
        assert_eq!(
            record_structure(AirRules::Held, conditions, equilibrium, heat_flow),
            Modelled::NotModelled,
            "{name}"
        );
        assert_eq!(
            record_structure(
                AirRules::Batch,
                &conditions.clone().without_climate(),
                equilibrium,
                heat_flow
            ),
            Modelled::NotModelled,
            "{name}"
        );
    }
    eprintln!("airs {airs:?}; airless {airless:?}");
    assert!(airless.contains(&"Moon"), "{airless:?}");
    assert!(
        airs.len() >= 4 && airless.len() >= 2,
        "{airs:?} {airless:?}"
    );
}

/// The table's Earth, Venus and Titan under the air's switch (P14.T54.b.a): each carries a
/// structure that cools aloft to a skin below its `T_eq`, with water's, the dry and methane's α;
/// and P14.T24.e's Venus test, re-checked on P14.T57.a's Venus: 300–370 K at 1 bar (356.4 K, the
/// golden `planetary/vertical_structure`'s). The figures are printed for the plan's record.
#[test]
fn the_table_s_earth_venus_and_titan_carry_their_structures_under_the_switch() {
    let table = table_under_the_switch();
    let lapse = [
        ("Earth", WATER_LAPSE_FACTOR),
        ("Venus", DRY_LAPSE_FACTOR),
        ("Titan", METHANE_LAPSE_FACTOR),
    ];
    for (name, alpha) in lapse {
        let air = table
            .iter()
            .find(|air| air.name == name)
            .unwrap_or_else(|| panic!("{name} is in the table"));
        let Modelled::Ok(Some(VerticalStructure::RadiativeConvective(profile))) = &air.structure
        else {
            panic!("{name}: {:?}", air.structure);
        };
        let (t_s, p_s) = (
            profile.surface_temperature().value(),
            profile.surface_pressure().value(),
        );
        let (skin, top) = (
            profile.skin_temperature().value(),
            profile.tropopause_pressure().value(),
        );
        let one_bar = profile.temperature_at(Pascals::new(1.0e5)).value();
        eprintln!(
            "{name}: T_s {t_s:.2} K at {:.4} bar, α {}, T_eq {:.2} K, T_skin {skin:.2} K, \
             tropopause {:.4} bar, {} segments, {one_bar:.2} K at 1 bar",
            p_s / 1e5,
            profile.lapse_factor(),
            air.equilibrium.value(),
            top / 1e5,
            profile.nodes().len() - 1
        );
        assert_same_bits(profile.lapse_factor(), alpha);
        assert!(
            skin < air.equilibrium.value() && skin < t_s,
            "{name}: {skin} K"
        );
        assert!(top < p_s, "{name}: a tropopause at its ground");
        for pair in profile.nodes().windows(2) {
            assert!(
                pair[1].temperature() < pair[0].temperature(),
                "{name}: {pair:?}"
            );
        }
        if name == "Venus" {
            assert!(
                (300.0..=370.0).contains(&one_bar),
                "Venus at 1 bar: {one_bar} K"
            );
        }
    }
}

/// The record at the version in force holds no structure, and the batch's surface is the held one
/// with its structure alone added: over the first ten whole systems, every surface held is the
/// batch's with its structure not modelled, so that every golden keeps 23's bits.
#[test]
fn the_held_record_carries_no_structure() {
    let mut surfaces = 0_usize;
    for ((ctx, system), batch) in whole().iter().zip(batch_snapshots()).take(10) {
        let held = system.snapshot_at(ctx, UniverseTime::EPOCH);
        assert_eq!(held.bodies().len(), batch.bodies().len());
        for (record, other) in held.bodies().iter().zip(batch.bodies()) {
            let id = record.identity().id();
            match (record.surface(), other.surface()) {
                (Section::Ok(held), Section::Ok(batch)) => {
                    assert_eq!(held.vertical_structure(), Modelled::NotModelled, "{id}");
                    let stripped = batch.clone().with_vertical_structure(Modelled::NotModelled);
                    assert_eq!(held, &stripped, "{id}");
                    surfaces += 1;
                }
                (held, batch) => assert_eq!(held, batch, "{id}"),
            }
        }
    }
    assert!(surfaces > 20, "{surfaces}");
}

/// Under the batch's rules, over the first [`AIR_SAMPLE`] whole systems at the epoch, every surface
/// with an air carries its structure, which starts at its conditions, takes its skin from its bulk
/// section's `T_eq` and its figures' heat flow and cools aloft, and every airless one carries none,
/// `Ok(None)`. No surface's structure is refused, so none is not modelled.
#[test]
fn under_the_batch_every_air_carries_its_structure_and_every_airless_surface_none() {
    let (mut airs, mut airless) = (0_usize, 0_usize);
    let mut isothermal = Vec::new();
    let mut refused = Vec::new();
    let mut deepest = (0_usize, String::new());
    for snapshot in batch_snapshots() {
        for record in snapshot.bodies() {
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            let id = record.identity().id();
            match (has_air(surface), surface.vertical_structure()) {
                (true, Modelled::Ok(Some(VerticalStructure::RadiativeConvective(profile)))) => {
                    check_profile(record, surface, profile);
                    airs += 1;
                    let segments = profile.nodes().len() - 1;
                    if segments == 0 {
                        isothermal.push(format!(
                            "{id}: {:?} at {:.2} K",
                            surface.state(),
                            profile.surface_temperature().value()
                        ));
                    }
                    if segments > deepest.0 {
                        deepest = (segments, format!("{id}: {:?}", surface.state()));
                    }
                }
                (false, Modelled::Ok(None)) => {
                    airless += 1;
                }
                (_, Modelled::NotModelled) => refused.push(format!("{id}: {:?}", surface.state())),
                (air, structure) => panic!("{id}: air {air}, {structure:?}"),
            }
            if surface.state() == SurfaceState::Airless {
                assert!(!has_air(surface), "{id}");
            }
        }
    }
    eprintln!(
        "{airs} airs ({} isothermal: {isothermal:?}), {airless} airless; the deepest adiabat {} \
         segments ({}); {} refused: {refused:?}",
        isothermal.len(),
        deepest.0,
        deepest.1,
        refused.len()
    );
    assert!(refused.is_empty(), "{refused:?}");
    assert!(airs > 20 && airless > 20, "{airs} airs, {airless} airless");
}

/// What a generated body's structure is checked against: its name and whether its surface is one.
type Wanted = (&'static str, fn(&Surface) -> bool);

/// A generated Earth, Venus and Titan, the first of each in the sample's order under the batch's
/// rules, carry plausible structures: finite, cooling aloft through the troposphere, and their
/// skins below their equilibrium temperatures and their grounds. The figures are printed for the
/// plan's record; the bounds are loose.
#[test]
fn a_generated_earth_venus_and_titan_carry_plausible_structures() {
    let condensable = |s: &Surface| s.climate_regime().condensable();
    let t_s = |s: &Surface| s.conditions().mean_surface_temperature().value();
    let p_s = |s: &Surface| s.conditions().surface_pressure().value();
    let wanted: [Wanted; 3] = [
        ("Earth", |s| {
            s.state() == SurfaceState::Temperate
                && s.climate_regime().condensable() == Some(Gas::Water.substance())
                && (275.0..=305.0).contains(&s.conditions().mean_surface_temperature().value())
                && (0.5e5..=3e5).contains(&s.conditions().surface_pressure().value())
        }),
        ("Venus", |s| {
            s.state() == SurfaceState::RunawayGreenhouse
                && s.conditions().surface_pressure().value() > 10e5
                && s.conditions().mean_surface_temperature().value() > 500.0
        }),
        ("Titan", |s| {
            s.climate_regime().condensable() == Some(Gas::Methane.substance())
                && (80.0..=110.0).contains(&s.conditions().mean_surface_temperature().value())
                && s.conditions().surface_pressure().value() > 0.5e5
        }),
    ];
    let mut found: [Option<(BodyRecord, String)>; 3] = [None, None, None];
    'systems: for (k, (ctx, system)) in whole().iter().enumerate() {
        let built;
        let snapshot = if let Some(snapshot) = batch_snapshots().get(k) {
            snapshot
        } else {
            built = batch_snapshot(ctx, system);
            &built
        };
        for record in snapshot.bodies() {
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            for (slot, (name, is)) in found.iter_mut().zip(wanted) {
                if slot.is_none() && is(surface) {
                    *slot = Some((
                        record.clone(),
                        format!("{name} ({:?}, system {k})", record.identity().id()),
                    ));
                }
            }
            if found.iter().all(Option::is_some) {
                break 'systems;
            }
        }
    }
    for ((name, _), slot) in wanted.iter().zip(&found) {
        let Some((record, label)) = slot else {
            panic!("the sample holds no generated {name}");
        };
        let Section::Ok(surface) = record.surface() else {
            unreachable!("found by its surface");
        };
        let Section::Ok(bulk) = record.bulk() else {
            panic!("{label}: no bulk section");
        };
        let profile = profile_of(surface);
        check_profile(record, surface, profile);
        let t_eq = bulk.equilibrium_temperature().value();
        let skin = profile.skin_temperature().value();
        let top = profile.tropopause_pressure().value();
        let one_bar =
            (p_s(surface) > 1e5).then(|| profile.temperature_at(Pascals::new(1e5)).value());
        eprintln!(
            "{label}: {:?}, T_s {:.2} K at {:.4} bar, condensable {:?}, α {}, T_eq {t_eq:.2} K, \
             T_skin {skin:.2} K, tropopause {:.4} bar, {} segments, at 1 bar {one_bar:?} K",
            surface.state(),
            t_s(surface),
            p_s(surface) / 1e5,
            condensable(surface).map(|id| id.substance().key().as_str().to_owned()),
            profile.lapse_factor(),
            top / 1e5,
            profile.nodes().len() - 1,
        );
        assert!(skin < t_eq && skin < t_s(surface), "{label}: {skin} K");
        assert!(top < p_s(surface), "{label}: a tropopause at its ground");
        assert!(
            (0.5..=1.0).contains(&profile.lapse_factor()),
            "{label}: α {}",
            profile.lapse_factor()
        );
        assert_same_bits(
            profile.temperature_at(profile.surface_pressure()).value(),
            t_s(surface),
        );
        assert_same_bits(
            profile.temperature_at(Pascals::new(top / 2.0)).value(),
            skin,
        );
    }
}

/// The same seed twice gives the same structures: the first ten whole systems generated again
/// from the seed, under the batch's rules, give every surface's structure bit for bit (by its
/// `Debug` form, Rust's shortest round-trip decimals).
#[test]
fn the_same_seed_twice_gives_the_same_structure() {
    let mut structures = 0_usize;
    for ((ctx, _), first) in whole().iter().zip(batch_snapshots()).take(10) {
        let again = batch_snapshot(ctx, &generate(SEED, ctx));
        assert_eq!(again.bodies().len(), first.bodies().len());
        for (a, b) in again.bodies().iter().zip(first.bodies()) {
            if let (Section::Ok(a), Section::Ok(b)) = (a.surface(), b.surface()) {
                assert_eq!(
                    format!("{:?}", a.vertical_structure()),
                    format!("{:?}", b.vertical_structure())
                );
                structures += usize::from(matches!(a.vertical_structure(), Modelled::Ok(Some(_))));
            }
        }
    }
    assert!(structures > 5, "{structures}");
}
