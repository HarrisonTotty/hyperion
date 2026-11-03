//! The tests of P11.T5's fix, [`AWD_DONORS_VERSION`]: a white dwarf fed helium has no nova, and so
//! is a slow host, only under the rules that turn it on, read at one choke point,
//! `classify::nova_recurrence_under`, through each timeline's rules.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Under the arm a donor of each helium phase gives no nova and every other donor the held
//!   recurrence, bit for bit; under the held arm every donor gives it (`awd_donors.rs` holds the
//!   phases to the engine's helium surface and the census's helium kind).
//! - Under the arm alone a pinned grid system moves: a layer-D record whose first attempt's 1.264
//!   M☉ white dwarf is fed 3.8 × 10⁻⁸ M☉ yr⁻¹ by a 0.582 M☉ helium star, a fast host recurring
//!   every 60 years under the held arm (from generator version 25), is a slow one with no nova, in
//!   the census's marks as in `carved_class`.
//! - The catalogue: under the arm alone the fast class's sample cells keep no host fed helium, its
//!   scan refusing them, and the scan keeps exactly the cells' entries. A finding, pinned: the slow
//!   class's hosts fed helium keep the nova clocks their scan draws.
//! - Slow: a census of 10⁶ slab records under every 27 hold and under every 27 hold but this one.
//!   The members are the same pairs and differ only where a fast host is fed helium, which is slow
//!   with no nova under the arm, so no helium-fed host is fast; and the fast hosts' nova rate,
//!   Σ 1/`P_rec`, falls by the helium-fed hosts' share of it.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's tests and the class's slow census: the golden
//! `catalogue_classes/accreting_white_dwarfs`, `awd::tests::the_hosts_layers_against_the_share_rows`
//! and, slow, `tables_hold::the_accreting_white_dwarf_samplers_against_the_engine`'s pinned misses,
//! each as the fast hosts fed helium leave the class; and this module's tests of the version's arm
//! (2026-11-01). Nothing moves at 24–26.

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::{
    AccretingWdFast, AccretingWdSlow, AwdProcess, AwdSide,
};
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell};
use crate::galaxy::imf::{BandShares, MassBand};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::galaxy::rules::GalaxyRules;
use crate::id::{Layer, SystemId};
use crate::planetary::testing::in_shares;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::scan::scan_cell;
use crate::stellar::binary::testing::{
    AccretingMarks, CensusPart, CensusRecords, CensusSite, ClassMember, class_census_in,
};
use crate::stellar::binary::{CarvedClass, SegmentKind};
use crate::stellar::binary::{nova_recurrence_period, nova_recurrence_under};
use crate::tables::binary::AwdDonorKind;
use crate::units::{SolarMasses, SolarMassesPerYear, Years};

/// The seed of the Milky Way fixture: `hyperion-fit`'s `binary_awd.toml`'s, whose census found
/// the pinned system.
const SEED: u64 = 0x0b17_15a0_0000_0001;

/// The pinned grid system: a layer-D record whose first attempt's white dwarf a helium star feeds
/// (found by a census of 4 × 10⁵ slab records, one of its four fast hosts fed helium).
const PINNED: u64 = 0x6259_da17_ee00_000b;

/// The slow census's slab records, layers C–E together: the brief's 10⁶.
const CENSUS_RECORDS: u64 = 1_000_000;

/// The parts the slow census's cells are cut into, one job each.
const CENSUS_PARTS: u64 = 16;

/// The threads the slow census runs on: its reservation in `.config/nextest.toml`.
const CENSUS_THREADS: usize = 16;

/// The bands a census runs, whose primaries can make a member.
const CENSUS_BANDS: [MassBand; 3] = [MassBand::C, MassBand::D, MassBand::E];

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn hydrogen_rules() -> StellarRules {
    StellarRules {
        awd_donors: AwdDonorRules::HydrogenDonors,
        ..version()
    }
}

/// The Milky Way fixture, under the version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// `galaxy` under its own rules with the stellar rules `stellar`.
fn with_stellar(galaxy: &Galaxy, stellar: StellarRules) -> Galaxy {
    let rules = galaxy.rules();
    galaxy.clone().with_rules(GalaxyRules { stellar, ..rules })
}

/// The members of the census of the one layer-D cell that holds grid record `id` in `galaxy`, and
/// the record's own.
fn pinned_members(galaxy: &Galaxy, id: SystemId) -> Vec<ClassMember> {
    let record = placement::resolve(galaxy, id).expect("the pinned record resolves");
    assert_eq!(record.layer(), Layer::D);
    let centre = record.epoch_position().to_light_years_f64().map(|v| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a position in the root cube, inside the i32 light-years of its frame"
        )]
        let ly = v.floor() as i32;
        ly
    });
    class_census_in(
        galaxy,
        MassBand::D,
        CensusSite::new(centre, [0, 0, 0]),
        CensusRecords::All,
        u64::MAX,
        CensusPart::new(0, 1),
    )
    .into_members()
    .into_iter()
    .filter(|m| m.record().id() == id)
    .collect()
}

/// `member`'s timeline's segments, each its kind and its span, Julian years: what the engine made
/// of the pair, which the timeline's rules, part of its value, do not compare.
fn spans(member: &ClassMember) -> Vec<(SegmentKind, Years, Years)> {
    member
        .timeline()
        .segments()
        .iter()
        .map(|s| (s.kind(), s.start(), s.end()))
        .collect()
}

/// The version's rules keep the held arm, every version below 27 does and 27 gives a hydrogen
/// donor alone its novae. The tripwire: the bump to 27 deletes this hold.
#[test]
fn the_awd_donors_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < AWD_DONORS_VERSION,
        "the bump to 27 deletes P11.T5's helium-donor hold: `AWD_DONORS_VERSION`, \
         `AwdDonorRules` with its `EveryDonor` arm and `has_novae`, `StellarRules::awd_donors`, \
         its readers' calls in `nova_recurrence_under` and `AwdProcess::has_no_nova`, this \
         module and its line in `stellar/rules.rs`"
    );
    assert_eq!(version().awd_donors, AwdDonorRules::EveryDonor);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).awd_donors;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), AwdDonorRules::EveryDonor, "version {v}");
    }
    assert_eq!(at(27), AwdDonorRules::HydrogenDonors);
    assert_eq!(
        StellarRules::every_26_hold_with_its_retargets().awd_donors,
        AwdDonorRules::EveryDonor
    );
    assert_eq!(
        StellarRules::every_27_hold().awd_donors,
        AwdDonorRules::HydrogenDonors
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar.awd_donors,
        AwdDonorRules::HydrogenDonors
    );
}

/// Under the arm a donor of a helium phase gives no nova, every other the held recurrence bit for
/// bit; under the held arm every donor gives it.
#[test]
fn a_white_dwarf_fed_helium_has_no_nova_under_the_arm() {
    let helium = [
        Phase::HeliumMainSequence,
        Phase::HeliumHertzsprungGap,
        Phase::HeliumGiantBranch,
        Phase::HeliumWhiteDwarf,
    ];
    for (mass, rate) in [(0.6, 1.0e-10), (1.262, 3.8e-8), (1.38, 6.6e-11)] {
        let (mass, rate) = (SolarMasses::new(mass), SolarMassesPerYear::new(rate));
        let hydrogen = nova_recurrence_period(mass, rate).value();
        assert!(hydrogen.is_finite());
        for phase in Phase::ALL {
            let held = nova_recurrence_under(mass, rate, phase, version()).value();
            assert_same_bits(held, hydrogen);
            let arm = nova_recurrence_under(mass, rate, phase, hydrogen_rules()).value();
            if helium.contains(&phase) {
                assert!(arm.is_infinite() && arm > 0.0, "{phase:?}: {arm}");
            } else {
                assert_same_bits(arm, hydrogen);
            }
        }
    }
}

/// Under the arm alone the pinned system's pair moves from the fast class to the slow one, with no
/// nova in its marks, and nothing else of it moves: under the held arm its 1.264 M☉ white dwarf,
/// fed 3.8 × 10⁻⁸ M☉ yr⁻¹ by a 0.582 M☉ helium star, recurs every 60 years (at generator versions
/// 25 and 26; 1.262 M☉, 0.583 M☉ and 61 years at 24).
#[test]
fn the_pinned_helium_fed_host_is_slow_under_the_arm() {
    let id = SystemId::from_raw(PINNED).expect("a grid ID");
    let arm = with_stellar(held(), hydrogen_rules());
    let [before, after] = [held(), &arm].map(|galaxy| pinned_members(galaxy, id));
    assert_eq!(before.len(), 1, "the pinned record's one member");
    assert_eq!(after.len(), 1);
    let (held_member, arm_member) = (&before[0], &after[0]);
    assert_eq!(held_member.class(), CarvedClass::AccretingWdFast);
    assert_eq!(arm_member.class(), CarvedClass::AccretingWdSlow);
    assert_eq!(spans(held_member), spans(arm_member));
    let (h, a) = (
        held_member.accreting().expect("its marks"),
        arm_member.accreting().expect("its marks"),
    );
    println!(
        "{:?} of {:.3} M_sun onto {:.3} M_sun at {:.3e} M_sun/yr: held {:.3e} yr, under the arm {:e}",
        h.donor_phase,
        h.donor_mass.value(),
        h.white_dwarf_mass.value(),
        h.transfer_rate.value(),
        h.recurrence.value(),
        a.recurrence.value()
    );
    assert_eq!(h.donor_kind(), AwdDonorKind::Helium);
    assert_eq!(h.donor_phase, Phase::HeliumMainSequence);
    assert!((h.white_dwarf_mass.value() - 1.2645).abs() < 5e-4);
    assert!((h.donor_mass.value() - 0.5823).abs() < 5e-4);
    assert!((h.recurrence.value() / 59.54 - 1.0).abs() < 1e-3);
    assert!(
        a.recurrence.value().is_infinite() && a.recurrence.value() > 0.0,
        "no nova"
    );
    let unmoved = |m: &AccretingMarks| {
        (
            m.age,
            m.white_dwarf_mass,
            m.donor_mass,
            m.donor_phase,
            m.period,
            m.transfer_rate,
            m.magnetic,
        )
    };
    assert_eq!(unmoved(h), unmoved(a));
}

/// Under the arm alone the fast class's sample cells keep no host fed helium, and the scan keeps
/// exactly their entries, in any order; under the held arm the same cells keep some, so that the
/// fast side's scan, which reads the donor's kind under the arm alone, is what refuses them. The class's test
/// reads a catalogue timeline under the version's rules, which keep every donor's novae until the
/// bump.
#[test]
fn the_fast_class_keeps_no_host_fed_helium_under_the_arm() {
    let arm = with_stellar(held(), hydrogen_rules());
    let mut fed_helium = [0_u32; 2];
    let mut entries = [0_u32; 2];
    for (k, galaxy) in [held(), &arm].into_iter().enumerate() {
        let process = AwdProcess::new(galaxy, AwdSide::Fast);
        let class = AccretingWdFast::new(galaxy);
        for x in -3..3 {
            for z in [-1, 0] {
                let key = CatalogueCellKey::new(AwdSide::Fast.class(), [x, 50, z]).expect("a cell");
                let cell = CatalogueClassCell::generate(galaxy, &class, key).into_entries();
                let mut hosts = Vec::new();
                scan_cell(galaxy, &process, key, &mut hosts);
                let kept: Vec<_> = cell.iter().map(|e| e.value().scan()).collect();
                assert_eq!(hosts.iter().collect::<Vec<_>>(), kept, "{key:?}");
                for entry in &cell {
                    entries[k] += 1;
                    fed_helium[k] +=
                        u32::from(entry.value().binary().donor_kind() == AwdDonorKind::Helium);
                }
            }
        }
    }
    // The arm's second stream leaves the cells independent of their order.
    let class = AccretingWdFast::new(&arm);
    let keys: Vec<CatalogueCellKey> = (-3..1)
        .map(|x| CatalogueCellKey::new(AwdSide::Fast.class(), [x, 50, 0]).expect("a cell"))
        .collect();
    assert_order_independent(&keys, |key| {
        CatalogueClassCell::generate(&arm, &class, *key).into_entries()
    });
    println!(
        "fast entries: {} held, {} fed helium; {} under the arm, {} fed helium",
        entries[0], fed_helium[0], entries[1], fed_helium[1]
    );
    assert!(
        fed_helium[0] > 0,
        "the held arm's cells keep hosts fed helium"
    );
    assert_eq!(fed_helium[1], 0);
    assert_eq!(entries[1], entries[0] - fed_helium[0]);
}

/// A finding, pinned: under the arm alone the slow class's sample cells keep their hosts fed
/// helium, as slow hosts, and their scan marks still carry the hydrogen nova clock the scan draws
/// from the slow sampler's recurrence before the donor's kind, some of them erupting in the source
/// horizon. Giving them none needs scan marks without a nova (P11.T8.a's) or a recurrence drawn
/// after the kind (the refit's).
#[test]
fn the_slow_class_s_hosts_fed_helium_keep_their_nova_clocks() {
    let arm = with_stellar(held(), hydrogen_rules());
    let process = AwdProcess::new(&arm, AwdSide::Slow);
    let class = AccretingWdSlow::new(&arm);
    let (mut fed_helium, mut erupting, mut entries) = (0_u32, 0_u32, 0_u32);
    for x in -3..3 {
        for z in [-1, 0] {
            let key = CatalogueCellKey::new(AwdSide::Slow.class(), [x, 50, z]).expect("a cell");
            let cell = CatalogueClassCell::generate(&arm, &class, key).into_entries();
            let mut hosts = Vec::new();
            scan_cell(&arm, &process, key, &mut hosts);
            assert_eq!(hosts.len(), cell.len(), "{key:?}");
            for entry in &cell {
                entries += 1;
                if entry.value().binary().donor_kind() == AwdDonorKind::Helium {
                    fed_helium += 1;
                    let scan = entry.value().scan();
                    assert!(scan.recurrence().value().is_finite());
                    erupting += u32::from(!scan.eruptions().is_empty());
                }
            }
        }
    }
    println!(
        "slow entries under the arm: {entries}, {fed_helium} fed helium, {erupting} of them \
         erupting in the source horizon"
    );
    assert!(fed_helium > 0, "the cells keep slow hosts fed helium");
}

/// A census of `records` slab records in `galaxy`, each band's in its share of the systems, its
/// [`CENSUS_PARTS`] parts joined in order, on [`CENSUS_THREADS`] threads.
fn census(galaxy: &Galaxy, records: u64) -> Vec<ClassMember> {
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = CENSUS_BANDS
        .iter()
        .fold(0.0, |sum, &band| sum + shares.share(band));
    let jobs: Vec<(MassBand, u64)> = CENSUS_BANDS
        .iter()
        .flat_map(|&band| (0..CENSUS_PARTS).map(move |k| (band, k)))
        .collect();
    in_shares(jobs.len(), CENSUS_THREADS, |i| {
        let (band, k) = jobs[i];
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a record count under 2⁵³, rounded to a whole count"
        )]
        let in_band = (records as f64 * shares.share(band) / in_bands).round() as u64;
        class_census_in(
            galaxy,
            band,
            CensusSite::SOLAR_CIRCLE,
            CensusRecords::All,
            in_band.div_ceil(CENSUS_PARTS),
            CensusPart::new(k, CENSUS_PARTS),
        )
        .into_members()
    })
    .into_iter()
    .flatten()
    .collect()
}

/// Σ 1/`P_rec` over `members`' fast hosts, a year, and how many there are.
fn fast_rate<'a>(members: impl IntoIterator<Item = &'a ClassMember>) -> (f64, u32) {
    members
        .into_iter()
        .filter(|m| m.class() == CarvedClass::AccretingWdFast)
        .fold((0.0, 0), |(sum, n), m| {
            let recurrence = m.accreting().expect("a fast host's marks").recurrence;
            (sum + 1.0 / recurrence.value(), n + 1)
        })
}

/// Slow: over a census of 10⁶ slab records under every 27 hold, and under every 27 hold but this
/// one, the members are the same pairs in the same order, and differ only where the held arm's
/// fast host is fed helium: under the arm that host is slow, with no nova, so no helium-fed host
/// is fast. The fast hosts' nova rate falls by the helium-fed hosts' share of it, to rounding.
#[test]
#[ignore = "slow: two censuses of 10⁶ records through the binary engine"]
fn the_helium_fed_hosts_leave_the_fast_class_in_a_census() {
    // Built under each, since the displaced classes' table, from 26, is built under the galaxy's
    // stellar rules.
    let every_27 = GalaxyRules::every_27_hold();
    let [held, arm] = [
        GalaxyRules {
            stellar: StellarRules {
                awd_donors: AwdDonorRules::EveryDonor,
                ..every_27.stellar
            },
            ..every_27
        },
        every_27,
    ]
    .map(|rules| {
        Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
            .expect("the Milky Way fixture builds")
    });
    let [held_members, arm_members] = [&held, &arm].map(|galaxy| census(galaxy, CENSUS_RECORDS));
    assert_eq!(held_members.len(), arm_members.len());
    let mut moved = Vec::new();
    for (h, a) in held_members.iter().zip(&arm_members) {
        assert_eq!(h.record(), a.record());
        assert_eq!(spans(h), spans(a));
        let fed_helium = h
            .accreting()
            .is_some_and(|m| m.donor_kind() == AwdDonorKind::Helium);
        if a.class() == CarvedClass::AccretingWdFast {
            assert!(!fed_helium, "a helium-fed fast host: {:?}", a.record().id());
        }
        if h.class() == a.class() {
            continue;
        }
        assert!(
            fed_helium,
            "{:?} moved with a hydrogen donor",
            a.record().id()
        );
        assert_eq!(
            (h.class(), a.class()),
            (CarvedClass::AccretingWdFast, CarvedClass::AccretingWdSlow)
        );
        let recurrence = a.accreting().expect("its marks").recurrence.value();
        assert!(recurrence.is_infinite(), "{recurrence}");
        moved.push(h);
    }
    let (held_rate, held_fast) = fast_rate(&held_members);
    let (arm_rate, arm_fast) = fast_rate(&arm_members);
    let (helium_rate, helium_fast) = fast_rate(moved.iter().copied());
    let records = CENSUS_RECORDS;
    println!(
        "{records} records: {held_fast} fast hosts held, {arm_fast} under the arm; {helium_fast} \
         fed helium ({:.2}% of the hosts) made {:.1}% of the novae; {:.3e} novae a year a host \
         held, {:.3e} under the arm",
        100.0 * f64::from(helium_fast) / f64::from(held_fast),
        100.0 * helium_rate / held_rate,
        held_rate / f64::from(held_fast),
        arm_rate / f64::from(arm_fast),
    );
    assert!(helium_fast > 0, "the census holds helium-fed fast hosts");
    assert_eq!(arm_fast, held_fast - helium_fast);
    let fall = 1.0 - arm_rate / held_rate;
    assert!(
        (fall - helium_rate / held_rate).abs() < 1e-9,
        "the rate fell by {fall}, the helium-fed hosts' share {}",
        helium_rate / held_rate
    );
}
