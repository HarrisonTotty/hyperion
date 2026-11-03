//! P14.T33.c, the composition census (decision-composition; plan 14's T33.c): the generated galaxy's
//! composition measured over real systems, as the generator version builds their records.
//!
//! The systems are `planetary::testing`'s volume-limited sample about the Sun-like point, the
//! system tests' universe ([`SEED`]), nearest first, so that the first 400 are the sample of every
//! other plan-14 census. Each is generated whole and walked as the audit walks it
//! ([`audit_walk_system`]): every planet, member and moon present at the epoch with a bulk section,
//! its record as `snapshot_at` builds it and its derivation. The ordinary census reads
//! [`ORDINARY_SYSTEMS`] systems, at least 10⁵ bodies, and the slow one [`SLOW_SYSTEMS`], at least
//! 10⁶, two slow tests, each on threads of its own (`.config/nextest.toml` reserves their
//! slots). The counts do not depend on the
//! threads: each share's systems are fixed, and the shares are merged in order.
//!
//! It prints, by host class ([`HostClass`]):
//!
//! - the table of surface state × dominant gas × deck materials × haze × escape outcome;
//! - the table of surface state × dominant condensate and phase × crust lithology × surface
//!   liquid;
//!
//! and asserts that every registry row is reached by some record ([`reach`]), or is listed in
//! [`UNREACHED`] with the reason it is not. It also counts, each with its first body, every state
//! that contradicts its own figures, every value out of its physical range and every failure of the
//! audit's MUST-FIX items ([`AuditBody`]); every gas envelope whose section at the clock window's
//! two ends, or either side of its next recorded change inside the window, is not its section at
//! the epoch's; and the lists earlier tasks recorded for it: the condensates whose density the
//! registry has not read, the envelopes' modes with no τ, the cold-trapped condensates, the reached
//! rows whose condensed optics are stand-ins, and the solid surfaces' ages by tectonic regime. Of
//! those, the census asserts only that no envelope changes section inside the clock window and
//! that the findings the holds at 25 and 26 fixed are absent ([`FIXED_UNDER_25`],
//! [`FIXED_UNDER_26`]), but the bodies [`LEFT_UNDER_26`] lists: the rest are its findings, printed
//! for review.
//!
//! The tests are native only: on WebAssembly, which has no threads, the shares would run one after
//! another, thousands of systems on one thread, and the records' bit-for-bit agreement there is
//! the goldens' to check, not a census's.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use super::audit_tests::{AuditBody, AuditCase, Item, audit_walk_system};
use super::tests::SEED;
use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::rules::GalaxyRules;
use crate::planetary::chem::condense::NotFormed;
use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase};
use crate::planetary::derive::atmosphere::SNOWBALL_ICE_COVER;
use crate::planetary::derive::atmosphere::{
    AIRLESS_PRESSURE, Gas, SurfaceState, WATER_MELTING_POINT,
};
use crate::planetary::hooks::aerosols::{AerosolDeck, AerosolMode};
use crate::planetary::hooks::figures::Subsurface;
use crate::planetary::hooks::surface::Forcing;
use crate::planetary::record::{Modelled, RecordSection, SectionState};
use crate::planetary::testing::in_shares;
use crate::planetary::testing::{SampleFilter, sample_records, with_every_27_hold};
use crate::substance::{self, CondensedOptics, SubstanceId, by_key};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// The ordinary census's systems: about 15 bodies each with a bulk section (91,781 in the first
/// 6,000), so at least 10⁵.
const ORDINARY_SYSTEMS: usize = 6_800;

/// The slow census's systems, for at least 10⁶ bodies.
const SLOW_SYSTEMS: usize = 68_000;

/// The threads the ordinary census runs on, the slots `.config/nextest.toml` reserves for it.
const ORDINARY_THREADS: usize = 16;

/// The threads the slow census runs on, the slots `.config/nextest.toml` reserves for it.
const SLOW_THREADS: usize = 16;

/// The systems of one share of a census, in sample order.
const SYSTEMS_PER_SHARE: usize = 20;

/// The rise in an envelope's helium mass fraction Y by escape at which the census calls it
/// helium-enhanced: one percentage point, a label of the census's, not a physical figure.
const HELIUM_ENHANCED_RISE: f64 = 0.01;

/// The highest mean surface temperature the census takes as physical: a sanity bound of the
/// census's, not a physical figure.
const HOTTEST_SURFACE: Kelvin = Kelvin::new(10_000.0);

/// The highest surface pressure the census takes as physical, 10 kbar: a sanity bound of the
/// census's, not a physical figure.
const DEEPEST_AIR: Pascals = Pascals::new(1e9);

/// The lowest and highest bulk densities the census takes as physical: sanity bounds of the
/// census's, not physical figures.
const DENSITY_RANGE: (KilogramsPerCubicMetre, KilogramsPerCubicMetre) = (
    KilogramsPerCubicMetre::new(10.0),
    KilogramsPerCubicMetre::new(30_000.0),
);

/// The registry rows no record of the census reaches, each with the reason; a listed row a census
/// reaches is printed.
const UNREACHED: &[(&str, &str)] = &[
    (
        "FeH",
        "no CEA record (`formation::NOT_MODELLED`): the envelopes' network carries none, and every \
         envelope lists it not modelled",
    ),
    (
        "NaCl",
        "no law aloft (`aloft::NOT_MODELLED`: sodium condenses as Na₂S), and no rule deposits a \
         surface salt",
    ),
    (
        "FeS",
        "no law aloft (`aloft::NOT_MODELLED`: Fe + H₂S needs iron grains, rained out), and no rule \
         makes a surface sulphide",
    ),
    (
        "SiO2",
        "displaced aloft by MgSiO₃ in every envelope, and no crust rule makes quartz",
    ),
    ("TiO2", "displaced aloft by CaTiO₃ in every envelope"),
    (
        "brine_mgso4",
        "an ocean's liquid is `H2O`, `brine_nacl` or `nh3_h2o` (P14.T51.g.b); sulphate oceans are \
         P14.T55.k's",
    ),
    (
        "brine_naclo4",
        "no rule makes a perchlorate brine, and the perchlorates have no rows (P14.T49.h)",
    ),
    (
        "brine_mgclo4",
        "no rule makes a perchlorate brine, and the perchlorates have no rows (P14.T49.h)",
    ),
    (
        "anorthosite",
        "R09's reference rows only: a generated primary crust is `feldspathic` (P14.T51.c)",
    ),
    (
        "lava_glass",
        "R08's regolith row (P14.T24.c's dusts): no crust rule names it (P14.T51.c)",
    ),
    (
        "ca_al_residue",
        "a lava world's evaporated lag: no rule makes one before P14.T55.b's silicate vapour",
    ),
    (
        "iron_nickel",
        "no rule makes a metal surface: no lithology of P14.T51.c is a metal",
    ),
    ("SiC", "carbon-rich surfaces are P14.T55.d's"),
    (
        "MgSO4",
        "no rule deposits a salt; sulphate oceans are P14.T55.k's",
    ),
    (
        "Na2CO3",
        "R09's reference Ceres only: no rule deposits a salt, and the crust stores its carbonate \
         as a mass, not a row",
    ),
    (
        "CaCO3",
        "no rule deposits a salt: the crust stores its carbonate as a mass, not a row",
    ),
    ("carbonaceous", "carbon-rich surfaces are P14.T55.d's"),
];

/// A system's host class, P14.T33.b's groups (`tests/planetary_end_to_end.rs`): a single star by
/// its phase at the epoch and, on the main sequence, its initial mass; or several stars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HostClass {
    /// One main-sequence star of 0.7–1.3 M☉.
    FgkDwarf,
    /// One main-sequence star of 0.1–0.6 M☉.
    MDwarf,
    /// One main-sequence star of any other mass.
    OtherDwarf,
    /// One protostar or pre-main-sequence star.
    PreMainSequence,
    /// One living star past the main sequence.
    Evolved,
    /// One remnant: a white dwarf, a neutron star or a black hole.
    Remnant,
    /// One star in no class above, a brown dwarf among them.
    OtherSingle,
    /// Two stars or more.
    Multiple,
}

impl HostClass {
    /// The host class of `ctx`'s system.
    fn of(ctx: &SystemContext) -> Self {
        let [star] = ctx.stars() else {
            return Self::Multiple;
        };
        let m = star.initial_mass().value();
        match star.state_at(UniverseTime::EPOCH).map(|s| s.phase()) {
            Some(Phase::MainSequence) if (0.7..1.3).contains(&m) => Self::FgkDwarf,
            Some(Phase::MainSequence) if (0.1..0.6).contains(&m) => Self::MDwarf,
            Some(Phase::MainSequence) => Self::OtherDwarf,
            Some(Phase::Protostar | Phase::PreMainSequence) => Self::PreMainSequence,
            Some(phase) if phase.is_remnant() => Self::Remnant,
            Some(phase) if phase.is_living() && phase != Phase::Substellar => Self::Evolved,
            Some(_) | None => Self::OtherSingle,
        }
    }

    /// Its name in the tables.
    const fn name(self) -> &'static str {
        match self {
            Self::FgkDwarf => "single FGK dwarf (0.7–1.3 M☉)",
            Self::MDwarf => "single M dwarf (0.1–0.6 M☉)",
            Self::OtherDwarf => "single other dwarf",
            Self::PreMainSequence => "single pre-main-sequence",
            Self::Evolved => "single evolved",
            Self::Remnant => "single remnant",
            Self::OtherSingle => "single other",
            Self::Multiple => "multiple",
        }
    }
}

/// The key of the row `id`.
fn key(id: SubstanceId) -> String {
    id.substance().key().as_str().to_owned()
}

/// The most bodies a finding lists by name ([`Found::bodies`]).
const LISTED_BODIES: usize = 60;

/// One finding of the census: how many bodies show it, and the first [`LISTED_BODIES`] of them.
#[derive(Debug, Clone)]
struct Found {
    count: usize,
    bodies: Vec<String>,
}

/// What a census holds: its counts, tables and lists, merged share by share.
#[derive(Debug, Default)]
struct Census {
    systems: usize,
    bodies: usize,
    /// Systems and bodies by host class.
    classes: BTreeMap<HostClass, (usize, usize)>,
    /// Bodies by host class and (state, dominant gas, decks, haze, escape outcome).
    air: BTreeMap<(HostClass, [String; 5]), usize>,
    /// Bodies by host class and (state, dominant condensate and phase, crust lithology, surface
    /// liquid).
    ground: BTreeMap<(HostClass, [String; 4]), usize>,
    /// Bodies reaching each registry row.
    reached: BTreeMap<SubstanceId, usize>,
    /// The findings by kind.
    findings: BTreeMap<String, Found>,
    /// Every `Snowball` above 273.15 K, with its M3 line (its `T_rest`).
    hot_snowballs: Vec<String>,
    /// The gas envelopes (and supercritical ones) watched over the clock window.
    envelopes_watched: usize,
    /// The bodies near a section's edge ([`near_a_section_edge`]), watched at [`WATCH_TIMES`]
    /// times.
    envelopes_near_an_edge: usize,
    /// Those of them with a recorded change inside the clock window.
    envelopes_with_a_change: usize,
    /// Every watched envelope whose section changes inside the clock window.
    envelope_changes: Vec<String>,
    /// Condensate entries whose density the registry has not read, by substance and phase.
    densities_not_read: BTreeMap<(SubstanceId, String), usize>,
    /// Envelope modes with no τ, by material and whether a deck's or a haze's.
    modes_without_depth: BTreeMap<(SubstanceId, &'static str), usize>,
    /// Condensate entries with a cold-trapped area, by substance.
    cold_trapped: BTreeMap<SubstanceId, usize>,
    /// Solid surfaces by tectonic regime: all, and those whose surface is at least 90% of their
    /// system's age.
    surface_ages: BTreeMap<String, (usize, usize)>,
    /// The rows an air or an envelope names but holds not, by row and why: not formed (and why)
    /// or not modelled.
    named_absent: BTreeMap<(SubstanceId, String), usize>,
}

impl Census {
    /// Adds `other`, a later share's.
    fn merge(&mut self, other: Self) {
        fn add<K: Ord>(into: &mut BTreeMap<K, usize>, from: BTreeMap<K, usize>) {
            for (k, n) in from {
                *into.entry(k).or_default() += n;
            }
        }
        self.systems += other.systems;
        self.bodies += other.bodies;
        for (class, (systems, bodies)) in other.classes {
            let entry = self.classes.entry(class).or_default();
            entry.0 += systems;
            entry.1 += bodies;
        }
        add(&mut self.air, other.air);
        add(&mut self.ground, other.ground);
        add(&mut self.reached, other.reached);
        for (kind, found) in other.findings {
            let entry = self.findings.entry(kind).or_insert(Found {
                count: 0,
                bodies: Vec::new(),
            });
            entry.count += found.count;
            let room = LISTED_BODIES.saturating_sub(entry.bodies.len());
            entry.bodies.extend(found.bodies.into_iter().take(room));
        }
        self.hot_snowballs.extend(other.hot_snowballs);
        self.envelopes_watched += other.envelopes_watched;
        self.envelopes_near_an_edge += other.envelopes_near_an_edge;
        self.envelopes_with_a_change += other.envelopes_with_a_change;
        self.envelope_changes.extend(other.envelope_changes);
        add(&mut self.densities_not_read, other.densities_not_read);
        add(&mut self.modes_without_depth, other.modes_without_depth);
        add(&mut self.cold_trapped, other.cold_trapped);
        for (regime, (all, old)) in other.surface_ages {
            let entry = self.surface_ages.entry(regime).or_default();
            entry.0 += all;
            entry.1 += old;
        }
        add(&mut self.named_absent, other.named_absent);
    }

    /// Counts the finding `kind` on the body `line` describes.
    fn found(&mut self, kind: &str, line: String) {
        let entry = self.findings.entry(kind.to_owned()).or_insert(Found {
            count: 0,
            bodies: Vec::new(),
        });
        entry.count += 1;
        if entry.bodies.len() < LISTED_BODIES {
            entry.bodies.push(line);
        }
    }

    /// The census of one present body `case` of `system` (of `ctx`, of host class `class`, the
    /// sample's system `rank` from 0).
    fn add(
        &mut self,
        class: HostClass,
        rank: usize,
        (ctx, system): (&SystemContext, &PlanetarySystem),
        case: &AuditCase<'_>,
    ) {
        self.bodies += 1;
        self.classes.entry(class).or_default().1 += 1;
        let record = case.record;
        let label = format!("#{rank} {} {}", record.identity().id(), case.kind);
        let state = state_label(record, case.derived);
        let [gas, decks, haze] = air_columns(record);
        let escape = escape_label(case.derived);
        *self
            .air
            .entry((class, [state.clone(), gas, decks, haze, escape]))
            .or_default() += 1;
        let [condensate, crust, liquid] = ground_columns(record);
        *self
            .ground
            .entry((class, [state, condensate, crust, liquid]))
            .or_default() += 1;
        reach(record, &mut |id| *self.reached.entry(id).or_default() += 1);
        let audit = AuditBody::of(case);
        for (item, line) in &audit.faults {
            let line = line.strip_prefix("census ").unwrap_or(line);
            self.found(
                &format!("the audit's {item:?} fails on {}", case.kind),
                format!("#{rank} {line}"),
            );
        }
        self.states(&label, case, &audit);
        self.values(&label, case);
        self.envelope(&label, (ctx, system), case);
        self.lists(ctx, record);
    }

    /// The states that contradict their own figures.
    fn states(&mut self, label: &str, case: &AuditCase<'_>, audit: &AuditBody) {
        let record = case.record;
        let solid = record
            .bulk()
            .ok()
            .is_some_and(|bulk| bulk.class().has_solid_surface());
        if record.surface().ok().is_some() && !solid {
            self.found(
                "a surface on a body with no solid surface",
                label.to_owned(),
            );
        }
        if record.envelope().ok().is_some() && solid {
            self.found("an envelope on a solid body", label.to_owned());
        }
        let Some(surface) = case.surface() else {
            return;
        };
        let conditions = surface.conditions();
        let (t_s, p_s) = (
            conditions.mean_surface_temperature().value(),
            conditions.surface_pressure().value(),
        );
        let airless = AIRLESS_PRESSURE.value();
        if surface.state() == SurfaceState::Snowball && !rule_5_holds(surface) {
            self.found(
                "a Snowball its own figures do not make (rule 5)",
                format!(
                    "{label}: T_s {t_s:.2} K, ice {:.4}, liquid water {}, {:?}",
                    surface.figures().ice_fraction(),
                    lists_liquid_water(surface),
                    surface.climate_regime().forcing()
                ),
            );
        }
        let line = format!(
            "{label}: {:?} {:?}, T_s {t_s:.2} K, p_s {:.4e} bar, {}",
            surface.state(),
            surface.material(),
            p_s / 1e5,
            leading_gases(conditions.gases())
        );
        match surface.state() {
            SurfaceState::Snowball if t_s > WATER_MELTING_POINT.value() => {
                let ceiling = audit
                    .stated
                    .iter()
                    .find(|(item, ..)| *item == Item::M3)
                    .map_or("no M3 line", |(.., line)| {
                        line.split_once(": ")
                            .map_or(line.as_str(), |(_, rest)| rest)
                    });
                let line = format!("{line}; M3: {ceiling}");
                self.hot_snowballs.push(line.clone());
                self.found("a Snowball above 273.15 K", line);
            }
            SurfaceState::Airless if p_s >= airless => {
                self.found("an Airless surface at or above 100 Pa", line);
            }
            SurfaceState::Tenuous if p_s >= airless => {
                self.found("a Tenuous air at or above 100 Pa", line);
            }
            SurfaceState::Tenuous if conditions.gases().is_empty() => {
                self.found("a Tenuous air with no gas", line);
            }
            SurfaceState::Temperate
            | SurfaceState::Desert
            | SurfaceState::Snowball
            | SurfaceState::RunawayGreenhouse
                if p_s < airless =>
            {
                self.found("a Temperate, Snowball or runaway air below 100 Pa", line);
            }
            SurfaceState::RunawayGreenhouse
                if surface
                    .figures()
                    .surface_liquids()
                    .iter()
                    .any(|&(id, area)| id == Gas::Water.substance() && area > 0.0) =>
            {
                self.found("a runaway with liquid water at its surface", line);
            }
            SurfaceState::GasEnvelope
            | SurfaceState::MagmaOcean
            | SurfaceState::Airless
            | SurfaceState::RunawayGreenhouse
            | SurfaceState::Temperate
            | SurfaceState::Desert
            | SurfaceState::Snowball
            | SurfaceState::Tenuous
            | SurfaceState::SteamEnvelope => {}
        }
    }

    /// The values out of their physical range: the surface's temperature, pressure and air, the
    /// bulk section's radius, density, gravity and equilibrium temperature.
    fn values(&mut self, label: &str, case: &AuditCase<'_>) {
        let record = case.record;
        if let Some(bulk) = record.bulk().ok() {
            let (r, rho, g, t_eq) = (
                bulk.radius().value(),
                bulk.density().value(),
                bulk.surface_gravity().value(),
                bulk.equilibrium_temperature().value(),
            );
            let line = format!(
                "{label}: {:?}, R {r:.4} R⊕, ρ {rho:.1} kg/m³, g {g:.3} m/s², T_eq {t_eq:.2} K",
                bulk.class()
            );
            if !(r.is_finite() && r > 0.0 && g.is_finite() && g > 0.0) {
                self.found(
                    "a radius or gravity not finite or not positive",
                    line.clone(),
                );
            }
            if !(DENSITY_RANGE.0.value()..=DENSITY_RANGE.1.value()).contains(&rho) {
                self.found("a bulk density outside 10–30,000 kg/m³", line.clone());
            }
            if !t_eq.is_finite() || t_eq < 0.0 {
                self.found("an equilibrium temperature not finite or below 0 K", line);
            } else if t_eq <= 0.0 {
                self.found(
                    "unlit: an equilibrium temperature of 0 K, P14.T24.i's",
                    line,
                );
            }
        }
        let Some(surface) = case.surface() else {
            return;
        };
        let conditions = surface.conditions();
        let (t_s, p_s) = (
            conditions.mean_surface_temperature().value(),
            conditions.surface_pressure().value(),
        );
        let line = format!(
            "{label}: {:?}, T_s {t_s:.3} K, p_s {:.4e} bar, T_eq {:.2} K",
            surface.state(),
            p_s / 1e5,
            case.derived.equilibrium_temperature().value()
        );
        if t_s.is_nan() || t_s <= 0.0 {
            self.found("a mean surface temperature of 0 K or below", line.clone());
        } else if t_s > HOTTEST_SURFACE.value() {
            self.found("a mean surface temperature above 10,000 K", line.clone());
        }
        if !p_s.is_finite() || p_s < 0.0 {
            self.found("a surface pressure not finite or below 0", line.clone());
        } else if p_s > DEEPEST_AIR.value() {
            self.found("a surface pressure above 10 kbar", line.clone());
        }
        let gases = conditions.gases();
        let total: f64 = gases.iter().map(|&(_, x)| x).sum();
        if !gases.is_empty() && (total.is_nan() || (total - 1.0).abs() > 1e-9) {
            self.found("an air's mole fractions not summing to 1", line);
        }
    }

    /// Watches the envelope of a present planet or member with a gas envelope or a supercritical
    /// one over the clock window: its section at the window's two ends against the epoch's, and
    /// either side of its recorded change inside the window, if it has one (main's ruling of
    /// 2026-10-27; plan 14's P14.T54.b record, i's bullet).
    fn envelope(
        &mut self,
        label: &str,
        (ctx, system): (&SystemContext, &PlanetarySystem),
        case: &AuditCase<'_>,
    ) {
        let record = case.record;
        let gas_envelope = record
            .bulk()
            .ok()
            .is_some_and(|bulk| !bulk.class().has_solid_surface());
        let epoch = record.section_state(RecordSection::Envelope);
        let watched =
            case.kind != "moons" && (gas_envelope || epoch != SectionState::NotApplicable);
        let near = near_a_section_edge(case);
        if !(watched || near) {
            return;
        }
        self.envelopes_watched += usize::from(watched);
        self.envelopes_near_an_edge += usize::from(near);
        let section = |t: UniverseTime| {
            system
                .body_at(ctx, case.body.index, t)
                .expect("a walked body is in its system")
                .section_state(RecordSection::Envelope)
        };
        let times: Vec<(String, UniverseTime)> = if near {
            (0..WATCH_TIMES)
                .map(|k| {
                    (
                        format!("{k}/{} of the window", WATCH_TIMES - 1),
                        watch_time(k),
                    )
                })
                .collect()
        } else {
            vec![
                ("the window's start".to_owned(), ClockWindow::START),
                ("its end".to_owned(), ClockWindow::END),
            ]
        };
        let mut changes = Vec::new();
        for (name, t) in times {
            let then = section(t);
            if then != epoch {
                changes.push(format!("{then:?} at {name}"));
            }
        }
        if let Some(change) = case
            .now
            .changes_at
            .filter(|&at| at > ClockWindow::START && at <= ClockWindow::END)
        {
            self.envelopes_with_a_change += 1;
            let before = change
                .checked_sub(Span::from_seconds(1))
                .expect("a time inside the window less a second is a time");
            let (was, is) = (section(before), section(change));
            if was != is {
                changes.push(format!(
                    "{was:?} before its change at {change:?}, {is:?} at it"
                ));
            }
        }
        if !changes.is_empty() {
            self.envelope_changes.push(format!(
                "{label}: {epoch:?} at the epoch; {}",
                changes.join("; ")
            ));
        }
    }

    /// The lists earlier tasks recorded for the census, and the rows an air or an envelope names
    /// but holds not.
    fn lists(&mut self, ctx: &SystemContext, record: &BodyRecord) {
        let mut absent = |id: SubstanceId, why: String| {
            *self.named_absent.entry((id, why)).or_default() += 1;
        };
        if let Some(Modelled::Ok(inventory)) = record.surface().ok().map(Surface::aerosols) {
            for &(id, why) in inventory.not_formed() {
                absent(id, format!("an air's not formed: {}", not_formed(why)));
            }
        }
        if let Some(envelope) = record.envelope().ok() {
            let atmosphere = envelope.atmosphere();
            for &(id, why) in atmosphere.not_formed() {
                absent(id, format!("an envelope's not formed: {}", not_formed(why)));
            }
            for &id in atmosphere.not_modelled() {
                absent(id, "an envelope's not modelled".to_owned());
            }
        }
        if let Some(surface) = record.surface().ok() {
            for entry in surface.condensates().entries() {
                if entry.reservoir_gel().is_none() {
                    *self
                        .densities_not_read
                        .entry((entry.substance(), format!("{:?}", entry.phase())))
                        .or_default() += 1;
                }
                if entry.trapped_area_fraction() > 0.0 {
                    *self.cold_trapped.entry(entry.substance()).or_default() += 1;
                }
            }
            let figures = surface.figures();
            let age = ctx.age_at_epoch().value();
            let entry = self
                .surface_ages
                .entry(format!("{:?}", figures.tectonic_regime()))
                .or_default();
            entry.0 += 1;
            if figures.surface_age().value() >= 0.9 * age {
                entry.1 += 1;
            }
        }
        if let Some(envelope) = record.envelope().ok() {
            let atmosphere = envelope.atmosphere();
            for mode in atmosphere.modes() {
                if mode.optical_depth().is_err() {
                    let of = if is_haze(atmosphere.decks(), mode) {
                        "haze"
                    } else {
                        "deck"
                    };
                    *self
                        .modes_without_depth
                        .entry((mode.material(), of))
                        .or_default() += 1;
                }
            }
        }
    }

    /// The census as markdown: its tables and lists, `title` its heading.
    #[expect(
        clippy::too_many_lines,
        reason = "one report, each table and list in turn"
    )]
    fn report(&self, title: &str) -> String {
        let mut out = String::new();
        let w = &mut out;
        let line = "a string takes a line";
        writeln!(
            w,
            "## {title}: {} systems, {} bodies\n",
            self.systems, self.bodies
        )
        .expect(line);
        writeln!(
            w,
            "| Host class | Systems | Bodies |\n| --- | ---: | ---: |"
        )
        .expect(line);
        for (class, (systems, bodies)) in &self.classes {
            writeln!(w, "| {} | {systems} | {bodies} |", class.name()).expect(line);
        }
        writeln!(
            w,
            "\n### Surface state × dominant gas × deck materials × haze × escape outcome"
        )
        .expect(line);
        for (class, (_, bodies)) in &self.classes {
            writeln!(
                w,
                "\n#### {} ({bodies} bodies)\n\n| State | Dominant gas | Decks | Haze | Escape \
                 | Bodies |\n| --- | --- | --- | --- | --- | ---: |",
                class.name()
            )
            .expect(line);
            for (row, n) in by_count(&self.air, *class) {
                writeln!(w, "| {} | {n} |", row.join(" | ")).expect(line);
            }
        }
        writeln!(
            w,
            "\n### Surface state × dominant condensate and phase × crust lithology × surface liquid"
        )
        .expect(line);
        for (class, (_, bodies)) in &self.classes {
            writeln!(
                w,
                "\n#### {} ({bodies} bodies)\n\n| State | Condensate | Crust | Liquid | Bodies \
                 |\n| --- | --- | --- | --- | ---: |",
                class.name()
            )
            .expect(line);
            for (row, n) in by_count(&self.ground, *class) {
                writeln!(w, "| {} | {n} |", row.join(" | ")).expect(line);
            }
        }
        writeln!(
            w,
            "\n### Registry rows never reached\n\n| Row | Kind | Reason | Named but absent \
             |\n| --- | --- | --- | --- |"
        )
        .expect(line);
        for id in substance::ids().filter(|id| !self.reached.contains_key(id)) {
            let reason = UNREACHED
                .iter()
                .find(|(k, _)| *k == key(id))
                .map_or("NOT LISTED", |(_, why)| why);
            let named: Vec<String> = self
                .named_absent
                .range((id, String::new())..)
                .take_while(|((at, _), _)| *at == id)
                .map(|((_, why), n)| format!("{why} {n}"))
                .collect();
            writeln!(
                w,
                "| `{}` | {} | {reason} | {} |",
                key(id),
                id.substance().kind().name(),
                named.join("; ")
            )
            .expect(line);
        }
        let listed_reached: Vec<String> = UNREACHED
            .iter()
            .filter_map(|(k, _)| {
                let id = substance::by_key(k)?;
                self.reached.get(&id).map(|n| format!("`{k}` ({n})"))
            })
            .collect();
        writeln!(
            w,
            "\nListed but reached: {}",
            if listed_reached.is_empty() {
                "none".to_owned()
            } else {
                listed_reached.join(", ")
            }
        )
        .expect(line);
        let stand_ins: Vec<String> = self
            .reached
            .iter()
            .filter(|(id, _)| {
                id.substance()
                    .condensed_optics()
                    .is_some_and(CondensedOptics::is_stand_in)
            })
            .map(|(&id, n)| format!("`{}` {n}", key(id)))
            .collect();
        writeln!(
            w,
            "\n### Stand-ins reached (condensed optics), bodies\n\n{}",
            stand_ins.join(", ")
        )
        .expect(line);
        writeln!(
            w,
            "\n### Findings\n\n| Finding | Bodies | First |\n| --- | ---: | --- |"
        )
        .expect(line);
        for (kind, found) in &self.findings {
            let first = found.bodies.first().map_or("", String::as_str);
            writeln!(w, "| {kind} | {} | {first} |", found.count).expect(line);
        }
        for (kind, found) in &self.findings {
            writeln!(
                w,
                "\n#### {kind}: the first {} of {}\n",
                found.bodies.len(),
                found.count
            )
            .expect(line);
            for body in &found.bodies {
                writeln!(w, "- {body}").expect(line);
            }
        }
        writeln!(
            w,
            "\n### Gas envelopes changing section inside the clock window: {} of {} watched ({} \
             with a recorded change inside it, {} near a section's edge watched at {} times)",
            self.envelope_changes.len(),
            self.envelopes_watched,
            self.envelopes_with_a_change,
            self.envelopes_near_an_edge,
            WATCH_TIMES
        )
        .expect(line);
        for change in &self.envelope_changes {
            writeln!(w, "- {change}").expect(line);
        }
        writeln!(
            w,
            "\n### Snowballs above 273.15 K: {}",
            self.hot_snowballs.len()
        )
        .expect(line);
        for snowball in &self.hot_snowballs {
            writeln!(w, "- {snowball}").expect(line);
        }
        writeln!(
            w,
            "\n### Condensates whose density is not read, entries\n\n{}",
            counted(&self.densities_not_read, |(id, phase)| {
                format!("`{}` {phase}", key(*id))
            })
        )
        .expect(line);
        writeln!(
            w,
            "\n### Envelope modes with no τ, modes\n\n{}",
            counted(&self.modes_without_depth, |(id, of)| {
                format!("`{}` {of}", key(*id))
            })
        )
        .expect(line);
        writeln!(
            w,
            "\n### Cold-trapped condensates, entries\n\n{}",
            counted(&self.cold_trapped, |id| format!("`{}`", key(*id)))
        )
        .expect(line);
        writeln!(
            w,
            "\n### Solid surfaces by tectonic regime, and those at least 90% of their system's \
             age\n\n| Regime | Surfaces | Unresurfaced |\n| --- | ---: | ---: |"
        )
        .expect(line);
        for (regime, (all, old)) in &self.surface_ages {
            writeln!(w, "| {regime} | {all} | {old} |").expect(line);
        }
        out
    }

    /// Asserts that no watched envelope changes section inside the clock window, naming those
    /// that do: from part 3, a reservoir counts by its solve's mean surface temperature, not by
    /// the temperature the passes settle it at (research-leans-8 §1).
    fn assert_no_envelope_changes(&self) {
        assert_eq!(
            self.envelope_changes,
            Vec::<String>::new(),
            "gas envelopes changing section inside the clock window"
        );
    }

    /// Asserts that no body shows a finding of [`FIXED_UNDER_25`], naming the first ones that do.
    fn assert_fixed_under_25(&self) {
        let shown: Vec<(&str, usize, &[String])> = FIXED_UNDER_25
            .iter()
            .filter_map(|&kind| {
                self.findings
                    .get(kind)
                    .map(|found| (kind, found.count, found.bodies.as_slice()))
            })
            .collect();
        assert_eq!(shown, Vec::new(), "findings 25's holds fixed");
    }

    /// Asserts that no body shows a finding of [`FIXED_UNDER_25`] or [`FIXED_UNDER_26`] but those
    /// [`LEFT_UNDER_26`] lists, naming the first ones that do.
    fn assert_fixed_under_26(&self) {
        self.assert_fixed_under_25();
        let shown: Vec<(&str, usize, Vec<&String>)> = FIXED_UNDER_26
            .iter()
            .filter_map(|&kind| {
                let found = self.findings.get(kind)?;
                let left = |line: &&String| {
                    LEFT_UNDER_26
                        .iter()
                        .any(|&(of, body)| of == kind && line.starts_with(body))
                };
                let (left, bodies): (Vec<&String>, Vec<&String>) =
                    found.bodies.iter().partition(left);
                let count = found.count - left.len();
                (count > 0).then_some((kind, count, bodies))
            })
            .collect();
        assert_eq!(shown, Vec::new(), "findings 26's holds fix");
    }

    /// Asserts that every registry row is reached or listed in [`UNREACHED`], each listed key a
    /// row; a listed row reached is printed (the report's "Listed but reached").
    fn assert_reached(&self) {
        for (k, _) in UNREACHED {
            assert!(substance::by_key(k).is_some(), "{k} is not a registry row");
        }
        let unlisted: Vec<String> = substance::ids()
            .filter(|id| !self.reached.contains_key(id))
            .map(key)
            .filter(|k| !UNREACHED.iter().any(|(listed, _)| listed == k))
            .collect();
        assert_eq!(
            unlisted,
            Vec::<String>::new(),
            "rows no record reaches, not listed with a reason"
        );
    }
}

/// The findings of the census at version 24 that a hold at 25 fixed, each held at none under the
/// version's rules ([`Census::assert_fixed_under_25`]): the audit's M6 on a rocky belt's hot
/// ice-bulk members, P14.T56.g's lag asked of them (`member_lag`).
const FIXED_UNDER_25: [&str; 1] = ["the audit's M6 fails on members"];

/// The findings a hold at 26 fixed, each held at none under the version's rules
/// ([`Census::assert_fixed_under_26`]), with 25's ([`FIXED_UNDER_25`]): no `Snowball` above
/// 273.15 K and none its own figures do not make (P14.T53.e's eyeball, from 26), no runaway over a sea
/// (P14.T53.b's dry deck, from 26); and of the audit's, none of M3 and M6 on any kind of body (the moon's own
/// sky; P14.T56.g's planet lag). The envelope watch's count is printed, not asserted: no rule yet
/// keeps a body whose passes settle on a critical point on one side of it.
const FIXED_UNDER_26: [&str; 9] = [
    "a Snowball above 273.15 K",
    "a Snowball its own figures do not make (rule 5)",
    "a runaway with liquid water at its surface",
    "the audit's M3 fails on planets",
    "the audit's M3 fails on members",
    "the audit's M3 fails on moons",
    "the audit's M6 fails on planets",
    "the audit's M6 fails on members",
    "the audit's M6 fails on moons",
];

/// The bodies that still show a finding of [`FIXED_UNDER_26`], by the finding
/// and the start of the body's line, its system's rank and its ID; each a finding, not a fix's.
///
/// None: #17936 and #2487, the `Snowball`s above 273.15 K that P14.T24.j's deck set off while its
/// gate read only the listed liquid, keep their deck now that it reads the open water its ramp
/// reads.
const LEFT_UNDER_26: [(&str, &str); 0] = [];

/// How many times across the clock window, both ends included, the census reads the envelope
/// section of a body near a section's edge ([`near_a_section_edge`]).
pub(super) const WATCH_TIMES: i64 = 41;

/// How near its critical temperature a column lies, as `T` ÷ `T_c` − 1, for the census to read
/// its body's section at [`WATCH_TIMES`] times: a census bound.
const NEAR_CRITICAL: f64 = 1e-6;

/// How near [`THIN_ENVELOPE_FRACTION`] a column's share lies, as a part of it, for the census to
/// read its body's section at [`WATCH_TIMES`] times: a census bound.
///
/// [`THIN_ENVELOPE_FRACTION`]: crate::planetary::params::THIN_ENVELOPE_FRACTION
const NEAR_THRESHOLD: f64 = 0.1;

/// Whether a column of the body `case`'s air, at its mean surface temperature or its partition's,
/// lies within [`NEAR_CRITICAL`] of its critical temperature, or holds a share within
/// [`NEAR_THRESHOLD`] of the thin envelope's: a body whose envelope section could change with no
/// input changing.
fn near_a_section_edge(case: &AuditCase<'_>) -> bool {
    let Some(bulk) = case.record.bulk().ok() else {
        return false;
    };
    let gravity = bulk.surface_gravity();
    let air = case.derived.atmosphere();
    let threshold = crate::planetary::params::THIN_ENVELOPE_FRACTION;
    air.critical_columns(gravity)
        .chain(air.partitioned_reservoir(gravity))
        .any(|column| {
            (column.temperature_ratio() - 1.0).abs() <= NEAR_CRITICAL
                || (column.share() / threshold - 1.0).abs() <= NEAR_THRESHOLD
        })
}

/// The `k`th of [`WATCH_TIMES`] times evenly across the clock window, its start the 0th and its
/// end the last.
pub(super) fn watch_time(k: i64) -> UniverseTime {
    let width = 2 * crate::time::CLOCK_WINDOW_H.seconds();
    ClockWindow::START
        .checked_add(Span::from_seconds(width * k / (WATCH_TIMES - 1)))
        .expect("a time across the window is a time")
}

/// Whether the published condensates of `surface` list liquid water: open water, a sea under ice
/// being its ice.
fn lists_liquid_water(surface: &Surface) -> bool {
    surface
        .condensates()
        .entry(Gas::Water.substance(), CondensatePhase::Liquid)
        .is_some()
}

/// Whether the published figures of the `Snowball` `surface` make it one by P14.T53.e's rule 5
/// under the rules in force: its ices on at least [`SNOWBALL_ICE_COVER`] of its surface, and from
/// 26 on a world locked 1:1 no open water (P14.T53.e's eyeball).
fn rule_5_holds(surface: &Surface) -> bool {
    let locked = surface.climate_regime().forcing() == Forcing::Locked;
    surface.figures().ice_fraction() >= SNOWBALL_ICE_COVER
        && !(locked && lists_liquid_water(surface))
}

/// The entries of `map`, each as `text` names it with its count, joined.
fn counted<K>(map: &BTreeMap<K, usize>, text: impl Fn(&K) -> String) -> String {
    map.iter()
        .map(|(k, n)| format!("{} {n}", text(k)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The rows of `table` of host class `class`, most bodies first, ties in key order.
fn by_count<const N: usize>(
    table: &BTreeMap<(HostClass, [String; N]), usize>,
    class: HostClass,
) -> Vec<(&[String; N], usize)> {
    let mut rows: Vec<(&[String; N], usize)> = table
        .iter()
        .filter(|((c, _), _)| *c == class)
        .map(|((_, row), &n)| (row, n))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    rows
}

/// The body's state in the tables: its surface's, or for a body with none its derivation's
/// published state, said so.
fn state_label(record: &BodyRecord, derived: &DerivedBody) -> String {
    let published = derived.atmosphere().published_state();
    match record.surface() {
        Section::Ok(surface) => format!("{:?}", surface.state()),
        Section::NotApplicable if published == SurfaceState::GasEnvelope => {
            "GasEnvelope".to_owned()
        }
        Section::NotApplicable => format!("{published:?} under a supercritical envelope"),
        Section::NotModelled => format!("{published:?}, surface not modelled"),
        Section::NotResolved => "not resolved".to_owned(),
    }
}

/// The record's dominant gas, deck materials and hazes: its surface air's, or its envelope's.
fn air_columns(record: &BodyRecord) -> [String; 3] {
    let named = |ids: Vec<SubstanceId>| {
        if ids.is_empty() {
            "none".to_owned()
        } else {
            ids.into_iter().map(key).collect::<Vec<_>>().join("+")
        }
    };
    let layers = |decks: &[AerosolDeck], modes: &[AerosolMode]| {
        let mut condensates: Vec<SubstanceId> = decks.iter().map(AerosolDeck::condensate).collect();
        condensates.sort_unstable();
        condensates.dedup();
        let mut hazes: Vec<SubstanceId> = modes
            .iter()
            .filter(|mode| is_haze(decks, mode))
            .map(AerosolMode::material)
            .collect();
        hazes.sort_unstable();
        hazes.dedup();
        (named(condensates), named(hazes))
    };
    if let Section::Ok(surface) = record.surface() {
        let gas = surface
            .conditions()
            .gases()
            .first()
            .map_or_else(|| "none".to_owned(), |&(id, _)| key(id));
        let (decks, haze) = match surface.aerosols() {
            Modelled::Ok(inventory) => layers(inventory.decks(), inventory.modes()),
            Modelled::NotModelled => ("not modelled".to_owned(), "not modelled".to_owned()),
        };
        return [gas, decks, haze];
    }
    match record.envelope() {
        Section::Ok(envelope) => {
            let atmosphere = envelope.atmosphere();
            let gas = atmosphere
                .composition()
                .iter()
                .fold(
                    None,
                    |best: Option<(SubstanceId, f64)>, &(id, x)| match best {
                        Some((_, top)) if top >= x => best,
                        Some(_) | None => Some((id, x)),
                    },
                )
                .map_or_else(|| "none".to_owned(), |(id, _)| key(id));
            let (decks, haze) = layers(atmosphere.decks(), atmosphere.modes());
            [gas, decks, haze]
        }
        Section::NotModelled => [
            "envelope not modelled".to_owned(),
            "—".to_owned(),
            "—".to_owned(),
        ],
        Section::NotApplicable | Section::NotResolved => [
            "surface not modelled".to_owned(),
            "—".to_owned(),
            "—".to_owned(),
        ],
    }
}

/// Whether `mode` is a haze, not a deck's: one whose deck none of `decks` condenses (a haze, a
/// dust or a sulphate layer names its own material as its deck).
fn is_haze(decks: &[AerosolDeck], mode: &AerosolMode) -> bool {
    !decks.iter().any(|deck| deck.condensate() == mode.deck())
}

/// The escape outcome in the tables (decision-composition §1.5: bare cores, helium-enhanced
/// envelopes, O₂ remnants and CO worlds), every label the body's outcome bears, joined.
fn escape_label(derived: &DerivedBody) -> String {
    let Some(escape) = derived.atmosphere().escape() else {
        return "not recorded".to_owned();
    };
    let mut labels = Vec::new();
    if let Some(envelope) = escape.envelope() {
        labels.push(if envelope.hydrogen_kept_share() <= 0.0 {
            "bare core"
        } else if envelope.helium_mass_fraction() - envelope.initial_helium_mass_fraction()
            >= HELIUM_ENHANCED_RISE
        {
            "helium-enhanced envelope"
        } else {
            "envelope kept"
        });
    }
    let oxygen = escape.oxygen();
    if oxygen.kept().value() > 0.0 {
        labels.push("O2 remnant");
    } else if oxygen.water_lost().value() > 0.0 {
        labels.push("water lost");
    }
    if escape.carbon_monoxide().value() > 0.0 {
        labels.push("CO world");
    }
    if labels.is_empty() {
        "nothing lost to note".to_owned()
    } else {
        labels.join("+")
    }
}

/// The record's dominant condensate with its phase, its crust's lid lithology and its leading
/// surface liquid: "no surface" for a record whose surface is not `Ok`.
fn ground_columns(record: &BodyRecord) -> [String; 3] {
    let Section::Ok(surface) = record.surface() else {
        return ["no surface".to_owned(), "—".to_owned(), "—".to_owned()];
    };
    let condensate = surface
        .condensates()
        .entries()
        .iter()
        .fold(None, |best: Option<&CondensateEntry>, entry| match best {
            Some(top) if top.reservoir().value() >= entry.reservoir().value() => best,
            Some(_) | None => Some(entry),
        })
        .map_or_else(
            || "none".to_owned(),
            |entry| format!("{} {:?}", key(entry.substance()), entry.phase()),
        );
    let crust = match surface.crust() {
        Modelled::Ok(crust) => key(crust.lid_lithology()),
        Modelled::NotModelled => "not modelled".to_owned(),
    };
    let liquid = surface
        .figures()
        .surface_liquids()
        .iter()
        .fold(
            None,
            |best: Option<(SubstanceId, f64)>, &(id, area)| match best {
                Some((_, top)) if top >= area => best,
                Some(_) | None if area > 0.0 => Some((id, area)),
                Some(_) | None => best,
            },
        )
        .map_or_else(|| "none".to_owned(), |(id, _)| key(id));
    [condensate, crust, liquid]
}

/// Why a condensable forms no deck, in the census's words.
fn not_formed(why: NotFormed) -> String {
    match why {
        NotFormed::BulkGas => "a bulk gas".to_owned(),
        NotFormed::Displaced { by } => format!("displaced by {}", by.as_str()),
        NotFormed::Supercritical => "supercritical".to_owned(),
    }
}

/// The leading three gases of an air, with their mole fractions.
fn leading_gases(gases: &[(SubstanceId, f64)]) -> String {
    gases
        .iter()
        .take(3)
        .map(|&(id, x)| format!("{} {x:.3}", key(id)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Every registry row the record names as present, each given to `mark`: its surface air's gases,
/// its climate's condensable, its liquids and ices, its subsurface ocean's and its wet epoch's
/// liquids, its condensates, its crust's lithologies, its aerosols' decks, modes and absorbers and
/// its modifiers' deposits; its envelope's supercritical condensable, its gases at 1 bar and 10
/// mbar, its decks, modes and absorbers. A condensable not formed or not modelled is not present.
fn reach(record: &BodyRecord, mark: &mut dyn FnMut(SubstanceId)) {
    if let Section::Ok(surface) = record.surface() {
        for &(id, x) in surface.conditions().gases() {
            if x > 0.0 {
                mark(id);
            }
        }
        if let Some(id) = surface.climate_regime().condensable() {
            mark(id);
        }
        let figures = surface.figures();
        for &(id, area) in figures
            .surface_liquids()
            .iter()
            .chain(figures.surface_ices())
        {
            if area > 0.0 {
                mark(id);
            }
        }
        if let Subsurface::Ocean(ocean) = figures.subsurface_ocean() {
            mark(ocean.liquid());
        }
        if let Some(wet) = figures.wet_epoch() {
            mark(wet.liquid());
        }
        for entry in surface.condensates().entries() {
            mark(entry.substance());
        }
        if let Modelled::Ok(crust) = surface.crust() {
            for id in [
                crust.primary(),
                crust.secondary(),
                crust.tertiary(),
                crust.bulk(),
            ]
            .into_iter()
            .flatten()
            {
                mark(id);
            }
            mark(crust.lid_lithology());
            mark(crust.provinces());
        }
        if let Modelled::Ok(inventory) = surface.aerosols() {
            inventory.decks().iter().for_each(|d| mark(d.condensate()));
            inventory.modes().iter().for_each(|m| mark(m.material()));
            inventory.absorbers().iter().for_each(|a| mark(a.species()));
        }
        if let Modelled::Ok(modifiers) = surface.modifiers() {
            for deposit in modifiers.volcanic_deposits() {
                mark(deposit.substance());
            }
            for deposit in modifiers.weathering_deposits() {
                mark(deposit.substance());
            }
        }
    }
    if let Section::Ok(envelope) = record.envelope() {
        if let Some(id) = envelope.supercritical() {
            mark(id);
        }
        let atmosphere = envelope.atmosphere();
        for &(id, x) in atmosphere
            .composition()
            .iter()
            .chain(atmosphere.upper_gas())
        {
            if x > 0.0 {
                mark(id);
            }
        }
        atmosphere.decks().iter().for_each(|d| mark(d.condensate()));
        atmosphere.modes().iter().for_each(|m| mark(m.material()));
        atmosphere
            .absorbers()
            .iter()
            .for_each(|a| mark(a.species()));
    }
}

/// The census of `records` (of `galaxy`): each system generated whole at [`SEED`] and walked.
fn census_of(galaxy: &Galaxy, records: &[SystemRecord], first: usize) -> Census {
    let mut census = Census::default();
    for (rank, record) in (first..).zip(records) {
        let ctx = SystemContext::from_record(galaxy, record);
        let class = HostClass::of(&ctx);
        let system = generate(SEED, &ctx);
        census.systems += 1;
        census.classes.entry(class).or_default().0 += 1;
        audit_walk_system("census", &ctx, &system, &mut |case| {
            census.add(class, rank, (&ctx, &system), case);
        });
    }
    census
}

/// The census of the sample's first `systems` systems, on up to `threads` threads, printed under
/// `title` with its wall time.
fn run(title: &str, systems: usize, threads: usize) -> Census {
    let start = Instant::now();
    let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way's parameters build a galaxy");
    let records = sample_records(&galaxy, systems, SampleFilter::ALL)
        .expect("the solar circle holds the census's systems");
    let sampled = start.elapsed().as_secs_f64();
    let shares = records.len().div_ceil(SYSTEMS_PER_SHARE);
    let mut census = Census::default();
    for share in in_shares(shares, threads, |k| {
        let end = records.len().min((k + 1) * SYSTEMS_PER_SHARE);
        census_of(
            &galaxy,
            &records[k * SYSTEMS_PER_SHARE..end],
            k * SYSTEMS_PER_SHARE,
        )
    }) {
        census.merge(share);
    }
    eprintln!(
        "{}\n{title}: {:.1} s on {} threads ({sampled:.1} s drawing the sample)",
        census.report(title),
        start.elapsed().as_secs_f64(),
        hyperion_testkit::threads::granted(threads).min(shares)
    );
    census
}

/// The ordinary census (P14.T33.c), over [`ORDINARY_SYSTEMS`] systems: at least 10⁵ bodies, every
/// registry row reached or listed with its reason (a listed row it reaches is printed), none of the
/// findings 25's and 26's holds fixed ([`FIXED_UNDER_26`]) and no envelope changing section inside
/// the clock window.
#[test]
#[ignore = "slow: 10⁵ bodies of 6,800 systems, on sixteen threads"]
fn composition_census_reaches_every_row_or_lists_why() {
    let census = run("the ordinary census", ORDINARY_SYSTEMS, ORDINARY_THREADS);
    assert!(census.bodies >= 100_000, "{} bodies", census.bodies);
    census.assert_reached();
    census.assert_fixed_under_26();
    census.assert_no_envelope_changes();
}

/// The slow census (P14.T33.c), over [`SLOW_SYSTEMS`] systems: at least 10⁶ bodies, every registry
/// row reached or listed with its reason (a listed row it reaches is printed), none of the findings
/// 25's and 26's holds fixed ([`FIXED_UNDER_26`]) but the bodies [`LEFT_UNDER_26`] lists, and no
/// envelope changing section inside the clock window.
#[test]
#[ignore = "slow: 10⁶ bodies of 68,000 systems, on sixteen threads"]
fn composition_census_over_a_million_bodies_reaches_every_row_or_lists_why() {
    let census = run("the slow census", SLOW_SYSTEMS, SLOW_THREADS);
    assert!(census.bodies >= 1_000_000, "{} bodies", census.bodies);
    census.assert_reached();
    census.assert_fixed_under_26();
    census.assert_no_envelope_changes();
}

/// Bodies the census found off a runaway's state at 26 and under every 27 hold, by ID
/// ([`composition_census_s_steam_envelopes_are_a_runaway_s_under_every_28_hold`]): the three of
/// `supercritical_envelope_tests`'s `OFF_A_RUNAWAY_S_STATE`.
const STEAM_OFF_A_RUNAWAY_S_STATE: [&str; 3] = [
    "01fffb2d60000000.0600",
    "02000b2be0010000.0400",
    "01ffeb2b60000000.0200",
];

/// What one body of the census publishes, for P14.T53.b part 3's B5
/// ([`composition_census_s_steam_envelopes_are_a_runaway_s_under_every_28_hold`]).
#[derive(Debug, Clone)]
struct SteamStates {
    /// Its published state, the state its published solve was solved under and its water's.
    states: (SurfaceState, SurfaceState, SurfaceState),
    /// Its mean surface temperature, K, and surface pressure, bar (`NaN` for none).
    t_s: f64,
    p_s: f64,
    /// Its record's surface section, and whether that section lists H₂O liquid or solid.
    surface: SectionState,
    condensed_water: bool,
    /// Its passes, and whether they ended at a jump.
    passes: u32,
    jump: bool,
}

impl SteamStates {
    fn of(case: &AuditCase<'_>) -> Self {
        let air = case.derived.atmosphere();
        let condensed_water = case.surface().is_some_and(|surface| {
            surface.condensates().entries().iter().any(|e| {
                e.substance() == Gas::Water.substance()
                    && matches!(e.phase(), CondensatePhase::Liquid | CondensatePhase::Solid)
            })
        });
        Self {
            states: (air.published_state(), air.state(), air.water_state()),
            t_s: air.surface_temperature().value(),
            p_s: air.surface_pressure().map_or(f64::NAN, Pascals::value) / 1e5,
            surface: case.record.section_state(RecordSection::Surface),
            condensed_water,
            passes: case.derived.atmosphere_passes(),
            jump: case.derived.convergence().is_jump(),
        }
    }

    /// Whether it publishes the same as `other`, its figures to the bit.
    fn same(&self, other: &Self) -> bool {
        self.states == other.states
            && self.t_s.total_cmp(&other.t_s).is_eq()
            && self.p_s.total_cmp(&other.p_s).is_eq()
            && self.surface == other.surface
            && self.condensed_water == other.condensed_water
            && (self.passes, self.jump) == (other.passes, other.jump)
    }

    /// Whether it is a steam envelope over a state that is not a runaway's.
    fn off_a_runaway_s_state(&self) -> bool {
        self.states.0 == SurfaceState::SteamEnvelope
            && self.states.1 != SurfaceState::RunawayGreenhouse
    }

    fn line(&self) -> String {
        format!(
            "{:?} over {:?} (water {:?}), T_s {:.3} K, p_s {:.4e} bar, surface {:?}{}, {} \
             passes{}",
            self.states.0,
            self.states.1,
            self.states.2,
            self.t_s,
            self.p_s,
            self.surface,
            if self.condensed_water {
                " with H2O condensed"
            } else {
                ""
            },
            self.passes,
            if self.jump { ", a jump" } else { "" }
        )
    }
}

/// Every body of the census under one set of rules, by "#{rank} {id} {kind}".
type SteamCensus = BTreeMap<String, SteamStates>;

/// The census's [`SteamStates`] under `rules`, which wraps each share's generation on its own
/// thread, printed under `title`: its steam envelopes over each state and those off a runaway's
/// state, listed.
fn steam_census(
    title: &str,
    rules: impl Fn(&dyn Fn() -> SteamCensus) -> SteamCensus + Sync,
) -> SteamCensus {
    let start = Instant::now();
    let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way's parameters build a galaxy");
    let records = sample_records(&galaxy, ORDINARY_SYSTEMS, SampleFilter::ALL)
        .expect("the solar circle holds the census's systems");
    let shares = records.len().div_ceil(SYSTEMS_PER_SHARE);
    let mut all = SteamCensus::new();
    for share in in_shares(shares, ORDINARY_THREADS, |k| {
        let first = k * SYSTEMS_PER_SHARE;
        let end = records.len().min(first + SYSTEMS_PER_SHARE);
        rules(&|| {
            let mut bodies = SteamCensus::new();
            for (rank, record) in (first..).zip(&records[first..end]) {
                let ctx = SystemContext::from_record(&galaxy, record);
                let system = generate(SEED, &ctx);
                audit_walk_system("census", &ctx, &system, &mut |case| {
                    let label = format!("#{rank} {} {}", case.record.identity().id(), case.kind);
                    bodies.insert(label, SteamStates::of(case));
                });
            }
            bodies
        })
    }) {
        all.extend(share);
    }
    let mut over = BTreeMap::<String, usize>::new();
    for body in all.values() {
        if body.states.0 == SurfaceState::SteamEnvelope {
            *over
                .entry(format!("{:?} (water {:?})", body.states.1, body.states.2))
                .or_default() += 1;
        }
    }
    let off: Vec<_> = all
        .iter()
        .filter(|(_, body)| body.off_a_runaway_s_state())
        .collect();
    eprintln!(
        "{title}: {} bodies; steam envelopes over each state {over:?}; off a runaway's state {} \
         ({:.1} s):",
        all.len(),
        off.len(),
        start.elapsed().as_secs_f64()
    );
    for (label, body) in off {
        eprintln!("  {label}: {}", body.line());
    }
    all
}

/// P14.T53.b part 3's B5 over the ordinary census's systems (decision-p14-t53b-part3 §5 and §8), at
/// 26, under every 27 hold and under every 28 hold: the steam envelopes over each state and those
/// off a runaway's state, listed, [`STEAM_OFF_A_RUNAWAY_S_STATE`] among them at 26 and 27. Under
/// every 28 hold (`STEAM_DIVISION_VERSION`) none is off a runaway's state but a magma ocean's, which
/// the label precedes (§5), and every body 28's hold moves keeps B1 (no steam envelope or runaway
/// listing H₂O condensed), B4 (a finite temperature) and B5 (no steam envelope with a surface
/// section), listed.
#[test]
#[ignore = "slow: 10⁵ bodies of 6,800 systems three times, on eight threads"]
fn composition_census_s_steam_envelopes_are_a_runaway_s_under_every_28_hold() {
    use crate::planetary::testing::{with_every_27_hold, with_every_28_hold};
    let at_26 = steam_census("at 26", |run| run());
    let at_27 = steam_census("under every 27 hold", |run| with_every_27_hold(run));
    let at_28 = steam_census("under every 28 hold", |run| with_every_28_hold(run));
    for on in [&at_26, &at_27, &at_28] {
        assert!(on.len() >= 100_000, "{} bodies", on.len());
    }
    let listed = |on: &SteamCensus, id: &str| {
        on.iter()
            .any(|(label, body)| label.contains(id) && body.off_a_runaway_s_state())
    };
    for id in STEAM_OFF_A_RUNAWAY_S_STATE {
        assert!(listed(&at_26, id), "{id} at 26");
        assert!(listed(&at_27, id), "{id} under every 27 hold");
    }
    let mut failures = Vec::new();
    for (label, body) in &at_28 {
        if body.off_a_runaway_s_state() && body.states.1 != SurfaceState::MagmaOcean {
            failures.push(format!("B5: {label}: {}", body.line()));
        }
    }
    eprintln!("moved by 28's hold:");
    let mut moved = 0_usize;
    for (label, after) in &at_28 {
        let Some(before) = at_27.get(label) else {
            failures.push(format!("{label} under every 28 hold alone"));
            continue;
        };
        if before.same(after) {
            continue;
        }
        moved += 1;
        eprintln!("  {label}: {} → {}", before.line(), after.line());
        let runaway_or_steam = after.states.1 == SurfaceState::RunawayGreenhouse
            || after.states.0 == SurfaceState::SteamEnvelope;
        if runaway_or_steam && after.condensed_water {
            failures.push(format!("B1: {label}: {}", after.line()));
        }
        if after.states.0 == SurfaceState::SteamEnvelope && after.surface == SectionState::Ok {
            failures.push(format!("B5 surface: {label}: {}", after.line()));
        }
        if !(after.t_s.is_finite() && after.t_s > 0.0) {
            failures.push(format!("B4: {label}: {}", after.line()));
        }
    }
    eprintln!("moved by 28's hold: {moved}");
    assert_eq!(at_28.len(), at_27.len(), "28's hold moves no body's walk");
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The first `systems` systems of the census's sample under every hold of version 27, each body
/// walked ([`audit_walk_system`]) into its share's tally with its system's rank and context, the
/// tallies in sample order.
///
/// The galaxy is built under `GalaxyRules::every_27_hold`, its own and the stellar holds, and each
/// share's systems are generated under [`with_every_27_hold`] on its own thread, since the
/// planetary holds' switches are each thread's.
fn tallies_under_every_27_hold<T: Default + Send>(
    systems: usize,
    visit: impl Fn(&mut T, usize, &SystemContext, &AuditCase<'_>) + Sync,
) -> Vec<T> {
    let galaxy = Galaxy::from_params_with_rules(
        SEED,
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_27_hold(),
    )
    .expect("the Milky Way's parameters build a galaxy");
    let records = sample_records(&galaxy, systems, SampleFilter::ALL)
        .expect("the solar circle holds the census's systems");
    let shares = records.len().div_ceil(SYSTEMS_PER_SHARE);
    in_shares(shares, ORDINARY_THREADS, |k| {
        let first = k * SYSTEMS_PER_SHARE;
        let end = records.len().min(first + SYSTEMS_PER_SHARE);
        let mut tally = T::default();
        with_every_27_hold(|| {
            for (rank, record) in (first..).zip(&records[first..end]) {
                let ctx = SystemContext::from_record(&galaxy, record);
                let system = generate(SEED, &ctx);
                audit_walk_system("census", &ctx, &system, &mut |case| {
                    visit(&mut tally, rank, &ctx, case);
                });
            }
        });
        tally
    })
}

/// "#{rank} {id} {kind}", by which the counts below name a body.
fn named(rank: usize, case: &AuditCase<'_>) -> String {
    format!("#{rank} {} {}", case.record.identity().id(), case.kind)
}

/// What P14.T51.m's coupled condensables would change on a surface that holds both `H2O` and
/// `NH3` at a mean temperature at or above `nh3_h2o`'s eutectic (research-galaxy-close-out §3), by
/// the phases its record publishes them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AmmoniaWater {
    /// Liquid `NH3` beside water ice, which would lie as `nh3_h2o`.
    LiquidBesideIce,
    /// Liquid `NH3` beside liquid water, which would lie together as `nh3_h2o`.
    LiquidBesideLiquid,
    /// Both solid below `NH3`'s own melt, where a eutectic melt would turn the water ice liquid.
    EutecticMelt,
    /// Any other pair: both solid at or above `NH3`'s own melt (its ice lying colder than the
    /// mean), or a supercritical member.
    Other,
}

/// The census's count of the surfaces that hold both `H2O` and `NH3`.
#[derive(Debug, Default)]
struct AmmoniaWaterCount {
    /// Bodies walked.
    bodies: usize,
    /// Bodies whose published condensates list `H2O`, at any temperature.
    water: usize,
    /// Bodies whose surface's air holds `NH3`, at any temperature.
    airborne_ammonia: usize,
    /// Bodies whose published condensates list `NH3`, at any temperature.
    ammonia: usize,
    /// Of those, the bodies that list `H2O` too.
    both: usize,
    /// Of those, the bodies at a mean surface temperature at or above the eutectic, by what would
    /// change, each named.
    movers: BTreeMap<AmmoniaWater, Vec<String>>,
}

/// The temperatures [`AmmoniaWaterCount`] reads, the registry's: `nh3_h2o`'s eutectic, its first
/// melt, and `NH3`'s own melt, its triple point.
#[derive(Debug, Clone, Copy)]
struct AmmoniaWaterMelts {
    /// `nh3_h2o`'s eutectic, below which the pair is solid.
    eutectic: Kelvin,
    /// `NH3`'s own melt.
    ammonia: Kelvin,
}

impl AmmoniaWaterMelts {
    /// The registry's.
    fn of_registry() -> Self {
        let eutectic = by_key("nh3_h2o")
            .and_then(|id| id.substance().condensed())
            .expect("nh3_h2o is a registry row with condensed columns")
            .transition()
            .temperature();
        let ammonia = Gas::Ammonia
            .substance()
            .substance()
            .phase_diagram()
            .expect("NH3 has a phase diagram")
            .triple_temperature();
        Self { eutectic, ammonia }
    }
}

impl AmmoniaWaterCount {
    /// Counts `case`, of the sample's system `rank`.
    fn add(&mut self, melts: AmmoniaWaterMelts, rank: usize, case: &AuditCase<'_>) {
        self.bodies += 1;
        let Some(surface) = case.surface() else {
            return;
        };
        let (ammonia, water) = (Gas::Ammonia.substance(), Gas::Water.substance());
        let phases = |id: SubstanceId| -> Vec<CondensatePhase> {
            surface
                .condensates()
                .entries()
                .iter()
                .filter(|e| e.substance() == id)
                .map(CondensateEntry::phase)
                .collect()
        };
        let (nh3, h2o) = (phases(ammonia), phases(water));
        self.water += usize::from(!h2o.is_empty());
        let gases = surface.conditions().gases();
        self.airborne_ammonia += usize::from(gases.iter().any(|&(id, _)| id == ammonia));
        if nh3.is_empty() {
            return;
        }
        self.ammonia += 1;
        if h2o.is_empty() {
            return;
        }
        self.both += 1;
        let t_s = surface.conditions().mean_surface_temperature();
        if t_s < melts.eutectic {
            return;
        }
        let has = |of: &[CondensatePhase], phase| of.contains(&phase);
        let kind = if has(&nh3, CondensatePhase::Liquid) {
            if has(&h2o, CondensatePhase::Liquid) {
                AmmoniaWater::LiquidBesideLiquid
            } else if has(&h2o, CondensatePhase::Solid) {
                AmmoniaWater::LiquidBesideIce
            } else {
                AmmoniaWater::Other
            }
        } else if has(&nh3, CondensatePhase::Solid)
            && has(&h2o, CondensatePhase::Solid)
            && t_s < melts.ammonia
        {
            AmmoniaWater::EutecticMelt
        } else {
            AmmoniaWater::Other
        };
        self.movers.entry(kind).or_default().push(format!(
            "{} at {:.2} K: NH3 {nh3:?}, H2O {h2o:?}",
            named(rank, case),
            t_s.value()
        ));
    }

    /// Adds `other`, a later share's.
    fn merge(&mut self, other: Self) {
        self.bodies += other.bodies;
        self.water += other.water;
        self.airborne_ammonia += other.airborne_ammonia;
        self.ammonia += other.ammonia;
        self.both += other.both;
        for (kind, bodies) in other.movers {
            self.movers.entry(kind).or_default().extend(bodies);
        }
    }

    /// The count, with every mover named.
    fn report(&self, melts: AmmoniaWaterMelts) -> String {
        let mut report = format!(
            "P14.T51.m's count under every 27 hold: {} bodies; {} list H2O at their surface and {} \
             carry NH3 in their air; {} list NH3 at their surface, {} with H2O; eutectic {:.2} K, \
             NH3's melt {:.2} K\n",
            self.bodies,
            self.water,
            self.airborne_ammonia,
            self.ammonia,
            self.both,
            melts.eutectic.value(),
            melts.ammonia.value()
        );
        let line = "a string takes a line";
        for (kind, bodies) in &self.movers {
            writeln!(report, "  {kind:?}: {}", bodies.len()).expect(line);
            for body in bodies {
                writeln!(report, "    {body}").expect(line);
            }
        }
        report
    }
}

/// P14.T51.m's count (research-galaxy-close-out §3, step 1): over the ordinary census under every
/// hold of version 27, the bodies whose surface holds both `H2O` and `NH3` at a mean surface
/// temperature at or above `nh3_h2o`'s eutectic, 176.15 K, counted by what the coupled
/// condensables would change ([`AmmoniaWater`]). It finds none: no surface lists `NH3`, or holds it
/// in its air, at any temperature, so T51.m is ruled out on its count, and a body this test finds is
/// the case for building it.
#[test]
#[ignore = "slow: 10⁵ bodies of 6,800 systems under every 27 hold, on sixteen threads"]
fn composition_census_under_every_27_hold_holds_no_ammonia_beside_water_above_their_eutectic() {
    let melts = AmmoniaWaterMelts::of_registry();
    let start = Instant::now();
    let mut count = AmmoniaWaterCount::default();
    for share in tallies_under_every_27_hold(
        ORDINARY_SYSTEMS,
        |tally: &mut AmmoniaWaterCount, rank, _, case| tally.add(melts, rank, case),
    ) {
        count.merge(share);
    }
    let report = count.report(melts);
    eprintln!("{report}({:.1} s)", start.elapsed().as_secs_f64());
    assert!(count.bodies >= 100_000, "{} bodies", count.bodies);
    assert!(
        count.movers.is_empty(),
        "a body holds NH3 beside H2O above their eutectic: P14.T51.m is no longer ruled out\n{report}"
    );
}

/// The mass under which Perez-Becker and Chiang find a rocky planet above
/// [`EVAPORATING_SURFACE`] gone: 0.1 M⊕. "Rocky planets with masses < 0.1 `M_Earth` … and surface
/// temperatures > 2000 K are found to disintegrate entirely in < 10 Gyr" (Perez-Becker and Chiang
/// 2013, MNRAS 433, 2294; arXiv:1302.2147, abstract, as decision-triton-and-e101 reads it).
const EVAPORATING_MASS: EarthMasses = EarthMasses::new(0.1);

/// The surface temperature above which Perez-Becker and Chiang find a rocky planet under
/// [`EVAPORATING_MASS`] gone: 2,000 K ([`EVAPORATING_MASS`]'s source).
const EVAPORATING_SURFACE: Kelvin = Kelvin::new(2_000.0);

/// The edges of P14.T55.b's system-age bins (research-galaxy-close-out §4), in Gyr: under 1, 1–10
/// and over 10.
const SURVIVOR_AGE_EDGES_GYR: [f64; 2] = [1.0, 10.0];

/// The bodies under [`EVAPORATING_MASS`] whose warmest zone is above [`EVAPORATING_SURFACE`] in a
/// system at least 1 Gyr old that the census publishes present under every 27 hold, by their
/// system's rank and their ID; none is above it at its mean. Each is hot by its hosts' light alone,
/// as under every 26 hold, since its host left the main sequence: four moons of a 21 M⊕ planet at
/// 1,012 S⊕ about a 1.23 M☉ star on its first giant branch, at 5.90 Gyr, at 2,008–2,045 K; and a
/// belt member of 2.8 × 10⁻⁵ M⊕ at 1,407 S⊕ about a core-helium-burning 2.1 M☉ star, at 1.05 Gyr,
/// at 2,214 K. P14.T55.b.a's evaporation removes them under every 28 hold, each on its host's main
/// sequence.
const HOT_SMALL_SURVIVORS: [(usize, &str); 5] = [
    (835, "42006cadffc00004.0201"),
    (835, "42006cadffc00004.0202"),
    (835, "42006cadffc00004.0203"),
    (835, "42006cadffc00004.0204"),
    (2_299, "41ffacb600000006.e101"),
];

/// The bodies under [`EVAPORATING_MASS`] above [`EVAPORATING_SURFACE`] at their mean, and so at
/// their warmest zone, in a system at least 1 Gyr old that the census publishes present under
/// every 27 hold since P14.T1.e's paired light: two captured moons of 5 × 10⁻¹² and 10⁻¹⁰ M⊕ of a
/// 2.8 M⊕ planet at 0.049 au about the accretor of a pair, grown from 0.90 to 1.53 M☉ and now at
/// 10 L☉, at 2,184 K, at 2.49 Gyr. P14.T55.b.a's evaporation removes them under every 28 hold.
const PAIRED_LIGHT_SURVIVORS: [(usize, &str); 2] = [
    (6_630, "4200aca9ffc00001.0501"),
    (6_630, "4200aca9ffc00001.0502"),
];

/// The census's count of the present bodies under [`EVAPORATING_MASS`] whose surface is above
/// [`EVAPORATING_SURFACE`].
#[derive(Debug, Default)]
struct SurvivorCount {
    /// Bodies walked.
    bodies: usize,
    /// The hot small bodies by the temperature read ("mean" or "warmest zone"), their system's
    /// age bin (an index into [`SURVIVOR_AGE_EDGES_GYR`]'s three bins) and their kind.
    by_age: BTreeMap<(&'static str, usize, &'static str), usize>,
    /// Those of them in a system at least 1 Gyr old, by the temperature read: each one's system's
    /// rank, its ID and its line.
    old: BTreeMap<&'static str, Vec<(usize, String, String)>>,
}

impl SurvivorCount {
    /// Counts the present body of `case`, of the system of rank `rank` and context `ctx`, if it is
    /// under [`EVAPORATING_MASS`] and above [`EVAPORATING_SURFACE`] at its mean or its warmest
    /// zone.
    fn add(&mut self, rank: usize, ctx: &SystemContext, case: &AuditCase<'_>) {
        self.bodies += 1;
        let mass = case
            .record
            .mass()
            .ok()
            .copied()
            .unwrap_or(case.derived.mass());
        if mass >= EVAPORATING_MASS {
            return;
        }
        let conditions = case.surface().map(Surface::conditions);
        let mean = conditions.map_or(
            case.derived.surface_temperature(),
            SurfaceConditions::mean_surface_temperature,
        );
        let warmest = conditions.map_or(mean, |c| c.climate_range().1);
        let age_gyr = ctx.age_at_epoch().value() / 1e9;
        let bin = age_bin(age_gyr);
        for (read, t) in [("mean", mean), ("warmest zone", warmest)] {
            if t <= EVAPORATING_SURFACE {
                continue;
            }
            *self.by_age.entry((read, bin, case.kind)).or_default() += 1;
            if bin > 0 {
                self.old.entry(read).or_default().push((
                    rank,
                    case.record.identity().id().to_string(),
                    format!(
                        "{} of {:.3e} M⊕ at {:.0} K, {age_gyr:.2} Gyr",
                        named(rank, case),
                        mass.value(),
                        t.value()
                    ),
                ));
            }
        }
    }

    /// Adds `share`'s counts.
    fn merge(&mut self, share: Self) {
        self.bodies += share.bodies;
        for (key, n) in share.by_age {
            *self.by_age.entry(key).or_default() += n;
        }
        for (read, bodies) in share.old {
            self.old.entry(read).or_default().extend(bodies);
        }
    }

    /// The count's report, under every hold of `version`.
    fn report(&self, version: u16) -> String {
        let line = "a string takes a line";
        let mut report = format!(
            "P14.T55.b's survivors under every {version} hold: {} bodies; under {} M⊕ above {} K\n",
            self.bodies,
            EVAPORATING_MASS.value(),
            EVAPORATING_SURFACE.value()
        );
        for ((read, bin, kind), n) in &self.by_age {
            writeln!(report, "  by its {read}: {} {kind}: {n}", AGE_BINS[*bin]).expect(line);
        }
        for (read, bodies) in &self.old {
            writeln!(report, "  past 1 Gyr by its {read}: {}", bodies.len()).expect(line);
            for (_, _, body) in bodies {
                writeln!(report, "    {body}").expect(line);
            }
        }
        report
    }

    /// The bodies past 1 Gyr above [`EVAPORATING_SURFACE`] at their warmest zone, by their
    /// system's rank and their ID, in order.
    fn old_at_their_warmest(&self) -> Vec<(usize, &str)> {
        let mut survivors: Vec<(usize, &str)> = self
            .old
            .get("warmest zone")
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|(rank, id, _)| (*rank, id.as_str()))
            .collect();
        survivors.sort_unstable();
        survivors
    }
}

/// The names of the age bins of [`SURVIVOR_AGE_EDGES_GYR`].
const AGE_BINS: [&str; 3] = ["< 1 Gyr", "1–10 Gyr", "> 10 Gyr"];

/// The bin of [`SURVIVOR_AGE_EDGES_GYR`] a system `age_gyr` old falls in.
fn age_bin(age_gyr: f64) -> usize {
    SURVIVOR_AGE_EDGES_GYR
        .iter()
        .filter(|&&edge| age_gyr >= edge)
        .count()
}

/// P14.T55.b's survival count (research-galaxy-close-out §4): over the ordinary census under every
/// hold of version 27, the present bodies under 0.1 M⊕ whose surface is above 2,000 K, by their
/// system's age, under 1, 1–10 and over 10 Gyr, the surface read at its mean temperature (the
/// record's, or its derivation's where its surface is not `Ok`) and at its warmest zone. The two
/// [`PAIRED_LIGHT_SURVIVORS`] are above it at their mean, and they and the five
/// [`HOT_SMALL_SURVIVORS`] at their warmest zone past 1 Gyr: bodies that would have evaporated,
/// pinned under 27's rules, which P14.T55.b.a removes at 28
/// ([`composition_census_under_every_28_hold_publishes_no_hot_small_body_past_its_evaporation`]).
#[test]
#[ignore = "slow: 10⁵ bodies of 6,800 systems under every 27 hold, on sixteen threads"]
fn composition_census_under_every_27_hold_publishes_seven_hot_small_bodies_past_1_gyr() {
    let start = Instant::now();
    let mut count = SurvivorCount::default();
    for share in tallies_under_every_27_hold(
        ORDINARY_SYSTEMS,
        |tally: &mut SurvivorCount, rank, ctx, case| tally.add(rank, ctx, case),
    ) {
        count.merge(share);
    }
    let report = count.report(27);
    eprintln!("{report}({:.1} s)", start.elapsed().as_secs_f64());
    assert!(count.bodies >= 100_000, "{} bodies", count.bodies);
    let mut at_their_mean: Vec<(usize, &str)> = count
        .old
        .get("mean")
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|(rank, id, _)| (*rank, id.as_str()))
        .collect();
    at_their_mean.sort_unstable();
    assert_eq!(at_their_mean, PAIRED_LIGHT_SURVIVORS, "{report}");
    let mut survivors: Vec<(usize, &str)> = HOT_SMALL_SURVIVORS
        .into_iter()
        .chain(PAIRED_LIGHT_SURVIVORS)
        .collect();
    survivors.sort_unstable();
    assert_eq!(count.old_at_their_warmest(), survivors, "{report}");
}

/// [`tallies_under_every_27_hold`]'s shares under every hold of version 28 instead
/// ([`with_every_28_hold`](crate::planetary::testing::with_every_28_hold)), each system visited
/// whole: the galaxy's rules are 27's, since no hold of the galaxy's is at 28.
fn tallies_under_every_28_hold<T: Default + Send>(
    systems: usize,
    visit: impl Fn(&mut T, usize, &SystemContext, &PlanetarySystem) + Sync,
) -> Vec<T> {
    let galaxy = Galaxy::from_params_with_rules(
        SEED,
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_27_hold(),
    )
    .expect("the Milky Way's parameters build a galaxy");
    let records = sample_records(&galaxy, systems, SampleFilter::ALL)
        .expect("the solar circle holds the census's systems");
    let shares = records.len().div_ceil(SYSTEMS_PER_SHARE);
    in_shares(shares, ORDINARY_THREADS, |k| {
        let first = k * SYSTEMS_PER_SHARE;
        let end = records.len().min(first + SYSTEMS_PER_SHARE);
        let mut tally = T::default();
        crate::planetary::testing::with_every_28_hold(|| {
            for (rank, record) in (first..).zip(&records[first..end]) {
                let ctx = SystemContext::from_record(&galaxy, record);
                let system = generate(SEED, &ctx);
                visit(&mut tally, rank, &ctx, &system);
            }
        });
        tally
    })
}

/// The census's count of the bodies P14.T55.b.a's evaporation has taken by the epoch, beside the
/// hot small bodies it publishes present ([`SurvivorCount`]), and of the belts whose named members
/// it has all taken.
#[derive(Debug, Default)]
struct EvaporationCount {
    /// The present hot small bodies.
    survivors: SurvivorCount,
    /// The bodies `Destroyed { Evaporated }` at the epoch, by their system's age bin (an index
    /// into [`AGE_BINS`]) and their kind.
    by_age: BTreeMap<(usize, &'static str), usize>,
    /// Each of them: its system's rank, its ID, whether some star of its zone had left its main
    /// sequence when it went, and its line.
    taken: Vec<(usize, String, bool, String)>,
}

impl EvaporationCount {
    /// Counts the bodies of `system`, of rank `rank` and context `ctx`, that its records publish
    /// `Destroyed { Evaporated }` at the epoch: a planet's or a member's history's ending, and a
    /// moon's state as its record reads it ([`moon_state`] on its planet's history,
    /// [`BodyFate::moon_evaporation`]).
    fn add(&mut self, rank: usize, ctx: &SystemContext, system: &PlanetarySystem) {
        let t = UniverseTime::EPOCH;
        let hosts: Vec<FateHost<'_>> = system
            .zones
            .iter()
            .map(|zone| fate_host(ctx, zone))
            .collect();
        let fates = system.fates(&hosts);
        let fate_of = |index: BodyIndex| {
            system
                .bodies
                .iter()
                .zip(&fates)
                .find(|(body, _)| body.index == index)
                .and_then(|(_, fate)| fate.as_ref())
        };
        let age_gyr = ctx.age_at_epoch().value() / 1e9;
        // A belt whose named members have all evaporated ends with the last of them.
        for (belt, zone) in system.belts.iter().filter_map(|belt| {
            system
                .zones
                .iter()
                .position(|zone| zone.host() == belt.host())
                .map(|k| (belt, k))
        }) {
            let ended = system
                .belt_evaporation(belt, &hosts[zone], t)
                .and_then(|evaporation| evaporation.ended);
            if ended.is_some_and(|at| at <= t) {
                *self.by_age.entry((age_bin(age_gyr), "belts")).or_default() += 1;
            }
        }
        for body in &system.bodies {
            let (kind, state) = match &body.part {
                Part::Planet(_) | Part::Member(_) => {
                    let fate = fate_of(body.index).expect("a planet or member has a history");
                    let kind = if matches!(body.part, Part::Planet(_)) {
                        "planets"
                    } else {
                        "members"
                    };
                    (kind, fate.at(t).state())
                }
                Part::Moon(moon) => {
                    let OrbitHost::Body(parent) = body.host else {
                        unreachable!("a moon orbits a body")
                    };
                    let Some(fate) = fate_of(parent) else {
                        continue;
                    };
                    let evaporated = fate.moon_evaporation(moon.satellite.mass());
                    let state = moon_state(
                        fate.at(t).state(),
                        &moon.satellite,
                        ctx.age_at_epoch(),
                        t,
                        evaporated,
                    );
                    ("moons", state)
                }
                Part::Ring(_) => continue,
            };
            let BodyState::Destroyed {
                cause: DestructionCause::Evaporated,
                at,
            } = state
            else {
                continue;
            };
            *self.by_age.entry((age_bin(age_gyr), kind)).or_default() += 1;
            let evolved = system.zone_of(body).members().any(|member| {
                ctx.history_state_at(usize::from(member), at)
                    .is_some_and(|state| past_main_sequence(state.phase()))
            });
            let gone_gyr = ctx.age_at(at).value() / 1e9;
            self.taken.push((
                rank,
                body.index.body_id(system.system).to_string(),
                evolved,
                format!(
                    "#{rank} {} {kind} of {:.3e} M⊕, gone at {gone_gyr:.3} Gyr of {age_gyr:.2}{}",
                    body.index.body_id(system.system),
                    body.mass().value(),
                    if evolved { ", its host evolved" } else { "" }
                ),
            ));
        }
    }

    /// Adds `share`'s counts.
    fn merge(&mut self, share: Self) {
        self.survivors.merge(share.survivors);
        for (key, n) in share.by_age {
            *self.by_age.entry(key).or_default() += n;
        }
        self.taken.extend(share.taken);
    }
}

/// P14.T55.b.a's twin of
/// [`composition_census_under_every_27_hold_publishes_seven_hot_small_bodies_past_1_gyr`]: over the
/// ordinary census under every hold of version 28, no body under 0.1 M⊕ is published above
/// 2,000 K past 1 Gyr, at its mean or at its warmest zone, and each of the five
/// [`HOT_SMALL_SURVIVORS`] and the two [`PAIRED_LIGHT_SURVIVORS`] is `Destroyed { Evaporated }`.
/// Each goes while every star of its zone is on its main sequence: the five, at the source's
/// substellar zero-albedo temperature, about 1,600–1,900 K there, where a rock of
/// 10⁻⁷–3 × 10⁻⁵ M⊕ is gone within a few hundred million years; the two once their accretor has
/// brightened. The bodies evaporated by the epoch are counted by their system's age, the young
/// systems' `e101`-class bodies among them.
#[test]
#[ignore = "slow: 10⁵ bodies of 6,800 systems under every 28 hold, on sixteen threads"]
fn composition_census_under_every_28_hold_publishes_no_hot_small_body_past_its_evaporation() {
    let start = Instant::now();
    let mut count = EvaporationCount::default();
    for share in tallies_under_every_28_hold(
        ORDINARY_SYSTEMS,
        |tally: &mut EvaporationCount, rank, ctx, system| {
            audit_walk_system("census", ctx, system, &mut |case| {
                tally.survivors.add(rank, ctx, case);
            });
            tally.add(rank, ctx, system);
        },
    ) {
        count.merge(share);
    }
    count.taken.sort_unstable();
    let line = "a string takes a line";
    let mut report = count.survivors.report(28);
    writeln!(report, "  evaporated by the epoch: {}", count.taken.len()).expect(line);
    for ((bin, kind), n) in &count.by_age {
        writeln!(report, "    {} {kind}: {n}", AGE_BINS[*bin]).expect(line);
    }
    for (_, _, _, body) in &count.taken {
        writeln!(report, "    {body}").expect(line);
    }
    eprintln!("{report}({:.1} s)", start.elapsed().as_secs_f64());
    // The census's 10⁵ bodies, each walked present or counted evaporated.
    let bodies = count.survivors.bodies + count.taken.len();
    assert!(bodies >= 100_000, "{bodies} bodies");
    assert!(
        count.survivors.old.is_empty(),
        "a body under 0.1 M⊕ is published above 2,000 K past 1 Gyr under every 28 hold\n{report}"
    );
    for (rank, id) in HOT_SMALL_SURVIVORS
        .into_iter()
        .chain(PAIRED_LIGHT_SURVIVORS)
    {
        let taken = count
            .taken
            .iter()
            .find(|(r, i, ..)| *r == rank && i == id)
            .unwrap_or_else(|| panic!("#{rank} {id} is not evaporated\n{report}"));
        assert!(
            !taken.2,
            "#{rank} {id} went once its host evolved, not on its main sequence\n{report}"
        );
    }
}
