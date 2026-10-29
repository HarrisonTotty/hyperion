//! P14.T56.g's tests: a hot ice bulk's surface is its heated shell's rock lag
//! (decision-p14-t56g §4, §5, §7 and §8 (d)), on the record as generator version 24 builds it.
//! Their comparison against P14.T56.f's arms alone, and P14.T51.g.a's census of the ocean's floor,
//! went with the 23 → 24 bump's holds (P14.T54.b.l): P14.T51.g.b's solve publishes the ocean, and
//! its census, `system/ocean_tests.rs`, holds that no lagged body publishes one.

use super::{HOT_ICE_LIMIT, shell_lag};
use crate::Seed;
use crate::planetary::derive::SurfaceLag;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::irradiation::{BondAlbedo, equilibrium_temperature};
use crate::planetary::derive::photometry::PhaseTemplate;
use crate::planetary::hooks::crust::{CrustLithology, LidCrust};
use crate::planetary::record::BodyRecord;
use crate::planetary::surface::RECORD_TIME;
use crate::planetary::system::tests::{
    GOLDEN_SYSTEMS_SEED, SEED, golden_contexts, look72_audit_contexts, look72_galaxy, sample,
};
use crate::planetary::system::*;
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

/// What P14.T56.g publishes over a set of systems ([`LagCensus::visit`]).
#[derive(Debug, Default)]
struct LagCensus {
    /// Each lagged record: its kind, its `T0` and its surface.
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
    /// Moons whose derivation's lag is not their own bulk's, or that are lagged with no time hot
    /// (asserted none).
    verdict_broken: Vec<String>,
    /// Icy members whose lag is not their T0's, and planets with a lag (asserted none).
    primary_broken: Vec<String>,
    /// How many records were read, and how many moons with an arm.
    records: u32,
    moons_with_arm: u32,
}

impl LagCensus {
    /// Adds the system `name` of context `ctx`, `system`, walked at [`RECORD_TIME`].
    #[expect(
        clippy::too_many_lines,
        reason = "one walk of a system's records, each checked and listed"
    )]
    fn visit(&mut self, name: &str, ctx: &SystemContext, system: &PlanetarySystem) {
        let t = RECORD_TIME;
        if ctx.age_at(t).value() <= 0.0 {
            return;
        }
        let snapshot = system.snapshot_at(ctx, t);
        let epoch = Epoch::new(system, ctx, t).with_shades();
        let cosines = HourCosines::new();
        for body in system.bodies() {
            let id = format!("{name} {}", body.index.body_id(ctx.id()));
            let Some(record) = record_of(snapshot.bodies(), body.index) else {
                continue;
            };
            let Some(bulk) = record.bulk().ok() else {
                continue;
            };
            self.records += 1;
            // The body's verdict, its derivation's, and its T0.
            let (kind, lag, t0) = match (&body.part, body.host) {
                (Part::Moon(part), OrbitHost::Body(parent)) => {
                    let primary = system.body(parent).expect("a moon's parent is a body");
                    let host = fate_host(ctx, system.zone_of(primary));
                    let fate = system.fate_of(primary, &host);
                    // The parent as its record builds it: a moon's passes read its planet's
                    // field from the planet's record (`ParentNow::planet_field`, P14.T54.b.c and
                    // e), which `parent_now`, building none, leaves unread.
                    let label = crate::planetary::label::label(system, parent)
                        .expect("every body has a label");
                    let (_, now) = system.primary_record(&epoch, &cosines, primary, label, &fate);
                    let derived = system
                        .record_moon(&epoch, body, (&now, &fate))
                        .expect("a moon with a bulk section is derived");
                    assert_eq!(
                        derived.equilibrium_temperature(),
                        bulk.equilibrium_temperature(),
                        "{id}: the census reads the record's derivation"
                    );
                    let lag = derived.surface_lag();
                    let ice = now.moon_ice.as_ref().and_then(|m| m.ice);
                    let t0 =
                        ice.map_or_else(|| zero_albedo(derived.flux()), |ice| ice.t0(&part.orbit));
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
                    // A lag is the arm's: some time hot.
                    if let Some(ice) = ice {
                        self.moons_with_arm += 1;
                        let heated = ice.history.hot_for(t0).value() > 0.0;
                        if lag == SurfaceLag::Lagged && !heated {
                            self.verdict_broken
                                .push(format!("{id}: lagged with no time hot"));
                        }
                    }
                    (format!("{:?}", body.kind()), lag, t0)
                }
                (Part::Planet(_) | Part::Member(_), _) => {
                    let host = fate_host(ctx, system.zone_of(body));
                    let fate = system.fate_of(body, &host);
                    let now = system.parent_now(&epoch, &cosines, body.index, &fate);
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
            if lag == SurfaceLag::Lagged {
                self.lagged.push(format!(
                    "{id} ({kind}, T0 {:.1} K): {}",
                    t0.value(),
                    surface_line(record)
                ));
                if water_lid(record) {
                    self.lagged_water_lid.push(id.clone());
                }
            }
            let Some(surface) = record.surface().ok() else {
                continue;
            };
            if lag == SurfaceLag::Lagged {
                let template = record.photometry().ok().map(BodyPhotometry::template);
                if surface.material() != SurfaceMaterial::Rock
                    || (surface.state() == SurfaceState::Airless
                        && template != Some(PhaseTemplate::Mercury))
                {
                    self.lagged_not_rock
                        .push(format!("{id}: {}", surface_line(record)));
                }
            } else if surface.material() == SurfaceMaterial::Ice && t0 > HOT_ICE_LIMIT {
                let line = format!(
                    "{id} ({kind}, {:?}): {:?}, T0 {:.1} K, T_eq {:.1} K, T_s {:.1} K, p_s {:.4e} \
                     Pa",
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
    }

    /// The census's assertions (decision-p14-t56g §4, §7.2): every lagged record is rock
    /// (`Mercury`'s template where airless), under a lid that is not `H2O`; no airless record has
    /// an `Ice` surface above the melting point at T0, a planet's above all, which goes to main
    /// before the commit; the verdict is the arm's, a member's its T0's.
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
            assert!(
                line.contains("material Rock")
                    && (!line.contains("state Airless") || line.contains("Some(Mercury)")),
                "{name}: {line}"
            );
            assert!(
                !self.lagged_water_lid.iter().any(|id| id == name),
                "{name}'s lid is H2O: {line}"
            );
        }
    }
}

/// The census of the systems `sets` (each its set's name, its seed and its context), each
/// generated and walked ([`LagCensus::visit`]).
fn lag_census(sets: &[(&str, Seed, &SystemContext)]) -> LagCensus {
    let mut census = LagCensus::default();
    for &(name, seed, ctx) in sets {
        census.visit(name, ctx, &generate(seed, ctx));
    }
    census
}

/// P14.T56.g over the golden systems and LOOK72's 30 (`look72_audit_contexts`)
/// (decision-p14-t56g §4, §8 (d); P14.T54.b.k): every lagged record is rock, `Mercury`'s where
/// airless, under a lid that is not `H2O`; no airless record shows ice above the melting point at
/// T0; the verdict is the retention arm's, a member's its T0's; `subgiant` 0101, `red_giant` 0503
/// and 0504 and the members e202 and e204 are lagged rock. Count (ix) is listed.
///
/// `red_giant` e206 is lagged too, by name (main, 2026-10-23): an icy member at T0 289.0 K that
/// lay airless on the cold branch under an `H2O` lid at `T_eq` 220.2 K under P14.T56.f's arms
/// alone, and that the `ocean_froze` gate moves to its secondary crust, at a `T_eq` of 280.7 K.
#[test]
fn a_hot_ice_bulk_s_surface_is_its_heated_shell_s_rock_lag() {
    let start = std::time::Instant::now();
    let look72 = look72_audit_contexts();
    let look72_seed = look72_galaxy().seed();
    let goldens = golden_contexts();
    let sets: Vec<(&str, Seed, &SystemContext)> = goldens
        .iter()
        .map(|(name, ctx)| (*name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx))
        .chain(look72.iter().map(|ctx| ("look72", look72_seed, ctx)))
        .collect();
    let census = lag_census(&sets);
    census.print("the golden systems and LOOK72's");
    eprintln!("in {:.1} s", start.elapsed().as_secs_f64());
    census.assert_holds();
    census.assert_lagged(&GOLDEN_MUST_FIX);
    census.assert_lagged(&[RED_GIANT_E206]);
}

/// P14.T56.g's census over the 400 whole systems, by name: as the golden systems' test, with the
/// six hot ice bulks P14.T56.f leaves `Icy` there lagged rock, and 42002cb1ff800003.0501's surface
/// (decision-p14-t56g §5).
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn a_hot_ice_bulk_s_surface_is_its_rock_lag_over_the_sample() {
    let sets: Vec<(&str, Seed, &SystemContext)> =
        sample().iter().map(|ctx| ("sample", SEED, ctx)).collect();
    let census = lag_census(&sets);
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
    census.assert_lagged(&["sample 42002cb1ff800003.0501"]);
}

/// The arm's verdict on a surface: the rock lag wherever it thins a bulk; bare with no water or no
/// time hot.
#[test]
fn the_retention_arm_s_verdict_lags_only_what_it_thins() {
    assert_eq!(
        shell_lag(0.3749, Metres::new(1.184e6), Years::new(7.25e9)),
        SurfaceLag::Lagged
    );
    // A bulk the arm empties is lagged too: its material is rock either way.
    assert_eq!(
        shell_lag(0.375, Metres::new(1.2e5), Years::new(2.32e9)),
        SurfaceLag::Lagged
    );
    assert_eq!(
        shell_lag(0.375, Metres::new(1.2e5), Years::new(0.0)),
        SurfaceLag::Bare
    );
    assert_eq!(
        shell_lag(0.0, Metres::new(1.2e5), Years::new(9e9)),
        SurfaceLag::Bare
    );
}

/// 42002cb1ff800003.0501's surface (decision-p14-t56g §5, §10.4).
fn print_0501() {
    let ctx = sample()
        .iter()
        .find(|ctx| ctx.id().raw() == 0x4200_2cb1_ff80_0003)
        .expect("the sample holds 42002cb1ff800003");
    let line = {
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
    };
    eprintln!("42002cb1ff800003.0501: {line}");
}

/// 42002cb1ff800003.0501, the one hot ice bulk of the 400 whose `T_eq` lay below the melting
/// point (a capture airless under an `H2O` lid on the cold branch): its state, `T_s`, `T_eq`,
/// `p_s`, lid, cover and ice-held air (decision-p14-t56g §5): the lag, against P14.T56.f's arms
/// alone (a comparison gone with the 23 → 24 bump's holds), moved its lid to rock and its `T_eq`
/// towards 0.97 T0.
#[test]
#[ignore = "a measurement for the bless note, run by name"]
fn the_0501_capture_s_surface_under_both_arms() {
    print_0501();
}
