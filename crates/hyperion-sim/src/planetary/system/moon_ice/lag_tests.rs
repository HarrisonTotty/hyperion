//! P14.T56.g's tests: a hot ice bulk's surface is its heated shell's rock lag
//! (decision-p14-t56g §4, §5, §7 and §8 (d)), under the moon bulk's batch rules
//! ([`MoonBulkRules::Batch`]) against P14.T56.f's arms alone ([`MoonBulkRules::BulkOnly`]).
//!
//! And P14.T51.g.a's census (decision-p14-t51g Q9, Q4): with every 24 hold in force, a regular
//! moon's published ocean ends at the shallower of its rock and ice V's melting pressure at its
//! 273 K base, or is not modelled beyond it, and no lagged moon takes the ocean branch
//! ([`OceanCensus`]), over the golden systems, the 400 and LOOK72 by name.

use std::collections::BTreeMap;

use hyperion_testkit::float::bits;

use super::{HOT_ICE_LIMIT, MoonBulkRules, shell_lag, with_moon_bulk};
use crate::Seed;
use crate::planetary::derive::SurfaceLag;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::derive::photometry::PhaseTemplate;
use crate::planetary::hooks::crust::{CrustLithology, LidCrust};
use crate::planetary::hooks::figures::interior::ICE_SHELL_LIMIT;
use crate::planetary::hooks::figures::{OceanFloor, Subsurface, TectonicRegime};
use crate::planetary::hooks::ocean::{OceanFloorRules, base_liquidus, with_ocean_floor};
use crate::planetary::moons::regular::{
    ICE_CONDUCTIVITY_COEFFICIENT, ICE_MELTING_POINT, WATER_DENSITY, with_ocean_under_a_lag,
};
use crate::planetary::moons::tides::TidalBranch;
use crate::planetary::record::BodyRecord;
use crate::planetary::surface::RECORD_TIME;
use crate::planetary::system::tests::{SEED, golden_systems, golden_systems_afresh, sample};
use crate::planetary::system::*;
use crate::planetary::testing::with_every_24_hold;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{EarthFluxes, Kelvin, Metres, Years};

/// The hot ice bulks P14.T56.f thins and leaves `Icy` in the golden systems, and `red_giant`'s
/// hot icy belt members (P14.T56.g's must-fix; brief-notes-24).
const GOLDEN_MUST_FIX: [&str; 5] = [
    "subgiant 42006c9a00000004.0101",
    "red_giant 4200ecd200000003.0503",
    "red_giant 4200ecd200000003.0504",
    "red_giant 4200ecd200000003.e202",
    "red_giant 4200ecd200000003.e204",
];

/// `red_giant`'s third hot icy member, at T0 289.0 K, which the lag's own census found (P14.T56.g;
/// main, 2026-10-23).
const RED_GIANT_E206: &str = "red_giant 4200ecd200000003.e206";

/// The hot ice bulks P14.T56.f thins and leaves `Icy` in the 400 (its census, 2026-10-22).
const SAMPLE_MUST_FIX: [&str; 6] = [
    "sample 21fff6597ff00000.0401",
    "sample 42002cb200000000.0501",
    "sample 42002cae00000005.0301",
    "sample 42002cae00000005.0401",
    "sample 42002cb1ff800003.0501",
    "sample 42002cb1ff800003.0601",
];

/// The zero-albedo equilibrium temperature `T0` at the flux `flux`, the members' rule's.
fn zero_albedo(flux: EarthFluxes) -> Kelvin {
    equilibrium_temperature(flux, BondAlbedo::from_fraction(0.0))
}

/// The record of `index` in `records`.
fn record_of(records: &[BodyRecord], index: BodyIndex) -> Option<&BodyRecord> {
    records.iter().find(|record| record.index() == index)
}

/// What the census tells of a record's surface: its state, material, template, `T_s`, `T_eq`,
/// `p_s`, lid and cover.
fn surface_line(record: &BodyRecord) -> String {
    let Some(surface) = record.surface().ok() else {
        return format!("no surface ({:?})", record.surface());
    };
    let bulk = record
        .bulk()
        .ok()
        .expect("a record with a surface has a bulk");
    let (conditions, figures) = (surface.conditions(), surface.figures());
    format!(
        "class {:?}, water {:.4}, state {:?}, material {:?}, template {:?}, T_s {:.2} K, T_eq \
         {:.2} K, p_s {:.4e} Pa, lid {:?}, cover: ice {:.4}, ocean {:.4}, ices {:?}, liquids {:?}",
        bulk.class(),
        bulk.fractions().water(),
        surface.state(),
        surface.material(),
        record.photometry().ok().map(BodyPhotometry::template),
        conditions.mean_surface_temperature().value(),
        bulk.equilibrium_temperature().value(),
        conditions.surface_pressure().value(),
        surface.crust().ok().map(|crust| (
            crust.lid(),
            crust.primary(),
            crust.secondary(),
            crust.bulk()
        )),
        figures.ice_fraction(),
        figures.ocean_fraction(),
        figures.surface_ices(),
        figures.surface_liquids(),
    )
}

/// Whether `record`'s published lid is `H2O`, a frozen ocean's shell, which a lagged body cannot
/// hold (P14.T56.g's `ocean_froze` gate).
fn water_lid(record: &BodyRecord) -> bool {
    record
        .surface()
        .ok()
        .and_then(|surface| surface.crust().ok().copied())
        .is_some_and(|crust| {
            let named = match crust.lid() {
                LidCrust::Primary => crust.primary(),
                LidCrust::Secondary => crust.secondary(),
                LidCrust::Bulk => crust.bulk(),
            };
            named == Some(CrustLithology::Water.substance())
        })
}

/// A bulk section's figures but its temperatures: its radius, density, gravity, class and
/// fractions, which the lag moves none of.
fn bulk_figures(record: &BodyRecord) -> Option<String> {
    let bulk = record.bulk().ok()?;
    Some(format!(
        "{:?} {:?} {:?} {:?} {:?}",
        bulk.radius(),
        bulk.density(),
        bulk.surface_gravity(),
        bulk.class(),
        bulk.fractions()
    ))
}

/// What P14.T56.g moves over a set of systems ([`LagCensus::visit`]).
#[derive(Debug, Default)]
struct LagCensus {
    /// Each lagged record: its kind, its surface under `BulkOnly` and under the batch.
    lagged: Vec<String>,
    /// Airless records with an `Ice` surface above the melting point at T0 (asserted none).
    airless_hot_ice: Vec<String>,
    /// Lagged records whose surface is not rock, or airless and not `Mercury`'s (asserted none).
    lagged_not_rock: Vec<String>,
    /// Lagged records whose lid is `H2O` (asserted none).
    lagged_water_lid: Vec<String>,
    /// Count (ix): every other record with an `Ice` surface above the melting point at T0, with its
    /// kind, class, state, T0, `T_eq`, `T_s` and `p_s` (listed, not asserted).
    other_hot_ice: Vec<String>,
    /// The lids the lag moves, `BulkOnly`'s to the batch's, by name.
    lids_moved: Vec<String>,
    /// `T_eq` moves of the lagged records, by name.
    t_eq_moved: Vec<String>,
    /// Lagged records whose bulk's figures moved (asserted none).
    bulk_moved: Vec<String>,
    /// Records that moved and are not lagged (asserted none).
    unlagged_moved: Vec<String>,
    /// Moons whose verdict is not "thinned ⇔ w′ < w₀", or whose derivation's lag is not the
    /// arm's own (asserted none); and moons thinned that keep all their water (listed).
    verdict_broken: Vec<String>,
    thinned_whole: Vec<String>,
    /// Icy members whose lag is not their T0's, and planets with a lag (asserted none).
    primary_broken: Vec<String>,
    /// How many records were read, and how many moons with an arm.
    records: u32,
    moons_with_arm: u32,
}

impl LagCensus {
    /// Adds the system `name` of context `ctx` generated held, under P14.T56.f's arms alone and
    /// under the batch.
    #[expect(
        clippy::too_many_lines,
        reason = "one walk of a system's records under the three rules, each checked and listed"
    )]
    fn visit(
        &mut self,
        name: &str,
        ctx: &SystemContext,
        (held, bulk_only, batch): (&PlanetarySystem, &PlanetarySystem, &PlanetarySystem),
    ) {
        let t = RECORD_TIME;
        if ctx.age_at(t).value() <= 0.0 {
            return;
        }
        let held_snapshot = held.snapshot_at(ctx, t);
        let before = with_moon_bulk(MoonBulkRules::BulkOnly, || bulk_only.snapshot_at(ctx, t));
        with_moon_bulk(MoonBulkRules::Batch, || {
            let after = batch.snapshot_at(ctx, t);
            let epoch = Epoch::new(batch, ctx, t).with_shades();
            let cosines = HourCosines::new();
            for body in batch.bodies() {
                let id = format!("{name} {}", body.index.body_id(ctx.id()));
                let (Some(old), Some(new)) = (
                    record_of(before.bodies(), body.index),
                    record_of(after.bodies(), body.index),
                ) else {
                    continue;
                };
                let Some(bulk) = new.bulk().ok() else {
                    if format!("{old:?}") != format!("{new:?}") {
                        self.unlagged_moved.push(id);
                    }
                    continue;
                };
                self.records += 1;
                // The body's verdict, its derivation's, and its T0.
                let (kind, lag, t0) = match (&body.part, body.host) {
                    (Part::Moon(part), OrbitHost::Body(parent)) => {
                        let primary = batch.body(parent).expect("a moon's parent is a body");
                        let host = fate_host(ctx, batch.zone_of(primary));
                        let fate = batch.fate_of(primary, &host);
                        let now = batch.parent_now(&epoch, &cosines, parent, &fate);
                        let derived = batch
                            .record_moon(&epoch, body, (&now, &fate))
                            .expect("a moon with a bulk section is derived");
                        assert_eq!(
                            derived.equilibrium_temperature(),
                            bulk.equilibrium_temperature(),
                            "{id}: the census reads the record's derivation"
                        );
                        let lag = derived.surface_lag();
                        let ice = now.moon_ice.as_ref().and_then(|m| m.ice);
                        let t0 = ice
                            .map_or_else(|| zero_albedo(derived.flux()), |ice| ice.t0(&part.orbit));
                        // The derivation's lag is the arm's own verdict, carried.
                        let orbit = part.satellite.orbit_at(&part.parent, ctx.age_at(t));
                        let own = PlanetarySystem::moon_own_bulk(part, &orbit, &now);
                        let stated = match own {
                            MoonBulk::Derived => SurfaceLag::Bare,
                            MoonBulk::Kept { .. } => SurfaceLag::Lagged,
                            MoonBulk::Stated { lag, .. } => lag,
                        };
                        if stated != lag {
                            self.verdict_broken
                                .push(format!("{id}: own bulk {own:?}, derivation {lag:?}"));
                        }
                        // Thinned is the arm's t_hot > 0 with w₀ > 0, which is w′ < w₀.
                        if let Some(ice) = ice {
                            self.moons_with_arm += 1;
                            let inside = match part.satellite.moon() {
                                SatelliteMoon::Captured(captured) => {
                                    PlanetarySystem::capture_formed_inside(captured.kind(), &now)
                                }
                                SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => false,
                            };
                            let w0 = if inside {
                                0.0
                            } else {
                                record_of(held_snapshot.bodies(), body.index)
                                    .and_then(|r| r.bulk().ok())
                                    .map_or(0.0, |b| b.fractions().water())
                            };
                            let w1 = bulk.fractions().water();
                            let heated = ice.history.hot_for(t0).value() > 0.0;
                            let thinned = heated && w0 > 0.0;
                            let lagged = lag == SurfaceLag::Lagged;
                            if thinned != lagged {
                                self.verdict_broken.push(format!(
                                    "{id}: thinned {thinned} (w₀ {w0}, heated {heated}), {lag:?}"
                                ));
                            }
                            if thinned && w1 >= w0 {
                                self.thinned_whole.push(format!("{id}: w₀ {w0} = w′ {w1}"));
                            } else if thinned != (w1 < w0) {
                                self.verdict_broken
                                    .push(format!("{id}: thinned {thinned}, w₀ {w0}, w′ {w1}"));
                            }
                        }
                        (format!("{:?}", body.kind()), lag, t0)
                    }
                    (Part::Planet(_) | Part::Member(_), _) => {
                        let host = fate_host(ctx, batch.zone_of(body));
                        let fate = batch.fate_of(body, &host);
                        let now = batch.parent_now(&epoch, &cosines, body.index, &fate);
                        let derived = now.derived.expect("a body with a bulk section is derived");
                        assert_eq!(
                            derived.equilibrium_temperature(),
                            bulk.equilibrium_temperature(),
                            "{id}: the census reads the record's derivation"
                        );
                        let lag = derived.surface_lag();
                        let t0 = zero_albedo(derived.flux());
                        // An icy belt member, by the code's own rule, is lagged above the melting
                        // point at its T0; a planet never.
                        let expected = if icy_member_ice(body).is_some() && t0 > HOT_ICE_LIMIT {
                            SurfaceLag::Lagged
                        } else {
                            SurfaceLag::Bare
                        };
                        if lag != expected {
                            self.primary_broken.push(format!(
                                "{id} ({:?}, {:?}): T0 {:.1} K, {lag:?}",
                                body.kind(),
                                bulk.class(),
                                t0.value()
                            ));
                        }
                        (format!("{:?}", body.kind()), lag, t0)
                    }
                    (Part::Ring(_), _)
                    | (
                        Part::Moon(_),
                        OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre,
                    ) => continue,
                };
                let moved = format!("{old:?}") != format!("{new:?}");
                if lag == SurfaceLag::Bare {
                    if moved {
                        self.unlagged_moved.push(id.clone());
                    }
                } else {
                    self.lagged.push(format!(
                        "{id} ({kind}, T0 {:.1} K):\n      BulkOnly {}\n      Batch    {}",
                        t0.value(),
                        surface_line(old),
                        surface_line(new)
                    ));
                    if bulk_figures(old) != bulk_figures(new) {
                        self.bulk_moved.push(id.clone());
                    }
                    if water_lid(new) {
                        self.lagged_water_lid.push(id.clone());
                    }
                    let (t_old, t_new) = (
                        old.bulk().ok().map(|b| b.equilibrium_temperature().value()),
                        Some(bulk.equilibrium_temperature().value()),
                    );
                    if t_old.map(bits) != t_new.map(bits) {
                        self.t_eq_moved
                            .push(format!("{id}: T_eq {t_old:?} → {t_new:?} K"));
                    }
                    let lid = |record: &BodyRecord| {
                        record
                            .surface()
                            .ok()
                            .and_then(|s| s.crust().ok().copied())
                            .map(|c| {
                                format!(
                                    "{:?} {:?}/{:?}/{:?}",
                                    c.lid(),
                                    c.primary(),
                                    c.secondary(),
                                    c.bulk()
                                )
                            })
                    };
                    if lid(old) != lid(new) {
                        self.lids_moved
                            .push(format!("{id}: {:?} → {:?}", lid(old), lid(new)));
                    }
                }
                let Some(surface) = new.surface().ok() else {
                    continue;
                };
                if lag == SurfaceLag::Lagged {
                    let template = new.photometry().ok().map(BodyPhotometry::template);
                    if surface.material() != SurfaceMaterial::Rock
                        || (surface.state() == SurfaceState::Airless
                            && template != Some(PhaseTemplate::Mercury))
                    {
                        self.lagged_not_rock
                            .push(format!("{id}: {}", surface_line(new)));
                    }
                } else if surface.material() == SurfaceMaterial::Ice && t0 > HOT_ICE_LIMIT {
                    let line = format!(
                        "{id} ({kind}, {:?}): {:?}, T0 {:.1} K, T_eq {:.1} K, T_s {:.1} K, p_s \
                         {:.4e} Pa",
                        bulk.class(),
                        surface.state(),
                        t0.value(),
                        bulk.equilibrium_temperature().value(),
                        surface.conditions().mean_surface_temperature().value(),
                        surface.conditions().surface_pressure().value()
                    );
                    if surface.state() == SurfaceState::Airless {
                        self.airless_hot_ice.push(line);
                    } else {
                        self.other_hot_ice.push(line);
                    }
                }
            }
        });
    }

    /// Prints the census for the bless note.
    fn print(&self, what: &str) {
        eprintln!(
            "{what}: {} records, {} moons with an arm; {} lagged:\n  {}",
            self.records,
            self.moons_with_arm,
            self.lagged.len(),
            self.lagged.join("\n  ")
        );
        eprintln!(
            "count (ix), {} other Ice surfaces above the melting point at T0:\n  {}",
            self.other_hot_ice.len(),
            self.other_hot_ice.join("\n  ")
        );
        eprintln!(
            "lids the lag moves ({}):\n  {}",
            self.lids_moved.len(),
            self.lids_moved.join("\n  ")
        );
        eprintln!(
            "T_eq moves ({}):\n  {}",
            self.t_eq_moved.len(),
            self.t_eq_moved.join("\n  ")
        );
        eprintln!("thinned bulks of all water: {:?}", self.thinned_whole);
    }

    /// The census's assertions (decision-p14-t56g §4, §7.2): every lagged record is rock
    /// (`Mercury`'s template where airless); no airless record has an `Ice` surface above the
    /// melting point at T0, a planet's above all, which goes to main before the commit; the
    /// verdict is the arm's, thinned ⇔ w′ < w₀; against `BulkOnly` no record moves but lagged ones,
    /// whose bulks' figures stay to the bit.
    fn assert_holds(&self) {
        assert_eq!(self.lagged_not_rock, Vec::<String>::new());
        assert_eq!(self.lagged_water_lid, Vec::<String>::new());
        assert_eq!(
            self.airless_hot_ice,
            Vec::<String>::new(),
            "an airless Ice surface above the melting point at T0 (an Icy planet among them goes \
             to main before the commit, decision-p14-t56g §4)"
        );
        assert_eq!(self.verdict_broken, Vec::<String>::new());
        assert_eq!(self.primary_broken, Vec::<String>::new());
        assert_eq!(self.unlagged_moved, Vec::<String>::new());
        assert_eq!(self.bulk_moved, Vec::<String>::new());
    }

    /// Asserts that each of `names` is lagged, rock and, airless, `Mercury`'s, under a lid that is
    /// not `H2O`.
    fn assert_lagged(&self, names: &[&str]) {
        for name in names {
            let line = self
                .lagged
                .iter()
                .find(|line| line.starts_with(&format!("{name} ")))
                .unwrap_or_else(|| panic!("{name} is lagged: {:#?}", self.lagged));
            let batch = line
                .lines()
                .last()
                .expect("a lagged line has its batch's surface");
            assert!(
                batch.contains("material Rock")
                    && (!batch.contains("state Airless") || batch.contains("Some(Mercury)")),
                "{name}: {line}"
            );
            assert!(
                !self.lagged_water_lid.iter().any(|id| id == name),
                "{name}'s lid is H2O: {line}"
            );
        }
    }

    /// Asserts that the lag moved the lids, and only the lids' `T_eq`, of `names` alone.
    fn assert_lids_moved(&self, names: &[&str]) {
        let ids = |list: &[String]| -> Vec<String> {
            list.iter()
                .map(|line| line.split(':').next().unwrap_or_default().to_owned())
                .collect()
        };
        assert_eq!(ids(&self.lids_moved), names, "{:#?}", self.lids_moved);
        assert_eq!(ids(&self.t_eq_moved), names, "{:#?}", self.t_eq_moved);
    }
}

/// P14.T56.g over the golden systems (decision-p14-t56g §4, §8 (d)): every lagged record is rock,
/// `Mercury`'s where airless; no airless record shows ice above the melting point at T0; the
/// verdict is the retention arm's, a member's its T0's; `subgiant` 0101, `red_giant` 0503 and 0504
/// and the members e202 and e204 are lagged rock, their bulks' figures unchanged; against
/// P14.T56.f's arms alone no record moves but lagged ones. Count (ix) is listed.
///
/// `red_giant` e206 is lagged too, by name (main, 2026-10-23): an icy member at T0 289.0 K that
/// lay airless on the cold branch under an `H2O` lid at `T_eq` 220.2 K. The `ocean_froze` gate
/// moves its lid to its secondary crust, its `T_eq` to 280.7 K, and it is the one lid and the one
/// `T_eq` the lag moves here.
#[test]
fn a_hot_ice_bulk_s_surface_is_its_heated_shell_s_rock_lag() {
    let bulk_only = with_moon_bulk(MoonBulkRules::BulkOnly, golden_systems_afresh);
    let batch = with_moon_bulk(MoonBulkRules::Batch, golden_systems_afresh);
    let mut census = LagCensus::default();
    for (((name, ctx, held), (_, _, bulk_only)), (_, _, batch)) in
        golden_systems().iter().zip(&bulk_only).zip(&batch)
    {
        census.visit(name, ctx, (held, bulk_only, batch));
    }
    census.print("the golden systems");
    census.assert_holds();
    census.assert_lagged(&GOLDEN_MUST_FIX);
    census.assert_lagged(&[RED_GIANT_E206]);
    census.assert_lids_moved(&[RED_GIANT_E206]);
}

/// P14.T56.g's census over the 400 whole systems, by name: as the golden systems' test, with the
/// six hot ice bulks P14.T56.f leaves `Icy` there lagged rock, and 42002cb1ff800003.0501's surface
/// under both arms (decision-p14-t56g §5).
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn a_hot_ice_bulk_s_surface_is_its_rock_lag_over_the_sample() {
    let mut census = LagCensus::default();
    for ctx in sample() {
        let held = generate(SEED, ctx);
        let bulk_only = with_moon_bulk(MoonBulkRules::BulkOnly, || generate(SEED, ctx));
        let batch = with_moon_bulk(MoonBulkRules::Batch, || generate(SEED, ctx));
        census.visit("sample", ctx, (&held, &bulk_only, &batch));
    }
    census.print("the sample");
    if let Some(line) = census
        .lagged
        .iter()
        .find(|line| line.starts_with("sample 42002cb1ff800003.0501 "))
    {
        eprintln!("42002cb1ff800003.0501:\n  {line}");
    }
    assert!(census.records > 5_000, "{}", census.records);
    census.assert_holds();
    census.assert_lagged(&SAMPLE_MUST_FIX);
    census.assert_lids_moved(&["sample 42002cb1ff800003.0501"]);
}

/// The arm's verdict on a surface: the rock lag wherever it thins a bulk, under the batch's rules
/// alone; bare with no water, no time hot, or under P14.T56.f's arms alone.
#[test]
fn the_retention_arm_s_verdict_lags_only_what_it_thins() {
    let (w, r, t) = (0.3749, Metres::new(1.184e6), Years::new(7.25e9));
    let batch = |run: fn() -> SurfaceLag| with_moon_bulk(MoonBulkRules::Batch, run);
    assert_eq!(
        batch(|| shell_lag(0.3749, Metres::new(1.184e6), Years::new(7.25e9))),
        SurfaceLag::Lagged
    );
    // A bulk the arm empties is lagged too: its material is rock either way.
    assert_eq!(
        batch(|| shell_lag(0.375, Metres::new(1.2e5), Years::new(2.32e9))),
        SurfaceLag::Lagged
    );
    assert_eq!(
        batch(|| shell_lag(0.375, Metres::new(1.2e5), Years::new(0.0))),
        SurfaceLag::Bare
    );
    assert_eq!(
        batch(|| shell_lag(0.0, Metres::new(1.2e5), Years::new(9e9))),
        SurfaceLag::Bare
    );
    assert_eq!(
        with_moon_bulk(MoonBulkRules::BulkOnly, || shell_lag(w, r, t)),
        SurfaceLag::Bare
    );
    assert_eq!(shell_lag(w, r, t), SurfaceLag::Bare, "held at 23");
}

/// 42002cb1ff800003.0501's surface under the rules `rules` (decision-p14-t56g §5, §10.4).
fn print_0501(rules: MoonBulkRules) {
    let ctx = sample()
        .iter()
        .find(|ctx| ctx.id().raw() == 0x4200_2cb1_ff80_0003)
        .expect("the sample holds 42002cb1ff800003");
    let line = with_moon_bulk(rules, || {
        let system = generate(SEED, ctx);
        let snapshot = system.snapshot_at(ctx, RECORD_TIME);
        let body = system
            .bodies()
            .iter()
            .find(|body| body.index.body_id(ctx.id()).to_string() == "42002cb1ff800003.0501")
            .expect("the body is in its system");
        let record = record_of(snapshot.bodies(), body.index).expect("the body has a record");
        let held = match body.host {
            OrbitHost::Body(parent) => {
                let epoch = Epoch::new(&system, ctx, RECORD_TIME).with_shades();
                let primary = system.body(parent).expect("a moon's parent is a body");
                let host = fate_host(ctx, system.zone_of(primary));
                let fate = system.fate_of(primary, &host);
                let now = system.parent_now(&epoch, &HourCosines::new(), parent, &fate);
                system
                    .record_moon(&epoch, body, (&now, &fate))
                    .map(|derived| {
                        format!(
                            "{:?}, lag {:?}",
                            derived.atmosphere().ice_held(),
                            derived.surface_lag()
                        )
                    })
            }
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => None,
        };
        format!(
            "{}, regime {:?}, IceHeld {held:?}",
            surface_line(record),
            record.surface().ok().map(Surface::climate_regime)
        )
    });
    eprintln!("{rules:?}: 42002cb1ff800003.0501: {line}");
}

/// 42002cb1ff800003.0501, the one hot ice bulk of the 400 whose `T_eq` lay below the melting
/// point (a capture airless under an `H2O` lid on the cold branch): its state, `T_s`, `T_eq`,
/// `p_s`, lid, cover and ice-held air under P14.T56.f's arms alone and under the batch
/// (decision-p14-t56g §5): the lag moves its lid to rock and its `T_eq` towards 0.97 T0.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn the_0501_capture_s_surface_under_both_arms() {
    print_0501(MoonBulkRules::BulkOnly);
    print_0501(MoonBulkRules::Batch);
}

// P14.T51.g.a: the ocean's floor at its liquidus, and none under a lag (decision-p14-t51g Q9, Q4).

/// `system`'s snapshot at [`RECORD_TIME`] under every 24 hold ([`with_every_24_hold`], the ocean
/// floor's among them), the ocean floor's at `floor`, its context `ctx`; `None` for a system not
/// yet formed then.
fn snapshot_at_24(
    system: &PlanetarySystem,
    ctx: &SystemContext,
    floor: OceanFloorRules,
) -> Option<SystemSnapshot> {
    (ctx.age_at(RECORD_TIME).value() > 0.0).then(|| {
        with_every_24_hold(|| with_ocean_floor(floor, || system.snapshot_at(ctx, RECORD_TIME)))
    })
}

/// The pressure at the radius `r` in the water of a body of radius `radius` and mass `mass`, the
/// hydrostatic column over its rock at [`WATER_DENSITY`] (P14.T51.g's structure), written here
/// apart from the rule's own.
fn column_pressure(radius: f64, mass: f64, r: f64) -> f64 {
    let rho = WATER_DENSITY.value();
    let pi = core::f64::consts::PI;
    rho * GRAVITATIONAL_CONSTANT
        * ((mass - 4.0 / 3.0 * pi * rho * radius * radius * radius) * (1.0 / r - 1.0 / radius)
            + 2.0 * pi / 3.0 * rho * (radius * radius - r * r))
}

/// What P14.T51.g.a publishes and moves over a set of systems ([`OceanCensus::visit`]), every 24
/// hold in force.
#[derive(Debug, Default)]
struct OceanCensus {
    /// Moon records with a surface, regular moons among them, and their published oceans.
    moons: u32,
    regular: u32,
    oceans: u32,
    /// Count (xiii): the regular moons whose whole hydrosphere is liquid on rock under the shell,
    /// with F, e, shell, depth and the pressure at the rock.
    rock: Vec<String>,
    /// The oceans that end on ice V, with the same.
    ice_v: Vec<String>,
    /// The members left absent: a shell whose base lies at or beyond ice V's liquidus.
    absent: Vec<String>,
    /// Lagged moons, and those the gate moves off the ocean branch, by name and origin.
    lagged: u32,
    moved_off: Vec<String>,
    /// Every invariant that fails, by name (asserted none).
    broken: Vec<String>,
    /// Each regular moon's published ocean, by name.
    published: BTreeMap<String, Subsurface>,
    /// The counts by kind: oceans by floor and class, absent members by class, and the moons moved
    /// off the branch by origin and class.
    by_kind: BTreeMap<String, u32>,
}

impl OceanCensus {
    /// Adds the system `name` of context `ctx`, `system`, generated under every 24 hold, and its
    /// snapshot at [`RECORD_TIME`] under them, `snapshot`.
    #[expect(
        clippy::too_many_lines,
        reason = "one walk of a system's moons, each published ocean checked and listed"
    )]
    fn visit(
        &mut self,
        name: &str,
        ctx: &SystemContext,
        (system, snapshot): (&PlanetarySystem, &SystemSnapshot),
    ) {
        let t = RECORD_TIME;
        let p_f = base_liquidus(ICE_MELTING_POINT).value();
        let km = |m: f64| m / 1e3;
        with_every_24_hold(|| {
            let epoch = Epoch::new(system, ctx, t).with_shades();
            let cosines = HourCosines::new();
            for body in system.bodies() {
                let (Part::Moon(part), OrbitHost::Body(parent)) = (&body.part, body.host) else {
                    continue;
                };
                let id = format!("{name} {}", body.index.body_id(ctx.id()));
                let Some(record) = record_of(snapshot.bodies(), body.index) else {
                    continue;
                };
                let (Some(surface), Some(bulk)) = (record.surface().ok(), record.bulk().ok())
                else {
                    continue;
                };
                self.moons += 1;
                let figures = surface.figures();
                let primary = system.body(parent).expect("a moon's parent is a body");
                let host = fate_host(ctx, system.zone_of(primary));
                let fate = system.fate_of(primary, &host);
                let now = system.parent_now(&epoch, &cosines, parent, &fate);
                let derived = system
                    .record_moon(&epoch, body, (&now, &fate))
                    .expect("a moon with a surface is derived");
                let origin = match part.satellite.moon() {
                    SatelliteMoon::Regular(_) => "regular",
                    SatelliteMoon::GiantImpact(_) => "giant-impact",
                    SatelliteMoon::Captured(_) => "captured",
                };
                let branch = derived.tides().branch();
                let published = *figures.subsurface_ocean();
                let regime = figures.tectonic_regime();
                if derived.surface_lag() == SurfaceLag::Lagged {
                    self.lagged += 1;
                    if branch == TidalBranch::Ocean
                        || matches!(published, Subsurface::Ocean(_))
                        || regime == TectonicRegime::IceShell
                    {
                        self.broken.push(format!(
                            "{id} ({origin}): lagged on {branch:?}, {published:?}, {regime:?}"
                        ));
                    }
                    let open =
                        with_ocean_under_a_lag(|| system.record_moon(&epoch, body, (&now, &fate)))
                            .expect("a moon with a surface is derived");
                    if open.tides().branch() == TidalBranch::Ocean {
                        *self
                            .by_kind
                            .entry(format!("moved off, {origin}, {:?}", bulk.class()))
                            .or_default() += 1;
                        self.moved_off.push(format!(
                            "{id} ({origin}, {:?}): Ocean → {branch:?}, k₂ {:.4} → {:.4}, F {:.2} \
                             → {:.2} mW m⁻², T_eq {:.1} K",
                            bulk.class(),
                            open.tides().k2(),
                            derived.tides().k2(),
                            open.tides().heat_flow().value() * 1e3,
                            derived.tides().heat_flow().value() * 1e3,
                            derived.equilibrium_temperature().value()
                        ));
                    }
                }
                let SatelliteMoon::Regular(_) = part.satellite.moon() else {
                    continue;
                };
                self.regular += 1;
                self.published.insert(id.clone(), published);
                let (r, rho) = (derived.radius().value(), derived.density().value());
                let mass = 4.0 / 3.0 * core::f64::consts::PI * r * r * r * rho;
                let water = bulk.fractions().water();
                let share = (water * rho / WATER_DENSITY.value()).min(1.0);
                let r_w = r * crate::math::cbrt(1.0 - share);
                let flow = derived.ocean_heat_flow().value();
                let e = part
                    .satellite
                    .orbit_at(&part.parent, ctx.age_at(t))
                    .eccentricity()
                    .value();
                let on_branch = branch == TidalBranch::Ocean;
                // The rule at the inputs the derivation gave it, under either floor: the published
                // member is the floor's, and a rock floor's is 23's to the bit.
                let rule = |rules| {
                    with_ocean_floor(rules, || {
                        Subsurface::of_regular_moon(
                            derived.radius(),
                            derived.density(),
                            water,
                            derived.equilibrium_temperature(),
                            derived.ocean_heat_flow(),
                        )
                    })
                };
                if on_branch && rule(OceanFloorRules::Floored) != published {
                    self.broken.push(format!(
                        "{id}: published {published:?}, not the rule's {:?}",
                        rule(OceanFloorRules::Floored)
                    ));
                }
                let class = format!("{:?}", bulk.class());
                match published {
                    Subsurface::Ocean(ocean) => {
                        self.oceans += 1;
                        *self
                            .by_kind
                            .entry(format!("{:?}, {class}", ocean.floor()))
                            .or_default() += 1;
                        if ocean.floor() == OceanFloor::Rock
                            && rule(OceanFloorRules::Held) != published
                        {
                            self.broken
                                .push(format!("{id}: a rock floor not 23's, {published:?}"));
                        }
                        let (shell, depth) =
                            (ocean.shell_thickness().value(), ocean.thickness().value());
                        let floor = r - shell - depth;
                        let at_floor = column_pressure(r, mass, floor);
                        let top = r - shell;
                        let ocean_mass = 4.0 / 3.0
                            * core::f64::consts::PI
                            * WATER_DENSITY.value()
                            * (top * top * top - floor * floor * floor);
                        let line = format!(
                            "{id}: {:?}, {:.1} km under {:.1} km, F {:.2} mW m⁻², e {e:.4}, P at \
                             its floor {:.1} MPa, at the rock {:.1} MPa, x_w {water:.3}",
                            ocean.floor(),
                            km(depth),
                            km(shell),
                            flow * 1e3,
                            at_floor / 1e6,
                            column_pressure(r, mass, r_w) / 1e6
                        );
                        let fails = [
                            (!on_branch, "off the ocean branch"),
                            (
                                !(depth.is_finite() && depth > 0.0),
                                "a thickness not positive",
                            ),
                            (shell + depth > r - r_w + 1.0, "deeper than its layer"),
                            (
                                ocean_mass > water * mass * (1.0 + 1e-12),
                                "more than its water",
                            ),
                            (
                                at_floor > p_f * (1.0 + 1e-12),
                                "its floor past the liquidus",
                            ),
                            (
                                ocean.floor() == OceanFloor::IceV
                                    && (at_floor / p_f - 1.0).abs() >= 1e-9,
                                "an ice-V floor off the liquidus",
                            ),
                            (
                                ocean.floor() == OceanFloor::Rock && (floor - r_w).abs() > 1.0,
                                "a rock floor off its rock",
                            ),
                            (
                                !matches!(ocean.floor(), OceanFloor::Rock | OceanFloor::IceV),
                                "a floor neither rock nor ice V",
                            ),
                        ];
                        for (failed, what) in fails {
                            if failed {
                                self.broken.push(format!("{line}: {what}"));
                            }
                        }
                        match ocean.floor() {
                            OceanFloor::Rock => self.rock.push(line),
                            OceanFloor::IceIii
                            | OceanFloor::IceV
                            | OceanFloor::IceVi
                            | OceanFloor::IceVii => self.ice_v.push(line),
                        }
                    }
                    Subsurface::NotComputed => {
                        let shell = ICE_CONDUCTIVITY_COEFFICIENT
                            * crate::math::ln(273.0 / derived.equilibrium_temperature().value())
                            / flow;
                        let at_base = column_pressure(r, mass, r - shell);
                        let line = format!(
                            "{id} ({:?}): a shell of {:.1} km to {:.1} MPa, F {:.2} mW m⁻², e \
                             {e:.4}",
                            bulk.class(),
                            km(shell),
                            at_base / 1e6,
                            flow * 1e3
                        );
                        if !on_branch
                            || at_base < p_f * (1.0 - 1e-9)
                            || shell <= ICE_SHELL_LIMIT.value()
                        {
                            self.broken.push(format!("{line}: absent off the rule"));
                        }
                        *self.by_kind.entry(format!("absent, {class}")).or_default() += 1;
                        self.absent.push(line);
                    }
                    Subsurface::NoOcean => {
                        if on_branch {
                            self.broken
                                .push(format!("{id}: on the ocean branch, publishing none"));
                        }
                    }
                }
            }
        });
    }

    /// Prints the census for the bless note: count (xiii), the absent members and the moons the
    /// lag's gate moves off the ocean branch.
    fn print(&self, what: &str) {
        eprintln!(
            "{what}: {} moons with a surface, {} regular, {} publishing an ocean; {} lagged",
            self.moons, self.regular, self.oceans, self.lagged
        );
        eprintln!(
            "count (xiii), {} whole hydrospheres liquid on rock:\n  {}",
            self.rock.len(),
            self.rock.join("\n  ")
        );
        eprintln!(
            "{} oceans on ice V:\n  {}",
            self.ice_v.len(),
            self.ice_v.join("\n  ")
        );
        eprintln!(
            "{} members absent (a shell's base past ice V's liquidus):\n  {}",
            self.absent.len(),
            self.absent.join("\n  ")
        );
        eprintln!(
            "{} lagged moons moved off the ocean branch:\n  {}",
            self.moved_off.len(),
            self.moved_off.join("\n  ")
        );
        eprintln!("by kind: {:?}", self.by_kind);
    }

    /// The census's assertions (decision-p14-t51g Q9's test list): every published ocean on the
    /// ocean branch, the rule's at its derivation's inputs, positive, within its layer, at most its
    /// water's mass, and on rock, 23's to the bit, or on ice V at no more than ice V's liquidus, an
    /// ice-V floor at it to 10⁻⁹; every absent member on the
    /// branch with its shell's base at or past the liquidus, under a shell thicker than an ice
    /// shell regime's limit, so that its regime is its branch's; no lagged moon on the ocean
    /// branch, publishing an ocean or an ice shell's regime.
    fn assert_holds(&self) {
        assert_eq!(self.broken, Vec::<String>::new());
    }
}

/// `record`'s `Debug` with its figures' subsurface ocean cut out.
fn without_its_ocean(record: &BodyRecord) -> String {
    let text = format!("{record:?}");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(start) = rest.find("subsurface_ocean: ") {
        out.push_str(&rest[..start]);
        let end = rest[start..]
            .find(", surface_age: ")
            .expect("the figures' ocean is followed by their surface age");
        rest = &rest[start + end..];
    }
    out.push_str(rest);
    out
}

/// P14.T51.g.a over the golden systems, every 24 hold in force (decision-p14-t51g Q9): every
/// published ocean is positive, within its layer, at most its water's mass and on rock or ice V at
/// no more than ice V's 626.09 MPa; every absent member's shell's base lies past it; no lagged
/// moon is on the ocean branch, publishes an ocean or is an ice shell's regime. Against the ocean
/// floor held, every 24 hold else in force, no record moves but in a regular moon's published
/// ocean, so that neither the floor nor its absent case moves a pass. Count (xiii), the absent
/// members and the lagged moons moved off the branch are listed.
#[test]
fn a_golden_moon_s_ocean_ends_at_its_liquidus_and_none_under_a_lag() {
    let floored = with_every_24_hold(golden_systems_afresh);
    let held =
        with_every_24_hold(|| with_ocean_floor(OceanFloorRules::Held, golden_systems_afresh));
    let mut census = OceanCensus::default();
    let (mut moved, mut oceans_moved) = (Vec::new(), 0_u32);
    for ((name, ctx, floored), (_, _, held)) in floored.into_iter().zip(held) {
        let (Some(after), Some(before)) = (
            snapshot_at_24(&floored, &ctx, OceanFloorRules::Floored),
            snapshot_at_24(&held, &ctx, OceanFloorRules::Held),
        ) else {
            continue;
        };
        census.visit(name, &ctx, (&floored, &after));
        assert_eq!(before.bodies().len(), after.bodies().len(), "{name}");
        for (old, new) in before.bodies().iter().zip(after.bodies()) {
            if without_its_ocean(old) != without_its_ocean(new) {
                moved.push(format!("{name} {}", new.index().body_id(ctx.id())));
            } else if format!("{old:?}") != format!("{new:?}") {
                oceans_moved += 1;
            }
        }
    }
    census.print("the golden systems");
    eprintln!("{oceans_moved} records whose published ocean alone the floor moves");
    census.assert_holds();
    assert_eq!(
        moved,
        Vec::<String>::new(),
        "records the floor moves beyond their ocean"
    );
    assert!(census.regular > 0 && census.oceans > 0, "{census:?}");
}

/// P14.T51.g.a's census over the 400 whole systems, by name, as the golden systems' test, every 24
/// hold in force.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn a_moon_s_ocean_ends_at_its_liquidus_over_the_sample() {
    let mut census = OceanCensus::default();
    for ctx in sample() {
        let system = with_every_24_hold(|| generate(SEED, ctx));
        if let Some(snapshot) = snapshot_at_24(&system, ctx, OceanFloorRules::Floored) {
            census.visit("sample", ctx, (&system, &snapshot));
        }
    }
    census.print("the sample");
    assert!(census.moons > 500, "{}", census.moons);
    census.assert_holds();
}

/// The seed of look-72's universe, LOOK72 (`tests/observe_brief.rs`'s).
const LOOK72: u64 = 0xa49f_649b_ef43_4b20;

/// LOOK72's three regular moons whose oceans look-92-93 read on the wire at 23
/// (decision-p14-t51g Q9): 020e56ef60080002.0c01, 1,395.1 km under 51.3 km, and
/// 420e5bbe00800024.0306, 976.1 km under 29.8 km, which end on ice V at 24 (about 336 and 493 km
/// at the look's inferred bulk); and 420e5bbe00800032.0701, 519.5 km to rock at 429 MPa under
/// 18.5 km, which keeps its whole hydrosphere on rock (count (xiii)'s example).
const LOOK72_OCEANS: [&str; 3] = [
    "020e56ef60080002.0c01",
    "420e5bbe00800024.0306",
    "420e5bbe00800032.0701",
];

/// The systems of the 62 bodies of LOOK72 whose surface state the triage of 2026-10-22 found at
/// odds with their temperature or phases (triage-absurd-states; decision-absurd-states-24 R4.3),
/// by which every 24 member's census reads LOOK72.
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

/// P14.T51.g.a's census over LOOK72, every 24 hold in force: the systems of its three named
/// oceans and of the triage's 62 bodies, as the golden systems' test, with the three by name.
/// 020e56ef60080002.0c01 and 420e5bbe00800024.0306 end on ice V, their depths printed (the
/// ruling's 336 and 493 km are at the look's inferred bulk), and 420e5bbe00800032.0701 keeps its
/// whole hydrosphere on rock, 23's to the bit by the census, within 2 km of the 519.5 km the look
/// read at 23 under 24's other holds.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn look72_s_oceans_end_at_their_liquidus() {
    let galaxy = crate::galaxy::Galaxy::new(Seed::new(LOOK72)).with_full_potential();
    let mut systems: std::collections::BTreeSet<&str> =
        LOOK72_TRIAGED_SYSTEMS.into_iter().collect();
    systems.extend(LOOK72_OCEANS.map(|id| id.split_once('.').expect("a body ID").0));
    let mut census = OceanCensus::default();
    for id in systems {
        let ctx = SystemContext::for_system(&galaxy, id.parse().expect("a system ID"))
            .expect("a system of LOOK72");
        let system = with_every_24_hold(|| generate(galaxy.seed(), &ctx));
        if let Some(snapshot) = snapshot_at_24(&system, &ctx, OceanFloorRules::Floored) {
            census.visit("look72", &ctx, (&system, &snapshot));
        }
    }
    census.print("LOOK72");
    census.assert_holds();
    let published = |id: &str| {
        *census
            .published
            .get(&format!("look72 {id}"))
            .unwrap_or_else(|| panic!("{id} is a regular moon of the census"))
    };
    for id in &LOOK72_OCEANS[..2] {
        let Subsurface::Ocean(ocean) = published(id) else {
            panic!("{id}: an ocean on ice V, not {:?}", published(id));
        };
        assert_eq!(ocean.floor(), OceanFloor::IceV, "{id}: {ocean:?}");
    }
    let id = LOOK72_OCEANS[2];
    let Subsurface::Ocean(whole) = published(id) else {
        panic!("{id}: an ocean on rock, not {:?}", published(id));
    };
    assert_eq!(whole.floor(), OceanFloor::Rock, "{id}: {whole:?}");
    assert!(
        (whole.thickness().value() / 1e3 - 519.5).abs() < 2.0,
        "{id}: {whole:?}"
    );
}
