//! A small body's taxonomic class on the record (P14.T51.e's wiring; decision-p14-t51e-contract
//! §1.4 and decision-p14-t51e-qs §3.2): held at 23, so that every record is 23's; the reference wind
//! term and the anchor by T51.d's own rule; the unshielded wind term against the shielded one, and a
//! moon inside its planet's magnetopause; and, with every hold at 24 in force
//! (`planetary::testing::with_every_24_hold`, the air's sections' batch beside it through
//! `PlanetarySystem::with_record_rules`), the census of the plan-14 sample's classes by kind, the
//! Q share of its ordinary chondrites, the belt's zones, its V-types, its Kuiper-belt colours, its
//! cometary moons, its bodies without a crust and its planets, two runs equal, and `body_at`
//! giving the snapshot's class in any order.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::planetary::hooks::modifiers::PlanetMagnetosphere;
use crate::planetary::hooks::small_bodies::{Q_TYPE_WEATHERING_TIME, VENEER_SHARE};
use crate::planetary::record::{BodyRecord, Section, SystemSnapshot};
use crate::planetary::system::RecordRules;
use crate::planetary::system::air::AirRules;
use crate::planetary::system::tests::{SEED, golden_systems, sample};
use crate::planetary::testing::with_every_24_hold;
use crate::planetary::{BodyIndex, SystemContext, generate};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::{AstronomicalUnits, EarthFluxes};

/// How many of the sample's systems the census generates under every hold at 24.
const CENSUS_SAMPLE: usize = 400;

/// The ruling's `w_ref`, to its table's five figures (decision-p14-t51e-qs, "Computed here").
const RULING_REFERENCE_WIND_TERM: f64 = 0.159_98;

/// `w_ref`'s bits as T51.d's rule gives them at P14.T51.e's wiring, 0.159 977 687 244 190 4, so
/// that a change to the rule or to the reference body, which every record's class reads, shows.
const REFERENCE_WIND_TERM_BITS: u64 = 0x3fc4_7a26_1b66_d98d;

/// Nesvorný et al. 2010's anchor against the ruling's: 1 Myr at 1 au against 1 Myr at 2.5 au,
/// (2.5 ÷ 1)² (decision-p14-t51e-qs §3.1).
const NESVORNY_FACTOR: f64 = 6.25;

/// The inner main belt's outer edge, 2.5 au, and the outer belt's inner edge, 2.82 au: `DeMeo` and
/// Carry 2013's zones (Icarus 226, 723: inner 2.1–2.5 au, outer 2.82–3.3 au; the outer bounds *from
/// memory*), applied to every host's bodies by their heliocentric distance and by the Sun's
/// distance of their flux, labelled.
const INNER_BELT_EDGE_AU: f64 = 2.5;
/// See [`INNER_BELT_EDGE_AU`].
const OUTER_BELT_EDGE_AU: f64 = 2.82;

/// The record's rules at 24 for a system generated under [`with_every_24_hold`]: the thread's
/// (the cold traps' and the small bodies' batches among them), with the air's sections' batch,
/// which only [`PlanetarySystem::with_record_rules`] reaches, so that the modifiers are modelled.
fn every_record_batch() -> RecordRules {
    RecordRules {
        air: AirRules::Batch,
        ..RecordRules::in_force()
    }
}

/// The modifiers' inputs of an airless body with no field orbiting `primary` on `orbit` about
/// `ModifierHost::sun()`, the rest as [`reference_inputs`]'s.
fn inputs_at(none: &Condensates, orbit: HeliocentricOrbit, primary: Primary) -> ModifierInputs<'_> {
    ModifierInputs {
        orbit,
        primary,
        ..reference_inputs(none)
    }
}

/// A heliocentric orbit of semi-major axis `a_au` au and eccentricity `e`.
fn orbit_at(a_au: f64, e: f64) -> HeliocentricOrbit {
    HeliocentricOrbit {
        semi_major_axis: Metres::from(AstronomicalUnits::new(a_au)),
        eccentricity: e,
    }
}

/// The modifiers T51.d derives from `inputs`.
fn derived(inputs: &ModifierInputs<'_>) -> SurfaceModifiers {
    SurfaceModifiers::derive(inputs).unwrap()
}

/// The wind's clock's time `τ_Q`, Julian years, of a body of wind term `w0` against `w_ref`.
fn q_type_time(w0: f64, reference: f64) -> f64 {
    let wind = WindTerms::new(w0, reference).unwrap();
    WeatheringClocks::new(Years::ZERO, wind, 0.0, AirShield::Open)
        .unwrap()
        .q_type_time()
        .value()
}

/// The small bodies' types are held until P14.T54.b's bump (P14.T54.b.l; decision-p14-t51e-contract
/// §1.4 and §4): below it the rules in force are the held ones, under which every golden system's
/// record, at both times its golden pins, holds its taxonomic class not modelled, and the held arm
/// computes nothing (no reference wind term, and a body the batch classes is not modelled).
#[test]
fn the_small_body_types_are_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < SMALL_BODY_TYPES_VERSION,
        "the 23 → 24 bump (P14.T54.b.l) puts every planet's, member's and moon's taxonomic class \
         on the record (P14.T51.e, a P14.T54.b.j rider): `Ok(None)` for a planet and an unclassed \
         body, not modelled where the record lacks what its rule reads. Delete \
         `SMALL_BODY_TYPES_VERSION`, `SmallBodyRules` with its `Held` arm, `SmallBodyTypes::Held`, \
         `with_small_body_rules` and its arm in `planetary::testing::with_every_24_hold`, \
         `RecordRules::small_bodies` (and `RecordRules` with `PlanetarySystem::with_record_rules` \
         once the other holds go too); the source scan \
         `small_bodies_nothing_on_the_record_reads_the_types_yet` with its `HELD` list, \
         `names_its_hold` and `the_small_bodies_hold_check_reads_the_code_alone`, and \
         `system/small_bodies.rs`' and its tests' `HELD` lines in \
         `modifiers_nothing_on_the_record_reads_the_modifiers_yet`; \
         `the_small_bodies_switch_sets_its_rules_back` and this test. P14.T54.b.j's \
         conversion then sends the member: absent for not modelled, `null` for none \
         (decision-p14-t51e-contract §4). If the rider misses P14.T54.b.i's commit, renumber the \
         hold to 25 instead"
    );
    assert_eq!(SmallBodyRules::in_force(), SmallBodyRules::Held);
    assert_eq!(RecordRules::in_force().small_bodies, SmallBodyRules::Held);
    assert_eq!(
        SmallBodyTypes::under(SmallBodyRules::Held),
        SmallBodyTypes::Held
    );
    let mut surfaces = 0_usize;
    for (name, ctx, system) in golden_systems() {
        for t in [UniverseTime::EPOCH, ClockWindow::END] {
            for record in system.snapshot_at(ctx, t).bodies() {
                let Section::Ok(surface) = record.surface() else {
                    continue;
                };
                assert_eq!(
                    surface.taxonomic_class(),
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

/// The switch sets its rules on its thread and back after, and a system generated under it carries
/// them in its record rules.
#[test]
fn the_small_bodies_switch_sets_its_rules_back() {
    assert_eq!(SmallBodyRules::in_force(), SmallBodyRules::Held);
    let inside = with_small_body_rules(SmallBodyRules::Batch, || {
        (
            SmallBodyRules::in_force(),
            RecordRules::in_force().small_bodies,
            with_small_body_rules(SmallBodyRules::Held, SmallBodyRules::in_force),
            SmallBodyRules::in_force(),
        )
    });
    assert_eq!(
        inside,
        (
            SmallBodyRules::Batch,
            SmallBodyRules::Batch,
            SmallBodyRules::Held,
            SmallBodyRules::Batch
        )
    );
    assert_eq!(SmallBodyRules::in_force(), SmallBodyRules::Held);
    assert_eq!(
        with_every_24_hold(SmallBodyRules::in_force),
        SmallBodyRules::Batch
    );
}

/// `w_ref` by T51.d's own rule on the stated reference body (a circular 2.5 au orbit about
/// `ModifierHost::sun()`, no field, no air) is the ruling's 0.15998 to its five figures and its
/// pinned bits ([`REFERENCE_WIND_TERM_BITS`]), the same bits through the query's types; and its
/// `τ_Q` is `T_Q` to 10⁻¹² relative (decision-p14-t51e-qs §3.2).
#[test]
fn the_reference_wind_term_is_the_ruling_s() {
    let w_ref = reference_wind_term();
    assert!(
        (w_ref - RULING_REFERENCE_WIND_TERM).abs() <= 0.5e-5,
        "w_ref {w_ref} against the ruling's {RULING_REFERENCE_WIND_TERM}"
    );
    assert_same_bits(w_ref, f64::from_bits(REFERENCE_WIND_TERM_BITS));
    let SmallBodyTypes::Batch {
        reference_wind_term: shared,
    } = SmallBodyTypes::under(SmallBodyRules::Batch)
    else {
        panic!("the batch's types carry the reference");
    };
    assert_same_bits(shared, w_ref);
    let none = Condensates::default();
    let reference = derived(&reference_inputs(&none));
    let t_q = Q_TYPE_WEATHERING_TIME.value();
    let tau = q_type_time(reference.unshielded_wind_term(), w_ref);
    assert!(((tau - t_q) / t_q).abs() < 1e-12, "τ_Q {tau} yr");
    // The stated values the wind term does not read leave it unmoved.
    let other = ModifierInputs {
        gravity: MetresPerSecondSquared::new(9.8),
        surface_age: Years::new(4.567e9),
        surface_temperature: Kelvin::new(250.0),
        heat_flow: WattsPerSquareMetre::new(0.05),
        lid: CrustLithology::Basalt.substance(),
        ..reference_inputs(&none)
    };
    assert_same_bits(derived(&other).unshielded_wind_term(), w_ref);
}

/// About one host, `τ_Q` by T51.d's own rule scales as a² √(1 − e²) (decision-p14-t51e-qs §3.2): a
/// 1 au circular twin's is 0.16 of the anchor's, an orbit of e = 0.6 at 2.5 au 0.8 of it and one at
/// 1 au 0.128, and one of e = 0.05 at 5.2 au 4.3264 √0.9975 of it, each to 10⁻¹² relative; and on
/// each, an airless body with no field orbiting the star, whose shielded share is 1, has its
/// unshielded wind term equal to `weathering().wind()` to the bit.
#[test]
fn the_q_type_time_scales_as_a_squared_root_one_minus_e_squared_by_t51d_s_rule() {
    let none = Condensates::default();
    let w_ref = reference_wind_term();
    let t_q = Q_TYPE_WEATHERING_TIME.value();
    for (a, e, share) in [
        (2.5, 0.0, 1.0),
        (1.0, 0.0, 0.16),
        (2.5, 0.6, 0.8),
        (1.0, 0.6, 0.128),
        (5.2, 0.05, 4.3264 * 0.9975_f64.sqrt()),
    ] {
        let modifiers = derived(&inputs_at(&none, orbit_at(a, e), Primary::Star));
        let w0 = modifiers.unshielded_wind_term();
        assert_same_bits(w0, modifiers.weathering().wind());
        let tau = q_type_time(w0, w_ref);
        let expected = share * t_q;
        assert!(
            ((tau - expected) / expected).abs() < 1e-12,
            "a {a} au, e {e}: τ_Q {tau} yr against {expected}"
        );
    }
}

/// A moon inside its planet's dayside magnetopause (a Jupiter's 30 µT stand-in field at Europa's
/// 671,000 km, 5.2 au from the Sun) has T51.d's wind term 0, shielded, and its unshielded term its
/// planet's orbit's, the same bits as a body on that orbit about the star, and positive: the
/// planet's plasma's labelled stand-in (decision-p14-t51e-qs §1.2). So an old ordinary chondrite
/// there is S by the wind's clock, where its shielded term would leave it to the regolith's. Under
/// air above the ion floor the unshielded term is unmoved too, and the air's shield is read
/// beside it.
#[test]
fn a_moon_inside_its_planet_s_magnetopause_takes_its_planet_s_orbit_s_wind() {
    let none = Condensates::default();
    let orbit = orbit_at(5.2, 0.048);
    let jupiter = PlanetMagnetosphere {
        radius: Metres::new(6.9911e7),
        field: MagneticField::Strong,
        semi_major_axis: Metres::new(6.711e8),
    };
    let moon = derived(&inputs_at(&none, orbit, Primary::Planet(jupiter)));
    let free = derived(&inputs_at(&none, orbit, Primary::Star));
    assert_same_bits(moon.weathering().wind(), 0.0);
    assert!(moon.unshielded_wind_term() > 0.0);
    assert_same_bits(moon.unshielded_wind_term(), free.weathering().wind());
    assert_same_bits(moon.unshielded_wind_term(), free.unshielded_wind_term());
    let w_ref = reference_wind_term();
    let old = Years::new(4.567e9);
    let class = |w0: f64| {
        let clocks = WeatheringClocks::new(
            old,
            WindTerms::new(w0, w_ref).unwrap(),
            0.1,
            AirShield::Open,
        )
        .unwrap();
        let bare = Veneer::new(0.0, 0.0).unwrap();
        TaxonomicClass::of(&TaxonomyInputs::new(
            TaxonomyBodyKind::OtherMoon,
            Some(CrustLithology::ChondriteLl),
            Some(bare),
            Some(clocks),
        ))
    };
    assert_eq!(
        class(moon.unshielded_wind_term()),
        Ok(Some(TaxonomicClass::S))
    );
    assert_eq!(class(moon.weathering().wind()), Ok(Some(TaxonomicClass::Q)));
    let aired = derived(&ModifierInputs {
        surface_pressure: Pascals::new(1.0),
        ..inputs_at(&none, orbit, Primary::Star)
    });
    assert_same_bits(aired.weathering().wind(), 0.0);
    assert_same_bits(aired.unshielded_wind_term(), free.unshielded_wind_term());
}

/// The air's shield reads the modifiers' own comparison: open at and below a column p ÷ g of
/// `ION_STOPPING_COLUMN`, above it beyond, and above it for a column that is not a number.
#[test]
fn the_air_shield_is_the_modifiers_ion_floor() {
    let g = MetresPerSecondSquared::new(2.0);
    let floor = ION_STOPPING_COLUMN.value();
    let at = |column: f64| air_shield(Pascals::new(column * 2.0), g);
    assert_eq!(at(0.0), AirShield::Open);
    assert_eq!(at(floor), AirShield::Open);
    assert_eq!(at(floor.next_up()), AirShield::AboveIonFloor);
    assert_eq!(at(1.0), AirShield::AboveIonFloor);
    assert_eq!(
        air_shield(Pascals::ZERO, MetresPerSecondSquared::ZERO),
        AirShield::AboveIonFloor
    );
}

/// One body of the census: its record's class with what the rule read of it, and its surface.
#[derive(Debug, Clone)]
struct CensusBody {
    /// Its system's place in the census.
    system: usize,
    /// Its index.
    index: BodyIndex,
    /// Its ID, for the lists.
    id: String,
    /// Its kind.
    kind: TaxonomyBodyKind,
    /// Its parent, for a moon.
    parent: Option<BodyIndex>,
    /// Its record's class.
    class: Modelled<Option<TaxonomicClass>>,
    /// The class at Nesvorný's anchor ([`NESVORNY_FACTOR`] `T_Q`).
    slow_class: Modelled<Option<TaxonomicClass>>,
    /// Its lid, where its crust is modelled.
    lid: Option<CrustLithology>,
    /// Its veneer's `s_t` and `s_i`, where its modifiers are modelled.
    veneer: Option<(f64, f64)>,
    /// Its modifiers' unshielded and shielded wind terms and maturity.
    wind: Option<(f64, f64, f64)>,
    /// Its heliocentric semi-major axis, a moon's planet's, au.
    distance_au: f64,
    /// The distance from the Sun at which it would receive its orbit-averaged flux, au: 1 ÷ √(S ÷
    /// S⊕) of its conditions' stellar flux.
    equivalent_au: f64,
    /// Its bulk section's radius, km.
    radius_km: f64,
    /// Its bulk section's surface gravity.
    gravity: Option<MetresPerSecondSquared>,
    /// Its surface.
    surface: Surface,
}

/// The census: the first [`CENSUS_SAMPLE`] systems of the sample, each generated and read under
/// every hold at 24 with the air's sections' batch, at the epoch.
struct Census {
    /// Each system with its context.
    systems: Vec<(SystemContext, PlanetarySystem)>,
    /// Each body with a surface section, in the systems' order and index order.
    bodies: Vec<CensusBody>,
}

/// The census, built once (each system generated afresh inside the switches).
fn census() -> &'static Census {
    static CENSUS: OnceLock<Census> = OnceLock::new();
    CENSUS.get_or_init(|| {
        with_every_24_hold(|| {
            let systems: Vec<(SystemContext, PlanetarySystem)> = sample()
                .iter()
                .take(CENSUS_SAMPLE)
                .map(|ctx| {
                    let system = generate(SEED, ctx).with_record_rules(every_record_batch());
                    (ctx.clone(), system)
                })
                .collect();
            let mut bodies = Vec::new();
            for (at, (ctx, system)) in systems.iter().enumerate() {
                let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
                census_of(at, system, &snapshot, &mut bodies);
            }
            Census { systems, bodies }
        })
    })
}

/// The heliocentric semi-major axis, au, of `record` of `snapshot`: its own orbit's, a moon's
/// planet's.
fn heliocentric_au(
    snapshot: &SystemSnapshot,
    record: &BodyRecord,
    parent: Option<BodyIndex>,
) -> f64 {
    let of = |record: &BodyRecord| match record.orbit() {
        Section::Ok(orbit) => AstronomicalUnits::from(orbit.elements().semi_major_axis()).value(),
        Section::NotResolved | Section::NotModelled | Section::NotApplicable => f64::NAN,
    };
    match parent {
        None => of(record),
        Some(parent) => snapshot
            .bodies()
            .iter()
            .find(|planet| planet.index() == parent)
            .map_or(f64::NAN, of),
    }
}

/// Adds every body of `snapshot` of `system` (the census's `at`) with a surface section to
/// `bodies`.
fn census_of(
    at: usize,
    system: &PlanetarySystem,
    snapshot: &SystemSnapshot,
    bodies: &mut Vec<CensusBody>,
) {
    let w_ref = reference_wind_term();
    for record in snapshot.bodies() {
        let Section::Ok(surface) = record.surface() else {
            continue;
        };
        let body = system
            .body(record.index())
            .expect("a record with a surface is a body's");
        let kind = taxonomy_kind(system, body).expect("a body with a surface is not a ring");
        let parent = match body.host {
            OrbitHost::Body(parent) => Some(parent),
            OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => None,
        };
        let (gravity, radius_km) = match record.bulk() {
            Section::Ok(bulk) => (
                Some(bulk.surface_gravity()),
                Metres::from(bulk.radius()).value() / 1e3,
            ),
            Section::NotResolved | Section::NotModelled | Section::NotApplicable => {
                (None, f64::NAN)
            }
        };
        let flux = EarthFluxes::from(surface.conditions().stellar_flux()).value();
        let lid = match surface.crust() {
            Modelled::Ok(crust) => {
                CrustLithology::of_key(crust.lid_lithology().substance().key().as_str())
            }
            Modelled::NotModelled => None,
        };
        let modifiers = match surface.modifiers() {
            Modelled::Ok(modifiers) => Some(modifiers),
            Modelled::NotModelled => None,
        };
        let veneer = modifiers.map(|m| {
            let v = veneer_of(m, surface.condensates()).expect("a record's veneer is in range");
            (v.tholin_share(), v.ice_share())
        });
        let wind = modifiers.map(|m| {
            (
                m.unshielded_wind_term(),
                m.weathering().wind(),
                m.maturity(),
            )
        });
        bodies.push(CensusBody {
            system: at,
            index: record.index(),
            id: record.identity().id().to_string(),
            kind,
            parent,
            class: surface.taxonomic_class(),
            slow_class: surface_class(kind, w_ref * NESVORNY_FACTOR, gravity, surface),
            lid,
            veneer,
            wind,
            distance_au: heliocentric_au(snapshot, record, parent),
            equivalent_au: 1.0 / flux.sqrt(),
            radius_km,
            gravity,
            surface: surface.clone(),
        });
    }
}

/// Whether `class` is of the S complex as `DeMeo` and Carry 2013 group it, S and Q together (§6.1).
fn s_complex(class: TaxonomicClass) -> bool {
    matches!(
        class,
        TaxonomicClass::S
            | TaxonomicClass::Q
            | TaxonomicClass::Sa
            | TaxonomicClass::Sq
            | TaxonomicClass::Sr
            | TaxonomicClass::Sv
    )
}

/// Whether `class` is of the C complex, or a low-albedo primitive class beside it (B, C, Cb, Cg,
/// Cgh, Ch; and P and D, the outer belt's dark types).
fn c_type(class: TaxonomicClass) -> bool {
    matches!(
        class,
        TaxonomicClass::B
            | TaxonomicClass::C
            | TaxonomicClass::Cb
            | TaxonomicClass::Cg
            | TaxonomicClass::Cgh
            | TaxonomicClass::Ch
            | TaxonomicClass::P
            | TaxonomicClass::D
    )
}

/// A class as the census prints it.
fn name_of(class: Modelled<Option<TaxonomicClass>>) -> &'static str {
    match class {
        Modelled::Ok(Some(class)) => class.name(),
        Modelled::Ok(None) => "none",
        Modelled::NotModelled => "not modelled",
    }
}

/// Every record's class is the rule's over its own published members (`surface_class` again, at
/// the query's `w_ref`); the census counts the classes by kind and prints them (P14.T51.e's census
/// test, decision-t24i-airless-state D3.3's helper); every planet is `Ok(None)`; a member or moon
/// not of a Kuiper-like belt whose record has no crust is not modelled; every body whose modifiers
/// are not modelled is not modelled either, but a planet and a member or moon whose lid is of the
/// closed not-classed set, which are `Ok(None)`; and the census holds each of the classes S, V, P,
/// C, K, Xc, red and neutral at least once. Q, D, Ch and A are not asserted: the sample may hold
/// none (Q is expected to be rare, D needs a majority tholin cover on a classed lid, Ch a CM lid,
/// and A an ultramafic one), and they are printed.
#[test]
fn under_every_24_hold_the_census_counts_the_classes_by_kind() {
    use TaxonomicClass as T;
    let census = census();
    let w_ref = reference_wind_term();
    let mut counts: BTreeMap<(TaxonomyBodyKind, &str), usize> = BTreeMap::new();
    let mut seen: BTreeSet<TaxonomicClass> = BTreeSet::new();
    let (mut no_crust, mut no_modifiers) = (0_usize, 0_usize);
    for body in &census.bodies {
        assert_eq!(
            body.class,
            surface_class(body.kind, w_ref, body.gravity, &body.surface),
            "{}",
            body.id
        );
        *counts.entry((body.kind, name_of(body.class))).or_default() += 1;
        if let Modelled::Ok(Some(class)) = body.class {
            seen.insert(class);
        }
        match body.kind {
            TaxonomyBodyKind::Planet => assert_eq!(body.class, Modelled::Ok(None), "{}", body.id),
            TaxonomyBodyKind::AsteroidBeltMember | TaxonomyBodyKind::OtherMoon => {
                if body.lid.is_none() {
                    no_crust += 1;
                    assert_eq!(body.class, Modelled::NotModelled, "{}", body.id);
                }
            }
            TaxonomyBodyKind::KuiperBody => {}
        }
        if body.wind.is_none() && body.kind != TaxonomyBodyKind::Planet {
            no_modifiers += 1;
            // With no veneer and no clocks, the rule gives none only for a lid of the closed
            // not-classed set; every other body reads what it was not given.
            let unclassed =
                TaxonomicClass::of(&TaxonomyInputs::new(body.kind, body.lid, None, None))
                    == Ok(None);
            let expected = if unclassed {
                Modelled::Ok(None)
            } else {
                Modelled::NotModelled
            };
            assert_eq!(body.class, expected, "{}: no modifiers", body.id);
        }
    }
    println!(
        "P14.T51.e census: {} systems, {} bodies with a surface; {no_crust} members and moons \
         with no crust, {no_modifiers} with no modifiers",
        census.systems.len(),
        census.bodies.len()
    );
    for ((kind, class), count) in &counts {
        println!("  {:<20} {class:<14} {count}", format!("{kind:?}"));
    }
    for class in [T::S, T::V, T::P, T::C, T::K, T::Xc, T::Red, T::Neutral] {
        assert!(
            seen.contains(&class),
            "the census holds no {} body",
            class.name()
        );
    }
    assert!(
        counts.keys().any(|&(kind, class)| {
            kind == TaxonomyBodyKind::AsteroidBeltMember && class != "not modelled"
        }),
        "the census classes some belt member"
    );
}

/// The Q share (decision-p14-t51e-qs §3.2): over the census, the ordinary chondrites' Q bodies are
/// at most 0.01 of their S and Q together (Carry et al. 2016, Icarus 268, 340: Q-types "rare among
/// main-belt asteroids"), each Q body listed with its host, distance, `t_x` and `τ_Q`, and the
/// share printed again at Nesvorný et al. 2010's anchor, 6.25 `T_Q`.
#[test]
fn under_every_24_hold_ordinary_chondrites_are_rarely_q() {
    let census = census();
    let w_ref = reference_wind_term();
    let ordinary = |body: &&CensusBody| {
        matches!(
            body.lid,
            Some(
                CrustLithology::ChondriteH
                    | CrustLithology::ChondriteL
                    | CrustLithology::ChondriteLl
            )
        )
    };
    let share = |class: fn(&CensusBody) -> Modelled<Option<TaxonomicClass>>| {
        let (mut q, mut s) = (0_usize, 0_usize);
        for body in census.bodies.iter().filter(ordinary) {
            match class(body) {
                Modelled::Ok(Some(TaxonomicClass::Q)) => q += 1,
                Modelled::Ok(Some(TaxonomicClass::S)) => s += 1,
                Modelled::Ok(_) | Modelled::NotModelled => {}
            }
        }
        (q, s)
    };
    let (q, s) = share(|body| body.class);
    let (slow_q, slow_s) = share(|body| body.slow_class);
    let ratio = |q: usize, s: usize| {
        let all = q + s;
        if all == 0 {
            0.0
        } else {
            f64::from(u32::try_from(q).unwrap()) / f64::from(u32::try_from(all).unwrap())
        }
    };
    println!(
        "P14.T51.e Q share: {q} Q of {} S and Q ({:.4}); at 6.25 T_Q {slow_q} of {} ({:.4})",
        q + s,
        ratio(q, s),
        slow_q + slow_s,
        ratio(slow_q, slow_s)
    );
    for body in census.bodies.iter().filter(ordinary) {
        let Some((w0, _, m)) = body.wind else {
            continue;
        };
        if body.class == Modelled::Ok(Some(TaxonomicClass::Q))
            || body.slow_class == Modelled::Ok(Some(TaxonomicClass::Q))
        {
            let ctx = &census.systems[body.system].0;
            println!(
                "  Q: {} ({:?}) about a {:.3} M☉ host of {:.3} Gyr at {:.3} au (flux's {:.3} au): \
                 t_x {:.4e} yr, τ_Q {:.4e} yr, m {m:.4}, at 6.25 T_Q {}",
                body.id,
                body.kind,
                ctx.stars()[0].initial_mass().value(),
                ctx.age_at_epoch().value() / 1e9,
                body.distance_au,
                body.equivalent_au,
                body.surface.figures().surface_age().value(),
                q_type_time(w0, w_ref),
                name_of(body.slow_class)
            );
        }
    }
    println!(
        "  ordinary chondrites: {} (S {s}, Q {q})",
        census.bodies.iter().filter(ordinary).count()
    );
    for body in census.bodies.iter().filter(ordinary) {
        let clocks = body.wind.map_or_else(String::new, |(w0, _, m)| {
            let t_x = body.surface.figures().surface_age().value();
            format!("t_x ÷ τ_Q {:.3e}, m {m:.3}", t_x / q_type_time(w0, w_ref))
        });
        println!(
            "  {} ({:?}, {}, {:.0} km) {} at {:.2} au, flux's {:.2} au: {clocks}",
            body.id,
            body.kind,
            body.lid.map_or("none", CrustLithology::key),
            body.radius_km,
            name_of(body.class),
            body.distance_au,
            body.equivalent_au
        );
    }
    assert!(
        q + s > 0,
        "the census holds no ordinary chondrite classed S or Q"
    );
    assert!(
        ratio(q, s) <= 0.01,
        "the Q share {q} of {} is over Carry et al. 2016's band",
        q + s
    );
}

/// The belt's zones, qualitatively (P14.T51.e's test, `DeMeo` and Carry 2014's proportion, the S
/// complex being S and Q together as `DeMeo` and Carry 2013 group them, §6.1): the census's
/// asteroid-belt members inside [`INNER_BELT_EDGE_AU`] and beyond [`OUTER_BELT_EDGE_AU`], by their
/// heliocentric distance and by the Sun's distance of their flux, are printed by class and lid
/// with their S-complex and C-type shares and their radii; the inner zone by flux holds more of
/// the S complex than of the C type; and a V-type member exists.
///
/// The outer zone is printed, not asserted: the members, at least 400 km across, are
/// differentiated (P14.T51.c), so a rocky one takes a basaltic lid (V) and an icy one a frozen
/// ocean's `H2O` lid, which the closed not-classed set leaves unclassed; the C complex's lids
/// (`phyllosilicate`, `chondrite_ci`, `chondrite_cm`) come from bodies under 200 km, the moons.
#[test]
fn under_every_24_hold_the_belt_s_zones_are_printed_and_a_member_is_v_type() {
    let census = census();
    let members: Vec<&CensusBody> = census
        .bodies
        .iter()
        .filter(|body| body.kind == TaxonomyBodyKind::AsteroidBeltMember)
        .collect();
    let zone = |name: &str, distance: fn(&CensusBody) -> f64, inside: fn(f64) -> bool| {
        let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
        let mut lids: BTreeMap<&str, usize> = BTreeMap::new();
        let (mut s, mut c, mut v, mut all) = (0_usize, 0_usize, 0_usize, 0_usize);
        let (mut smallest, mut largest) = (f64::INFINITY, 0.0_f64);
        for body in members.iter().filter(|body| inside(distance(body))) {
            *classes.entry(name_of(body.class)).or_default() += 1;
            *lids
                .entry(body.lid.map_or("no crust", CrustLithology::key))
                .or_default() += 1;
            smallest = smallest.min(body.radius_km);
            largest = largest.max(body.radius_km);
            if let Modelled::Ok(Some(class)) = body.class {
                all += 1;
                s += usize::from(s_complex(class));
                c += usize::from(c_type(class));
                v += usize::from(class == TaxonomicClass::V);
            }
        }
        let share = |n: usize| {
            if all == 0 {
                0.0
            } else {
                100.0 * f64::from(u32::try_from(n).unwrap())
                    / f64::from(u32::try_from(all).unwrap())
            }
        };
        println!(
            "P14.T51.e belt zone {name}: {classes:?}; lids {lids:?}; classed {all}: V {:.0}%, S \
             complex {s} ({:.0}%), C type {c} ({:.0}%); radii {smallest:.0}–{largest:.0} km",
            share(v),
            share(s),
            share(c)
        );
        (s, c)
    };
    let heliocentric: fn(&CensusBody) -> f64 = |body| body.distance_au;
    let equivalent: fn(&CensusBody) -> f64 = |body| body.equivalent_au;
    // DeMeo and Carry 2013's inner belt by mass (Table 6, as decision-p14-t51e-qs reads it), whose
    // largest bodies the members' sizes are.
    println!("P14.T51.e DeMeo and Carry 2013 Table 6, inner belt by mass: V 69%, S 21%, C 6%");
    let mut inner_by_flux = (0, 0);
    for (by, distance) in [("a", heliocentric), ("flux", equivalent)] {
        let inner = zone(&format!("inner by {by} (< 2.5 au)"), distance, |a| {
            a < INNER_BELT_EDGE_AU
        });
        if by == "flux" {
            inner_by_flux = inner;
        }
        zone(&format!("middle by {by}"), distance, |a| {
            (INNER_BELT_EDGE_AU..OUTER_BELT_EDGE_AU).contains(&a)
        });
        zone(&format!("outer by {by} (>= 2.82 au)"), distance, |a| {
            a >= OUTER_BELT_EDGE_AU
        });
    }
    let v_types: Vec<String> = members
        .iter()
        .filter(|body| body.class == Modelled::Ok(Some(TaxonomicClass::V)))
        .map(|body| {
            format!(
                "{} ({}, {:.0} km) at {:.2} au, flux's {:.2} au",
                body.id,
                body.lid.map_or("no crust", CrustLithology::key),
                body.radius_km,
                body.distance_au,
                body.equivalent_au
            )
        })
        .collect();
    println!("P14.T51.e V-type members: {}", v_types.len());
    for line in v_types.iter().take(8) {
        println!("  {line}");
    }
    assert!(
        !v_types.is_empty(),
        "a large differentiated member can be V-type"
    );
    let (s, c) = inner_by_flux;
    assert!(
        s > c,
        "the inner belt's members by their flux: S complex {s} against C type {c}"
    );
}

/// The Kuiper-belt colours (P14.T51.e's test): every Kuiper-belt body, member or moon, is red at a
/// tholin share of [`VENEER_SHARE`] and over and neutral below, whatever its lid; the census holds
/// members and moons of each colour, which are printed with their shares.
#[test]
fn under_every_24_hold_kuiper_belt_bodies_are_red_or_neutral_by_their_cover() {
    let census = census();
    let mut seen: BTreeMap<(bool, &str), Vec<String>> = BTreeMap::new();
    for body in census
        .bodies
        .iter()
        .filter(|body| body.kind == TaxonomyBodyKind::KuiperBody)
    {
        let Some((s_t, s_i)) = body.veneer else {
            assert_eq!(body.class, Modelled::NotModelled, "{}", body.id);
            continue;
        };
        let expected = if s_t >= VENEER_SHARE {
            TaxonomicClass::Red
        } else {
            TaxonomicClass::Neutral
        };
        assert_eq!(body.class, Modelled::Ok(Some(expected)), "{}", body.id);
        seen.entry((body.parent.is_some(), expected.name()))
            .or_default()
            .push(format!(
                "{} s_t {s_t:.3} s_i {s_i:.3} lid {}",
                body.id,
                body.lid.map_or("none", CrustLithology::key)
            ));
    }
    for ((moon, colour), bodies) in &seen {
        let kind = if *moon { "moons" } else { "members" };
        println!("P14.T51.e Kuiper {kind} {colour}: {}", bodies.len());
        for body in bodies.iter().take(5) {
            println!("  {body}");
        }
    }
    for moon in [false, true] {
        for colour in [TaxonomicClass::Red, TaxonomicClass::Neutral] {
            assert!(
                seen.contains_key(&(moon, colour.name())),
                "the census holds no {} Kuiper-belt {}",
                colour.name(),
                if moon { "moon" } else { "member" }
            );
        }
    }
}

/// The cometary bodies (P14.T51.e's test): a member or moon whose lid is `cometary` is not classed
/// under a majority of uncovered ice, D under a majority tholin cover and P otherwise; the
/// census's are printed by class with their distances, those nearest the flux of 4 au first, and
/// it holds a dry P one. D and the not-classed case are not asserted: the sample holds none.
///
/// The sample holds no cometary body under a published water-ice cover: on an ice bulk under a
/// `cometary` lid the crust's frozen water is not listed among the condensates, the lid's row being
/// the surface the albedo reads and the ice the table beneath its lag
/// (`chem::partition::CrustWater::Unlisted`, decision-p14-t54a-ganymede §C3.1), so every dry one is
/// P, the dark class of the record's own surface. The not-classed case under a water-ice cover is
/// `hooks::small_bodies`' hand-built test
/// (`a_cometary_moon_under_water_ice_is_not_classed_and_a_dry_one_p`).
#[test]
fn under_every_24_hold_cometary_bodies_are_unclassed_under_ice_and_p_when_dry() {
    let census = census();
    let mut seen: BTreeMap<&str, Vec<(f64, String)>> = BTreeMap::new();
    for body in census.bodies.iter().filter(|body| {
        body.lid == Some(CrustLithology::Cometary)
            && matches!(
                body.kind,
                TaxonomyBodyKind::AsteroidBeltMember | TaxonomyBodyKind::OtherMoon
            )
    }) {
        let Some((s_t, s_i)) = body.veneer else {
            assert_eq!(body.class, Modelled::NotModelled, "{}", body.id);
            continue;
        };
        let expected = if s_t >= VENEER_SHARE {
            Some(TaxonomicClass::D)
        } else if s_i >= VENEER_SHARE {
            None
        } else {
            Some(TaxonomicClass::P)
        };
        assert_eq!(body.class, Modelled::Ok(expected), "{}", body.id);
        let entries: Vec<String> = body
            .surface
            .condensates()
            .entries()
            .iter()
            .map(|entry| {
                format!(
                    "{} {:?} {:.3}{}",
                    entry.substance().substance().key().as_str(),
                    entry.phase(),
                    entry.area_fraction(),
                    if entry.is_seasonal() { " seasonal" } else { "" }
                )
            })
            .collect();
        seen.entry(name_of(body.class)).or_default().push((
            (body.equivalent_au - 4.0).abs(),
            format!(
                "{} ({:?}, {:.0} km) at {:.2} au, flux's {:.2} au, T_s {:.1} K, s_t {s_t:.3} \
                 s_i {s_i:.3}, condensates {entries:?}",
                body.id,
                body.kind,
                body.radius_km,
                body.distance_au,
                body.equivalent_au,
                body.surface.conditions().mean_surface_temperature().value()
            ),
        ));
    }
    for (class, bodies) in &mut seen {
        println!(
            "P14.T51.e cometary {class}: {}, nearest the flux of 4 au first",
            bodies.len()
        );
        bodies.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, body) in bodies.iter().take(8) {
            println!("  {body}");
        }
    }
    let dry = seen.get("P").map_or(0, Vec::len);
    let nearest = seen
        .get("P")
        .and_then(|bodies| bodies.first())
        .map_or(f64::NAN, |(off, _)| *off);
    println!(
        "P14.T51.e cometary P: {dry} dry, the nearest the flux of 4 au {nearest:.2} au off it"
    );
    assert!(dry > 0, "the census holds no dry cometary body");
}

/// Over the census: every body orbiting its star with no field and no air above the ion floor, its
/// shielded share 1, has its unshielded wind term equal to `weathering().wind()` to the bit; and
/// every moon whose shielded term is 0 under no air, inside its planet's magnetopause, has a
/// positive unshielded term, the same bits as its planet's and its sibling moons', its planet's
/// orbit's.
#[test]
fn under_every_24_hold_the_unshielded_wind_term_is_the_shielded_one_where_nothing_shields() {
    let census = census();
    let open = |body: &CensusBody| {
        body.gravity.is_some_and(|g| {
            air_shield(body.surface.conditions().surface_pressure(), g) == AirShield::Open
        })
    };
    let (mut equal, mut inside, mut windless) = (0_usize, 0_usize, 0_usize);
    for body in &census.bodies {
        let Some((w0, wind, _)) = body.wind else {
            continue;
        };
        let unfielded = body.surface.figures().magnetic_field() == MagneticField::None;
        if body.parent.is_none() && unfielded && open(body) {
            assert_same_bits(w0, wind);
            equal += 1;
        }
        // A host with no X-ray and ultraviolet light has no wind, shielded or not.
        if w0 <= 0.0 {
            assert_same_bits(wind, 0.0);
            windless += 1;
            continue;
        }
        if let Some(parent) = body.parent
            && wind <= 0.0
            && open(body)
            && unfielded
        {
            for sibling in census.bodies.iter().filter(|other| {
                other.system == body.system
                    && (other.parent == Some(parent) || other.index == parent)
            }) {
                if let Some((other, ..)) = sibling.wind {
                    assert_same_bits(other, w0);
                }
            }
            inside += 1;
        }
    }
    println!(
        "P14.T51.e unshielded wind: {equal} unshielded bodies equal to the bit; {inside} moons \
         inside a magnetopause; {windless} bodies of windless hosts"
    );
    assert!(equal > 0, "the census holds unshielded bodies");
}

/// A member without a crust on the record is not modelled, while a Kuiper-belt body's class reads
/// no lid (decision-p14-t51e-contract §1.5): a census member's classed surface stripped of its
/// crust, as the rule reads it; and the held arm reads nothing of the same body.
#[test]
fn under_every_24_hold_a_member_without_a_crust_is_not_modelled() {
    let census = census();
    let w_ref = reference_wind_term();
    let body = census
        .bodies
        .iter()
        .find(|body| {
            body.kind == TaxonomyBodyKind::AsteroidBeltMember
                && matches!(body.class, Modelled::Ok(Some(_)))
        })
        .expect("the census classes some belt member");
    let bare = body.surface.clone().with_crust(None);
    assert_eq!(
        surface_class(
            TaxonomyBodyKind::AsteroidBeltMember,
            w_ref,
            body.gravity,
            &bare
        ),
        Modelled::NotModelled
    );
    assert_eq!(
        surface_class(TaxonomyBodyKind::OtherMoon, w_ref, body.gravity, &bare),
        Modelled::NotModelled
    );
    let kuiper = surface_class(TaxonomyBodyKind::KuiperBody, w_ref, body.gravity, &bare);
    assert!(
        matches!(
            kuiper,
            Modelled::Ok(Some(TaxonomicClass::Red | TaxonomicClass::Neutral))
        ),
        "{}: as a Kuiper-belt body with no crust, {kuiper:?}",
        body.id
    );
    assert_eq!(
        surface_class(TaxonomyBodyKind::Planet, w_ref, body.gravity, &bare),
        Modelled::Ok(None)
    );
    let unweathered = bare.with_modifiers(Modelled::NotModelled);
    assert_eq!(
        surface_class(
            TaxonomyBodyKind::KuiperBody,
            w_ref,
            body.gravity,
            &unweathered
        ),
        Modelled::NotModelled
    );
    // The held arm reads nothing: the same classed body is not modelled under it.
    let system = &census.systems[body.system].1;
    let of = system
        .body(body.index)
        .expect("a census body is of its system");
    assert_eq!(
        record_taxonomic_class(
            SmallBodyTypes::Held,
            (system, of),
            body.gravity,
            &body.surface
        ),
        Modelled::NotModelled
    );
    assert_eq!(
        record_taxonomic_class(
            SmallBodyTypes::Batch {
                reference_wind_term: w_ref
            },
            (system, of),
            body.gravity,
            &body.surface
        ),
        body.class
    );
}

/// Deterministic, with no draw: the first systems of the census generated and read again under
/// the same switches give the same classes, body by body.
#[test]
fn under_every_24_hold_two_runs_give_the_same_classes() {
    let census = census();
    let again = with_every_24_hold(|| {
        sample()
            .iter()
            .take(3)
            .flat_map(|ctx| {
                let system = generate(SEED, ctx).with_record_rules(every_record_batch());
                let snapshot = system.snapshot_at(ctx, UniverseTime::EPOCH);
                snapshot
                    .bodies()
                    .iter()
                    .filter_map(|record| match record.surface() {
                        Section::Ok(surface) => Some((record.index(), surface.taxonomic_class())),
                        Section::NotResolved | Section::NotModelled | Section::NotApplicable => {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    let first: Vec<_> = census
        .bodies
        .iter()
        .filter(|body| body.system < 3)
        .map(|body| (body.index, body.class))
        .collect();
    assert!(
        !first.is_empty(),
        "the census's first systems hold surfaces"
    );
    assert_eq!(first, again);
}

/// Under every hold at 24 (the census's systems, generated and read inside the switches), on the
/// first three systems with both moons and Kuiper-belt bodies, `body_at` gives each body the class
/// its snapshot gives it, and the same whatever order its system's bodies are asked in: the class
/// reads its own record's members, a moon's kind its parent's belt, and its query's reference wind
/// term, and keeps nothing between bodies (the sim-determinism skill's call-order rule).
#[test]
fn under_every_24_hold_body_at_gives_the_snapshot_s_class_in_any_order() {
    let census = census();
    let t = UniverseTime::EPOCH;
    let class_of = |record: &BodyRecord| match record.surface() {
        Section::Ok(surface) => Some(surface.taxonomic_class()),
        Section::NotResolved | Section::NotModelled | Section::NotApplicable => None,
    };
    let holds = |at: usize, test: &dyn Fn(&CensusBody) -> bool| {
        census
            .bodies
            .iter()
            .any(|body| body.system == at && test(body))
    };
    let chosen: Vec<usize> = (0..census.systems.len())
        .filter(|&at| {
            holds(at, &|body| body.kind == TaxonomyBodyKind::KuiperBody)
                && holds(at, &|body| body.parent.is_some())
        })
        .take(3)
        .collect();
    assert_eq!(
        chosen.len(),
        3,
        "the census holds three systems with moons and Kuiper-belt bodies"
    );
    let mut classed = 0_usize;
    with_every_24_hold(|| {
        for &at in &chosen {
            let (ctx, system) = &census.systems[at];
            let snapshot = system.snapshot_at(ctx, t);
            let indices = system.indices();
            assert_eq!(snapshot.bodies().len(), indices.len());
            for (record, &index) in snapshot.bodies().iter().zip(&indices) {
                let alone = system.body_at(ctx, index, t).expect("a body of its system");
                assert_eq!(
                    class_of(record),
                    class_of(&alone),
                    "{}",
                    record.identity().id()
                );
                classed += usize::from(matches!(class_of(record), Some(Modelled::Ok(Some(_)))));
            }
            assert_order_independent(&indices, |&index| {
                class_of(&system.body_at(ctx, index, t).expect("a body of its system"))
            });
        }
    });
    assert!(classed > 0, "the chosen systems class some body");
}
