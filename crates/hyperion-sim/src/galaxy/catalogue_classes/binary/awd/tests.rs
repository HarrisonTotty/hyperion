//! The accreting white dwarf classes' tests (P11.T8.a): their share rows and bounds, their split at
//! L, their entries against the shared test at every age of the horizon, the scan against the
//! cells, the test's thinning, resolution and order independence, the stars, the reach, a slow
//! host's pinned eruptions, the local density and a golden. The galaxy totals over seeds, the
//! sampler against the engine's members and a scan of 10⁶ hosts are slow tests
//! (`tests/binary_awd.rs`).

use std::sync::OnceLock;

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::pins::Tolerance;

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, CatalogueEntry, check_class_headroom, resolve_entry, system_of,
};
use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::id::SystemId;
use crate::stellar::binary::BinaryClass;
use crate::stellar::binary::scan::{engine_calls, scan_cell};
use crate::tables::binary::{
    AWD_FAST_SAMPLER, AWD_SLOW_SAMPLER, CLASS_SHARES, MILKY_WAY_FORMED_MASS,
};
use crate::time::{SourceHorizon, UniverseTime};
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The seed of the Milky Way fixture the tests share.
const SEED: u64 = 0x0b17_08a0_0000_0001;

/// The fixture, with its kinematic tables, so that hosts move; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential()
    })
}

/// The class cell `cell` of `side`.
fn key(side: AwdSide, cell: [i32; 3]) -> CatalogueCellKey {
    CatalogueCellKey::new(side.class(), cell).expect("a cell of the root cube")
}

/// The sample cells: a run along x through the solar circle, in and just below the plane.
fn sample_cells(side: AwdSide) -> Vec<CatalogueCellKey> {
    (-3..3)
        .flat_map(|x| [key(side, [x, 50, 0]), key(side, [x, 50, -1])])
        .collect()
}

/// The sample cells' entries of `side`, generated once.
fn sample_entries(side: AwdSide) -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<AwdEntry>>)] {
    type Cells = Vec<(CatalogueCellKey, Vec<CatalogueEntry<AwdEntry>>)>;
    static FAST: OnceLock<Cells> = OnceLock::new();
    static SLOW: OnceLock<Cells> = OnceLock::new();
    let generate = || {
        sample_cells(side)
            .into_iter()
            .map(|key| (key, cell_entries(key)))
            .collect()
    };
    match side {
        AwdSide::Fast => FAST.get_or_init(generate),
        AwdSide::Slow => SLOW.get_or_init(generate),
    }
}

/// The entries of cell `key`, by its class's process.
fn cell_entries(key: CatalogueCellKey) -> Vec<CatalogueEntry<AwdEntry>> {
    let galaxy = galaxy();
    match key.class() {
        ClassId::ACCRETING_WHITE_DWARF => {
            CatalogueClassCell::generate(galaxy, &AccretingWdFast::new(galaxy), key).into_entries()
        }
        ClassId::ACCRETING_WHITE_DWARF_SLOW => {
            CatalogueClassCell::generate(galaxy, &AccretingWdSlow::new(galaxy), key).into_entries()
        }
        other => panic!("{other} is no accreting white dwarf class"),
    }
}

/// Every sample entry of both classes.
fn all_entries() -> impl Iterator<Item = &'static CatalogueEntry<AwdEntry>> {
    [AwdSide::Fast, AwdSide::Slow]
        .into_iter()
        .flat_map(|side| sample_entries(side).iter().flat_map(|(_, e)| e))
}

// 1. The share rows and the bounds.

/// Each class and the features' lists together hold its share row's hosts, the catalogue all but
/// the features' share (P11.T8.f): at the Milky Way fixture the fitted rows (P15.T10.b's refit at
/// 27) give the fast class 0.47 × 10⁶ and the slow 7.6 × 10⁶ (pinned as measured), together
/// inside the brainstorm's 6–12 × 10⁶ (the scratch rows' 4.5 × 10⁶ each); and each class draws
/// over all its recurrence ranks, [0, 1], from its own fitted sampler, whose recurrence lies all on
/// its side of L (the scratch classes shared one sampler, split at L in halves).
#[test]
fn each_class_holds_its_share_row_s_hosts() {
    let galaxy = galaxy();
    let formed = galaxy
        .fields()
        .components()
        .iter()
        .fold(0.0, |sum, c| sum + c.count())
        * galaxy.mean_formed_mass().value();
    assert!((formed / MILKY_WAY_FORMED_MASS - 1.0).abs() < 1e-9);
    let fast = AwdProcess::new(galaxy, AwdSide::Fast);
    let slow = AwdProcess::new(galaxy, AwdSide::Slow);
    let mut rows = [0.0; 2];
    for (k, (side, process)) in [(AwdSide::Fast, &fast), (AwdSide::Slow, &slow)]
        .into_iter()
        .enumerate()
    {
        let row = CLASS_SHARES.row(side.carved());
        let fields = galaxy.fields();
        // The row's hosts, before the features' lists take their share.
        let by_row: f64 = process
            .components()
            .iter()
            .map(|c| {
                let component = fields.component(c.id());
                row.hosts_per_formed_mass(component.population())
                    * galaxy.mean_formed_mass().value()
                    * component.ages().born_fraction()
                    * component.count_with_unborn()
            })
            .sum();
        let kept: f64 = process
            .components()
            .iter()
            .map(|c| {
                let component = fields.component(c.id());
                c.hosts_per_system() * component.count_with_unborn()
                    / catalogue_share(galaxy, row, component)
            })
            .sum();
        let hosts = process.expected_hosts(galaxy);
        println!(
            "{side:?}: the row's {by_row:.6e} hosts, the catalogue's {hosts:.6e} ({:.4} of them)",
            hosts / by_row
        );
        assert!(
            (kept / by_row - 1.0).abs() < 1e-9,
            "{side:?}: {kept} {by_row}"
        );
        assert!(hosts < by_row && hosts > 0.9 * by_row, "{side:?}: {hosts}");
        rows[k] = by_row;
        assert_eq!(
            process.components().len(),
            galaxy.fields().components().len()
        );
    }
    println!("the rows' hosts {rows:?}");
    let mut pins = hyperion_testkit::pins!(
        "galaxy/catalogue_classes/binary/awd",
        "each_class_holds_its_share_row_s_hosts"
    );
    pins.within("row_hosts", rows, Tolerance::relative(1e-9));
    assert!((6.0e6..=12.0e6).contains(&(rows[0] + rows[1])), "{rows:?}");
    for process in [&fast, &slow] {
        assert_eq!(process.recurrence_ranks(), (0.0, 1.0));
    }
}

/// A cell's bound holds the class's intensity at every point tried in it, in the disc and at the
/// centre, and the centre's cell, the fullest, leaves the index its headroom.
#[test]
fn the_bound_holds_the_intensity_and_the_index_has_room() {
    use hyperion_testkit::lcg::Lcg;
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x0b17_08a0_00b0);
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let process = AwdProcess::new(galaxy, side);
        let mut largest = 0.0_f64;
        for cell in [[0, 0, 0], [-1, -1, -1], [0, 50, 0], [3, 50, -1], [0, 20, 0]] {
            let key = key(side, cell);
            let bound = process.bound(galaxy, &key).value();
            largest = largest.max(bound);
            for _ in 0..200 {
                let words = [(); 3].map(|()| lcg.next_u64());
                let p = PointLy::from(&key.position_from_words(words));
                let density = process.hosts_density(galaxy, &p).value();
                assert!(density <= bound, "{cell:?}: {density} > {bound}");
            }
        }
        assert_eq!(
            check_class_headroom(side.class(), PerCubicLightYear::new(largest)),
            Ok(())
        );
    }
}

/// No fitted accreting sampler draws a white dwarf at or above the Chandrasekhar mass, which
/// `NovaCurve::of` refuses and `nova_source` would refuse: the white dwarf mass's limits and its
/// draws at every rank, over the recurrence's span, are a nova's, under
/// [`ACCRETING_WHITE_DWARF_MAX_MASS`](crate::tables::binary::ACCRETING_WHITE_DWARF_MAX_MASS). The
/// refit at 27 before P11.T4's `dwarf_limit` drew the slow side's to 2.12 M☉ (train 27 found it).
#[test]
fn no_fitted_white_dwarf_reaches_the_chandrasekhar_mass() {
    use crate::stellar::binary::light_curves::NovaCurve;
    use crate::tables::binary::ACCRETING_WHITE_DWARF_MAX_MASS;
    for (side, table) in [
        (AwdSide::Fast, &AWD_FAST_SAMPLER),
        (AwdSide::Slow, &AWD_SLOW_SAMPLER),
    ] {
        let sampler = ClassSampler::new(table);
        let (lo, hi) = table.mark(AwdMark::WhiteDwarfMass).limits();
        assert!(hi <= ACCRETING_WHITE_DWARF_MAX_MASS, "{side:?}: {hi}");
        let (first, last) = table.mark(AwdMark::Recurrence).limits();
        for i in 0..=64 {
            let condition = first + (last - first) * f64::from(i) / 64.0;
            for j in 0..=64 {
                let mass =
                    sampler.value_at(AwdMark::WhiteDwarfMass, condition, f64::from(j) / 64.0);
                assert!(
                    NovaCurve::of(crate::units::SolarMasses::new(mass)).is_ok(),
                    "{side:?} at {condition}: {mass} M_sun"
                );
            }
        }
        println!("{side:?}: white dwarfs of {lo}–{hi} M_sun");
    }
}

// 2. The entries.

/// The sample cells hold entries of both classes, and each resolves by its ID to itself, through
/// the class's process, the production lookup and plan 03's resolve. The fitted fast row gives the
/// solar circle a tenth of the scratch row's hosts (P15.T10.b's refit at 27), so the sample holds
/// a dozen fast hosts and under two hundred slow ones.
#[test]
fn entries_resolve_to_themselves() {
    let galaxy = galaxy();
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let entries: Vec<_> = sample_entries(side).iter().flat_map(|(_, e)| e).collect();
        println!("{side:?}: {} entries", entries.len());
        let least = match side {
            AwdSide::Fast => 10,
            AwdSide::Slow => 50,
        };
        assert!(entries.len() >= least, "{side:?}: {}", entries.len());
        for entry in entries {
            let member = match side {
                AwdSide::Fast => {
                    resolve_entry(galaxy, &AccretingWdFast::new(galaxy), entry.id()).unwrap()
                }
                AwdSide::Slow => {
                    resolve_entry(galaxy, &AccretingWdSlow::new(galaxy), entry.id()).unwrap()
                }
            };
            assert_eq!(member.entry(), entry);
            let system = system_of(galaxy, entry.id()).unwrap();
            assert_eq!(system.record(), entry.value().record());
            assert_eq!(
                placement::resolve(galaxy, SystemId::from(entry.id())),
                Ok(*entry.value().record())
            );
        }
    }
}

/// Every entry passes the shared test with its own class (plan 11's design note 10), and at every
/// age of the source horizon its pair is the same accreting white dwarf: a white dwarf fed by its
/// donor, a cataclysmic variable or a symbiotic star.
#[test]
fn every_entry_is_its_class_at_every_age_of_the_horizon() {
    let start = SourceHorizon::START.since_epoch().as_julian_years_f64();
    let end = SourceHorizon::END.since_epoch().as_julian_years_f64();
    for entry in all_entries() {
        let value = entry.value();
        let class = ClassId::from_value(entry.id().class()).unwrap();
        let side = if class == ClassId::ACCRETING_WHITE_DWARF {
            AwdSide::Fast
        } else {
            AwdSide::Slow
        };
        let now = value.record().age_at_epoch();
        let timeline = value.timeline();
        assert_eq!(carved_class(timeline, now), Some(side.carved()));
        for k in 0..=16 {
            let t = start + (end - start) * f64::from(k) / 16.0;
            let found = timeline.class_at(Years::new(now.value() + t));
            assert!(
                matches!(
                    found,
                    BinaryClass::CataclysmicVariable(_) | BinaryClass::Symbiotic
                ),
                "{:?} at {t} years: {found:?}",
                entry.id()
            );
        }
    }
}

/// The two classes split the hosts at L: every fast host recurs within L and lists no eruption,
/// every slow host beyond it with at most two eruptions, inside the source horizon; and the rank
/// split partitions the sampler.
#[test]
fn the_two_classes_split_the_hosts_at_l() {
    let l = LIGHT_CROSSING_L.as_julian_years_f64();
    let horizon = crate::events::TimeWindow::new(SourceHorizon::START, SourceHorizon::END).unwrap();
    let mut slow_eruptions = [0_u32; 3];
    for side in [AwdSide::Fast, AwdSide::Slow] {
        for (_, entries) in sample_entries(side) {
            for entry in entries {
                let scan = entry.value().scan();
                assert_eq!(scan.side(), side);
                match side {
                    AwdSide::Fast => {
                        assert!(scan.recurrence().value() <= l);
                        assert_eq!(scan.eruptions(), &[] as &[UniverseTime]);
                        let mut events = Vec::new();
                        scan.eruptions_in(horizon, &mut events);
                        assert!(!events.is_empty(), "a fast host erupts in L + 2H");
                    }
                    AwdSide::Slow => {
                        assert!(scan.recurrence().value() > l);
                        let times = scan.eruptions();
                        assert!(times.len() <= 2);
                        assert!(times.iter().all(|&t| horizon.contains(t)));
                        assert!(times.windows(2).all(|w| w[0] < w[1]));
                        slow_eruptions[times.len()] += 1;
                    }
                }
            }
        }
    }
    // Most slow hosts miss the horizon; a few erupt in it.
    assert!(slow_eruptions[0] > slow_eruptions[1], "{slow_eruptions:?}");
    assert!(slow_eruptions[1] > 0, "{slow_eruptions:?}");
}

/// The scan keeps exactly each cell's entries, with their own marks, and neither the scan nor the
/// cells run the binary engine.
#[test]
fn the_scan_keeps_exactly_the_cells_entries() {
    let galaxy = galaxy();
    let before = engine_calls();
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let process = AwdProcess::new(galaxy, side);
        for (key, entries) in sample_entries(side) {
            let mut hosts = Vec::new();
            scan_cell(galaxy, &process, *key, &mut hosts);
            let scanned: Vec<_> = hosts.iter().collect();
            let kept: Vec<_> = entries.iter().map(|e| e.value().scan()).collect();
            assert_eq!(scanned, kept, "{key:?}");
            let fresh = cell_entries(*key);
            assert_eq!(&fresh, entries);
        }
    }
    assert_eq!(engine_calls(), before);
}

/// Under one candidate in ten that the site's thinning keeps fails the class's test (P11.T8): the
/// scratch table's recurrences and masses never land on the other side of L.
#[test]
fn under_one_candidate_in_ten_fails_the_test() {
    let galaxy = galaxy();
    let mut candidates = 0_u32;
    let mut failed = 0_u32;
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let process = AwdProcess::new(galaxy, side);
        for key in sample_cells(side) {
            let bound = process.bound(galaxy, &key);
            for index in 0..candidate_count(galaxy.seed(), &key, bound) {
                let id = key.candidate_id(index, 0).unwrap();
                if process.site(galaxy, &key, id, bound).is_none() {
                    continue;
                }
                candidates += 1;
                let kept = process
                    .candidate(galaxy, &key, id, bound)
                    .and_then(|c| process.test(c));
                failed += u32::from(kept.is_none());
            }
        }
    }
    println!("{failed} of {candidates} thinned candidates failed the test");
    assert!(candidates > 100, "{candidates}");
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
    let cells: Vec<CatalogueCellKey> = sample_cells(AwdSide::Fast)
        .into_iter()
        .take(4)
        .chain(sample_cells(AwdSide::Slow).into_iter().take(4))
        .collect();
    assert_order_independent(&cells, |key| {
        let stray =
            CatalogueSystemId::new(ClassId::CORE_COLLAPSE.value(), key.cell(), 0, 0).unwrap();
        let _ = placement::resolve(galaxy, SystemId::from(stray));
        cell_entries(*key)
    });
}

// 3. The systems.

/// An entry's stars are its pair: two stars, whose states at the epoch are the marked white dwarf
/// and donor, on the marked orbit, the pair of its class; plan 03's resolve and `system_of` give
/// the same stars; and its record is counted in layer C, D or E (`the_hosts_layers_against_the_share_rows`).
#[test]
fn an_entry_s_stars_are_its_pair() {
    let galaxy = galaxy();
    for entry in all_entries().step_by(7) {
        let value = entry.value();
        let stars = value.stars(galaxy);
        assert_eq!(stars.star_count(), 2);
        assert_eq!(stars.record(), value.record());
        let now = stars.state_at(UniverseTime::EPOCH).unwrap();
        assert_eq!(now.stars()[0], *value.binary().white_dwarf());
        assert_eq!(now.stars()[1], *value.binary().donor());
        assert!(matches!(
            now.pairs()[0].class(),
            BinaryClass::CataclysmicVariable(_) | BinaryClass::Symbiotic
        ));
        assert_eq!(now.pairs()[0].orbit(), Some(value.binary().orbit()));
        let system = system_of(galaxy, entry.id()).unwrap();
        assert_eq!(system.star_model(), None);
        assert_eq!(system.stars(galaxy), Some(stars));
        assert!(matches!(
            value.record().layer(),
            crate::id::Layer::C | crate::id::Layer::D | crate::id::Layer::E
        ));
    }
}

/// A host moves on one straight line at its drawn velocity, and stays within plan 09's 4,096 ly
/// reach of its cell over the source horizon.
#[test]
fn hosts_stay_within_the_reach() {
    let span_s = (CLOCK_WINDOW_H.as_seconds_f64() * 2.0) + LIGHT_CROSSING_L.as_seconds_f64();
    for entry in all_entries() {
        let value = entry.value();
        let v = value.scan().velocity().metres_per_second();
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let travel_ly = speed * span_s / METRES_PER_LIGHT_YEAR;
        assert!(travel_ly < LIFETIME_REACH.value(), "{travel_ly} ly");
        assert_eq!(
            value.motion().position_at(UniverseTime::EPOCH),
            *value.scan().position()
        );
        assert_eq!(
            value.motion().velocity_at(SourceHorizon::START),
            value.scan().velocity()
        );
    }
}

/// The hosts' primaries by layer, against the fitted rows' (P15.T10.b's refit at 27): fast C
/// 0.108, D 0.868 and E 0.024, slow C 0.378, D 0.608 and E 0.014, the engine's members whose
/// primary of 8 M☉ or more left a white dwarf in the pair among them (the scratch rows' C 0.75 and D
/// 0.25 alone). Pinned as measured over the sample cells.
#[test]
fn the_hosts_layers_against_the_share_rows() {
    let mut counts = [0_u32; 3];
    for entry in all_entries() {
        match entry.value().record().layer() {
            crate::id::Layer::C => counts[0] += 1,
            crate::id::Layer::D => counts[1] += 1,
            crate::id::Layer::E => counts[2] += 1,
            other => panic!("a host's primary in layer {other:?}"),
        }
    }
    let all = f64::from(counts.iter().sum::<u32>());
    println!(
        "layers C, D and E {counts:?}: {:.3} in D, {:.3} in E",
        f64::from(counts[1]) / all,
        f64::from(counts[2]) / all
    );
    let mut pins = hyperion_testkit::pins!(
        "galaxy/catalogue_classes/binary/awd",
        "the_hosts_layers_against_the_share_rows"
    );
    pins.exact("layer_counts", counts);
}

// 4. Pins.

/// One slow host's eruptions in the source horizon, pinned to the nanosecond: the first slow host
/// of the sample cells that erupts in it, its ID pinned too, and its eruptions as seconds and
/// nanoseconds from the epoch. P11.T9's nova listing inherits it.
#[test]
fn a_slow_host_s_eruptions_are_pinned() {
    let entry = sample_entries(AwdSide::Slow)
        .iter()
        .flat_map(|(_, e)| e)
        .find(|e| !e.value().scan().eruptions().is_empty())
        .expect("a sample slow host erupts in the horizon");
    let scan = entry.value().scan();
    let times: Vec<(i64, u32)> = scan
        .eruptions()
        .iter()
        .map(|t| (t.seconds(), t.subsec_nanos()))
        .collect();
    println!(
        "{:016x}: recurrence {} years, eruptions {times:?}",
        SystemId::from(entry.id()).raw(),
        scan.recurrence().value()
    );
    let mut pins = hyperion_testkit::pins!(
        "galaxy/catalogue_classes/binary/awd",
        "a_slow_host_s_eruptions_are_pinned"
    );
    pins.exact("host", SystemId::from(entry.id()).raw());
    pins.exact("eruptions", times);
}

/// The local density of both classes' hosts at the Sun-like point, the plan's test against Pala et
/// al. 2020, 4.8 × 10⁻⁶ pc⁻³ within 30%, and the fixture's catalogue total, pinned as the fitted
/// rows give them (P15.T10.b's refit at 27, each donor kind scaled to its observed figure): the
/// density 5.28 × 10⁻⁶, 1.10 of Pala et al.'s, now within its 30% (the scratch rows' 6.48 × 10⁻⁶
/// missed it), and the catalogue's 8.02 × 10⁶ hosts, the features' lists holding the rest of the
/// rows' (the scratch rows' 9 × 10⁶ in all).
#[test]
fn the_local_density_and_the_milky_way_s_total() {
    let galaxy = galaxy();
    let sun = PointLy::from(&GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap());
    let density: f64 = [AwdSide::Fast, AwdSide::Slow]
        .into_iter()
        .map(|side| {
            AwdProcess::new(galaxy, side)
                .hosts_density(galaxy, &sun)
                .value()
        })
        .sum::<f64>()
        * LIGHT_YEARS_PER_PARSEC
        * LIGHT_YEARS_PER_PARSEC
        * LIGHT_YEARS_PER_PARSEC;
    let total: f64 = [AwdSide::Fast, AwdSide::Slow]
        .into_iter()
        .map(|side| AwdProcess::new(galaxy, side).expected_hosts(galaxy))
        .sum();
    println!(
        "{density:e} pc^-3 at the Sun-like point, {:.3} of Pala et al.'s 4.8e-6; {total:e} hosts",
        density / 4.8e-6
    );
    assert!((density / 4.8e-6 - 1.0).abs() < 0.3, "{density}");
    // The catalogue's hosts of both classes, and the local density, pc⁻³, as measured.
    let mut pins = hyperion_testkit::pins!(
        "galaxy/catalogue_classes/binary/awd",
        "the_local_density_and_the_milky_way_s_total"
    );
    pins.within("catalogue_hosts", total, Tolerance::relative(1e-9));
    pins.within("local_density", density, Tolerance::relative(1e-6));
}

/// The first two sample cells of each class pinned: each cell's bound, candidates and entries, and
/// each entry's scan marks, binary and history.
#[test]
fn accreting_white_dwarf_golden() {
    let galaxy = galaxy();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for side in [AwdSide::Fast, AwdSide::Slow] {
        let process = AwdProcess::new(galaxy, side);
        for (key, entries) in sample_entries(side).iter().take(2) {
            let bound = process.bound(galaxy, key);
            let label = format!("{} cell {:?}", side.class(), key.cell());
            w.f64(&format!("{label} bound"), bound.value());
            w.line(&format!(
                "{label} candidates {} entries {}",
                candidate_count(galaxy.seed(), key, bound),
                entries.len()
            ));
            for entry in entries {
                let value = entry.value();
                let scan = value.scan();
                let binary = value.binary();
                let history = value.history();
                let id = format!("{label} {:016x}", SystemId::from(entry.id()).raw());
                w.line(&format!(
                    "{id} component {} donor {:?}",
                    scan.component().unwrap().index(),
                    binary.donor_kind()
                ));
                let p = scan.position();
                w.line(&format!("{id} cell {:?}", p.cell().to_array()));
                for (axis, metres) in p.offset_metres().iter().enumerate() {
                    w.f64(&format!("{id} offset {axis}"), *metres);
                }
                for (axis, v) in scan.velocity().metres_per_second().iter().enumerate() {
                    w.f64(&format!("{id} velocity {axis}"), *v);
                }
                w.f64(
                    &format!("{id} white dwarf"),
                    scan.white_dwarf_mass().value(),
                );
                w.f64(&format!("{id} rate"), scan.transfer_rate().value());
                w.f64(&format!("{id} recurrence"), scan.recurrence().value());
                w.line(&format!(
                    "{id} clock origin {} eruptions {:?}",
                    scan.clock().origin(),
                    scan.eruptions()
                ));
                w.f64(&format!("{id} donor"), binary.donor().mass().value());
                for (name, star) in [
                    ("white dwarf", binary.white_dwarf()),
                    ("donor", binary.donor()),
                ] {
                    w.f64(&format!("{id} {name} radius"), star.radius().value());
                    w.f64(
                        &format!("{id} {name} luminosity"),
                        star.luminosity().value(),
                    );
                }
                let orbit = binary.orbit();
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
                w.f64(
                    &format!("{id} primary initial"),
                    history.primary_initial_mass().value(),
                );
                w.f64(
                    &format!("{id} donor initial"),
                    history.donor_initial_mass().value(),
                );
                w.f64(&format!("{id} age"), history.age_at_epoch().value());
                w.f64(&format!("{id} feh"), value.composition().fe_h().value());
                // Each star's own model at the epoch, what the system's stars hold beside the
                // pair's timeline.
                let stars = value.stars(galaxy);
                for (k, model) in stars.stars().iter().enumerate() {
                    let state = model
                        .state_at(UniverseTime::EPOCH)
                        .expect("a star at the epoch");
                    w.line(&format!("{id} model {k} {:?}", state.phase()));
                    w.f64(&format!("{id} model {k} mass"), state.mass().value());
                    w.f64(
                        &format!("{id} model {k} luminosity"),
                        state.luminosity().value(),
                    );
                }
            }
        }
    }
    golden!("catalogue_classes/accreting_white_dwarfs", w.as_str());
}
