//! P14.T27.b's tests: a rogue planet's hold, its context, its object derived with no star, its
//! satellites with design note 13's ejection cut, their indices, and its determinism.

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, ResolveSystemError, SystemRecord, generate_cell};
use crate::galaxy::substellar::{ROGUE_PLANET_MAX_MSUN, ROGUE_PLANET_MIN_MSUN};
use crate::id::Layer;
use crate::orbit::KeplerElements;
use crate::planetary::context::SystemContext;
use crate::planetary::derive::{OrbitSense, PlanetClass};
use crate::planetary::fate::BodyState;
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::label::BodyLabel;
use crate::planetary::moons::CaptureKind;
use crate::planetary::moons::irregular::KOZAI_GAP;
use crate::planetary::moons::regular::MOON_ICE_DENSITY;
use crate::planetary::placement::{OrbitHost, mutual_hill_radius};
use crate::planetary::record::{BodyKind, BodyRecord, Population, Section, SectionState};
use crate::planetary::rings::Ring;
use crate::planetary::satellites::{
    Satellite, SatelliteFrame, SatelliteMoon, Satellites, satellites_inside,
};
use crate::planetary::system::{PlanetarySystem, RoguePlanet, generate, generate_planets};
use crate::planetary::testing::{
    SampleFilter, sample_records, with_every_25_hold, with_every_26_hold,
};
use crate::stellar::substellar::giant_cooling;
use crate::stellar::system::draw_metallicity;
use crate::time::{ClockWindow, UniverseTime};
use crate::units::Years;
use crate::units::consts::{COSMIC_BACKGROUND_TEMPERATURE_K, GRAVITATIONAL_CONSTANT};

const SEED: u64 = 0x0e14_0027_b000_0001;

/// 2√3, design note 7's gap between one moon's apocentre and the next one's pericentre.
const GAP: f64 = 3.464_101_615_137_754_6;

fn galaxy() -> &'static Galaxy {
    static GALAXY: std::sync::OnceLock<Galaxy> = std::sync::OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

fn seed() -> Seed {
    galaxy().seed()
}

/// The first `n` rogue planets of the cells of layer G about the solar circle, 26,000 ly out on
/// +y, in a fixed walk of the cells: a slab two cells thick about the plane, widening outwards
/// from the cell at the Sun-like point.
fn rogue_planets(n: usize) -> Vec<SystemRecord> {
    let mut found = Vec::with_capacity(n);
    let mut cell = Vec::new();
    'walk: for ring in 0_i32.. {
        for x in -ring..=ring {
            for y in -ring..=ring {
                if x.abs().max(y.abs()) != ring {
                    continue;
                }
                for z in [-1, 0] {
                    let key = CellKey::new(Layer::RoguePlanet, [x, 6_500 + y, z])
                        .expect("the solar circle lies inside the cube");
                    generate_cell(galaxy(), key, &mut cell);
                    found.extend(cell.iter().copied());
                    if found.len() >= n {
                        break 'walk;
                    }
                }
            }
        }
    }
    found.truncate(n);
    assert!(
        found.iter().all(|r| r.kind() == SystemKind::RoguePlanet),
        "layer G places rogue planets alone"
    );
    found
}

/// `record` with its mass `mass` and its age at the epoch `age`, the rest its own.
fn with_mass_and_age(record: &SystemRecord, mass: SolarMasses, age: Years) -> SystemRecord {
    SystemRecord::from_parts(
        record.id(),
        *record.epoch_position(),
        record.origin(),
        record.population(),
        mass,
        age,
    )
}

/// The first `n` rogue planets of [`rogue_planets`], their masses spread log-uniformly across plan
/// 13's 0.3 M⊕–13 `M_J`, each at the middle of its share, in that order: a sample of every regime
/// the derivation has, where the drawn masses (Sumi et al.'s law) give few giants.
fn rogue_planets_of_every_mass(n: usize) -> Vec<SystemRecord> {
    let (lo, hi) = (ROGUE_PLANET_MIN_MSUN, ROGUE_PLANET_MAX_MSUN);
    rogue_planets(n)
        .iter()
        .zip(0_u32..)
        .map(|(record, i)| {
            let share = (f64::from(i) + 0.5) / f64::from(u32::try_from(n).unwrap());
            let mass = SolarMasses::new(lo * math::powf(hi / lo, share));
            with_mass_and_age(record, mass, record.age_at_epoch())
        })
        .collect()
}

/// The context and whole system of `record` under the rogue planets' rules, from its record.
fn whole(record: &SystemRecord) -> (SystemContext, PlanetarySystem) {
    with_rogue_planets(RoguePlanetRules::Bodies, || {
        let ctx = SystemContext::from_record(galaxy(), record);
        let system = generate(seed(), &ctx);
        (ctx, system)
    })
}

/// P14.T27.b is held until the 25 → 26 bump: a rogue planet's system is refused, inside 26's
/// helper alone, apart from the brown dwarfs' hold.
#[test]
fn the_rogue_planets_are_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < ROGUE_PLANET_HOSTS_VERSION,
        "the 25 → 26 bump gives the rogue planets their bodies (P14.T27.b): delete \
         `ROGUE_PLANET_HOSTS_VERSION`, `RoguePlanetRules` with its `Held` arm and the held arm of \
         `generates`, the tests' `with_rogue_planets`, its switch in `with_every_26_hold` and \
         this test; open the client's `SYSTEM` view to rogue planets (`SystemsPanel.tsx`'s \
         `freeFloating`, which then holds nothing); delete `tests/substellar_placement.rs`'s \
         `a_free_floating_object_has_no_stellar_context_yet` with the brown dwarfs' half; turn \
         the server's `a_rogue_planet_s_bodies_are_refused_until_the_26_bump` into a test of its \
         bodies; delete `admit_free_floating_hosts_in_this_process` with T27.a's hold, fold the \
         server's `scene_free_floating.rs` into `scene.rs` and turn `scene.rs`'s \
         `a_ship_beside_a_free_floating_object_is_not_in_its_frame_before_26` into a test of the \
         arrival; `frame_at` then walks layers F and G whatever the arms"
    );
    let rogue = rogue_planets(1)[0];
    let refused = Err(ResolveSystemError::LayerNotGenerated(Layer::RoguePlanet));
    assert_eq!(RoguePlanetRules::in_force(), RoguePlanetRules::Held);
    assert!(!generates(SystemKind::RoguePlanet));
    assert_eq!(SystemContext::for_system(galaxy(), rogue.id()), refused);
    assert_eq!(
        with_every_26_hold(RoguePlanetRules::in_force),
        RoguePlanetRules::Bodies
    );
    assert_eq!(
        with_every_25_hold(RoguePlanetRules::in_force),
        RoguePlanetRules::Held
    );
    assert_eq!(
        with_every_26_hold(|| {
            with_rogue_planets(RoguePlanetRules::Held, RoguePlanetRules::in_force)
        }),
        RoguePlanetRules::Held
    );
    // The two holds are apart: each arm leaves the other's kind as it was.
    with_rogue_planets(RoguePlanetRules::Bodies, || {
        assert!(generates(SystemKind::RoguePlanet));
        assert!(!generates(SystemKind::BrownDwarf));
        let context = SystemContext::for_system(galaxy(), rogue.id()).expect("generated");
        assert_eq!(context.host_kind(), HostKind::RoguePlanet);
    });
    with_substellar_hosts(SubstellarHostRules::BrownDwarfs, || {
        assert!(!generates(SystemKind::RoguePlanet));
        assert_eq!(SystemContext::for_system(galaxy(), rogue.id()), refused);
    });
    assert_eq!(RoguePlanetRules::in_force(), RoguePlanetRules::Held);
}

/// The context's lean: no star, and the object's plan 13 record, its mass, age and [Fe/H], with a
/// grid system's surroundings at its own mass; `for_system` and `from_record` agree.
#[test]
fn a_rogue_planet_s_context_holds_no_star_and_its_own_record() {
    for record in rogue_planets(40) {
        with_rogue_planets(RoguePlanetRules::Bodies, || {
            let ctx = SystemContext::for_system(galaxy(), record.id()).expect("generated");
            assert_eq!(ctx, SystemContext::from_record(galaxy(), &record));
            assert_eq!(ctx.host_kind(), HostKind::RoguePlanet);
            assert_eq!(ctx.stars(), []);
            assert!(ctx.hierarchy().is_none());
            assert_eq!(ctx.zones(), []);
            assert_eq!(ctx.zone_stars(), []);
            assert_eq!(ctx.xuv_histories(), []);
            assert_eq!(ctx.rogue_planet_mass(), Some(record.primary_initial_mass()));
            assert_eq!(ctx.system_mass(), record.primary_initial_mass());
            assert_eq!(ctx.composition(), &draw_metallicity(galaxy(), &record));
            assert_same_bits(ctx.age_at_epoch().value(), record.age_at_epoch().value());
            assert!(ctx.alpha_fe().is_some());
            let tidal = ctx.tidal_radius().value();
            assert!(tidal.is_finite() && tidal > 0.0);
            assert!(ctx.strip_radius() < ctx.tidal_radius());
            assert_eq!(ctx.heap_bytes(), 0);
        });
    }
}

/// Every stellar system and every brown dwarf is bit-identical under both arms of the rogue
/// planets' hold: its context and its planets.
#[test]
fn every_system_with_stars_is_bit_identical_under_both_arms() {
    let records = sample_records(
        galaxy(),
        120,
        SampleFilter::primary_masses(SolarMasses::new(0.08), SolarMasses::new(2.0)),
    )
    .unwrap();
    for record in &records {
        let run = |rules| {
            with_rogue_planets(rules, || {
                let ctx = SystemContext::from_record(galaxy(), record);
                let planets = generate_planets(seed(), &ctx);
                (ctx, planets)
            })
        };
        assert_eq!(
            run(RoguePlanetRules::Held),
            run(RoguePlanetRules::Bodies),
            "{:?}",
            record.id()
        );
    }
}

/// What [`ten_thousand_rogue_planets_derive_without_a_star`] counted.
#[derive(Debug, Default)]
struct Census {
    objects: usize,
    born: usize,
    giants: usize,
    solids: usize,
    with_moons: usize,
    moons: usize,
    irregulars: usize,
    rings: usize,
    ejected_with_cut: usize,
    /// The largest move of an effective temperature against the lane's of eaeb2d7f, which took
    /// `T_eq` as 0 K: T − (T⁴ − `T_CMB`⁴)^¼, K, and its body.
    largest_t_eff_move: (f64, String),
}

/// The plan's test (b), under the 26 arm: 10⁴ rogue planets of 0.3 M⊕–13 `M_J` derive without a
/// star, every figure finite, with the sections that need one `NotModelled`; and their
/// satellites keep P14.T22.b's properties at −H, the epoch and +H.
#[test]
fn ten_thousand_rogue_planets_derive_without_a_star() {
    let mut census = Census::default();
    for record in rogue_planets_of_every_mass(10_000) {
        let (ctx, system) = whole(&record);
        assert!(system.zones().is_empty() && system.hosts().is_empty());
        assert!(system.belts().is_empty() && system.halo().is_none());
        assert_eq!(system.planets().count(), 0);
        let object = *system.rogue_planet().expect("a rogue planet's object");
        assert_eq!(
            object.mass(),
            EarthMasses::from(record.primary_initial_mass())
        );
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            let snapshot = system.snapshot_at(&ctx, t);
            assert_eq!(snapshot.belts().state(), SectionState::NotApplicable);
            assert_eq!(snapshot.halo().state(), SectionState::NotApplicable);
            let record_of = snapshot.body(BodyIndex::PRIMARY).expect("the object");
            assert_eq!(
                record_of,
                &system.body_at(&ctx, BodyIndex::PRIMARY, t).unwrap()
            );
            check_object(&ctx, &object, record_of, t, &mut census);
            for found in system.satellites() {
                check_satellites(&ctx, &system, &object, found, t, &mut census);
            }
        }
    }
    println!("{census:?}");
    println!(
        "the largest effective temperature's move against T_eq = 0 K: {:.3e} K, {}; T_s is not \
         modelled",
        census.largest_t_eff_move.0, census.largest_t_eff_move.1
    );
    // Pinned as it stands: 0.095 K, on a 5.3 K body, a small, old and metal-poor rock whose own
    // heat, 734 K⁴, is far below the 10⁶ K⁴ research expected (a finding).
    assert!(census.largest_t_eff_move.0 < 0.1, "{census:?}");
    assert_eq!(census.objects, 30_000);
    assert!(census.born > 29_000, "{census:?}");
    assert!(census.giants > 3_000 && census.solids > 3_000, "{census:?}");
    assert!(
        census.with_moons > 1_000 && census.rings > 100,
        "{census:?}"
    );
    assert!(census.irregulars > 100, "{census:?}");
}

/// The object's record at `t`: finite figures where it is born, `NotApplicable` where not.
fn check_object(
    ctx: &SystemContext,
    object: &RoguePlanet,
    record: &BodyRecord,
    t: UniverseTime,
    census: &mut Census,
) {
    let id = ctx.id();
    census.objects += 1;
    assert_eq!(record.identity().kind(), BodyKind::Planet);
    assert_eq!(
        record.identity().parent(),
        None,
        "{id:?}: the system's root"
    );
    assert_eq!(
        record.identity().label().ok().map(BodyLabel::as_str),
        Some("A")
    );
    assert_eq!(record.mass(), &Section::Ok(object.mass()));
    assert_eq!(record.population(), &Section::NotApplicable);
    if record.identity().state() != BodyState::Present {
        assert_eq!(record.identity().state(), BodyState::NotYetFormed);
        assert!(ctx.age_at(t).value() <= 0.0);
        assert_eq!(record.bulk(), &Section::NotApplicable);
        assert!(record.position().is_none());
        return;
    }
    census.born += 1;
    assert_eq!(
        record.position(),
        Some(crate::coords::SystemPosition::ORIGIN)
    );
    assert_eq!(record.orbit(), &Section::NotApplicable, "{id:?}");
    for state in [
        record.photometry().state(),
        record.figure().state(),
        record.rotation().state(),
    ] {
        assert_eq!(state, SectionState::NotModelled, "{id:?}");
    }
    assert_eq!(record.hooks().state(), SectionState::Ok);
    let bulk = record.bulk().ok().expect("a born rogue planet derives");
    let fractions = bulk.fractions();
    for x in [
        bulk.radius().value(),
        bulk.density().value(),
        bulk.surface_gravity().value(),
    ] {
        assert!(x.is_finite() && x > 0.0, "{id:?}: {bulk:?}");
    }
    let parts = [
        fractions.iron(),
        fractions.rock(),
        fractions.water(),
        fractions.envelope(),
    ];
    assert!(parts.iter().all(|f| (0.0..=1.0).contains(f)), "{id:?}");
    assert!((parts.iter().sum::<f64>() - 1.0).abs() < 1e-9, "{id:?}");
    // The cosmic background's light alone reaches it.
    assert_same_bits(
        bulk.equilibrium_temperature().value(),
        COSMIC_BACKGROUND_TEMPERATURE_K,
    );
    let t_eff = bulk
        .effective_temperature()
        .expect("a rogue planet radiates its own heat")
        .value();
    assert!(
        t_eff.is_finite() && (1.0..3_000.0).contains(&t_eff),
        "{id:?}: {t_eff} K"
    );
    let cmb4 = math::powi(COSMIC_BACKGROUND_TEMPERATURE_K, 4);
    let moved = t_eff - (math::powi(t_eff, 4) - cmb4).sqrt().sqrt();
    assert!(moved.is_finite() && moved >= 0.0, "{id:?}: {moved} K");
    if moved > census.largest_t_eff_move.0 {
        census.largest_t_eff_move = (moved, format!("{:?} of {t_eff} K", record.identity().id()));
    }
    let solid = bulk.class().has_solid_surface();
    let (surface, envelope) = if solid {
        (SectionState::NotModelled, SectionState::NotApplicable)
    } else {
        (SectionState::NotApplicable, SectionState::NotModelled)
    };
    assert_eq!(record.surface().state(), surface, "{id:?}");
    assert_eq!(record.envelope().state(), envelope, "{id:?}");
    if object.mass() >= EarthMasses::from(JupiterMasses::new(0.414)) {
        assert_eq!(bulk.class(), PlanetClass::GasGiant, "{id:?}");
    }
    census.giants += usize::from(bulk.class().is_giant());
    census.solids += usize::from(solid);
}

/// The sense a moon goes round its parent in.
fn sense(moon: &SatelliteMoon) -> OrbitSense {
    match moon {
        SatelliteMoon::Captured(captured) => captured.sense(),
        SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => OrbitSense::Prograde,
    }
}

/// Whether a moon is exempt from the non-crossing rule: an irregular or a rocky planet's small
/// capture (ruling 83.8).
fn irregular(moon: &SatelliteMoon) -> bool {
    match moon {
        SatelliteMoon::Captured(captured) => captured.kind() != CaptureKind::Large,
        SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => false,
    }
}

/// P14.T22.b's properties of the object's satellites `found` at `t`, as
/// `tests/planetary_properties.rs` asserts them about a planet, its Hill sphere and limits its
/// birth orbit's, which its generators read; a moon's density, which its record does not model,
/// at its least, pure ice's. And design note 13's cut: every moon of an ejected planet inside a
/// tenth of its birth Hill radius.
fn check_satellites(
    ctx: &SystemContext,
    system: &PlanetarySystem,
    object: &RoguePlanet,
    found: &Satellites,
    t: UniverseTime,
    census: &mut Census,
) {
    assert_eq!(found.parent_index(), BodyIndex::PRIMARY);
    let Some(parent) = found.parent() else {
        return;
    };
    let mut ordered: Vec<(KeplerElements, &Satellite)> = Vec::new();
    for moon in found.moons() {
        let r = system.body_at(ctx, moon.index(), t).unwrap();
        if r.identity().state() != BodyState::Present {
            continue;
        }
        assert_eq!(
            r.identity().parent(),
            Some(OrbitHost::Body(BodyIndex::PRIMARY))
        );
        assert_eq!(r.bulk(), &Section::NotModelled);
        let orbit = *r
            .orbit()
            .ok()
            .expect("a present moon has an orbit")
            .elements();
        let id = moon.index();
        let (apo, peri, e) = (
            orbit.apoapsis(),
            orbit.periapsis(),
            orbit.eccentricity().value(),
        );
        assert!(
            apo < parent.hill_radius_at_pericentre(),
            "{id:?} beyond the Hill radius"
        );
        let reach = match moon.moon() {
            SatelliteMoon::Captured(_) => orbit.semi_major_axis(),
            SatelliteMoon::Regular(_) | SatelliteMoon::GiantImpact(_) => apo,
        };
        assert!(
            reach < parent.stability_limit(e, sense(moon.moon())),
            "{id:?} beyond its limit"
        );
        assert!(
            peri > parent.roche_limit_fluid(MOON_ICE_DENSITY),
            "{id:?} inside Roche"
        );
        assert!(peri > parent.radius(), "{id:?} inside its planet");
        if let Some(bound) = object.moon_bound() {
            assert!(apo < bound, "{id:?} of an ejected planet beyond its cut");
        }
        let position = r.position().expect("a present moon has a position");
        assert!(position.metres().iter().all(|x| x.is_finite()));
        census.moons += 1;
        census.irregulars += usize::from(irregular(moon.moon()));
        ordered.push((orbit, moon));
    }
    census.with_moons += usize::from(!ordered.is_empty() && t == UniverseTime::EPOCH);
    check_crossings(ctx, parent, &ordered, t);
    for ring in found.rings() {
        let r = system.body_at(ctx, ring.index(), t).unwrap();
        if r.identity().state() != BodyState::Present {
            continue;
        }
        let Section::Ok(Population::Ring(ring)) = r.population() else {
            panic!("a present ring carries its extent");
        };
        assert!(ring.outer_edge() <= parent.roche_limit_fluid(ring.material().density()));
        assert!(ring.inner_edge() >= parent.radius());
        census.rings += usize::from(t == UniverseTime::EPOCH);
    }
    if t == UniverseTime::EPOCH {
        let uncut = uncut_satellites(ctx, system);
        census.ejected_with_cut += usize::from(uncut.moons().len() > found.moons().len());
    }
}

/// Ruling 83.8's non-crossing about the parent `parent` at `t`, over its present moons `ordered`
/// with their orbits then: regular, giant-impact and Triton-like moons keep design note 7's gap,
/// and each irregular clears them all and avoids the Kozai gap.
fn check_crossings(
    ctx: &SystemContext,
    parent: &crate::planetary::moons::MoonParent,
    ordered: &[(KeplerElements, &Satellite)],
    t: UniverseTime,
) {
    let mut kept: Vec<_> = ordered
        .iter()
        .filter(|(_, moon)| !irregular(moon.moon()))
        .collect();
    kept.sort_by(|a, b| {
        a.0.semi_major_axis()
            .value()
            .total_cmp(&b.0.semi_major_axis().value())
    });
    let host = SolarMasses::from(parent.mass());
    for pair in kept.windows(2) {
        let ((inner, mi), (outer, mo)) = (pair[0], pair[1]);
        let hill = mutual_hill_radius(
            mi.mass(),
            mo.mass(),
            host,
            inner.semi_major_axis(),
            outer.semi_major_axis(),
        );
        assert!(
            outer.periapsis() - inner.apoapsis() >= hill * GAP * (1.0 - 1e-9),
            "{:?} crosses {:?} at {t:?}",
            mi.index(),
            mo.index()
        );
    }
    let clear_of = kept
        .iter()
        .map(|(orbit, moon)| match moon.moon() {
            SatelliteMoon::GiantImpact(impact) => impact
                .orbit_at(parent, ctx.age_at(ClockWindow::END))
                .apoapsis(),
            SatelliteMoon::Regular(_) | SatelliteMoon::Captured(_) => orbit.apoapsis(),
        })
        .fold(Metres::new(0.0), |far, a| if a > far { a } else { far });
    let (lo, hi) = (KOZAI_GAP.0.value(), KOZAI_GAP.1.value());
    for (orbit, moon) in ordered.iter().filter(|(_, moon)| irregular(moon.moon())) {
        assert!(
            orbit.periapsis() > clear_of,
            "{:?} reaches the regulars",
            moon.index()
        );
        let degrees = moon
            .local_orbit_at(parent, ctx.age_at(t))
            .inclination()
            .value()
            .to_degrees();
        assert!(
            !(lo < degrees && degrees < hi),
            "{:?} at {degrees}°",
            moon.index()
        );
    }
}

/// The object's satellites as its generators give them with no cut.
fn uncut_satellites(ctx: &SystemContext, system: &PlanetarySystem) -> Satellites {
    let found = &system.satellites()[0];
    let Some(parent) = found.parent() else {
        return Satellites::none(BodyIndex::PRIMARY);
    };
    satellites_inside(
        seed(),
        BodyIndex::PRIMARY,
        parent,
        None,
        (None, ctx.age_at_epoch()),
        SatelliteFrame::OrbitalPlane,
        None,
    )
}

/// The plan's test (b): a 1 `M_J` rogue of 1 Gyr has an effective temperature within 20% of plan
/// 13's `giant_cooling` at its age, and so do giants of 0.5–12 `M_J` at 0.1–10 Gyr; below 0.3
/// `M_J`, the heat is its solids'.
#[test]
fn a_jupiter_mass_rogue_of_a_gigayear_glows_at_its_cooling_temperature() {
    let template = rogue_planets(1)[0];
    let of = |m_jup: f64, age: f64| {
        let record = with_mass_and_age(
            &template,
            SolarMasses::from(JupiterMasses::new(m_jup)),
            Years::new(age),
        );
        let (ctx, system) = whole(&record);
        let bulk = *system
            .body_at(&ctx, BodyIndex::PRIMARY, UniverseTime::EPOCH)
            .unwrap()
            .bulk()
            .ok()
            .expect("a born rogue planet derives");
        let cooling = giant_cooling(
            JupiterMasses::new(m_jup),
            Years::new(age),
            ctx.composition(),
        )
        .map(|state| state.effective_temperature().value());
        (bulk, cooling)
    };
    let (bulk, cooling) = of(1.0, 1.0e9);
    let ratio = bulk.effective_temperature().unwrap().value() / cooling.unwrap();
    println!(
        "1 M_J at 1 Gyr: {:?} against {cooling:?}, {ratio:.4}",
        bulk.effective_temperature()
    );
    assert!((ratio - 1.0).abs() < 0.2, "{ratio}");
    for m in [0.5, 2.0, 5.0, 12.0] {
        for age in [1.0e8, 1.0e9, 1.0e10] {
            let (bulk, cooling) = of(m, age);
            let ratio = bulk.effective_temperature().unwrap().value() / cooling.unwrap();
            assert!((ratio - 1.0).abs() < 0.2, "{m} M_J at {age} yr: {ratio}");
        }
    }
    // An Earth and a Neptune glow by their rock's heat alone, tens of kelvin, and fade with age.
    for m_earth in [1.0, 17.0] {
        let t_eff = |age: f64| {
            let record = with_mass_and_age(
                &template,
                SolarMasses::from(EarthMasses::new(m_earth)),
                Years::new(age),
            );
            let (ctx, system) = whole(&record);
            system
                .body_at(&ctx, BodyIndex::PRIMARY, UniverseTime::EPOCH)
                .unwrap()
                .bulk()
                .ok()
                .and_then(crate::planetary::record::BulkProperties::effective_temperature)
                .unwrap()
                .value()
        };
        let (young, old) = (t_eff(1.0e9), t_eff(1.0e10));
        println!("{m_earth} M_earth: {young} K at 1 Gyr, {old} K at 10 Gyr");
        assert!(
            (10.0..80.0).contains(&old) && young > old,
            "{m_earth}: {young} {old}"
        );
    }
}

/// Research-leans-12 §4 (d): a rogue of a Jupiter mass or more formed alone, and its heavy
/// elements are its cloud's, its record's Z (Z☉ × 10^[Fe/H]) times its mass, its envelope 1 − Z;
/// an ejected rogue under a Jupiter mass keeps Thorngren et al.'s disc-formed heavy elements.
#[test]
fn a_heavy_rogue_holds_its_cloud_s_own_metals() {
    use crate::planetary::derive::composition::giant_heavy_elements;
    let mut checked = 0;
    for template in rogue_planets(12) {
        for m_jup in [1.0, 2.0, 5.0, 12.0, 0.9] {
            let record = with_mass_and_age(
                &template,
                SolarMasses::from(JupiterMasses::new(m_jup)),
                Years::new(1.0e9),
            );
            let (ctx, system) = whole(&record);
            let mass = system.rogue_planet().unwrap().mass();
            let bulk = *system
                .body_at(&ctx, BodyIndex::PRIMARY, UniverseTime::EPOCH)
                .unwrap()
                .bulk()
                .ok()
                .expect("a born rogue planet derives");
            let f = bulk.fractions();
            let heavy = (f.iron() + f.rock() + f.water()) * mass.value();
            if ejected(mass) {
                let disc = giant_heavy_elements(mass).value();
                assert!((heavy / disc - 1.0).abs() < 1e-9, "{m_jup}: {heavy} M⊕");
                assert!(f.envelope() < 0.9, "{m_jup}: {}", f.envelope());
            } else {
                let z = ctx.composition().z().value();
                assert!(
                    (heavy - z * mass.value()).abs() <= 1e-12 * mass.value(),
                    "{m_jup} M_J: {heavy} M⊕ against Z M = {} M⊕",
                    z * mass.value()
                );
                assert!((f.envelope() - (1.0 - z)).abs() <= 1e-12, "{m_jup}");
                assert_eq!(bulk.class(), PlanetClass::GasGiant);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 48);
}

/// Design note 13's cut: an ejected planet keeps exactly the moons its generators give it whose
/// apocentre lies inside a tenth of its birth Hill radius, and a planet of a Jupiter mass or more
/// keeps every one. The cut bites on some sample planets.
#[test]
fn an_ejected_planet_keeps_only_the_moons_inside_a_tenth_of_its_birth_hill_radius() {
    let (mut cut, mut alone) = (0, 0);
    for record in rogue_planets_of_every_mass(3_000) {
        let (ctx, system) = whole(&record);
        let object = *system.rogue_planet().unwrap();
        let found = &system.satellites()[0];
        let Some(parent) = found.parent() else {
            continue;
        };
        assert_same_bits(
            parent.hill_radius().value(),
            object.birth_orbit().hill_radius(object.mass()).value(),
        );
        let uncut = uncut_satellites(&ctx, &system);
        let end = ctx.age_at(ClockWindow::END);
        let widest = |moon: &SatelliteMoon| match moon {
            SatelliteMoon::GiantImpact(impact) => impact.orbit_at(parent, end).apoapsis(),
            SatelliteMoon::Regular(moon) => moon.orbit().apoapsis(),
            SatelliteMoon::Captured(moon) => moon.orbit().apoapsis(),
        };
        let kept: Vec<_> = found.moons().iter().map(|moon| *moon.moon()).collect();
        if let Some(bound) = object.moon_bound() {
            assert!(ejected(object.mass()));
            assert_same_bits(
                bound.value(),
                EJECTED_MOON_HILL_SHARE * parent.hill_radius().value(),
            );
            let inside: Vec<_> = uncut
                .moons()
                .iter()
                .map(|moon| *moon.moon())
                .filter(|moon| widest(moon) < bound)
                .collect();
            assert_eq!(kept, inside, "{:?}", ctx.id());
            cut += usize::from(inside.len() < uncut.moons().len());
        } else {
            assert!(!ejected(object.mass()));
            assert_eq!(found.moons(), uncut.moons(), "{:?}", ctx.id());
            alone += 1;
        }
        // No regular moon is ever cut, so the rings are the uncut set's.
        let regular = |satellites: &Satellites| {
            satellites
                .moons()
                .iter()
                .filter(|moon| matches!(moon.moon(), SatelliteMoon::Regular(_)))
                .count()
        };
        assert_eq!(regular(found), regular(&uncut), "{:?}", ctx.id());
    }
    println!("{cut} ejected planets lost moons; {alone} formed alone");
    assert!(cut > 10 && alone > 100, "{cut} {alone}");
}

/// Design note 3, as plan 13 states it: body `0x0000` is the object, its moons are `0x0100`
/// upward, densely, and its rings `0x0080` upward by kind; their labels follow the object's, `A`.
#[test]
fn a_rogue_planet_is_body_0x0000_with_moons_from_0x0100_and_rings_from_0x0080() {
    let (mut moons, mut rings) = (0, 0);
    for record in rogue_planets_of_every_mass(2_000) {
        let (ctx, system) = whole(&record);
        let indices = system.indices();
        assert_eq!(indices.first(), Some(&BodyIndex::PRIMARY));
        assert!(
            system.body(BodyIndex::PRIMARY).is_none(),
            "the object is not a body"
        );
        let found = &system.satellites()[0];
        for (n, moon) in (1_u8..).zip(found.moons()) {
            let index = BodyIndex::new(BodySlot::Planet(n), BodySub::Primary).unwrap();
            assert_eq!(moon.index(), index);
            assert_eq!(index.get(), u16::from(n) << 8);
            moons += 1;
        }
        // Moons are numbered in Roman numerals by their primordial semi-major axes (design note
        // 22), after the object's label.
        let mut by_axis: Vec<(f64, BodyIndex)> = system
            .bodies()
            .iter()
            .filter_map(|body| Some((body.orbit()?.semi_major_axis().value(), body.index())))
            .collect();
        by_axis.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (n, &(_, index)) in (1_u8..).zip(&by_axis) {
            let record = system.body_at(&ctx, index, UniverseTime::EPOCH).unwrap();
            assert_eq!(
                record.identity().label().ok(),
                Some(&BodyLabel::moon(
                    &BodyLabel::rogue_planet(),
                    core::num::NonZeroU8::new(n).unwrap()
                ))
            );
        }
        // A ring's sub-index is its kind's, as a planet's: the dusty ring `0x80`, the massive
        // `0x81` (P14.T20).
        let mut last = None;
        for ring in found.rings() {
            let index = ring.index();
            assert_eq!(index.slot(), BodySlot::Stellar);
            assert!(matches!(index.sub(), BodySub::Ring(0 | 1)), "{index:?}");
            assert!((0x0080..=0x0081).contains(&index.get()));
            assert!(last < Some(index), "rings in index order");
            assert_eq!(index.parent(), Some(BodyIndex::PRIMARY));
            last = Some(index);
            rings += 1;
        }
        let expected: Vec<BodyIndex> = core::iter::once(BodyIndex::PRIMARY)
            .chain(found.moons().iter().map(Satellite::index))
            .chain(found.rings().iter().map(Ring::index))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(indices, expected);
        let snapshot = system.snapshot_at(&ctx, UniverseTime::EPOCH);
        let listed: Vec<BodyIndex> = snapshot.bodies().iter().map(BodyRecord::index).collect();
        assert_eq!(listed, indices);
        let object = snapshot.body(BodyIndex::PRIMARY).unwrap();
        let in_record = |section: &Section<Vec<BodyIndex>>| section.ok().cloned().unwrap();
        assert_eq!(
            in_record(object.moons()),
            found
                .moons()
                .iter()
                .map(Satellite::index)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            in_record(object.rings()),
            found.rings().iter().map(Ring::index).collect::<Vec<_>>()
        );
    }
    println!("{moons} moons, {rings} rings");
    assert!(moons > 500 && rings > 20, "{moons} {rings}");
}

/// The birth orbit reads words 2–4 of its object's `planet.origin` stream: its axis log-uniform
/// across [`BIRTH_AXIS_AU`], its plane isotropic.
#[test]
fn a_birth_orbit_reads_words_two_to_four_of_its_object_s_origin_stream() {
    let (lo, hi) = BIRTH_AXIS_AU;
    let (mut inside, mut north) = (0_u32, 0_u32);
    let records = rogue_planets(4_000);
    for record in &records {
        let object = BodyIndex::PRIMARY.body_id(record.id());
        let birth = BirthOrbit::draw(seed(), object);
        let mut stream = Stream::open(seed(), tags::PLANET_ORIGIN, ObjectKey::from(object));
        stream.seek(BIRTH_ORBIT_WORD);
        let au = lo * math::powf(hi / lo, stream.uniform_open());
        assert_same_bits(
            birth.semi_major_axis().value(),
            Metres::from(AstronomicalUnits::new(au)).value(),
        );
        let au = AstronomicalUnits::from(birth.semi_major_axis()).value();
        assert!((lo..hi).contains(&au));
        let snow =
            AstronomicalUnits::from(birth_host(&crate::stellar::Composition::SOLAR).snow_line)
                .value();
        inside += u32::from(au < snow);
        north += u32::from(birth.plane().inclination().value() < core::f64::consts::FRAC_PI_2);
        let elements = birth.elements(EarthMasses::new(1.0));
        assert_same_bits(elements.eccentricity().value(), 0.0);
        let mu = elements.gravitational_parameter().value();
        let expected =
            GRAVITATIONAL_CONSTANT * crate::units::Kilograms::from(BIRTH_HOST_MASS).value();
        assert!((mu / expected - 1.0).abs() < 1e-5);
    }
    // About a quarter form inside the snow line, half on each side of the plane.
    let n = f64::from(u32::try_from(records.len()).unwrap());
    let share = f64::from(inside) / n;
    let expected = math::ln(2.26 / lo) / math::ln(hi / lo);
    assert!(
        (share - expected).abs() < 0.03,
        "{share} against {expected}"
    );
    assert!((f64::from(north) / n - 0.5).abs() < 0.03);
}

/// D4 under the 26 arm: a rogue planet's system is the same every time and in every order, and
/// whole generation with its records at the epoch runs on every one of a sample with finite
/// figures, and its queries agree with its records.
#[test]
fn a_rogue_planet_s_system_is_the_same_every_time_and_in_every_order() {
    let records = rogue_planets(200);
    with_every_26_hold(|| {
        let whole = |record: &SystemRecord| {
            let ctx = SystemContext::for_system(galaxy(), record.id()).unwrap();
            let system = generate(seed(), &ctx);
            let snapshot = system.snapshot_at(&ctx, UniverseTime::EPOCH);
            (system, snapshot)
        };
        let first: Vec<_> = records.iter().map(whole).collect();
        assert_order_independent(&records, whole);
        for ((system, snapshot), record) in first.iter().zip(&records) {
            assert_eq!((system, snapshot), (&whole(record).0, &whole(record).1));
            let ctx = SystemContext::for_system(galaxy(), record.id()).unwrap();
            for body in snapshot.bodies() {
                let index = body.index();
                let at = system.state_at(&ctx, index, UniverseTime::EPOCH).unwrap();
                assert_eq!(
                    at.map(|(position, _)| position),
                    body.position(),
                    "{:?}",
                    record.id()
                );
                assert_eq!(
                    system
                        .position_at(&ctx, index, UniverseTime::EPOCH)
                        .unwrap(),
                    body.position()
                );
                let hooks = system.hooks_at(&ctx, index, UniverseTime::EPOCH).unwrap();
                assert_eq!(hooks.is_some(), body.identity().kind() != BodyKind::Ring);
                assert_eq!(
                    system.surface_seed(index).unwrap().is_some(),
                    body.identity().kind() != BodyKind::Ring
                );
                assert!(system.rotation_of(&ctx, index).unwrap().is_none());
            }
            assert!(system.event_models(&ctx).is_empty());
            assert!(
                system
                    .habitable_zone_at(&ctx, OrbitHost::Star(0), UniverseTime::EPOCH)
                    .is_none()
            );
        }
        assert!(
            first.iter().any(|(system, _)| !system.bodies().is_empty()),
            "some of the sample have satellites"
        );
    });
}
