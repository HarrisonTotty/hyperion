//! The tests of plan 11's binary classes on features' lists (P11.T8.f) under every 27 hold.

use std::fmt::Write as _;

use hyperion_testkit::order::assert_order_independent;
use std::sync::OnceLock;

use super::*;
use crate::Seed;
use crate::galaxy::POPULATIONS;
use crate::galaxy::ages::SubDisc;
use crate::galaxy::catalogue_classes::binary::{AccretingWdFast, AccretingWdSlow};
use crate::galaxy::catalogue_classes::testing::assert_complementary;
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell};
use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureMarks};
use crate::galaxy::features::ids::FeatureProcess;
use crate::galaxy::features::members::{
    FeatureInterior, FeatureLevelList, MemberRecord, NoInteriorCache, resolve_member,
};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::stellar::binary::scan::engine_calls;
use crate::stellar::binary::testing::MemberCells;

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

/// Features register the binary classes, and the catalogue keeps a share of every component's
/// hosts in [0, 1], the rest the features'.
#[test]
fn features_list_the_binary_classes_and_the_catalogue_keeps_the_rest() {
    let on = listed();
    let mut listing = 0;
    for (_, feature) in features(on, 40) {
        if FeatureBinaries::of_feature(on, &feature, None).is_some_and(|b| !b.is_empty()) {
            listing += 1;
        }
    }
    assert!(listing > 40, "{listing} features list a binary class");
    for component in on.fields().components() {
        for row in CLASS_SHARES.rows() {
            let kept = catalogue_share(on, row, component);
            assert!((0.0..=1.0).contains(&kept) && kept.is_finite(), "{kept}");
        }
    }
}

/// The binary classes' entries follow plan 09's on every feature's list: its recent entries, its
/// supernovae and its luminous blue variables, come first, and every entry after them is of a
/// binary class.
#[test]
fn the_binary_classes_entries_follow_every_recent_entry() {
    let on = listed();
    let mut recent = 0;
    let mut binaries = 0;
    for (_, feature) in features(on, 150) {
        let list = FeatureLevelList::of(on, &feature);
        let kept: Vec<_> = list
            .entries()
            .take_while(|e| !is_listed_binary(e.class()))
            .collect();
        recent += kept.len();
        let rest: Vec<_> = list.entries().skip(kept.len()).collect();
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
/// with no call into the binary engine. Under every 27 hold the fast side's scan refuses a host
/// fed helium (P11.T5's fix, `AwdProcess::has_no_nova`), about the fitted fast donor kinds' 4.2%;
/// the rest it keeps but a few tenths of a per cent.
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
    let mut fed_helium = 0_u32;
    'hosts: for feature_ref in refs {
        for index in 1..=8_191_u16 {
            let id = FeatureMemberId::new(feature_ref, MemberSlot::FeatureLevel { index }).unwrap();
            let host =
                BinaryHost::listed(SystemId::from(id), *feature.position(), *binaries.host());
            if awd_marks_at(galaxy, &process, host).is_some() {
                kept += 1;
            } else {
                fed_helium += u32::from(process.has_no_nova(galaxy, host.key()));
            }
            hosts += 1;
            if hosts == 1_000_000 {
                break 'hosts;
            }
        }
    }
    println!("{kept} of {hosts} listed fast hosts kept, {fed_helium} refused as fed helium");
    assert_eq!(hosts, 1_000_000);
    assert_eq!(engine_calls(), before);
    assert!(kept + fed_helium > 990_000, "{kept} + {fed_helium}");
}

/// Budgets: for each class and population, the catalogue's expected hosts and the features' share
/// of them, plan 09's φ by band weighted by the class's bands, return the class's budget, its row's
/// hosts per solar mass formed times the population's formed mass (the X-ray binaries' on their
/// side), to 10⁻⁶. The field holds none: the grid redraws them (P11.T7).
#[test]
fn the_catalogue_and_the_features_hold_each_class_s_budget() {
    let on = listed();
    let formed = on.mean_formed_mass().value();
    let mut report = String::new();
    for row in CLASS_SHARES.rows() {
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

/// The globular clusters' listed X-ray binaries over the walk at 27, as measured: 366 against the
/// 419.9 expected, a finding (303 against 419.6 under the fit before train 27's engine; re-measured
/// on P15.T10.b's refit on it, and under the engine's raised segment cap, 367 before it). The floor refuses the ultracompact hosts the refit draws above
/// about an hour, the scan's cold white dwarf donor being half the engine's mass there
/// (`xrb::kinds_tests`' `PINNED_ULTRACOMPACT_ENTRIES`; for P11.T8.b).
const PINNED_GLOBULAR_X_RAY_BINARIES: u32 = 366;

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
            // Old systems keep every white dwarf their draws make, and the X-ray binaries but the
            // ultracompact hosts the floor refuses.
            assert_eq!(drawn[2], PINNED_GLOBULAR_X_RAY_BINARIES, "{expected:?}");
            for i in 0..2 {
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

/// The streams a draw of the engine's members is split between: sixteen, whatever the cores, as
/// the integration tests' (`tests/common/binaries.rs`).
const MEMBER_STREAMS: u64 = 16;

/// `n` of the engine's prior members of `class` near the solar circle, drawn on
/// [`MEMBER_STREAMS`] streams at once (one thread each; on WebAssembly, which has no threads, in
/// turn) from the cells that `cells` keeps, which they share, and joined in stream order.
fn engine_members(
    cells: &MemberCells,
    class: CarvedClass,
    n: usize,
) -> Vec<crate::stellar::binary::testing::ClassMember> {
    let share = n.div_ceil(16);
    let mut members = Vec::with_capacity(n + 16);
    #[cfg(not(target_family = "wasm"))]
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..MEMBER_STREAMS)
            .map(|s| scope.spawn(move || cells.members_in(class, share, s)))
            .collect();
        for handle in handles {
            members.extend(handle.join().expect("a member draw finishes"));
        }
    });
    #[cfg(target_family = "wasm")]
    for s in 0..MEMBER_STREAMS {
        members.extend(cells.members_in(class, share, s));
    }
    members.truncate(n);
    members
}

/// `n` of the engine's stellar mergers near the solar circle, each with its weight, drawn on
/// [`MEMBER_STREAMS`] streams at once (one thread each; on WebAssembly, in turn) and joined in
/// stream order.
fn engine_mergers(galaxy: &Galaxy, n: usize) -> Vec<(f64, f64, f64, f64)> {
    use crate::stellar::binary::testing::brute_force_merger_members_in;
    let share = n.div_ceil(16);
    let mut members = Vec::with_capacity(n + 16);
    #[cfg(not(target_family = "wasm"))]
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..MEMBER_STREAMS)
            .map(|s| {
                scope.spawn(move || {
                    brute_force_merger_members_in(galaxy, CarvedClass::StellarMerger, share, s)
                })
            })
            .collect();
        for handle in handles {
            members.extend(handle.join().expect("a merger draw finishes"));
        }
    });
    #[cfg(target_family = "wasm")]
    for s in 0..MEMBER_STREAMS {
        members.extend(brute_force_merger_members_in(
            galaxy,
            CarvedClass::StellarMerger,
            share,
            s,
        ));
    }
    members.truncate(n);
    members
        .iter()
        .map(|m| {
            let [a, b] = m.before().stars().map(|s| s.mass().value());
            let (heavy, light) = (a.max(b), a.min(b));
            (
                crate::math::log10(a + b),
                light / heavy,
                crate::math::log10(m.record().primary_initial_mass().value()),
                m.weight(),
            )
        })
        .collect()
}

/// The chi-square per degree of freedom of `observed`, each value with its weight, against
/// `expected`'s deciles, the weighted counts scaled to the members' effective number (Σw)² ÷ Σw²,
/// and that number; `None` with fewer than 50 expected values or 10 observed.
fn decile_chi_square(expected: &[f64], observed: &[(f64, f64)]) -> Option<(f64, f64)> {
    let mut sorted: Vec<f64> = expected.iter().copied().filter(|v| v.is_finite()).collect();
    let observed: Vec<(f64, f64)> = observed
        .iter()
        .copied()
        .filter(|(v, _)| v.is_finite())
        .collect();
    if sorted.len() < 50 || observed.len() < 10 {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    let edges: Vec<f64> = (1..10).map(|k| sorted[k * sorted.len() / 10]).collect();
    let mut weights = [0.0_f64; 10];
    for &(v, w) in &observed {
        weights[edges.partition_point(|&e| e <= v)] += w;
    }
    let total: f64 = observed.iter().map(|(_, w)| w).sum();
    let squares: f64 = observed.iter().map(|(_, w)| w * w).sum();
    let effective = total * total / squares;
    let each = effective / 10.0;
    let statistic: f64 = weights
        .iter()
        .map(|w| {
            let o = w / total * effective;
            (o - each) * (o - each) / each
        })
        .sum();
    Some((statistic / 9.0, effective))
}

/// A mark of an accreting white dwarf: its name, and its value of a listed entry and of an engine
/// member.
type AwdMarkOf = (
    &'static str,
    fn(&AwdEntry) -> f64,
    fn(&crate::stellar::binary::testing::AccretingMarks) -> f64,
);

/// A mark of an X-ray binary: its name, and its value of a listed entry and of an engine member.
type XrbMarkOf = (
    &'static str,
    fn(&XrbEntry) -> f64,
    fn(&crate::stellar::binary::testing::XrayMarks) -> f64,
);

/// The scratch tables' tolerance, as P11.T8.a's and T8.c's: each mark's chi-square per degree of
/// freedom under 290, which refuses only a mark whose engine members all but fall into one tenth of
/// the listed entries' law. P15.T10.b's fit must bring every mark under the 99.9% point, about 3.1
/// at nine degrees of freedom; a mark over it is a finding, not a failure.
const SCRATCH_CHI_SQUARE_PER_DOF: f64 = 290.0;

/// The classes' samplers as they draw inside features (P11.T8.f), under every 27 hold, against the
/// engine's members of each class near the solar circle (`brute_force_class_members`): every mark
/// both define, the engine's members binned in the listed entries' deciles, by chi-square per
/// degree of freedom, printed for the record, each under the scratch tables' tolerance: 290 for the
/// accreting white dwarfs and the mergers, and 0.97 of the members compared for the X-ray binaries,
/// as P11.T8.b's test holds them. The
/// listed entries are the walk's features' and the centre's; the neutron-star mergers, of which the
/// lists expect under one, are not compared.
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine"]
fn the_listed_samplers_against_the_engine_s_members() {
    let galaxy = listed();
    let mut entries: Vec<&ListedBinary> = the_entries().iter().map(|(_, _, e)| e).collect();
    let model = galaxy.centre().unwrap();
    let recent = crate::galaxy::features::centre::recent::CentreRecent::new(
        galaxy,
        model,
        crate::galaxy::features::centre::events::victims::victim_count(galaxy).unwrap(),
    );
    let centre: Vec<ListedBinary> = recent
        .list(galaxy)
        .entries()
        .filter(|e| e.class() == ClassId::STELLAR_MERGER || e.class() == ClassId::XRAY_BINARY)
        .filter_map(|e| {
            recent
                .binaries()
                .entry(galaxy, model, e.class(), e.index())
                .map(|(entry, _)| entry)
        })
        .collect();
    entries.extend(centre.iter());
    let mut report = String::new();
    let cells = MemberCells::new(galaxy);
    compare_accreting(&cells, &entries, &mut report);
    compare_xray(&cells, &entries, &mut report);
    compare_mergers(galaxy, &entries, &mut report);
    eprint!("{report}");
}

/// Values of one mark, each with a weight of one.
fn unit(v: Vec<f64>) -> Vec<(f64, f64)> {
    v.into_iter().map(|x| (x, 1.0)).collect()
}

/// The accreting white dwarfs' marks of `entries` against the engine's members, drawn from
/// `cells`, into `report`.
fn compare_accreting(cells: &MemberCells, entries: &[&ListedBinary], report: &mut String) {
    for (side, carved) in [
        (AwdSide::Fast, CarvedClass::AccretingWdFast),
        (AwdSide::Slow, CarvedClass::AccretingWdSlow),
    ] {
        let listed: Vec<&AwdEntry> = entries
            .iter()
            .filter_map(|e| match e {
                ListedBinary::AccretingWd(entry) if entry.scan().side() == side => Some(entry),
                _ => None,
            })
            .collect();
        let engine = engine_members(cells, carved, 300);
        let marks: [AwdMarkOf; 5] = [
            (
                "white dwarf mass",
                |e| e.scan().white_dwarf_mass().value(),
                |m| m.white_dwarf_mass.value(),
            ),
            (
                "log donor mass",
                |e| crate::math::log10(e.binary().donor().mass().value()),
                |m| crate::math::log10(m.donor_mass.value()),
            ),
            (
                "log period",
                |e| crate::math::log10(e.binary().orbit().period().value() / 3_600.0),
                |m| {
                    m.period
                        .map_or(f64::NAN, |p| crate::math::log10(p.value() / 3_600.0))
                },
            ),
            (
                "log rate",
                |e| crate::math::log10(e.scan().transfer_rate().value()),
                |m| crate::math::log10(m.transfer_rate.value()),
            ),
            (
                "log recurrence",
                |e| crate::math::log10(e.scan().recurrence().value()),
                |m| crate::math::log10(m.recurrence.value()),
            ),
        ];
        for (name, listed_of, engine_of) in marks {
            let expected: Vec<f64> = listed.iter().map(|e| listed_of(e)).collect();
            let observed = unit(
                engine
                    .iter()
                    .filter_map(|m| m.accreting().map(engine_of))
                    .collect(),
            );
            let (per_dof, n) = decile_chi_square(&expected, &observed).unwrap();
            writeln!(
                report,
                "{carved:?} {name}: χ²/dof {per_dof:.1} ({n:.0} of {} listed)",
                listed.len()
            )
            .unwrap();
            assert!(
                per_dof < SCRATCH_CHI_SQUARE_PER_DOF,
                "{carved:?} {name}: {per_dof}"
            );
        }
    }
}

/// The X-ray binaries' marks of `entries` against the engine's members, drawn from `cells`, into
/// `report`.
fn compare_xray(cells: &MemberCells, entries: &[&ListedBinary], report: &mut String) {
    let listed: Vec<&XrbEntry> = entries
        .iter()
        .filter_map(|e| match e {
            ListedBinary::XrayBinary(entry) => Some(entry),
            _ => None,
        })
        .collect();
    let engine = engine_members(cells, CarvedClass::XrayBinary, 200);
    let marks: [XrbMarkOf; 4] = [
        (
            "log accretor mass",
            |e| crate::math::log10(e.scan().accretor_mass().value()),
            |m| crate::math::log10(m.accretor_mass.value()),
        ),
        (
            "log period",
            |e| crate::math::log10(e.scan().period().value() / 86_400.0),
            |m| {
                m.period
                    .map_or(f64::NAN, |p| crate::math::log10(p.value() / 86_400.0))
            },
        ),
        (
            "log donor mass",
            |e| crate::math::log10(e.binary().donor().mass().value()),
            |m| crate::math::log10(m.donor_mass.value()),
        ),
        (
            "log rate",
            |e| {
                e.scan()
                    .transfer_rate()
                    .map_or(f64::NAN, |r| crate::math::log10(r.value()))
            },
            |m| {
                m.transfer_rate
                    .map_or(f64::NAN, |r| crate::math::log10(r.value()))
            },
        ),
    ];
    for (name, listed_of, engine_of) in marks {
        let expected: Vec<f64> = listed.iter().map(|e| listed_of(e)).collect();
        let observed = unit(
            engine
                .iter()
                .filter_map(|m| m.xray().map(engine_of))
                .collect(),
        );
        let Some((per_dof, n)) = decile_chi_square(&expected, &observed) else {
            writeln!(report, "XrayBinary {name}: too few to compare").unwrap();
            continue;
        };
        writeln!(
            report,
            "XrayBinary {name}: χ²/dof {per_dof:.1} ({n:.0} of {} listed)",
            listed.len()
        )
        .unwrap();
        assert!(per_dof < 0.97 * n, "XrayBinary {name}: {per_dof} of {n}");
    }
}

/// The stellar mergers' marks of `entries` against the engine's weighted mergers, into `report`.
fn compare_mergers(galaxy: &Galaxy, entries: &[&ListedBinary], report: &mut String) {
    let log = crate::math::log10;
    let listed: Vec<(f64, f64, f64)> = entries
        .iter()
        .filter_map(|e| match e {
            ListedBinary::StellarMerger(entry) => {
                let scan = entry.scan();
                let (m1, m2) = (scan.primary_mass().value(), scan.secondary_mass().value());
                Some((log(m1 + m2), m2 / m1, log(m1)))
            }
            _ => None,
        })
        .collect();
    let engine = engine_mergers(galaxy, 1_000);
    for (name, i) in [
        ("log total mass", 0),
        ("mass ratio", 1),
        ("log primary mass", 2),
    ] {
        let pick = |t: (f64, f64, f64)| [t.0, t.1, t.2][i];
        let expected: Vec<f64> = listed.iter().map(|&t| pick(t)).collect();
        let observed: Vec<(f64, f64)> = engine
            .iter()
            .map(|&(a, b, c, w)| (pick((a, b, c)), w))
            .collect();
        let Some((per_dof, n)) = decile_chi_square(&expected, &observed) else {
            writeln!(
                report,
                "StellarMerger {name}: too few to compare ({} listed)",
                listed.len()
            )
            .unwrap();
            continue;
        };
        writeln!(
            report,
            "StellarMerger {name}: χ²/dof {per_dof:.1} (effective {n:.0}, {} listed)",
            listed.len()
        )
        .unwrap();
        assert!(
            per_dof < SCRATCH_CHI_SQUARE_PER_DOF,
            "StellarMerger {name}: {per_dof}"
        );
    }
}

/// The globulars hold what the catalogue gives up for them: for the bulge, the thick disc and the
/// halo, the globulars' expected hosts of each class in closed form, their expected mass over the
/// population's mean system mass (plan 09's φ, as P09.T6's budget test counts their members) times
/// the mass formed per system, the class's rate and its bands' weights, equal the hosts the
/// catalogue's components give up for them ([`features_share`]) to 10⁻⁶; and each walked
/// globular's list expects its own share of them, its mass's. The X-ray binaries, whose globular
/// count is P09.T9.e's encounter law, are left out.
#[test]
fn the_globulars_hold_what_the_catalogue_gives_up_for_them() {
    let galaxy = listed();
    let shares = galaxy.feature_shares();
    let (bulge, thick, halo) = shares.globulars().population_masses();
    let per_system = galaxy.mean_formed_mass().value();
    let weights = shares.globular_band_weights();
    for row in CLASS_SHARES.rows() {
        if row.class() == CarvedClass::XrayBinary {
            continue;
        }
        let band_weight: f64 = MassBand::ALL
            .iter()
            .map(|&b| row.layer_share(b) * weights[b.index()])
            .sum();
        for (population, mass) in [
            (Population::Bulge, bulge),
            (Population::ThickDisc, thick),
            (Population::Halo, halo),
        ] {
            let rate = row.hosts_per_formed_mass(population);
            let held = mass / galaxy.mean_system_mass(population).value()
                * per_system
                * rate
                * band_weight;
            let given: f64 = galaxy
                .fields()
                .components()
                .iter()
                .filter(|c| c.population() == population)
                .map(|c| {
                    c.count_with_unborn()
                        * c.ages().born_fraction()
                        * rate
                        * per_system
                        * features_share(galaxy, row, c)
                })
                .sum();
            assert!(
                (held / given - 1.0).abs() < 1e-6,
                "{:?} {population:?}: held {held:e}, given up {given:e}",
                row.class()
            );
        }
    }
    // Each walked globular's list: its mass's share of them.
    for feature in FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular).take(20) {
        let Some(binaries) = FeatureBinaries::of_feature(galaxy, &feature, None) else {
            continue;
        };
        let FeatureMarks::Globular(marks) = feature.marks() else {
            unreachable!("a globular's marks")
        };
        let population = binaries.host().population();
        for &(class, expected) in binaries.expected() {
            if class == ClassId::XRAY_BINARY {
                continue;
            }
            let row = CLASS_SHARES.row(carved_of(class).unwrap());
            let band_weight: f64 = MassBand::ALL
                .iter()
                .map(|&b| row.layer_share(b) * weights[b.index()])
                .sum();
            let want = marks.mass().value() / galaxy.mean_system_mass(population).value()
                * per_system
                * row.hosts_per_formed_mass(population)
                * band_weight;
            assert!(
                (expected / want - 1.0).abs() < 1e-12,
                "{class}: {expected} {want}"
            );
        }
    }
}

/// No catalogue feature's list comes near its 13-bit index: over every globular of the walk, the
/// fullest list's expected entries, plan 09's and plan 11's, keep eight standard deviations under
/// 8,192 by a wide margin, ω Centauri's mass included (printed).
#[test]
fn the_fullest_globular_s_list_fits_its_index() {
    use crate::galaxy::features::members::level_list::{fits_list_index, registered_classes};
    let galaxy = listed();
    let (mut fullest, mut heaviest) = (0.0_f64, 0.0_f64);
    for feature in FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular) {
        let expected: f64 = registered_classes(galaxy, &feature)
            .iter()
            .map(ListClass::expected)
            .sum();
        if let FeatureMarks::Globular(marks) = feature.marks() {
            heaviest = heaviest.max(marks.mass().value());
        }
        fullest = fullest.max(expected);
    }
    // ω Centauri's 3.94 × 10⁶ M☉ (Baumgardt and Hilker) at the halo's rates, the walk's heaviest
    // scaled.
    eprintln!(
        "fullest globular list {fullest:.1} expected entries; heaviest globular {heaviest:.3e} M☉"
    );
    assert!(fits_list_index(fullest * 4.0, 8_192), "{fullest}");
    assert!(fullest > 10.0, "{fullest}");
}

/// The listed entries are a pure function of their IDs: drawn in either order, each feature's
/// binary entries are the same, bit for bit.
#[test]
fn the_listed_entries_are_order_independent() {
    let galaxy = listed();
    let keys: Vec<(FeatureRecord, u16)> = the_entries()
        .iter()
        .step_by(37)
        .map(|(f, i, _)| (*f, *i))
        .collect();
    assert!(keys.len() > 20, "{}", keys.len());
    assert_order_independent(&keys, |(feature, index)| {
        FeatureLevelList::of(galaxy, feature).binary_entry(galaxy, *index)
    });
}
