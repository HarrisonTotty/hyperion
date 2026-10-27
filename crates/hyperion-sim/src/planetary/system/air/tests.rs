//! The air's sections on the record (P14.T54.b): held at 23, so that every record is 23's; under
//! the batch's rules, the structure of every surface with an air, and none for every airless one,
//! and the aerosol and absorber inventory of every air, the empty one for every airless surface, on
//! the Solar System table and over the plan-14 sample (`system::tests::whole`).

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::math;
use crate::planetary::chem::condense::{NotFormed, ResolveOpticalDepthError, SizeModeError};
use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
use crate::planetary::hooks::aerosols::{
    Absorber, AerosolDeck, AerosolMode, MARS_DUST_OPTICAL_DEPTH, NEAR_CRITICAL_LIQUID_SHARE,
    ParticleSize, ShapeClass, TITAN_HAZE_OPTICAL_DEPTH, dust_optical_depth,
};
use crate::planetary::hooks::figures::{VolatileHistory, table_figures};
use crate::planetary::hooks::structure::{
    DRY_LAPSE_FACTOR, METHANE_LAPSE_FACTOR, WATER_LAPSE_FACTOR, effective_temperature,
    internal_temperature, skin_temperature,
};
use crate::planetary::hooks::surface::regime::{ClimateInputs, surface_reservoirs};
use crate::planetary::hooks::surface::tests::table;
use crate::planetary::record::{BodyRecord, Section, Surface, SystemSnapshot};
use crate::planetary::surface::CoarseInputs;
use crate::planetary::system::RecordRules;
use crate::planetary::system::tests::{SEED, golden_systems, golden_systems_afresh, sample, whole};
use crate::planetary::testing::with_every_24_hold;
use crate::planetary::{PlanetarySystem, SystemContext, generate};
use crate::substance::{CondensedPhase, SubstanceId, by_key};
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
/// both times its golden pins, holds its vertical structure, its aerosol inventory and its
/// modifiers (P14.T54.b.c, behind the same hold) not modelled, and every gas-envelope record's
/// envelope (P14.T54.b.i, behind the same hold) not modelled. The server's
/// `the_members_of_later_tasks_are_absent_and_a_moon_s_ocean_is_computed` holds the wire's members
/// absent on every golden body, and `every_present_planet_has_exactly_one_of_a_surface_and_an_envelope`
/// the envelope `not_modelled`. That the held arm calls none of the pure modules is held by the
/// source scans' `HELD` lists (`nothing_on_the_record_calls_the_inventory_yet`,
/// `nothing_on_the_record_calls_the_condensation_yet`,
/// `modifiers_nothing_on_the_record_reads_the_modifiers_yet`,
/// `nothing_on_the_record_calls_the_envelope_atmosphere_yet`) and here by its arm: the table's
/// Venus, whose inventory the batch builds, records none under `Held` (and the modifiers' arm in
/// `system::modifiers`' tests, `the_held_record_computes_no_modifiers`, and the envelope's in
/// `system::envelope`'s, `the_held_envelope_computes_nothing`).
#[test]
fn the_air_sections_are_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < AIR_SECTIONS_VERSION,
        "the 23 → 24 bump (P14.T54.b.l) puts the air's sections on the record: every surface's \
         vertical structure (P14.T24.e), `null` for one with no air, its aerosol and absorber \
         inventory (P14.T24.c with T24.g), the empty inventory for one with no air and not \
         modelled for a magma ocean's air, and its modifiers (P14.T51.d with T51.j, \
         P14.T54.b.c), and every sub-Neptune's and giant's envelope section (P14.T24.d with \
         T52.d on T50.b's abundances, P14.T54.b.i). Delete \
         `AIR_SECTIONS_VERSION`, `AirRules` with its `Held` arm, `record_structure`'s, \
         `record_aerosols`', `record_modifiers`', `planet_field`'s and `record_envelope`'s rules, \
         `RecordRules::air` \
         (and `RecordRules` with `PlanetarySystem::with_record_rules` once the cold traps' hold \
         goes too); the five source scans with their `HELD` lists \
         (`nothing_on_the_record_calls_the_inventory_yet`, \
         `nothing_on_the_record_calls_the_condensation_yet`, \
         `substance_aerosol_nothing_on_the_record_reads_the_extinction_yet`, \
         `modifiers_nothing_on_the_record_reads_the_modifiers_yet`, \
         `nothing_on_the_record_calls_the_envelope_atmosphere_yet`); \
         `the_venus_test_is_rechecked_at_the_23_to_24_bump`, \
         `the_held_record_carries_no_structure`, `the_held_record_computes_no_modifiers`, \
         `the_held_envelope_computes_nothing` and this \
         test. The server's \
         `the_members_of_later_tasks_are_absent_and_a_moon_s_ocean_is_computed` then takes the \
         structure as present, `null` on an airless surface, the aerosols as present, the empty \
         object on an airless surface, and the modifiers as present where the record models them, \
         and `every_present_planet_has_exactly_one_of_a_surface_and_an_envelope` the envelope `ok` \
         where modelled (decision-p14-t54b-open §1.3, §3.3)"
    );
    assert_eq!(AirRules::in_force(), AirRules::Held);
    assert_eq!(RecordRules::in_force().air, AirRules::Held);
    let (mut surfaces, mut envelopes) = (0_usize, 0_usize);
    for (name, ctx, system) in golden_systems() {
        for t in [UniverseTime::EPOCH, ClockWindow::END] {
            for record in system.snapshot_at(ctx, t).bodies() {
                if record
                    .bulk()
                    .ok()
                    .is_some_and(|bulk| !bulk.class().has_solid_surface())
                {
                    let id = record.identity().id();
                    assert_eq!(record.envelope(), &Section::NotModelled, "{name} {id}");
                    envelopes += 1;
                }
                let Section::Ok(surface) = record.surface() else {
                    continue;
                };
                let id = record.identity().id();
                assert_eq!(
                    surface.vertical_structure(),
                    Modelled::NotModelled,
                    "{name} {id}"
                );
                assert_eq!(surface.aerosols(), Modelled::NotModelled, "{name} {id}");
                assert_eq!(surface.modifiers(), Modelled::NotModelled, "{name} {id}");
                surfaces += 1;
            }
        }
    }
    assert!(surfaces > 20, "{surfaces} golden surfaces");
    assert!(envelopes > 10, "{envelopes} golden gas envelopes");
    let venus = table_under_the_switch()
        .into_iter()
        .find(|air| air.name == "Venus")
        .expect("Venus is in the table");
    assert!(matches!(venus.inventory(), Modelled::Ok(ref i) if !i.decks().is_empty()));
    assert_eq!(
        record_aerosols(AirRules::Held, &venus.surface(), &venus.structure),
        Modelled::NotModelled
    );
}

/// A body of the Solar System table, with what its record's structure and inventory read and its
/// structure under the air's switch (the batch's rules).
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
    /// Its state.
    state: SurfaceState,
    /// Whether its air keeps Venus's deck: its table atmosphere's, one solve under the deck and so
    /// [`RunawayDeck::Kept`] (`Atmosphere::runaway_deck`).
    runaway_deck: RunawayDeck,
    /// Its surface gravity.
    gravity: MetresPerSecondSquared,
    /// Its figures after their second stage, as a record's surface holds them.
    figures: GlobalFigures,
}

impl TableAir {
    /// What its record's inventory reads beside its structure, its photochemistry's star the
    /// Sun's 5,772 K (IAU 2015 Resolution B3's nominal `T_eff`).
    fn surface(&self) -> AirSurface<'_> {
        AirSurface {
            state: self.state,
            runaway_deck: self.runaway_deck,
            conditions: &self.conditions,
            figures: &self.figures,
            gravity: self.gravity,
            star_temperature: Some(Kelvin::new(5_772.0)),
        }
    }

    /// Its inventory under [`AirRules::Batch`].
    fn inventory(&self) -> Modelled<AerosolInventory> {
        record_aerosols(AirRules::Batch, &self.surface(), &self.structure)
    }
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
                state: body.atmosphere().state(),
                runaway_deck: body.atmosphere().runaway_deck(),
                gravity: body.surface_gravity(),
                figures: published,
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
            ..
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

/// The record at the version in force holds no structure, no aerosol inventory and no modifiers
/// (P14.T54.b.c), and the batch's surface is the held one with those three alone added: over the
/// first ten whole systems, every surface held is the batch's with all three not modelled, so that
/// every golden keeps 23's bits.
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
                    assert_eq!(held.aerosols(), Modelled::NotModelled, "{id}");
                    assert_eq!(held.modifiers(), Modelled::NotModelled, "{id}");
                    let stripped = batch
                        .clone()
                        .with_vertical_structure(Modelled::NotModelled)
                        .with_aerosols(Modelled::NotModelled)
                        .with_modifiers(Modelled::NotModelled);
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

/// The same seed twice gives the same air's sections: the first ten whole systems generated again
/// from the seed, under the batch's rules, give every surface's structure and aerosol inventory
/// bit for bit (by their `Debug` forms, Rust's shortest round-trip decimals).
#[test]
fn the_same_seed_twice_gives_the_same_air_sections() {
    let (mut structures, mut decks) = (0_usize, 0_usize);
    for ((ctx, _), first) in whole().iter().zip(batch_snapshots()).take(10) {
        let again = batch_snapshot(ctx, &generate(SEED, ctx));
        assert_eq!(again.bodies().len(), first.bodies().len());
        for (a, b) in again.bodies().iter().zip(first.bodies()) {
            if let (Section::Ok(a), Section::Ok(b)) = (a.surface(), b.surface()) {
                assert_eq!(
                    format!("{:?}", a.vertical_structure()),
                    format!("{:?}", b.vertical_structure())
                );
                assert_eq!(format!("{:?}", a.aerosols()), format!("{:?}", b.aerosols()));
                structures += usize::from(matches!(a.vertical_structure(), Modelled::Ok(Some(_))));
                decks += a.aerosols().ok().map_or(0, |i| i.decks().len());
            }
        }
    }
    eprintln!("{structures} structures and {decks} decks the same bits twice");
    assert!(structures > 5, "{structures} structures, {decks} decks");
}

/// The registry row of `key`.
fn row(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a registry row"))
}

/// The structure of a record's surface as [`record_aerosols`] takes it, owned.
fn owned_structure(surface: &Surface) -> Modelled<Option<VerticalStructure>> {
    match surface.vertical_structure() {
        Modelled::Ok(structure) => Modelled::Ok(structure.cloned()),
        Modelled::NotModelled => Modelled::NotModelled,
    }
}

/// What the record's inventory of `surface`, of record `record`, reads beside its structure, its
/// photochemistry's star's effective temperature `star`, which the record does not hold, under the
/// air's batch alone: 23's states, the escape's rules held, under which every runaway keeps
/// Venus's deck ([`RunawayDeck::Kept`]; the record does not hold its deck either).
fn air_surface_of<'a>(
    record: &BodyRecord,
    surface: &'a Surface,
    star: Option<Kelvin>,
) -> AirSurface<'a> {
    let Section::Ok(bulk) = record.bulk() else {
        panic!("{}: a surface with no bulk section", record.identity().id());
    };
    AirSurface {
        state: surface.state(),
        runaway_deck: RunawayDeck::Kept,
        conditions: surface.conditions(),
        figures: surface.figures(),
        gravity: bulk.surface_gravity(),
        star_temperature: star,
    }
}

/// Every surface of the first [`AIR_SAMPLE`] whole systems' records at the epoch under the batch's
/// rules whose inventory is modelled, with its record, its system's context and the inventory.
fn batch_inventories() -> impl Iterator<
    Item = (
        &'static SystemContext,
        &'static BodyRecord,
        &'static Surface,
        &'static AerosolInventory,
    ),
> {
    whole()
        .iter()
        .zip(batch_snapshots())
        .flat_map(|((ctx, _), snapshot)| snapshot.bodies().iter().map(move |record| (ctx, record)))
        .filter_map(|(ctx, record)| {
            let Section::Ok(surface) = record.surface() else {
                return None;
            };
            match surface.aerosols() {
                Modelled::Ok(inventory) => Some((ctx, record, surface, inventory)),
                Modelled::NotModelled => None,
            }
        })
}

/// On the Solar System table, under the air's switch, the record's inventory is
/// `AerosolInventory::derive` over the published surface's own members, as plan 14's T54.b lists
/// them (the state, the surface pressure, gases, molar mass and liquid-water flag, the gravity, the
/// structure, the figures' cloud fraction, liquids' cover and outgassing, and the photochemistry's
/// star), for each body with an air, and the empty inventory for each without (the Moon among
/// them); a magma ocean's is not modelled while it holds an air (R08's S5) and empty once its
/// volatiles are gone; a tenuous air's (P14.T53.c) is not modelled at 24 (P14.T55.n); held, or with
/// no structure, it is not modelled.
#[test]
fn the_record_s_inventory_is_the_derivation_s_or_empty_without_air() {
    let (mut airs, mut airless) = (Vec::new(), Vec::new());
    for air in table_under_the_switch() {
        let name = air.name;
        let surface = air.surface();
        match &air.structure {
            Modelled::Ok(Some(VerticalStructure::RadiativeConvective(profile))) => {
                let inputs = AerosolInputs {
                    state: air.state,
                    surface_pressure: air.conditions.surface_pressure(),
                    gases: air.conditions.gases(),
                    mean_molar_mass_g_per_mol: air
                        .conditions
                        .mean_molar_mass_g_per_mol()
                        .expect("an air's molar mass"),
                    gravity: air.gravity,
                    structure: profile,
                    cloud_fraction: air.figures.cloud_fraction(),
                    star_temperature: Some(Kelvin::new(5_772.0)),
                    surface_liquid_cover: air.figures.ocean_fraction(),
                    outgassing: air.figures.outgassing(),
                    liquid_water: air.conditions.liquid_water(),
                    runaway_deck: air.runaway_deck,
                };
                let derived =
                    AerosolInventory::derive(&inputs).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(air.inventory(), Modelled::Ok(derived), "{name}");
                airs.push(name);
            }
            Modelled::Ok(None) => {
                assert_eq!(
                    air.inventory(),
                    Modelled::Ok(AerosolInventory::empty()),
                    "{name}"
                );
                airless.push(name);
            }
            Modelled::Ok(Some(VerticalStructure::Envelope(_))) => {
                panic!("{name}: a surface carries an envelope's profile")
            }
            Modelled::NotModelled => panic!("{name}: the table's structure is refused"),
        }
        let melt = AirSurface {
            state: SurfaceState::MagmaOcean,
            ..surface
        };
        let melt_s = if air.conditions.gases().is_empty() {
            Modelled::Ok(AerosolInventory::empty())
        } else {
            Modelled::NotModelled
        };
        assert_eq!(
            record_aerosols(AirRules::Batch, &melt, &air.structure),
            melt_s,
            "{name}"
        );
        // P14.T53.c's tenuous air: not modelled at 24, whatever its structure (P14.T55.n).
        let tenuous = AirSurface {
            state: SurfaceState::Tenuous,
            ..surface
        };
        assert_eq!(
            record_aerosols(AirRules::Batch, &tenuous, &air.structure),
            Modelled::NotModelled,
            "{name}"
        );
        assert_eq!(
            record_aerosols(AirRules::Held, &surface, &air.structure),
            Modelled::NotModelled,
            "{name}"
        );
        assert_eq!(
            record_aerosols(AirRules::Batch, &surface, &Modelled::NotModelled),
            Modelled::NotModelled,
            "{name}"
        );
    }
    eprintln!("airs {airs:?}; airless {airless:?}");
    assert!(
        airless.contains(&"Moon") && airs.len() >= 4,
        "{airs:?} {airless:?}"
    );
}

/// Whether `mode` lies within `deck`, its base no deeper than the deck's and its top no shallower,
/// to 10⁻¹² of each: P14.T24.g's acid modes span fractions of the deck's ln p, so the outermost
/// lands on its ends to rounding.
fn inside_its_deck(mode: &AerosolMode, deck: &AerosolDeck) -> bool {
    let profile = mode.profile();
    let slack = 1.0 + 1e-12;
    profile.base().value() <= deck.base().value() * slack
        && profile.top().value() * slack >= deck.top().value()
}

/// The table's body `name` under the switch, with its inventory, which must be modelled.
fn table_inventory(name: &str) -> (TableAir, AerosolInventory) {
    let air = table_under_the_switch()
        .into_iter()
        .find(|air| air.name == name)
        .unwrap_or_else(|| panic!("{name} is in the table"));
    match air.inventory() {
        Modelled::Ok(inventory) => (air, inventory),
        Modelled::NotModelled => panic!("{name}: no inventory"),
    }
}

/// The table's median Venus and Earth under the switch (P14.T54.b.b): Venus carries P14.T24.g's
/// acid deck, its four modes inside the deck and their τ(550) above 10, so that R08 classes it a
/// cloud deck, and no sulphate layer; Earth a water deck whose vapour below is its air's water to
/// 10⁻¹², and P14.T24.g's sulphate layer, one mode naming `H2SO4` with no deck.
#[test]
fn the_table_s_venus_carries_its_acid_deck_and_earth_its_water_deck_and_sulphate_layer() {
    let acid = row("H2SO4");
    let (_, venus) = table_inventory("Venus");
    let deck = venus
        .decks()
        .iter()
        .find(|d| d.condensate() == acid)
        .expect("Venus's acid deck");
    let modes: Vec<&AerosolMode> = venus.modes().iter().filter(|m| m.deck() == acid).collect();
    let tau = venus.deck_optical_depth(acid).expect("the acid deck's τ");
    eprintln!(
        "the table's Venus: an acid deck from {:.4} bar at {:.1} K to {:.4} bar, {} modes, \
         τ(550) {tau:.3}",
        deck.base().value() / 1e5,
        deck.base_temperature().value(),
        deck.top().value() / 1e5,
        modes.len()
    );
    assert_eq!(modes.len(), 4);
    assert!(tau > 10.0, "τ {tau}");
    for mode in &modes {
        assert!(inside_its_deck(mode, deck), "{mode:?} outside {deck:?}");
    }
    let (earth_air, earth) = table_inventory("Earth");
    let water = Gas::Water.substance();
    let deck = earth
        .decks()
        .iter()
        .find(|d| d.condensate() == water)
        .expect("Earth's water deck");
    let x = earth_air
        .conditions
        .gases()
        .iter()
        .find(|(id, _)| *id == water)
        .map(|&(_, x)| x)
        .expect("water in Earth's air");
    assert!(
        (deck.vapour_below() - x).abs() < 1e-12,
        "{} against {x}",
        deck.vapour_below()
    );
    assert!(earth.decks().iter().all(|d| d.condensate() != acid));
    let layers = earth.modes().iter().filter(|m| m.deck() == acid).count();
    assert_eq!(layers, 1, "Earth's sulphate layer");
}

/// The table's Mars and Titan under the switch (P14.T54.b.b): Mars carries no dust, since the
/// table's Mars holds 0.11 bar of N₂ until P14.T53.a, above the dust's 0.1 bar (stated, P14.T53.c's
/// to fix; the golden `planetary/aerosols` pins the measured Mars's dust); Titan the methane haze,
/// an aggregate of τ(550) within a factor of two of Titan's 8.1, whose column mass and temperature
/// are not modelled, and its methane deck, every mode of it covering the figures' cloud fraction.
///
/// **Titan's disc mean, stated.** A deck's τ(550) is in-cloud, over its coverage, the snapshot
/// path's cloud fraction (P14.T51.b's stand-in; T24.c's record), so its disc mean is their
/// product. Huygens's descent saw no optically thick cloud (Tomasko et al. 2005, Nature 438, 765),
/// and Titan's observed cloud cover is about 1% (T24.c's record): the figures are printed beside
/// that for the plan's record, not asserted.
#[test]
fn the_table_s_mars_carries_no_dust_and_titan_its_haze_beside_its_disc_mean_cloud() {
    let (mars_air, mars) = table_inventory("Mars");
    eprintln!(
        "the table's Mars: {:.4} bar, {} decks, no dust",
        mars_air.conditions.surface_pressure().value() / 1e5,
        mars.decks().len()
    );
    assert!(
        mars.modes()
            .iter()
            .all(|m| m.material() != row("mars_dust"))
    );
    let (titan_air, titan) = table_inventory("Titan");
    let tholin = row("tholin");
    let haze = titan
        .modes()
        .iter()
        .find(|m| m.material() == tholin)
        .expect("Titan's haze");
    assert_eq!(haze.shape(), ShapeClass::Aggregate);
    assert!(matches!(haze.size(), Ok(ParticleSize::Aggregate(_))));
    assert!(haze.column_mass().is_none() && haze.temperature().is_none());
    let haze_tau = haze.optical_depth().expect("the haze's τ").value();
    assert!(
        (TITAN_HAZE_OPTICAL_DEPTH / 2.0..=TITAN_HAZE_OPTICAL_DEPTH * 2.0).contains(&haze_tau),
        "{haze_tau}"
    );
    let methane = Gas::Methane.substance();
    let cover = titan_air.figures.cloud_fraction();
    let mut in_cloud = 0.0;
    for mode in titan.modes().iter().filter(|m| m.deck() == methane) {
        assert_same_bits(mode.coverage(), cover);
        in_cloud += mode.optical_depth().map_or(0.0, |d| d.value());
    }
    eprintln!(
        "the table's Titan: the haze's τ(550) {haze_tau:.3}; its methane deck's in-cloud τ(550) \
         {in_cloud:.3} over a cover of {cover:.3}, a disc mean of {:.3}, against Huygens's no \
         optically thick cloud and an observed cover of about 1% (a disc mean of {:.4})",
        in_cloud * cover,
        in_cloud * 0.01
    );
    assert!(in_cloud > 0.0, "Titan's methane deck");
}

/// Under the batch's rules, over the first [`AIR_SAMPLE`] whole systems at the epoch, every
/// surface with a structure carries its inventory, none refused, but a magma ocean's air, which is
/// not modelled (R08's S5; counted), and every surface with no air the empty inventory; and each
/// air's is the derivation's over its record's own
/// members (its state, conditions, figures and bulk section's gravity) with one of its system's
/// stars' effective temperatures, or none, which the record does not hold.
#[test]
fn under_the_batch_every_air_carries_its_inventory_and_every_airless_surface_the_empty_one() {
    let (mut airs, mut empty) = (0_usize, 0_usize);
    let (mut decks, mut modes, mut absorbers, mut not_formed) =
        (0_usize, 0_usize, 0_usize, 0_usize);
    let mut refused = Vec::new();
    let mut melts_with_air = Vec::new();
    for ((ctx, _), snapshot) in whole().iter().zip(batch_snapshots()) {
        let stars: Vec<Option<Kelvin>> = ctx
            .stars()
            .iter()
            .filter_map(|star| star.state_at(UniverseTime::EPOCH))
            .map(|state| Some(state.effective_temperature()))
            .chain([None])
            .collect();
        for record in snapshot.bodies() {
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            let id = record.identity().id();
            if surface.state() == SurfaceState::MagmaOcean
                && !surface.conditions().gases().is_empty()
            {
                assert_eq!(surface.aerosols(), Modelled::NotModelled, "{id}");
                melts_with_air.push(format!(
                    "{id} at {:.4} bar",
                    surface.conditions().surface_pressure().value() / 1e5
                ));
                continue;
            }
            let Modelled::Ok(inventory) = surface.aerosols() else {
                refused.push(format!(
                    "{id}: {:?} at {:.4} bar",
                    surface.state(),
                    surface.conditions().surface_pressure().value() / 1e5
                ));
                continue;
            };
            match surface.vertical_structure() {
                Modelled::Ok(None) => {
                    assert_eq!(inventory, &AerosolInventory::empty(), "{id}");
                    empty += 1;
                }
                Modelled::Ok(Some(_)) => {
                    let structure = owned_structure(surface);
                    let rebuilt = stars.iter().any(|&star| {
                        record_aerosols(
                            AirRules::Batch,
                            &air_surface_of(record, surface, star),
                            &structure,
                        ) == Modelled::Ok(inventory.clone())
                    });
                    assert!(rebuilt, "{id}: no star of its system gives its inventory");
                    airs += 1;
                    decks += inventory.decks().len();
                    modes += inventory.modes().len();
                    absorbers += inventory.absorbers().len();
                    not_formed += inventory.not_formed().len();
                }
                Modelled::NotModelled => panic!("{id}: an inventory with no structure"),
            }
        }
    }
    eprintln!(
        "{airs} airs: {decks} decks, {modes} modes, {absorbers} absorbers, {not_formed} not \
         formed; {empty} empty; {} magma oceans' airs not modelled: {melts_with_air:?}; {} \
         refused: {refused:?}",
        melts_with_air.len(),
        refused.len()
    );
    assert!(refused.is_empty(), "{refused:?}");
    assert!(airs > 20 && empty > 20, "{airs} airs, {empty} empty");
}

/// Over the same sample under the batch's rules, every inventory's modes hold their members as the
/// wire reads them (decision-p14-t54b-open §2.2):
///
/// - a mode belongs to a listed deck by its condensate, or is a rule's (the dust's `mars_dust`,
///   the haze's `tholin`, a sulphate layer's `H2SO4`), naming its own material with no deck, which
///   the wire sends as `null`;
/// - an acid deck and a sulphate layer never share `H2SO4` on one body: a body with an acid deck
///   has its four modes inside it and no other mode naming `H2SO4`, and one without at most the
///   layer;
/// - a deck's mode covers the figures' cloud fraction, and a rule's the whole sky;
/// - every mode but an aggregate carries the temperature its optics were taken at, finite and
///   positive, and an aggregate none, nor a column mass.
#[test]
fn under_the_batch_every_mode_holds_its_deck_coverage_and_temperature() {
    let acid = row("H2SO4");
    let rules = [row("mars_dust"), row("tholin"), acid];
    let (mut acid_decks, mut layers, mut hazes) = (0_usize, 0_usize, 0_usize);
    let mut by_deck = std::collections::BTreeMap::<String, usize>::new();
    for (_, record, surface, inventory) in batch_inventories() {
        let id = record.identity().id();
        let cover = surface.figures().cloud_fraction();
        for mode in inventory.modes() {
            let listed = inventory
                .decks()
                .iter()
                .any(|deck| deck.condensate() == mode.deck());
            if listed {
                assert_same_bits(mode.coverage(), cover);
            } else {
                assert!(
                    mode.deck() == mode.material() && rules.contains(&mode.material()),
                    "{id}: {mode:?} names no listed deck"
                );
                assert_same_bits(mode.coverage(), 1.0);
            }
            match (mode.shape(), mode.temperature()) {
                (ShapeClass::Aggregate, None) => {
                    assert!(mode.column_mass().is_none(), "{id}: {mode:?}");
                    hazes += 1;
                }
                (ShapeClass::Aggregate, Some(_)) | (_, None) => panic!("{id}: {mode:?}"),
                (_, Some(t)) => assert!(t.value().is_finite() && t.value() > 0.0, "{id}"),
            }
            *by_deck
                .entry(mode.deck().substance().key().as_str().to_owned())
                .or_default() += 1;
        }
        let acid_modes: Vec<&AerosolMode> = inventory
            .modes()
            .iter()
            .filter(|m| m.deck() == acid)
            .collect();
        if let Some(deck) = inventory.decks().iter().find(|d| d.condensate() == acid) {
            assert_eq!(acid_modes.len(), 4, "{id}: the acid's four modes alone");
            for mode in &acid_modes {
                assert!(
                    inside_its_deck(mode, deck),
                    "{id}: {mode:?} outside its deck"
                );
            }
            acid_decks += 1;
        } else {
            assert!(acid_modes.len() <= 1, "{id}: {acid_modes:?}");
            layers += acid_modes.len();
        }
    }
    eprintln!(
        "{acid_decks} acid decks, {layers} sulphate layers, {hazes} hazes; modes by deck \
         {by_deck:?}"
    );
}

/// Over the same sample under the batch's rules, the dust is the rule's (P14.T24.c): a mode of
/// `mars_dust` where `dust_optical_depth`, at the surface's own pressure, ground temperature,
/// molar mass, gravity and liquids' cover, is above 0, its τ(550) that to 10⁻¹² and at most Mars's
/// 0.5, and none elsewhere. The thin dry airs that carry it are printed.
#[test]
fn under_the_batch_the_dust_is_the_rule_s() {
    let dust = row("mars_dust");
    let mut dusts = 0_usize;
    for (_, record, surface, inventory) in batch_inventories() {
        let id = record.identity().id();
        let Section::Ok(bulk) = record.bulk() else {
            panic!("{id}: no bulk section");
        };
        let conditions = surface.conditions();
        let rule = dust_optical_depth(
            conditions.surface_pressure(),
            conditions.mean_surface_temperature(),
            conditions.mean_molar_mass_g_per_mol().unwrap_or(f64::NAN),
            bulk.surface_gravity(),
            surface.figures().ocean_fraction(),
        );
        let found: Vec<&AerosolMode> = inventory
            .modes()
            .iter()
            .filter(|m| m.material() == dust)
            .collect();
        match found.as_slice() {
            [] => assert!(rule <= 0.0, "{id}: no dust where the rule gives {rule}"),
            [mode] => {
                let tau = mode.optical_depth().expect("the dust's τ").value();
                assert!(
                    (tau - rule).abs() <= 1e-12 * rule
                        && rule > 0.0
                        && rule <= MARS_DUST_OPTICAL_DEPTH,
                    "{id}: the dust's τ {tau} against the rule's {rule}"
                );
                eprintln!(
                    "{id}: dust τ(550) {tau:.4} at {:.5} bar",
                    conditions.surface_pressure().value() / 1e5
                );
                dusts += 1;
            }
            more => panic!("{id}: {} dust modes", more.len()),
        }
    }
    eprintln!("{dusts} dust modes over the sample");
}

/// What [`tally_liquids`] counts of inventories' liquid modes (grain-density F4 and F1).
#[derive(Debug, Default)]
struct LiquidTally {
    /// Liquid modes, and of them those of a row with no phase diagram (P14.T24.g's acid's).
    modes: usize,
    no_diagram: usize,
    /// The liquid modes above [`NEAR_CRITICAL_LIQUID_SHARE`] of their critical temperature, each
    /// refused its size and τ, by body and material.
    refused: Vec<String>,
    /// The modelled liquid nearest its critical point: its share of `T_c`, by body and material.
    nearest: (f64, String),
    /// Liquid water modes, those whose top lies below the triple point on the structure, and water
    /// ice modes.
    droplets: usize,
    supercooled_tops: usize,
    ices: usize,
}

/// Tallies the liquid modes of `inventory`, of the body `id` whose air's structure is `structure`,
/// into `tally`, asserting F4's refusal (every liquid above [`NEAR_CRITICAL_LIQUID_SHARE`] of its
/// critical temperature has no size and no τ, for want of its grain density) and F1's (no liquid
/// water mode below the triple point).
fn tally_liquids(
    id: &str,
    inventory: &AerosolInventory,
    structure: &VerticalStructure,
    tally: &mut LiquidTally,
) {
    let water = Gas::Water.substance();
    let triple = water
        .substance()
        .phase_diagram()
        .expect("water's phase diagram")
        .triple_temperature()
        .value();
    for mode in inventory.modes() {
        let material = mode.material();
        let key = material.substance().key();
        match mode.phase() {
            CondensedPhase::Solid => {
                tally.ices += usize::from(material == water);
                continue;
            }
            CondensedPhase::Liquid => tally.modes += 1,
        }
        let t = mode.temperature().expect("a liquid's temperature").value();
        match material.substance().phase_diagram() {
            Some(diagram) => {
                let share = t / diagram.critical_temperature().value();
                if share > NEAR_CRITICAL_LIQUID_SHARE {
                    assert!(
                        matches!(
                            mode.size(),
                            Err(SizeModeError::NoDensity {
                                phase: CondensedPhase::Liquid,
                                ..
                            })
                        ) && matches!(
                            mode.optical_depth(),
                            Err(ResolveOpticalDepthError::Size(SizeModeError::NoDensity {
                                phase: CondensedPhase::Liquid,
                                ..
                            }))
                        ),
                        "{id}: {key} published at {share} T_c: {mode:?}"
                    );
                    tally.refused.push(format!("{id}: {key} at {t} K"));
                } else if share > tally.nearest.0 {
                    tally.nearest = (share, format!("{id}: {key}"));
                }
            }
            None => tally.no_diagram += 1,
        }
        if material == water {
            assert!(t >= triple, "{id}: a supercooled water mode at {t} K");
            tally.droplets += 1;
            let top = structure.temperature_at(mode.profile().top()).value();
            tally.supercooled_tops += usize::from(top < triple);
        }
    }
}

/// The record's rules at 24: the cold traps' and the air's sections' batches, which a system takes
/// with [`PlanetarySystem::with_record_rules`] beside the thread's holds
/// (`planetary::testing::with_every_24_hold`).
fn every_record_batch() -> RecordRules {
    use crate::planetary::hooks::cold_traps::ColdTrapRules;

    RecordRules {
        cold_traps: ColdTrapRules::Batch,
        air: AirRules::Batch,
        ..RecordRules::in_force()
    }
}

/// What P14.T52.f's count (xii) lists of the records it walks (decision-n2-critical-decks §5):
/// the condensables labelled `Supercritical`, each by body and key with its x P ÷ `p_c` where the
/// structure cools through its critical temperature and the air's bulk gas, and the decks based
/// at the ground (a [`chem::condense`](crate::planetary::chem::condense) `Bottom` base) whose
/// vapour's x P there is at or above its critical pressure.
#[derive(Debug, Default)]
struct CriticalTally {
    labelled: Vec<String>,
    deep: Vec<String>,
}

impl CriticalTally {
    /// Adds the inventory `inventory` of the body `id`, whose surface is `surface` and whose air's
    /// structure is `structure`.
    fn add(
        &mut self,
        id: &str,
        surface: &Surface,
        structure: &VerticalStructure,
        inventory: &AerosolInventory,
    ) {
        let gases = surface.conditions().gases();
        let share = |of: SubstanceId| {
            gases
                .iter()
                .find(|&&(gas, _)| gas == of)
                .map_or(0.0, |&(_, x)| x)
        };
        let bulk = gases
            .iter()
            .fold(None::<(SubstanceId, f64)>, |best, &(gas, x)| match best {
                Some((_, most)) if most >= x => best,
                _ => Some((gas, x)),
            })
            .map_or_else(
                || "none".to_owned(),
                |(gas, _)| gas.substance().key().to_string(),
            );
        let ground = surface.conditions().surface_pressure().value();
        for &(row, why) in inventory.not_formed() {
            if why != NotFormed::Supercritical {
                continue;
            }
            let key = row.substance().key();
            let diagram = row.substance().phase_diagram().expect("a row's own curve");
            let level = pressure_at(structure, ground, diagram.critical_temperature().value());
            let ratio = share(row) * level / diagram.critical_pressure().value();
            self.labelled.push(format!(
                "{id}: {key} at x P {ratio:.3} p_c, its bulk {bulk}"
            ));
        }
        for deck in inventory.decks() {
            let Some(diagram) = deck.condensate().substance().phase_diagram() else {
                continue;
            };
            let at_ground = (deck.base().value() - ground).abs() <= 1e-9 * ground;
            let x_p = share(deck.condensate()) * ground;
            if at_ground && x_p >= diagram.critical_pressure().value() {
                self.deep.push(format!(
                    "{id}: {} at the ground, x P {:.3} p_c",
                    deck.condensate().substance().key(),
                    x_p / diagram.critical_pressure().value()
                ));
            }
        }
    }
}

/// The pressure, Pa, at which `structure`, from the ground pressure `ground` up, first cools to
/// `temperature` kelvin, by bisection in ln p between the scan's levels a tenth of an e-fold
/// apart; NaN where it never does over 30 e-folds.
fn pressure_at(structure: &VerticalStructure, ground: f64, temperature: f64) -> f64 {
    let at = |u: f64| structure.temperature_at(Pascals::new(math::exp(u))).value();
    let top = math::ln(ground);
    if at(top) <= temperature {
        return ground;
    }
    let mut deep = top;
    for step in 1..=300_u32 {
        let shallow = top - 0.1 * f64::from(step);
        if at(shallow) <= temperature {
            let (mut warm, mut cold) = (deep, shallow);
            for _ in 0..60 {
                let mid = f64::midpoint(warm, cold);
                if at(mid) <= temperature {
                    cold = mid;
                } else {
                    warm = mid;
                }
            }
            return math::exp(f64::midpoint(warm, cold));
        }
        deep = shallow;
    }
    f64::NAN
}

/// The F4, F1 and count (xii) tallies of the first [`AIR_SAMPLE`] whole systems and the golden
/// systems at the epoch with every hold at 24 in force ([`with_every_24_hold`] and
/// [`every_record_batch`]), each system generated and read inside the switches.
fn every_hold_tallies() -> (LiquidTally, CriticalTally) {
    with_every_24_hold(|| {
        let mut tallies = (LiquidTally::default(), CriticalTally::default());
        let samples = sample()
            .iter()
            .take(AIR_SAMPLE)
            .map(|ctx| (ctx.clone(), generate(SEED, ctx)));
        let goldens = golden_systems_afresh()
            .into_iter()
            .map(|(_, ctx, system)| (ctx, system));
        for (ctx, system) in samples.chain(goldens) {
            let snapshot = system
                .with_record_rules(every_record_batch())
                .snapshot_at(&ctx, UniverseTime::EPOCH);
            tally_snapshot(&snapshot, &mut tallies);
        }
        tallies
    })
}

/// Adds every surface of `snapshot` that carries its structure and its inventory to `tallies`.
fn tally_snapshot(
    snapshot: &SystemSnapshot,
    (liquids, critical): &mut (LiquidTally, CriticalTally),
) {
    for record in snapshot.bodies() {
        let Section::Ok(surface) = record.surface() else {
            continue;
        };
        let (Modelled::Ok(Some(structure)), Modelled::Ok(inventory)) =
            (surface.vertical_structure(), surface.aerosols())
        else {
            continue;
        };
        let id = record.identity().id().to_string();
        tally_liquids(&id, inventory, structure, liquids);
        critical.add(&id, surface, structure, inventory);
    }
}

/// F4 and F1 of decision-p14-t54a-grain-density §6, and P14.T52.f's count (xii), over the first
/// [`AIR_SAMPLE`] whole systems and the golden systems with every hold at 24 in force
/// ([`every_hold_tallies`]; decision-t24i-airless-state D3.3, decision-n2-critical-decks §4), over
/// the same sample under the air's batch alone (23's states and every other hold), and over the
/// Solar System table under the switch:
///
/// - **F4:** no liquid mode published with a size or a τ lies above 0.99 of its material's
///   critical temperature, where the grain-density mirror holds no liquid, so that every liquid
///   the record publishes is one the client can evaluate: the inventory refuses those there their
///   size and τ (`AerosolMode::of_mode`), each counted and printed. Under the air's batch alone
///   it refuses none: the two nitrogen decks it once refused (`02001b2ca0000000.0400` and
///   `02001b2c9ffe0000.0500`, based at nitrogen's critical point under kilobar methane airs) are
///   P14.T52.f's, labelled `supercritical`, which is asserted. With every hold, printed, 0500's
///   nitrogen is its bulk, and 0400's is still labelled in this tree, P14.T53.b part 2 (under
///   whose supercritical envelope its surface does not apply) not having landed here;
/// - **Count (xii)**, printed: every condensable labelled `supercritical`, with x P ÷ `p_c`
///   where the structure cools through its critical temperature and the air's bulk, and every
///   deck based at the ground with x P ≥ `p_c` there;
/// - **F1:** no liquid water mode lies below water's triple point. The choice is the loop's
///   (`chem::condense`'s phase rule): a deck takes the phase of its base, liquid at or above its
///   condensate's first melt and ice below, so that a supercooled droplet forms as an ice crystal
///   (a stated simplification), and a liquid deck that rises above its freezing level keeps its
///   phase and its base's density (one phase a deck). Such decks are counted, their top's
///   temperature on the structure below the triple point, and printed for the plan's record.
#[test]
fn no_liquid_mode_lies_near_its_critical_point_or_below_water_s_triple_point() {
    let (mut tally, critical) = every_hold_tallies();
    eprintln!(
        "every 24 hold: {} liquid modes, {} refused near their critical point: {:?}; {} labelled \
         supercritical: {:?}; {} decks at the ground past p_c: {:?}",
        tally.modes,
        tally.refused.len(),
        tally.refused,
        critical.labelled.len(),
        critical.labelled,
        critical.deep.len(),
        critical.deep
    );
    let mut alone = (LiquidTally::default(), CriticalTally::default());
    for snapshot in batch_snapshots() {
        tally_snapshot(snapshot, &mut alone);
    }
    let (alone_liquids, alone_critical) = alone;
    eprintln!(
        "the air's batch alone: {} refused: {:?}; {} labelled supercritical: {:?}; {} decks at \
         the ground past p_c: {:?}",
        alone_liquids.refused.len(),
        alone_liquids.refused,
        alone_critical.labelled.len(),
        alone_critical.labelled,
        alone_critical.deep.len(),
        alone_critical.deep
    );
    assert!(
        alone_liquids.refused.is_empty(),
        "{:?}",
        alone_liquids.refused
    );
    for body in ["02001b2ca0000000.0400", "02001b2c9ffe0000.0500"] {
        assert!(
            alone_critical
                .labelled
                .iter()
                .any(|line| line.starts_with(&format!("{body}: N2 "))),
            "{body}'s N2 is labelled supercritical under the air's batch alone: {:?}",
            alone_critical.labelled
        );
    }
    for air in table_under_the_switch() {
        if let (Modelled::Ok(Some(structure)), Modelled::Ok(inventory)) =
            (&air.structure, air.inventory())
        {
            tally_liquids(air.name, &inventory, structure, &mut tally);
        }
    }
    let LiquidTally {
        modes: liquids,
        no_diagram,
        refused,
        nearest,
        droplets,
        supercooled_tops,
        ices,
    } = tally;
    eprintln!(
        "{liquids} liquid modes ({no_diagram} of rows with no phase diagram); {} refused near \
         their critical point: {refused:?}; the modelled nearest at {:.4} T_c ({}); {droplets} \
         liquid water modes, {supercooled_tops} of them with a top below the triple point; {ices} \
         water ice modes",
        refused.len(),
        nearest.0,
        nearest.1
    );
    assert!(droplets > 0, "the table's Earth's droplets");
}

/// The relative move from `a` to `b`, 0 for two zeros.
fn moved(a: f64, b: f64) -> f64 {
    (b - a).abs() / a.abs().max(b.abs()).max(f64::MIN_POSITIVE)
}

/// The record's inventory is continuous in time (P14.T24.c's record leaves continuity in time and
/// across state changes to P14.T54.b, on the record): over the first ten whole systems under the
/// batch's rules, at the epoch and at the clock window's end, 1,000 years on, every surface in the
/// same state at both lists the same decks, modes (by material, phase and deck) and absorbers, and
/// each deck's base, each mode's τ(550) and each absorber's column moves by at most 10⁻³ of
/// itself, its surface temperature's and pressure's moves printed beside. A deck that formed or
/// vanished would show as lists that differ.
#[test]
fn the_record_s_inventory_is_continuous_in_time() {
    let (mut surfaces, mut largest, mut inputs) = (0_usize, 0.0_f64, 0.0_f64);
    for ((ctx, system), first) in whole().iter().zip(batch_snapshots()).take(10) {
        let later = system
            .clone()
            .with_record_rules(air_batch())
            .snapshot_at(ctx, ClockWindow::END);
        for (a, b) in first.bodies().iter().zip(later.bodies()) {
            let (Section::Ok(sa), Section::Ok(sb)) = (a.surface(), b.surface()) else {
                continue;
            };
            let (Modelled::Ok(ia), Modelled::Ok(ib)) = (sa.aerosols(), sb.aerosols()) else {
                continue;
            };
            if sa.state() != sb.state() {
                continue;
            }
            let id = a.identity().id();
            let decks = |i: &AerosolInventory| -> Vec<SubstanceId> {
                i.decks().iter().map(AerosolDeck::condensate).collect()
            };
            let modes = |i: &AerosolInventory| -> Vec<(SubstanceId, CondensedPhase, SubstanceId)> {
                i.modes()
                    .iter()
                    .map(|m| (m.material(), m.phase(), m.deck()))
                    .collect()
            };
            let species = |i: &AerosolInventory| -> Vec<SubstanceId> {
                i.absorbers().iter().map(Absorber::species).collect()
            };
            assert_eq!(decks(ia), decks(ib), "{id}");
            assert_eq!(modes(ia), modes(ib), "{id}");
            assert_eq!(species(ia), species(ib), "{id}");
            let mut shifts: Vec<f64> = Vec::new();
            for (x, y) in ia.decks().iter().zip(ib.decks()) {
                shifts.push(moved(x.base().value(), y.base().value()));
            }
            for (x, y) in ia.modes().iter().zip(ib.modes()) {
                match (x.optical_depth(), y.optical_depth()) {
                    (Ok(dx), Ok(dy)) => shifts.push(moved(dx.value(), dy.value())),
                    (Err(_), Err(_)) => {}
                    (dx, dy) => panic!("{id}: {dx:?} then {dy:?}"),
                }
            }
            for (x, y) in ia.absorbers().iter().zip(ib.absorbers()) {
                shifts.push(moved(x.column_per_m2(), y.column_per_m2()));
            }
            let (ca, cb) = (sa.conditions(), sb.conditions());
            inputs = inputs
                .max(moved(
                    ca.mean_surface_temperature().value(),
                    cb.mean_surface_temperature().value(),
                ))
                .max(moved(
                    ca.surface_pressure().value(),
                    cb.surface_pressure().value(),
                ));
            for shift in shifts {
                assert!(shift <= 1e-3, "{id}: a move of {shift}");
                largest = largest.max(shift);
            }
            surfaces += 1;
        }
    }
    eprintln!(
        "{surfaces} surfaces over 1,000 years: the inventory's largest move {largest:e}, the \
         surface temperature's or pressure's {inputs:e}"
    );
    assert!(surfaces > 5, "{surfaces}");
}

/// The members of `inventory` that P14.T24.g's sulphur does not set, by their `Debug` forms: the
/// decks but the acid's and SO₂'s, the modes that name neither (whole, or with `sizes` false by
/// their material, phase, deck, column mass, profile, coverage and temperature alone), the
/// absorbers but SO₂, and the condensates not formed but those two.
fn without_the_sulphur(inventory: &AerosolInventory, sizes: bool) -> String {
    let sulphur = [row("H2SO4"), row("SO2")];
    let decks: Vec<_> = inventory
        .decks()
        .iter()
        .filter(|d| !sulphur.contains(&d.condensate()))
        .collect();
    let modes: Vec<String> = inventory
        .modes()
        .iter()
        .filter(|m| !sulphur.contains(&m.deck()))
        .map(|m| {
            if sizes {
                format!("{m:?}")
            } else {
                format!(
                    "{:?} {:?} {:?} {:?} {:?} {:?} {:?}",
                    m.material(),
                    m.phase(),
                    m.deck(),
                    m.column_mass(),
                    m.profile(),
                    m.coverage(),
                    m.temperature()
                )
            }
        })
        .collect();
    let absorbers: Vec<_> = inventory
        .absorbers()
        .iter()
        .filter(|a| !sulphur.contains(&a.species()))
        .collect();
    let not_formed: Vec<_> = inventory
        .not_formed()
        .iter()
        .filter(|(id, _)| !sulphur.contains(id))
        .collect();
    format!("{decks:?} {modes:?} {absorbers:?} {not_formed:?}")
}

/// The record's inventory changes with its surface's state only through P14.T24.g's sulphur, whose
/// case the state and a runaway's deck set (`SulphurCycle::case_of`, P14.T54.b.g): over the first
/// ten whole systems under the batch's rules, each air's record members given each state that
/// holds an air, its photochemistry's star the Sun's, the airless state the empty inventory and a
/// magma ocean none, not modelled while it holds an air (R08's S5).
///
/// - Temperate and snowball, whose sulphur adds no SO₂ to the loop's air, keep every deck, mode,
///   absorber and condensate not formed but the sulphur's to the bit.
/// - A runaway greenhouse's cycling SO₂ joins the loop's air: the decks but the acid's and every
///   mode's column, profile, coverage and temperature keep their bits, but a mode's size and τ
///   move where the SO₂ is the air's most abundant gas, whose viscosity sets the sizes (a thin air
///   flipped to a runaway, which T24.g's SO₂ cap, with P14.T24.d's batch, bounds); counted.
/// - A runaway solved without Venus's deck (`RunawayDeck::Deckless`, P14.T53.b part 1 (b)) takes
///   case 2's dry branch, once through, as a temperate world's: its inventory is the temperate
///   one's to the bit.
#[test]
fn the_record_s_inventory_changes_with_the_state_only_in_its_sulphur() {
    let sun = Some(Kelvin::new(5_772.0));
    let mut airs = 0_usize;
    let mut resized = Vec::new();
    for (_, snapshot) in whole().iter().zip(batch_snapshots()).take(10) {
        for record in snapshot.bodies() {
            let Section::Ok(surface) = record.surface() else {
                continue;
            };
            if !matches!(surface.vertical_structure(), Modelled::Ok(Some(_))) {
                continue;
            }
            let id = record.identity().id();
            let structure = owned_structure(surface);
            let base = air_surface_of(record, surface, sun);
            let of_deck = |state, runaway_deck| match record_aerosols(
                AirRules::Batch,
                &AirSurface {
                    state,
                    runaway_deck,
                    ..base
                },
                &structure,
            ) {
                Modelled::Ok(inventory) => inventory,
                Modelled::NotModelled => panic!("{id}: {state:?} refused"),
            };
            let of = |state| of_deck(state, RunawayDeck::Kept);
            let temperate = of(SurfaceState::Temperate);
            assert_eq!(
                without_the_sulphur(&of(SurfaceState::Snowball), true),
                without_the_sulphur(&temperate, true),
                "{id}: a snowball"
            );
            let runaway = of(SurfaceState::RunawayGreenhouse);
            assert_eq!(
                without_the_sulphur(&runaway, false),
                without_the_sulphur(&temperate, false),
                "{id}: a runaway greenhouse"
            );
            if without_the_sulphur(&runaway, true) != without_the_sulphur(&temperate, true) {
                resized.push(id.to_string());
            }
            assert_eq!(
                of_deck(SurfaceState::RunawayGreenhouse, RunawayDeck::Deckless),
                temperate,
                "{id}: a runaway solved without the deck"
            );
            assert_eq!(
                of(SurfaceState::Airless),
                AerosolInventory::empty(),
                "{id}: airless"
            );
            let melt = AirSurface {
                state: SurfaceState::MagmaOcean,
                ..base
            };
            assert_eq!(
                record_aerosols(AirRules::Batch, &melt, &structure),
                Modelled::NotModelled,
                "{id}: a magma ocean"
            );
            airs += 1;
        }
    }
    eprintln!(
        "{airs} airs flipped; {} resized as runaways by their SO₂: {resized:?}",
        resized.len()
    );
    assert!(airs > 5, "{airs}");
}

/// Under the batch's rules, on the first three sample systems with moons, a snapshot is `body_at`
/// body by body, bit for bit (signed zeros included, by `Debug`, as
/// `a_snapshot_is_body_at_body_by_body` holds under the rules in force), and `body_at` gives each
/// body the same record whatever order its system's bodies are asked in: the air's sections and the
/// modifiers, which read a moon's planet's record and its zone's brightest star, keep nothing
/// between bodies (the sim-determinism skill's call-order rule).
#[test]
fn under_the_batch_a_snapshot_is_body_at_and_body_at_keeps_no_order() {
    let t = UniverseTime::EPOCH;
    let mut systems = 0;
    for ((ctx, system), snapshot) in whole().iter().zip(batch_snapshots()) {
        let has_moons = snapshot
            .bodies()
            .iter()
            .any(|record| CoarseInputs::planet_of(record).is_some());
        if !has_moons {
            continue;
        }
        let batch = system.clone().with_record_rules(air_batch());
        let indices = batch.indices();
        assert_eq!(snapshot.bodies().len(), indices.len());
        for (record, &index) in snapshot.bodies().iter().zip(&indices) {
            let alone = batch.body_at(ctx, index, t).expect("a body of its system");
            assert_eq!(format!("{record:?}"), format!("{alone:?}"));
        }
        assert_order_independent(&indices, |&index| {
            format!("{:?}", batch.body_at(ctx, index, t).expect("a body"))
        });
        systems += 1;
        if systems == 3 {
            break;
        }
    }
    assert_eq!(systems, 3, "the sample holds three systems with moons");
}
