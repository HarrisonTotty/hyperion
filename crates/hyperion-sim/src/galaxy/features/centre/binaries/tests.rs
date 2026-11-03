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
/// spare. The binary classes' entries follow its victims and its recent entries. The counts, the
/// drawn entries and the margin are printed for the record.
#[test]
fn the_centre_lists_the_binary_classes_after_its_recent_ones() {
    let on = listed();
    let (recent, list) = list_of(on);
    let kept: Vec<ListEntry> = list
        .entries()
        .take_while(|e| {
            !crate::galaxy::catalogue_classes::binary::feature::is_listed_binary(e.class())
        })
        .collect();
    assert_ne!(kept.len(), 0);
    assert!(list.entries().skip(kept.len()).all(|e| {
        crate::galaxy::catalogue_classes::binary::feature::is_listed_binary(e.class())
    }));
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

/// The digest (`f64_digest`) of the centre's first [`PINNED_BINARIES`] kept binary entries in index
/// order ([`the_centre_s_binary_entries_are_pinned`]): each one's ID (high, low 32 bits), class,
/// initial mass, age, position and velocity. Blessed natively on x86-64 after P09.T37 drew the
/// centre's list over its reach, and again at generator version 26; the wasip1 suite compares it.
const CENTRE_BINARIES_DIGEST: u64 = 0x4c49_2496_b09c_7610;

/// The centre's binary entries the pin reads.
const PINNED_BINARIES: usize = 40;

/// The centre's binary entries' bits under every 27 hold, pinned before the 27 bump: their places
/// are drawn as the recent classes' are (`centre::recent::position`), so a change to either moves
/// this.
#[test]
fn the_centre_s_binary_entries_are_pinned() {
    use hyperion_testkit::golden::f64_digest;
    let galaxy = listed();
    let model = galaxy.centre().unwrap();
    let (recent, list) = list_of(galaxy);
    let mut values = Vec::new();
    let kept = list
        .entries()
        .filter(|e| crate::galaxy::catalogue_classes::binary::feature::is_listed_binary(e.class()))
        .filter_map(|e| recent.binaries().entry(galaxy, model, e.class(), e.index()))
        .take(PINNED_BINARIES);
    for (entry, member) in kept {
        let raw = member.record().id().raw();
        values.push(f64::from(u32::try_from(raw >> 32).unwrap()));
        values.push(f64::from(u32::try_from(raw & 0xffff_ffff).unwrap()));
        let class = LISTED_CLASSES
            .iter()
            .position(|&c| c == entry.class())
            .unwrap();
        values.push(f64::from(u32::try_from(class).unwrap()));
        values.push(member.record().primary_initial_mass().value());
        values.push(member.record().age_at_epoch().value());
        let at = member.local_position();
        values.extend([at.x, at.y, at.z]);
        values.extend(member.velocity_km_s());
    }
    assert_eq!(
        values.len(),
        PINNED_BINARIES * 11,
        "{PINNED_BINARIES} kept entries"
    );
    let digest = f64_digest(&values);
    eprintln!("{} values, digest {digest:#018x}", values.len());
    assert_eq!(digest, CENTRE_BINARIES_DIGEST, "{digest:#018x}");
}
