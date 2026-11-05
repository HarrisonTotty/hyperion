//! The stellar merger class's tests (P11.T8.c): its share row, rate and bounds, its entries
//! against the shared test at every age of the horizon on both sides of T, one ID across T, the
//! scan against the cells, the test's thinning, resolution and order independence, the stars, the
//! reach, the layers and a golden. The sampler against the engine's own mergers is a slow test
//! (`tests/binary_mergers.rs`).

use std::sync::OnceLock;

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::PointLy;
use crate::galaxy::catalogue_classes::binary::feature::catalogue_share;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, CatalogueEntry, check_class_headroom, resolve_entry, system_of,
};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::id::Layer;
use crate::stellar::binary::BinaryClass;
use crate::stellar::binary::scan::{engine_calls, scan_merger_cell};
use crate::tables::binary::{CLASS_SHARES, MILKY_WAY_FORMED_MASS, MergerChannel};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L, SourceHorizon, Span, UniverseTime};
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The seed of the Milky Way fixture the tests share.
const SEED: u64 = 0x0b17_08c0_0000_0001;

/// The fixture, with its kinematic tables, so that mergers move; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential()
    })
}

/// The class cell `cell`.
fn key(cell: [i32; 3]) -> CatalogueCellKey {
    CatalogueCellKey::new(ClassId::STELLAR_MERGER, cell).expect("a cell of the root cube")
}

/// The sample cells: a run along x through the solar circle, in and just below the plane.
fn sample_cells() -> Vec<CatalogueCellKey> {
    (-2..2)
        .flat_map(|x| [key([x, 6, 0]), key([x, 6, -1])])
        .collect()
}

/// The entries of cell `key`.
fn cell_entries(key: CatalogueCellKey) -> Vec<CatalogueEntry<MergerEntry>> {
    let galaxy = galaxy();
    CatalogueClassCell::generate(galaxy, &StellarMergers::new(galaxy), key).into_entries()
}

/// The sample cells' entries, generated once.
fn sample_entries() -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<MergerEntry>>)] {
    type Cells = Vec<(CatalogueCellKey, Vec<CatalogueEntry<MergerEntry>>)>;
    static CELLS: OnceLock<Cells> = OnceLock::new();
    CELLS.get_or_init(|| {
        sample_cells()
            .into_iter()
            .map(|key| (key, cell_entries(key)))
            .collect()
    })
}

/// Every sample entry.
fn all_entries() -> impl Iterator<Item = &'static CatalogueEntry<MergerEntry>> {
    sample_entries().iter().flat_map(|(_, e)| e)
}

/// The time `years` Julian years from `t`.
fn shifted(t: UniverseTime, years: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(years.abs() * crate::units::consts::SECONDS_PER_JULIAN_YEAR)
        .expect("a span of years");
    if years >= 0.0 {
        t.checked_add(span).expect("on the clock")
    } else {
        t.checked_sub(span).expect("on the clock")
    }
}

// 1. The share row, the rate and the bounds.

/// The class and the features' lists together hold its share row's mergers, the catalogue all but
/// the features' share (P11.T8.f): at the Milky Way fixture the row's 9 × 10⁴ in the source
/// horizon, 0.34 a year, inside Kochanek et al.'s (2014) 0.2–0.5 a year, the brainstorm's figure,
/// of which the catalogue's are 0.9962.
#[test]
fn the_class_holds_its_share_row_s_mergers_at_kochanek_s_rate() {
    let galaxy = galaxy();
    let formed = galaxy
        .fields()
        .components()
        .iter()
        .fold(0.0, |sum, c| sum + c.count())
        * galaxy.mean_formed_mass().value();
    assert!((formed / MILKY_WAY_FORMED_MASS - 1.0).abs() < 1e-9);
    let process = StellarMergers::new(galaxy);
    let row = CLASS_SHARES.row(CarvedClass::StellarMerger);
    let by_row: f64 = process
        .hosts()
        .components()
        .iter()
        .map(|c| {
            let component = galaxy.fields().component(c.id());
            c.hosts_per_system() * component.count_with_unborn()
                / catalogue_share(galaxy, row, component)
        })
        .sum();
    let hosts = process.hosts().expected_hosts(galaxy);
    println!(
        "the row's {by_row:.1} mergers, the catalogue's {hosts:.1} ({:.4} of them)",
        hosts / by_row
    );
    assert!((by_row / 9.0e4 - 1.0).abs() < 1e-9, "{by_row}");
    assert!(hosts < by_row && hosts > 0.99 * by_row, "{hosts}");
    let rate = process.mergers_per_year(galaxy);
    println!("{rate:.4} stellar mergers a year in the catalogue");
    assert!((0.2..=0.5).contains(&rate), "{rate}");
    assert_eq!(
        process.hosts().components().len(),
        galaxy.fields().components().len()
    );
}

/// A cell's bound holds the class's intensity at every point tried in it, in the disc and at the
/// centre, and the centre's cell, the fullest, leaves the index its headroom.
#[test]
fn the_bound_holds_the_intensity_and_the_index_has_room() {
    use hyperion_testkit::lcg::Lcg;
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x0b17_08c0_00b0);
    let process = StellarMergers::new(galaxy);
    let mut largest = 0.0_f64;
    for cell in [[0, 0, 0], [-1, -1, -1], [0, 6, 0], [1, 6, -1], [0, 3, 0]] {
        let key = key(cell);
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
        check_class_headroom(ClassId::STELLAR_MERGER, PerCubicLightYear::new(largest)),
        Ok(())
    );
}

// 2. The entries.

/// The sample cells hold entries, and each resolves by its ID to itself, through the class's
/// process, the production lookup and plan 03's resolve.
#[test]
fn entries_resolve_to_themselves() {
    let galaxy = galaxy();
    let process = StellarMergers::new(galaxy);
    let entries: Vec<_> = all_entries().collect();
    assert!(entries.len() > 50, "{}", entries.len());
    for entry in entries {
        let member = resolve_entry(galaxy, &process, entry.id()).unwrap();
        assert_eq!(member.entry(), entry);
        let system = system_of(galaxy, entry.id()).unwrap();
        assert_eq!(system.record(), entry.value().record());
        assert_eq!(
            placement::resolve(galaxy, crate::id::SystemId::from(entry.id())),
            Ok(*entry.value().record())
        );
    }
}

/// Every entry passes the shared test with its own class (design note 10), its pair's
/// `merger_age` is T, and at every age of the source horizon a contact entry is the contact pair
/// before T and the merged star from T: a contact binary of two main-sequence stars, then one
/// main-sequence star, the secondary gone (the other channels' are `channels_hold`'s).
#[test]
fn every_entry_is_its_class_at_every_age_of_the_horizon() {
    let start = SourceHorizon::START.since_epoch().as_julian_years_f64();
    let end = SourceHorizon::END.since_epoch().as_julian_years_f64();
    for entry in all_entries() {
        if entry.value().scan().channel() != MergerChannel::Contact {
            continue;
        }
        let value = entry.value();
        let now = value.record().age_at_epoch();
        let timeline = value.timeline();
        let t = value.scan().merger_age();
        assert_eq!(
            carved_class(timeline, now),
            Some(CarvedClass::StellarMerger)
        );
        assert_eq!(timeline.merger_age(), Some(t));
        assert_eq!(value.record().age_at(value.scan().merger_time()), t);
        let horizon = (0..=16).map(|k| start + (end - start) * f64::from(k) / 16.0);
        for age in horizon.map(|dt| Years::new(now.value() + dt)) {
            let state = timeline.state_at(age);
            if age < t {
                assert_eq!(
                    state.kind(),
                    SegmentKind::Contact,
                    "{:?} at {age:?}",
                    entry.id()
                );
                assert_eq!(timeline.class_at(age), BinaryClass::Contact);
                assert_eq!(state.stars(), value.pair().stars());
                assert!(
                    state
                        .stars()
                        .iter()
                        .all(|s| s.phase() == Phase::MainSequence)
                );
            } else {
                assert_eq!(
                    state.kind(),
                    SegmentKind::Merged,
                    "{:?} at {age:?}",
                    entry.id()
                );
                assert_eq!(state.stars()[0], *value.product());
                assert_eq!(state.stars()[1].phase(), Phase::NoRemnant);
                assert_eq!(state.orbit(), None);
            }
        }
    }
}

/// One ID on both sides of T: the system a contact entry's ID resolves to is a pair of two stars in
/// contact a year before T, and a single merged star, the pair's mass less no more than its
/// main-sequence winds, a year after; the merged star is a main-sequence star younger in its main
/// sequence than either star was (P11.T4.d's rejuvenation).
#[test]
fn one_id_is_the_pair_before_t_and_the_merged_star_after() {
    let galaxy = galaxy();
    let mut checked = 0;
    for entry in all_entries() {
        let value = entry.value();
        if value.scan().channel() != MergerChannel::Contact {
            continue;
        }
        let t = value.scan().merger_time();
        let (before, after) = (shifted(t, -1.0), shifted(t, 1.0));
        if before < SourceHorizon::START || after >= SourceHorizon::END {
            continue;
        }
        let system = system_of(galaxy, entry.id()).unwrap();
        let stars = system
            .stars(galaxy)
            .expect("a merger's stars are its class's");
        let pair = stars.state_at(before).expect("the system exists before T");
        assert_eq!(pair.stars().len(), 2);
        assert!(
            pair.stars()
                .iter()
                .all(|s| s.phase() == Phase::MainSequence)
        );
        assert_eq!(pair.pairs()[0].class(), BinaryClass::Contact);
        let merged = stars.state_at(after).expect("the system exists after T");
        let living: Vec<_> = merged
            .stars()
            .iter()
            .filter(|s| s.phase() != Phase::NoRemnant)
            .collect();
        assert_eq!(living.len(), 1, "{:?}", entry.id());
        let product = living[0];
        assert_eq!(product, value.product());
        assert_eq!(product.phase(), Phase::MainSequence);
        let total = value.scan().total_mass().value();
        assert!(product.mass().value() <= total * (1.0 + 1e-12));
        assert!(product.mass().value() >= 0.9 * total, "{:?}", entry.id());
        let tau = value.pair().stars().map(|s| s.phase_fraction());
        assert!(product.phase_fraction() <= tau[0].max(tau[1]) + 1e-12);
        checked += 1;
    }
    // The contact channel holds some 15% of the entries (`MERGER_SAMPLER`'s weight).
    assert!(checked > 20, "{checked}");
}

/// The contact orbit of a contact entry: the primary fills its Roche lobe exactly, the secondary
/// its own, and the periods lie in hours to days, as contact binaries' do.
#[test]
fn the_pair_before_t_is_in_contact() {
    for entry in all_entries() {
        if entry.value().scan().channel() != MergerChannel::Contact {
            continue;
        }
        let pair = entry.value().pair();
        let [s1, s2] = pair.stars();
        let a = pair.orbit().semi_major_axis();
        let lobe = |q: f64| roche_lobe_radius(q, a).value() / SOLAR_RADIUS_M;
        let (m1, m2) = (s1.mass().value(), s2.mass().value());
        assert!(m2 <= m1);
        assert!((s1.radius().value() / lobe(m1 / m2) - 1.0).abs() < 1e-12);
        assert!((s2.radius().value() / lobe(m2 / m1) - 1.0).abs() < 1e-12);
        let hours = pair.orbit().period().value() / 3_600.0;
        assert!((0.5..240.0).contains(&hours), "{:?}: {hours} h", entry.id());
    }
}

/// The scan keeps exactly each cell's entries, with their own marks, and neither the scan nor the
/// cells run the binary engine.
#[test]
fn the_scan_keeps_exactly_the_cells_entries() {
    let galaxy = galaxy();
    let process = StellarMergers::new(galaxy);
    let before = engine_calls();
    for (key, entries) in sample_entries() {
        let mut scanned = Vec::new();
        scan_merger_cell(galaxy, &process, *key, &mut scanned);
        let kept: Vec<_> = entries.iter().map(|e| *e.value().scan()).collect();
        assert_eq!(scanned, kept, "{key:?}");
        assert_eq!(&cell_entries(*key), entries);
    }
    assert_eq!(engine_calls(), before);
}

/// Under one candidate in ten that the site's thinning keeps fails the class's test or the scan's
/// main-sequence check (P11.T8).
#[test]
fn under_one_candidate_in_ten_fails_the_test() {
    let galaxy = galaxy();
    let process = StellarMergers::new(galaxy);
    let mut candidates = 0_u32;
    let mut failed = 0_u32;
    for key in sample_cells() {
        let bound = process.bound(galaxy, &key);
        for index in 0..candidate_count(galaxy.seed(), &key, bound) {
            let id = key.candidate_id(index, 0).unwrap();
            let site = process
                .hosts()
                .site(galaxy, &key, id, bound, tags::CLASS_MERGER_SITE);
            if site.is_none() {
                continue;
            }
            candidates += 1;
            let kept = process
                .candidate(galaxy, &key, id, bound)
                .and_then(StellarMergers::test);
            failed += u32::from(kept.is_none());
        }
    }
    println!("{failed} of {candidates} thinned candidates failed the test");
    assert!(candidates > 50, "{candidates}");
    assert!(
        f64::from(failed) < 0.1 * f64::from(candidates),
        "{failed} of {candidates}"
    );
}

/// The cells do not depend on the order they are generated in, nor on what was resolved between
/// them.
#[test]
fn cells_do_not_depend_on_order() {
    let galaxy = galaxy();
    let cells: Vec<CatalogueCellKey> = sample_cells().into_iter().take(4).collect();
    assert_order_independent(&cells, |key| {
        let stray =
            CatalogueSystemId::new(ClassId::CORE_COLLAPSE.value(), key.fine_cell(), 0, 0).unwrap();
        let _ = placement::resolve(galaxy, crate::id::SystemId::from(stray));
        cell_entries(*key)
    });
}

// 3. The systems.

/// An entry's stars at the epoch, through plan 03's resolve and `system_of`, are its own; its
/// record is counted in its primary's layer, and the primary's mass lies in it.
#[test]
fn an_entry_s_stars_are_its_own() {
    let galaxy = galaxy();
    for entry in all_entries().step_by(5) {
        let value = entry.value();
        let stars = value.stars(galaxy);
        assert_eq!(stars.star_count(), 2);
        assert_eq!(stars.record(), value.record());
        let system = system_of(galaxy, entry.id()).unwrap();
        assert_eq!(system.star_model(), None);
        assert_eq!(system.stars(galaxy), Some(stars));
        let band = crate::galaxy::imf::MassBand::ALL
            .into_iter()
            .find(|b| Layer::from(*b) == value.record().layer())
            .expect("a stellar layer");
        let m1 = value.scan().primary_mass().value();
        assert!(band.lo() <= m1 && m1 <= band.hi(), "{m1} in {band:?}");
    }
}

/// A merger moves on one straight line at its drawn velocity, and stays within plan 09's
/// 4,096 ly reach of its cell over the source horizon.
#[test]
fn mergers_stay_within_the_reach() {
    let span_s = (CLOCK_WINDOW_H.as_seconds_f64() * 2.0) + LIGHT_CROSSING_L.as_seconds_f64();
    for entry in all_entries() {
        let value = entry.value();
        let v = value.scan().velocity().metres_per_second();
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        assert!(speed > 0.0);
        let travel_ly = speed * span_s / METRES_PER_LIGHT_YEAR;
        assert!(travel_ly < LIFETIME_REACH.value(), "{travel_ly} ly");
        assert_eq!(
            value.motion().position_at(UniverseTime::EPOCH),
            *value.scan().position()
        );
        let t = value.scan().merger_time();
        assert_eq!(value.scan().position_at(t), value.motion().position_at(t));
    }
}

/// The mergers' primaries by layer, against the share row's A 0.1, B 0.2, C 0.5, D 0.15 and
/// E 0.05: near the solar circle the old components hold no main-sequence primary in D or E, so
/// those layers' share falls to the young disc's mergers. Pinned as measured over the sample cells.
#[test]
fn the_mergers_layers_against_the_share_row() {
    let mut counts = [0_u32; 5];
    for entry in all_entries() {
        let layer = entry.value().record().layer();
        let slot = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
            .iter()
            .position(|&l| l == layer)
            .expect("a stellar layer");
        counts[slot] += 1;
    }
    println!("layers A–E: {counts:?}");
    let mut pins = hyperion_testkit::pins!(
        "galaxy/catalogue_classes/binary/merger",
        "the_mergers_layers_against_the_share_row"
    );
    pins.exact("layer_counts", counts);
}

// 4. Pins.

/// The first two sample cells pinned: each cell's bound, candidates and entries, and the first
/// [`GOLDEN_ENTRIES`] entries' scan marks, pair and merged star.
#[test]
fn stellar_merger_golden() {
    let galaxy = galaxy();
    let process = StellarMergers::new(galaxy);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (key, entries) in sample_entries().iter().take(2) {
        let bound = process.bound(galaxy, key);
        let label = format!("cell {:?}", key.cell());
        w.f64(&format!("{label} bound"), bound.value());
        w.line(&format!(
            "{label} candidates {} entries {}",
            candidate_count(galaxy.seed(), key, bound),
            entries.len()
        ));
        for entry in entries.iter().take(GOLDEN_ENTRIES) {
            let value = entry.value();
            let scan = value.scan();
            let id = format!(
                "{label} {:016x}",
                crate::id::SystemId::from(entry.id()).raw()
            );
            w.line(&format!(
                "{id} component {} layer {:?}",
                scan.component().unwrap().index(),
                value.record().layer()
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
            w.f64(&format!("{id} primary"), scan.primary_mass().value());
            w.f64(&format!("{id} secondary"), scan.secondary_mass().value());
            for (name, star) in [
                ("primary", &value.pair().stars()[0]),
                ("secondary", &value.pair().stars()[1]),
                ("product", value.product()),
            ] {
                w.f64(&format!("{id} {name} mass"), star.mass().value());
                w.f64(&format!("{id} {name} radius"), star.radius().value());
                w.f64(
                    &format!("{id} {name} luminosity"),
                    star.luminosity().value(),
                );
                w.f64(&format!("{id} {name} tau"), star.phase_fraction());
            }
            let orbit = value.pair().orbit();
            w.f64(&format!("{id} period"), orbit.period().value());
            let o = orbit.orientation();
            w.f64(&format!("{id} inclination"), o.inclination().value());
            w.f64(&format!("{id} node"), o.ascending_node().value());
            w.f64(
                &format!("{id} periapsis"),
                o.argument_of_periapsis().value(),
            );
            w.f64(
                &format!("{id} mean anomaly"),
                orbit.mean_anomaly_at_epoch().value(),
            );
        }
    }
    golden!("catalogue_classes/stellar_mergers", w.as_str());
}

/// The entries of each golden cell whose marks are pinned.
const GOLDEN_ENTRIES: usize = 12;

/// The density of mergers in the horizon at the Sun-like point, per cubic parsec, printed beside
/// the galaxy's total for the record; the class follows its share row.
#[test]
fn the_local_density_of_mergers() {
    let galaxy = galaxy();
    let sun = PointLy::from(&GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap());
    let density = StellarMergers::new(galaxy)
        .hosts()
        .hosts_density(galaxy, &sun)
        .value();
    let ly_per_pc = crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
    let per_pc3 = density * ly_per_pc * ly_per_pc * ly_per_pc;
    println!("{per_pc3:e} mergers in the horizon per pc^3 at the Sun-like point");
    assert!(per_pc3 > 0.0);
}
