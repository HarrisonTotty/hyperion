//! P14.T54.b.f's tests over the sample: the cold traps in the passes (decision-p14-t54b-f-coldtraps
//! §9), every 24 hold in force (`planetary::testing::with_every_24_hold`), the cold traps' batch
//! among them, the systems generated afresh inside the switches: each pass that qualifies builds
//! or reuses a field of its own inputs, the published pass's field rides to the record, which
//! builds none, and the record's condensates are that pass's to the bit.

use std::collections::BTreeMap;
use std::num::FpCategory;
use std::sync::OnceLock;

use hyperion_testkit::float::bits;

use super::*;
use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase, Condensates};
use crate::planetary::derive::atmosphere::spun_read::{self, SpunRead};
use crate::planetary::derive::atmosphere::{Gas, SurfaceMaterial, SurfaceState};
use crate::planetary::derive::passes::{ContextFit, Convergence};
use crate::planetary::derive::{PassDriver, Under};
use crate::planetary::hooks::cold_traps::field_log::{self, FieldEvent};
use crate::planetary::hooks::cold_traps::{
    ColdTrapRules, FieldInputs, NoColdTraps, PassTraps, is_collisional, same_craters,
    with_fields_rebuilt,
};
use crate::planetary::hooks::surface::HourCosines;
use crate::planetary::label;
use crate::planetary::system::tests::{SEED, sample, whole};
use crate::planetary::testing::with_every_24_hold;
use crate::time::UniverseTime;

/// How many of the sample's systems the tests build: the ruling's 60 (§9 test 1).
const TRAP_SAMPLE: usize = 60;

/// A body's kind, for the histogram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// A planet.
    Planet,
    /// A belt's member.
    Member,
    /// A moon.
    Moon,
}

/// A present body of a sample system with a solid surface, under every 24 hold at the epoch: its
/// system's context, its kind, its record and a moon's planet's, its derivation, the events of
/// its derivation's fields as its record derived it ([`field_log::capture`]), and its published
/// pass's publication, run again at the albedo and the context it read (`PassDriver::Again`).
pub(crate) struct TrapBody {
    /// Its system's context.
    pub(crate) ctx: &'static SystemContext,
    /// Its kind.
    pub(crate) kind: Kind,
    /// Its record.
    pub(crate) record: BodyRecord,
    /// A moon's planet's record.
    pub(crate) planet: Option<BodyRecord>,
    /// Its derivation.
    pub(crate) derived: DerivedBody,
    /// Its derivation's fields, built and reused, in order.
    pub(crate) events: Vec<FieldEvent>,
    /// Its published pass's publication, run again.
    pub(crate) again: Option<SpunRead>,
}

impl TrapBody {
    /// Its surface section, which every trap body has.
    pub(crate) fn surface(&self) -> &Surface {
        match self.record.surface() {
            Section::Ok(surface) => surface,
            other => panic!("{}: {:?}", self.record.identity().id(), other.state()),
        }
    }

    /// Its published pass's cold traps.
    pub(crate) fn traps(&self) -> &PassTraps {
        self.derived.cold_traps()
    }

    /// Whether its passes were bracketed.
    pub(crate) fn bracketed(&self) -> bool {
        matches!(self.derived.convergence(), Convergence::Bracketed { .. })
    }

    /// Its ID, for messages.
    pub(crate) fn id(&self) -> String {
        format!("{}", self.record.identity().id())
    }
}

/// The sample's first [`TRAP_SAMPLE`] systems, generated afresh under every 24 hold, built once:
/// call it inside the switches (a system's records derive lazily, under the rules then in force).
fn batch_systems() -> &'static [(&'static SystemContext, PlanetarySystem)] {
    static SYSTEMS: OnceLock<Vec<(&'static SystemContext, PlanetarySystem)>> = OnceLock::new();
    SYSTEMS.get_or_init(|| {
        with_every_24_hold(|| {
            sample()
                .iter()
                .take(TRAP_SAMPLE)
                .map(|ctx| (ctx, generate(SEED, ctx)))
                .collect()
        })
    })
}

/// The trap bodies of the sample's first [`TRAP_SAMPLE`] systems under every 24 hold, built once.
pub(crate) fn trap_bodies() -> &'static [TrapBody] {
    static BODIES: OnceLock<Vec<TrapBody>> = OnceLock::new();
    BODIES.get_or_init(|| {
        with_every_24_hold(|| {
            let before = field_log::record_builds();
            let bodies: Vec<TrapBody> = batch_systems()
                .iter()
                .flat_map(|(ctx, system)| bodies_of(ctx, system))
                .collect();
            assert_eq!(
                field_log::record_builds(),
                before,
                "the record path builds no field"
            );
            bodies
        })
    })
}

/// The trap bodies of `system`, of context `ctx`, at the epoch: its records as its snapshot builds
/// them, body by body, each with its derivation, its fields' events and its published pass run
/// again.
fn bodies_of(ctx: &'static SystemContext, system: &PlanetarySystem) -> Vec<TrapBody> {
    let t = UniverseTime::EPOCH;
    let epoch = Epoch::new(system, ctx, t).with_shades();
    let cosines = HourCosines::new();
    let hosts: Vec<_> = system
        .zones
        .iter()
        .map(|zone| fate_host(ctx, zone))
        .collect();
    let fates = system.fates(&hosts);
    let mut parents: Vec<(BodyIndex, BodyRecord, ParentNow, &BodyFate<'_>)> = Vec::new();
    let mut bodies = Vec::new();
    for (body, fate) in system.bodies.iter().zip(&fates) {
        let label = label::label(system, body.index).expect("every body has a label");
        match &body.part {
            Part::Planet(_) | Part::Member(_) => {
                let fate = fate.as_ref().expect("a planet or member orbits a zone");
                let ((record, now), events) = field_log::capture(|| {
                    system.primary_record(&epoch, &cosines, body, label, fate)
                });
                if let Some(derived) = &now.derived {
                    let at = fate.at(t);
                    let (_, again) = spun_read::capture(|| {
                        system.primary_derived(
                            &epoch,
                            &cosines,
                            (body, fate),
                            &at,
                            PassDriver::Again(Under::Read),
                        )
                    });
                    bodies.push(TrapBody {
                        ctx,
                        kind: match &body.part {
                            Part::Member(_) => Kind::Member,
                            Part::Planet(_) | Part::Moon(_) | Part::Ring(_) => Kind::Planet,
                        },
                        record: record.clone(),
                        planet: None,
                        derived: *derived,
                        events,
                        again,
                    });
                }
                parents.push((body.index, record, now, fate));
            }
            Part::Moon(_) => {
                let OrbitHost::Body(parent) = body.host else {
                    unreachable!("a moon's host is a body")
                };
                let (planet, now, fate) = parents
                    .iter()
                    .find(|(at, ..)| *at == parent)
                    .map(|(_, planet, now, fate)| (planet, now, *fate))
                    .expect("a moon's planet comes before it");
                let (record, events) = field_log::capture(|| {
                    system.satellite_record(&epoch, &cosines, body, label, now, fate)
                });
                if let Some(derived) = system.record_moon(&epoch, body, (now, fate)) {
                    let (_, again) = spun_read::capture(|| {
                        system.record_moon_driven(
                            &epoch,
                            body,
                            (now, fate),
                            PassDriver::Again(Under::Read),
                        )
                    });
                    bodies.push(TrapBody {
                        ctx,
                        kind: Kind::Moon,
                        record,
                        planet: Some(planet.clone()),
                        derived: *derived.body(),
                        events,
                        again,
                    });
                }
            }
            Part::Ring(_) => {}
        }
    }
    bodies.retain(|body| matches!(body.record.surface(), Section::Ok(_)));
    bodies
}

/// Whether two condensates are one to the bit: every entry's substance, phase, area, trapped area,
/// reservoir and seasonal flag, in order, and its whole `Debug` form; where `ice_bulk`, water's
/// entries aside, since the record lists an ice bulk's crust water, which its pass's area mix reads
/// as its regolith instead (decision-p14-t54a-ganymede §C1.4; rule (f)'s own test,
/// `same_publication`, sets them aside alike).
fn same_condensates(a: &Condensates, b: &Condensates, ice_bulk: bool) -> bool {
    let kept = |c: &Condensates| -> Vec<CondensateEntry> {
        c.entries()
            .iter()
            .filter(|e| !(ice_bulk && e.substance() == Gas::Water.substance()))
            .copied()
            .collect()
    };
    let (a, b) = (kept(a), kept(b));
    a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| {
            x.substance() == y.substance()
                && x.phase() == y.phase()
                && bits(x.area_fraction()) == bits(y.area_fraction())
                && bits(x.trapped_area_fraction()) == bits(y.trapped_area_fraction())
                && bits(x.reservoir().value()) == bits(y.reservoir().value())
                && x.is_seasonal() == y.is_seasonal()
        })
        && format!("{a:?}") == format!("{b:?}")
}

/// The trapped entries of `condensates`: each solid entry's substance and trapped area above zero.
fn trapped(condensates: &Condensates) -> Vec<(String, u64)> {
    condensates
        .entries()
        .iter()
        .filter(|e| e.phase() == CondensatePhase::Solid && e.trapped_area_fraction() > 0.0)
        .map(|e| {
            (
                e.substance().substance().key().as_str().to_owned(),
                bits(e.trapped_area_fraction()),
            )
        })
        .collect()
}

/// §9 test 1, with test 8's sample half: over the sample's first 60 systems under every 24 hold,
/// every body's record condensates are its published pass's to the bit (keys, phases, areas,
/// trapped areas and reservoirs, its whole entries; an ice bulk's water aside, whose crust the
/// record lists), bracketed bodies included, and no trapped entry lies on one side alone, an ice
/// bulk's water included. A qualifying airless body whose column holds nothing that
/// condenses (L2's case) keeps its delivered water and carbon dioxide traps on its pass and its
/// record wherever its field traps them, a species its column holds aside (it takes what is left
/// of its condensed reservoir, none where nothing condenses).
#[test]
fn under_the_batch_every_record_s_condensates_are_its_published_pass_s() {
    let bodies = trap_bodies();
    let (mut checked, mut qualifying, mut bracketed, mut l2) = (0_u32, 0_u32, 0_u32, 0_u32);
    let mut mismatches = Vec::new();
    for body in bodies {
        let Some(again) = &body.again else {
            continue;
        };
        let record = body.surface().condensates();
        let ice_bulk = body.surface().material() == SurfaceMaterial::Ice;
        checked += 1;
        if !same_condensates(record, &again.condensates, ice_bulk)
            || trapped(record) != trapped(&again.condensates)
        {
            mismatches.push(format!(
                "{} ({:?}): record {record:?}; pass {:?}",
                body.id(),
                body.traps().as_ref().err(),
                again.condensates
            ));
        }
        let Ok(traps) = body.traps() else {
            continue;
        };
        qualifying += 1;
        bracketed += u32::from(body.bracketed());
        let air = body.derived.atmosphere();
        if !air.column_inventory().condenses() {
            l2 += 1;
            for gas in [Gas::Water, Gas::CarbonDioxide] {
                // A species its column holds takes what is left of its condensed reservoir, none
                // where nothing condenses; a delivered one, its placeholder column (T51.f).
                if air.partial_pressures().of(gas).value() > 0.0 {
                    continue;
                }
                let f_ct = traps.field().trap(gas.substance()).map_or(0.0, |t| t.1);
                let entry = record.entry(gas.substance(), CondensatePhase::Solid);
                assert_eq!(
                    entry.is_some_and(|e| e.trapped_area_fraction() > 0.0),
                    f_ct > 0.0,
                    "{}: {gas:?}'s delivered traps, f_ct {f_ct:e}: {record:?}",
                    body.id()
                );
            }
        }
    }
    eprintln!(
        "{checked} records against their published passes; {qualifying} read a field ({bracketed} \
         bracketed); {l2} of those hold nothing that condenses (L2's case)"
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert!(checked > 500 && qualifying > 20, "{checked} {qualifying}");
}

/// The runs of equal inputs over a derivation's events, a slot's runs restarting where a slot
/// opens: how many fields were built, how many runs of equal inputs the qualifying passes made,
/// and how many qualifying passes there were.
fn runs_of(events: &[FieldEvent]) -> (u32, u32, u32) {
    let (mut builds, mut runs, mut passes) = (0_u32, 0_u32, 0_u32);
    let mut last: Option<&FieldInputs> = None;
    for event in events {
        let inputs: &FieldInputs = match event {
            FieldEvent::Opened => {
                last = None;
                continue;
            }
            FieldEvent::Built(inputs) => {
                builds += 1;
                inputs
            }
            FieldEvent::Reused(inputs) => inputs,
        };
        passes += 1;
        if !last.is_some_and(|held| held.same(inputs)) {
            runs += 1;
        }
        last = Some(inputs);
    }
    (builds, runs, passes)
}

/// §9 test 2: for each body the fields built equal the runs of equal inputs over its qualifying
/// passes, and the record path builds none (asserted where the bodies are built, and here: no
/// trapped entry without a field); the histogram of builds per body, by kind, is stated, with the
/// published passes' reasons for no field.
#[test]
fn the_passes_build_one_field_per_run_of_equal_inputs_and_the_record_none() {
    let bodies = trap_bodies();
    let mut histogram: BTreeMap<(Kind, u32), u32> = BTreeMap::new();
    let mut reasons: BTreeMap<(Kind, &'static str), u32> = BTreeMap::new();
    let (mut builds, mut passes) = (0_u32, 0_u32);
    for body in bodies {
        let (built, runs, qualifying) = runs_of(&body.events);
        assert_eq!(built, runs, "{}: {} events", body.id(), body.events.len());
        builds += built;
        passes += qualifying;
        if qualifying > 0 {
            *histogram.entry((body.kind, built)).or_default() += 1;
        }
        let reason = match body.traps() {
            Ok(_) => "read a field",
            Err(why) => why.label(),
        };
        *reasons.entry((body.kind, reason)).or_default() += 1;
        if body.traps().is_err() {
            assert!(
                trapped(body.surface().condensates()).is_empty(),
                "{}: a trapped entry with no field",
                body.id()
            );
        }
    }
    eprintln!("{builds} fields built over {passes} qualifying passes");
    eprintln!("builds per qualifying body, by kind: {histogram:?}");
    eprintln!("published passes by their cold traps: {reasons:?}");
    assert!(builds > 0 && passes >= builds, "{builds} {passes}");
}

/// The census the bless note takes (decision-p14-t54b-f-coldtraps §6; P14.T54.b.e's finding),
/// stated, not asserted: the airless bodies whose air's surface pressure is exactly 0 over partial
/// pressures above 0, which `is_collisional_air` calls collisional by 0 ≥ 0, so that they take no
/// cold traps; and every qualifying body with an `H2O` lid, its traps on T49.i's 0.832 frost as
/// the lid's row, pending P14.T49.m: `f_ct` and `A_s` by species and its zonal entries.
#[test]
fn the_cold_traps_census_is_stated() {
    let bodies = trap_bodies();
    let (mut zero_pressure, mut zero_collisional) = (Vec::new(), 0_u32);
    for body in bodies {
        let air = body.derived.atmosphere();
        let gravity = body.derived.surface_gravity();
        if air.state() == SurfaceState::Airless
            && air
                .surface_pressure()
                .is_some_and(|p| p.value().classify() == FpCategory::Zero)
            && Gas::ALL
                .iter()
                .any(|&gas| air.partial_pressures().of(gas).value() > 0.0)
        {
            zero_collisional += u32::from(is_collisional(air, gravity));
            zero_pressure.push(format!(
                "{} ({:?}, collisional {})",
                body.id(),
                body.kind,
                is_collisional(air, gravity)
            ));
        }
    }
    eprintln!(
        "airless with a pressure of exactly 0 over partials above 0: {} ({zero_collisional} of \
         them collisional by 0 ≥ 0): {zero_pressure:?}",
        zero_pressure.len()
    );
    for body in bodies {
        let (Ok(traps), Some(lid)) = (body.traps(), body.derived.crust_lid()) else {
            continue;
        };
        if lid.row() != Gas::Water.substance() {
            continue;
        }
        let f_x = traps.inputs().sky().exposed_fraction;
        let entries: Vec<String> = body
            .surface()
            .condensates()
            .entries()
            .iter()
            .filter(|e| e.phase() == CondensatePhase::Solid)
            .map(|e| {
                format!(
                    "{} f_ct {:.4e} A_s {:.4e} zonal {:.4e}",
                    e.substance().substance().key(),
                    e.trapped_area_fraction() / f_x,
                    e.trapped_area_fraction(),
                    e.area_fraction() - e.trapped_area_fraction()
                )
            })
            .collect();
        eprintln!(
            "pending P14.T49.m, an H2O lid on T49.i's frost: {} ({:?}) regolith A {:.4}, f_x \
             {f_x:.4}: {entries:?}",
            body.id(),
            body.kind,
            traps.inputs().regolith_albedo()
        );
    }
}

/// §9 test 3: with every qualifying pass building its field afresh and with the slot's reuse, every
/// derivation of the sample is the same to the bit, its pass count, its convergence and context
/// fit and its contexts included.
#[test]
fn fields_reused_and_rebuilt_give_every_derivation_to_the_bit() {
    let (mut checked, mut fields) = (0_u32, 0_u32);
    with_every_24_hold(|| {
        for (ctx, system) in batch_systems() {
            let t = UniverseTime::EPOCH;
            let epoch = Epoch::new(system, ctx, t).with_shades();
            let cosines = HourCosines::new();
            let hosts: Vec<_> = system
                .zones
                .iter()
                .map(|zone| fate_host(ctx, zone))
                .collect();
            let fates = system.fates(&hosts);
            for (body, fate) in system.bodies.iter().zip(&fates) {
                if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
                    continue;
                }
                let fate = fate.as_ref().expect("a planet or member orbits a zone");
                let at = fate.at(t);
                if at.orbit().is_none() {
                    // Not present at the time: no derivation.
                    continue;
                }
                let derive = || {
                    system
                        .primary_derived(&epoch, &cosines, (body, fate), &at, PassDriver::Solve)
                        .0
                };
                let (warm, cold) = (derive(), with_fields_rebuilt(derive));
                assert_eq!(format!("{warm:?}"), format!("{cold:?}"), "{:?}", body.index);
                checked += 1;
                fields += u32::from(warm.cold_traps().is_ok());
            }
        }
    });
    eprintln!("{checked} derivations, {fields} with a field, warm against cold");
    assert!(checked > 300 && fields > 10, "{checked} {fields}");
}

/// §9 test 4: the sample's systems' snapshots, taken forward and reversed, are the same to the
/// bit, and so is one system's taken twice with another's between, on the same thread.
#[test]
fn under_the_batch_the_snapshots_do_not_depend_on_their_order() {
    with_every_24_hold(|| {
        let systems = batch_systems();
        let snapshot = |(ctx, system): &(&SystemContext, PlanetarySystem)| -> String {
            format!("{:?}", system.snapshot_at(ctx, UniverseTime::EPOCH))
        };
        let forward: Vec<String> = systems.iter().map(snapshot).collect();
        let mut reversed: Vec<String> = systems.iter().rev().map(snapshot).collect();
        reversed.reverse();
        for (k, (a, b)) in forward.iter().zip(&reversed).enumerate() {
            assert!(a == b, "system {k} moved with the order");
        }
        let first = snapshot(&systems[0]);
        // Another system between, whose snapshot is not read.
        let between = snapshot(&systems[1]);
        assert!(between == forward[1], "a later run moved");
        assert!(first == snapshot(&systems[0]), "a second run moved");
        assert!(first == forward[0], "a later run moved");
    });
}

/// §9 test 6: for every body whose published pass read a field, bracketed ones included, the
/// carried field is a fresh build over its inputs to the bit, and its inputs are that pass's: its
/// regolith's albedo the derivation's, and the heat flow, craters and crust of the context it read.
#[test]
fn every_carried_field_is_its_published_pass_s() {
    let (mut checked, mut bracketed) = (0_u32, 0_u32);
    for body in trap_bodies() {
        let Ok(traps) = body.traps() else {
            continue;
        };
        let inputs = traps.inputs();
        assert!(
            inputs.field().same_bits(traps.field()),
            "{}: the carried field is not its inputs'",
            body.id()
        );
        let spun = body.derived.spun().expect("a field is read over a record");
        let context = spun.context();
        let first = context.figures().expect("a field's context holds figures");
        assert_eq!(
            bits(inputs.regolith_albedo()),
            bits(body.derived.regolith_albedo()),
            "{}",
            body.id()
        );
        assert_eq!(
            bits(inputs.heat_flow().value()),
            bits(context.heat_flow().value()),
            "{}",
            body.id()
        );
        assert!(
            same_craters(inputs.craters(), first.craters()),
            "{}",
            body.id()
        );
        assert_eq!(inputs.crust(), first.crust(), "{}", body.id());
        checked += 1;
        bracketed += u32::from(body.bracketed());
    }
    eprintln!("{checked} carried fields, {bracketed} of bracketed bodies");
    assert!(checked > 20, "{checked}");
}

/// §9 test 7: no body whose record has figures publishes a pass whose context holds none; a pass
/// under C₀ of a qualifying body reads no field (`NoColdTraps::NoFigures`) and publishes the
/// condensates the hold gives it, P14.T51.a's; and a climate given (the passes' stand-in, the
/// table's bodies) reads none, labelled "(no cold traps)".
#[test]
fn no_published_pass_reads_no_figures_and_a_figures_free_pass_publishes_t51a_s() {
    let bodies = trap_bodies();
    let mut over_records = 0_u32;
    for body in bodies {
        let Some(spun) = body.derived.spun() else {
            continue;
        };
        over_records += 1;
        if spun.next().figures().is_some() {
            assert!(
                spun.context().figures().is_some(),
                "{}: a figures-free published pass",
                body.id()
            );
        }
    }
    let c0 = qualifying_passes_under_c0_publish_t51a_s();
    with_every_24_hold(|| {
        for (name, body) in crate::planetary::derive::solar::historic_table() {
            assert_eq!(
                body.cold_traps().as_ref().err(),
                Some(&NoColdTraps::ClimateGiven),
                "{name}"
            );
        }
    });
    assert!(
        NoColdTraps::ClimateGiven
            .label()
            .contains("(no cold traps)")
    );
    eprintln!("{over_records} bodies over their records; {c0} qualifying bodies' passes under C0");
    assert!(over_records > 500 && c0 > 5, "{over_records} {c0}");
}

/// §9 test 7's second half: over the first 20 systems, each qualifying planet's and member's pass
/// at its published albedo under C₀ (`PassDriver::Once`) reads no field, for want of figures where
/// it qualifies, and publishes the condensates the cold traps' hold gives that pass, to the bit:
/// how many read none for want of figures.
fn qualifying_passes_under_c0_publish_t51a_s() -> u32 {
    let mut c0 = 0_u32;
    with_every_24_hold(|| {
        for (ctx, system) in batch_systems().iter().take(20) {
            let held = system.clone().with_record_rules(RecordRules {
                cold_traps: ColdTrapRules::Held,
                ..system.record
            });
            let t = UniverseTime::EPOCH;
            let hosts: Vec<_> = system
                .zones
                .iter()
                .map(|zone| fate_host(ctx, zone))
                .collect();
            let fates = system.fates(&hosts);
            for (body, fate) in system.bodies.iter().zip(&fates) {
                if !matches!(body.part, Part::Planet(_) | Part::Member(_)) {
                    continue;
                }
                let fate = fate.as_ref().expect("a planet or member orbits a zone");
                let at = fate.at(t);
                if at.orbit().is_none() {
                    continue;
                }
                let derive = |system: &PlanetarySystem, driver| {
                    let epoch = Epoch::new(system, ctx, t).with_shades();
                    let cosines = HourCosines::new();
                    spun_read::capture(|| {
                        system
                            .primary_derived(&epoch, &cosines, (body, fate), &at, driver)
                            .0
                    })
                };
                let (solved, _) = derive(system, PassDriver::Solve);
                if solved.cold_traps().is_err() {
                    continue;
                }
                // One pass under C₀ at the published albedo, under the batch and under the hold.
                let driver = PassDriver::Once(solved.albedo());
                let ((batch, read), (_, held_read)) =
                    (derive(system, driver), derive(&held, driver));
                // Under C₀ its air may not qualify (its night floor reads no heat flow); where it
                // does, it reads no field for want of figures.
                let why = batch.cold_traps().as_ref().err().copied();
                assert!(
                    matches!(why, Some(NoColdTraps::NoFigures | NoColdTraps::Unqualified)),
                    "{:?}: {why:?}",
                    body.index
                );
                let (Some(read), Some(held_read)) = (read, held_read) else {
                    panic!("{:?}: a solid surface's pass publishes", body.index);
                };
                assert!(
                    same_condensates(&read.condensates, &held_read.condensates, false),
                    "{:?}: {:?} against {:?}",
                    body.index,
                    read.condensates,
                    held_read.condensates
                );
                c0 += u32::from(why == Some(NoColdTraps::NoFigures));
            }
        }
    });
    c0
}

/// §9 test 9: at generator version 23, the cold traps held, the passes build and reuse no field,
/// and no record's basis carries one, over the first 20 whole systems (the cache's, version 23's).
#[test]
fn held_the_passes_build_no_field() {
    assert_eq!(ColdTrapRules::in_force(), ColdTrapRules::Held);
    let mut surfaces = 0_u32;
    for (ctx, system) in whole().iter().take(20) {
        let (snapshot, events) =
            field_log::capture(|| system.snapshot_at(ctx, UniverseTime::EPOCH));
        assert!(
            events
                .iter()
                .all(|event| matches!(event, FieldEvent::Opened)),
            "{:?}: a field under the hold",
            ctx.id()
        );
        for record in snapshot.bodies() {
            if let Section::Ok(surface) = record.surface() {
                surfaces += 1;
                assert!(
                    surface
                        .basis()
                        .is_none_or(|basis| basis.cold_traps.is_none()),
                    "{}",
                    record.identity().id()
                );
                assert_eq!(
                    trapped(surface.condensates()),
                    Vec::new(),
                    "{}",
                    record.identity().id()
                );
            }
        }
    }
    assert!(surfaces > 50, "{surfaces}");
}

/// The bodies' fits under the bracket's second round, for the placement test's list.
#[must_use]
pub(crate) fn fit_of(body: &TrapBody) -> Option<ContextFit> {
    match body.derived.convergence() {
        Convergence::Converged => None,
        Convergence::Bracketed { fit, .. } => Some(fit),
    }
}
