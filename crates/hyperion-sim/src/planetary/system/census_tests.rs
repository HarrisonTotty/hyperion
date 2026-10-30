//! P14.T33.c, the composition census (decision-composition; plan 14's T33.c): the generated galaxy's
//! composition measured over real systems, as generator version 24 builds their records, and as
//! version 25 will under every hold at 25 ([`Rules`]).
//!
//! The systems are `planetary::testing`'s volume-limited sample about the Sun-like point, the
//! system tests' universe ([`SEED`]), nearest first, so that the first 400 are the sample of every
//! other plan-14 census. Each is generated whole and walked as the audit walks it
//! ([`audit_walk_system`]): every planet, member and moon present at the epoch with a bulk section,
//! its record as `snapshot_at` builds it and its derivation. The ordinary census reads
//! [`ORDINARY_SYSTEMS`] systems, at least 10⁵ bodies, and the slow one [`SLOW_SYSTEMS`], at least
//! 10⁶, each at version 24 and under every 25 hold: four slow tests, each on threads of its own
//! (`.config/nextest.toml` reserves their slots). The counts do not depend on the threads: each
//! share's systems are fixed, and the shares are merged in order.
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
//! rows whose condensed optics are stand-ins, and the solid surfaces' ages by tectonic regime. None
//! of those is asserted at 24: they are the census's findings, printed for review. Under every 25
//! hold the findings a hold at 25 fixes are asserted absent ([`FIXED_UNDER_25`]).
//!
//! The tests are native only: on WebAssembly, which has no threads, the shares would run one after
//! another, thousands of systems on one thread, and the records' bit-for-bit agreement there is
//! the goldens' to check, not a census's.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use hyperion_testkit::threads::in_shares;

use super::audit_tests::{AuditBody, AuditCase, Item, audit_walk_system};
use super::tests::SEED;
use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::SystemRecord;
use crate::planetary::chem::condense::NotFormed;
use crate::planetary::chem::partition::CondensateEntry;
use crate::planetary::derive::atmosphere::{
    AIRLESS_PRESSURE, Gas, SurfaceState, WATER_MELTING_POINT,
};
use crate::planetary::hooks::aerosols::{AerosolDeck, AerosolMode};
use crate::planetary::hooks::figures::Subsurface;
use crate::planetary::record::{Modelled, RecordSection, SectionState};
use crate::planetary::testing::{SampleFilter, sample_records, with_every_25_hold};
use crate::substance::{self, CondensedOptics, SubstanceId};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// The ordinary census's systems: about 15 bodies each with a bulk section (91,781 in the first
/// 6,000), so at least 10⁵.
const ORDINARY_SYSTEMS: usize = 6_800;

/// The slow census's systems, for at least 10⁶ bodies.
const SLOW_SYSTEMS: usize = 68_000;

/// The threads the ordinary census runs on, the slots `.config/nextest.toml` reserves for it.
const ORDINARY_THREADS: usize = 8;

/// The threads the slow census runs on, the slots `.config/nextest.toml` reserves for it.
const SLOW_THREADS: usize = 8;

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

/// The registry rows no record of the census reaches, each with the reason; the ordinary census
/// holds the list exact.
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
            SurfaceState::Temperate | SurfaceState::Snowball | SurfaceState::RunawayGreenhouse
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
        if case.kind == "moons" || !(gas_envelope || epoch != SectionState::NotApplicable) {
            return;
        }
        self.envelopes_watched += 1;
        let section = |t: UniverseTime| {
            system
                .body_at(ctx, case.body.index, t)
                .expect("a walked body is in its system")
                .section_state(RecordSection::Envelope)
        };
        let mut changes = Vec::new();
        for (name, t) in [
            ("the window's start", ClockWindow::START),
            ("its end", ClockWindow::END),
        ] {
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
             with a recorded change inside it)",
            self.envelope_changes.len(),
            self.envelopes_watched,
            self.envelopes_with_a_change
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
        assert_eq!(shown, Vec::new(), "findings 25's holds fix");
    }

    /// Asserts that every registry row is reached or listed in [`UNREACHED`], each listed key a
    /// row; and for [`Listing::Exact`], also that no listed row is reached.
    fn assert_reached(&self, listing: Listing) {
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
        if listing == Listing::Exact {
            let reached: Vec<&str> = UNREACHED
                .iter()
                .map(|(k, _)| *k)
                .filter(|k| substance::by_key(k).is_some_and(|id| self.reached.contains_key(&id)))
                .collect();
            assert_eq!(
                reached,
                Vec::<&str>::new(),
                "rows listed as never reached that a record reaches"
            );
        }
    }
}

/// The findings of the census at version 24 that a hold at 25 fixes, each held at none under
/// every 25 hold ([`Census::assert_fixed_under_25`]): the audit's M6 on a rocky belt's hot
/// ice-bulk members, P14.T56.g's lag asked of them (`member_lag`).
const FIXED_UNDER_25: [&str; 1] = ["the audit's M6 fails on members"];

/// How a census holds [`UNREACHED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Listing {
    /// Every row never reached is listed, and no listed row is reached.
    Exact,
    /// Every row never reached is listed; a listed row reached is printed.
    Covering,
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

/// The rules a census generates its systems under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rules {
    /// Generator version 24's, as it ships.
    At24,
    /// Every hold of version 25 in force ([`with_every_25_hold`]), set on each share's own thread,
    /// since the holds' switches are each thread's.
    Every25Hold,
}

/// The census of `records` (of `galaxy`) under `rules`: each system generated whole at [`SEED`]
/// and walked.
fn census_of(galaxy: &Galaxy, records: &[SystemRecord], first: usize, rules: Rules) -> Census {
    let mut census = Census::default();
    let mut walk = || {
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
    };
    match rules {
        Rules::At24 => walk(),
        Rules::Every25Hold => with_every_25_hold(walk),
    }
    census
}

/// The census of the sample's first `systems` systems under `rules`, on up to `threads` threads,
/// printed under `title` with its wall time.
fn run(title: &str, systems: usize, threads: usize, rules: Rules) -> Census {
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
            rules,
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
/// registry row reached or listed with its reason, and the list exact.
#[test]
#[ignore = "slow: 10⁵ bodies of 6,800 systems, on eight threads"]
fn composition_census_reaches_every_row_or_lists_why() {
    let census = run(
        "the ordinary census",
        ORDINARY_SYSTEMS,
        ORDINARY_THREADS,
        Rules::At24,
    );
    assert!(census.bodies >= 100_000, "{} bodies", census.bodies);
    census.assert_reached(Listing::Exact);
}

/// The ordinary census under every hold of version 25 ([`Rules::Every25Hold`]): at least 10⁵
/// bodies, every registry row reached or listed with its reason, and none of the findings 25's
/// holds fix ([`FIXED_UNDER_25`]).
#[test]
#[ignore = "slow: 10⁵ bodies of 6,800 systems under every 25 hold, on eight threads"]
fn composition_census_under_every_25_hold_reaches_every_row_or_lists_why() {
    let census = run(
        "the ordinary census under every 25 hold",
        ORDINARY_SYSTEMS,
        ORDINARY_THREADS,
        Rules::Every25Hold,
    );
    assert!(census.bodies >= 100_000, "{} bodies", census.bodies);
    census.assert_reached(Listing::Covering);
    census.assert_fixed_under_25();
}

/// The slow census (P14.T33.c), over [`SLOW_SYSTEMS`] systems: at least 10⁶ bodies, every registry
/// row reached or listed with its reason; a listed row it reaches is printed.
#[test]
#[ignore = "slow: 10⁶ bodies of 68,000 systems, on eight threads"]
fn composition_census_over_a_million_bodies_reaches_every_row_or_lists_why() {
    let census = run("the slow census", SLOW_SYSTEMS, SLOW_THREADS, Rules::At24);
    assert!(census.bodies >= 1_000_000, "{} bodies", census.bodies);
    census.assert_reached(Listing::Covering);
}

/// The slow census under every hold of version 25 ([`Rules::Every25Hold`]): at least 10⁶ bodies,
/// every registry row reached or listed with its reason, and none of the findings 25's holds fix
/// ([`FIXED_UNDER_25`]).
#[test]
#[ignore = "slow: 10⁶ bodies of 68,000 systems under every 25 hold, on eight threads"]
fn composition_census_under_every_25_hold_over_a_million_bodies_reaches_every_row_or_lists_why() {
    let census = run(
        "the slow census under every 25 hold",
        SLOW_SYSTEMS,
        SLOW_THREADS,
        Rules::Every25Hold,
    );
    assert!(census.bodies >= 1_000_000, "{} bodies", census.bodies);
    census.assert_reached(Listing::Covering);
    census.assert_fixed_under_25();
}
