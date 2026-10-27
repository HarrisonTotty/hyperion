//! P14.T54.b.k's audit of the 23 → 24 batch (design-k §2.1, §3.1): the MUST-FIX items of
//! brief-notes-24 that a record shows, asserted over one walk with every 24 hold in force and the
//! record's rules at their batch ([`every_record_batch`]), so that the air's sections, the
//! envelope, the modifiers and the taxonomic class are on every record the walk reads, as 24 will
//! publish them:
//!
//! - **M1**, P14.T53.b part 1 (V3.9 part 1): no runaway beyond the maximum-greenhouse limit now
//!   or under Venus's deck below 430 K, and every runaway between the limits (c)'s, its hothouses
//!   named with their decks and their sides of water's saturation temperature
//!   (decision-p14-t54b-k-hothouses §3);
//! - **M3**, P14.T53.a's K4 ceiling: no body beyond the maximum-greenhouse limit now lies above
//!   max(273.15 K, `T_rest`) (decision-absurd-states-24 R4.2; decision-bump-plan-after-22 K4's
//!   test 3);
//! - **M4**, P14.T53.b part 2: no `Ok` surface under a supercritical reservoir
//!   (`assert_envelope_tags`' rule, under the air's batch);
//! - **M6**, P14.T56.g: every lagged record's surface rock, `Mercury`'s where airless, and no
//!   airless record with an `Ice` surface above the melting point at T0;
//! - **M7**, P14.T51.g.b's invariants on every published ocean, its floor under its own bound
//!   (main's rulings on T51.g.a's MUST-FIX and on k-1's audit, 2026-10-27);
//! - **M9**, P14.T54.b.e's young lava: no airless silicate lid younger than a regolith publishes a
//!   regolith above 0.11;
//! - **M10**, decision-p14-t53c-tenuous-aerosols: every `Tenuous` record's aerosols not modelled;
//! - **M11**, the V3.8 valve's precondition (decision-p14-t54b-open §1.2.5): every new member
//!   finite and in its range.
//!
//! The census set ([`census_sets`]) is design-k §0's set C, every committed 24 census's: the first
//! 60 sample systems, the 15 golden systems and LOOK72's systems ([`look72_audit_contexts`]: the 27
//! of its triaged bodies and the three of its named oceans); the audit set ([`audit_sets`]) is its
//! set A, the 400, the goldens and LOOK72's, run by name, walked once a process for both of its
//! tests ([`audit_set_census`]).

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Instant;

use hyperion_testkit::float::bits;

use super::tests::{
    EnvelopeBody, GOLDEN_SYSTEMS_SEED, HOTHOUSE_BODY, LOOK72_OCEANS, SEED, envelope_tag_faults,
    every_record_batch, golden_contexts, limit_now, look72_audit_contexts, look72_galaxy,
    runaway_at_peak, runaway_now, sample,
};
use super::*;
use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::derive::SurfaceLag;
use crate::planetary::derive::atmosphere::{
    ACID_CYCLING_TEMPERATURE, Absorber, Gas, GreenhouseCeiling, GreenhouseConstants, PresentState,
    RunawayDeck, SurfaceMaterial, SurfaceState, WATER_MELTING_POINT, grey_surface_temperature,
    solved_as_a_hothouse, water_saturation_temperature, with_present_state,
};
use crate::planetary::derive::habitable_zone::HabitableLimit;
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::derive::photometry::PhaseTemplate;
use crate::planetary::derive::photometry::area_mix::{
    CAPPED_ROWS, REGOLITH_FORMATION_TIME, StandIn, capped_lid, sun_bands,
};
use crate::planetary::hooks::crust::{CrustLithology, LidCrust};
use crate::planetary::hooks::figures::{OceanFloor, Subsurface, TectonicRegime};
use crate::planetary::hooks::modifiers::{VolcanicDeposit, WeatheringDeposit};
use crate::planetary::hooks::ocean::{WaterColumn, implausibility};
use crate::planetary::hooks::structure::VerticalStructure;
use crate::planetary::hooks::surface::night_floor;
use crate::planetary::moons::regular::WATER_DENSITY;
use crate::planetary::record::{BulkProperties, Modelled};
use crate::planetary::system::moon_ice::HOT_ICE_LIMIT;
use crate::planetary::testing::with_every_24_hold;
use crate::substance::Element;
use crate::substance::phase::{high_pressure_melting, ice_ih_melting_temperature};
use crate::units::{Kelvin, Kilograms, Metres, Pascals};

/// One system of a census: its set ("sample", the golden system's name, or "look72"), its
/// universe's seed and its context.
type AuditSet = (&'static str, Seed, SystemContext);

/// Design-k §0's set C ([module](self)): the first 60 sample systems, the 15 golden systems and
/// LOOK72's 30 ([`look72_audit_contexts`]).
fn census_sets() -> Vec<AuditSet> {
    sets_of(60)
}

/// Design-k §0's set A ([module](self)): the 400 sample systems, the golden systems and LOOK72's
/// 30.
fn audit_sets() -> Vec<AuditSet> {
    sets_of(sample().len())
}

/// The first `samples` sample systems, the golden systems and LOOK72's audit systems, in that
/// order.
fn sets_of(samples: usize) -> Vec<AuditSet> {
    let look72_seed = look72_galaxy().seed();
    sample()
        .iter()
        .take(samples)
        .map(|ctx| ("sample", SEED, ctx.clone()))
        .chain(
            golden_contexts()
                .into_iter()
                .map(|(name, ctx)| (name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx)),
        )
        .chain(
            look72_audit_contexts()
                .into_iter()
                .map(|ctx| ("look72", look72_seed, ctx)),
        )
        .collect()
}

/// One present planet, member or moon with a bulk section and a derivation at the epoch, as
/// [`audit_walk`] gives it: its record as `snapshot_at` builds it (the epoch with its shades and
/// its small bodies' rules), its derivation and what the items read beside them.
struct AuditCase<'a> {
    /// Its set ([`AuditSet`]).
    set: &'a str,
    /// The body.
    body: &'a Body,
    /// "planets", "members" or "moons".
    kind: &'static str,
    /// Its record at the epoch.
    record: &'a BodyRecord,
    /// Its derivation over its record.
    derived: &'a DerivedBody,
    /// Its primary's state at the epoch (a moon's planet's).
    now: &'a ParentNow,
    /// Σ Fᵢ ÷ `S_eff` at the runaway limit now ([`runaway_now`]), at the runaway limit at the hosts'
    /// peak ([`runaway_at_peak`]) and at the maximum-greenhouse limit now ([`limit_now`]): a moon's
    /// its planet's.
    ratios: [f64; 3],
    /// Its derivation again, its passes driven by a driver (M1's first solve).
    again: &'a dyn Fn(PassDriver) -> DerivedBody,
}

impl AuditCase<'_> {
    /// "{set} {id} {kind}", by which every list names it.
    fn label(&self) -> String {
        format!("{} {} {}", self.set, self.record.identity().id(), self.kind)
    }

    /// Its record's surface, `None` where it is not `Ok`.
    fn surface(&self) -> Option<&Surface> {
        self.record.surface().ok()
    }

    /// Its zero-albedo equilibrium temperature T0, P14.T56.g's (`moon_ice/lag_tests.rs`'s
    /// census's): a moon's its parent's moon ice's at its orbit, where its parent's moon bulk reads
    /// one, else its derivation's flux at a Bond albedo of 0.
    fn t0(&self) -> Kelvin {
        let bare = || equilibrium_temperature(self.derived.flux(), BondAlbedo::from_fraction(0.0));
        match &self.body.part {
            Part::Moon(part) => self
                .now
                .moon_ice
                .as_ref()
                .and_then(|ice| ice.ice)
                .map_or_else(bare, |ice| ice.t0(&part.orbit)),
            Part::Planet(_) | Part::Member(_) | Part::Ring(_) => bare(),
        }
    }
}

/// Every present planet, member and moon of `system` (of the set `set` and context `ctx`) with a
/// bulk section and a derivation at the epoch, whatever its surface's tag, in body order, each
/// given to `visit` ([`AuditCase`]); its records under the system's own record rules.
fn audit_walk_system(
    set: &str,
    ctx: &SystemContext,
    system: &PlanetarySystem,
    visit: &mut dyn FnMut(&AuditCase<'_>),
) {
    let t = UniverseTime::EPOCH;
    if ctx.age_at_epoch().value() <= 0.0 {
        return;
    }
    let epoch = Epoch::new(system, ctx, t)
        .with_shades()
        .with_small_bodies(system.record.small_bodies);
    let cosines = HourCosines::new();
    for body in system.bodies() {
        let kind = match body.part {
            Part::Planet(_) => "planets",
            Part::Member(_) => "members",
            Part::Moon(_) | Part::Ring(_) => continue,
        };
        let zone = system.zone_of(body);
        let host = fate_host(ctx, zone);
        let fate = system.fate_of(body, &host);
        let ratios = fate.at(t).orbit().map_or([f64::NAN; 3], |orbit| {
            [
                runaway_now(&epoch, zone, orbit),
                runaway_at_peak(&epoch, zone, orbit),
                limit_now(HabitableLimit::MaximumGreenhouse, &epoch, zone, orbit),
            ]
        });
        let label = label::label(system, body.index).expect("every body has a label");
        let (record, now) = system.primary_record(&epoch, &cosines, body, label, &fate);
        if let (Section::Ok(_), Some(derived)) = (record.bulk(), now.derived.as_ref()) {
            let at = fate.at(t);
            let again = |driver| {
                system
                    .primary_derived(&epoch, &cosines, (body, &fate), &at, driver)
                    .0
            };
            visit(&AuditCase {
                set,
                body,
                kind,
                record: &record,
                derived,
                now: &now,
                ratios,
                again: &again,
            });
        }
        for child in system.children(body.index) {
            if !matches!(child.part, Part::Moon(_)) {
                continue;
            }
            let label = label::label(system, child.index).expect("every body has a label");
            let moon_record = system.satellite_record(&epoch, &cosines, child, label, &now, &fate);
            if moon_record.bulk().ok().is_none() {
                continue;
            }
            let Some(moon) = system.record_moon(&epoch, child, (&now, &fate)) else {
                continue;
            };
            let again = |driver| {
                *system
                    .record_moon_driven(&epoch, child, (&now, &fate), driver)
                    .expect("a moon derived once is derived again")
                    .body()
            };
            visit(&AuditCase {
                set,
                body: child,
                kind: "moons",
                record: &moon_record,
                derived: moon.body(),
                now: &now,
                ratios,
                again: &again,
            });
        }
    }
}

/// Every system of `sets` generated afresh at its seed under the rules in force on the calling
/// thread, its records under [`every_record_batch`], walked ([`audit_walk_system`]), in order; to
/// be called inside [`with_every_24_hold`], since the tests' caches are version 23's.
fn audit_walk(sets: &[AuditSet], visit: &mut dyn FnMut(&AuditCase<'_>)) {
    let rules = every_record_batch();
    for (set, seed, ctx) in sets {
        let system = generate(*seed, ctx).with_record_rules(rules);
        audit_walk_system(set, ctx, &system, visit);
    }
}

/// A MUST-FIX item the audit asserts ([module](self)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Item {
    M1,
    M3,
    M4,
    M6,
    M7,
    M9,
    M10,
    M11,
}

/// A runaway's side of water's saturation temperature at its surface pressure, its passes' mean
/// surface temperature against `water_saturation_temperature` there (P14.T53.b part 1 (c)'s
/// convention; count (xi)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Saturation {
    /// Below it.
    Under,
    /// At or above it, or not judged (a pressure under water's triple point).
    Above,
}

/// Arm (c)'s hothouses over design-k §0's set A, by their sample planets' IDs, each with its deck
/// and its side of water's saturation temperature (decision-p14-t54b-k-hothouses §1, §3: each a
/// dry past runaway between the limits now, judged on its first, deckless solve).
const AUDIT_SET_HOTHOUSES: [(&str, RunawayDeck, Saturation); 6] = [
    (
        "02002b2ca0010000.0200",
        RunawayDeck::Kept,
        Saturation::Under,
    ),
    (
        "02000b2d5ffd0000.0100",
        RunawayDeck::Kept,
        Saturation::Under,
    ),
    (
        "02002b2bdffd0000.0100",
        RunawayDeck::Kept,
        Saturation::Under,
    ),
    (
        "02002b2ca0030001.0300",
        RunawayDeck::Deckless,
        Saturation::Above,
    ),
    (
        "02000b2d5ffd0000.0200",
        RunawayDeck::Deckless,
        Saturation::Above,
    ),
    (HOTHOUSE_BODY, RunawayDeck::Kept, Saturation::Above),
];

/// The ocean of P14.T51.g.b's deepest ice V floor over design-k §0's set A, a sample planet's,
/// stated by name (main's M7 ruling, 2026-10-27): a Rocky `Temperate`'s, 61.4 km under a 0.1 km
/// shell, its floor at 627.11 MPa, past T51.g.a's 626.09 MPa and at its own bound.
const DEEPEST_ICE_V_FLOOR: &str = "41ffecb600000001.0200";

/// What the audit holds of one body ([`AuditCase`]): each item's failures on it and each line an
/// item states of it by name, so that a census of thousands keeps no record.
struct AuditBody {
    /// "{set} {id} {kind}".
    label: String,
    /// Whether its record's surface is `Ok`.
    surface: bool,
    /// Each item's failure on it, by its line.
    faults: Vec<(Item, String)>,
    /// Each list an item states by name, the list's title and its line.
    stated: Vec<(Item, &'static str, String)>,
    /// For a runaway between the limits now (M1 (c)), its deck and its side of water's
    /// saturation temperature; `None` for every other body.
    hothouse: Option<(RunawayDeck, Saturation)>,
}

impl AuditBody {
    /// The audit of the body `case`.
    fn of(case: &AuditCase<'_>) -> Self {
        let mut body = Self {
            label: case.label(),
            surface: case.surface().is_some(),
            faults: Vec::new(),
            stated: Vec::new(),
            hothouse: None,
        };
        body.runaway(case);
        body.ceiling(case);
        body.envelope_tags(case);
        body.lag(case);
        body.ocean(case);
        body.young_lava(case);
        body.tenuous(case);
        body.finite(case);
        body
    }

    /// Adds a failure of `item`.
    fn fault(&mut self, item: Item, line: &str) {
        self.faults.push((item, format!("{}: {line}", self.label)));
    }

    /// Adds a line of `item`'s list `list`.
    fn state(&mut self, item: Item, list: &'static str, line: &str) {
        self.stated
            .push((item, list, format!("{}: {line}", self.label)));
    }

    /// M1, P14.T53.b part 1 (V3.9 part 1; decision-t53b-arm-c §5.4–5.5; decision-p14-t54b-k-hothouses
    /// §3), on a record whose surface is a `RunawayGreenhouse`: (a) it lies inside the
    /// maximum-greenhouse limit now ([`limit_now`] at least 1); (b) under Venus's deck its mean
    /// surface temperature is at least the acid's cycling temperature, 430 K; (c) between the limits
    /// now ([`runaway_now`] below 1) it is (c)'s, its first solve with (c) off
    /// (`PresentState::WithoutHothouse`) a `Temperate` that (c) judges a hothouse
    /// (`solved_as_a_hothouse`). Each of (c)'s is stated by name with both solves' figures, and its
    /// deck and its side of water's saturation temperature are held
    /// ([`hothouse`](Self::hothouse)).
    fn runaway(&mut self, case: &AuditCase<'_>) {
        let Some(surface) = case.surface() else {
            return;
        };
        if surface.state() != SurfaceState::RunawayGreenhouse {
            return;
        }
        let saturation =
            |p: f64| water_saturation_temperature(Pascals::new(p)).map_or(f64::NAN, Kelvin::value);
        let air = case.derived.atmosphere();
        let deck = air.runaway_deck();
        let t_s = surface.conditions().mean_surface_temperature().value();
        let air_t_s = air.surface_temperature().value();
        let p_s = air.surface_pressure().map_or(f64::NAN, Pascals::value);
        let [now, peak, maximum] = case.ratios;
        let line = format!(
            "{deck:?} T_s {t_s:.2} K (passes' {air_t_s:.2} K), A {:.4}, p_s {:.4e} bar (T_sat \
             {:.3} K); runaway now {now:.4}, at the peak {peak:.4}, maximum now {maximum:.4}",
            case.derived.albedo().value(),
            p_s / 1e5,
            saturation(p_s)
        );
        if maximum.is_nan() || maximum < 1.0 {
            self.fault(
                Item::M1,
                &format!("(a) a runaway beyond the maximum-greenhouse limit now: {line}"),
            );
            return;
        }
        if deck == RunawayDeck::Kept && (t_s.is_nan() || t_s < ACID_CYCLING_TEMPERATURE.value()) {
            self.fault(
                Item::M1,
                &format!("(b) a runaway under Venus's deck below 430 K: {line}"),
            );
        }
        if !(now.is_nan() || now < 1.0) {
            return;
        }
        let first = with_present_state(PresentState::WithoutHothouse, || {
            (case.again)(PassDriver::Solve)
        });
        let first_air = first.atmosphere();
        let first_p = first_air
            .surface_pressure()
            .map_or(f64::NAN, Pascals::value);
        let line = format!(
            "first solve {:?} T_s {:.2} K, A {:.4}, p_s {:.4e} bar (T_sat {:.3} K); published \
             {line}",
            first_air.state(),
            first_air.surface_temperature().value(),
            first.albedo().value(),
            first_p / 1e5,
            saturation(first_p)
        );
        if first_air.state() != SurfaceState::Temperate || !solved_as_a_hothouse(first_air) {
            self.fault(
                Item::M1,
                &format!("(c) a runaway between the limits not (c)'s: {line}"),
            );
        }
        let side = if air_t_s < saturation(p_s) {
            Saturation::Under
        } else {
            Saturation::Above
        };
        self.hothouse = Some((deck, side));
        self.state(Item::M1, "(c)'s hothouses between the limits", &line);
    }

    /// M3, K4's ceiling (decision-bump-plan-after-22 K4, test 3; decision-absurd-states-24 R4.2):
    /// a body beyond the maximum-greenhouse limit now (its ratio below 1) on a solid crust takes
    /// the ceiling, and its passes' `T_s` lies at or under max(273.15 K, `T_rest`), `T_rest` the
    /// grey surface of nitrogen's and hydrogen's terms alone over its published partial pressures
    /// from its base temperature, the larger of `T_eq` and the night floor at the heat flow its
    /// published pass read (`surface_mix::airless_mean` under the unlit mean's rules, recomputed
    /// here, not read from the ceiling); a body the ceiling reaches lies beyond the limit, and one
    /// beyond it the ceiling does not reach is a magma ocean (a molten crust's, which K4 excepts).
    fn ceiling(&mut self, case: &AuditCase<'_>) {
        let Some(surface) = case.surface() else {
            return;
        };
        let derived = case.derived;
        let air = derived.atmosphere();
        let beyond = case.ratios[2] < 1.0;
        let ceiling = air.greenhouse_ceiling();
        let t_s = air.surface_temperature().value();
        match (beyond, ceiling) {
            (false, GreenhouseCeiling::None) => return,
            (false, GreenhouseCeiling::MaximumGreenhouse) => {
                self.fault(
                    Item::M3,
                    &format!(
                        "the ceiling inside the limit, its ratio {:.6}",
                        case.ratios[2]
                    ),
                );
                return;
            }
            (true, GreenhouseCeiling::None) => {
                if air.state() == SurfaceState::MagmaOcean {
                    self.state(Item::M3, "beyond on a molten crust", &format!("{t_s:.2} K"));
                } else {
                    self.fault(
                        Item::M3,
                        &format!(
                            "beyond the limit ({:.6}) with no ceiling, {:?} at {t_s:.3} K",
                            case.ratios[2],
                            air.state()
                        ),
                    );
                }
                return;
            }
            (true, GreenhouseCeiling::MaximumGreenhouse) => {}
        }
        let heat_flow = surface
            .basis()
            .map_or_else(|| surface.figures().heat_flow(), |basis| basis.heat_flow);
        let (t_eq, floor) = (
            derived.equilibrium_temperature().value(),
            night_floor(heat_flow).value(),
        );
        let base = Kelvin::new(if floor > t_eq { floor } else { t_eq });
        let rest = GreenhouseConstants::in_force()
            .terms(air.partial_pressures(), base, derived.surface_gravity())
            .iter()
            .filter(|term| matches!(term.absorber(), Absorber::Nitrogen | Absorber::Hydrogen))
            .fold(0.0, |sum, term| sum + term.depth());
        let t_rest = grey_surface_temperature(base, rest).value();
        let cap = t_rest.max(WATER_MELTING_POINT.value());
        let line = format!(
            "{:?} T_s {t_s:.4} K (the record's {:.4} K), cap {cap:.4} K (T_rest {t_rest:.4} K \
             from {:.3} K), p_s {:.4e} bar, maximum ratio {:.5}",
            air.state(),
            surface.conditions().mean_surface_temperature().value(),
            base.value(),
            air.surface_pressure().map_or(f64::NAN, Pascals::value) / 1e5,
            case.ratios[2]
        );
        // An air the cap holds, at 273.15 K or at its nitrogen's and hydrogen's `T_rest` above its
        // base; an airless body at its base lies at a `T_rest` of its base, under no cap.
        if t_s.is_nan() || t_s > cap * (1.0 + 1e-9) {
            self.fault(Item::M3, &format!("above its ceiling: {line}"));
        } else if t_s >= cap * (1.0 - 1e-9) && cap > base.value() * (1.0 + 1e-9) {
            self.state(Item::M3, "at the cap", &line);
        } else {
            self.state(Item::M3, "under the cap, or airless at its base", &line);
        }
    }

    /// M4, P14.T53.b part 2's tags under the air's batch (`envelope_tag_faults`), with every body a
    /// supercritical reservoir hides stated.
    fn envelope_tags(&mut self, case: &AuditCase<'_>) {
        let body = EnvelopeBody::of(case.set, case.kind, case.record, case.derived);
        for fault in envelope_tag_faults(&body, AirRules::Batch) {
            self.faults.push((Item::M4, fault));
        }
        if let Some(hidden) = &body.hidden {
            self.state(Item::M4, "hidden", &EnvelopeBody::column_line(hidden));
        }
    }

    /// M6, P14.T56.g (decision-p14-t56g §4, §7.2): a lagged record's surface is rock, `Mercury`'s
    /// template where airless, under a lid that is not `H2O`; no airless record has an `Ice` surface
    /// above the melting point at T0, a planet's above all; an icy belt member is lagged exactly
    /// above it and a planet never (the code's own rule). Count (ix), every other `Ice` surface
    /// above it, is stated.
    fn lag(&mut self, case: &AuditCase<'_>) {
        let Some(surface) = case.surface() else {
            return;
        };
        let lag = case.derived.surface_lag();
        let t0 = case.t0();
        let hot = t0 > HOT_ICE_LIMIT;
        let lagged = lag == SurfaceLag::Lagged;
        match &case.body.part {
            Part::Planet(_) if lagged => {
                self.fault(Item::M6, "a lagged planet");
            }
            Part::Member(_) if lagged != (icy_member_ice(case.body).is_some() && hot) => {
                self.fault(
                    Item::M6,
                    &format!("a member {lag:?} at T0 {:.1} K", t0.value()),
                );
            }
            Part::Planet(_) | Part::Member(_) | Part::Moon(_) | Part::Ring(_) => {}
        }
        let state = surface.state();
        let template = case.record.photometry().ok().map(BodyPhotometry::template);
        let line = format!(
            "{:?}, {state:?}, {:?}, template {template:?}, T0 {:.1} K, T_eq {:.1} K, T_s {:.1} K, \
             p_s {:.4e} Pa",
            case.record.bulk().ok().map(BulkProperties::class),
            surface.material(),
            t0.value(),
            case.derived.equilibrium_temperature().value(),
            surface.conditions().mean_surface_temperature().value(),
            surface.conditions().surface_pressure().value()
        );
        if lagged {
            let water_lid = surface.crust().ok().is_some_and(|crust| {
                let named = match crust.lid() {
                    LidCrust::Primary => crust.primary(),
                    LidCrust::Secondary => crust.secondary(),
                    LidCrust::Bulk => crust.bulk(),
                };
                named == Some(CrustLithology::Water.substance())
            });
            if surface.material() != SurfaceMaterial::Rock
                || (state == SurfaceState::Airless && template != Some(PhaseTemplate::Mercury))
                || water_lid
            {
                self.fault(
                    Item::M6,
                    &format!("lagged, not rock (H2O lid {water_lid}): {line}"),
                );
            }
            self.state(Item::M6, "lagged", &line);
        } else if surface.material() == SurfaceMaterial::Ice && hot {
            if state == SurfaceState::Airless {
                self.fault(
                    Item::M6,
                    &format!("an airless Ice surface above the melting point at T0: {line}"),
                );
            } else {
                self.state(
                    Item::M6,
                    "count (ix), other Ice surfaces above the melting point at T0",
                    &line,
                );
            }
        }
    }

    /// M7, P14.T51.g.b's invariants on every published ocean, every 24 hold in force (main's
    /// rulings, 2026-10-27, on T51.g.a's MUST-FIX and on k-1's audit: T51.g.a's narrower floor,
    /// 626.09 MPa, applies under its own hold alone): its floor rock, ice III or ice V, at most
    /// T51.g.b's own bound, the melting pressure of water's high-pressure ice at the floor's
    /// temperature in the solve, T*, ice Ih's melting point at the shell's base's pressure
    /// (`high_pressure_melting`, as `Hydrosphere::base_at` reads it, to `implausibility`'s 10⁻⁶);
    /// a positive thickness; shell and depth within R − `r_w` (to 1 m); its mass at 1,000 kg m⁻³
    /// at most x M, x its record's water (its bulk's, or its condensed water's where larger, as the
    /// rule reads it); and the plan's own list (`implausibility`); no lagged record with an ocean or
    /// an ice shell's regime. The oceans are stated by floor (the ice III floors by count), and
    /// LOOK72's three named oceans and [`DEEPEST_ICE_V_FLOOR`] by name.
    #[expect(
        clippy::too_many_lines,
        reason = "one check of main's list and the plan's on each ocean, in turn"
    )]
    fn ocean(&mut self, case: &AuditCase<'_>) {
        let Some(surface) = case.surface() else {
            return;
        };
        let figures = surface.figures();
        let published = *figures.subsurface_ocean();
        let regime = figures.tectonic_regime();
        let lagged = case.derived.surface_lag() == SurfaceLag::Lagged;
        if lagged && (published != Subsurface::NoOcean || regime == TectonicRegime::IceShell) {
            self.fault(
                Item::M7,
                &format!("lagged, publishing {published:?} under {regime:?}"),
            );
        }
        let Subsurface::Ocean(ocean) = published else {
            return;
        };
        let Some(bulk) = case.record.bulk().ok() else {
            return;
        };
        let radius = Metres::from(bulk.radius());
        let mass = Kilograms::from(match &case.body.part {
            Part::Moon(part) => part.satellite.mass(),
            Part::Planet(_) | Part::Member(_) | Part::Ring(_) => case.derived.mass(),
        });
        let r = radius.value();
        let water = Gas::Water.substance();
        let column = surface
            .condensates()
            .entries()
            .iter()
            .filter(|entry| {
                entry.substance() == water
                    && matches!(
                        entry.phase(),
                        CondensatePhase::Solid | CondensatePhase::Liquid
                    )
            })
            .fold(0.0, |sum, entry| sum + entry.reservoir().value());
        let condensed = column * 4.0 * core::f64::consts::PI * r * r / mass.value();
        let bulk_water = bulk.fractions().water();
        let x = if condensed > bulk_water {
            condensed
        } else {
            bulk_water
        };
        let inputs = OceanInputs {
            radius,
            mass,
            water_fraction: x,
            surface_temperature: surface.conditions().mean_surface_temperature(),
            heat_flow: figures.heat_flow(),
            solutes: case.derived.ocean_solutes(),
        };
        let water_column = WaterColumn::of_water(radius, mass, x);
        let (shell, depth) = (ocean.shell_thickness().value(), ocean.thickness().value());
        let floor_radius = r - shell - depth;
        let top = r - shell;
        let floor_pressure = water_column.pressure_at(Metres::new(floor_radius)).value();
        let ocean_mass = 4.0 / 3.0
            * core::f64::consts::PI
            * WATER_DENSITY.value()
            * (top * top * top - floor_radius * floor_radius * floor_radius);
        let t_star = ice_ih_melting_temperature(water_column.pressure_at(Metres::new(top)));
        let bound =
            high_pressure_melting(t_star).map_or(f64::NAN, |(pressure, _)| pressure.value());
        let line = format!(
            "{:?}, {:.1} km under {:.1} km, its floor at {:.2} MPa against its bound {:.2} MPa at T* \
             {:.3} K, base {:.2} K, x {x:.4}, F {:.2} mW m⁻², {:?} {:?}",
            ocean.floor(),
            depth / 1e3,
            shell / 1e3,
            floor_pressure / 1e6,
            bound / 1e6,
            t_star.value(),
            ocean.base_temperature().value(),
            figures.heat_flow().value() * 1e3,
            bulk.class(),
            surface.state()
        );
        let fails = [
            (
                !matches!(
                    ocean.floor(),
                    OceanFloor::Rock | OceanFloor::IceIii | OceanFloor::IceV
                ),
                "a floor neither rock, ice III nor ice V",
            ),
            (
                floor_pressure.is_nan() || bound.is_nan() || floor_pressure > bound * (1.0 + 1e-6),
                "its floor past its ice's melting pressure at T*",
            ),
            (
                !(depth.is_finite() && depth > 0.0),
                "a thickness not positive",
            ),
            (
                (shell + depth).is_nan() || shell + depth > r - water_column.base().value() + 1.0,
                "deeper than its water",
            ),
            (
                ocean_mass.is_nan() || ocean_mass > x * mass.value() * (1.0 + 1e-9),
                "more than its water",
            ),
        ];
        for (failed, what) in fails {
            if failed {
                self.fault(Item::M7, &format!("{what}: {line}"));
            }
        }
        if let Some(fault) = implausibility(&inputs, &ocean) {
            self.fault(Item::M7, &format!("{fault}: {line}"));
        }
        let floor = match ocean.floor() {
            OceanFloor::Rock => "oceans on rock",
            OceanFloor::IceIii => "oceans on ice III",
            OceanFloor::IceV => "oceans on ice V",
            OceanFloor::IceVi | OceanFloor::IceVii => "oceans on ice VI or VII",
        };
        let named = LOOK72_OCEANS
            .iter()
            .any(|id| case.set == "look72" && case.record.identity().id().to_string() == *id);
        if named {
            self.state(Item::M7, "LOOK72's named oceans", &line);
        }
        if case.set == "sample" && case.record.identity().id().to_string() == DEEPEST_ICE_V_FLOOR {
            self.state(Item::M7, "the deepest ice V floor, by name", &line);
        }
        self.state(Item::M7, floor, &line);
    }

    /// M9, P14.T54.b.e's young lava (decision-p14-t54b-e-findings §A): a published pass that took
    /// a maturity on an airless (or tenuous) silicate lid outside `CAPPED_ROWS` whose surface is
    /// younger than `REGOLITH_FORMATION_TIME` publishes its regolith at most 0.11 about a Sun, on
    /// its stand-in's arm (`StandIn::UnweatheredLava`), as `young_lava_census` asserts.
    fn young_lava(&mut self, case: &AuditCase<'_>) {
        let state = case.derived.atmosphere().state();
        let Ok(pass) = case.derived.weathered() else {
            return;
        };
        let lid = pass.parts().lid;
        if !matches!(state, SurfaceState::Airless | SurfaceState::Tenuous)
            || !capped_lid(lid, state)
            || CAPPED_ROWS.contains(&lid.substance().key().as_str())
        {
            return;
        }
        let t_s = pass.parts().surface_age.value();
        if t_s >= REGOLITH_FORMATION_TIME.value() {
            return;
        }
        let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
        let regolith = pass.regolith().bond(state, &sun_bands());
        let line = format!(
            "{} ({state:?}, t_s {t_s:.3e} yr): its regolith {:.4} ({:?}), Bond {:.4}",
            lid.substance().key().as_str(),
            regolith.bond,
            regolith.stand_in,
            case.derived.albedo().value()
        );
        if regolith.bond.is_nan()
            || regolith.bond > moon
            || regolith.stand_in != Some(StandIn::UnweatheredLava)
        {
            self.fault(Item::M9, &format!("young lava above the cap: {line}"));
        }
        self.state(Item::M9, "young lava on the stand-in's arm", &line);
    }

    /// M10, decision-p14-t53c-tenuous-aerosols §2.3: a `Tenuous` record's aerosols are not
    /// modelled, never the empty inventory.
    fn tenuous(&mut self, case: &AuditCase<'_>) {
        let Some(surface) = case.surface() else {
            return;
        };
        if surface.state() != SurfaceState::Tenuous {
            return;
        }
        match surface.aerosols() {
            Modelled::NotModelled => self.state(Item::M10, "tenuous", ""),
            Modelled::Ok(inventory) => self.fault(
                Item::M10,
                &format!(
                    "a tenuous record's aerosols modelled, {} modes",
                    inventory.modes().len()
                ),
            ),
        }
    }

    /// M11, every member P14.T54.b first sends finite and in its range: the surface's vertical
    /// structure (starting at the published conditions, its skin at most `T_s`, its adiabat cooling
    /// aloft), its aerosol inventory (each deck's top above its base, each mode's τ(550), column,
    /// coverage and profile, each absorber's column), its modifiers (k, the maturity, the covers
    /// and the deposits' shares), and the envelope (`T_int`, `T_irr`, the join and the adiabat, the
    /// composition summing to 1, the decks' and modes' figures, no supercritical condensable at
    /// 24); and no member's `Debug` holding a NaN or an infinity.
    #[expect(
        clippy::too_many_lines,
        reason = "one check of each new member's figures, in turn"
    )]
    fn finite(&mut self, case: &AuditCase<'_>) {
        let unit = |v: f64| (0.0..=1.0).contains(&v);
        let non_negative = |v: f64| v.is_finite() && v >= 0.0;
        let positive = |v: f64| v.is_finite() && v > 0.0;
        let mut faults: Vec<String> = Vec::new();
        if let Some(surface) = case.surface() {
            let conditions = surface.conditions();
            match surface.vertical_structure() {
                Modelled::Ok(Some(VerticalStructure::RadiativeConvective(profile))) => {
                    let t_s = conditions.mean_surface_temperature().value();
                    if bits(profile.surface_temperature().value()) != bits(t_s)
                        || bits(profile.surface_pressure().value())
                            != bits(conditions.surface_pressure().value())
                    {
                        faults.push("a structure off its surface's conditions".to_owned());
                    }
                    let skin = profile.skin_temperature().value();
                    if !(positive(skin) && skin <= t_s) {
                        faults.push(format!("a skin at {skin} K over {t_s} K"));
                    }
                    let nodes = profile.nodes();
                    if nodes.is_empty()
                        || nodes.iter().any(|node| {
                            !(positive(node.pressure().value())
                                && positive(node.temperature().value()))
                        })
                        || nodes.windows(2).any(|pair| {
                            !(pair[1].pressure() < pair[0].pressure()
                                && pair[1].temperature() < pair[0].temperature())
                        })
                    {
                        faults.push("an adiabat not cooling aloft, or not finite".to_owned());
                    }
                }
                Modelled::Ok(Some(VerticalStructure::Envelope(_))) => {
                    faults.push("an envelope's profile on a surface".to_owned());
                }
                Modelled::Ok(None) | Modelled::NotModelled => {}
            }
            if let Modelled::Ok(inventory) = surface.aerosols() {
                for deck in inventory.decks() {
                    let (base, top) = (deck.base().value(), deck.top().value());
                    if !positive(top)
                        || !base.is_finite()
                        || top > base
                        || !positive(deck.base_temperature().value())
                    {
                        faults.push(format!(
                            "a deck of {} from {base} to {top} Pa",
                            deck.condensate().substance().key()
                        ));
                    }
                }
                for mode in inventory.modes() {
                    let profile = mode.profile();
                    let (base, top) = (profile.base().value(), profile.top().value());
                    let tau_ok = !mode
                        .optical_depth()
                        .is_ok_and(|depth| !non_negative(depth.value()));
                    let column_ok = mode
                        .column_mass()
                        .is_none_or(|column| non_negative(column.value()));
                    let temperature_ok = mode.temperature().is_none_or(|t| positive(t.value()));
                    if !(tau_ok
                        && column_ok
                        && temperature_ok
                        && unit(mode.coverage())
                        && positive(top)
                        && top <= base
                        && base.is_finite()
                        && profile.exponent().is_finite())
                    {
                        faults.push(format!(
                            "a mode of {}: {mode:?}",
                            mode.material().substance().key()
                        ));
                    }
                }
                for absorber in inventory.absorbers() {
                    if !non_negative(absorber.column_per_m2()) {
                        faults.push(format!("an absorber: {absorber:?}"));
                    }
                }
            }
            if let Modelled::Ok(m) = surface.modifiers() {
                let shares: f64 = m
                    .volcanic_deposits()
                    .iter()
                    .map(VolcanicDeposit::area_fraction)
                    .chain(
                        m.weathering_deposits()
                            .iter()
                            .map(WeatheringDeposit::area_fraction),
                    )
                    .sum();
                if !(non_negative(m.weathering_rate())
                    && unit(m.maturity())
                    && unit(m.organic_cover())
                    && unit(m.organic_ground())
                    && non_negative(m.resurfacing_m_per_yr())
                    && non_negative(m.unshielded_wind_term())
                    && (0.0..=1.0 + 1e-12).contains(&shares))
                {
                    faults.push(format!("modifiers out of range: {m:?}"));
                }
            }
            for (member, text) in [
                ("structure", format!("{:?}", surface.vertical_structure())),
                ("aerosols", format!("{:?}", surface.aerosols())),
                ("modifiers", format!("{:?}", surface.modifiers())),
                (
                    "taxonomic class",
                    format!("{:?}", surface.taxonomic_class()),
                ),
                (
                    "subsurface ocean",
                    format!("{:?}", surface.figures().subsurface_ocean()),
                ),
            ] {
                if not_finite(&text) {
                    faults.push(format!("the {member} not finite: {text}"));
                }
            }
        }
        if let Section::Ok(envelope) = case.record.envelope() {
            let atmosphere = envelope.atmosphere();
            let profile = atmosphere.profile();
            let join = profile.join_pressure().value();
            let composition = atmosphere.composition();
            let total: f64 = composition.iter().map(|&(_, x)| x).sum();
            let nodes = profile.nodes();
            if envelope.supercritical().is_some()
                || !positive(atmosphere.internal_temperature().value())
                || !positive(atmosphere.irradiation_temperature().value())
                || !positive(join)
                || nodes.windows(2).any(|pair| {
                    pair[1]
                        .pressure()
                        .value()
                        .partial_cmp(&pair[0].pressure().value())
                        != Some(core::cmp::Ordering::Greater)
                })
                || total.is_nan()
                || (total - 1.0).abs() >= 1e-9
                || composition.iter().any(|&(_, x)| x.is_nan() || x < 0.0)
                || !non_negative(atmosphere.haze_optical_depth())
                || !non_negative(envelope.abundances().per_hydrogen(Element::He))
            {
                faults.push(format!(
                    "an envelope out of range: T_int {:?}, T_irr {:?}, join {join} Pa, \
                     composition {total}, haze {}",
                    atmosphere.internal_temperature(),
                    atmosphere.irradiation_temperature(),
                    atmosphere.haze_optical_depth()
                ));
            }
            for deck in atmosphere.decks() {
                if !positive(deck.base().value()) {
                    faults.push(format!(
                        "an envelope deck of {} based at {:?}",
                        deck.condensate().substance().key(),
                        deck.base()
                    ));
                }
            }
            for mode in atmosphere.modes() {
                if mode
                    .optical_depth()
                    .is_ok_and(|depth| !non_negative(depth.value()))
                    || !unit(mode.coverage())
                {
                    faults.push(format!(
                        "an envelope mode of {}: {mode:?}",
                        mode.material().substance().key()
                    ));
                }
            }
            if not_finite(&format!("{envelope:?}")) {
                faults.push("the envelope not finite".to_owned());
            }
        }
        for fault in faults {
            self.fault(Item::M11, &fault);
        }
    }
}

/// Whether the `Debug` text `text` holds a NaN or an infinity: a word `NaN`, `inf` or `-inf`.
fn not_finite(text: &str) -> bool {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_'))
        .any(|word| matches!(word, "NaN" | "inf" | "-inf"))
}

/// The audit of every body of `sets` ([`AuditBody`]), every 24 hold in force and the record's
/// rules at their batch, each system generated afresh, with the walk's wall time, s.
fn audit_census(sets: &[AuditSet]) -> (Vec<AuditBody>, f64) {
    let start = Instant::now();
    let bodies = with_every_24_hold(|| {
        let mut bodies = Vec::new();
        audit_walk(sets, &mut |case| bodies.push(AuditBody::of(case)));
        bodies
    });
    (bodies, start.elapsed().as_secs_f64())
}

/// The audit over design-k §0's set C ([`census_sets`]), built once a process.
fn census() -> &'static (Vec<AuditBody>, f64) {
    static CENSUS: OnceLock<(Vec<AuditBody>, f64)> = OnceLock::new();
    CENSUS.get_or_init(|| audit_census(&census_sets()))
}

/// The audit over design-k §0's set A ([`audit_sets`]), built once a process, so that its two
/// tests run by name together walk it once.
fn audit_set_census() -> &'static (Vec<AuditBody>, f64) {
    static CENSUS: OnceLock<(Vec<AuditBody>, f64)> = OnceLock::new();
    CENSUS.get_or_init(|| audit_census(&audit_sets()))
}

/// Prints the audit's lists of `bodies` over `what`, every failure first as a `TO MAIN:` line,
/// and asserts that no item fails: the MUST-FIX asserts.
fn assert_must_fix(what: &str, (bodies, seconds): &(Vec<AuditBody>, f64)) {
    let surfaces = bodies.iter().filter(|body| body.surface).count();
    eprintln!(
        "the audit over {what}: {} bodies with a bulk section, {surfaces} with a surface, in \
         {seconds:.1} s",
        bodies.len()
    );
    let mut lists: BTreeMap<(Item, &str), Vec<&str>> = BTreeMap::new();
    let mut faults: BTreeMap<Item, Vec<&str>> = BTreeMap::new();
    for body in bodies {
        for (item, line) in &body.faults {
            faults.entry(*item).or_default().push(line);
        }
        for (item, list, line) in &body.stated {
            lists.entry((*item, list)).or_default().push(line);
        }
    }
    for ((item, list), lines) in &lists {
        // The long lists by count, and the ice III floors (main's M7 ruling), the rest by name.
        let by_count = lines.len() > 40 || (*item, *list) == (Item::M7, "oceans on ice III");
        let shown = if by_count { 0 } else { lines.len() };
        eprintln!(
            "{item:?} {list}: {}\n  {}",
            lines.len(),
            lines[..shown].join("\n  ")
        );
    }
    for (item, lines) in &faults {
        for line in lines {
            eprintln!("TO MAIN: {item:?} fails: {line}");
        }
    }
    let failing: Vec<String> = faults
        .iter()
        .map(|(item, lines)| format!("{item:?}: {}", lines.len()))
        .collect();
    assert_eq!(failing, Vec::<String>::new(), "the MUST-FIX asserts");
    let ice_iii = lists
        .get(&(Item::M7, "oceans on ice III"))
        .map_or(0, Vec::len);
    eprintln!("M7: the ice III floors: {ice_iii}");
}

/// M1's named set over `bodies` (decision-p14-t54b-k-hothouses §3): every runaway between the
/// limits now ([`AuditBody::hothouse`]), with its deck and its side of water's saturation
/// temperature, is exactly one of `expected`'s sample planets with its deck and side, and every one
/// of them is found; each that is not (any other, a lost one, or one whose deck or side changed)
/// fails by name.
fn assert_hothouses(
    what: &str,
    bodies: &[AuditBody],
    expected: &[(&str, RunawayDeck, Saturation)],
) {
    let found: BTreeMap<&str, (RunawayDeck, Saturation)> = bodies
        .iter()
        .filter_map(|body| {
            body.hothouse
                .map(|hothouse| (body.label.as_str(), hothouse))
        })
        .collect();
    let named: BTreeMap<String, (RunawayDeck, Saturation)> = expected
        .iter()
        .map(|&(id, deck, side)| (format!("sample {id} planets"), (deck, side)))
        .collect();
    let mut faults = Vec::new();
    for (&label, got) in &found {
        match named.get(label) {
            None => faults.push(format!("{label}: a hothouse not named, {got:?}")),
            Some(want) if want != got => {
                faults.push(format!("{label}: {got:?}, named {want:?}"));
            }
            Some(_) => {}
        }
    }
    for (label, want) in &named {
        if !found.contains_key(label.as_str()) {
            faults.push(format!("{label}: a named hothouse lost, {want:?}"));
        }
    }
    eprintln!(
        "M1 (c)'s hothouses over {what}: {} ({} named)",
        found.len(),
        named.len()
    );
    for fault in &faults {
        eprintln!("TO MAIN: M1's named set: {fault}");
    }
    assert_eq!(faults, Vec::<String>::new(), "M1's named hothouses");
}

/// The audit's MUST-FIX asserts (P14.T54.b.k; design-k §3.1) over design-k §0's set C, every 24
/// hold in force and the record's rules at their batch: M1 (part 1's runaways, (c)'s hothouse
/// [`HOTHOUSE_BODY`] alone, decked and above water's saturation temperature), M3 (K4's ceiling),
/// M4 (part 2's tags), M6 (T56.g's lag), M7 (T51.g.b's invariants, its own floor bound, the ice
/// III floors by count), M9 (e's young lava), M10 (a tenuous air's aerosols) and M11 (every new
/// member finite and in range), each failure a `TO MAIN:` line; the walk's time is printed.
#[test]
fn under_every_24_hold_the_audit_s_must_fix_asserts_hold() {
    let census = census();
    assert!(census.0.len() > 1_500, "{} bodies", census.0.len());
    let what = "the census set (the first 60, the goldens, LOOK72's 30)";
    assert_must_fix(what, census);
    assert_hothouses(
        what,
        &census.0,
        &[(HOTHOUSE_BODY, RunawayDeck::Kept, Saturation::Above)],
    );
}

/// The same over design-k §0's set A (the 400, the goldens, LOOK72's 30), a measurement for the
/// bless note; [`DEEPEST_ICE_V_FLOOR`] is stated by name.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn under_every_24_hold_the_audit_s_must_fix_asserts_hold_over_the_audit_set() {
    let census = audit_set_census();
    assert!(census.0.len() > 6_000, "{} bodies", census.0.len());
    assert_must_fix("the audit set (the 400, the goldens, LOOK72's 30)", census);
}

/// M1 over design-k §0's set A, as main rules it (decision-p14-t54b-k-hothouses §3), a measurement
/// for the bless note: no runaway beyond the maximum-greenhouse limit now or under Venus's deck
/// below 430 K, every runaway between the limits now (c)'s (its first solve a `Temperate` that (c)
/// judges a hothouse), and (c)'s hothouses exactly [`AUDIT_SET_HOTHOUSES`]' six with their decks
/// and their sides of water's saturation temperature: any seventh, a lost one, or a changed deck
/// or side fails, by name. Its walk is the audit's ([`audit_set_census`]).
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn under_every_24_hold_m1_s_six_hothouses_hold_over_the_audit_set() {
    let (bodies, _) = audit_set_census();
    assert!(bodies.len() > 6_000, "{} bodies", bodies.len());
    let faults: Vec<&str> = bodies
        .iter()
        .flat_map(|body| &body.faults)
        .filter(|(item, _)| *item == Item::M1)
        .map(|(_, line)| line.as_str())
        .collect();
    let stated: Vec<&str> = bodies
        .iter()
        .flat_map(|body| &body.stated)
        .filter(|(item, ..)| *item == Item::M1)
        .map(|(.., line)| line.as_str())
        .collect();
    eprintln!("M1 (c)'s hothouses:\n  {}", stated.join("\n  "));
    assert_eq!(faults, Vec::<&str>::new(), "M1 over the audit set");
    assert_hothouses(
        "the audit set (the 400, the goldens, LOOK72's 30)",
        bodies,
        &AUDIT_SET_HOTHOUSES,
    );
}
