//! The tests of P11.T8.c's fix, from generator version 27: the stellar merger class draws the
//! engine's channels, a contact pair at the tidal limit or an evolved donor whose common envelope
//! merges it with its companion, by `MERGER_SAMPLER_31`'s channel weights, and a dynamical
//! main-sequence merger beside them (`engine_channels_hold.rs`, which holds the three channels'
//! shares). These hold them under every 26 hold.
//!
//! - Every evolved donor is past its main sequence and living at T, fills its lobe on its orbit,
//!   detached before T; the engine itself, run on the pair rebuilt from the entry, sends its onset
//!   of transfer to a common envelope after a detached history, and leaves the entry's merged star
//!   bit for bit.
//! - Every contact pair's mass ratio lies in the contact mark's 0.06–0.15.
//! - Every entry is its class at every age, the rate stays Kochanek et al.'s, under one candidate
//!   in ten fails, the scan keeps exactly the cells' entries without stepping the engine, and the
//!   cells do not depend on the order they are generated in.
//! - Slow: the sampler against the engine's own mergers under every 26 hold, χ² per degree of
//!   freedom by mark, pinned as measured, and the engine's channel split and its evolved donors'
//!   kinds before the merger printed.
//!
//! The movers under the hold, found by turning it on at the version (and back off) over the sim's,
//! the server's and the fitting crate's tests and the merger classes' slow tests: the goldens
//! `catalogue_classes/stellar_mergers` and `catalogue_classes/neutron_star_mergers` (its remnant
//! masses, P11.T8.d's rider); the version-arm tests of `merger/tests.rs` (every age, one ID, the
//! contact pair, the pinned layers) and `nsm/tests.rs`'s
//! `the_inspiral_is_peters_s_and_the_remnant_follows_the_total_mass`; and the galaxy-wide scan's pin
//! of `tests/binary_mergers.rs`. No other golden and no census moves.

use std::sync::OnceLock;

use super::*;
use crate::Seed;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::{CatalogueClassCell, CatalogueEntry};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::stellar::binary::BinaryInput;
use crate::stellar::binary::class_merger::{donor_onset_merger, onset_segment_kinds};
use crate::stellar::binary::scan::{engine_calls, scan_merger_cell};
use crate::tables::binary::{MERGER_SAMPLER_31, MergerChannel, MergerPick};
use crate::time::{CLOCK_WINDOW_H, SourceHorizon};

use super::reference::{decile_chi_square, members};

/// The seed of the Milky Way fixture, P11.T8.c's tests'.
const SEED: u64 = 0x0b17_08c0_0000_0001;

/// The phases an evolved donor is in at its merger.
const EVOLVED: [Phase; 5] = [
    Phase::HertzsprungGap,
    Phase::FirstGiantBranch,
    Phase::CoreHeliumBurning,
    Phase::EarlyAgb,
    Phase::ThermallyPulsingAgb,
];

/// The fixture built under every 26 hold, with its kinematic tables: built so, since the field share
/// and the carve-out are read when a galaxy is built; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        let params = GalaxyParams::milky_way_like();
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            params,
            GalaxyRules::of_version(crate::GENERATOR_VERSION),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential()
    })
}

/// P11.T8.c's sample cells: a run along x through the solar circle, in and just below the plane.
fn sample_cells() -> Vec<CatalogueCellKey> {
    (-2..2)
        .flat_map(|x| {
            [0, -1].map(|z| {
                CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, 6, z])
                    .expect("a cell of the root cube")
            })
        })
        .collect()
}

/// The sample cells' entries under every 26 hold, generated once.
fn entries() -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<MergerEntry>>)] {
    type Cells = Vec<(CatalogueCellKey, Vec<CatalogueEntry<MergerEntry>>)>;
    static CELLS: OnceLock<Cells> = OnceLock::new();
    CELLS.get_or_init(|| {
        let galaxy = galaxy();
        let process = StellarMergers::new(galaxy);
        sample_cells()
            .into_iter()
            .map(|key| {
                let cell = CatalogueClassCell::generate(galaxy, &process, key);
                (key, cell.into_entries())
            })
            .collect()
    })
}

/// Every sample entry under every 26 hold.
fn all_entries() -> impl Iterator<Item = &'static MergerEntry> {
    entries()
        .iter()
        .flat_map(|(_, cell)| cell.iter().map(CatalogueEntry::value))
}

/// Every contact pair's mass ratio lies in the contact mark's 0.06–0.15: its primary is heavy
/// enough that the secondary at the largest ratio is a track's.
#[test]
fn every_contact_pair_is_at_the_tidal_limit() {
    let mut checked = 0;
    for entry in all_entries() {
        if entry.scan().channel() != MergerChannel::Contact {
            continue;
        }
        let q = entry.scan().secondary_mass().value() / entry.scan().primary_mass().value();
        assert!((0.06..=0.15).contains(&q), "{:?}: q {q}", entry.scan().id());
        checked += 1;
    }
    // From 31 the fitted channel weights give the contact channel some 3% of the entries.
    assert!(checked >= 3, "{checked}");
}

/// Every evolved donor is past its main sequence and living at T, its companion on its main
/// sequence; it fills its Roche lobe on its circular orbit; the pair is detached before T; and the
/// engine, built at T on the pair rebuilt from the entry's own marks (with the entry's angles,
/// which no onset reads), finds the onset unstable, a detached history then a common envelope,
/// and leaves the entry's merged star bit for bit.
#[test]
fn every_evolved_donor_merges_as_the_engine_merges_it() {
    let galaxy = galaxy();
    let mut checked = 0;
    for entry in all_entries() {
        match entry.scan().channel() {
            MergerChannel::EvolvedDonor => {}
            MergerChannel::Contact => {
                assert_eq!(entry.pair().before(), SegmentKind::Contact);
                continue;
            }
            MergerChannel::DynamicalMainSequence => continue,
        }
        let pair = entry.pair();
        let [donor, companion] = pair.stars();
        assert!(EVOLVED.contains(&donor.phase()), "{:?}", donor.phase());
        assert_eq!(companion.phase(), Phase::MainSequence);
        assert_eq!(pair.before(), SegmentKind::Detached);
        let a = pair.orbit().semi_major_axis();
        let lobe = roche_lobe_radius(donor.mass().value() / companion.mass().value(), a).value();
        assert!((donor.radius().value() * SOLAR_RADIUS_M / lobe - 1.0).abs() < 1e-12);
        let t = entry.scan().merger_age();
        let before = entry
            .timeline()
            .state_at(Years::new(t.value() * (1.0 - 1e-9)));
        assert_eq!(before.kind(), SegmentKind::Detached);
        assert_eq!(before.stars(), pair.stars());
        // The engine's helper, on the pair rebuilt from the entry's own marks.
        let system = entry.record().id();
        let draws = [0, 1].map(|b| StarDraws::for_star(galaxy.seed(), BodyId::new(system, b)));
        let input = BinaryInput::new(
            entry.scan().primary_mass(),
            entry.scan().secondary_mass(),
            *entry.scan().composition(),
            *pair.orbit(),
            draws,
            entry.scan().age_at_epoch(),
        )
        .unwrap()
        .with_rules(galaxy.rules().stellar);
        let until = Years::new(
            t.value().max(entry.scan().age_at_epoch().value())
                + CLOCK_WINDOW_H.as_julian_years_f64(),
        );
        let rebuilt = donor_onset_merger(&input, t, until).expect("the entry's pair merges");
        assert_eq!(&rebuilt, entry.product());
        let (kinds, after) = onset_segment_kinds(&input, t, until);
        assert_eq!(
            (kinds.as_slice(), after),
            (
                &[SegmentKind::Detached, SegmentKind::CommonEnvelope][..],
                SegmentKind::Merged
            ),
            "{:?}",
            entry.scan().id()
        );
        checked += 1;
    }
    assert!(checked > 100, "{checked}");
}

/// Every entry passes the shared test with its own class at every age of the source horizon:
/// before T its pair as marked, contact or detached, and from T the merged star.
#[test]
fn every_entry_is_its_class_at_every_age_under_every_26_hold() {
    let start = SourceHorizon::START.since_epoch().as_julian_years_f64();
    let end = SourceHorizon::END.since_epoch().as_julian_years_f64();
    for entry in all_entries() {
        let now = entry.record().age_at_epoch();
        let timeline = entry.timeline();
        let t = entry.scan().merger_age();
        assert_eq!(
            carved_class(timeline, now),
            Some(CarvedClass::StellarMerger)
        );
        assert_eq!(timeline.merger_age(), Some(t));
        for k in 0..=16 {
            let age = Years::new(now.value() + start + (end - start) * f64::from(k) / 16.0);
            let state = timeline.state_at(age);
            if age < t {
                assert_eq!(state.kind(), entry.pair().before());
                assert_eq!(state.stars(), entry.pair().stars());
            } else {
                assert_eq!(state.kind(), SegmentKind::Merged);
                assert_eq!(state.stars()[0], *entry.product());
                assert_eq!(state.stars()[1].phase(), Phase::NoRemnant);
            }
        }
    }
}

/// The rate stays Kochanek et al.'s (2014) 0.2–0.5 a year: the channels move no host's place.
#[test]
fn the_rate_stays_kochanek_s_under_every_26_hold() {
    let rate = StellarMergers::new(galaxy()).mergers_per_year(galaxy());
    assert!((0.2..=0.5).contains(&rate), "{rate}");
}

/// Under one candidate in ten that the site's thinning keeps fails the scan's checks or the
/// class's test, and the scan keeps exactly the cells' entries without running the engine.
#[test]
fn under_one_in_ten_fails_and_the_scan_keeps_the_entries() {
    let galaxy = galaxy();
    let process = StellarMergers::new(galaxy);
    let before = engine_calls();
    let (mut thinned, mut failed) = (0_u32, 0_u32);
    for (key, cell) in entries() {
        let bound = process.bound(galaxy, key);
        for index in 0..candidate_count(galaxy.seed(), key, bound) {
            let id = key.candidate_id(index, 0).unwrap();
            let site = process
                .hosts()
                .site(galaxy, key, id, bound, tags::CLASS_MERGER_SITE);
            if site.is_none() {
                continue;
            }
            thinned += 1;
            let kept = process
                .candidate(galaxy, key, id, bound)
                .and_then(StellarMergers::test);
            failed += u32::from(kept.is_none());
        }
        let mut scanned = Vec::new();
        scan_merger_cell(galaxy, &process, *key, &mut scanned);
        let kept: Vec<_> = cell.iter().map(|e| *e.value().scan()).collect();
        assert_eq!(scanned, kept, "{key:?}");
    }
    println!("{failed} of {thinned} thinned candidates failed");
    assert!(
        f64::from(failed) < 0.1 * f64::from(thinned),
        "{failed} of {thinned}"
    );
    assert_eq!(engine_calls(), before);
}

/// The cells under every 26 hold do not depend on the order they are generated in, nor on what was
/// resolved between them: the channel, the evolved donor's living-mass search and the engine built
/// at its merger keep no state.
#[test]
fn the_channels_cells_do_not_depend_on_order() {
    use hyperion_testkit::order::assert_order_independent;
    let galaxy = galaxy();
    let process = StellarMergers::new(galaxy);
    let cells: Vec<CatalogueCellKey> = sample_cells().into_iter().take(4).collect();
    assert_order_independent(&cells, |key| {
        let stray =
            CatalogueSystemId::new(ClassId::CORE_COLLAPSE.value(), key.fine_cell(), 0, 0).unwrap();
        // Whether the stray ID names a system is beside the point: only asking it could disturb
        // what follows.
        let _ = crate::galaxy::placement::resolve(galaxy, crate::id::SystemId::from(stray));
        CatalogueClassCell::generate(galaxy, &process, *key).into_entries()
    });
}

/// The sampler against the engine under every 26 hold (P11.T8.c's fix): the pair's total mass,
/// mass ratio and primary's initial mass of 3,000 weighted engine mergers in the deciles of 1,000
/// entries about the solar circle, by chi-square per degree of freedom, pinned as measured (a miss
/// of P15.T10.b's 3.1 is a finding); and the engine's channel split, its pre-main-sequence mergers
/// left out, against the sampler's weights.
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine"]
fn the_channel_sampler_against_the_engine_under_every_26_hold() {
    use crate::math;
    let galaxy = galaxy();
    let process = StellarMergers::new(galaxy);
    let mut catalogue: Vec<[f64; 3]> = Vec::new();
    'cells: for x in [0, -1, 1, -2, 2, -3, 3] {
        for y in 5..=7 {
            for z in [0, -1] {
                if catalogue.len() >= 1_000 {
                    break 'cells;
                }
                let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, y, z]).unwrap();
                let cell = CatalogueClassCell::generate(galaxy, &process, key);
                catalogue.extend(cell.entries().iter().map(|e| {
                    let scan = e.value().scan();
                    let (m1, m2) = (scan.primary_mass().value(), scan.secondary_mass().value());
                    [math::log10(m1 + m2), m2 / m1, math::log10(m1)]
                }));
            }
        }
    }
    catalogue.truncate(1_000);
    let mut split = [0.0_f64; 3];
    let mut kinds = [0.0_f64; 3];
    let engine: Vec<([f64; 3], f64)> = members(galaxy, 3_000)
        .iter()
        .filter_map(|m| {
            let t = m.merger_age().value();
            let before = m.timeline().state_at(Years::new(t * (1.0 - 1e-9)));
            let phases = before.stars().map(|s| s.phase());
            if phases
                .iter()
                .any(|p| matches!(p, Phase::Protostar | Phase::PreMainSequence))
            {
                return None;
            }
            let [a, b] = before.stars().map(|s| s.mass().value());
            let heavier = phases[usize::from(b > a)];
            let lighter = phases[usize::from(b <= a)];
            let slot = if phases.iter().all(|p| *p == Phase::MainSequence) {
                0
            } else if EVOLVED.contains(&heavier) && lighter == Phase::MainSequence {
                1
            } else {
                2
            };
            split[slot] += m.weight();
            if slot == 1 {
                let kind = match before.kind() {
                    SegmentKind::Detached => 0,
                    SegmentKind::StableTransfer { .. } => 1,
                    SegmentKind::Contact
                    | SegmentKind::CommonEnvelope
                    | SegmentKind::Merged
                    | SegmentKind::Disrupted { .. } => 2,
                };
                kinds[kind] += m.weight();
            }
            let marks = [
                math::log10(a + b),
                a.min(b) / a.max(b),
                math::log10(m.record().primary_initial_mass().value()),
            ];
            Some((marks, m.weight()))
        })
        .collect();
    let total: f64 = split.iter().sum();
    println!(
        "engine (weighted, pre-main-sequence left out): two main-sequence stars {:.3}, an evolved \
         donor beside a main-sequence star {:.3}, others {:.3}; sampler (contact, evolved donor, \
         dynamical main sequence) {:?}",
        split[0] / total,
        split[1] / total,
        split[2] / total,
        MERGER_SAMPLER_31.pick(MergerPick::EngineChannel).weights()
    );
    let evolved: f64 = kinds.iter().sum();
    println!(
        "  the engine's evolved donors just before the merger: detached {:.3}, in stable transfer \
         {:.3}, other {:.3}; the class's all detached",
        kinds[0] / evolved,
        kinds[1] / evolved,
        kinds[2] / evolved
    );
    let mut measured = [0.0; 3];
    for (k, name) in ["log total mass", "mass ratio", "log primary mass"]
        .iter()
        .enumerate()
    {
        let expected: Vec<f64> = catalogue.iter().map(|m| m[k]).collect();
        let observed: Vec<(f64, f64)> = engine.iter().map(|(m, w)| (m[k], *w)).collect();
        let (per_dof, effective) = decile_chi_square(&expected, &observed);
        println!("  {name}: chi2/dof {per_dof:.2} (effective {effective:.0})");
        measured[k] = per_dof;
    }
    for (m, pinned) in measured.iter().zip(PINNED_CHI_SQUARE) {
        assert!((m / pinned - 1.0).abs() < 1e-6, "{measured:?}");
    }
}

/// The chi-square per degree of freedom of the total mass, the mass ratio and the primary's mass,
/// as measured (2026-11-01; 41.9, 12.0 and 65.3 for version 24's contact-only class against the
/// version's engine), re-pinned at generator versions 25 (14.23, 12.14 and 16.02 at 24), 26
/// (14.46, 12.04 and 16.41 at 25) and 27 (14.56, 9.75 and 16.18 at 26), on 27's engine, whose
/// raised segment cap moves none of it (the same bits at 64): the masses' over P15.T10.b's 3.1, a
/// finding, and from 27 the mass ratio's under it; and at 30 (16.46, 3.12 and 16.69; 16.19, 3.09
/// and 16.42 at 29) on v30's refitted binary rows, the mass ratio's now just over it (landing round
/// 4), a finding; and at 31 (14.63, 8.30 and 10.69) on the fitted merger sampler
/// (`MERGER_SAMPLER_31`, P15.T10.b), its mass ratio's now far over P15.T10.b's 3.1, a finding.
const PINNED_CHI_SQUARE: [f64; 3] = [
    14.627_675_840_494_17,
    8.299_529_498_705_436,
    10.687_228_294_519_818,
];
