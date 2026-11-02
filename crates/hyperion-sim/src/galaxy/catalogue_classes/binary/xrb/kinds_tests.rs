//! The tests of the three kinds the carving names (P11.T8.b's fix), under every 27 hold: version
//! 27's table weighs all nine kinds the census counts and the class draws them in its shares; and,
//! through a table that draws the three alone, each new kind's cells are the same in a fresh galaxy
//! and in any order, its entries are their class at every age of the horizon, the scan keeps
//! exactly them without the engine, the test thins at most one candidate in ten, and their marks
//! hold their kinds' figures.

use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

use super::*;
use crate::galaxy::catalogue_classes::binary::BinaryHost;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::{CatalogueClassCell, CatalogueEntry};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::stellar::binary::scan::{engine_calls, scan_cell, ultracompact_donor_mass};
use crate::stellar::binary::xrb_class::XRB_MIN_TRANSFER_RATE;
use crate::stellar::binary::{
    BinaryClass, BinaryParams, HmxbKind, SYMBIOTIC_XRB_MIN_LUMINOSITY, XrbKind, classify,
    wind_fed_luminosity, xrb_line,
};
use crate::tables::binary::{SamplerPick, XRB_SAMPLER, XRB_SAMPLER_26, XRB_SAMPLER_27};
use crate::time::SourceHorizon;
use crate::units::consts::SECONDS_PER_DAY;

/// The seed of the fixture the tests share, the class's own tests' (`tests`).
const SEED: u64 = 0x0b17_08b0_0000_0001;

/// The black holes' greatest share of the young disc's high-mass hosts: 0.01, main's bound, Cyg X-1
/// the one confirmed among about 150 catalogued high-mass X-ray binaries (Fortin et al. 2023, A&A
/// 671, A149; recalled).
const MAX_BLACK_HOLE_SHARE: f64 = 0.01;

/// The carving's three kinds.
const NEW: [XrbHostKind; 3] = [
    XrbHostKind::Ultracompact,
    XrbHostKind::Symbiotic,
    XrbHostKind::BlackHoleHighMass,
];

/// A table of version 27's marks that draws the carving's three kinds alone, half each of the
/// low-mass side and all of the high-mass side, so that their hosts fill the sample cells.
static NEW_KINDS: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    XRB_SAMPLER_27.marks(),
    &[
        SamplerPick::new("low_mass_kind", &[0.0, 0.0, 0.0, 0.0, 0.5, 0.5]),
        SamplerPick::new("high_mass_kind", &[0.0, 0.0, 0.5]),
    ],
);

/// The fixture under every 27 hold, with its kinematic tables, so that hosts move; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential()
    })
}

/// The class's process in the fixture, over version 27's table.
fn process() -> &'static XrayBinaries {
    static PROCESS: OnceLock<XrayBinaries> = OnceLock::new();
    PROCESS.get_or_init(|| XrayBinaries::new(galaxy()))
}

/// The class's process in the fixture over [`NEW_KINDS`].
fn new_kinds() -> &'static XrayBinaries {
    static PROCESS: OnceLock<XrayBinaries> = OnceLock::new();
    PROCESS.get_or_init(|| XrayBinaries::of_table(galaxy(), &NEW_KINDS))
}

/// The class cell `cell`.
fn key(cell: [i32; 3]) -> CatalogueCellKey {
    CatalogueCellKey::new(ClassId::XRAY_BINARY, cell).expect("a cell of the root cube")
}

/// The sample cells, the class's own tests' (`tests`): a block about the centre and a band across
/// the inner disc, where the young disc's high-mass hosts are; in and just below the plane.
fn sample_cells() -> Vec<CatalogueCellKey> {
    let centre = (-4..4).flat_map(|x| (-4..4).flat_map(move |y| [[x, y, 0], [x, y, -1]]));
    let band = (-12..12).flat_map(|x| (10..14).flat_map(move |y| [[x, y, 0], [x, y, -1]]));
    centre.chain(band).map(key).collect()
}

/// The sample cells' entries of `process`.
fn entries_of(process: &XrayBinaries) -> Vec<(CatalogueCellKey, Vec<CatalogueEntry<XrbEntry>>)> {
    sample_cells()
        .into_iter()
        .map(|key| {
            let cell = CatalogueClassCell::generate(galaxy(), process, key);
            (key, cell.into_entries())
        })
        .collect()
}

/// The sample cells' entries over [`NEW_KINDS`], generated once.
fn new_kind_entries() -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<XrbEntry>>)] {
    type Cells = Vec<(CatalogueCellKey, Vec<CatalogueEntry<XrbEntry>>)>;
    static CELLS: OnceLock<Cells> = OnceLock::new();
    CELLS.get_or_init(|| entries_of(new_kinds()))
}

/// Every sample entry over [`NEW_KINDS`] of `kind`.
fn new_entries_of(kind: XrbHostKind) -> impl Iterator<Item = &'static XrbEntry> {
    new_kind_entries()
        .iter()
        .flat_map(|(_, e)| e)
        .map(CatalogueEntry::value)
        .filter(move |e| e.scan().kind() == kind)
}

/// The weight of `kind` in `table`'s choice of its side over the choice's weights: its share of its
/// side's hosts; zero for a kind past the choice's weights.
fn side_share(table: &ClassSamplerTable<XrbMark, XrbPick>, kind: XrbHostKind) -> f64 {
    let (pick, kinds): (_, &[XrbHostKind]) = if kind.is_high_mass() {
        (XrbPick::HighMassKind, &XrbHostKind::HIGH_MASS)
    } else {
        (XrbPick::LowMassKind, &XrbHostKind::LOW_MASS)
    };
    let weights = table.pick(pick).weights();
    let place = kinds
        .iter()
        .position(|&k| k == kind)
        .expect("a side's kind");
    weights
        .get(place)
        .map_or(0.0, |w| w / weights.iter().sum::<f64>())
}

// 1. Version 27's table and its kinds.

/// From 27 the class draws from version 27's table, which weighs all nine kinds: the kinds'
/// expected hosts are their weights' shares of their side's hosts and sum to the class's total, so
/// that no host the row counts goes undrawn, and the black holes are at most 0.01 of the young
/// disc's high-mass hosts. Below 27 the class's table weighs the first six alone, and the
/// carving's kinds have no hosts.
#[test]
fn from_27_the_class_draws_every_kind_its_row_counts() {
    let (galaxy, process) = (galaxy(), process());
    assert_eq!(process.sampler().table(), &XRB_SAMPLER_27);
    assert_eq!(XRB_SAMPLER_27.marks(), XRB_SAMPLER_26.marks());
    let weights = |pick| XRB_SAMPLER_27.pick(pick).weights();
    assert_eq!(
        weights(XrbPick::LowMassKind).len(),
        XrbHostKind::LOW_MASS.len()
    );
    assert_eq!(
        weights(XrbPick::HighMassKind).len(),
        XrbHostKind::HIGH_MASS.len()
    );
    let sum = weights(XrbPick::LowMassKind)
        .iter()
        .chain(weights(XrbPick::HighMassKind))
        .sum::<f64>();
    assert!((sum - 1.0).abs() < 1e-12, "{sum}");
    let total = process.expected_hosts(galaxy);
    let by_kind = process.expected_hosts_by_kind(galaxy);
    println!("expected hosts by kind from 27: {by_kind:?} of {total}");
    assert!((by_kind.iter().sum::<f64>() / total - 1.0).abs() < 1e-12);
    for (kind, hosts) in XrbHostKind::ALL.into_iter().zip(by_kind) {
        let side = if kind.is_high_mass() {
            XrbSide::HighMass
        } else {
            XrbSide::LowMass
        };
        let share = side_share(&XRB_SAMPLER_27, kind);
        assert!(share > 0.0, "{kind:?}");
        let of_side = hosts / process.expected_hosts_on(galaxy, side);
        assert!((of_side - share).abs() < 1e-12, "{kind:?}");
    }
    let black_holes = side_share(&XRB_SAMPLER_27, XrbHostKind::BlackHoleHighMass);
    println!("black holes {black_holes:.4} of the high-mass hosts");
    assert!(black_holes <= MAX_BLACK_HOLE_SHARE, "{black_holes}");
    let version = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let held = XrayBinaries::new(&version);
    assert_eq!(held.sampler().table(), &XRB_SAMPLER);
    for kind in NEW {
        assert!(held.expected_hosts_of(&version, kind) <= 0.0, "{kind:?}");
    }
}

/// The kinds the class draws in the sample cells are its table's: each side's entries of each kind
/// inside the Poisson interval of its weight's share of the side's entries, at α = 10⁻³ over the
/// nine kinds.
#[test]
fn the_drawn_kinds_follow_their_weights() {
    let entries = entries_of(process());
    for (high_mass, kinds, pick) in [
        (false, &XrbHostKind::LOW_MASS[..], XrbPick::LowMassKind),
        (true, &XrbHostKind::HIGH_MASS[..], XrbPick::HighMassKind),
    ] {
        let of_side: Vec<XrbHostKind> = entries
            .iter()
            .flat_map(|(_, e)| e)
            .map(|e| e.value().scan().kind())
            .filter(|k| k.is_high_mass() == high_mass)
            .collect();
        let weights = XRB_SAMPLER_27.pick(pick).weights();
        let sum = weights.iter().sum::<f64>();
        #[expect(clippy::cast_precision_loss, reason = "a few thousand entries")]
        let n = of_side.len() as f64;
        for (kind, w) in kinds.iter().zip(weights) {
            let count = of_side.iter().filter(|k| *k == kind).count();
            let expected = n * w / sum;
            println!("{kind:?}: {count} drawn, {expected:.1} expected");
            assert_poisson_count(
                &format!("{kind:?}"),
                u64::try_from(count).expect("a few thousand entries"),
                expected,
                ALPHA / 9.0,
            );
        }
    }
}

/// The new kinds' cells are the same in a fresh galaxy of the same seed, and do not depend on the
/// order they are generated in.
#[test]
fn the_new_kinds_cells_are_the_same_in_a_fresh_galaxy_and_any_order() {
    let fresh = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_27_hold(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
    .with_full_potential();
    let process = XrayBinaries::of_table(&fresh, &NEW_KINDS);
    let keys: Vec<CatalogueCellKey> = sample_cells().into_iter().step_by(7).collect();
    for key in &keys {
        let again = CatalogueClassCell::generate(&fresh, &process, *key).into_entries();
        let first = &new_kind_entries()
            .iter()
            .find(|(k, _)| k == key)
            .expect("a sample cell")
            .1;
        assert_eq!(&again, first, "{key:?}");
    }
    assert_order_independent(&keys, |key| {
        CatalogueClassCell::generate(&fresh, &process, *key).into_entries()
    });
}

// 2. The new kinds' entries, through a table that draws them alone.

/// Every entry of the carving's kinds is an X-ray binary by `carved_class` and its scan's class by
/// `classify` at 17 ages across the source horizon, as every host is (the shared test).
#[test]
fn every_new_kind_entry_is_its_class_at_every_age() {
    let start = SourceHorizon::START.since_epoch().as_julian_years_f64();
    let end = SourceHorizon::END.since_epoch().as_julian_years_f64();
    let mut counts = [0_usize; 3];
    for (k, kind) in NEW.into_iter().enumerate() {
        for entry in new_entries_of(kind) {
            counts[k] += 1;
            let now = entry.record().age_at_epoch();
            let timeline = entry.timeline();
            assert_eq!(carved_class(timeline, now), Some(CarvedClass::XrayBinary));
            let class = entry.scan().class();
            for step in 0..=16 {
                let t = start + (end - start) * f64::from(step) / 16.0;
                let at = Years::new(now.value() + t);
                let found = classify(&timeline.state_at(at), &timeline.class_context(at));
                assert_eq!(found, class, "{kind:?} at {t} years");
            }
        }
    }
    println!("entries: {counts:?} ultracompact, symbiotic, black-hole high-mass");
    assert!(counts.iter().all(|&n| n >= 10), "{counts:?}");
}

/// The scan of the sample cells keeps exactly their entries of the carving's kinds, bit for bit,
/// without running the binary engine.
#[test]
fn the_scan_keeps_exactly_the_new_kinds_entries() {
    let before = engine_calls();
    for (key, entries) in new_kind_entries() {
        let mut hosts = Vec::new();
        scan_cell(galaxy(), new_kinds(), *key, &mut hosts);
        let kept: Vec<_> = entries.iter().map(|e| *e.value().scan()).collect();
        assert_eq!(hosts, kept, "{key:?}");
    }
    assert_eq!(engine_calls(), before);
}

/// Of the candidates the site keeps, the scan and the test together refuse at most one in ten of
/// each new kind (P11.T8's bound), as measured: an ultracompact host fed under the floor, and a
/// symbiotic host whose neutron star takes too little of its giant's wind.
#[test]
fn at_most_one_new_kind_candidate_in_ten_is_refused() {
    let (galaxy, process) = (galaxy(), new_kinds());
    let mut tried = [0_u32; 3];
    let mut refused = [0_u32; 3];
    for key in sample_cells() {
        let bound = process.bound(galaxy, &key);
        for index in 0..candidate_count(galaxy.seed(), &key, bound) {
            let id = key.candidate_id(index, 0).expect("a candidate's ID");
            let Some((site, component, host_side)) = process.site(galaxy, &key, id, bound) else {
                continue;
            };
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
            let k = NEW
                .iter()
                .position(|&each| each == kind)
                .expect("a new kind");
            tried[k] += 1;
            let kept = process
                .draw_candidate(galaxy, &key, id, bound)
                .and_then(XrayBinaries::test_candidate);
            refused[k] += u32::from(kept.is_none());
        }
    }
    println!("refused {refused:?} of {tried:?}");
    for k in 0..3 {
        assert!(tried[k] >= 10, "{:?}: {}", NEW[k], tried[k]);
        assert!(
            f64::from(refused[k]) <= 0.1 * f64::from(tried[k]),
            "{:?}: {} of {}",
            NEW[k],
            refused[k],
            tried[k]
        );
    }
}

/// An ultracompact host's period lies in 10–80 minutes; its donor is a helium white dwarf filling
/// its Roche lobe at the mass the period gives it; it is fed at or above the floor by gravitational
/// radiation, held to its Eddington rate; and it is a transient, taking a recurrence, exactly when
/// it is fed below the irradiated disc's line.
#[test]
fn ultracompact_hosts_lie_at_10_to_80_minutes_and_fill_their_lobes() {
    for entry in new_entries_of(XrbHostKind::Ultracompact) {
        let (scan, binary) = (entry.scan(), entry.binary());
        let minutes = scan.period().value() / 60.0;
        assert!((10.0..=80.0).contains(&minutes), "{minutes}");
        let donor = binary.donor();
        assert_eq!(donor.phase(), Phase::HeliumWhiteDwarf);
        assert_eq!(binary.accretor().phase(), Phase::NeutronStar);
        assert_eq!(
            donor.mass(),
            ultracompact_donor_mass(scan.period(), scan.accretor_mass())
        );
        let lobe = roche_lobe_radius(
            donor.mass().value() / scan.accretor_mass().value(),
            binary.orbit().semi_major_axis(),
        )
        .value()
            / SOLAR_RADIUS_M;
        assert!((donor.radius().value() / lobe - 1.0).abs() < 1e-12);
        let rate = scan.transfer_rate().expect("a host in stable transfer");
        assert!(rate >= XRB_MIN_TRANSFER_RATE, "{rate:?}");
        let line = xrb_line(
            scan.accretor_mass(),
            donor.mass(),
            binary.orbit().semi_major_axis(),
        );
        assert_eq!(
            scan.is_transient(),
            rate < line,
            "{rate:?} against {line:?}"
        );
        let kind = if scan.is_transient() {
            XrbKind::Transient
        } else {
            XrbKind::Persistent
        };
        assert_eq!(scan.class(), BinaryClass::LowMassXrayBinary(kind));
    }
}

/// A symbiotic host's neutron star takes its giant's wind at or above the symbiotic X-ray binaries'
/// floor, as `classify` reads it; the giant lies inside its Roche lobe on a period of 200–3,000
/// days, and the pair is detached.
#[test]
fn every_symbiotic_accretor_is_at_or_above_the_floor() {
    for entry in new_entries_of(XrbHostKind::Symbiotic) {
        let (scan, binary) = (entry.scan(), entry.binary());
        let days = scan.period().value() / SECONDS_PER_DAY;
        assert!((200.0..=3_000.0).contains(&days), "{days}");
        assert_eq!(binary.accretor().phase(), Phase::NeutronStar);
        let giant = binary.donor();
        assert_eq!(giant.phase(), Phase::FirstGiantBranch);
        assert!((1.0..=3.0).contains(&giant.mass().value()));
        assert_eq!(scan.transfer_rate(), None);
        let separation = binary.orbit().semi_major_axis();
        let lobe = roche_lobe_radius(
            giant.mass().value() / scan.accretor_mass().value(),
            separation,
        )
        .value()
            / SOLAR_RADIUS_M;
        assert!(giant.radius().value() < lobe);
        let accretor = binary.accretor();
        let luminosity = wind_fed_luminosity(
            (accretor.mass(), accretor.radius()),
            giant,
            separation,
            &BinaryParams::default(),
        );
        assert!(luminosity >= SYMBIOTIC_XRB_MIN_LUMINOSITY, "{luminosity:?}");
        assert_eq!(
            scan.class(),
            BinaryClass::LowMassXrayBinary(XrbKind::Symbiotic)
        );
    }
}

/// A black-hole high-mass host is a black hole of 15–30 M☉ fed through its inner Lagrangian point
/// by a supergiant of 20–60 M☉ 2–10 days out, a supergiant X-ray binary displaced from its site by
/// its recoil.
#[test]
fn black_hole_high_mass_hosts_are_fed_by_supergiants() {
    for entry in new_entries_of(XrbHostKind::BlackHoleHighMass) {
        let (scan, binary) = (entry.scan(), entry.binary());
        assert_eq!(binary.accretor().phase(), Phase::BlackHole);
        assert!((15.0..=30.0).contains(&scan.accretor_mass().value()));
        assert!((20.0..=60.0).contains(&binary.donor().mass().value()));
        let days = scan.period().value() / SECONDS_PER_DAY;
        assert!((2.0..=10.0).contains(&days), "{days}");
        assert!(scan.transfer_rate().is_some());
        assert_eq!(
            scan.class(),
            BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant)
        );
        let recoil = scan.recoil().expect("a catalogue high-mass host's recoil");
        assert_eq!(
            *scan.position(),
            scan.site()
                .translated(recoil.displacement())
                .expect("a host within its reach")
        );
    }
}
