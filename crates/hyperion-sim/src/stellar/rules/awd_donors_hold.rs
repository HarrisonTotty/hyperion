//! The tests of P11.T5's fix: a white dwarf fed helium has no nova, and so is a slow host, read at
//! one choke point, `classify::nova_recurrence_under`. Its hold is deleted (the 27 fold); these are
//! its tests of the new arm.
//!
//! - A donor of each helium phase gives no nova and every other donor hydrogen's recurrence, bit
//!   for bit (`awd_donors.rs` holds the phases to the engine's helium surface and the census's
//!   helium kind).
//! - A pinned grid system: a layer-D record whose first attempt's 1.370 M☉ white dwarf is fed
//!   5.3 × 10⁻⁸ M☉ yr⁻¹ by a 0.477 M☉ helium star (at 27; at 25 and 26 a 1.264 M☉ dwarf fed
//!   3.8 × 10⁻⁸ M☉ yr⁻¹ by a 0.582 M☉ helium star, a fast host recurring every 60 years under the
//!   held arm) is a slow one with no nova, in the census's marks as in `carved_class`.
//! - The catalogue: the fast class's sample cells keep no host fed helium, its scan refusing them,
//!   and the scan keeps exactly the cells' entries. A finding, pinned: the slow class's hosts fed
//!   helium keep the nova clocks their scan draws.
//! - Slow: a census of 10⁶ slab records under every 27 hold, in which no helium-fed host is fast.

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::order::assert_order_independent;

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
use crate::stellar::binary::CarvedClass;
use crate::stellar::binary::scan::scan_cell;
use crate::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, ClassMember, class_census_in,
};
use crate::stellar::binary::{nova_recurrence_period, nova_recurrence_under};
use crate::tables::binary::AwdDonorKind;
use crate::units::{SolarMasses, SolarMassesPerYear};

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

/// The Milky Way fixture, under the version's rules.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
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

/// A donor of a helium phase gives no nova, every other hydrogen's recurrence bit for bit.
#[test]
fn a_white_dwarf_fed_helium_has_no_nova() {
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
            let arm = nova_recurrence_under(mass, rate, phase).value();
            if helium.contains(&phase) {
                assert!(arm.is_infinite() && arm > 0.0, "{phase:?}: {arm}");
            } else {
                assert_same_bits(arm, hydrogen);
            }
        }
    }
}

/// The pinned system's pair is a slow host, with no nova in its marks: its 1.370 M☉ white dwarf is
/// fed 5.3 × 10⁻⁸ M☉ yr⁻¹ by a 0.477 M☉ helium star, re-measured at 27, whose engine and grid move
/// the record's history (under the held arm, at generator versions 25 and 26, a 1.264 M☉ dwarf fed
/// 3.8 × 10⁻⁸ M☉ yr⁻¹ by a 0.582 M☉ helium star recurred every 60 years, a fast host; 1.262 M☉,
/// 0.583 M☉ and 61 years at 24).
#[test]
fn the_pinned_helium_fed_host_is_slow() {
    let id = SystemId::from_raw(PINNED).expect("a grid ID");
    let members = pinned_members(galaxy(), id);
    assert_eq!(members.len(), 1, "the pinned record's one member");
    let member = &members[0];
    assert_eq!(member.class(), CarvedClass::AccretingWdSlow);
    let a = member.accreting().expect("its marks");
    println!(
        "{:?} of {:.3} M_sun onto {:.3} M_sun at {:.3e} M_sun/yr: recurrence {:e}",
        a.donor_phase,
        a.donor_mass.value(),
        a.white_dwarf_mass.value(),
        a.transfer_rate.value(),
        a.recurrence.value()
    );
    assert_eq!(a.donor_kind(), AwdDonorKind::Helium);
    assert_eq!(a.donor_phase, Phase::HeliumMainSequence);
    assert!((a.white_dwarf_mass.value() - 1.3700).abs() < 5e-4, "{a:?}");
    assert!((a.donor_mass.value() - 0.4770).abs() < 5e-4, "{a:?}");
    assert!(
        a.recurrence.value().is_infinite() && a.recurrence.value() > 0.0,
        "no nova"
    );
}

/// The fast class's sample cells keep no host fed helium, and the scan keeps exactly their
/// entries, in any order: the fast side's scan reads the donor's kind and refuses them.
#[test]
fn the_fast_class_keeps_no_host_fed_helium() {
    let galaxy = galaxy();
    let (mut fed_helium, mut entries) = (0_u32, 0_u32);
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
                entries += 1;
                fed_helium +=
                    u32::from(entry.value().binary().donor_kind() == AwdDonorKind::Helium);
            }
        }
    }
    // The class's second stream leaves the cells independent of their order.
    let keys: Vec<CatalogueCellKey> = (-3..1)
        .map(|x| CatalogueCellKey::new(AwdSide::Fast.class(), [x, 50, 0]).expect("a cell"))
        .collect();
    assert_order_independent(&keys, |key| {
        CatalogueClassCell::generate(galaxy, &class, *key).into_entries()
    });
    println!("fast entries: {entries}, {fed_helium} fed helium");
    assert!(entries > 0, "the cells keep fast hosts");
    assert_eq!(fed_helium, 0);
}

/// A finding, pinned: the slow class's sample cells keep their hosts fed
/// helium, as slow hosts, and their scan marks still carry the hydrogen nova clock the scan draws
/// from the slow sampler's recurrence before the donor's kind, some of them erupting in the source
/// horizon. Giving them none needs scan marks without a nova (P11.T8.a's) or a recurrence drawn
/// after the kind (the refit's).
#[test]
fn the_slow_class_s_hosts_fed_helium_keep_their_nova_clocks() {
    let arm = galaxy();
    let process = AwdProcess::new(arm, AwdSide::Slow);
    let class = AccretingWdSlow::new(arm);
    let (mut fed_helium, mut erupting, mut entries) = (0_u32, 0_u32, 0_u32);
    for x in -3..3 {
        for z in [-1, 0] {
            let key = CatalogueCellKey::new(AwdSide::Slow.class(), [x, 50, z]).expect("a cell");
            let cell = CatalogueClassCell::generate(arm, &class, key).into_entries();
            let mut hosts = Vec::new();
            scan_cell(arm, &process, key, &mut hosts);
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
        "slow entries: {entries}, {fed_helium} fed helium, {erupting} of them \
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

/// Slow: over a census of 10⁶ slab records under every 27 hold no fast host is fed helium, and
/// every host fed helium is slow with no nova (under the held arm the helium-fed hosts were fast,
/// and made a large share of the fast hosts' novae).
#[test]
#[ignore = "slow: a census of 10⁶ records through the binary engine"]
fn the_helium_fed_hosts_leave_the_fast_class_in_a_census() {
    let galaxy = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_27_hold(),
    )
    .expect("the Milky Way fixture builds");
    let members = census(&galaxy, CENSUS_RECORDS);
    let mut slow_fed_helium = 0_u32;
    for member in &members {
        let Some(marks) = member.accreting() else {
            continue;
        };
        if marks.donor_kind() != AwdDonorKind::Helium {
            continue;
        }
        assert_ne!(
            member.class(),
            CarvedClass::AccretingWdFast,
            "a helium-fed fast host: {:?}",
            member.record().id()
        );
        if member.class() == CarvedClass::AccretingWdSlow {
            slow_fed_helium += 1;
            let recurrence = marks.recurrence.value();
            assert!(recurrence.is_infinite(), "{recurrence}");
        }
    }
    let (rate, fast) = fast_rate(&members);
    println!(
        "{CENSUS_RECORDS} records: {fast} fast hosts, {:.3e} novae a year a host; \
         {slow_fed_helium} slow hosts fed helium",
        rate / f64::from(fast)
    );
    assert!(
        slow_fed_helium > 0,
        "the census holds helium-fed slow hosts"
    );
}
