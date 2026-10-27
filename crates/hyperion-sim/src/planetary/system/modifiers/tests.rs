//! The modifiers on the record (P14.T54.b.c): held at 23, so that every record is 23's; under the
//! batch's rules (the air's sections' hold lifted, `AirRules::Batch`), every solid surface with a
//! crust and a host star carries `SurfaceModifiers::derive` over its published members and what its
//! system gives beside them, R09's inputs carry them to the field's header, and its palette takes
//! the registry's `FeO_T`, the `ferric` deposit and the `tholin` cover, over the plan-14 sample
//! (`system::tests::whole`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use hyperion_surface::field::{MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole};
use hyperion_surface::material::takes_organic_cover;
use hyperion_surface::substance_key::SubstanceKey;
use hyperion_testkit::float::{assert_same_bits, bits};

use super::*;
use crate::id::BodyId;
use crate::planetary::chem::inventory::Reservoir;
use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::derive::atmosphere::{
    Gas, NoMaturity, PassMaturity, PassWeathered, SurfaceMaterial, SurfaceState,
};
use crate::planetary::derive::photometry::area_mix::{
    AreaMixWeathering, CAPPED_ROWS, LidWeathering, REGOLITH_FORMATION_TIME, Regolith, StandIn,
    capped_lid, sun_bands,
};
use crate::planetary::fate::BodyFate;
use crate::planetary::hooks::crust::LidCrust;
use crate::planetary::hooks::modifiers::{
    DUSTY_SHARE, DeriveModifiersError, IO_SO2_FROST_SHARE, IO_SULPHUR_SHARE, ION_STOPPING_COLUMN,
    ModifierInput, VolcanicDeposit, WeatheringDeposit, bears_organic_cover, organic_ground_entries,
};
use crate::planetary::hooks::surface::HourCosines;
use crate::planetary::label;
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::Surface;
use crate::planetary::surface::{CoarseInputs, RECORD_TIME};
use crate::planetary::system::tests::{SEED, golden_systems, whole};
use crate::planetary::system::{
    Body, Epoch, ParentNow, Part, PlanetarySystem, RecordRules, fate_host,
};
use crate::planetary::{BodyIndex, SystemContext, generate};
use crate::substance::optics::palette_feo_mass_fraction;
use crate::substance::{Element, by_key, ids};
use crate::time::UniverseTime;
use crate::units::Kelvin;
use crate::units::consts::{GM_SUN, METRES_PER_AU};

/// How many of the sample's whole systems the record tests build under the batch's rules.
const MODIFIER_SAMPLE: usize = 40;

/// The rules in force, with the air's sections, the modifiers among them, under the batch's: the
/// cold traps stay held.
fn air_batch() -> RecordRules {
    RecordRules {
        air: AirRules::Batch,
        ..RecordRules::in_force()
    }
}

/// A present body with a solid surface of a sample system under the batch's rules at the epoch:
/// its record and, for a moon, its planet's, with what its modifiers read that the record does not
/// publish, each from its own source rather than the record path's: its zone's brightest star with
/// the context's X-ray and ultraviolet history, its primary's orbit then, a moon's semi-major axis
/// about its planet, and its derivation's mantle sulphur and partial pressures.
struct BatchBody {
    ctx: &'static SystemContext,
    record: BodyRecord,
    planet: Option<BodyRecord>,
    star: Option<ModifierStar>,
    seasonal: Option<KeplerElements>,
    age: Years,
    moon_axis: Option<Metres>,
    sulphur: Kilograms,
    gases: PartialPressures,
    /// Its derivation's published pass's maturity's outcome (P14.T54.b.e).
    weathered: PassMaturity,
    /// Its derivation's Bond albedo, its equilibrium temperature, K, and its state.
    published: (f64, f64, SurfaceState),
}

impl BatchBody {
    /// Its surface section, which every batch body has.
    fn surface(&self) -> &Surface {
        match self.record.surface() {
            Section::Ok(surface) => surface,
            other => panic!("{}: {:?}", self.record.identity().id(), other.state()),
        }
    }
}

/// The batch bodies of the first [`MODIFIER_SAMPLE`] whole systems, built once.
fn batch_bodies() -> &'static [BatchBody] {
    static BODIES: OnceLock<Vec<BatchBody>> = OnceLock::new();
    BODIES.get_or_init(|| {
        whole()
            .iter()
            .take(MODIFIER_SAMPLE)
            .flat_map(|(ctx, system)| {
                bodies_of(ctx, &system.clone().with_record_rules(air_batch()))
            })
            .collect()
    })
}

/// The star the modifiers of a body whose primary is `primary` read: its zone's brightest, with
/// its X-ray and ultraviolet history from the context.
fn star_of(epoch: &Epoch<'_>, system: &PlanetarySystem, primary: &Body) -> Option<ModifierStar> {
    epoch
        .brightest(system.zone_of(primary))
        .map(|(member, state)| ModifierStar {
            radius: state.radius(),
            xuv: epoch.ctx.xuv_history(usize::from(member)),
        })
}

/// The batch bodies of `system`, of context `ctx`, at the epoch: its records as its snapshot
/// builds them, body by body, beside each body's own derivation.
fn bodies_of(ctx: &'static SystemContext, system: &PlanetarySystem) -> Vec<BatchBody> {
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
    let sulphur = |elements: &crate::planetary::chem::inventory::ElementInventory| {
        elements.mass(Element::S, Reservoir::Mantle)
    };
    for (body, fate) in system.bodies.iter().zip(&fates) {
        let label = label::label(system, body.index).expect("every body has a label");
        match &body.part {
            Part::Planet(_) | Part::Member(_) => {
                let fate = fate.as_ref().expect("a planet or member orbits a zone");
                let (record, now) = system.primary_record(&epoch, &cosines, body, label, fate);
                if let Some(derived) = &now.derived {
                    bodies.push(BatchBody {
                        ctx,
                        record: record.clone(),
                        planet: None,
                        star: star_of(&epoch, system, body),
                        seasonal: fate.at(t).orbit().copied(),
                        age: ctx.age_at(t),
                        moon_axis: None,
                        sulphur: sulphur(derived.element_inventory()),
                        gases: *derived.atmosphere().partial_pressures(),
                        weathered: *derived.weathered(),
                        published: (
                            derived.albedo().value(),
                            derived.equilibrium_temperature().value(),
                            derived.atmosphere().state(),
                        ),
                    });
                }
                parents.push((body.index, record, now, fate));
            }
            Part::Moon(moon) => {
                let OrbitHost::Body(parent) = body.host else {
                    unreachable!("a moon's host is a body")
                };
                let (planet, now, fate) = parents
                    .iter()
                    .find(|(at, ..)| *at == parent)
                    .map(|(_, planet, now, fate)| (planet, now, *fate))
                    .expect("a moon's planet comes before it");
                let record = system.satellite_record(&epoch, &cosines, body, label, now, fate);
                if let Some(derived) = system.record_moon(&epoch, body, (now, fate)) {
                    let primary = system.body(parent).expect("a moon's planet is a body");
                    bodies.push(BatchBody {
                        ctx,
                        record,
                        planet: Some(planet.clone()),
                        star: star_of(&epoch, system, primary),
                        seasonal: fate.at(t).orbit().copied(),
                        age: ctx.age_at(t),
                        moon_axis: Some(
                            moon.satellite
                                .orbit_at(&moon.parent, ctx.age_at(t))
                                .semi_major_axis(),
                        ),
                        sulphur: sulphur(derived.element_inventory()),
                        gases: *derived.atmosphere().partial_pressures(),
                        weathered: *derived.weathered(),
                        published: (
                            derived.albedo().value(),
                            derived.equilibrium_temperature().value(),
                            derived.atmosphere().state(),
                        ),
                    });
                }
            }
            Part::Ring(_) => {}
        }
    }
    bodies.retain(|body| matches!(body.record.surface(), Section::Ok(_)));
    bodies
}

/// The modifiers' inputs of `body`, rebuilt by hand from its record's published members (the
/// surface's conditions, figures, condensates and crust, the bulk section's gravity, and a moon's
/// planet's bulk radius and field) and what the record does not publish ([`BatchBody`]); `None`
/// where the record gives no crust, no star or no planet's field.
fn rebuilt_inputs(body: &BatchBody) -> Option<ModifierInputs<'_>> {
    let surface = body.surface();
    let Section::Ok(bulk) = body.record.bulk() else {
        panic!("{}: a surface with no bulk", body.record.identity().id());
    };
    let Modelled::Ok(crust) = surface.crust() else {
        return None;
    };
    let star = body.star?;
    let seasonal = body.seasonal.as_ref().expect("a present body has an orbit");
    let primary = match (&body.planet, body.moon_axis) {
        (None, _) => Primary::Star,
        (Some(planet), Some(semi_major_axis)) => {
            let Section::Ok(planet_bulk) = planet.bulk() else {
                return None;
            };
            let field = if planet_bulk.class().has_solid_surface() {
                let Section::Ok(planet_surface) = planet.surface() else {
                    return None;
                };
                planet_surface.figures().magnetic_field()
            } else {
                MagneticField::Strong
            };
            Primary::Planet(PlanetMagnetosphere {
                radius: Metres::from(planet_bulk.radius()),
                field,
                semi_major_axis,
            })
        }
        (Some(_), None) => unreachable!("a moon has its semi-major axis about its planet"),
    };
    let lid = match crust.lid() {
        LidCrust::Primary => crust.primary(),
        LidCrust::Secondary => crust.secondary(),
        LidCrust::Bulk => crust.bulk(),
    }
    .expect("the lid names a crust");
    let (conditions, figures) = (surface.conditions(), surface.figures());
    let water = match (conditions.liquid_water(), figures.wet_epoch()) {
        (true, _) => WaterHistory::WetNow,
        (false, Some(epoch)) => WaterHistory::DrySince(epoch.end()),
        (false, None) => WaterHistory::NeverWet,
    };
    Some(ModifierInputs {
        host: ModifierHost {
            radius: Metres::from(star.radius),
            xuv_luminosity: star.xuv.luminosity(body.age),
            gravitational_parameter: seasonal.gravitational_parameter(),
        },
        orbit: HeliocentricOrbit {
            semi_major_axis: seasonal.semi_major_axis(),
            eccentricity: seasonal.eccentricity().value(),
        },
        primary,
        gravity: bulk.surface_gravity(),
        surface_pressure: conditions.surface_pressure(),
        field: figures.magnetic_field(),
        surface_age: figures.surface_age(),
        resurfacing_time: resurfacing_time(figures.impactors().crust(), figures.heat_flow()),
        heat_flow: figures.heat_flow(),
        surface_temperature: conditions.mean_surface_temperature(),
        mantle_sulphur: body.sulphur,
        condensates: surface.condensates(),
        lid,
        gases: body.gases,
        water,
    })
}

/// Under the batch's rules every sample surface's modifiers are `SurfaceModifiers::derive` over
/// its inputs rebuilt by hand ([`rebuilt_inputs`]), to the bit (their renderings, every `f64` at
/// its shortest round trip), and not modelled exactly where the rebuilt inputs are none or the
/// derivation refuses them. Every refusal is counted and printed: the unlit bodies' mean surface
/// temperature of 0 K (`OutOfRange(SurfaceTemperature)`) is the only one.
#[test]
fn the_record_s_modifiers_are_the_derivation_s_of_its_published_surface() {
    let (mut modelled, mut moons, mut no_inputs) = (0_usize, 0_usize, 0_usize);
    let mut refusals: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for body in batch_bodies() {
        let id = body.record.identity().id();
        let surface = body.surface();
        match (surface.modifiers(), rebuilt_inputs(body)) {
            (Modelled::Ok(held), Some(inputs)) => {
                let derived = SurfaceModifiers::derive(&inputs).unwrap_or_else(|e| {
                    panic!("{id}: the record holds what the inputs refuse: {e}")
                });
                assert_eq!(format!("{held:?}"), format!("{derived:?}"), "{id}");
                modelled += 1;
                moons += usize::from(body.planet.is_some());
            }
            (Modelled::NotModelled, Some(inputs)) => {
                let Err(refused) = SurfaceModifiers::derive(&inputs) else {
                    panic!("{id}: the record holds none of the modifiers its inputs give");
                };
                refusals
                    .entry(format!("{refused:?}"))
                    .or_default()
                    .push(format!(
                        "{id} at {:.3} K",
                        surface.conditions().mean_surface_temperature().value()
                    ));
            }
            (Modelled::NotModelled, None) => no_inputs += 1,
            (Modelled::Ok(_), None) => panic!("{id}: modifiers on a record that gives no inputs"),
        }
    }
    eprintln!(
        "{modelled} surfaces carry their modifiers ({moons} moons); {no_inputs} give no inputs \
         (no crust, star or planet's field); refusals {refusals:#?}"
    );
    assert!(modelled > 100, "{modelled}");
    let unlit = format!(
        "{:?}",
        DeriveModifiersError::OutOfRange(ModifierInput::SurfaceTemperature)
    );
    for (refusal, bodies) in &refusals {
        assert_eq!(refusal, &unlit, "{bodies:?}");
    }
    for body in batch_bodies() {
        if surface_is_unlit(body.surface()) {
            assert_eq!(body.surface().modifiers(), Modelled::NotModelled);
        }
    }
}

/// Whether `surface`'s record states it unlit: its mean surface temperature not positive.
fn surface_is_unlit(surface: &Surface) -> bool {
    let mean = surface.conditions().mean_surface_temperature().value();
    !(mean.is_finite() && mean > 0.0)
}

/// Under the held rules, which the version in force gives, the record computes no modifiers: a
/// surface whose batch record carries them gives none under `Held`, and no planet's field is read
/// (the hold's tripwire, `the_air_sections_are_held_only_until_the_24_bump`, holds every golden
/// surface's modifiers not modelled).
#[test]
fn the_held_record_computes_no_modifiers() {
    assert_eq!(AirRules::in_force(), AirRules::Held);
    let body = batch_bodies()
        .iter()
        .find(|body| matches!(body.surface().modifiers(), Modelled::Ok(_)))
        .expect("a batch surface carries its modifiers");
    let surface = body.surface();
    let case = ModifierCase {
        star: body.star,
        primary: RecordPrimary::Star,
        mantle_sulphur: Some(body.sulphur),
    };
    let Section::Ok(bulk) = body.record.bulk() else {
        panic!("a surface has a bulk section");
    };
    let read = ModifierSurface {
        seasonal: body.seasonal.as_ref().expect("an orbit"),
        age: body.age,
        gravity: bulk.surface_gravity(),
        conditions: surface.conditions(),
        figures: surface.figures(),
        condensates: surface.condensates(),
        crust: surface.crust().ok().copied(),
        gases: &body.gases,
        weathered: None,
    };
    assert!(matches!(
        record_modifiers(AirRules::Batch, &case, &read),
        Modelled::Ok(_)
    ));
    assert_eq!(
        record_modifiers(AirRules::Held, &case, &read),
        Modelled::NotModelled
    );
    let planet = batch_bodies()
        .iter()
        .find_map(|body| body.planet.as_ref())
        .expect("a batch moon");
    assert!(planet_field(AirRules::Batch, planet).is_some());
    assert_eq!(planet_field(AirRules::Held, planet), None);
}

/// The Solar System table's Moon, Mercury, Mars and Titan through the record's assembly under the
/// switch ([`record_modifiers`]), each about the present Sun (1 R☉, its X-ray and ultraviolet
/// history of 1 M☉ and zero-age 0.7 L☉ at the table's 4.57 Gyr) on its own orbit, with its
/// derivation's air and sulphur, the table's figures after their second stage and no condensates;
/// Mars on its own crust (`basalt_fe` at its surface), the others on the Moon's, whose lid reads
/// only into `ferric`'s test. The loose shapes: the Moon's k within a factor 2 of 1 on a mature
/// surface, with no cover and no deposit; Mercury's k above 4; Mars barely weathered, its air
/// stopping the wind; Titan's k, m and cover exactly +0 under its 1.5 bar. Each is printed, with
/// its deposits.
#[test]
fn the_table_s_moon_mercury_mars_and_titan_take_their_shapes_under_the_switch() {
    use crate::orbit::{Eccentricity, Orientation};
    use crate::planetary::derive::solar::{SOLAR_AGE, table_bodies};
    use crate::planetary::hooks::crust::CrustComposition;
    use crate::planetary::hooks::crust::tests::{mars, moon};
    use crate::planetary::hooks::figures::{VolatileHistory, table_figures};
    use crate::planetary::hooks::surface::tests::table;
    use crate::units::{GravitationalParameter, Radians, SolarLuminosities, SolarMasses};

    let none = Condensates::default();
    let star = ModifierStar {
        radius: SolarRadii::new(1.0),
        xuv: XuvHistory::new(SolarMasses::new(1.0), SolarLuminosities::new(0.7)),
    };
    let table = table();
    let conditions_of = |name: &str| &table.iter().find(|(n, ..)| *n == name).unwrap().2;
    let moon_crust = CrustComposition::derive(&moon(conditions_of("Moon"))).unwrap();
    let mars_crust = CrustComposition::derive(&mars(conditions_of("Mars"))).unwrap();
    let mut shapes = BTreeMap::new();
    for (name, body, conditions) in &table {
        if !["Moon", "Mercury", "Mars", "Titan"].contains(name) {
            continue;
        }
        let &(_, _, _, a_au, e) = table_bodies().find(|(n, ..)| n == name).unwrap();
        let seasonal = KeplerElements::from_semi_major_axis(
            Metres::new(a_au * METRES_PER_AU),
            GravitationalParameter::new(GM_SUN),
            Eccentricity::new(e).unwrap(),
            Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap();
        let figures = table_figures(body)
            .unwrap()
            .with_surface(conditions, VolatileHistory::NONE);
        let crust = if *name == "Mars" {
            &mars_crust
        } else {
            &moon_crust
        };
        let modifiers = record_modifiers(
            AirRules::Batch,
            &ModifierCase {
                star: Some(star),
                primary: RecordPrimary::Star,
                mantle_sulphur: Some(body.element_inventory().mass(Element::S, Reservoir::Mantle)),
            },
            &ModifierSurface {
                seasonal: &seasonal,
                age: SOLAR_AGE,
                gravity: body.surface_gravity(),
                conditions,
                figures: &figures,
                condensates: &none,
                crust: Some(crust),
                gases: body.atmosphere().partial_pressures(),
                weathered: None,
            },
        );
        let Modelled::Ok(m) = modifiers else {
            panic!("{name}: not modelled");
        };
        eprintln!(
            "{name}: k {:.4e} (wind {:.3e}, dust {:.3e}), m {:.6}, cover {:.3e}, resurfacing \
             {:.3e} m/yr, volcanic {:?}, weathering {:?}",
            m.weathering_rate(),
            m.weathering().wind(),
            m.weathering().dust(),
            m.maturity(),
            m.organic_cover(),
            m.resurfacing_m_per_yr(),
            m.volcanic_deposits(),
            m.weathering_deposits()
        );
        shapes.insert(*name, m);
    }
    let moon = &shapes["Moon"];
    assert!((0.5..2.0).contains(&moon.weathering_rate()) && moon.maturity() > 0.99);
    assert_same_bits(moon.organic_cover(), 0.0);
    assert!(moon.volcanic_deposits().is_empty() && moon.weathering_deposits().is_empty());
    assert!(shapes["Mercury"].weathering_rate() > 4.0);
    let mars = &shapes["Mars"];
    assert!(mars.weathering_rate() < 1e-100 && mars.maturity() < 1e-100);
    assert_same_bits(mars.weathering().wind(), 0.0);
    let titan = &shapes["Titan"];
    for value in [
        titan.weathering_rate(),
        titan.maturity(),
        titan.organic_cover(),
    ] {
        assert_same_bits(value, 0.0);
    }
}

/// Each loose physical shape of the modifiers over the sample under the batch (the design's
/// Moon-, Mercury-, Mars-, Io- and Titan-like checks), with the first body of each kind printed:
///
/// - every value in its range, the two deposits together at most the surface;
/// - under air above the ion floor no wind and no organic cover reach the ground, and above
///   224 kg m⁻² (`exp(−Σ ÷ 0.3)` underflowing to 0) no dust either: k, m and the cover exactly +0
///   (Titan-like);
/// - an airless, field-free body about a Sun-like star (0.8–1.25 M☉, 3.5–6 Gyr) at 0.8–1.25 au on
///   a surface older than 3 Gyr weathers as the Moon (k in 0.25–4, m above 0.9), and one inside
///   0.5 au faster (k above 2, Mercury-like);
/// - an Io's deposits only at 1 W m⁻² and over, below SO₂'s frost point, in Io's shares;
/// - `ferric` only on a dry `basalt_fe` lid, over [`DUSTY_SHARE`] of the dry, ice-free land
///   (Mars-like);
/// - an organic cover only on airless ground that can take one.
#[test]
fn the_modifiers_take_their_physical_shapes_over_the_sample() {
    let mut kinds: BTreeMap<&str, (usize, String)> = BTreeMap::new();
    for body in batch_bodies() {
        let Modelled::Ok(m) = body.surface().modifiers() else {
            continue;
        };
        for kind in check_shapes(body, m) {
            let entry = kinds.entry(kind).or_insert_with(|| (0, describe(body, m)));
            entry.0 += 1;
        }
    }
    for (kind, (count, first)) in &kinds {
        eprintln!("{kind}: {count}, first {first}");
    }
    for kind in ["airless", "aired"] {
        assert!(kinds.contains_key(kind), "{kind}: {kinds:?}");
    }
}

/// A body's figures for a test's print.
fn describe(body: &BatchBody, m: &SurfaceModifiers) -> String {
    let surface = body.surface();
    format!(
        "{} ({}) a {:.3} au, p_s {:.3e} Pa, T_s {:.1} K, t_s {:.3e} yr, field {:?}: k {:.4e} \
         (wind {:.3e}, dust {:.3e}), m {:.6}, ground {:.3}, cover {:.3e}, resurfacing {:.3e} \
         m/yr, volcanic {:?}, weathering {:?}",
        body.record.identity().id(),
        if body.planet.is_some() {
            "moon"
        } else {
            "about its star"
        },
        body.seasonal
            .as_ref()
            .map_or(0.0, |o| o.semi_major_axis().value() / METRES_PER_AU),
        surface.conditions().surface_pressure().value(),
        surface.conditions().mean_surface_temperature().value(),
        surface.figures().surface_age().value(),
        surface.figures().magnetic_field(),
        m.weathering_rate(),
        m.weathering().wind(),
        m.weathering().dust(),
        m.maturity(),
        m.organic_ground(),
        m.organic_cover(),
        m.resurfacing_m_per_yr(),
        m.volcanic_deposits(),
        m.weathering_deposits(),
    )
}

/// Asserts the shapes of [`the_modifiers_take_their_physical_shapes_over_the_sample`] on `body`,
/// whose modifiers are `m`, and names the kinds it is of.
fn check_shapes(body: &BatchBody, m: &SurfaceModifiers) -> Vec<&'static str> {
    let id = body.record.identity().id();
    let surface = body.surface();
    let (conditions, figures) = (surface.conditions(), surface.figures());
    let Section::Ok(bulk) = body.record.bulk() else {
        panic!("{id}: no bulk");
    };
    let column = conditions.surface_pressure().value() / bulk.surface_gravity().value();
    let unit = |v: f64| (0.0..=1.0).contains(&v) && v.is_sign_positive();
    let k = m.weathering_rate();
    assert!(
        k.is_finite() && k >= 0.0 && k.is_sign_positive(),
        "{id}: k {k}"
    );
    assert!(unit(m.maturity()) && unit(m.organic_cover()), "{id}");
    let deposits = m
        .volcanic_deposits()
        .iter()
        .map(VolcanicDeposit::area_fraction);
    let all = deposits.chain(
        m.weathering_deposits()
            .iter()
            .map(WeatheringDeposit::area_fraction),
    );
    assert!(all.sum::<f64>() <= 1.0 + 1e-12, "{id}");
    let mut kinds = Vec::new();
    if column > ION_STOPPING_COLUMN.value() {
        kinds.push("aired");
        assert_same_bits(m.weathering().wind(), 0.0);
        assert_same_bits(m.organic_cover(), 0.0);
        if column > 224.0 {
            kinds.push("titan_like_or_thicker");
            for value in [k, m.maturity(), m.organic_cover()] {
                assert_same_bits(value, 0.0);
            }
        }
    } else {
        kinds.push("airless");
        let seasonal = body.seasonal.as_ref().expect("an orbit");
        let au = seasonal.semi_major_axis().value() / METRES_PER_AU;
        let solar_mass = seasonal.gravitational_parameter().value() / GM_SUN;
        let sun_like =
            (0.8..1.25).contains(&solar_mass) && (3.5e9..6e9).contains(&body.age.value());
        let bare = body.planet.is_none() && figures.magnetic_field() == MagneticField::None;
        let old = figures.surface_age().value() > 3e9;
        if bare && sun_like && old && (0.8..1.25).contains(&au) {
            kinds.push("moon_like");
            assert!(
                (0.25..4.0).contains(&k) && m.maturity() > 0.9,
                "{id}: k {k}"
            );
        }
        if bare && sun_like && au < 0.5 {
            kinds.push("mercury_like");
            assert!(k > 2.0, "{id}: k {k}");
        }
    }
    if !m.volcanic_deposits().is_empty() {
        kinds.push("io_like");
        assert!(figures.heat_flow().value() >= 1.0, "{id}");
        assert!(
            conditions.mean_surface_temperature().value() < 112.06,
            "{id}"
        );
        let shares: Vec<_> = m
            .volcanic_deposits()
            .iter()
            .map(|d| bits(d.area_fraction()))
            .collect();
        let io = [IO_SO2_FROST_SHARE, IO_SULPHUR_SHARE].map(bits);
        assert_eq!(shares, io, "{id}");
    }
    if let [ferric] = m.weathering_deposits() {
        kinds.push("mars_like");
        check_ferric(body, m, ferric.area_fraction());
    }
    if m.organic_cover() > 0.0 {
        kinds.push("organic_cover");
        assert!(
            column <= ION_STOPPING_COLUMN.value() && m.organic_ground() > 0.0,
            "{id}"
        );
    }
    kinds
}

/// Asserts that `body`'s `ferric` deposit of area `area`, under its modifiers `m`, lies on a dry
/// `basalt_fe` lid over [`DUSTY_SHARE`] of its dry, ice-free land, less its volcanic deposits.
fn check_ferric(body: &BatchBody, m: &SurfaceModifiers, area: f64) {
    let id = body.record.identity().id();
    let surface = body.surface();
    let Modelled::Ok(crust) = surface.crust() else {
        panic!("{id}: a deposit with no crust");
    };
    assert_eq!(crust.lid_lithology(), by_key("basalt_fe").unwrap(), "{id}");
    assert!(!surface.conditions().liquid_water(), "{id}");
    let covered = surface
        .condensates()
        .entries()
        .iter()
        .filter(|e| match e.phase() {
            CondensatePhase::Solid => !e.is_seasonal(),
            CondensatePhase::Liquid => true,
            CondensatePhase::Supercritical => false,
        })
        .fold(0.0, |sum, e| sum + e.area_fraction());
    let erupted = m
        .volcanic_deposits()
        .iter()
        .fold(0.0, |sum, d| sum + d.area_fraction());
    let land = (1.0 - covered).clamp(0.0, 1.0) - erupted;
    assert!(
        (area - DUSTY_SHARE * land).abs() < 1e-12,
        "{id}: {area} on {land}"
    );
}

/// R09's inputs carry the record's modifiers to the field's header: under the batch the inputs'
/// weathering and organic cover are the record's to the bit, and none where the record's are not
/// modelled; each palette entry's `FeO_T` is the registry's for a solid
/// ([`palette_feo_mass_fraction`]) and none for a liquid; its `Deposit` entries are the weathering
/// deposits' and its `Cover` entry the `tholin` exactly where the cover is above 0; and no palette
/// holds more than [`MaterialPalette::MAX_ENTRIES`]; and every body `for_body` reads under the
/// rules in force reads under the batch too, so that the batch's new `Deposit` and `Cover` entries
/// push no palette past its entries unseen (a refusal by the palette or a modifier's entry would
/// drop the body from the batch's read). The generated Moon-like bodies' highlands and maria are
/// printed.
#[test]
fn under_the_batch_the_inputs_carry_the_record_s_modifiers() {
    let (mut read, mut weathered, mut covered, mut ferric) = (0_usize, 0_usize, 0_usize, 0_usize);
    let mut unread: BTreeMap<String, usize> = BTreeMap::new();
    let mut read_ids = BTreeSet::new();
    for body in batch_bodies() {
        let id = body.record.identity().id();
        let inputs = match CoarseInputs::for_body(&body.record, body.planet.as_ref(), body.ctx) {
            Ok(inputs) => inputs,
            Err(error) => {
                *unread.entry(format!("{error:?}")).or_default() += 1;
                continue;
            }
        };
        read += 1;
        read_ids.insert(id);
        let modifiers = body.surface().modifiers();
        match modifiers {
            Modelled::Ok(m) => {
                let w = inputs.weathering().expect("the weathering parts");
                assert_same_bits(w.maturity, m.maturity());
                assert_same_bits(w.weathering_rate, m.weathering_rate());
                assert_same_bits(
                    inputs.organic_cover().expect("the cover"),
                    m.organic_cover(),
                );
                weathered += 1;
            }
            Modelled::NotModelled => {
                assert_eq!(
                    (inputs.weathering(), inputs.organic_cover()),
                    (None, None),
                    "{id}"
                );
            }
        }
        let palette = inputs.palette();
        assert!(palette.len() <= MaterialPalette::MAX_ENTRIES, "{id}");
        let (deposits, cover) = check_palette(palette, modifiers, &id.to_string());
        ferric += deposits;
        covered += usize::from(cover);
    }
    eprintln!(
        "under the batch: {read} bodies read, {weathered} weathered, {ferric} with `ferric`, \
         {covered} with the `tholin` cover; not read {unread:?}"
    );
    assert!(read > 100 && weathered > 100, "{read} {weathered}");
    let in_force = read_under_the_rules_in_force();
    let lost: Vec<_> = in_force.difference(&read_ids).collect();
    assert!(
        lost.is_empty(),
        "{} of {} bodies read under the rules in force are not read under the batch: {lost:?}",
        lost.len(),
        in_force.len()
    );
    print_moon_like_palettes();
}

/// The bodies of the first [`MODIFIER_SAMPLE`] whole systems that `for_body` reads at the epoch
/// under the rules in force, each with its planet's record from the same snapshot.
fn read_under_the_rules_in_force() -> BTreeSet<BodyId> {
    whole()
        .iter()
        .take(MODIFIER_SAMPLE)
        .flat_map(|(ctx, system)| {
            let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
            snapshot
                .bodies()
                .iter()
                .filter(|record| {
                    let planet = CoarseInputs::planet_of(record).and_then(|i| snapshot.body(i));
                    CoarseInputs::for_body(record, planet, ctx).is_ok()
                })
                .map(|record| record.identity().id())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Asserts `palette`'s `FeO_T`, deposits and cover against the record's modifiers `modifiers` for
/// the body `id`, and gives its number of `Deposit` entries and whether it holds the cover.
fn check_palette(
    palette: &MaterialPalette,
    modifiers: Modelled<&SurfaceModifiers>,
    id: &str,
) -> (usize, bool) {
    let row = |entry: &PaletteEntry| by_key(entry.substance.as_str()).expect("a palette row");
    for entry in palette.entries() {
        let expected = match (modifiers, entry.role) {
            (Modelled::NotModelled, _) | (_, PaletteRole::Liquid) => None,
            (Modelled::Ok(_), _) => palette_feo_mass_fraction(row(entry)),
        };
        assert_eq!(
            entry.feo_mass_fraction.map(bits),
            expected.map(bits),
            "{id}: {}",
            entry.substance
        );
    }
    let of_role = |role: PaletteRole| {
        palette
            .entries()
            .iter()
            .filter(move |e| e.role == role)
            .map(|e| e.substance)
            .collect::<Vec<_>>()
    };
    let tholin = SubstanceKey::new("tholin").expect("a key");
    let (deposits, cover) = match modifiers {
        Modelled::Ok(m) => (
            m.weathering_deposits()
                .iter()
                .map(|d| d.substance().substance().key())
                .collect::<Vec<_>>(),
            if m.organic_cover() > 0.0 {
                vec![tholin]
            } else {
                vec![]
            },
        ),
        Modelled::NotModelled => (vec![], vec![]),
    };
    assert_eq!(of_role(PaletteRole::Deposit), deposits, "{id}");
    assert_eq!(of_role(PaletteRole::Cover), cover, "{id}");
    if let Some(entry) = palette.find(PaletteRole::Cover, tholin) {
        let organic = palette.get(entry).expect("the cover's entry");
        assert_eq!(organic.mechanics, MechanicsFamily::Organic, "{id}");
    }
    (deposits.len(), !cover.is_empty())
}

/// Prints the `FeO_T` of the palettes of the batch's airless bodies whose crust holds
/// `feldspathic` highlands or `basalt_fe` maria, the generated Moon-like bodies (against the
/// plan's 0.05 and 0.20), and asserts each is the registry's.
fn print_moon_like_palettes() {
    let mut printed = 0;
    for body in batch_bodies() {
        let Ok(inputs) = CoarseInputs::for_body(&body.record, body.planet.as_ref(), body.ctx)
        else {
            continue;
        };
        let feo: Vec<_> = inputs
            .palette()
            .entries()
            .iter()
            .filter(|e| ["feldspathic", "basalt_fe"].contains(&e.substance.as_str()))
            .map(|e| (e.substance.as_str().to_owned(), e.feo_mass_fraction))
            .collect();
        if feo.len() == 2 && printed < 3 {
            eprintln!(
                "{} highlands and maria: {feo:?}",
                body.record.identity().id()
            );
            printed += 1;
        }
        for (key, value) in feo {
            let registry = by_key(&key).and_then(palette_feo_mass_fraction);
            assert_eq!(value.map(bits), registry.map(bits), "{key}");
        }
    }
}

/// The surface crate's cover rule (`takes_organic_cover`, decision-r10-t1c-maturity §3.2) against
/// the sim's (`organic_ground`): on every generated body under the batch, every entry the sim
/// counts is an `Ice` entry the surface crate marks, and every other entry it marks is one the
/// palette cannot tell from those (a seasonal ice, one with no area, an ice the figures alone put
/// on the ground, or nitrogen beside only such), each kind counted and printed; and registry-wide
/// the two carbon tests agree on every condensed row with a phase diagram.
#[test]
fn the_cover_rule_marks_the_sim_s_organic_ground_on_every_generated_body() {
    let mut beyond: BTreeMap<&str, usize> = BTreeMap::new();
    let mut counted_total = 0_usize;
    for body in batch_bodies() {
        let Ok(inputs) = CoarseInputs::for_body(&body.record, body.planet.as_ref(), body.ctx)
        else {
            continue;
        };
        let id = body.record.identity().id();
        let palette = inputs.palette();
        let marked: BTreeSet<SubstanceKey> = palette
            .entries()
            .iter()
            .filter(|e| takes_organic_cover(palette, e))
            .map(|e| e.substance)
            .collect();
        let condensates = body.surface().condensates();
        let counted: BTreeSet<SubstanceKey> = organic_ground_entries(condensates)
            .map(|e| e.substance().substance().key())
            .collect();
        assert!(
            counted.is_subset(&marked),
            "{id}: {counted:?} against {marked:?}"
        );
        counted_total += counted.len();
        for key in marked.difference(&counted) {
            let entry = condensates.entries().iter().find(|e| {
                e.phase() == CondensatePhase::Solid && e.substance().substance().key() == *key
            });
            let kind = match entry {
                None => "an ice the figures alone put on the ground",
                Some(e) if e.is_seasonal() => "a seasonal ice",
                Some(e) if e.area_fraction() <= 0.0 => "an ice with no area",
                Some(_) => {
                    assert_eq!(key.as_str(), "N2", "{id}: {key}");
                    "nitrogen beside only such"
                }
            };
            *beyond.entry(kind).or_default() += 1;
        }
    }
    eprintln!("the sim counts {counted_total} ices; the palette marks beyond them {beyond:?}");
    check_carbon_rules_registry_wide();
}

/// The two carbon tests on every condensed row with a phase diagram: the surface crate's on a
/// palette of the row's `Ice` entry (and of nitrogen's beside it) against the sim's
/// [`bears_organic_cover`].
fn check_carbon_rules_registry_wide() {
    let entry = |key: SubstanceKey| PaletteEntry {
        substance: key,
        role: PaletteRole::Ice,
        normal_albedo_bvr: [0.5; 3],
        phase_row: 1,
        grain_density_kg_m3: 1_000.0,
        transition: Kelvin::new(100.0),
        mechanics: MechanicsFamily::VolatileIce,
        liquid: None,
        feo_mass_fraction: None,
    };
    let nitrogen = Gas::Nitrogen.substance();
    let mut rows = 0;
    for id in ids() {
        let row = id.substance();
        if row.condensed().is_none() || row.phase_diagram().is_none() || id == nitrogen {
            continue;
        }
        let alone = MaterialPalette::new(vec![entry(row.key())]).expect("a palette");
        let marked = takes_organic_cover(&alone, &alone.entries()[0]);
        assert_eq!(marked, bears_organic_cover(id), "{}", row.key());
        let mut pair = vec![entry(row.key()), entry(nitrogen.substance().key())];
        pair.sort_by_key(|e| e.substance);
        let beside = MaterialPalette::new(pair).expect("a palette");
        let n2 = beside
            .entries()
            .iter()
            .find(|e| e.substance == nitrogen.substance().key())
            .expect("nitrogen's entry");
        assert_eq!(
            takes_organic_cover(&beside, n2),
            marked,
            "{} beside N2",
            row.key()
        );
        rows += 1;
    }
    assert!(rows > 10, "{rows}");
}

/// The same seed twice gives the same modifiers: the first sample systems' records under the
/// batch, generated again from the seed, carry the cached ones' to their renderings.
#[test]
fn the_same_seed_twice_gives_the_same_modifiers() {
    let mut compared = 0;
    for (ctx, _) in whole().iter().take(4) {
        let again = generate(SEED, ctx).with_record_rules(air_batch());
        for body in bodies_of(ctx, &again) {
            let cached = batch_bodies()
                .iter()
                .find(|b| b.record.identity().id() == body.record.identity().id())
                .expect("the body is in the cached batch");
            assert_eq!(
                format!("{:?}", body.surface().modifiers()),
                format!("{:?}", cached.surface().modifiers()),
                "{}",
                body.record.identity().id()
            );
            compared += usize::from(matches!(body.surface().modifiers(), Modelled::Ok(_)));
        }
    }
    eprintln!("{compared} modifiers the same bits twice");
    assert!(compared > 5, "{compared}");
}

/// At the version in force, which holds the modifiers off the record, R09's inputs carry none of
/// them on the golden systems' bodies: no weathering, no organic cover, no `FeO_T`, no `Deposit`
/// or `Cover` entry, so that every field and palette keeps its 23 bytes.
#[test]
fn the_held_inputs_carry_no_modifiers() {
    let mut read = 0;
    for (name, ctx, system) in golden_systems() {
        let snapshot = system.snapshot_at(ctx, RECORD_TIME);
        let records = snapshot.bodies();
        for record in records {
            let planet = CoarseInputs::planet_of(record)
                .and_then(|index| records.iter().find(|r| r.index() == index));
            let Ok(inputs) = CoarseInputs::for_body(record, planet, ctx) else {
                continue;
            };
            let id = record.identity().id();
            assert_eq!(
                (inputs.weathering(), inputs.organic_cover()),
                (None, None),
                "{name} {id}"
            );
            for entry in inputs.palette().entries() {
                assert_eq!(entry.feo_mass_fraction, None, "{name} {id}");
                assert!(
                    !matches!(entry.role, PaletteRole::Deposit | PaletteRole::Cover),
                    "{name} {id}"
                );
            }
            read += 1;
        }
    }
    assert!(read > 10, "{read}");
}

/// The rows' `FeO_T` (`Substance::feo_mass_fraction`) and the palette's: the melt rows' FeO(T)
/// over 100, `ferric`'s by its mode by weight, troilite's by its formula, the iron-free and
/// metallic rows +0, the rows of no
/// modelled composition none, and `mars_dust`'s observed albedo none in the palette; every value
/// in [+0, 1].
#[test]
fn the_registry_s_feo_t_is_its_composition_s() {
    let feo = |key: &str| by_key(key).expect("a row").substance().feo_mass_fraction();
    for (key, expected) in [
        ("basalt", 0.109_698),
        ("lava_glass", 0.109_698),
        ("basalt_fe", 0.202),
        ("mars_dust", 0.202),
        ("ferric", 0.269_55),
        ("komatiite", 0.129),
        ("basalt_reduced", 0.0337),
        ("granite", 0.018_458),
        ("ultramafic", 0.080_534),
        ("anorthosite", 0.012_72),
        ("feldspathic", 0.051),
        ("FeS", 0.817_3),
    ] {
        let value = feo(key).unwrap_or_else(|| panic!("{key} has its FeO_T"));
        assert!((value - expected).abs() < 5e-5, "{key}: {value}");
        eprintln!("{key}: FeO_T {value:.6}");
    }
    // `ferric`'s is its mode's by weight: 90.4 wt% `basalt_fe`, 1.0 hematite, 1.8 magnetite and
    // 6.8 nanophase ferric oxide as Fe₂O₃, each part's iron as FeO by its formula (IUPAC 2021's
    // abridged Fe 55.845 and O 15.999).
    let (ferrous, ferric_oxide): (f64, f64) = (55.845 + 15.999, 2.0 * 55.845 + 3.0 * 15.999);
    let (hematite, magnetite) = (
        2.0 * ferrous / ferric_oxide,
        3.0 * ferrous / (ferrous + ferric_oxide),
    );
    assert!((hematite - 0.8998).abs() < 5e-5 && (magnetite - 0.9309).abs() < 5e-5);
    let basalt_fe = feo("basalt_fe").expect("basalt_fe's FeO_T");
    let mode = 0.904 * basalt_fe + 0.010 * hematite + 0.018 * magnetite + 0.068 * hematite;
    let ferric = feo("ferric").expect("ferric's FeO_T");
    assert!(
        (ferric - mode).abs() < 1e-12,
        "ferric: {ferric} against {mode}"
    );
    let zero = bits(0.0);
    for key in [
        "H2O",
        "CO2",
        "N2",
        "CH4",
        "CO",
        "SO2",
        "S8",
        "NaCl",
        "tholin",
        "soot",
        "graphite",
        "Fe",
        "iron_nickel",
    ] {
        assert_eq!(feo(key).map(bits), Some(zero), "{key}");
    }
    for key in [
        "chondrite_ci",
        "chondrite_cm",
        "chondrite_cv",
        "chondrite_h",
        "chondrite_l",
        "chondrite_ll",
        "chondrite_e",
        "phyllosilicate",
        "cometary",
        "ca_al_residue",
        "brine_nacl",
        "ch4_c2h6_n2",
    ] {
        assert_eq!(feo(key), None, "{key}");
    }
    let row = |key: &str| by_key(key).expect("a row");
    assert_eq!(palette_feo_mass_fraction(row("mars_dust")), None);
    for id in ids() {
        if let Some(value) = id.substance().feo_mass_fraction() {
            assert!(
                (0.0..=1.0).contains(&value) && value.is_sign_positive(),
                "{}: {value}",
                id.substance().key()
            );
            if id != row("mars_dust") {
                assert_eq!(palette_feo_mass_fraction(id).map(bits), Some(bits(value)));
            }
        }
    }
}

// P14.T54.b.e: the weathered area mix, every 24 hold in force, the air's sections under the batch.

/// The batch bodies of the first `count` sample systems, generated afresh under every 24 hold
/// with the weathered area mix at `rules` and the air's sections under the batch's rules: a
/// system's records derive lazily, so each is built inside the switches.
fn bodies_at_24(count: usize, rules: AreaMixWeathering) -> Vec<BatchBody> {
    use crate::planetary::derive::photometry::area_mix::with_area_mix_weathering;
    use crate::planetary::system::tests::sample;
    use crate::planetary::testing::with_every_24_hold;

    with_every_24_hold(|| {
        with_area_mix_weathering(rules, || {
            sample()
                .iter()
                .take(count)
                .flat_map(|ctx| {
                    let system = generate(SEED, ctx).with_record_rules(air_batch());
                    bodies_of(ctx, &system)
                })
                .collect()
        })
    })
}

/// The first [`MODIFIER_SAMPLE`] systems' batch bodies under the weathered area mix, built once.
fn weathered_bodies() -> &'static [BatchBody] {
    static BODIES: OnceLock<Vec<BatchBody>> = OnceLock::new();
    BODIES.get_or_init(|| bodies_at_24(MODIFIER_SAMPLE, AreaMixWeathering::Weathered))
}

/// The same under every 24 hold but the weathered area mix's, built once.
fn held_bodies_at_24() -> &'static [BatchBody] {
    static BODIES: OnceLock<Vec<BatchBody>> = OnceLock::new();
    BODIES.get_or_init(|| bodies_at_24(MODIFIER_SAMPLE, AreaMixWeathering::Held))
}

/// The golden systems, generated afresh under the switches, as [`golden_bodies_at_24`] keeps them:
/// each body borrows its context for good.
type Goldens = Vec<(&'static str, SystemContext, PlanetarySystem)>;

/// How a census view takes a lid younger than [`REGOLITH_FORMATION_TIME`] under the weathered area
/// mix (decision-p14-t54b-e-findings §A.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum YoungLava {
    /// The rules in force: it keeps the cap (`StandIn::UnweatheredLava`).
    Capped,
    /// P14.T54.b.e as built (1ebcd3b6): weathered as any other lid
    /// (`area_mix::with_young_lava_weathered`).
    AsBuilt,
}

/// `run`'s result under every 24 hold with the weathered area mix at `rules` and young lava as
/// `lava` takes it.
fn under_24<R>(rules: AreaMixWeathering, lava: YoungLava, run: impl FnOnce() -> R) -> R {
    use crate::planetary::derive::photometry::area_mix::{
        with_area_mix_weathering, with_young_lava_weathered,
    };
    use crate::planetary::testing::with_every_24_hold;

    with_every_24_hold(|| {
        with_area_mix_weathering(rules, || match lava {
            YoungLava::Capped => run(),
            YoungLava::AsBuilt => with_young_lava_weathered(run),
        })
    })
}

/// The golden systems' batch bodies under every 24 hold, the weathered area mix at `rules` and
/// young lava as `lava` takes it, the systems generated afresh into `systems` under the same
/// switches.
fn golden_bodies_at_24(
    rules: AreaMixWeathering,
    lava: YoungLava,
    systems: &'static OnceLock<Goldens>,
) -> Vec<BatchBody> {
    use crate::planetary::system::tests::golden_systems_afresh;

    let systems = systems.get_or_init(|| under_24(rules, lava, golden_systems_afresh));
    under_24(rules, lava, || {
        systems
            .iter()
            .flat_map(|(_, ctx, system)| {
                bodies_of(ctx, &system.clone().with_record_rules(air_batch()))
            })
            .collect()
    })
}

/// The golden systems' batch bodies under every 24 hold and the weathered area mix, generated
/// afresh, built once.
fn golden_weathered_bodies() -> &'static [BatchBody] {
    static SYSTEMS: OnceLock<Goldens> = OnceLock::new();
    static BODIES: OnceLock<Vec<BatchBody>> = OnceLock::new();
    BODIES.get_or_init(|| {
        golden_bodies_at_24(AreaMixWeathering::Weathered, YoungLava::Capped, &SYSTEMS)
    })
}

/// The same under every 24 hold but the weathered area mix's, built once.
fn golden_held_bodies_at_24() -> &'static [BatchBody] {
    static SYSTEMS: OnceLock<Goldens> = OnceLock::new();
    static BODIES: OnceLock<Vec<BatchBody>> = OnceLock::new();
    BODIES.get_or_init(|| golden_bodies_at_24(AreaMixWeathering::Held, YoungLava::Capped, &SYSTEMS))
}

/// The same under the weathered area mix as P14.T54.b.e built it, young lava weathered, built
/// once.
fn golden_as_built_bodies() -> &'static [BatchBody] {
    static SYSTEMS: OnceLock<Goldens> = OnceLock::new();
    static BODIES: OnceLock<Vec<BatchBody>> = OnceLock::new();
    BODIES.get_or_init(|| {
        golden_bodies_at_24(AreaMixWeathering::Weathered, YoungLava::AsBuilt, &SYSTEMS)
    })
}

/// The first [`MODIFIER_SAMPLE`] systems' batch bodies under the weathered area mix as P14.T54.b.e
/// built it, young lava weathered, built once.
fn as_built_bodies() -> &'static [BatchBody] {
    static BODIES: OnceLock<Vec<BatchBody>> = OnceLock::new();
    BODIES.get_or_init(|| {
        under_24(AreaMixWeathering::Weathered, YoungLava::AsBuilt, || {
            bodies_at_24(MODIFIER_SAMPLE, AreaMixWeathering::Weathered)
        })
    })
}

/// The Bond albedo about a Sun of the regolith `regolith`, airless: what its bins, or the
/// stand-in's arm, give under one light, for comparing two regoliths to the bit.
fn regolith_bond(regolith: &Regolith) -> f64 {
    regolith.bond(SurfaceState::Airless, &sun_bands()).bond
}

/// Rule (f) under the weathered area mix (P14.T54.b.e), over the first [`MODIFIER_SAMPLE`] sample
/// systems and the golden systems, every 24 hold in force and the air's sections under the batch:
/// every record whose published pass took a maturity carries its modifiers, and they are that
/// pass's to the bit, the modifiers over its parts, the record's condensates (the pass's, rule (f))
/// and its water; their maturity is the pass's; the pass's parts that the record publishes too
/// (the host, the orbit, what it orbits, the bulk section's gravity, the conditions' surface
/// pressure and mean temperature, the mantle's sulphur, the crust's lid and the air's gases) are
/// the record's own to the bit, and its field the figures'; the regolith its area mix read is the
/// record's lid weathered at that maturity (or capped, `basalt_reduced`), to the bit; and the
/// `ferric` deposit its area mix read is the record's. The context's surface age and heat flow,
/// which the figures (K(P*)) read from the published pass's own air, are stated against the
/// figures'.
///
/// R09's field header (`CoarseInputs::for_body`) carries that maturity to the bit, and its lid
/// entry the `FeO_T` the area mix's weathering read, so that the server's albedo scale
/// (`surface/service.rs::field_albedo_scale`, whose own test holds it to the albedos weathered at
/// its header's maturity) is solved on the albedos weathered at the maturity the area mix read
/// (P14.T54.b's "the server's albedo scale solved on the weathered albedos").
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "rule (f), member by member, with the header's half of the albedo scale's test"
)]
fn under_the_weathered_area_mix_every_record_s_modifiers_are_its_published_pass_s() {
    let (mut checked, mut capped, mut ferric, mut headers) = (0_usize, 0_usize, 0_usize, 0_usize);
    let mut none: BTreeMap<String, usize> = BTreeMap::new();
    let (mut age_moved, mut heat_moved) = (0.0_f64, 0.0_f64);
    let mut mismatches = Vec::new();
    for body in weathered_bodies().iter().chain(golden_weathered_bodies()) {
        let id = body.record.identity().id();
        let surface = body.surface();
        let pass = match &body.weathered {
            Ok(pass) => pass,
            Err(why) => {
                *none.entry(format!("{why:?}")).or_default() += 1;
                continue;
            }
        };
        let Modelled::Ok(record) = surface.modifiers() else {
            panic!("{id}: its pass took a maturity, and its record carries no modifiers");
        };
        let rebuilt = rebuilt_inputs(body).expect("a record with modifiers gives its inputs");
        let parts = pass.parts();
        let members = [
            (
                "host",
                format!("{:?}", parts.host),
                format!("{:?}", rebuilt.host),
            ),
            (
                "orbit",
                format!("{:?}", parts.orbit),
                format!("{:?}", rebuilt.orbit),
            ),
            (
                "primary",
                format!("{:?}", parts.primary),
                format!("{:?}", rebuilt.primary),
            ),
            (
                "gravity",
                format!("{:?}", parts.gravity),
                format!("{:?}", rebuilt.gravity),
            ),
            (
                "surface pressure",
                format!("{:?}", parts.surface_pressure),
                format!("{:?}", rebuilt.surface_pressure),
            ),
            (
                "surface temperature",
                format!("{:?}", parts.surface_temperature),
                format!("{:?}", rebuilt.surface_temperature),
            ),
            (
                "mantle sulphur",
                format!("{:?}", parts.mantle_sulphur),
                format!("{:?}", rebuilt.mantle_sulphur),
            ),
            (
                "lid",
                format!("{:?}", parts.lid),
                format!("{:?}", rebuilt.lid),
            ),
            (
                "gases",
                format!("{:?}", parts.gases),
                format!("{:?}", rebuilt.gases),
            ),
        ];
        for (member, pass_s, record_s) in members {
            if pass_s != record_s {
                mismatches.push(format!(
                    "{id} ({}): {member} {pass_s} against the record's {record_s}",
                    if body.planet.is_some() {
                        "moon"
                    } else {
                        "primary"
                    }
                ));
            }
        }
        let own = SurfaceModifiers::derive(
            &pass
                .parts()
                .with_published(surface.condensates(), pass.water()),
        )
        .unwrap_or_else(|e| panic!("{id}: {e}"));
        if format!("{record:?}") != format!("{own:?}") {
            mismatches.push(format!(
                "{id}: the modifiers {record:?} against the pass's {own:?}"
            ));
        }
        if record.maturity().total_cmp(&pass.maturity()).is_ne() {
            mismatches.push(format!(
                "{id}: the maturity {} against the pass's {}",
                record.maturity(),
                pass.maturity()
            ));
        }
        assert_eq!(parts.field, rebuilt.field, "{id}");
        let relative = |a: f64, b: f64| {
            if b > 0.0 {
                (a / b - 1.0).abs()
            } else {
                a.abs()
            }
        };
        age_moved = age_moved.max(relative(
            parts.surface_age.value(),
            rebuilt.surface_age.value(),
        ));
        heat_moved = heat_moved.max(relative(parts.heat_flow.value(), rebuilt.heat_flow.value()));
        let regolith = Regolith::weathered_lid(
            rebuilt.lid,
            surface.material(),
            LidWeathering {
                maturity: pass.maturity(),
                temperature: surface.conditions().mean_surface_temperature(),
                surface_age: parts.surface_age,
                state: body.published.2,
            },
        );
        assert_eq!(
            regolith.is_weathered(),
            pass.regolith().is_weathered(),
            "{id}"
        );
        assert_same_bits(regolith_bond(&regolith), regolith_bond(&pass.regolith()));
        capped += usize::from(!regolith.is_weathered());
        ferric += usize::from(!record.weathering_deposits().is_empty());
        checked += 1;
        // The field header carries the maturity, and the lid's entry the FeO_T the law read, read
        // at 24's rules (24's densities read an argon ice, which 23's refuse).
        let read = crate::planetary::testing::with_every_24_hold(|| {
            CoarseInputs::for_body(&body.record, body.planet.as_ref(), body.ctx)
        });
        if let Ok(inputs) = read {
            let header = inputs
                .weathering()
                .expect("a record with modifiers gives its header's");
            assert_same_bits(header.maturity, pass.maturity());
            let key = rebuilt.lid.substance().key();
            let entry = inputs
                .palette()
                .entries()
                .iter()
                .find(|entry| entry.substance == key)
                .unwrap_or_else(|| panic!("{id}: its lid's entry"));
            assert_eq!(
                entry.feo_mass_fraction,
                palette_feo_mass_fraction(rebuilt.lid),
                "{id}"
            );
            headers += 1;
        }
    }
    eprintln!(
        "rule (f) under the weathered area mix: {checked} records their published pass's to the \
         bit ({capped} on the stand-in's arm, {ferric} with ferric, {headers} field headers at \
         its maturity); the context's surface age \
         and heat flow against the figures' (K(P*)) at most {age_moved:.2e} and {heat_moved:.2e} \
         apart, relative; no maturity {none:?}"
    );
    assert_eq!(mismatches, Vec::<String>::new());
    assert!(checked > 100, "{checked}");
}

/// The same seed twice under the weathered area mix gives the same records and the same published
/// passes' maturities, to the bit (their renderings), over the first six sample systems.
#[test]
fn under_the_weathered_area_mix_the_same_seed_twice_gives_the_same_records() {
    let again = bodies_at_24(6, AreaMixWeathering::Weathered);
    let first: Vec<_> = weathered_bodies()
        .iter()
        .take(again.len())
        .map(|body| format!("{:?} {:?}", body.record, body.weathered))
        .collect();
    let twice: Vec<_> = again
        .iter()
        .map(|body| format!("{:?} {:?}", body.record, body.weathered))
        .collect();
    assert_eq!(first, twice);
    let weathered = again.iter().filter(|body| body.weathered.is_ok()).count();
    eprintln!(
        "the same seed twice: {} records, {weathered} weathered",
        again.len()
    );
    assert!(weathered > 0);
}

/// The weathered area mix's census (P14.T54.b.e), every 24 hold in force and the air's sections
/// under the batch, over the first [`MODIFIER_SAMPLE`] sample systems, printed for the record and
/// the bless note: the published passes' regoliths weathered, capped (`basalt_reduced`) or on the
/// stand-in's arm for want of a modelled `FeO_T`, an optics row or a density (each lid counted),
/// the passes that took no maturity by
/// why, the bodies with `ferric`, and against the same systems under the weathered area mix's hold
/// the largest moves of the Bond albedo and the equilibrium temperature, the bodies that moved
/// though they took no maturity, and every state flip by name. Under the hold every pass took
/// none.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one census: each count and move stated beside its basis"
)]
fn the_weathered_area_mix_s_census() {
    let (weathered, held) = (weathered_bodies(), held_bodies_at_24());
    assert_eq!(weathered.len(), held.len());
    assert!(
        held.iter()
            .all(|body| body.weathered == Err(NoMaturity::Held))
    );
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    let mut lids: BTreeMap<String, usize> = BTreeMap::new();
    let mut ferric = Vec::new();
    let mut flips = Vec::new();
    let mut moved_without = Vec::new();
    let mut largest_albedo = (0.0_f64, String::new());
    let mut largest_t_eq = (0.0_f64, String::new());
    // The airless weathered regoliths by maturity (below 10⁻³, to 0.5, above), and those that rose
    // above the cap's 0.11 with the brightest.
    let mut airless_by_maturity = [0_usize; 3];
    let mut above_the_cap = (0_usize, 0.0_f64, String::new());
    let lid_of = |body: &BatchBody| {
        body.surface().crust().ok().map_or_else(
            || "none".to_owned(),
            |crust| crust.lid_lithology().substance().key().as_str().to_owned(),
        )
    };
    for (body, before) in weathered.iter().zip(held) {
        let id = body.record.identity().id();
        assert_eq!(id, before.record.identity().id());
        let lid = lid_of(body);
        let outcome = match &body.weathered {
            Ok(pass) if pass.regolith().is_weathered() => "weathered".to_owned(),
            Ok(_) if CAPPED_ROWS.contains(&lid.as_str()) => "capped (basalt_reduced)".to_owned(),
            Ok(_) => "the stand-in's arm (FeO_T not modelled, no optics or no density)".to_owned(),
            Err(why) => format!("no maturity: {}", why.label()),
        };
        *outcomes.entry(outcome).or_default() += 1;
        if body.weathered.is_ok() {
            *lids.entry(lid.clone()).or_default() += 1;
        }
        if let Modelled::Ok(m) = body.surface().modifiers()
            && let Some(deposit) = m.weathering_deposits().first()
        {
            ferric.push(format!(
                "{id} ({lid}, {:?}) over {:.3}: {:.4} → {:.4}, held its lid {} ({:?})",
                body.published.2,
                deposit.area_fraction(),
                before.published.0,
                body.published.0,
                lid_of(before),
                before.published.2,
            ));
        }
        let (albedo, t_eq, state) = body.published;
        if let Ok(pass) = &body.weathered
            && pass.regolith().is_weathered()
            && state == SurfaceState::Airless
        {
            let m = pass.maturity();
            airless_by_maturity[usize::from(m >= 1e-3) + usize::from(m > 0.5)] += 1;
            let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
            if albedo > moon {
                above_the_cap.0 += 1;
                if albedo > above_the_cap.1 {
                    above_the_cap.1 = albedo;
                    above_the_cap.2 = format!("{id} ({lid}, m {m:.3e})");
                }
            }
        }
        let (was, t_was, state_was) = before.published;
        let describe = || {
            let pass = body.weathered.as_ref().ok();
            format!(
                "{id} ({lid}, {state:?}, m {:?}, k {:?}): {was:.4} → {albedo:.4}, T_eq \
                 {t_was:.2} → {t_eq:.2} K",
                pass.map(PassWeathered::maturity),
                pass.map(|pass| pass.weathering().weathering().value()),
            )
        };
        if (albedo - was).abs() > largest_albedo.0 {
            largest_albedo = ((albedo - was).abs(), describe());
        }
        if (t_eq - t_was).abs() > largest_t_eq.0 {
            largest_t_eq = ((t_eq - t_was).abs(), describe());
        }
        if body.weathered.is_err() && (albedo - was).abs() > 0.0 {
            moved_without.push(describe());
        }
        if state != state_was {
            flips.push(format!("{} {state_was:?} → {state:?}", describe()));
        }
    }
    let shown = |list: &[String]| {
        list.iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n    ")
    };
    eprintln!(
        "the weathered area mix's census over {} surfaces of {MODIFIER_SAMPLE} systems:\n  \
         outcomes {outcomes:#?}\n  lids of the passes with a maturity {lids:?}\n  ferric on {} \
         bodies:\n    {}\n  the largest albedo move {:.4}: {}\n  the largest T_eq move {:.3} K: \
         {}\n  moved with no maturity of their own: {}\n    {}\n  state flips: {}\n    {}\n  \
         airless weathered regoliths by maturity (below 10⁻³, to 0.5, above) {airless_by_maturity:?}, \
         {} above the cap's 0.11, the brightest {:.4}: {}",
        weathered.len(),
        ferric.len(),
        shown(&ferric),
        largest_albedo.0,
        largest_albedo.1,
        largest_t_eq.0,
        largest_t_eq.1,
        moved_without.len(),
        shown(&moved_without),
        flips.len(),
        shown(&flips),
        above_the_cap.0,
        above_the_cap.1,
        above_the_cap.2,
    );
    assert!(
        outcomes.get("weathered").copied().unwrap_or(0) > 0,
        "{outcomes:?}"
    );
    young_lava_census();
}

/// One of the young-lava census's sets: its name, and its batch bodies under the fix, as built and
/// under the weathered area mix's hold, body by body in the same order.
type CensusSet = (
    &'static str,
    &'static [BatchBody],
    &'static [BatchBody],
    &'static [BatchBody],
);

/// The key of the lid's row whose regolith the pass `pass` read.
fn lid_key(pass: &PassWeathered) -> String {
    pass.parts().lid.substance().key().as_str().to_owned()
}

/// The published pass of `body` where it took a maturity and its lid is airless (or under a tenuous
/// air) and its row silicate ([`capped_lid`]), outside [`CAPPED_ROWS`].
fn airless_silicate(body: &BatchBody) -> Option<&PassWeathered> {
    let state = body.published.2;
    body.weathered.as_ref().ok().filter(|pass| {
        matches!(state, SurfaceState::Airless | SurfaceState::Tenuous)
            && capped_lid(pass.parts().lid, state)
            && !CAPPED_ROWS.contains(&lid_key(pass).as_str())
    })
}

/// decision-p14-t54b-e-findings §A.5's 42002cb1ffc00009.0301 and 42002cae0000000a.0101, the young
/// lava worlds whose fresh `komatiite` rows P14.T54.b.e as built published (Bond 0.5217 and
/// 0.3417), and their Held `T_eq`, K (31.16 and 141.30).
const YOUNG_LAVA_WORLDS: [(&str, f64); 2] = [
    ("42002cb1ffc00009.0301", 31.16),
    ("42002cae0000000a.0101", 141.30),
];

/// The census's young lava (decision-p14-t54b-e-findings §A.5's items 1–6, the assert and the
/// stop condition; item 7, the table's Io, is `moons::regular`'s), over the first
/// [`MODIFIER_SAMPLE`] sample systems and the golden systems, every 24 hold in force and the air's
/// sections under the batch, each body under three views: the weathered area mix's hold
/// (`Held`), P14.T54.b.e as built (young lava weathered as any lid) and the fix (young lava capped):
///
/// 1. the airless silicate lids on the new arm ([`StandIn::UnweatheredLava`]), by lid, with their
///    surface ages and their Bond albedo and `T_eq` Held → as built → fixed;
/// 2. as built's airless (or tenuous) weathered regoliths below m = 10⁻³, split into the new arm
///    and still weathered, by lid, each with its surface age and k, and how many are the sample's
///    `Airless` ones, P14.T54.b.e's census's basis;
/// 3. [`YOUNG_LAVA_WORLDS`] back at the cap's 0.1100, their Bond albedo and `T_eq` Held's to the
///    bit;
/// 4. the airless silicate lids outside [`CAPPED_ROWS`] with a maturity by surface age, below
///    10⁴ yr, in [10⁴ yr, 1 Myr), [1, 10) Myr and above, how sensitive the result is to
///    [`REGOLITH_FORMATION_TIME`], each body in [10⁴ yr, 10 Myr) listed;
/// 5. the brightest airless silicate regolith still weathered, by its own Bond albedo about a Sun;
/// 6. the largest Bond albedo and `T_eq` moves against Held after the fix and against as built,
///    and every state flip against Held by name.
///
/// It asserts that no airless silicate lid outside [`CAPPED_ROWS`] whose surface is younger than
/// [`REGOLITH_FORMATION_TIME`] publishes a regolith above 0.11 (each on the new arm), and the
/// ruling's stop condition: no body moves against as built but one on the new arm, airless, under
/// 1 Myr.
#[expect(
    clippy::too_many_lines,
    reason = "one census of the ruling's items, each stated beside its basis"
)]
fn young_lava_census() {
    let sun = sun_bands();
    let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
    let formation = REGOLITH_FORMATION_TIME.value();
    let airless =
        |state: SurfaceState| matches!(state, SurfaceState::Airless | SurfaceState::Tenuous);
    let young = |body: &BatchBody| {
        body.weathered.as_ref().is_ok_and(|pass| {
            pass.regolith().bond(SurfaceState::Airless, &sun).stand_in
                == Some(StandIn::UnweatheredLava)
        })
    };
    let p_v = |body: &BatchBody| {
        body.record
            .photometry()
            .ok()
            .map_or(f64::NAN, |photometry| photometry.geometric_albedo().v)
    };
    let sets: [CensusSet; 2] = [
        (
            "sample",
            weathered_bodies(),
            as_built_bodies(),
            held_bodies_at_24(),
        ),
        (
            "golden",
            golden_weathered_bodies(),
            golden_as_built_bodies(),
            golden_held_bodies_at_24(),
        ),
    ];
    // Item 1: lid → (count, t_s range, lines).
    let mut on_arm: BTreeMap<String, (usize, f64, f64, Vec<String>)> = BTreeMap::new();
    // Item 2: (lid, arm) → lines.
    let mut below: BTreeMap<(String, &str), Vec<String>> = BTreeMap::new();
    // Item 4: below 10⁴ yr, [10⁴ yr, 1 Myr), [1, 10) Myr, above.
    let mut by_age = [0_usize; 4];
    let mut brightest = (0.0_f64, String::new());
    // Item 4's bodies within a factor of 100 below or 10 above the step.
    let mut near_the_step = Vec::new();
    // Item 2's basis as P14.T54.b.e's census took it: the sample's `Airless` bodies alone.
    let mut below_basis = 0_usize;
    let mut largest = [
        (0.0_f64, String::new()),
        (0.0, String::new()),
        (0.0, String::new()),
        (0.0, String::new()),
    ];
    let (mut flips, mut stop, mut above, mut surfaces) =
        (Vec::new(), Vec::new(), Vec::new(), 0_usize);
    for (set, fixed, built, held) in sets {
        assert_eq!(fixed.len(), built.len(), "{set}");
        assert_eq!(fixed.len(), held.len(), "{set}");
        surfaces += fixed.len();
        for ((now, was), before) in fixed.iter().zip(built).zip(held) {
            let id = now.record.identity().id();
            assert_eq!(id, was.record.identity().id());
            assert_eq!(id, before.record.identity().id());
            let ((a, t, state), (a_was, t_was, _), (a_held, t_held, state_held)) =
                (now.published, was.published, before.published);
            let pass = now.weathered.as_ref().ok();
            let (m, t_s, k) = pass.map_or((f64::NAN, f64::NAN, f64::NAN), |pass| {
                (
                    pass.maturity(),
                    pass.parts().surface_age.value(),
                    pass.weathering().weathering().value(),
                )
            });
            let lid = pass.map_or_else(|| "none".to_owned(), lid_key);
            let line = format!(
                "{set} {id} ({lid}, {state:?}, t_s {t_s:.3e} yr, m {m:.3e}, k {k:.3e}): A \
                 {a_held:.4} → {a_was:.4} → {a:.4}, T_eq {t_held:.2} → {t_was:.2} → {t:.2} K, p_V \
                 {:.4} → {:.4}",
                p_v(was),
                p_v(now)
            );
            let is_young = young(now);
            // The stop condition: the fix moves a body on the new arm alone.
            let moved = format!("{:?}", now.record) != format!("{:?}", was.record)
                || bits(a) != bits(a_was)
                || bits(t) != bits(t_was)
                || state != was.published.2;
            if moved && !(is_young && airless(state) && t_s < formation) {
                stop.push(line.clone());
            }
            if is_young {
                assert!(airless_silicate(now).is_some() && t_s < formation, "{line}");
                let entry =
                    on_arm
                        .entry(lid.clone())
                        .or_insert((0, f64::INFINITY, 0.0, Vec::new()));
                entry.0 += 1;
                entry.1 = entry.1.min(t_s);
                entry.2 = entry.2.max(t_s);
                entry.3.push(line.clone());
            }
            // The assert: none above 0.11, each on the new arm.
            if let Some(pass) = airless_silicate(now)
                && t_s < formation
            {
                let regolith = pass.regolith().bond(state, &sun);
                if regolith.bond > moon || regolith.stand_in != Some(StandIn::UnweatheredLava) {
                    above.push(format!("{line}: its regolith {regolith:?}"));
                }
            }
            if let Some(pass) = airless_silicate(now) {
                let bin = if t_s < 1e4 {
                    0
                } else if t_s < formation {
                    1
                } else if t_s < 10.0 * formation {
                    2
                } else {
                    3
                };
                by_age[bin] += 1;
                let own = pass.regolith().bond(state, &sun).bond;
                if bin == 1 || bin == 2 {
                    near_the_step.push(format!("{line}; its regolith about a Sun {own:.4}"));
                }
                if pass.regolith().is_weathered() && own > brightest.0 {
                    brightest = (own, format!("{line}; its regolith about a Sun {own:.4}"));
                }
            }
            // Item 2: as built's airless weathered regoliths below m = 10⁻³.
            if let Ok(built_pass) = &was.weathered
                && built_pass.regolith().is_weathered()
                && airless(was.published.2)
                && built_pass.maturity() < 1e-3
            {
                let arm = if is_young {
                    "the new arm"
                } else {
                    "still weathered"
                };
                let basis = set == "sample" && was.published.2 == SurfaceState::Airless;
                below_basis += usize::from(basis);
                below
                    .entry((lid_key(built_pass), arm))
                    .or_default()
                    .push(format!(
                        "{set} {id} ({:?}) t_s {t_s:.3e} yr, k {k:.3e}",
                        was.published.2
                    ));
            }
            for (slot, delta) in [
                (0, (a - a_held).abs()),
                (1, (t - t_held).abs()),
                (2, (a - a_was).abs()),
                (3, (t - t_was).abs()),
            ] {
                if delta > largest[slot].0 {
                    largest[slot] = (delta, line.clone());
                }
            }
            if state != state_held {
                flips.push(format!("{line}: {state_held:?} → {state:?}"));
            }
        }
    }
    // Item 3: the two named worlds, back at the cap and at their Held T_eq to the bit.
    let mut named = Vec::new();
    for (name, t_eq) in YOUNG_LAVA_WORLDS {
        let find = |bodies: &'static [BatchBody]| {
            bodies
                .iter()
                .find(|body| body.record.identity().id().to_string() == name)
                .unwrap_or_else(|| panic!("{name} is in the sample"))
        };
        let (now, was, before) = (
            find(weathered_bodies()),
            find(as_built_bodies()),
            find(held_bodies_at_24()),
        );
        assert!(young(now), "{name}");
        assert_same_bits(now.published.0, moon);
        assert_same_bits(now.published.0, before.published.0);
        assert_same_bits(now.published.1, before.published.1);
        named.push(format!(
            "{name}: A {:.4} → as built {:.4} → fixed {:.4}, T_eq {:.2} → {:.2} → {:.2} K (the \
             fix's A and T_eq Held's to the bit; the ruling's Held T_eq {t_eq:.2} K), p_V as built \
             {:.4} → fixed {:.4}",
            before.published.0,
            was.published.0,
            now.published.0,
            before.published.1,
            was.published.1,
            now.published.1,
            p_v(was),
            p_v(now)
        ));
    }
    let arm_lines: Vec<String> = on_arm
        .iter()
        .map(|(lid, (count, low, high, lines))| {
            format!(
                "{lid}: {count}, t_s {low:.3e}–{high:.3e} yr\n      {}",
                lines.join("\n      ")
            )
        })
        .collect();
    let below_lines: Vec<String> = below
        .iter()
        .map(|((lid, arm), lines)| {
            format!(
                "{lid}, {arm}: {}\n      {}",
                lines.len(),
                lines.join("\n      ")
            )
        })
        .collect();
    let below_count: usize = below.values().map(Vec::len).sum();
    eprintln!(
        "young lava (decision-p14-t54b-e-findings §A.5) over {surfaces} surfaces of the first \
         {MODIFIER_SAMPLE} sample systems and the golden systems, each A Held → as built → fixed:\n  \
         (1) the new arm, airless silicate lids under {formation:.0e} yr: {}\n    {}\n  (2) as \
         built's airless or tenuous weathered regoliths below m = 1e-3: {below_count} ({below_basis} \
         of them the sample's Airless, P14.T54.b.e's census's basis)\n    {}\n  (3) {}\n  (4) the \
         airless silicate lids outside CAPPED_ROWS with a maturity by t_s (below 1e4 yr, [1e4 yr, \
         1 Myr), [1, 10) Myr, above) {by_age:?}; within [1e4 yr, 10 Myr):\n    {}\n  (5) the \
         brightest airless silicate regolith still weathered, about a Sun, {:.4}: {}\n  (6) the largest moves against Held: A {:.4}: {}\n      T_eq {:.3} K: {}\n    \
         against as built: A {:.4}: {}\n      T_eq {:.3} K: {}\n    state flips against Held: \
         {}\n      {}\n  the stop condition's moves elsewhere: {}; above 0.11 under 1 Myr: {}",
        on_arm.values().map(|entry| entry.0).sum::<usize>(),
        arm_lines.join("\n    "),
        below_lines.join("\n    "),
        named.join("\n      "),
        near_the_step.join("\n    "),
        brightest.0,
        brightest.1,
        largest[0].0,
        largest[0].1,
        largest[1].0,
        largest[1].1,
        largest[2].0,
        largest[2].1,
        largest[3].0,
        largest[3].1,
        flips.len(),
        flips.join("\n      "),
        stop.len(),
        above.len(),
    );
    assert_eq!(
        stop,
        Vec::<String>::new(),
        "the fix moved a body off the new arm"
    );
    assert_eq!(
        above,
        Vec::<String>::new(),
        "an airless silicate lid under 1 Myr above 0.11"
    );
    assert!(!on_arm.is_empty());
}
