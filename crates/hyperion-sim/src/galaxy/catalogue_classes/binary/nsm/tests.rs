//! The neutron-star merger class's tests (P11.T8.d): its share row's count and bounds, its entries
//! against the shared test at every age of the horizon on both sides of T, Peters's inspiral, the
//! remnant by total mass, one ID across T, the scan against the cells, the test's thinning,
//! resolution and order independence, the stars, the reach and a golden. A merger is rare, about
//! ten in a Milky Way-like galaxy, so the tests walk every cell of the fixture's. The ten seeds'
//! realised counts are a slow test (`tests/binary_mergers.rs`).

use std::sync::OnceLock;

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::PointLy;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, CatalogueEntry, check_class_headroom, resolve_entry, system_of,
};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::id::Layer;
use crate::orbit::peters_merger_time;
use crate::stellar::binary::BinaryClass;
use crate::stellar::binary::scan::{engine_calls, scan_nsm_cell};
use crate::stellar::remnant::collapse::MAX_NEUTRON_STAR_MASS;
use crate::tables::binary::{MILKY_WAY_FORMED_MASS, NsmPairKind};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L, Span, UniverseTime};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};

/// The seed of the Milky Way fixture the tests share.
const SEED: u64 = 0x0b17_08d0_0000_0001;

/// The class grid's cells per axis: 4,096 ly cells over the root cube's 131,072 ly.
const PER_AXIS: i32 = 16;

/// The fixture, with its kinematic tables, so that mergers move; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential()
    })
}

/// Every cell of the class, in x, y, z order.
fn every_cell() -> impl Iterator<Item = CatalogueCellKey> {
    (-PER_AXIS..PER_AXIS).flat_map(|x| {
        (-PER_AXIS..PER_AXIS).flat_map(move |y| {
            (-PER_AXIS..PER_AXIS).map(move |z| {
                CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, [x, y, z])
                    .expect("a cell of the root cube")
            })
        })
    })
}

/// The entries of cell `key`.
fn cell_entries(key: CatalogueCellKey) -> Vec<CatalogueEntry<NsmEntry>> {
    let galaxy = galaxy();
    CatalogueClassCell::generate(galaxy, &NeutronStarMergers::new(galaxy), key).into_entries()
}

/// Every cell of the fixture that holds an entry, with its entries, and every candidate the site's
/// thinning kept with whether it is an entry; walked once.
fn the_galaxy() -> &'static Walk {
    static WALK: OnceLock<Walk> = OnceLock::new();
    WALK.get_or_init(|| {
        let galaxy = galaxy();
        let process = NeutronStarMergers::new(galaxy);
        let mut walk = Walk::default();
        for key in every_cell() {
            let bound = process.bound(galaxy, &key);
            let mut entries = Vec::new();
            for index in 0..candidate_count(galaxy.seed(), &key, bound) {
                let id = key.candidate_id(index, 0).unwrap();
                if process
                    .hosts()
                    .site(galaxy, &key, id, bound, tags::CLASS_NSM_SITE)
                    .is_none()
                {
                    continue;
                }
                walk.thinned += 1;
                let kept = process
                    .candidate(galaxy, &key, id, bound)
                    .and_then(NeutronStarMergers::test);
                match kept {
                    Some(value) => entries.push(value),
                    None => walk.failed += 1,
                }
            }
            if !entries.is_empty() {
                walk.cells.push((key, entries));
            }
        }
        walk
    })
}

/// The fixture's walk.
#[derive(Debug, Default)]
struct Walk {
    cells: Vec<(CatalogueCellKey, Vec<NsmEntry>)>,
    thinned: u32,
    failed: u32,
}

/// Every entry of the fixture, with its cell.
fn all_entries() -> impl Iterator<Item = (&'static CatalogueCellKey, &'static NsmEntry)> {
    the_galaxy()
        .cells
        .iter()
        .flat_map(|(key, entries)| entries.iter().map(move |e| (key, e)))
}

/// The time `years` Julian years from `t`.
fn shifted(t: UniverseTime, years: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(years.abs() * SECONDS_PER_JULIAN_YEAR).expect("a span");
    if years >= 0.0 {
        t.checked_add(span).expect("on the clock")
    } else {
        t.checked_sub(span).expect("on the clock")
    }
}

// 1. The count and the bounds.

/// The class holds its share row's mergers: at the Milky Way fixture its components' expected
/// entries in the source horizon are the row's ten, the brainstorm's "about 10", inside the plan's
/// 8–12; and the fixture's own realised count is a Poisson draw of it.
#[test]
fn the_class_expects_about_ten_mergers_and_draws_them() {
    let galaxy = galaxy();
    let formed = galaxy
        .fields()
        .components()
        .iter()
        .fold(0.0, |sum, c| sum + c.count())
        * galaxy.mean_formed_mass().value();
    assert!((formed / MILKY_WAY_FORMED_MASS - 1.0).abs() < 1e-9);
    let expected = NeutronStarMergers::new(galaxy)
        .hosts()
        .expected_hosts(galaxy);
    assert!((expected / 10.0 - 1.0).abs() < 1e-9, "{expected}");
    assert!((8.0..=12.0).contains(&expected));
    let realised = all_entries().count();
    println!("{realised} neutron-star mergers against {expected:.3} expected");
    assert_poisson_count(
        "the fixture's neutron-star mergers",
        u64::try_from(realised).unwrap(),
        expected,
        ALPHA,
    );
}

/// A cell's bound holds the class's intensity at every point tried in it, and the centre's cell,
/// the fullest, leaves the index its headroom.
#[test]
fn the_bound_holds_the_intensity_and_the_index_has_room() {
    use hyperion_testkit::lcg::Lcg;
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x0b17_08d0_00b0);
    let process = NeutronStarMergers::new(galaxy);
    let mut largest = 0.0_f64;
    for cell in [[0, 0, 0], [-1, -1, -1], [0, 6, 0], [1, 6, -1], [0, 3, 0]] {
        let key = CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, cell).unwrap();
        let bound = process.bound(galaxy, &key).value();
        largest = largest.max(bound);
        for _ in 0..200 {
            let words = [(); 3].map(|()| lcg.next_u64());
            let p = PointLy::from(&key.position_from_words(words));
            let density = process.hosts().hosts_density(galaxy, &p).value();
            assert!(density <= bound, "{cell:?}: {density} > {bound}");
        }
    }
    assert_eq!(
        check_class_headroom(
            ClassId::NEUTRON_STAR_MERGER,
            PerCubicLightYear::new(largest)
        ),
        Ok(())
    );
}

// 2. The entries.

/// The fixture holds entries, and each resolves by its ID to itself, through the class's process,
/// the production lookup and plan 03's resolve.
#[test]
fn entries_resolve_to_themselves() {
    let galaxy = galaxy();
    let process = NeutronStarMergers::new(galaxy);
    assert!(all_entries().count() >= 3);
    for (_, value) in all_entries() {
        let id = value.scan().id();
        let member = resolve_entry(galaxy, &process, id).unwrap();
        assert_eq!(member.entry().value(), value);
        let system = system_of(galaxy, id).unwrap();
        assert_eq!(system.record(), value.record());
        assert_eq!(
            placement::resolve(galaxy, SystemId::from(id)),
            Ok(*value.record())
        );
    }
}

/// Every entry passes the shared test with its own class (design note 10), its pair's
/// `merger_age` is T, and at every age of the source horizon it is the inspiralling pair before T
/// and the remnant from T: two compact stars on the marked orbit, then one black hole of the
/// pair's mass, the secondary gone.
#[test]
fn every_entry_is_its_class_at_every_age_of_the_horizon() {
    let start = SourceHorizon::START.since_epoch().as_julian_years_f64();
    let end = SourceHorizon::END.since_epoch().as_julian_years_f64();
    for (_, value) in all_entries() {
        let now = value.record().age_at_epoch();
        let timeline = value.timeline();
        let t = value.scan().merger_age();
        assert_eq!(
            carved_class(timeline, now),
            Some(CarvedClass::NeutronStarMerger)
        );
        assert_eq!(timeline.merger_age(), Some(t));
        assert_eq!(value.record().age_at(value.scan().merger_time()), t);
        let horizon = (0..=16).map(|k| start + (end - start) * f64::from(k) / 16.0);
        for age in horizon.map(|dt| Years::new(now.value() + dt)) {
            let state = timeline.state_at(age);
            if age < t {
                assert_eq!(state.kind(), SegmentKind::Detached);
                assert_eq!(state.stars().map(|s| s.phase()), value.scan().phases());
                assert_eq!(state.orbit(), Some(value.pair().orbit()));
                let expected = match value.scan().kind() {
                    NsmPairKind::DoubleNeutronStar => BinaryClass::DoubleNeutronStar,
                    NsmPairKind::NeutronStarBlackHole => BinaryClass::None,
                };
                assert_eq!(timeline.class_at(age), expected);
            } else {
                assert_eq!(state.kind(), SegmentKind::Merged);
                assert_eq!(state.stars()[0], *value.product());
                assert_eq!(state.stars()[1].phase(), Phase::NoRemnant);
            }
        }
    }
}

/// The inspiral is Peters's: the marked circular orbit merges by gravitational waves in the time
/// from the source horizon's start to T; and the remnant follows the total mass, a black hole of
/// the pair's mass above plan 06's heaviest neutron star.
#[test]
fn the_inspiral_is_peters_s_and_the_remnant_follows_the_total_mass() {
    for (_, value) in all_entries() {
        let scan = value.scan();
        let [m1, m2] = scan.masses();
        let orbit = value.pair().orbit();
        let peters = peters_merger_time(m1, m2, orbit.semi_major_axis(), orbit.eccentricity());
        let to_merger = scan
            .merger_time()
            .checked_since(SourceHorizon::START)
            .unwrap()
            .as_julian_years_f64();
        assert!(
            (peters.value() / to_merger - 1.0).abs() < 1e-9,
            "{peters:?} {to_merger}"
        );
        let product = value.product();
        assert!(scan.total_mass() > MAX_NEUTRON_STAR_MASS);
        assert_eq!(product.phase(), Phase::BlackHole);
        assert_eq!(product.mass(), scan.total_mass());
        assert_eq!(scan.remnant_mass(), scan.total_mass());
        assert_eq!(
            scan.ejecta_mass(),
            crate::stellar::binary::light_curves::GW170817_EJECTA_MASS
        );
        for (star, phase) in value.pair().stars().iter().zip(scan.phases()) {
            assert_eq!(star.phase(), phase);
            assert!(star.luminosity().value().is_finite() && star.luminosity().value() >= 0.0);
            assert!(star.radius().value() > 0.0);
        }
        // Both progenitors died before T, and the primary progenitor is the heavier.
        let [p1, p2] = scan.progenitor_masses();
        assert!(p1 >= p2);
        assert!(scan.collapse_ages().iter().all(|c| *c < scan.merger_age()));
    }
}

/// One ID on both sides of T: the system its ID resolves to is two compact stars a year before T
/// and a lone black hole a year after.
#[test]
fn one_id_is_the_pair_before_t_and_the_remnant_after() {
    let galaxy = galaxy();
    let mut checked = 0;
    for (_, value) in all_entries() {
        let t = value.scan().merger_time();
        let (before, after) = (shifted(t, -1.0), shifted(t, 1.0));
        if before < SourceHorizon::START || after >= SourceHorizon::END {
            continue;
        }
        let system = system_of(galaxy, value.scan().id()).unwrap();
        let stars = system
            .stars(galaxy)
            .expect("a merger's stars are its class's");
        let pair = stars.state_at(before).expect("the system exists before T");
        let phases: Vec<Phase> = pair.stars().iter().map(StarState::phase).collect();
        assert_eq!(phases.as_slice(), value.scan().phases());
        let merged = stars.state_at(after).expect("the system exists after T");
        let left: Vec<_> = merged
            .stars()
            .iter()
            .filter(|s| s.phase() != Phase::NoRemnant)
            .collect();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0], value.product());
        checked += 1;
    }
    assert!(checked >= 3, "{checked}");
}

/// The scan keeps exactly each cell's entries, with their own marks, and neither the scan nor the
/// cells run the binary engine.
#[test]
fn the_scan_keeps_exactly_the_cells_entries() {
    let galaxy = galaxy();
    let process = NeutronStarMergers::new(galaxy);
    let before = engine_calls();
    for (key, entries) in &the_galaxy().cells {
        let mut scanned = Vec::new();
        scan_nsm_cell(galaxy, &process, *key, &mut scanned);
        let kept: Vec<_> = entries.iter().map(|e| *e.scan()).collect();
        assert_eq!(scanned, kept, "{key:?}");
        let fresh: Vec<_> = cell_entries(*key)
            .into_iter()
            .map(|e| e.value().clone())
            .collect();
        assert_eq!(&fresh, entries);
    }
    assert_eq!(engine_calls(), before);
}

/// Under one candidate in ten that the site's thinning keeps fails the scan's check that both
/// progenitors died before T, or the class's test (P11.T8).
#[test]
fn under_one_candidate_in_ten_fails_the_test() {
    let walk = the_galaxy();
    println!(
        "{} of {} thinned candidates failed the test",
        walk.failed, walk.thinned
    );
    assert!(walk.thinned >= 3, "{}", walk.thinned);
    assert!(f64::from(walk.failed) < 0.1 * f64::from(walk.thinned));
}

/// The cells do not depend on the order they are generated in, nor on what was resolved between
/// them.
#[test]
fn cells_do_not_depend_on_order() {
    let galaxy = galaxy();
    let cells: Vec<CatalogueCellKey> = the_galaxy().cells.iter().map(|(k, _)| *k).take(4).collect();
    assert_order_independent(&cells, |key| {
        let stray =
            CatalogueSystemId::new(ClassId::CORE_COLLAPSE.value(), key.fine_cell(), 0, 0).unwrap();
        let _ = placement::resolve(galaxy, SystemId::from(stray));
        cell_entries(*key)
    });
}

// 3. The systems.

/// An entry's stars, through plan 03's resolve and `system_of`, are its own, and its record is
/// counted in layer E.
#[test]
fn an_entry_s_stars_are_its_own() {
    let galaxy = galaxy();
    for (_, value) in all_entries() {
        let stars = value.stars(galaxy);
        assert_eq!(stars.star_count(), 2);
        assert_eq!(stars.record(), value.record());
        let system = system_of(galaxy, value.scan().id()).unwrap();
        assert_eq!(system.star_model(), None);
        assert_eq!(system.stars(galaxy), Some(stars));
        assert_eq!(value.record().layer(), Layer::E);
    }
}

/// A merger moves on one straight line at its drawn velocity, and stays within plan 09's
/// 4,096 ly reach of its cell over the source horizon.
#[test]
fn mergers_stay_within_the_reach() {
    let span_s = (CLOCK_WINDOW_H.as_seconds_f64() * 2.0) + LIGHT_CROSSING_L.as_seconds_f64();
    for (_, value) in all_entries() {
        let v = value.scan().velocity().metres_per_second();
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let travel_ly = speed * span_s / METRES_PER_LIGHT_YEAR;
        assert!(travel_ly < LIFETIME_REACH.value(), "{travel_ly} ly");
        assert_eq!(
            value.motion().position_at(UniverseTime::EPOCH),
            *value.scan().position()
        );
    }
}

// 4. Pins.

/// Every entry of the fixture pinned: its cell, scan marks, pair and remnant.
#[test]
fn neutron_star_merger_golden() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(&format!("entries {}", all_entries().count()));
    for (key, value) in all_entries() {
        let scan = value.scan();
        let id = format!(
            "cell {:?} {:016x}",
            key.cell(),
            SystemId::from(scan.id()).raw()
        );
        w.line(&format!(
            "{id} component {} kind {:?}",
            scan.component().index(),
            scan.kind()
        ));
        let p = scan.position();
        w.line(&format!("{id} cell {:?}", p.cell().to_array()));
        for (axis, metres) in p.offset_metres().iter().enumerate() {
            w.f64(&format!("{id} offset {axis}"), *metres);
        }
        for (axis, v) in scan.velocity().metres_per_second().iter().enumerate() {
            w.f64(&format!("{id} velocity {axis}"), *v);
        }
        w.line(&format!("{id} merger {}", scan.merger_time()));
        w.f64(&format!("{id} age"), scan.age_at_epoch().value());
        w.f64(&format!("{id} feh"), scan.composition().fe_h().value());
        for k in 0..2 {
            w.f64(&format!("{id} mass {k}"), scan.masses()[k].value());
            w.f64(
                &format!("{id} progenitor {k}"),
                scan.progenitor_masses()[k].value(),
            );
            w.f64(
                &format!("{id} collapse {k}"),
                scan.collapse_ages()[k].value(),
            );
            let star = &value.pair().stars()[k];
            w.f64(&format!("{id} luminosity {k}"), star.luminosity().value());
        }
        let orbit = value.pair().orbit();
        w.f64(&format!("{id} separation"), orbit.semi_major_axis().value());
        w.f64(
            &format!("{id} inclination"),
            orbit.orientation().inclination().value(),
        );
        w.f64(
            &format!("{id} mean anomaly"),
            orbit.mean_anomaly_at_epoch().value(),
        );
        w.line(&format!("{id} remnant {:?}", value.product().phase()));
        w.f64(
            &format!("{id} remnant mass"),
            value.product().mass().value(),
        );
    }
    golden!("catalogue_classes/neutron_star_mergers", w.as_str());
}
