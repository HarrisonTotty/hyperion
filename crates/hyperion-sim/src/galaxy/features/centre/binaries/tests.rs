//! The tests of plan 11's binary classes on the galactic centre's list (P11.T8.f) under every 27
//! hold.

use std::sync::OnceLock;

use super::*;
use crate::Seed;
use crate::galaxy::catalogue_classes::testing::assert_complementary;
use crate::galaxy::features::centre::events::victims::victim_count;
use crate::galaxy::features::centre::members::resolve_centre_member_with;
use crate::galaxy::features::centre::recent::CentreRecent;
use crate::galaxy::features::members::{ListEntry, ListIndex, centre_list_forecast};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;

/// The seed of the tests' Milky Way, the centre's recent classes' (`centre::recent`'s tests).
const SEED: u64 = 0x0935_0000_0000_0001;

/// The Milky Way at [`SEED`] under `rules`.
fn milky_way(rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules).unwrap()
}

/// The Milky Way under every 27 hold.
fn listed() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| milky_way(GalaxyRules::every_27_hold()))
}

/// The same galaxy under every 27 hold but P11.T8.f's.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        milky_way(GalaxyRules {
            feature_binaries: FeatureBinariesRules::Unlisted,
            ..GalaxyRules::every_27_hold()
        })
    })
}

/// The centre's list in `galaxy`, with its recent classes and its binaries.
fn list_of(
    galaxy: &Galaxy,
) -> (
    CentreRecent,
    crate::galaxy::features::members::FeatureLevelList,
) {
    let model = galaxy.centre().unwrap();
    let recent = CentreRecent::new(galaxy, model, victim_count(galaxy).unwrap());
    let list = recent.list(galaxy);
    (recent, list)
}

/// The Milky Way's centre lists the five classes after its victims and its recent classes, at the
/// 16-bit index: their expected hosts are the cluster's formed mass times each class's nuclear-disc
/// rate, within P11.T8.e's forecast, and the list keeps its eight standard deviations with room to
/// spare. Registering them moves no victim and no recent entry: under the held arm the list is the
/// same up to its recent classes' last index. The counts, the drawn entries and the margin are
/// printed for the record.
#[test]
fn the_centre_lists_the_binary_classes_after_its_recent_ones() {
    let (on, off) = (listed(), held());
    let (recent, list) = list_of(on);
    let (_, before) = list_of(off);
    let kept: Vec<ListEntry> = before.entries().collect();
    let now: Vec<ListEntry> = list.entries().take(kept.len()).collect();
    assert_eq!(kept, now);
    let binaries = recent.binaries();
    let expected: f64 = binaries.expected().iter().map(|&(_, e)| e).sum();
    let forecast = centre_list_forecast(on.params()).unwrap();
    assert!(
        expected <= forecast.candidates(),
        "{expected} against the forecast's {}",
        forecast.candidates()
    );
    let model = on.centre().unwrap();
    let formed = model.classes().primaries_formed() * on.mean_formed_mass().value();
    for &(class, n) in binaries.expected() {
        let want = formed * hosts_per_formed_mass(on, class, Population::NuclearDisc);
        assert!(
            (n / want - 1.0).abs() < 1e-12,
            "{class}: {n} against {want}"
        );
        assert_eq!(
            list.count(class),
            u32::try_from(list.entries().filter(|e| e.class() == class).count()).unwrap()
        );
    }
    let all = f64::from(list.len());
    let limit = f64::from(ListIndex::Centre.limit());
    let margin = limit - (all + 8.0 * all.sqrt() + 1.0);
    eprintln!(
        "the centre's binaries {:?}; drawn {:?}; the list {} entries, {margin:.1} indices to spare \
         at eight σ (forecast's margin {:.1})",
        binaries
            .expected()
            .iter()
            .map(|&(c, e)| (c.name(), (e * 10.0).round() / 10.0))
            .collect::<Vec<_>>(),
        LISTED_CLASSES.map(|c| (c.name(), list.count(c))),
        list.len(),
        forecast.margin(8.0)
    );
    assert!(margin > 40_000.0, "{margin}");
    assert!(before.entries().all(|e| {
        !crate::galaxy::catalogue_classes::binary::feature::is_listed_binary(e.class())
    }));
}

/// Each entry the centre keeps resolves through plan 09's resolve to the member it is, bit for
/// bit, is its own class at every age of the source horizon, and is of the centre's origin; a
/// candidate its class refuses resolves to "no such system"; most of the accreting white dwarfs'
/// candidates are kept, the old episodes holding nearly all the centre's mass.
#[test]
fn the_centre_s_binary_entries_resolve_and_are_their_classes() {
    let galaxy = listed();
    let model = galaxy.centre().unwrap();
    let (recent, list) = list_of(galaxy);
    let mut entries = Vec::new();
    let (mut kept, mut drawn) = ([0_u32; 5], [0_u32; 5]);
    for e in list
        .entries()
        .filter(|e| crate::galaxy::catalogue_classes::binary::feature::is_listed_binary(e.class()))
    {
        let i = LISTED_CLASSES.iter().position(|&c| c == e.class()).unwrap();
        drawn[i] += 1;
        // Every 25th of the accreting white dwarfs, and every other class's entry.
        if i < 2 && drawn[i] % 25 != 0 {
            continue;
        }
        let id = CentreMemberId::new(MemberSlot::FeatureLevel { index: e.index() }).unwrap();
        let resolved = resolve_centre_member_with(galaxy, model, id);
        match recent.binaries().entry(galaxy, model, e.class(), e.index()) {
            Some((entry, member)) => {
                kept[i] += 1;
                assert_eq!(resolved.as_ref(), Ok(&member));
                assert_eq!(member.record(), entry.record());
                assert_eq!(
                    entry.record().origin(),
                    SystemOrigin::CentreMember { attempt: 0 }
                );
                entries.push(entry);
            }
            None => assert_eq!(
                resolved,
                Err(crate::galaxy::placement::ResolveSystemError::NoSuchSystem),
                "{:?}",
                e.index()
            ),
        }
    }
    let checked = assert_complementary(
        entries
            .iter()
            .map(|e| (e.carved(), e.timeline(), e.record().age_at_epoch())),
    );
    eprintln!("drawn {drawn:?}, of the sampled kept {kept:?}, {checked} checked");
    let sampled_fast = drawn[0] / 25;
    assert!(
        f64::from(kept[0]) > 0.8 * f64::from(sampled_fast),
        "{kept:?} of {sampled_fast}"
    );
}
