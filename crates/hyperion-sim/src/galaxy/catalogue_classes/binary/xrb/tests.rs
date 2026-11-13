//! The X-ray binaries' class's tests (P11.T8.b): its share row, sides and bounds, the plan's
//! totals, its entries against the shared test and their kinds at every age of the horizon, the
//! scan against the cells, the test's thinning, resolution and order independence, the stars,
//! the recoil and the reach, the Be/X donors, the scratch table's claims against plan 06, and a
//! golden. The totals over seeds, the sampler against the engine's members and a scan of the
//! disc's hosts are slow tests (`tests/binary_xrb.rs`).

use std::sync::OnceLock;

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::pins::Tolerance;

use super::*;
use crate::galaxy::catalogue_classes::binary::{AccretingWdFast, AwdEntry};
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use crate::galaxy::catalogue_classes::testing::HIGH_MASS_XRB;
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, CatalogueEntry, check_class_headroom, resolve_entry, system_of,
};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::stellar::binary::classify;
use crate::stellar::binary::scan::{
    RATE_EXCESS_MARGIN_DEX, eddington_rate, engine_calls, scan_cell,
};
use crate::stellar::sse::lifetime as sse_lifetime;
use crate::tables::binary::{CLASS_SHARES, MILKY_WAY_FORMED_MASS, XRB_SAMPLER};
use crate::time::{SourceHorizon, UniverseTime};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
use crate::units::{Dex, HeliumExcess};

/// The seed of the Milky Way fixture the tests share.
const SEED: u64 = 0x0b17_08b0_0000_0001;

/// The fixture, with its kinematic tables, so that hosts move; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential()
    })
}

/// The class's process in the fixture.
fn process() -> &'static XrayBinaries {
    static PROCESS: OnceLock<XrayBinaries> = OnceLock::new();
    PROCESS.get_or_init(|| XrayBinaries::new(galaxy()))
}

/// The class cell `cell`.
fn key(cell: [i32; 3]) -> CatalogueCellKey {
    CatalogueCellKey::new(ClassId::XRAY_BINARY, cell).expect("a cell of the root cube")
}

/// The sample cells: a block about the centre, the old populations' densest, and a band across
/// the inner disc 5,100–7,200 ly out, where the young disc's high-mass hosts are; in and just
/// below the plane.
fn sample_cells() -> Vec<CatalogueCellKey> {
    let centre = (-4..4).flat_map(|x| (-4..4).flat_map(move |y| [[x, y, 0], [x, y, -1]]));
    let band = (-12..12).flat_map(|x| (10..14).flat_map(move |y| [[x, y, 0], [x, y, -1]]));
    centre.chain(band).map(key).collect()
}

/// The sample cells' entries, generated once.
fn sample_entries() -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<XrbEntry>>)] {
    type Cells = Vec<(CatalogueCellKey, Vec<CatalogueEntry<XrbEntry>>)>;
    static CELLS: OnceLock<Cells> = OnceLock::new();
    CELLS.get_or_init(|| {
        sample_cells()
            .into_iter()
            .map(|key| (key, cell_entries(key)))
            .collect()
    })
}

/// The entries of cell `key`.
fn cell_entries(key: CatalogueCellKey) -> Vec<CatalogueEntry<XrbEntry>> {
    CatalogueClassCell::generate(galaxy(), process(), key).into_entries()
}

/// Every sample entry.
fn all_entries() -> impl Iterator<Item = &'static CatalogueEntry<XrbEntry>> {
    sample_entries().iter().flat_map(|(_, e)| e)
}

/// The class's process in the fixture over [`HIGH_MASS_XRB`], which draws the high-mass kinds in
/// half its hosts: the refit at 27 leaves the sample cells none (P15.T10.b).
fn high_mass_process() -> &'static XrayBinaries {
    static PROCESS: OnceLock<XrayBinaries> = OnceLock::new();
    PROCESS.get_or_init(|| XrayBinaries::of_table(galaxy(), &HIGH_MASS_XRB))
}

/// The high-mass entries [`high_mass_process`] gives the sample cells, by cell, generated once.
fn high_mass_entries() -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<XrbEntry>>)] {
    type Cells = Vec<(CatalogueCellKey, Vec<CatalogueEntry<XrbEntry>>)>;
    static CELLS: OnceLock<Cells> = OnceLock::new();
    CELLS.get_or_init(|| {
        sample_cells()
            .into_iter()
            .map(|key| {
                let mut entries =
                    CatalogueClassCell::generate(galaxy(), high_mass_process(), key).into_entries();
                entries.retain(|e| e.value().scan().kind().is_high_mass());
                (key, entries)
            })
            .collect()
    })
}

/// Every sample entry, then every high-mass entry of [`high_mass_entries`].
fn with_high_mass() -> impl Iterator<Item = &'static CatalogueEntry<XrbEntry>> {
    all_entries().chain(high_mass_entries().iter().flat_map(|(_, e)| e))
}

/// A kind's place in [`XrbHostKind::ALL`].
fn place(kind: XrbHostKind) -> usize {
    XrbHostKind::ALL
        .iter()
        .position(|&k| k == kind)
        .expect("every kind is listed")
}

// 1. The share row, the sides and the bounds.

/// The plan's totals at Milky Way parameters: the class and the features' lists together hold its
/// share row's 10⁴ hosts (the plan's 0.7–1.5 × 10⁴), the catalogue all but the features' share
/// (P11.T8.f), of which the black-hole transients are 1,300 (the plan's 1,300 ± 400,
/// Corral-Santana et al. 2016) and the high-mass side its sampler's share, 0.0010 under the refit
/// at 27 (P15.T10.b), the scratch sampler's 0.16 before it; each side's components are its
/// populations', and the rate's ranks split at the line where the fitted law puts it.
#[test]
fn the_class_holds_its_share_row_s_hosts_and_the_plan_s_totals() {
    let galaxy = galaxy();
    let formed = galaxy
        .fields()
        .components()
        .iter()
        .fold(0.0, |sum, c| sum + c.count())
        * galaxy.mean_formed_mass().value();
    assert!((formed / MILKY_WAY_FORMED_MASS - 1.0).abs() < 1e-9);
    let process = process();
    let row = CLASS_SHARES.row(CarvedClass::XrayBinary);
    // Each side's hosts before the features' lists take their share: the row's.
    let row_hosts_on = |side: XrbSide| -> f64 {
        process
            .components()
            .iter()
            .filter(|c| c.side() == side)
            .map(|c| {
                let component = galaxy.fields().component(c.id());
                c.hosts_per_system() * component.count_with_unborn()
                    / catalogue_share(galaxy, row, component)
            })
            .sum()
    };
    let (low_row, high_row) = (
        row_hosts_on(XrbSide::LowMass),
        row_hosts_on(XrbSide::HighMass),
    );
    let row_total = low_row + high_row;
    let total = process.expected_hosts(galaxy);
    println!(
        "the row's {row_total:.1} hosts, the catalogue's {total:.1} ({:.4} of them)",
        total / row_total
    );
    assert!((row_total / 1.0e4 - 1.0).abs() < 1e-9, "{row_total}");
    assert!((7.0e3..=1.5e4).contains(&row_total));
    assert!(total < row_total && total > 0.99 * row_total, "{total}");
    let by_kind = process.expected_hosts_by_kind(galaxy);
    let summed: f64 = by_kind.iter().sum();
    assert!((summed / total - 1.0).abs() < 1e-12);
    println!("expected hosts by kind {by_kind:?}");
    let weights = process
        .sampler()
        .table()
        .pick(XrbPick::LowMassKind)
        .weights();
    let place_low = |kind| {
        XrbHostKind::LOW_MASS
            .iter()
            .position(|&k| k == kind)
            .expect("a low-mass kind")
    };
    let low_sum: f64 = weights.iter().sum();
    let black_hole_transients =
        low_row * weights[place_low(XrbHostKind::BlackHoleTransient)] / low_sum;
    assert!((black_hole_transients / 1_300.0 - 1.0).abs() < 1e-9);
    assert!((900.0..=1_700.0).contains(&black_hole_transients));
    let share = high_mass_share(process.sampler().table());
    assert!(
        (high_row / row_total - share).abs() < 1e-9,
        "{}",
        high_row / row_total
    );
    let high = process.expected_hosts_on(galaxy, XrbSide::HighMass);
    println!(
        "high-mass hosts: the row's {high_row:.1}, the catalogue's {high:.1}, a share of {share:.5}, \
         against Fortin et al.'s 152 catalogued"
    );
    // The high-mass side's share of the class's hosts under the refit at 27, its sampler's
    // high-mass weights over all its weights (P15.T10.b), pinned as measured: 0.0010 at 27, 10.2
    // hosts of the row's 10⁴, of which the catalogue holds 4.1 and the young features' lists the
    // rest, against Fortin et al.'s (2023, A&A 671, A149; recalled) 152 catalogued, a finding.
    let mut pins = hyperion_testkit::pins!(
        "galaxy/catalogue_classes/binary/xrb",
        "the_class_holds_its_share_row_s_hosts_and_the_plan_s_totals"
    );
    pins.within("high_mass_share", share, Tolerance::relative(1e-9));
    // The row alone, the same in every population, would give the young disc about 25.
    let fields = galaxy.fields();
    let young_by_row: f64 = process
        .components()
        .iter()
        .filter(|c| c.side() == XrbSide::HighMass)
        .map(|c| {
            let component = fields.component(c.id());
            row.hosts_per_formed_mass(component.population())
                * galaxy.mean_formed_mass().value()
                * component.ages().born_fraction()
                * component.count_with_unborn()
        })
        .sum();
    println!("the row alone gives the young disc {young_by_row:.1} hosts, the class {high_row:.1}");
    assert!((5.0..100.0).contains(&young_by_row), "{young_by_row}");
    for c in process.components() {
        let population = galaxy.fields().component(c.id()).population();
        assert_eq!(c.side(), XrbSide::of(population));
        assert!(c.hosts_per_system() > 0.0);
    }
    // The rank of the instability line in the fitted low-mass rate excess's law, as measured:
    // 0.944 at 27, the engine's low-mass members mostly transients.
    println!("the rate's ranks split at {}", process.rate_split());
    pins.within(
        "rate_split",
        process.rate_split(),
        Tolerance::absolute(1e-12),
    );
}

/// A cell's bound holds the class's intensity at every point tried in it, in the disc and at the
/// centre, and the centre's cell, the fullest, leaves the index its headroom.
#[test]
fn the_bound_holds_the_intensity_and_the_index_has_room() {
    use hyperion_testkit::lcg::Lcg;
    let galaxy = galaxy();
    let process = process();
    let mut lcg = Lcg::new(0x0b17_08b0_00b0);
    let mut largest = 0.0_f64;
    for cell in [[0, 0, 0], [-1, -1, -1], [0, 50, 0], [3, 12, -1], [0, 20, 0]] {
        let key = key(cell);
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
        check_class_headroom(ClassId::XRAY_BINARY, PerCubicLightYear::new(largest)),
        Ok(())
    );
}

// 2. The entries.

/// The sample cells hold hosts of every kind the class expects a hundred or more of in the galaxy,
/// and each entry resolves by its ID to itself, through the class's process, the production lookup
/// and plan 03's resolve. From 27 the class draws all nine kinds; the refit at 27 (P15.T10.b) gives
/// the catalogue under ten each of the other five, the black-hole persistent and symbiotic hosts
/// 4.4 and 2.8 and the high-mass kinds 4.1 together, too few for the sample to hold
/// ([`high_mass_entries`] holds the high-mass kinds).
#[test]
fn entries_resolve_to_themselves() {
    let galaxy = galaxy();
    let mut counts = [0_u32; XrbHostKind::ALL.len()];
    for entry in all_entries() {
        counts[place(entry.value().scan().kind())] += 1;
        let member = resolve_entry(galaxy, process(), entry.id()).unwrap();
        assert_eq!(member.entry(), entry);
        let system = system_of(galaxy, entry.id()).unwrap();
        assert_eq!(system.record(), entry.value().record());
        assert_eq!(
            placement::resolve(galaxy, SystemId::from(entry.id())),
            Ok(*entry.value().record())
        );
    }
    println!("sample hosts by kind {counts:?}");
    let expected = process().expected_hosts_by_kind(galaxy);
    for ((kind, n), hosts) in XrbHostKind::ALL.iter().zip(counts).zip(expected) {
        assert!(n > 0 || hosts < 100.0, "{kind:?}: {counts:?}");
    }
    assert!(counts.iter().filter(|&&n| n > 0).count() >= 4, "{counts:?}");
}

/// Every entry passes the shared test with its own class (plan 11's design note 10), and at every
/// age of the source horizon its pair is the class its kind names, as `classify` reads it; the
/// high-mass entries of [`high_mass_entries`] too.
#[test]
fn every_entry_is_its_kind_at_every_age_of_the_horizon() {
    let start = SourceHorizon::START.since_epoch().as_julian_years_f64();
    let end = SourceHorizon::END.since_epoch().as_julian_years_f64();
    for entry in with_high_mass() {
        let value = entry.value();
        let now = value.record().age_at_epoch();
        let timeline = value.timeline();
        assert_eq!(carved_class(timeline, now), Some(CarvedClass::XrayBinary));
        let class = value.scan().class();
        for k in 0..=16 {
            let t = start + (end - start) * f64::from(k) / 16.0;
            let at = Years::new(now.value() + t);
            let found = classify(&timeline.state_at(at), &timeline.class_context(at));
            assert_eq!(found, class, "{:?} at {t} years", entry.id());
        }
    }
}

/// The sides follow their populations: a high-mass host is the young disc's and carries its
/// recoil, which moved it from its site at the recoil over the years since its supernova; a
/// low-mass host is the old populations' and sits at its site; a transient alone has a
/// recurrence; a low-mass host's rate lies on its kind's side of its line. The high-mass hosts are
/// [`high_mass_entries`]', the sample cells holding none from 27.
#[test]
fn the_sides_follow_their_populations() {
    let galaxy = galaxy();
    let mut recoiled = 0_u32;
    for entry in with_high_mass() {
        let value = entry.value();
        let scan = value.scan();
        let kind = scan.kind();
        let population = galaxy
            .fields()
            .component(scan.component().unwrap())
            .population();
        assert_eq!(
            XrbSide::of(population).eq(&XrbSide::HighMass),
            kind.is_high_mass()
        );
        assert_eq!(value.record().population(), population);
        let transient = crate::stellar::binary::BinaryClass::LowMassXrayBinary(
            crate::stellar::binary::XrbKind::Transient,
        );
        assert_eq!(scan.recurrence().is_some(), scan.class() == transient);
        if let Some(recoil) = scan.recoil() {
            assert!(kind.is_high_mass());
            recoiled += 1;
            let moved = scan.site().displacement_to(scan.position());
            let expected = recoil.displacement();
            for (a, b) in moved.metres().iter().zip(expected.metres()) {
                assert!((a - b).abs() < 1.0e3 + 1e-12 * b.abs(), "{a} {b}");
            }
            // Within the table's years since a supernova of either high-mass mark: from 27 the
            // fitted Be/X ones, 1.5 × 10⁴ to 3.0 × 10⁷ years (the scratch table's 3 × 10⁵ to 2 ×
            // 10⁷), and the scratch supergiants', 3 × 10⁵ to 5 × 10⁶.
            let since = recoil.since_supernova().value();
            let (lo, hi) = [
                XrbMark::BexSinceSupernova,
                XrbMark::SupergiantSinceSupernova,
            ]
            .map(|mark| XRB_SAMPLER.mark(mark).limits())
            .into_iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (a, b)| {
                (lo.min(a), hi.max(b))
            });
            assert!(
                (math::exp10(lo) * (1.0 - 1e-12)..=math::exp10(hi) * (1.0 + 1e-12))
                    .contains(&since),
                "{since}"
            );
        } else {
            assert!(!kind.is_high_mass());
            assert_eq!(scan.position(), scan.site());
        }
        let binary = value.binary();
        assert_eq!(binary.transfer_rate(), scan.transfer_rate());
        if let XrbShape::LowMass { donor_mass, .. } = scan.shape() {
            let line = crate::stellar::binary::xrb_line(
                scan.accretor_mass(),
                donor_mass,
                binary.orbit().semi_major_axis(),
            );
            let excess = math::log10(binary.transfer_rate().unwrap().value() / line.value());
            // Held to the Eddington rate, but for a persistent host whose line lies above it.
            let eddington = eddington_rate(kind.accretor(), scan.accretor_mass()).value();
            let rate = binary.transfer_rate().unwrap().value();
            assert!(
                rate <= eddington * (1.0 + 1e-12)
                    || (!kind.is_transient() && line.value() > eddington),
                "{rate} against {eddington}"
            );
            if kind.is_transient() {
                assert!(excess < -0.5 * RATE_EXCESS_MARGIN_DEX, "{excess}");
            } else {
                assert!(excess > 0.5 * RATE_EXCESS_MARGIN_DEX, "{excess}");
            }
        }
    }
    assert!(recoiled > 20, "{recoiled} recoiled hosts");
}

/// The scan keeps exactly each cell's entries, with their own marks, and neither the scan nor the
/// cells run the binary engine.
#[test]
fn the_scan_keeps_exactly_the_cells_entries() {
    let galaxy = galaxy();
    let before = engine_calls();
    for (key, entries) in sample_entries() {
        let mut hosts = Vec::new();
        scan_cell(galaxy, process(), *key, &mut hosts);
        let scanned: Vec<_> = hosts.iter().collect();
        let kept: Vec<_> = entries.iter().map(|e| e.value().scan()).collect();
        assert_eq!(scanned, kept, "{key:?}");
        let fresh = cell_entries(*key);
        assert_eq!(&fresh, entries);
    }
    assert_eq!(engine_calls(), before);
}

/// No candidate that the scan keeps fails the class's test, and under one candidate in ten that
/// the site's thinning keeps is refused (P11.T8), as measured: from 27 the scan refuses a pair fed
/// under the floor (P11.T8.b's fix), which the class's test would refuse, among the transients the
/// fitted rate's tail takes far under their line and the ultracompact hosts (the finding of
/// `kinds_tests`' ultracompact pin), pinned by kind in [`XrbHostKind::ALL`]'s order with the
/// candidates the site keeps: at 27, 46 of 502, 22 neutron-star transients, 3 black-hole
/// transients and 21 ultracompact hosts.
#[test]
fn no_candidate_the_site_keeps_fails_the_test() {
    use crate::galaxy::catalogue_classes::binary::BinaryHost;
    let galaxy = galaxy();
    let process = process();
    let mut candidates = 0_u32;
    let mut failed = 0_u32;
    let mut refused = [0_u32; XrbHostKind::ALL.len()];
    for key in sample_cells() {
        let bound = process.bound(galaxy, &key);
        for index in 0..candidate_count(galaxy.seed(), &key, bound) {
            let id = key.candidate_id(index, 0).unwrap();
            let Some((site, component, host_side)) = process.site(galaxy, &key, id, bound) else {
                continue;
            };
            candidates += 1;
            let Some(candidate) = process.draw_candidate(galaxy, &key, id, bound) else {
                // The kind the scan drew, as it draws it.
                let host = BinaryHost::field(id, site, component);
                let mut stream = Stream::open(galaxy.seed(), tags::CLASS_XRB_KIND, host.key());
                let sampler = process.sampler();
                let kind = match host_side {
                    XrbSide::LowMass => {
                        XrbHostKind::LOW_MASS[sampler.pick(XrbPick::LowMassKind, &mut stream)]
                    }
                    XrbSide::HighMass => {
                        XrbHostKind::HIGH_MASS[sampler.pick(XrbPick::HighMassKind, &mut stream)]
                    }
                };
                refused[place(kind)] += 1;
                continue;
            };
            failed += u32::from(XrayBinaries::test_candidate(candidate).is_none());
        }
    }
    let total: u32 = refused.iter().sum();
    println!(
        "{failed} of {candidates} thinned candidates failed the test, {total} refused by the scan, \
         by kind {refused:?}"
    );
    assert!(candidates > 300, "{candidates}");
    assert_eq!(failed, 0);
    assert!(f64::from(total) < 0.1 * f64::from(candidates));
    let mut pins = hyperion_testkit::pins!(
        "galaxy/catalogue_classes/binary/xrb",
        "no_candidate_the_site_keeps_fails_the_test"
    );
    pins.exact("refused_by_kind", (refused, candidates));
}

/// The cells do not depend on the order they are generated in, nor on what was resolved between
/// them; and the accreting white dwarfs' cells are the same whatever X-ray binary was resolved
/// before them, as registering this class leaves P11.T8.a's entries (its golden is unchanged).
#[test]
fn cells_do_not_depend_on_order_and_leave_the_accreting_white_dwarfs() {
    let galaxy = galaxy();
    let cells: Vec<CatalogueCellKey> = sample_cells().into_iter().step_by(37).collect();
    assert_order_independent(&cells, |key| {
        let stray =
            CatalogueSystemId::new(ClassId::CORE_COLLAPSE.value(), key.cell(), 0, 0).unwrap();
        // Only to disturb what was generated before the cell; whether it resolves does not matter.
        let _ = placement::resolve(galaxy, SystemId::from(stray));
        cell_entries(*key)
    });
    let fast = AccretingWdFast::new(galaxy);
    let awd = |cell: [i32; 3]| -> Vec<CatalogueEntry<AwdEntry>> {
        let key = CatalogueCellKey::new(ClassId::ACCRETING_WHITE_DWARF, cell).unwrap();
        CatalogueClassCell::generate(galaxy, &fast, key).into_entries()
    };
    let before = awd([0, 12, 0]);
    for entry in all_entries().take(20) {
        // Only to resolve X-ray binaries between the two generations; each does, as tested above.
        let _ = placement::resolve(galaxy, SystemId::from(entry.id()));
    }
    assert_eq!(awd([0, 12, 0]), before);
}

// 3. The systems.

/// An entry's stars are its pair: two stars, whose states at the epoch are the marked compact
/// star and donor, on the marked orbit, the pair of its kind; plan 03's resolve and `system_of`
/// give the same stars; its record is counted in layer D or E, its compact star's progenitor's
/// (the refit at 27 draws progenitors from 3.8 M☉, the row's D 0.240 and E 0.760; the scratch
/// table's were all over 8 M☉); its donor's own model is living at the epoch but an ultracompact
/// host's, a white dwarf's progenitor's; and its compact star's progenitor's has left the same kind
/// of remnant in most hosts of layer E. Layer D's progenitors, whose own models leave white
/// dwarfs, made their neutron stars in the pair, and from 27 the fitted black holes' progenitors
/// reach down to 10.9 M☉ (the scratch table's 21–60 M☉), whose own models leave neutron stars.
#[test]
fn an_entry_s_stars_are_its_pair() {
    let galaxy = galaxy();
    // Layer D's hosts, then layer E's.
    let (mut agree, mut checked) = ([0_u32; 2], [0_u32; 2]);
    for entry in all_entries().step_by(5) {
        let value = entry.value();
        let stars = value.stars(galaxy);
        assert_eq!(stars.star_count(), 2);
        assert_eq!(stars.record(), value.record());
        let now = stars.state_at(UniverseTime::EPOCH).unwrap();
        assert_eq!(now.stars()[0], *value.binary().accretor());
        assert_eq!(now.stars()[1], *value.binary().donor());
        assert_eq!(now.pairs()[0].class(), value.scan().class());
        assert_eq!(now.pairs()[0].orbit(), Some(value.binary().orbit()));
        let system = system_of(galaxy, entry.id()).unwrap();
        assert_eq!(system.star_model(), None);
        assert_eq!(system.stars(galaxy), Some(stars.clone()));
        assert!(
            matches!(
                value.record().layer(),
                crate::id::Layer::D | crate::id::Layer::E
            ),
            "{:?}",
            value.record().layer()
        );
        let model = stars.stars()[0]
            .state_at(UniverseTime::EPOCH)
            .expect("a star at the epoch");
        let in_e = usize::from(value.record().layer() == crate::id::Layer::E);
        checked[in_e] += 1;
        agree[in_e] += u32::from(model.phase() == value.binary().accretor().phase());
        let donor = stars.stars()[1]
            .state_at(UniverseTime::EPOCH)
            .expect("a star at the epoch");
        // An ultracompact host's donor is a white dwarf, its own model its progenitor's, which has
        // left its main sequence (`XrayBinaries::draw_candidate`): a white dwarf too.
        let ultracompact = value.scan().kind() == XrbHostKind::Ultracompact;
        assert!(
            donor.phase().is_living() != ultracompact,
            "{:?}: {:?}",
            entry.id(),
            donor.phase()
        );
    }
    println!(
        "{agree:?} of {checked:?} progenitors' own models in layers D and E left the marked remnant"
    );
    assert!(checked[1] > 50, "{checked:?}");
    assert!(
        f64::from(agree[1]) >= 0.8 * f64::from(checked[1]),
        "{agree:?} of {checked:?}"
    );
}

/// A host moves on one straight line at its velocity, and stays within plan 09's 4,096 ly reach
/// of its cell over the source horizon, its recoil's displacement included, the high-mass hosts'
/// of [`high_mass_entries`] among them.
#[test]
fn hosts_stay_within_the_reach() {
    let span_s = (CLOCK_WINDOW_H.as_seconds_f64() * 2.0) + LIGHT_CROSSING_L.as_seconds_f64();
    let mut farthest = 0.0_f64;
    for (key, entries) in sample_entries().iter().chain(high_mass_entries()) {
        for entry in entries {
            let value = entry.value();
            let scan = value.scan();
            let speed = scan.velocity().speed().value();
            let travel_ly = speed * span_s / METRES_PER_LIGHT_YEAR;
            let half = 0.5 * f64::from(key.edge_ly());
            let centre =
                GalacticPosition::from_light_years(key.origin_ly().map(|o| f64::from(o) + half))
                    .unwrap();
            let from_cell =
                centre.displacement_to(scan.position()).length().value() / METRES_PER_LIGHT_YEAR;
            farthest = farthest.max(from_cell + travel_ly);
            assert!(
                from_cell + travel_ly < LIFETIME_REACH.value(),
                "{from_cell} + {travel_ly}"
            );
            assert_eq!(
                value.motion().position_at(UniverseTime::EPOCH),
                *scan.position()
            );
            assert_eq!(
                value.motion().velocity_at(SourceHorizon::START),
                scan.velocity()
            );
        }
    }
    println!("farthest reach {farthest:.0} ly");
}

/// The largest displacement a recoil can give: the table's fastest recoil over its longest time
/// since a supernova, the years held where the recoil reaches [`MAX_RECOIL_TRAVEL_LY`], at most
/// 3,000 ly for either high-mass kind. Unheld, the table refitted at 27 would carry a Be/X host
/// 5,940 ly (59.3 km/s for 3.0 × 10⁷ years), past plan 09's 4,096 ly reach.
///
/// [`MAX_RECOIL_TRAVEL_LY`]: crate::stellar::binary::scan::MAX_RECOIL_TRAVEL_LY
#[test]
fn the_recoil_s_largest_displacement_is_inside_the_reach() {
    use crate::stellar::binary::scan::MAX_RECOIL_TRAVEL_LY;
    let sampler = ClassSampler::new(&XRB_SAMPLER);
    for (speed, since) in [
        (XrbMark::BexRecoil, XrbMark::BexSinceSupernova),
        (XrbMark::SupergiantRecoil, XrbMark::SupergiantSinceSupernova),
    ] {
        let v = sampler.value_at(speed, 0.0, 1.0) * 1.0e3;
        let drawn = math::exp10(sampler.value_at(since, 0.0, 1.0)) * SECONDS_PER_JULIAN_YEAR;
        let t = drawn.min(MAX_RECOIL_TRAVEL_LY * METRES_PER_LIGHT_YEAR / v);
        let ly = v * t / METRES_PER_LIGHT_YEAR;
        println!(
            "{speed:?}: at most {ly:.0} ly, {:.0} unheld",
            v * drawn / METRES_PER_LIGHT_YEAR
        );
        assert!(ly <= MAX_RECOIL_TRAVEL_LY * (1.0 + 1e-12), "{ly}");
    }
}

/// A Be/X host's donor is plan 06's Be star by the draws its marks hold, a B star on its main
/// sequence; the attempts it takes to find them average about 36, plan 06's share of fast
/// rotators at its mass. The hosts are [`high_mass_entries`]', the sample cells holding none from
/// 27.
#[test]
fn be_x_donors_are_be_stars() {
    let rules = StellarRules::of_version(GENERATOR_VERSION);
    let mut attempts = Vec::new();
    for entry in with_high_mass() {
        let value = entry.value();
        if value.scan().kind() != XrbHostKind::BeX {
            continue;
        }
        let binary = value.binary();
        let draws = binary.draws(galaxy().seed(), value.record().id());
        assert!(is_be_donor(
            binary.donor(),
            value.composition(),
            &draws[1],
            rules
        ));
        assert_eq!(binary.donor().phase(), Phase::MainSequence);
        attempts.push(f64::from(binary.donor_attempt()));
    }
    #[expect(clippy::cast_precision_loss, reason = "a count of a few dozen")]
    let mean = attempts.iter().sum::<f64>() / attempts.len() as f64;
    println!("{} Be/X donors, mean attempt {mean:.1}", attempts.len());
    assert!(attempts.len() > 20);
    assert!((15.0..=70.0).contains(&mean), "{mean}");
}

/// A low-mass donor is a main-sequence star while its lobe is near its zero-age radius and an
/// evolved star beyond, whose core grows with its radius by Rappaport et al.'s relation; the
/// inversion gives the relation back.
#[test]
fn low_mass_donors_are_main_sequence_or_evolved_by_their_lobes() {
    for r in [0.6, 1.0, 2.0, 5.0, 10.0, 30.0, 100.0] {
        let core = giant_core_mass(SolarRadii::new(r)).value();
        let c4 = core * core * core * core;
        let back = 5_500.0 * c4 * math::powf_positive(core, 0.5) / (1.0 + 4.0 * c4) + 0.5;
        assert!((back / r - 1.0).abs() < 1e-9, "{r}: {core} -> {back}");
    }
    let comp = Composition::SOLAR;
    let age = Years::new(5.0e9);
    let ms = low_mass_donor_state(SolarMasses::new(0.4), SolarRadii::new(0.4), &comp, age);
    assert_eq!(ms.phase(), Phase::MainSequence);
    let sub = low_mass_donor_state(SolarMasses::new(0.8), SolarRadii::new(2.0), &comp, age);
    assert_eq!(sub.phase(), Phase::FirstGiantBranch);
    let giant = low_mass_donor_state(SolarMasses::new(0.8), SolarRadii::new(20.0), &comp, age);
    assert!(giant.core_mass() > sub.core_mass());
    assert!(giant.luminosity() > sub.luminosity());
    let t = giant.effective_temperature().value();
    assert!((3_000.0..=5_500.0).contains(&t), "{t}");
}

/// The table's progenitor ranges against plan 06's single stars, as its documentation states: at
/// [Fe/H] −0.5 to +0.3 with median draws, the neutron star's 9–17 M☉ leave neutron stars and the
/// black hole's 21–60 M☉ black holes; and a supergiant's core share is plan 06's at the start of
/// its Hertzsprung gap to 0.03.
#[test]
fn the_progenitor_ranges_leave_their_remnants_in_plan_06() {
    let draws = StarDraws::median();
    for feh in [-0.5, 0.0, 0.3] {
        let comp = Composition::from_fe_h(Dex::new(feh), HeliumExcess::ZERO);
        for (masses, phase) in [
            ([9.0, 12.0, 17.0], Phase::NeutronStar),
            ([21.0, 30.0, 60.0], Phase::BlackHole),
        ] {
            for m in masses {
                let age = Years::new(2.0e8);
                let state = Track::to_age(SolarMasses::new(m), &comp, &draws, age).state_at(age);
                assert_eq!(state.phase(), phase, "{m} M_sun at [Fe/H] {feh}");
            }
        }
    }
    let comp = Composition::SOLAR;
    for m in [14.0, 20.0, 25.0] {
        let mass = SolarMasses::new(m);
        let life = sse_lifetime(mass, &comp, &draws).value();
        let track = Track::to_age(mass, &comp, &draws, Years::new(life));
        let gap = (0..4_000)
            .map(|k| track.state_at(Years::new(life * f64::from(k) / 4_000.0)))
            .find(|s| s.phase() == Phase::HertzsprungGap)
            .expect("a massive star crosses the Hertzsprung gap");
        let share = gap.core_mass().value() / gap.mass().value();
        assert!(
            (share - SUPERGIANT_DONOR_CORE_SHARE).abs() < 0.03,
            "{m}: {share}"
        );
    }
}

// 4. The golden.

/// The first sample cells that hold hosts, with every kind the sample holds among their entries,
/// pinned: each cell's bound, candidates and entries, and each entry's scan marks, binary and
/// history. From 27 the sample holds four of the nine kinds (`entries_resolve_to_themselves`).
#[test]
fn xray_binary_golden() {
    let galaxy = galaxy();
    let process = process();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut held = [false; XrbHostKind::ALL.len()];
    for entry in all_entries() {
        held[place(entry.value().scan().kind())] = true;
    }
    let mut seen = [false; XrbHostKind::ALL.len()];
    for (key, entries) in sample_entries() {
        if entries.is_empty() || seen == held {
            continue;
        }
        let new_kind = entries
            .iter()
            .any(|e| !seen[place(e.value().scan().kind())]);
        if !new_kind {
            continue;
        }
        let bound = process.bound(galaxy, key);
        let label = format!("cell {:?}", key.cell());
        w.f64(&format!("{label} bound"), bound.value());
        w.line(&format!(
            "{label} candidates {} entries {}",
            candidate_count(galaxy.seed(), key, bound),
            entries.len()
        ));
        for entry in entries {
            seen[place(entry.value().scan().kind())] = true;
            pin_entry(&mut w, galaxy, &label, entry);
        }
    }
    assert_eq!(seen, held);
    assert!(held.iter().filter(|&&h| h).count() >= 4, "{held:?}");
    golden!("catalogue_classes/xray_binaries", w.as_str());
}

/// The lifetime the history reads is plan 06's, so that a high-mass host's age is its
/// progenitor's life and the years since its supernova, to rounding; over [`high_mass_entries`],
/// the sample cells holding no high-mass host from 27.
#[test]
fn a_high_mass_host_s_age_is_its_progenitor_s_life_and_its_recoil_s_years() {
    let galaxy = galaxy();
    let mut checked = 0_u32;
    for entry in with_high_mass() {
        let value = entry.value();
        let Some(recoil) = value.scan().recoil() else {
            continue;
        };
        let draws = StarDraws::for_star(galaxy.seed(), BodyId::new(value.record().id(), 0));
        let life = lifetime(
            track_mass_of(value.history().primary_initial_mass()),
            value.composition(),
            &draws,
        )
        .value();
        let age = value.history().age_at_epoch().value();
        assert!(
            (age - life - recoil.since_supernova().value()).abs() < 1e-6 * age,
            "{age} against {life}"
        );
        checked += 1;
    }
    assert!(checked > 20, "{checked}");
}

/// The same seed gives the same cells in two fresh galaxies and processes, so no state a galaxy or
/// a process holds lazily reaches an entry.
#[test]
fn the_same_seed_gives_the_same_cells_in_a_fresh_galaxy() {
    let fresh = || {
        let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential();
        let process = XrayBinaries::new(&galaxy);
        sample_cells()
            .into_iter()
            .step_by(23)
            .map(|key| CatalogueClassCell::generate(&galaxy, &process, key).into_entries())
            .collect::<Vec<_>>()
    };
    let first = fresh();
    assert!(first.iter().map(Vec::len).sum::<usize>() > 10);
    assert_eq!(first, fresh());
}

/// One entry's lines of the golden under `label`: its scan marks, binary, history and composition,
/// and each star's own model at the epoch, what the system's stars hold beside the pair's
/// timeline, with a Be/X donor's rotation by the draws its marks chose.
fn pin_entry(w: &mut GoldenWriter, galaxy: &Galaxy, label: &str, entry: &CatalogueEntry<XrbEntry>) {
    let value = entry.value();
    let scan = value.scan();
    let binary = value.binary();
    let history = value.history();
    let id = format!("{label} {:016x}", SystemId::from(entry.id()).raw());
    w.line(&format!(
        "{id} component {} kind {:?} donor attempt {}",
        scan.component().unwrap().index(),
        scan.kind(),
        binary.donor_attempt()
    ));
    let p = scan.position();
    w.line(&format!("{id} cell {:?}", p.cell().to_array()));
    for (axis, metres) in p.offset_metres().iter().enumerate() {
        w.f64(&format!("{id} offset {axis}"), *metres);
    }
    for (axis, v) in scan.velocity().metres_per_second().iter().enumerate() {
        w.f64(&format!("{id} velocity {axis}"), *v);
    }
    if let Some(recoil) = scan.recoil() {
        for (axis, v) in recoil.velocity().metres_per_second().iter().enumerate() {
            w.f64(&format!("{id} recoil {axis}"), *v);
        }
        w.f64(
            &format!("{id} since supernova"),
            recoil.since_supernova().value(),
        );
    }
    if let Some(recurrence) = scan.recurrence() {
        w.f64(&format!("{id} recurrence"), recurrence.value());
    }
    w.f64(&format!("{id} accretor"), scan.accretor_mass().value());
    w.f64(&format!("{id} period"), scan.period().value());
    if let Some(rate) = scan.transfer_rate() {
        w.f64(&format!("{id} rate"), rate.value());
    }
    for (name, star) in [("accretor", binary.accretor()), ("donor", binary.donor())] {
        w.line(&format!("{id} {name} {:?}", star.phase()));
        w.f64(&format!("{id} {name} mass"), star.mass().value());
        w.f64(&format!("{id} {name} core"), star.core_mass().value());
        w.f64(&format!("{id} {name} radius"), star.radius().value());
        w.f64(
            &format!("{id} {name} luminosity"),
            star.luminosity().value(),
        );
    }
    let orbit = binary.orbit();
    w.f64(&format!("{id} eccentricity"), orbit.eccentricity().value());
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
    // Each star's own model at the epoch, what the system's stars hold beside the pair's
    // timeline, and a Be/X donor's rotation by the draws its marks chose.
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
    if scan.kind() == XrbHostKind::BeX {
        let spin = stars.stars()[1]
            .rotation_at(UniverseTime::EPOCH)
            .expect("a Be star's model rotates at the epoch");
        w.f64(
            &format!("{id} donor model critical fraction"),
            spin.critical_fraction(),
        );
    }
}
