//! The tests of P11.T8.c's missing channel, [`MERGER_CHANNELS_VERSION`]: from 27 the stellar
//! merger class adds the engine's dynamical main-sequence mergers, a low-mass main-sequence donor
//! at its lobe that the engine merges dynamically, by `MERGER_SAMPLER`'s `EngineChannel` weights,
//! and draws an evolved donor's companion by its donor's kind's mark. The class at 26 is the held
//! arm verbatim (`channels_hold.rs` holds it, its chi-square pins among its tests); these hold the
//! new arm, under every 27 hold.
//!
//! - The tripwire; the held arm's cells are every 26 hold's, its first entries pinned bit for bit;
//!   and each of the three channels' share against its weight (χ², α = 10⁻³).
//! - Every dynamical main-sequence pair is a low-mass donor on its main sequence at its lobe
//!   beside a lighter companion on its main sequence inside its own, detached before T; the
//!   engine, built at T on the pair rebuilt from the entry, merges it dynamically from a detached
//!   history and leaves the entry's merged star bit for bit, of both stars' mass.
//! - Every evolved donor's companion lies inside its donor's kind's mark.
//! - Every entry is its class at every age, under one candidate in ten fails, the scan keeps
//!   exactly the cells' entries without stepping the engine, and the cells do not depend on the
//!   order they are generated in.
//! - Slow: the class against the engine's own mergers at 48,000 members, an effective 1,233 (17
//!   times P11.T8.c's 73), by chi-square per degree of freedom by mark under every 27 hold and
//!   under the 26 arm, pinned as measured, with the engine's channel split and its donors' mass
//!   ratios by kind printed.
//!
//! No value moves at 24–26: the field is read only under the 26 arm's channels
//! (`MergerClassRules::Channels`), and below 27 it takes the held arm, whose words are version
//! 26's. The class's golden runs at 24, under `MergerClassRules::ContactOnly`, so the held arm's
//! bits are pinned here (`the_held_arm_s_first_entries_are_pinned`) and by `channels_hold`'s slow
//! chi-square pins; the 27 bump deletes the pin with the hold.

use std::sync::OnceLock;

use super::reference::{decile_chi_square, members};
use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::{CatalogueClassCell, CatalogueEntry};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::stellar::binary::BinaryInput;
use crate::stellar::binary::class_merger::{donor_onset_merger, onset_segment_kinds};
use crate::stellar::binary::scan::{engine_calls, scan_merger_cell};
use crate::tables::binary::{MERGER_SAMPLER, MergerChannel, MergerMark, MergerPick};
use crate::time::{CLOCK_WINDOW_H, SourceHorizon};

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

/// The fixture built under every 27 hold, with its kinematic tables: built so, since the field
/// share and the carve-out are read when a galaxy is built; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        let params = GalaxyParams::milky_way_like();
        Galaxy::from_params_with_rules(Seed::new(SEED), params, GalaxyRules::every_27_hold())
            .expect("the Milky Way fixture's gas is mostly neutral")
            .with_full_potential()
    })
}

/// The fixture under every 27 hold but this one: the held arm, version 26's channels, with P11.T8.f's
/// hold set back too, whose catalogue gives up the features' share of its hosts.
fn held() -> Galaxy {
    galaxy().clone().with_rules(GalaxyRules {
        merger_channels: MergerChannelRules::ContactAndEvolved,
        feature_binaries:
            crate::galaxy::catalogue_classes::binary::feature::FeatureBinariesRules::Unlisted,
        ..GalaxyRules::every_27_hold()
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

/// The sample cells' entries under every 27 hold, generated once.
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

/// Every sample entry under every 27 hold.
fn all_entries() -> impl Iterator<Item = &'static MergerEntry> {
    entries()
        .iter()
        .flat_map(|(_, cell)| cell.iter().map(CatalogueEntry::value))
}

/// The version's class keeps the held arm, every version below 27 does, and 27 and every 27 hold
/// take the dynamical main sequence, beside the 26 arm's channels. The tripwire: the bump to 27
/// deletes this hold.
#[test]
fn the_merger_channels_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < MERGER_CHANNELS_VERSION,
        "the bump to 27 deletes P11.T8.c's missing-channel hold: `MERGER_CHANNELS_VERSION`, \
         `MergerChannelRules` with its `ContactAndEvolved` arm, `GalaxyRules::merger_channels`, \
         the scan's match on it with `MergerPick::Channel` and `MergerChannel::HELD`, and this \
         module's tripwire"
    );
    let at = |v: u32| GalaxyRules::of_version(GeneratorVersion::new(v)).merger_channels;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), MergerChannelRules::ContactAndEvolved, "version {v}");
    }
    assert_eq!(at(27), MergerChannelRules::WithDynamical);
    assert_eq!(
        GalaxyRules::every_26_hold_with_its_retargets().merger_channels,
        MergerChannelRules::ContactAndEvolved
    );
    assert_eq!(
        GalaxyRules::every_27_hold().merger_channels,
        MergerChannelRules::WithDynamical
    );
    assert_eq!(
        GalaxyRules::every_27_hold().merger_class,
        MergerClassRules::Channels
    );
}

/// The fixture built under every 26 hold, with its kinematic tables.
fn every_26() -> Galaxy {
    Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_26_hold_with_its_retargets(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
    .with_full_potential()
}

/// Under the held arm the class's cells are every 26 hold's, bit for bit: the field alone turns
/// the new arm on. A later hold at 27 that moves the class's cells sets itself back in [`held`]
/// too.
#[test]
fn the_held_arm_s_cells_are_every_26_hold_s() {
    let (every_26, held) = (every_26(), held());
    for key in sample_cells().into_iter().take(2) {
        let [a, b] = [&every_26, &held]
            .map(|g| CatalogueClassCell::generate(g, &StellarMergers::new(g), key).into_entries());
        assert_ne!(a.len(), 0, "{key:?}");
        assert_eq!(a, b, "{key:?}");
    }
}

/// The held arm's first entries of a solar-circle cell under every 26 hold, as measured
/// (2026-11-01): each one's channel and the bits of its primary's and secondary's initial masses
/// and of its merger time, seconds since the epoch.
const HELD_FIRST_ENTRIES: [(MergerChannel, u64, u64, u64); 8] = [
    (
        MergerChannel::EvolvedDonor,
        0x3ff2_c25d_4d48_5d82,
        0x3fe7_aaf9_b646_14bc,
        0xc298_70b7_839b_028f,
    ),
    (
        MergerChannel::EvolvedDonor,
        0x3ff2_2722_3223_5184,
        0x3fcb_5f31_9aaf_796b,
        0xc25a_f7f1_157e_b790,
    ),
    (
        MergerChannel::EvolvedDonor,
        0x3ff3_0fd5_5369_c739,
        0x3fc8_21b4_687d_b6ef,
        0xc298_a124_13cf_5a41,
    ),
    (
        MergerChannel::EvolvedDonor,
        0x3ffb_3e63_0992_4327,
        0x3fcc_0727_cc89_5464,
        0xc28d_4b78_4c23_9bfe,
    ),
    (
        MergerChannel::EvolvedDonor,
        0x3ff0_bdea_f038_a193,
        0x3fca_5696_21f4_73d0,
        0xc29d_d1ca_5a74_c796,
    ),
    (
        MergerChannel::EvolvedDonor,
        0x3ff0_6873_96a6_8215,
        0x3fc8_9a91_7e6e_c908,
        0xc292_89d5_a72e_ceee,
    ),
    (
        MergerChannel::Contact,
        0x3fe6_5830_23d9_8fe5,
        0x3fba_057b_074f_10ca,
        0xc281_21d6_cc06_48b4,
    ),
    (
        MergerChannel::EvolvedDonor,
        0x3ff0_d629_46ae_65d5,
        0x3fce_a150_1d53_b44a,
        0xc295_1c95_e69d_1848,
    ),
];

/// Every 26 hold's first entries of the cell along +y at the solar circle are pinned bit for bit:
/// the held arm's words, which the class's golden (at 24, contact pairs only) never reaches.
#[test]
fn the_held_arm_s_first_entries_are_pinned() {
    use hyperion_testkit::float::bits;
    let galaxy = every_26();
    let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [0, 6, 0]).unwrap();
    let cell = CatalogueClassCell::generate(&galaxy, &StellarMergers::new(&galaxy), key);
    let first: Vec<(MergerChannel, u64, u64, u64)> = cell
        .entries()
        .iter()
        .take(HELD_FIRST_ENTRIES.len())
        .map(|e| {
            let scan = e.value().scan();
            (
                scan.channel(),
                bits(scan.primary_mass().value()),
                bits(scan.secondary_mass().value()),
                bits(scan.merger_time().since_epoch().as_seconds_f64()),
            )
        })
        .collect();
    for (channel, m1, m2, t) in &first {
        println!("(MergerChannel::{channel:?}, {m1:#018x}, {m2:#018x}, {t:#018x})");
    }
    assert_eq!(first, HELD_FIRST_ENTRIES);
}

/// Each channel's share of the entries is its weight, by χ² at α = 10⁻³: the channel's word is the
/// scan's first on its stream, and the engine refuses few enough candidates of any channel that
/// the shares stay the weights'.
#[test]
fn each_channel_s_share_is_its_weight_under_every_27_hold() {
    use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};
    let mut counts = [0_u64; 3];
    for entry in all_entries() {
        let k = MergerChannel::ALL
            .iter()
            .position(|&c| c == entry.scan().channel())
            .expect("a channel of the choice");
        counts[k] += 1;
    }
    let n: u64 = counts.iter().sum();
    let weights = MERGER_SAMPLER.pick(MergerPick::EngineChannel).weights();
    #[expect(clippy::cast_precision_loss, reason = "counts of some hundreds")]
    let expected: Vec<f64> = weights.iter().map(|w| w * n as f64).collect();
    println!(
        "{counts:?} of {n} (contact, evolved donor, dynamical main sequence) against {weights:?}"
    );
    assert!(n > 200, "{n}");
    assert_p_value(
        "the channels' shares",
        chi_square_gof(&counts, &expected).p_value,
        ALPHA,
    );
}

/// Every dynamical main-sequence pair: its donor, the primary, a low-mass (BSE's k = 0) star on its
/// main sequence filling its Roche lobe on its circular orbit, beside a lighter companion on its
/// main sequence inside its own; detached before T; and the engine, built at T on the pair rebuilt
/// from the entry's own marks (with the entry's angles, which no onset reads), finds the onset
/// dynamical, merges the pair after a detached history and leaves the entry's merged star bit for
/// bit, a main-sequence star of both stars' mass (P11.T4.d's rule, every 26 hold's).
#[test]
fn every_dynamical_pair_merges_as_the_engine_merges_it() {
    let galaxy = galaxy();
    let mut checked = 0;
    for entry in all_entries() {
        if entry.scan().channel() != MergerChannel::DynamicalMainSequence {
            continue;
        }
        let pair = entry.pair();
        let [donor, companion] = pair.stars();
        let (md, mc) = (donor.mass().value(), companion.mass().value());
        assert_eq!(donor.phase(), Phase::MainSequence);
        assert_eq!(companion.phase(), Phase::MainSequence);
        assert!(
            md < 0.7 && mc <= md,
            "{:?}: {md} and {mc}",
            entry.scan().id()
        );
        assert_eq!(pair.before(), SegmentKind::Detached);
        let a = pair.orbit().semi_major_axis();
        let lobe = roche_lobe_radius(md / mc, a).value();
        let fill = donor.radius().value() * SOLAR_RADIUS_M / lobe;
        assert!(
            (fill - 1.0).abs() < 1e-12,
            "{:?}: {fill}",
            entry.scan().id()
        );
        let own = roche_lobe_radius(mc / md, a).value();
        let inside = companion.radius().value() * SOLAR_RADIUS_M / own;
        assert!(inside < 1.0, "{:?}: {inside}", entry.scan().id());
        let t = entry.scan().merger_age();
        let before = entry
            .timeline()
            .state_at(Years::new(t.value() * (1.0 - 1e-9)));
        assert_eq!(before.kind(), SegmentKind::Detached);
        assert_eq!(before.stars(), pair.stars());
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
            (kinds.first(), after),
            (Some(&SegmentKind::Detached), SegmentKind::Merged),
            "{:?}: {kinds:?}",
            entry.scan().id()
        );
        assert!(
            !kinds.contains(&SegmentKind::CommonEnvelope),
            "{:?}: {kinds:?}",
            entry.scan().id()
        );
        let product = entry.product();
        assert_eq!(product.phase(), Phase::MainSequence);
        assert!(
            (product.mass().value() / (md + mc) - 1.0).abs() < 1e-9,
            "{} against {}",
            product.mass().value(),
            md + mc
        );
        checked += 1;
    }
    assert!(checked > 50, "{checked}");
}

/// Every evolved donor's companion lies inside its donor's kind's mark's limits: a gap donor's
/// below 0.25 of it, a helium-burning donor's below a third, an asymptotic-giant-branch donor's and
/// a giant branch's anywhere in 0.05–1 (the scan's own unstable ratio holds the first two under
/// either arm; `scan::merger`'s `an_evolved_companion_s_mark_follows_its_donor_s_kind_from_27`
/// holds which mark each kind takes).
#[test]
fn every_evolved_companion_lies_inside_its_donor_s_kind_s_mark() {
    let mut kinds = [0_u32; 5];
    for entry in all_entries() {
        if entry.scan().channel() != MergerChannel::EvolvedDonor {
            continue;
        }
        let donor = entry.pair().stars()[0].phase();
        let k = EVOLVED
            .iter()
            .position(|&p| p == donor)
            .expect("an evolved donor");
        kinds[k] += 1;
        let mark = [
            MergerMark::GapMassRatio,
            MergerMark::EvolvedMassRatio,
            MergerMark::HeliumBurningMassRatio,
            MergerMark::AgbMassRatio,
            MergerMark::AgbMassRatio,
        ][k];
        let (lo, hi) = MERGER_SAMPLER.mark(mark).limits();
        let q = entry.scan().secondary_mass().value() / entry.scan().primary_mass().value();
        let floor =
            crate::stellar::sse::MIN_INITIAL_MASS.value() / entry.scan().primary_mass().value();
        assert!(
            q <= hi + 1e-12 && q >= lo.min(floor) - 1e-12,
            "{:?}: {donor:?}, q {q}",
            entry.scan().id()
        );
    }
    println!(
        "evolved donors by kind (gap, giant, helium burning, early and pulsing AGB): {kinds:?}"
    );
    assert!(kinds.iter().sum::<u32>() > 100, "{kinds:?}");
}

/// Every entry passes the shared test with its own class at every age of the source horizon:
/// before T its pair as marked, contact or detached, and from T the merged star.
#[test]
fn every_entry_is_its_class_at_every_age_under_every_27_hold() {
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

/// Under one candidate in ten that the site's thinning keeps fails the scan's checks or the
/// class's test, and the scan keeps exactly the cells' entries without running the engine.
#[test]
fn under_one_in_ten_fails_and_the_scan_keeps_the_entries_under_every_27_hold() {
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

/// The cells under every 27 hold do not depend on the order they are generated in, nor on what was
/// resolved between them.
#[test]
fn the_27_cells_do_not_depend_on_order() {
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

/// The engine members the slow test measures the class against: 48,000, an effective 1,233 under
/// these weights, 17 times P11.T8.c's 3,000 (an effective 73).
const REFERENCE_MEMBERS: usize = 48_000;

/// The class's entries the slow test draws per arm: 10,000, so that the deciles' own noise adds
/// about an eighth of the reference's effective number to a chi-square per degree of freedom.
const CATALOGUE_ENTRIES: usize = 10_000;

/// The catalogue cells about the reference's place: x in −1–0, y 6 and z in −1–0 (4,096 ly
/// cells), the 8,000 by 4,000 ly about the point 26,000 ly out along +y that the engine's members
/// are drawn from, in the plane.
fn reference_cells() -> [CatalogueCellKey; 4] {
    [[-1, 6, -1], [0, 6, -1], [-1, 6, 0], [0, 6, 0]].map(|at| {
        CatalogueCellKey::new(ClassId::STELLAR_MERGER, at).expect("a cell of the root cube")
    })
}

/// The marks the slow test compares, of a pair of masses `a` and `b` (M☉) whose primary began at
/// `m1`: log₁₀ of the total mass, the mass ratio and log₁₀ of the primary's initial mass.
fn compared_marks(a: f64, b: f64, m1: f64) -> [f64; 3] {
    use crate::math;
    [math::log10(a + b), a.min(b) / a.max(b), math::log10(m1)]
}

/// The compared marks and channel of the first [`CATALOGUE_ENTRIES`] entries of the class's own
/// candidates in [`reference_cells`] in `galaxy`, past each cell's count as well as inside it:
/// every candidate index is a draw of the same process, its site thinned on the class's
/// intensity, its marks on streams of its own ID, and its test the class's. Drawn on 16 threads,
/// each a run of indices across the four cells in turn, and joined in the threads' order.
fn catalogue(galaxy: &Galaxy) -> Vec<([f64; 3], MergerChannel)> {
    let process = StellarMergers::new(galaxy);
    let cells = reference_cells();
    let per_thread = CATALOGUE_ENTRIES.div_ceil(16);
    let mut out = Vec::with_capacity(CATALOGUE_ENTRIES + 16);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16_u32)
            .map(|thread| {
                let (process, cells) = (&process, &cells);
                scope.spawn(move || {
                    let bounds = cells.map(|key| process.bound(galaxy, &key));
                    let mut marks = Vec::with_capacity(per_thread);
                    let mut index = thread << 20;
                    while marks.len() < per_thread {
                        for (key, bound) in cells.iter().zip(bounds) {
                            let id = key.candidate_id(index, 0).expect("an index under 2^24");
                            let entry = process
                                .candidate(galaxy, key, id, bound)
                                .and_then(StellarMergers::test);
                            if let Some(entry) = entry {
                                let scan = entry.scan();
                                let (m1, m2) =
                                    (scan.primary_mass().value(), scan.secondary_mass().value());
                                marks.push((compared_marks(m1, m2, m1), scan.channel()));
                            }
                        }
                        index += 1;
                    }
                    marks.truncate(per_thread);
                    marks
                })
            })
            .collect();
        for handle in handles {
            out.extend(handle.join().expect("a thread's candidates finish"));
        }
    });
    out.truncate(CATALOGUE_ENTRIES);
    out
}

/// The weighted shares of `observed` in `expected`'s deciles, `decile_chi_square`'s cells.
fn decile_shares(expected: &[f64], observed: &[(f64, f64)]) -> [f64; 10] {
    let mut sorted = expected.to_vec();
    sorted.sort_by(f64::total_cmp);
    let edges: Vec<f64> = (1..10).map(|k| sorted[k * sorted.len() / 10]).collect();
    let mut shares = [0.0_f64; 10];
    for &(v, w) in observed {
        shares[edges.partition_point(|&e| e <= v)] += w;
    }
    let total: f64 = shares.iter().sum();
    shares.map(|s| s / total)
}

/// The weighted quantiles at `ranks` of the weighted values `v`.
fn weighted_quantiles(v: &mut [(f64, f64)], ranks: &[f64]) -> Vec<f64> {
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = v.iter().map(|x| x.1).sum();
    ranks
        .iter()
        .map(|r| {
            let mut sum = 0.0;
            v.iter()
                .find(|(_, w)| {
                    sum += w;
                    sum >= r * total
                })
                .map_or(f64::NAN, |x| x.0)
        })
        .collect()
}

/// The weighted shares of `masses` (M☉, with their weights) by the layer of their mass, A–E.
fn layer_shares(masses: impl Iterator<Item = (f64, f64)>) -> [f64; 5] {
    use crate::galaxy::imf::MassBand;
    let mut shares = [0.0_f64; 5];
    for (m, w) in masses {
        let band = MassBand::ALL.iter().rposition(|b| b.lo() <= m).unwrap_or(0);
        shares[band] += w;
    }
    let total: f64 = shares.iter().sum();
    shares.map(|s| s / total)
}

/// The engine's own mergers in `galaxy`, [`REFERENCE_MEMBERS`] of them with their pre-main-sequence
/// ones left out: each one's compared marks, its weight and whether the class's channels cover it
/// (two main-sequence stars, or an evolved donor beside a main-sequence star); with the split
/// among those and the others, the layers of its primaries, and its mass ratios by kind, printed.
fn engine_reference(galaxy: &Galaxy) -> Vec<([f64; 3], f64, bool)> {
    let mut split = [0.0_f64; 3];
    let mut by_kind: [Vec<(f64, f64)>; 5] = Default::default();
    let mut main_sequence: Vec<(f64, f64)> = Vec::new();
    let engine: Vec<([f64; 3], f64, bool)> = members(galaxy, REFERENCE_MEMBERS)
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
            let q = a.min(b) / a.max(b);
            let covered = if phases.iter().all(|p| *p == Phase::MainSequence) {
                split[0] += m.weight();
                main_sequence.push((q, m.weight()));
                true
            } else if let Some(k) = EVOLVED.iter().position(|&p| p == heavier)
                && lighter == Phase::MainSequence
            {
                split[1] += m.weight();
                by_kind[k].push((q, m.weight()));
                true
            } else {
                split[2] += m.weight();
                false
            };
            let marks = compared_marks(a, b, m.record().primary_initial_mass().value());
            Some((marks, m.weight(), covered))
        })
        .collect();
    let total: f64 = split.iter().sum();
    println!(
        "engine (weighted, pre-main-sequence left out): two main-sequence stars {:.3}, an evolved \
         donor beside a main-sequence star {:.3}, others {:.3}",
        split[0] / total,
        split[1] / total,
        split[2] / total
    );
    println!(
        "  by the primary's layer, A–E: {:.3?}",
        layer_shares(
            engine
                .iter()
                .map(|(m, w, _)| (crate::math::exp10(m[2]), *w))
        )
    );
    let ranks = [0.05, 0.25, 0.5, 0.75, 0.95];
    println!(
        "  two main-sequence stars' q at 5, 25, 50, 75, 95%: {:.3?}",
        weighted_quantiles(&mut main_sequence, &ranks)
    );
    for (phase, values) in EVOLVED.iter().zip(&mut by_kind) {
        if !values.is_empty() {
            println!(
                "  {phase:?} donors' q: {:.3?}",
                weighted_quantiles(values, &ranks)
            );
        }
    }
    engine
}

/// The class against the engine under every 27 hold and under the 26 arm (P11.T8.c's missing
/// channel): the pair's total mass, mass ratio and primary's initial mass of the engine's own
/// mergers, 48,000 members weighted as mergers in the horizon (an effective 1,233), in the
/// deciles of 10,000 of the class's entries in the same place ([`catalogue`]), by chi-square per
/// degree of freedom, pinned as measured (a miss of P15.T10.b's 3.1 is a finding); with the same
/// against the engine's mergers the class's channels cover, the engine's shares of the class's
/// deciles, and the engine's channel split and its donors' mass ratios by kind
/// ([`engine_reference`]), printed against the class's.
#[test]
#[ignore = "slow: 48,000 grid pairs run through the binary engine and 20,000 entries"]
fn the_engine_channels_against_the_engine_under_every_27_hold() {
    let galaxy = galaxy();
    let engine = engine_reference(galaxy);
    let held = held();
    let mut measured = [[0.0; 3]; 2];
    for (row, (name, g)) in [("every 27 hold", galaxy), ("the 26 arm", &held)]
        .into_iter()
        .enumerate()
    {
        let entries = catalogue(g);
        let shares: Vec<usize> = MergerChannel::ALL
            .iter()
            .map(|c| entries.iter().filter(|(_, e)| e == c).count())
            .collect();
        println!(
            "{name}: {} entries, by channel {shares:?}, by the primary's layer {:.3?}",
            entries.len(),
            layer_shares(entries.iter().map(|(m, _)| (crate::math::exp10(m[2]), 1.0)))
        );
        for (k, mark) in ["log total mass", "mass ratio", "log primary mass"]
            .iter()
            .enumerate()
        {
            let expected: Vec<f64> = entries.iter().map(|(m, _)| m[k]).collect();
            let observed: Vec<(f64, f64)> = engine.iter().map(|(m, w, _)| (m[k], *w)).collect();
            let (per_dof, effective) = decile_chi_square(&expected, &observed);
            let covered: Vec<(f64, f64)> = engine
                .iter()
                .filter(|(_, _, c)| *c)
                .map(|(m, w, _)| (m[k], *w))
                .collect();
            let (covered_per_dof, covered_effective) = decile_chi_square(&expected, &covered);
            println!(
                "  {mark}: chi2/dof {per_dof:.2} (effective {effective:.0}); against the engine's \
                 mergers the class's channels cover, {covered_per_dof:.2} (effective \
                 {covered_effective:.0}); the engine's shares of the class's deciles {:.3?}",
                decile_shares(&expected, &observed)
            );
            measured[row][k] = per_dof;
        }
    }
    for (row, pinned) in measured.iter().zip(PINNED_CHI_SQUARE) {
        for (m, p) in row.iter().zip(pinned) {
            assert!((m / p - 1.0).abs() < 1e-6, "{measured:?}");
        }
    }
}

/// The chi-square per degree of freedom of the total mass, the mass ratio and the primary's mass
/// under every 27 hold and under the 26 arm, as measured (2026-11-01, re-measured once every 27
/// hold took P15.T11.b's orphan streams, whose galaxy draws this reference, and again once it took
/// P11.T4's neutron-star accretion and P11.T6's Type Ia explosion together, and P11.T4's heavy
/// donors; at an effective 73 the 26 arm read 14.2, 12.1 and 16.0, `channels_hold`'s pins; at
/// generator version 25, measured again), all over P15.T10.b's 3.1: a finding, for P15.T10.b's fit.
/// The dynamical channel takes the mass ratio's from 167 to 68 (170 to 61 at 24); the masses' stay
/// high, the engine holding 13% of its primaries in layers D and E against the class's 1.3% and (at
/// 24) its giant-branch donors heavier (median 1.69 M☉) than the class's at the turn-off.
const PINNED_CHI_SQUARE: [[f64; 3]; 2] = [
    [
        214.125_299_548_683_64,
        68.196_568_091_053_56,
        311.814_346_742_384_1,
    ],
    [
        181.769_622_301_138_58,
        167.119_344_219_085_38,
        287.219_943_465_994_4,
    ],
];
