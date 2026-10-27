//! P14.T51.g.b's census (decision-p14-t51g): with every 24 hold in force, every body with a solid
//! surface, of every kind and origin, publishes the ocean the solve gives at its record's own
//! inputs ([`record_subsurface_ocean`]), each published ocean holds the plan's invariants
//! ([`implausibility`]), no lagged body publishes one, and a moon's regime is an ice shell's
//! exactly where its own tidal branch keeps a shell of at most 60 km (Q1). The census lists what
//! the ruling asks by name: the ocean-branch moons that publish absence or none (Q2), any whose
//! surface is colder than its equilibrium temperature, the cold-branch moons that publish an ocean,
//! the ice-shell mismatches (Q1), count (xiii)'s whole hydrospheres, and its movers against the
//! hold. Over the golden systems in CI, and the 400 and LOOK72 by name.

use std::collections::{BTreeMap, BTreeSet};

use super::tests::{SEED, golden_systems, golden_systems_afresh, sample};
use super::*;
use crate::planetary::hooks::figures::interior::ICE_SHELL_LIMIT;
use crate::planetary::hooks::figures::{OceanFloor, TectonicRegime};
use crate::planetary::hooks::ocean::{
    FindShellBaseError, Hydrosphere, OceanFloorRules, WaterColumn, absence, implausibility,
    with_ocean_floor, with_ocean_rules,
};
use crate::planetary::moons::tides::{MoonTides, TidalBranch};
use crate::planetary::record::BodyRecord;
use crate::planetary::surface::RECORD_TIME;
use crate::planetary::testing::with_every_24_hold;
use crate::substance::phase::{ICE_II_III_V, ice_ii_warmest};
use crate::units::Pascals;

/// `system`'s snapshot at [`RECORD_TIME`] under every 24 hold, with the ocean's rules `rules` and
/// its context `ctx`; `None` for a system not yet formed then.
fn snapshot_under(
    system: &PlanetarySystem,
    ctx: &SystemContext,
    rules: OceanRules,
) -> Option<SystemSnapshot> {
    (ctx.age_at(RECORD_TIME).value() > 0.0).then(|| {
        with_every_24_hold(|| with_ocean_rules(rules, || system.snapshot_at(ctx, RECORD_TIME)))
    })
}

/// The record of body `index` among `records`.
fn record_of(records: &[BodyRecord], index: BodyIndex) -> Option<&BodyRecord> {
    records.iter().find(|record| record.index() == index)
}

/// A body's kind for the census's counts.
fn kind_of(body: &Body) -> &'static str {
    match &body.part {
        Part::Planet(_) => "planet",
        Part::Member(_) => "member",
        Part::Moon(part) => match part.satellite.moon() {
            SatelliteMoon::Regular(_) => "regular moon",
            SatelliteMoon::GiantImpact(_) => "giant-impact moon",
            SatelliteMoon::Captured(_) => "captured moon",
        },
        Part::Ring(_) => "ring",
    }
}

/// What a body's record read beyond its surface: its derivation's solutes, lag and equilibrium
/// temperature, its mass, and a moon's tides with its branch's ocean.
struct Derived {
    solutes: OceanSolutes,
    lag: SurfaceLag,
    equilibrium: Kelvin,
    mass: EarthMasses,
    moon: Option<(MoonTides, Subsurface)>,
}

/// The derivation of `body` as its record took it, at the epoch `epoch`; `None` for a body absent
/// then or with no derivation.
fn derived_of(
    system: &PlanetarySystem,
    ctx: &SystemContext,
    epoch: &Epoch<'_>,
    cosines: &HourCosines,
    body: &Body,
) -> Option<Derived> {
    match (&body.part, body.host) {
        (Part::Moon(part), OrbitHost::Body(parent)) => {
            let primary = system.body(parent)?;
            let host = fate_host(ctx, system.zone_of(primary));
            let fate = system.fate_of(primary, &host);
            let now = system.parent_now(epoch, cosines, parent, &fate);
            let moon = system.record_moon(epoch, body, (&now, &fate))?;
            Some(Derived {
                solutes: moon.ocean_solutes(),
                lag: moon.surface_lag(),
                equilibrium: moon.equilibrium_temperature(),
                mass: part.satellite.mass(),
                moon: Some((moon.tides(), moon.subsurface())),
            })
        }
        (Part::Planet(_) | Part::Member(_), _) => {
            let host = fate_host(ctx, system.zone_of(body));
            let fate = system.fate_of(body, &host);
            let at = fate.at(epoch.t);
            if at.state() != BodyState::Present {
                return None;
            }
            let (derived, _, _) =
                system.primary_derived(epoch, cosines, (body, &fate), &at, PassDriver::Solve);
            Some(Derived {
                solutes: derived.ocean_solutes(),
                lag: derived.surface_lag(),
                equilibrium: derived.equilibrium_temperature(),
                mass: derived.mass(),
                moon: None,
            })
        }
        (Part::Moon(_) | Part::Ring(_), _) => None,
    }
}

/// What P14.T51.g.b publishes over a set of systems ([`Census::visit`]), every 24 hold in force.
#[derive(Debug, Default)]
struct Census {
    /// Records with a solid surface.
    surfaces: u32,
    /// The published oceans, absent members and nones by kind, and the oceans by floor.
    by_kind: BTreeMap<String, u32>,
    /// Count (xiii): the oceans whose whole hydrosphere below the shell is liquid on rock.
    whole: Vec<String>,
    /// Ocean-branch moons publishing absence or none (by the surface's liquidus or not), and
    /// moons whose surface is colder than their equilibrium temperature (Q2).
    branch: Vec<String>,
    /// Cold- or heated-branch moons publishing an ocean, with `T_eq`, `T_s`, `p_s`, F and the
    /// shell (Q2: a Titan under its air is expected).
    off_branch: Vec<String>,
    /// Q1's lists: an ice-shell regime under a published shell beyond 60 km, and an ocean-branch
    /// moon under a published shell of at most 60 km whose regime is not an ice shell's.
    ice_shell: Vec<String>,
    /// Every airless icy body publishing an ocean above its surface's liquidus (to main by name).
    airless: Vec<String>,
    /// Every `Temperate` record under 273.16 K that publishes an ocean, with the liquids on its
    /// surface: the same water read as a sea and as an ocean under a mean shell (for main).
    temperate: Vec<String>,
    /// Every published ocean, by name, with its line.
    published: BTreeMap<String, (Subsurface, String)>,
    /// Each moon's regime, by name.
    regimes: BTreeMap<String, TectonicRegime>,
    /// The oceans the lean's column test would have dropped, crossing ice II's field to a floor
    /// beyond it, which the ruled floor test publishes (decision-p14-t51gb-ice-ii §4.4 (4)).
    crossing: Vec<String>,
    /// Every invariant that fails, by name (asserted none).
    broken: Vec<String>,
}

impl Census {
    /// Adds the system `name` of context `ctx`, `system`, and its snapshot at [`RECORD_TIME`],
    /// `snapshot`, both under every 24 hold.
    #[expect(
        clippy::too_many_lines,
        reason = "one walk of a system's bodies, each published ocean checked and listed"
    )]
    fn visit(
        &mut self,
        name: &str,
        ctx: &SystemContext,
        (system, snapshot): (&PlanetarySystem, &SystemSnapshot),
    ) {
        let t = RECORD_TIME;
        let km = |m: f64| m / 1e3;
        with_every_24_hold(|| {
            let epoch = Epoch::new(system, ctx, t).with_shades();
            let cosines = HourCosines::new();
            for body in system.bodies() {
                let Some(record) = record_of(snapshot.bodies(), body.index) else {
                    continue;
                };
                let (Some(surface), Some(bulk)) = (record.surface().ok(), record.bulk().ok())
                else {
                    continue;
                };
                let Some(derived) = derived_of(system, ctx, &epoch, &cosines, body) else {
                    continue;
                };
                self.surfaces += 1;
                let id = format!("{name} {}", body.index.body_id(ctx.id()));
                let kind = kind_of(body);
                let figures = surface.figures();
                let conditions = surface.conditions();
                let published = *figures.subsurface_ocean();
                let regime = figures.tectonic_regime();
                let radius = Metres::from(bulk.radius());
                let mass = Kilograms::from(derived.mass);
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
                let inputs = OceanInputs {
                    radius,
                    mass,
                    water_fraction: if condensed > bulk_water {
                        condensed
                    } else {
                        bulk_water
                    },
                    surface_temperature: conditions.mean_surface_temperature(),
                    heat_flow: figures.heat_flow(),
                    solutes: derived.solutes,
                };
                let lagged = derived.lag == SurfaceLag::Lagged;
                let rule = if lagged {
                    Subsurface::NoOcean
                } else {
                    subsurface_ocean(&inputs)
                };
                if rule != published {
                    self.broken.push(format!(
                        "{id} ({kind}): published {published:?}, the rule's {rule:?}"
                    ));
                }
                let (t_s, t_eq) = (
                    conditions.mean_surface_temperature().value(),
                    derived.equilibrium.value(),
                );
                let f = figures.heat_flow().value();
                let class = format!("{:?}", bulk.class());
                let outcome = match published {
                    Subsurface::Ocean(ocean) => {
                        if let Some(fault) = implausibility(&inputs, &ocean) {
                            self.broken.push(format!("{id} ({kind}): {fault}"));
                        }
                        let (shell, depth) =
                            (ocean.shell_thickness().value(), ocean.thickness().value());
                        let line = format!(
                            "{id} ({kind}, {class}, {:?}): {:?}, {:.1} km under {:.1} km, {}, \
                             {:.2} g/kg, base {:.1} K; T_s {t_s:.1} K, T_eq {t_eq:.1} K, F {:.2} \
                             mW m⁻², x_w {:.4}",
                            surface.state(),
                            ocean.floor(),
                            km(depth),
                            km(shell),
                            ocean.liquid().substance().key().as_str(),
                            ocean.solute_mass_fraction() * 1e3,
                            ocean.base_temperature().value(),
                            f * 1e3,
                            inputs.water_fraction
                        );
                        let column = WaterColumn::of_water(radius, mass, inputs.water_fraction);
                        if ocean.floor() == OceanFloor::Rock
                            && shell + depth > r - column.base().value() - 1.0
                        {
                            self.whole.push(line.clone());
                        }
                        if surface.state() == SurfaceState::Temperate {
                            let liquids = figures.surface_liquids();
                            self.temperate
                                .push(format!("{line}; surface liquids {liquids:?}"));
                        }
                        if surface.state() == SurfaceState::Airless
                            && bulk.class() == PlanetClass::Icy
                            && Hydrosphere::of(&inputs).is_some_and(|water| {
                                water.base_under(inputs.heat_flow)
                                    == Err(FindShellBaseError::SurfaceLiquid)
                            })
                        {
                            self.airless.push(line.clone());
                        }
                        *self
                            .by_kind
                            .entry(format!("{kind}: on {:?}", ocean.floor()))
                            .or_default() += 1;
                        // The lean's column test (the warmest ice-II edge to its floor) would have
                        // dropped an ocean that crosses ice II's field to a floor beyond it.
                        let floor = Hydrosphere::of(&inputs)
                            .and_then(|water| water.base_under(inputs.heat_flow).ok())
                            .map(|base| base.floor_pressure.value());
                        let crossing = floor.is_some_and(|p| {
                            let p = if p < ICE_II_III_V.0 {
                                p
                            } else {
                                ICE_II_III_V.0
                            };
                            ice_ii_warmest(Pascals::new(p))
                                .is_some_and(|edge| ocean.base_temperature() < edge)
                        });
                        if crossing {
                            self.crossing.push(line.clone());
                        }
                        self.published.insert(id.clone(), (published, line));
                        "ocean"
                    }
                    Subsurface::NotComputed => {
                        let cause = Hydrosphere::of(&inputs).map(|water| {
                            match water.base_under(inputs.heat_flow) {
                                Ok(base) => format!("{:?}", absence(&base)),
                                Err(error) => format!("{error:?}"),
                            }
                        });
                        let cause = cause.unwrap_or_else(|| "no water".to_owned());
                        if !cause.starts_with("Some(") && cause != "TooCold" {
                            self.broken
                                .push(format!("{id} ({kind}): absent for no cause, {cause}"));
                        }
                        *self
                            .by_kind
                            .entry(format!("{kind}: absent, {cause}"))
                            .or_default() += 1;
                        "absent"
                    }
                    Subsurface::NoOcean => "none",
                };
                *self
                    .by_kind
                    .entry(format!("{kind}: {outcome}"))
                    .or_default() += 1;
                if lagged
                    && (published != Subsurface::NoOcean || regime == TectonicRegime::IceShell)
                {
                    self.broken
                        .push(format!("{id} ({kind}): lagged, {published:?}, {regime:?}"));
                }
                let Some((tides, branch)) = derived.moon else {
                    if regime == TectonicRegime::IceShell {
                        self.broken
                            .push(format!("{id} ({kind}): an ice shell's regime off a moon"));
                    }
                    continue;
                };
                self.regimes.insert(id.clone(), regime);
                let on_branch = tides.branch() == TidalBranch::Ocean;
                // Q1: the regime is an ice shell's exactly where the branch keeps a shell of at
                // most 60 km.
                let branch_shell = match branch {
                    Subsurface::Ocean(ocean) => Some(ocean.shell_thickness()),
                    Subsurface::NoOcean | Subsurface::NotComputed => None,
                };
                let thin = branch_shell.is_some_and(|shell| shell <= ICE_SHELL_LIMIT);
                if (regime == TectonicRegime::IceShell) != thin {
                    self.broken.push(format!(
                        "{id} ({kind}): {regime:?} under its branch's {branch:?}"
                    ));
                }
                if lagged && on_branch {
                    self.broken
                        .push(format!("{id} ({kind}): lagged on the ocean branch"));
                }
                let published_shell = match published {
                    Subsurface::Ocean(ocean) => Some(ocean.shell_thickness()),
                    Subsurface::NoOcean | Subsurface::NotComputed => None,
                };
                if regime == TectonicRegime::IceShell
                    && published_shell.is_none_or(|shell| shell > ICE_SHELL_LIMIT)
                {
                    self.ice_shell.push(format!(
                        "{id} ({kind}): an ice shell's regime under its published {:?}, its \
                         branch's shell {:.1} km",
                        published_shell.map(|shell| km(shell.value())),
                        branch_shell.map_or(f64::NAN, |shell| km(shell.value()))
                    ));
                }
                if on_branch
                    && regime != TectonicRegime::IceShell
                    && published_shell.is_some_and(|shell| shell <= ICE_SHELL_LIMIT)
                {
                    self.ice_shell.push(format!(
                        "{id} ({kind}): on the ocean branch under a published {:.1} km, {regime:?}",
                        published_shell.map_or(f64::NAN, |shell| km(shell.value()))
                    ));
                }
                let state = format!(
                    "T_eq {t_eq:.1} K, T_s {t_s:.1} K, p_s {:.3e} Pa, F {:.2} mW m⁻² (branch {:.2})",
                    conditions.surface_pressure().value(),
                    f * 1e3,
                    tides.heat_flow().value() * 1e3
                );
                if on_branch && t_s < t_eq {
                    self.branch
                        .push(format!("{id} ({kind}): its surface under T_eq, {state}"));
                }
                // Q2: an ocean-branch moon publishes an ocean, absence or none by its surface's
                // liquidus (or, the builder's case, none for an ocean under a metre), each listed.
                if on_branch && !matches!(published, Subsurface::Ocean(_)) {
                    let base = Hydrosphere::of(&inputs).map(|w| w.base_under(inputs.heat_flow));
                    let why = match (published, base) {
                        (Subsurface::NotComputed, _) => "absent",
                        (_, Some(Err(FindShellBaseError::SurfaceLiquid)) | None) => {
                            "by the surface's liquidus"
                        }
                        (_, Some(Ok(_))) => "under a metre",
                        (_, Some(Err(_))) => {
                            self.broken.push(format!(
                                "{id} ({kind}): on the ocean branch, its record keeps no base"
                            ));
                            "no base"
                        }
                    };
                    self.branch.push(format!(
                        "{id} ({kind}): on the ocean branch, publishing {published:?} ({why}), \
                         {state}"
                    ));
                }
                if !on_branch && let Subsurface::Ocean(ocean) = published {
                    self.off_branch.push(format!(
                        "{id} ({kind}, {:?}, {:?}): an ocean under {:.1} km, {state}",
                        tides.branch(),
                        surface.state(),
                        km(ocean.shell_thickness().value())
                    ));
                }
            }
        });
    }

    /// Prints the census for the bless note.
    fn print(&self, what: &str) {
        eprintln!("{what}: {} records with a solid surface", self.surfaces);
        eprintln!("by kind: {:#?}", self.by_kind);
        for (title, lines) in [
            (
                "count (xiii), whole hydrospheres liquid on rock (every kind)",
                &self.whole,
            ),
            ("temperate records publishing an ocean", &self.temperate),
            (
                "oceans crossing ice II's field, which the lean would drop",
                &self.crossing,
            ),
            (
                "ocean-branch moons publishing absence or none, or colder than T_eq",
                &self.branch,
            ),
            ("off-branch moons publishing an ocean", &self.off_branch),
            ("ice-shell mismatches (Q1's lists)", &self.ice_shell),
            (
                "airless icy bodies with an ocean above their liquidus",
                &self.airless,
            ),
        ] {
            eprintln!("{title}: {}\n  {}", lines.len(), lines.join("\n  "));
        }
    }

    /// The census's assertions: every published ocean the rule's at its record's inputs and
    /// plausible, none on a lagged body, a moon's ice-shell regime its branch's alone, and no
    /// airless icy body with an ocean above its liquidus.
    fn assert_holds(&self) {
        assert_eq!(self.broken, Vec::<String>::new());
        assert_eq!(self.airless, Vec::<String>::new());
    }
}

/// The census of the golden systems under the ocean's rules `rules` (every other 24 hold in force).
fn golden_census(rules: OceanRules) -> Census {
    let systems = with_every_24_hold(|| with_ocean_rules(rules, golden_systems_afresh));
    let mut census = Census::default();
    for (name, ctx, system) in &systems {
        if let Some(snapshot) = snapshot_under(system, ctx, rules) {
            census.visit(name, ctx, (system, &snapshot));
        }
    }
    census
}

/// P14.T51.g.b over the golden systems, every 24 hold in force: every body with a solid surface
/// publishes the solve's ocean at its record's inputs, each plausible, none lagged, and a moon's
/// ice-shell regime is its branch's alone. The census's lists are printed, with the published
/// oceans against P14.T51.g.a's floor (the hold) and version 23's, and the regimes that move.
#[test]
fn every_solid_body_publishes_its_ocean_under_the_hold() {
    let solved = golden_census(OceanRules::Solved);
    solved.print("the golden systems");
    solved.assert_holds();
    assert!(solved.surfaces > 50, "{}", solved.surfaces);
    let floored = with_every_24_hold(|| {
        with_ocean_rules(OceanRules::Held, || {
            with_ocean_floor(OceanFloorRules::Floored, golden_systems_afresh)
        })
    });
    let mut before = BTreeMap::new();
    let mut regimes = BTreeMap::new();
    for (name, ctx, system) in &floored {
        let Some(snapshot) = snapshot_under(system, ctx, OceanRules::Held) else {
            continue;
        };
        for record in snapshot.bodies() {
            let id = format!("{name} {}", record.index().body_id(ctx.id()));
            if let Some(surface) = record.surface().ok() {
                before.insert(id.clone(), *surface.figures().subsurface_ocean());
                regimes.insert(id, surface.figures().tectonic_regime());
            }
        }
    }
    let mut at_23 = BTreeMap::new();
    for (name, ctx, system) in golden_systems() {
        if ctx.age_at(RECORD_TIME).value() <= 0.0 {
            continue;
        }
        for record in system.snapshot_at(ctx, RECORD_TIME).bodies() {
            let id = format!("{name} {}", record.index().body_id(ctx.id()));
            if let Some(surface) = record.surface().ok() {
                at_23.insert(id, *surface.figures().subsurface_ocean());
            }
        }
    }
    let depth = |ocean: Option<&Subsurface>| match ocean {
        Some(Subsurface::Ocean(ocean)) => format!(
            "{:.1} km under {:.1} km on {:?}",
            ocean.thickness().value() / 1e3,
            ocean.shell_thickness().value() / 1e3,
            ocean.floor()
        ),
        other => format!("{other:?}"),
    };
    let named: BTreeSet<&String> = solved
        .published
        .keys()
        .chain(
            before
                .iter()
                .filter_map(|(id, ocean)| matches!(ocean, Subsurface::Ocean(_)).then_some(id)),
        )
        .collect();
    for id in named {
        eprintln!(
            "{id}: 23 {}; the floor {}; solved {}",
            depth(at_23.get(id)),
            depth(before.get(id)),
            depth(solved.published.get(id).map(|(ocean, _)| ocean))
        );
    }
    for (id, regime) in &solved.regimes {
        if regimes.get(id).is_some_and(|before| before != regime) {
            eprintln!("{id}: its regime {:?} → {regime:?}", regimes[id]);
        }
    }
}

/// Each moon's regime under the ocean held (P14.T51.g.a's floor, every other 24 hold in force),
/// by name, of the system `name` of context `ctx` generated under them, `system`, to list the
/// moons whose regime the branch's spherical shell moves (Q1's movers).
fn held_regimes(
    name: &str,
    ctx: &SystemContext,
    seed: Seed,
    regimes: &mut BTreeMap<String, (&'static str, TectonicRegime)>,
) {
    let system = with_every_24_hold(|| with_ocean_rules(OceanRules::Held, || generate(seed, ctx)));
    let Some(snapshot) = snapshot_under(&system, ctx, OceanRules::Held) else {
        return;
    };
    for body in system.bodies() {
        if !matches!(body.part, Part::Moon(_)) {
            continue;
        }
        let Some(surface) = record_of(snapshot.bodies(), body.index).and_then(|r| r.surface().ok())
        else {
            continue;
        };
        let id = format!("{name} {}", body.index.body_id(ctx.id()));
        regimes.insert(id, (kind_of(body), surface.figures().tectonic_regime()));
    }
}

/// Prints the moons whose regime moves from the ocean held, `held`, to the census's, by origin.
fn print_movers(census: &Census, held: &BTreeMap<String, (&'static str, TectonicRegime)>) {
    let mut by_origin = BTreeMap::<String, u32>::new();
    let mut lines = Vec::new();
    for (id, regime) in &census.regimes {
        if let Some(&(kind, before)) = held.get(id)
            && before != *regime
        {
            *by_origin
                .entry(format!("{kind}: {before:?} → {regime:?}"))
                .or_default() += 1;
            lines.push(format!("{id} ({kind}): {before:?} → {regime:?}"));
        }
    }
    eprintln!(
        "regime movers against the ocean held: {by_origin:?}\n  {}",
        lines.join("\n  ")
    );
}

/// P14.T51.g.b's census over the 400 whole systems, as the golden systems', every 24 hold in force.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn every_solid_body_publishes_its_ocean_over_the_sample() {
    let mut census = Census::default();
    let mut held = BTreeMap::new();
    for ctx in sample() {
        let system = with_every_24_hold(|| generate(SEED, ctx));
        if let Some(snapshot) = snapshot_under(&system, ctx, OceanRules::Solved) {
            census.visit("sample", ctx, (&system, &snapshot));
        }
        held_regimes("sample", ctx, SEED, &mut held);
    }
    census.print("the sample");
    print_movers(&census, &held);
    assert!(census.surfaces > 1_000, "{}", census.surfaces);
    census.assert_holds();
}

/// The seed of look-72's universe, LOOK72 (`tests/observe_brief.rs`'s; as
/// `moon_ice/lag_tests.rs` holds it).
const LOOK72: u64 = 0xa49f_649b_ef43_4b20;

/// LOOK72's three regular moons whose oceans look-92-93 read on the wire at 23 (decision-p14-t51g
/// Q9), as `moon_ice/lag_tests.rs` holds them.
const LOOK72_OCEANS: [&str; 3] = [
    "020e56ef60080002.0c01",
    "420e5bbe00800024.0306",
    "420e5bbe00800032.0701",
];

/// The systems of the 62 bodies of LOOK72's triage (decision-absurd-states-24 R4.3), as
/// `moon_ice/lag_tests.rs` holds them.
const LOOK72_TRIAGED_SYSTEMS: [&str; 21] = [
    "020e56ef20080000",
    "020e56ef20080001",
    "020e56ef20080004",
    "020e56ef20080005",
    "020e56ef20080006",
    "020e56ef20090004",
    "020e56ef60070001",
    "020e56ef60070005",
    "020e56ef60080000",
    "020e56ef60080003",
    "020e56ef60080004",
    "020e56ef60080007",
    "020e56ef60090005",
    "020e56ef60090006",
    "020e56efa0080000",
    "020e56efa0080003",
    "020e66ef60070004",
    "020e66efa0070007",
    "020e66efa007000b",
    "020e66efa007000c",
    "020e66efa0080003",
];

/// P14.T51.g.b's census over LOOK72, every 24 hold in force: the systems of its three named
/// oceans and of the triage's 62 bodies, as the golden systems', with the three by name: the
/// prototype's 020e56ef60080002.0c01 and 420e5bbe00800024.0306 on ice V, and
/// 420e5bbe00800032.0701 keeping its whole hydrosphere on rock (count (xiii); decision-p14-t51g
/// Q9).
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn look72_s_bodies_publish_their_oceans() {
    let galaxy = crate::galaxy::Galaxy::new(Seed::new(LOOK72)).with_full_potential();
    let mut systems: BTreeSet<&str> = LOOK72_TRIAGED_SYSTEMS.into_iter().collect();
    systems.extend(LOOK72_OCEANS.map(|id| id.split_once('.').expect("a body ID").0));
    let mut census = Census::default();
    let mut held = BTreeMap::new();
    for id in systems {
        let ctx = SystemContext::for_system(&galaxy, id.parse().expect("a system ID"))
            .expect("a system of LOOK72");
        let system = with_every_24_hold(|| generate(galaxy.seed(), &ctx));
        if let Some(snapshot) = snapshot_under(&system, &ctx, OceanRules::Solved) {
            census.visit("look72", &ctx, (&system, &snapshot));
        }
        held_regimes("look72", &ctx, galaxy.seed(), &mut held);
    }
    census.print("LOOK72");
    print_movers(&census, &held);
    for id in LOOK72_OCEANS {
        eprintln!(
            "{id}: {}",
            census
                .published
                .get(&format!("look72 {id}"))
                .map_or("no ocean published", |(_, line)| line.as_str())
        );
    }
    census.assert_holds();
}

/// Rule (f)'s reuse under the ocean's solve (the determinism review): over the golden systems,
/// generated and read under every 24 hold, each record's figures, which reuse the first stage its
/// passes' solve computed last, are those computed again (`with_first_stage_recomputed`), bit for
/// bit, at [`RECORD_TIME`]: the ocean a moon's passes' figures read (its branch's) and its
/// record's are one function of one pass, every moon origin's.
#[test]
fn under_the_ocean_the_record_s_figures_reuse_its_last_pass_s_k_to_the_bit() {
    let systems = with_every_24_hold(golden_systems_afresh);
    let mut moons = 0_u32;
    for (name, ctx, system) in &systems {
        if ctx.age_at(RECORD_TIME).value() <= 0.0 {
            continue;
        }
        let (reused, recomputed) = with_every_24_hold(|| {
            (
                system.snapshot_at(ctx, RECORD_TIME),
                with_first_stage_recomputed(|| system.snapshot_at(ctx, RECORD_TIME)),
            )
        });
        assert_eq!(format!("{reused:?}"), format!("{recomputed:?}"), "{name}");
        moons += system
            .bodies()
            .iter()
            .filter(|body| matches!(body.part, Part::Moon(_)))
            .filter(|body| {
                record_of(reused.bodies(), body.index)
                    .is_some_and(|record| matches!(record.surface(), Section::Ok(_)))
            })
            .count()
            .try_into()
            .unwrap_or(u32::MAX);
    }
    assert!(moons > 50, "{moons}");
}

/// Under the ocean's solve a system's hooks are the same twice and order independent (the
/// determinism review; `assert_order_independent`): the golden system with the most moons on the
/// ocean branch, generated under every 24 hold, read at [`RECORD_TIME`] body by body in any order.
#[test]
fn under_the_ocean_a_system_s_hooks_are_order_independent() {
    use hyperion_testkit::order::assert_order_independent;
    with_every_24_hold(|| {
        let systems = golden_systems_afresh();
        let (name, ctx, system) = systems
            .iter()
            .filter(|(_, ctx, _)| ctx.age_at(RECORD_TIME).value() > 0.0)
            .max_by_key(|(_, _, system)| {
                system
                    .bodies()
                    .iter()
                    .filter(|body| matches!(body.part, Part::Moon(_)))
                    .count()
            })
            .expect("a golden system with moons");
        let indices: Vec<BodyIndex> = system.bodies().iter().map(Body::index).collect();
        let first = system.snapshot_at(ctx, RECORD_TIME);
        assert_eq!(
            format!("{first:?}"),
            format!("{:?}", system.snapshot_at(ctx, RECORD_TIME)),
            "{name}: the same twice"
        );
        assert_order_independent(&indices, |&index| {
            system.hooks_at(ctx, index, RECORD_TIME).unwrap()
        });
    });
}
