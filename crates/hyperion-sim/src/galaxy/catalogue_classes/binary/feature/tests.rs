//! The tests of P11.T8.f's hold, `FEATURE_BINARIES_VERSION`, and of plan 11's binary classes on
//! features' lists under every 27 hold.

use std::fmt::Write as _;

use hyperion_testkit::float::assert_same_bits;
use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::POPULATIONS;
use crate::galaxy::ages::SubDisc;
use crate::galaxy::catalogue_classes::binary::{AccretingWdFast, AccretingWdSlow};
use crate::galaxy::catalogue_classes::testing::assert_complementary;
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell};
use crate::galaxy::features::catalogue::FeatureCatalogue;
use crate::galaxy::features::ids::FeatureProcess;
use crate::galaxy::features::members::{
    FeatureInterior, FeatureLevelList, MemberRecord, NoInteriorCache, resolve_member,
};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::stellar::binary::scan::engine_calls;

/// The seed of the tests' Milky Way.
const SEED: u64 = 0x1108_f000_0000_0001;

/// The Milky Way at [`SEED`] under `rules`, with its kinematic tables, which a list needs.
fn milky_way(rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
        .unwrap()
        .with_full_potential()
}

/// The Milky Way under every 27 hold: the binary classes listed.
fn listed() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| milky_way(GalaxyRules::every_27_hold()))
}

/// The same galaxy under every 27 hold but this one: the held arm.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        milky_way(GalaxyRules {
            feature_binaries: FeatureBinariesRules::Unlisted,
            ..GalaxyRules::every_27_hold()
        })
    })
}

/// The processes the tests walk: the young disc's nurseries, two of the old thin disc's sub-discs'
/// open clusters, and the globulars.
const PROCESSES: [FeatureProcess; 4] = [
    FeatureProcess::Nursery,
    FeatureProcess::OldOpenCluster(SubDisc::First),
    FeatureProcess::OldOpenCluster(SubDisc::Fourth),
    FeatureProcess::Globular,
];

/// The first `n` features of each of [`PROCESSES`] in the catalogue's walk of `galaxy`.
fn features(galaxy: &Galaxy, n: usize) -> Vec<(FeatureProcess, FeatureRecord)> {
    PROCESSES
        .iter()
        .flat_map(|&process| {
            FeatureCatalogue::walk_process(galaxy, process)
                .take(n)
                .map(move |f| (process, f))
        })
        .collect()
}

/// The features' binary entries that their lists hold, with each list's index.
fn binary_entries(galaxy: &Galaxy, n: usize) -> Vec<(FeatureRecord, u16, ListedBinary)> {
    let mut out = Vec::new();
    for (_, feature) in features(galaxy, n) {
        let list = FeatureLevelList::of(galaxy, &feature);
        for entry in list.entries() {
            if let Some(binary) = list.binary_entry(galaxy, entry.index()) {
                out.push((feature, entry.index(), binary));
            }
        }
    }
    out
}

/// The listed galaxy's entries over the first 300 features of each process, drawn once.
fn the_entries() -> &'static [(FeatureRecord, u16, ListedBinary)] {
    static ENTRIES: OnceLock<Vec<(FeatureRecord, u16, ListedBinary)>> = OnceLock::new();
    ENTRIES.get_or_init(|| binary_entries(listed(), 300))
}

/// P11.T8.f is held until the 26 → 27 bump: the rules in force are version 26's and before, under
/// which no feature lists a binary class.
#[test]
fn the_feature_binaries_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < FEATURE_BINARIES_VERSION,
        "the 26 → 27 bump puts P11.T8.f on the record: features and the galactic centre list plan \
         11's binary classes, the catalogue's classes give up the features' share of their hosts, \
         and a feature's binary members are drawn inside its hard–soft boundary. Delete \
         `FEATURE_BINARIES_VERSION`, `FeatureBinariesRules` with its `Unlisted` arm, \
         `GalaxyRules::feature_binaries`, the `match`es that read it and this test, in the bump's \
         own checkpoint commit"
    );
    assert_eq!(
        GalaxyRules::of_version(GENERATOR_VERSION).feature_binaries,
        FeatureBinariesRules::Unlisted
    );
}

/// The held arm is version 26's: no feature registers a binary class, its list is plan 09's
/// classes alone, and the catalogue keeps every host of every component.
#[test]
fn the_held_arm_lists_no_binary_class_and_the_catalogue_keeps_every_host() {
    let (off, on) = (held(), listed());
    let mut listing = 0;
    for (_, feature) in features(on, 40) {
        assert_eq!(FeatureBinaries::of_feature(off, &feature, None), None);
        let list = FeatureLevelList::of(off, &feature);
        assert!(
            list.entries().all(|e| !is_listed_binary(e.class())),
            "{:?}",
            feature.id()
        );
        if FeatureBinaries::of_feature(on, &feature, None).is_some_and(|b| !b.is_empty()) {
            listing += 1;
        }
    }
    assert!(listing > 40, "{listing} features list a binary class");
    for component in off.fields().components() {
        for row in on.rules().binary_tables.class_shares().rows() {
            assert_same_bits(catalogue_share(off, row, component), 1.0);
            let kept = catalogue_share(on, row, component);
            assert!((0.0..=1.0).contains(&kept) && kept.is_finite(), "{kept}");
        }
    }
}

/// Registering the binary classes moves no entry of plan 09's: every recent entry of a feature's
/// list, its supernovae and its luminous blue variables, keeps its index, its class, its number,
/// its position and its member, and the binary classes' entries follow them.
#[test]
fn registering_the_binary_classes_leaves_every_recent_entry_where_it_was() {
    let (off, on) = (held(), listed());
    let mut recent = 0;
    let mut binaries = 0;
    for (_, feature) in features(on, 150) {
        let (before, after) = (
            FeatureLevelList::of(off, &feature),
            FeatureLevelList::of(on, &feature),
        );
        let kept: Vec<_> = before.entries().collect();
        let now: Vec<_> = after.entries().take(kept.len()).collect();
        assert_eq!(kept, now, "{:?}", feature.id());
        for entry in &kept {
            assert_eq!(
                before.recent_entry(off, entry.index()),
                after.recent_entry(on, entry.index())
            );
            recent += 1;
        }
        let rest: Vec<_> = after.entries().skip(kept.len()).collect();
        assert!(rest.iter().all(|e| is_listed_binary(e.class())));
        binaries += rest.len();
    }
    eprintln!("{recent} recent entries kept, {binaries} binary entries after them");
    assert!(recent > 0 && binaries > 0, "{recent} {binaries}");
}

/// The most entries of a catalogue cell the complementarity test reads.
const PER_CELL: usize = 150;

/// The catalogue's half of complementarity, inside features and on the class grids: every listed
/// entry, and the first entries of every class in a block of cells about the Sun-like point and
/// the inner disc, passes `carved_class` with its own class at every age of the source horizon, on
/// both sides of a merger's T.
#[test]
fn every_entry_is_its_own_class_at_every_age() {
    let galaxy = listed();
    let listed = assert_complementary(
        the_entries()
            .iter()
            .map(|(_, _, e)| (e.carved(), e.timeline(), e.record().age_at_epoch())),
    );
    let mut catalogue = 0;
    let cells = |class: ClassId, cells: &[[i32; 3]]| -> Vec<CatalogueCellKey> {
        cells
            .iter()
            .map(|&c| CatalogueCellKey::new(class, c).unwrap())
            .collect()
    };
    let near = [[0, 50, 0], [1, 50, 0], [0, 6, 0], [0, 0, 0], [1, 1, 0]];
    let (fast, slow) = (AccretingWdFast::new(galaxy), AccretingWdSlow::new(galaxy));
    for key in cells(ClassId::ACCRETING_WHITE_DWARF, &near) {
        let cell = CatalogueClassCell::generate(galaxy, &fast, key);
        catalogue += assert_complementary(cell.entries().iter().take(PER_CELL).map(|e| {
            let v = e.value();
            (
                CarvedClass::AccretingWdFast,
                v.timeline(),
                v.record().age_at_epoch(),
            )
        }));
    }
    for key in cells(ClassId::ACCRETING_WHITE_DWARF_SLOW, &near) {
        let cell = CatalogueClassCell::generate(galaxy, &slow, key);
        catalogue += assert_complementary(cell.entries().iter().take(PER_CELL).map(|e| {
            let v = e.value();
            (
                CarvedClass::AccretingWdSlow,
                v.timeline(),
                v.record().age_at_epoch(),
            )
        }));
    }
    let process = XrayBinaries::new(galaxy);
    for key in cells(ClassId::XRAY_BINARY, &near) {
        let cell = CatalogueClassCell::generate(galaxy, &process, key);
        catalogue += assert_complementary(cell.entries().iter().take(PER_CELL).map(|e| {
            let v = e.value();
            (
                CarvedClass::XrayBinary,
                v.timeline(),
                v.record().age_at_epoch(),
            )
        }));
    }
    let mergers = StellarMergers::new(galaxy);
    let nsm = NeutronStarMergers::new(galaxy);
    for xy in [[0, 6, 0], [0, 0, 0], [1, 0, 0], [0, 1, 0]] {
        let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, xy).unwrap();
        let cell = CatalogueClassCell::generate(galaxy, &mergers, key);
        catalogue += assert_complementary(cell.entries().iter().take(PER_CELL).map(|e| {
            let v = e.value();
            (
                CarvedClass::StellarMerger,
                v.timeline(),
                v.record().age_at_epoch(),
            )
        }));
        let key = CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, xy).unwrap();
        let cell = CatalogueClassCell::generate(galaxy, &nsm, key);
        catalogue += assert_complementary(cell.entries().iter().take(PER_CELL).map(|e| {
            let v = e.value();
            (
                CarvedClass::NeutronStarMerger,
                v.timeline(),
                v.record().age_at_epoch(),
            )
        }));
    }
    eprintln!("{listed} listed entries and {catalogue} catalogue entries");
    assert!(listed > 1_000 && catalogue > 100, "{listed} {catalogue}");
}

/// A listed entry is a feature member: its ID resolves through plan 09's `resolve_member` to the
/// member its list gives, of its class's member class and its feature's origin and motion, whose
/// stars are its class's marked pair, bit for bit.
#[test]
fn a_listed_entry_resolves_to_its_member_and_its_stars_are_its_entry_s() {
    let galaxy = listed();
    let mut by_class = [0_u32; 5];
    for (feature, index, entry) in the_entries().iter().step_by(7) {
        let id = FeatureMemberId::new(
            feature.id().feature_ref(),
            MemberSlot::FeatureLevel { index: *index },
        )
        .unwrap();
        let member: MemberRecord = resolve_member(galaxy, &NoInteriorCache, id).unwrap();
        assert_eq!(member.record(), entry.record());
        assert_eq!(member.listed_class(), Some(entry.class()));
        assert_eq!(member.class(), &entry.member_class());
        assert_eq!(member.class().multiplicity, Multiplicity::Binary);
        assert!(matches!(
            member.record().origin(),
            SystemOrigin::FeatureMember { attempt: 0, .. }
        ));
        assert_eq!(member.record().id(), SystemId::from(id));
        let stars = member.stars(galaxy);
        assert_eq!(stars, entry.stars(galaxy));
        assert_eq!(stars.star_count(), 2);
        assert_eq!(stars.carved_pair().map(|(_, c)| c), Some(entry.carved()));
        let i = LISTED_CLASSES
            .iter()
            .position(|&c| c == entry.class())
            .unwrap();
        by_class[i] += 1;
    }
    eprintln!("resolved by class {by_class:?}");
    assert!(
        by_class[0] > 0 && by_class[1] > 0 && by_class[2] > 0,
        "{by_class:?}"
    );
}

/// The lists' entries are drawn from their classes' samplers and scan marks alone: drawing every
/// entry of the walk's features runs the binary engine not once.
#[test]
fn drawing_the_lists_runs_no_engine() {
    let galaxy = listed();
    let before = engine_calls();
    let entries = binary_entries(galaxy, 120);
    assert_eq!(engine_calls(), before, "{} entries drawn", entries.len());
    assert!(entries.len() > 300, "{}", entries.len());
}

/// The scan marks of 10⁶ listed hosts, an old open cluster's accreting white dwarfs, are drawn
/// with no call into the binary engine.
#[test]
#[ignore = "slow: 10⁶ listed hosts' scan marks"]
fn a_million_listed_hosts_scan_marks_run_no_engine() {
    let galaxy = listed();
    let (_, feature) = features(galaxy, 40)
        .into_iter()
        .find(|(p, _)| matches!(p, FeatureProcess::OldOpenCluster(_)))
        .unwrap();
    let binaries = FeatureBinaries::of_feature(galaxy, &feature, None).unwrap();
    let process = AwdProcess::new(galaxy, AwdSide::Fast);
    // 10⁶ distinct hosts: every index of the feature-level slots of 123 features.
    let refs: Vec<FeatureRef> = FeatureCatalogue::walk_process(galaxy, FeatureProcess::Nursery)
        .take(123)
        .map(|f| f.id().feature_ref())
        .collect();
    let before = engine_calls();
    let mut hosts = 0_u32;
    let mut kept = 0_u32;
    'hosts: for feature_ref in refs {
        for index in 1..=8_191_u16 {
            let id = FeatureMemberId::new(feature_ref, MemberSlot::FeatureLevel { index }).unwrap();
            let host =
                BinaryHost::listed(SystemId::from(id), *feature.position(), *binaries.host());
            kept += u32::from(awd_marks_at(galaxy, &process, host).is_some());
            hosts += 1;
            if hosts == 1_000_000 {
                break 'hosts;
            }
        }
    }
    assert_eq!(hosts, 1_000_000);
    assert_eq!(engine_calls(), before);
    assert!(kept > 990_000, "{kept}");
}

/// Budgets: for each class and population, the catalogue's expected hosts and the features' share
/// of them, plan 09's φ by band weighted by the class's bands, return the class's budget, its row's
/// hosts per solar mass formed times the population's formed mass (the X-ray binaries' on their
/// side), to 10⁻⁶. The field holds none: the grid redraws them (P11.T7).
#[test]
fn the_catalogue_and_the_features_hold_each_class_s_budget() {
    let (off, on) = (held(), listed());
    let formed = on.mean_formed_mass().value();
    let mut report = String::new();
    for row in on.rules().binary_tables.class_shares().rows() {
        let carved = row.class();
        for population in POPULATIONS {
            let components: Vec<&Component> = on
                .fields()
                .components()
                .iter()
                .filter(|c| c.population() == population)
                .collect();
            let systems = |c: &Component| c.count_with_unborn() * c.ages().born_fraction();
            let rate = match carved {
                CarvedClass::XrayBinary => XrayBinaries::new(on).hosts_per_formed_mass(population),
                _ => row.hosts_per_formed_mass(population),
            };
            let budget: f64 = components.iter().map(|c| rate * formed * systems(c)).sum();
            if budget <= 0.0 {
                continue;
            }
            let catalogue = catalogue_hosts(on, carved, population);
            let features: f64 = components
                .iter()
                .map(|c| rate * formed * systems(c) * features_share(on, row, c))
                .sum();
            let whole = catalogue_hosts(off, carved, population);
            let closure = (catalogue + features) / budget - 1.0;
            writeln!(
                report,
                "{carved:?} {population:?}: budget {budget:.6e}, catalogue {catalogue:.6e}, \
                 features {features:.6e} ({:.4} of it), closure {closure:.1e}",
                features / budget
            )
            .unwrap();
            assert!(
                closure.abs() < 1e-6,
                "{carved:?} {population:?}: {closure:e}"
            );
            assert!(
                (whole / budget - 1.0).abs() < 1e-6,
                "{carved:?} {population:?}"
            );
        }
    }
    eprint!("{report}");
}

/// The catalogue's expected hosts of `class` among `population`'s components in `galaxy`, before
/// its test.
fn catalogue_hosts(galaxy: &Galaxy, class: CarvedClass, population: Population) -> f64 {
    let fields = galaxy.fields();
    let of = |id: ComponentId, hosts_per_system: f64| {
        let component = fields.component(id);
        if component.population() == population {
            hosts_per_system * component.count_with_unborn()
        } else {
            0.0
        }
    };
    match class {
        CarvedClass::AccretingWdFast | CarvedClass::AccretingWdSlow => {
            let side = if class == CarvedClass::AccretingWdFast {
                AwdSide::Fast
            } else {
                AwdSide::Slow
            };
            AwdProcess::new(galaxy, side)
                .components()
                .iter()
                .map(|c| of(c.id(), c.hosts_per_system()))
                .sum()
        }
        CarvedClass::XrayBinary => XrayBinaries::new(galaxy)
            .components()
            .iter()
            .map(|c| of(c.id(), c.hosts_per_system()))
            .sum(),
        CarvedClass::StellarMerger => StellarMergers::new(galaxy)
            .hosts()
            .components()
            .iter()
            .map(|c| of(c.id(), c.hosts_per_system()))
            .sum(),
        CarvedClass::NeutronStarMerger => NeutronStarMergers::new(galaxy)
            .hosts()
            .components()
            .iter()
            .map(|c| of(c.id(), c.hosts_per_system()))
            .sum(),
    }
}

/// The cells' half of complementarity inside features: a feature's binary member, which from
/// version 27 its stars redraw out of any carved class (`carve::redraws`), holds no carved pair, so
/// that a carved binary inside a feature is its list's and never its cells' too.
#[test]
fn a_feature_s_binary_members_hold_no_carved_pair() {
    let galaxy = listed();
    let mut checked = 0;
    let mut cell_members = Vec::new();
    for (process, feature) in features(galaxy, 30) {
        if process == FeatureProcess::Nursery {
            continue;
        }
        let Some(interior) = FeatureInterior::of(galaxy, &feature) else {
            continue;
        };
        'cells: for cell in interior.grid().owned_cells() {
            for band in [MassBand::A, MassBand::B, MassBand::C] {
                interior.members_in_cell(galaxy, band, cell, &mut cell_members);
                for (member, _) in &cell_members {
                    if member.class().multiplicity != Multiplicity::Binary {
                        continue;
                    }
                    assert_eq!(member.stars(galaxy).carved_pair(), None, "{member:?}");
                    checked += 1;
                    if checked % 40 == 0 {
                        break 'cells;
                    }
                }
            }
        }
    }
    assert!(checked > 100, "{checked}");
}

/// What the lists hold over the walk's features, by process and class: expected hosts and drawn
/// entries, printed for the record.
#[test]
fn the_lists_hold_their_expected_hosts() {
    let galaxy = listed();
    let mut report = String::new();
    for process in PROCESSES {
        let mut expected = [0.0; 5];
        let mut drawn = [0_u32; 5];
        for feature in FeatureCatalogue::walk_process(galaxy, process).take(300) {
            let Some(binaries) = FeatureBinaries::of_feature(galaxy, &feature, None) else {
                continue;
            };
            for &(class, n) in binaries.expected() {
                expected[LISTED_CLASSES.iter().position(|&c| c == class).unwrap()] += n;
            }
            for entry in FeatureLevelList::of(galaxy, &feature).binary_entries(galaxy) {
                drawn[LISTED_CLASSES
                    .iter()
                    .position(|&c| c == entry.class())
                    .unwrap()] += 1;
            }
        }
        writeln!(
            report,
            "{process:?}: expected {expected:.3?}, drawn {drawn:?}"
        )
        .unwrap();
        if process == FeatureProcess::Globular {
            // Old systems keep every white dwarf and X-ray binary their draws make.
            for i in 0..3 {
                let sigma = expected[i].sqrt();
                assert!(
                    (f64::from(drawn[i]) - expected[i]).abs() < 5.0 * sigma + 1.0,
                    "{process:?} {i}: {} of {}",
                    drawn[i],
                    expected[i]
                );
            }
        }
        if process == FeatureProcess::Nursery {
            // No white dwarf has formed in the young disc's nurseries.
            assert_eq!((drawn[0], drawn[1]), (0, 0), "{expected:?}");
        }
    }
    eprint!("{report}");
}
