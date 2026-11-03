//! The tests of P11.T5's and P11.T8.b's fix, [`XRB_CLASS_VERSION`]: what `classify` counts as an
//! X-ray binary, a floor on the rate of Roche-lobe overflow onto a compact star, Be donors spun up
//! by stable transfer and a disc for a wind-fed black hole, only under the rules that turn it on,
//! read through [`XrbClassRules::carves`] and each timeline's rules.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Under the arm alone four pinned grid systems move, one a rule, found by censuses of 3 × 10⁵
//!   slab and 3 × 10⁵ young thin disc records: a neutron star fed 3.5 × 10⁻¹³ M☉ yr⁻¹ by a
//!   helium white dwarf is no X-ray binary (from generator version 25); a 24.5 M☉ star's wind
//!   feeding a black hole 30 days out, far inside its Roche lobe, makes none; a 12.3 M☉
//!   main-sequence star that gained mass from its neutron star's progenitor is a Be star, its pair
//!   a Be/X-ray binary; and a Be star beside a black hole makes none.
//! - The catalogue: under the arm and plan 15's fitted tables the scan refuses the candidates its
//!   sampler feeds under the floor, which the class's test will refuse at the bump, and keeps
//!   exactly the cells' entries; and a pair's classes at its ages do not depend on the order they
//!   are asked in.
//! - Slow: censuses of 10⁶ slab and 10⁶ young thin disc records under every 27 hold and under
//!   every 27 hold but this one: no member is fed under the floor, every member has a kind, the
//!   hydrogen-fed low-mass X-ray binaries it takes are pinned, and the young disc's Be/X share of its neutron-star
//!   high-mass hosts and its black holes' share of its high-mass hosts against their cited
//!   figures, pinned as measured.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's tests and the slow tests of the binaries, the catalogue
//! and the sky: the golden `sky/census_near_sun` (the grid's carve near the Sun) and, slow,
//! `tables_hold::the_x_ray_binary_sampler_against_the_engine`'s pinned misses (its Be/X members)
//! and `merger::channels_hold::the_channel_sampler_against_the_engine_under_every_26_hold`'s (the
//! grid redraws other systems); `carve::tests::a_carved_first_attempt_is_redrawn` and
//! `multiplicity::bound::tests::the_carved_record_lists_the_attempt_it_keeps`, whose pinned
//! record's first attempt is no X-ray binary under the arm (with the Be stars' rule); and this
//! module's tests of the version's arm (2026-11-01). Nothing moves at 24–26.

use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::threads::in_shares;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::catalogue_classes::binary::{BinaryTableRules, XrayBinaries};
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use crate::galaxy::imf::{BandShares, MassBand};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::SystemId;
use crate::math;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::scan::{XrbScanMarks, scan_cell};
use crate::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, ClassMember, XrayMarks, class_census_in,
};
use crate::stellar::binary::xrb_class::{SPUN_UP_GAINED_SHARE, XRB_MIN_TRANSFER_RATE};
use crate::stellar::binary::{BinaryClass, CarvedClass, Component, HmxbKind, SegmentKind, XrbKind};
use crate::tables::binary::{
    ClassSamplerTable, SamplerPick, XRB_SAMPLER_27, XrbHostKind, XrbMark, XrbPick,
};
use crate::units::{Seconds, Years};

/// The seed of the Milky Way fixture: `hyperion-fit`'s manifests', whose census found the pinned
/// systems.
const SEED: u64 = 0x0b17_15a0_0000_0001;

/// A layer-D record of the slab whose first attempt is a neutron star fed 3.5 × 10⁻¹³ M☉ yr⁻¹ by
/// a 0.0075 M☉ helium white dwarf on a 1.6-hour orbit (found at generator versions 25 and 26 by a
/// census of 3 × 10⁵ slab records, the first of four; at 24 it was `0x621d_daa7_f800_0000`, fed
/// 2.7 × 10⁻¹⁴ M☉ yr⁻¹).
const UNDER_THE_FLOOR: u64 = 0x61fb_d8d7_ea00_0007;

/// A layer-E record of the young thin disc whose first attempt is a black hole fed by the wind of
/// a 24.5 M☉ main-sequence star 30.3 days out.
const DISCLESS_BLACK_HOLE: u64 = 0x816d_5b1f_f000_0012;

/// A layer-E record of the slab whose first attempt is a neutron star 70.5 days from a 12.3 M☉
/// main-sequence star that gained mass in stable transfer.
const SPUN_UP_BE_STAR: u64 = 0x81af_b360_0000_0006;

/// The young thin disc's census site, `hyperion-fit`'s and `tables_hold`'s.
const YOUNG_SITE: CensusSite = CensusSite::new([0, 0, 0], [40_000, 40_000, 600]);

/// Each slow census's records, layers C–E together.
const CENSUS_RECORDS: u64 = 1_000_000;

/// The parts the slow censuses' cells are cut into, one job each.
const CENSUS_PARTS: u64 = 16;

/// The threads the slow censuses run on: their reservation in `.config/nextest.toml`.
const CENSUS_THREADS: usize = 16;

/// A layer-E record of the young thin disc whose first attempt is a 4.1 M☉ black hole 26.9 days
/// from a 19.3 M☉ main-sequence star that gained mass in stable transfer, a Be/X-ray binary under
/// the arm before its Be rule (found by a census of 3 × 10⁵ young thin disc records under the arm
/// alone).
const BLACK_HOLE_BESIDE_A_BE_STAR: u64 = 0x8273_a85f_d000_000b;

/// The bands a census runs, whose primaries can make a member.
const CENSUS_BANDS: [MassBand; 3] = [MassBand::C, MassBand::D, MassBand::E];

/// Be/X-ray binaries' share of the Galaxy's Be/X and supergiant X-ray binaries: about 0.65, Liu,
/// van Paradijs and van den Heuvel's (2006, A&A 455, 1165) 60% Be/X and 32% supergiant systems of
/// 114 (recalled), nearly all of whose compact stars are neutron stars.
const CITED_BEX_SHARE: f64 = 0.65;

/// Black holes' greatest share of the Galaxy's high-mass X-ray binaries: one of about a hundred,
/// Cyg X-1 (Liu et al. 2006; recalled). MWC 656, a Be star beside a black hole (Casares et al.
/// 2014, Nature 505, 378), is too faint in X-rays to be catalogued as one.
const CITED_BLACK_HOLE_SHARE: f64 = 0.01;

/// The young disc's high-mass hosts under every 27 hold, as measured: Be/X and supergiant neutron
/// stars and black holes. Black holes are 0.005 of all, within [`CITED_BLACK_HOLE_SHARE`], since
/// a Be star beside one makes no X-ray binary (main's ruling of 2026-11-01: before that rule 62
/// of the 64 black holes, 0.145 of all, were beside Be stars). Be/X are 0.989 of the neutron stars'
/// against [`CITED_BEX_SHARE`] (a finding: the census counts every neutron star beside a Be star,
/// quiescent or not, where the catalogued share is of those seen; the Be/X phase outlasts the
/// supergiant one some 10–100 times, so the Galaxy hides thousands of quiescent Be/X beside its
/// tens of supergiant systems, Meurs and van den Heuvel 1989, A&A 226, 88, recalled; the spun-up
/// Be/X's orbits are the observed ones, 301 of 360 at 10–330 days).
const PINNED_HIGH_MASS: [usize; 3] = [373, 4, 2];

/// The census's members of each kind under every 27 hold, in [`XrbHostKind::ALL`]'s order, in
/// the slab and in the young disc, as measured (2026-11-02, with P11.T4's pinned white dwarfs):
/// the floor of 10⁻¹² keeps 76 of the slab's 456 ultracompacts that 10⁻¹³ kept, and 184 of its
/// 567 members.
const PINNED_KINDS: [[usize; 9]; 2] = [
    [1, 98, 1, 2, 1, 0, 76, 5, 0],
    [2, 14, 31, 17, 373, 4, 27, 30, 2],
];

/// The longest period of an ultracompact member under every 27 hold: 80 minutes, the
/// ultracompact X-ray binaries' bound (in 't Zand, Jonker and Markwardt 2007, A&A 465, 953;
/// recalled), which the floor alone keeps in the slab (research-leans-30 §4).
const ULTRACOMPACT_MAX_PERIOD: Seconds = Seconds::new(80.0 * 60.0);

/// The ultracompact members at [`ULTRACOMPACT_MAX_PERIOD`] or longer under every 27 hold, in the
/// slab and in the young disc, as measured: none of the slab's, and ten of the young disc's, seven
/// neutron stars and three black holes fed through their inner Lagrangian points by helium stars
/// of 0.8–4.4 M☉, on their main sequence or in their Hertzsprung gap, 81 minutes to 3.1 days out
/// at 9 × 10⁻¹² to 10⁻³ M☉ yr⁻¹ (a finding: kinded ultracompact by their helium donors, not by
/// their periods).
const PINNED_LONG_ULTRACOMPACTS: [usize; 2] = [0, 10];

/// The hydrogen-fed low-mass X-ray binaries the floor takes under every 27 hold, in the slab and
/// in the young disc, as measured: three of the slab's, neutron-star transients beside substellar
/// donors of 0.012–0.014 M☉ 1.8–2.0 hours out fed 5.6–9.0 × 10⁻¹³ M☉ yr⁻¹ (a finding: the floor
/// of 10⁻¹² takes the faintest of the very faint transients), and two of the young disc's,
/// neutron stars beside 1.8–1.9 M☉ main-sequence stars 13–15 hours out fed 5.5 × 10⁻¹³ and
/// 5.5 × 10⁻¹⁶ M☉ yr⁻¹ (a finding: P11.T4's heavy donors leave a main-sequence donor barely in
/// contact).
const PINNED_FLOORED_HYDROGEN: [usize; 2] = [3, 2];

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The Milky Way fixture, under the version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// The fixture under the version's rules with this hold's arm on alone.
fn carved() -> Galaxy {
    let rules = held().rules();
    held().clone().with_rules(GalaxyRules {
        stellar: StellarRules {
            xrb_class: XrbClassRules::Carved,
            ..version()
        },
        ..rules
    })
}

/// The X-ray binary of grid record `id` in `galaxy`, if its first attempt makes one: the census of
/// the one cell that holds it, its member.
fn pinned(galaxy: &Galaxy, id: u64) -> Option<ClassMember> {
    let id = SystemId::from_raw(id).expect("a grid ID");
    let record = placement::resolve(galaxy, id).expect("the pinned record resolves");
    let band = MassBand::from(record.layer());
    assert!(
        CENSUS_BANDS.contains(&band),
        "a pinned record of band {band:?}"
    );
    let centre = record.epoch_position().to_light_years_f64().map(|v| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a position in the root cube, inside the i32 light-years of its frame"
        )]
        let ly = v.floor() as i32;
        ly
    });
    let members: Vec<ClassMember> = class_census_in(
        galaxy,
        band,
        CensusSite::new(centre, [0, 0, 0]),
        CensusRecords::All,
        u64::MAX,
        CensusPart::new(0, 1),
    )
    .into_members()
    .into_iter()
    .filter(|m| m.record().id() == id && m.class() == CarvedClass::XrayBinary)
    .collect();
    assert!(members.len() <= 1, "one pair of the record");
    members.into_iter().next()
}

/// The X-ray binary marks of [`pinned`]'s member.
fn pinned_marks(galaxy: &Galaxy, id: u64) -> Option<XrayMarks> {
    pinned(galaxy, id).map(|m| *m.xray().expect("an X-ray binary's marks"))
}

/// The version's rules keep the held arm, every version below 27 does and 27 carves anew. The
/// tripwire: the bump to 27 deletes this hold.
#[test]
fn the_xrb_class_is_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < XRB_CLASS_VERSION,
        "the bump to 27 deletes P11.T5's and P11.T8.b's X-ray binary hold: `XRB_CLASS_VERSION`, \
         `XrbClassRules` with its `Held` arm, `StellarRules::xrb_class`, `classify`'s four \
         calls of `carves` and the context's gainer masses, the X-ray binaries' scan's, this \
         module and its line in `stellar/rules.rs`"
    );
    assert_eq!(version().xrb_class, XrbClassRules::Held);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).xrb_class;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), XrbClassRules::Held, "version {v}");
    }
    assert_eq!(at(27), XrbClassRules::Carved);
    assert_eq!(
        StellarRules::every_27_hold().xrb_class,
        XrbClassRules::Carved
    );
    assert_eq!(
        GalaxyRules::every_27_hold().stellar.xrb_class,
        XrbClassRules::Carved
    );
    assert!(!XrbClassRules::Held.carves());
    assert!(XrbClassRules::Carved.carves());
}

/// Under the arm alone a neutron star fed by a helium white dwarf under the floor is no X-ray
/// binary: under the held arm it is a low-mass transient fed 3.5 × 10⁻¹³ M☉ yr⁻¹.
#[test]
fn a_transfer_under_the_floor_makes_no_x_ray_binary_under_the_arm() {
    let held_marks = pinned_marks(held(), UNDER_THE_FLOOR).expect("an X-ray binary held");
    println!("{held_marks:?}");
    assert_eq!(
        held_marks.class,
        BinaryClass::LowMassXrayBinary(XrbKind::Transient)
    );
    assert_eq!(held_marks.accretor_phase, Phase::NeutronStar);
    assert_eq!(held_marks.donor_phase, Phase::HeliumWhiteDwarf);
    let rate = held_marks.transfer_rate.expect("a stable transfer");
    assert!(rate < XRB_MIN_TRANSFER_RATE, "{rate:?}");
    assert_eq!(held_marks.member_kind(), Some(XrbHostKind::Ultracompact));
    assert_eq!(pinned_marks(&carved(), UNDER_THE_FLOOR), None);
}

/// Under the arm alone a black hole fed by a massive star's wind far inside its Roche lobe is no
/// X-ray binary: under the held arm it is a supergiant one.
#[test]
fn a_wind_fed_black_hole_needs_a_disc_under_the_arm() {
    let held_marks = pinned_marks(held(), DISCLESS_BLACK_HOLE).expect("an X-ray binary held");
    println!("{held_marks:?}");
    assert_eq!(
        held_marks.class,
        BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant)
    );
    assert_eq!(held_marks.accretor_phase, Phase::BlackHole);
    assert_eq!(held_marks.transfer_rate, None, "fed by a wind");
    assert_eq!(
        held_marks.member_kind(),
        Some(XrbHostKind::BlackHoleHighMass)
    );
    assert_eq!(pinned_marks(&carved(), DISCLESS_BLACK_HOLE), None);
}

/// Under the arm alone a B star that gained mass in stable transfer is a Be star, its pair with a
/// neutron star a Be/X-ray binary; under the held arm its own draw makes it none, and the pair no
/// X-ray binary.
#[test]
fn a_b_star_spun_up_by_transfer_is_a_be_star_under_the_arm() {
    assert_eq!(pinned_marks(held(), SPUN_UP_BE_STAR), None);
    let member = pinned(&carved(), SPUN_UP_BE_STAR).expect("an X-ray binary under the arm");
    let marks = *member.xray().expect("its marks");
    println!("{marks:?}");
    assert_eq!(marks.class, BinaryClass::HighMassXrayBinary(HmxbKind::BeX));
    assert_eq!(marks.accretor_phase, Phase::NeutronStar);
    assert_eq!(marks.donor_phase, Phase::MainSequence);
    assert_eq!(marks.member_kind(), Some(XrbHostKind::BeX));
    // Its gain: from its mass where it first took the compact star's progenitor's matter.
    let timeline = member.timeline();
    let state = timeline.state_at(marks.age);
    let be = Component::BOTH
        .into_iter()
        .find(|c| state.stars()[c.index()].phase() == Phase::MainSequence)
        .expect("the Be star");
    let start = timeline
        .segments()
        .iter()
        .find(|s| matches!(s.kind(), SegmentKind::StableTransfer { donor } if donor != be))
        .map(|s| {
            timeline.state_at(s.start()).stars()[be.index()]
                .mass()
                .value()
        })
        .expect("a stable transfer onto it");
    assert!(
        marks.donor_mass.value() >= start * (1.0 + SPUN_UP_GAINED_SHARE),
        "{start} M_sun at its first transfer, {} now",
        marks.donor_mass.value()
    );
}

/// Under the arm and plan 15's fitted tables, which feed some low-mass hosts under the floor, the
/// X-ray binaries' scan over version 27's table refuses exactly those of the held arm's hosts over
/// the same table in 3,200 cells about the centre, and keeps exactly the cells' entries in 128 of
/// them, none fed under the floor; version 27's table draws the carving's kinds too.
#[test]
fn the_scan_refuses_a_host_fed_under_the_floor_under_the_arm() {
    let rules = held().rules();
    let fitted = GalaxyRules {
        binary_tables: BinaryTableRules::Fitted,
        ..rules
    };
    let [held_fitted, arm_fitted] = [version(), carved().rules().stellar]
        .map(|stellar| held().clone().with_rules(GalaxyRules { stellar, ..fitted }));
    // The same table under both, version 27's, which draws the carving's kinds too.
    let [held_process, arm_process] =
        [&held_fitted, &arm_fitted].map(|g| XrayBinaries::of_table(g, &XRB_SAMPLER_27));
    let under = |h: &XrbScanMarks| h.transfer_rate().is_some_and(|r| r < XRB_MIN_TRANSFER_RATE);
    let (mut refused, mut kept) = (0_usize, 0_usize);
    for x in -20..20 {
        for y in -20..20 {
            for z in [-1, 0] {
                let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, [x, y, z]).expect("a cell");
                let (mut before, mut after) = (Vec::new(), Vec::new());
                scan_cell(&held_fitted, &held_process, key, &mut before);
                scan_cell(&arm_fitted, &arm_process, key, &mut after);
                let expected: Vec<&XrbScanMarks> = before.iter().filter(|h| !under(h)).collect();
                assert_eq!(after.iter().collect::<Vec<_>>(), expected, "{key:?}");
                refused += before.len() - after.len();
                kept += after.len();
                if (-4..4).contains(&x) && (-4..4).contains(&y) {
                    let entries =
                        CatalogueClassCell::generate(&arm_fitted, &arm_process, key).into_entries();
                    let scanned: Vec<_> = entries.iter().map(|e| e.value().scan()).collect();
                    assert_eq!(after.iter().collect::<Vec<_>>(), scanned, "{key:?}");
                }
            }
        }
    }
    println!("{kept} hosts kept under the arm, {refused} fed under the floor refused");
    assert!(
        refused > 0,
        "the fitted tables feed some hosts under the floor"
    );
}

/// Under the arm alone a Be star beside a black hole makes no X-ray binary: the pinned 19.3 M☉ star,
/// spun up by its transfer, is a Be star but feeds no neutron star, and its wind, far inside its
/// Roche lobe, makes the hole no disc.
#[test]
fn a_be_star_beside_a_black_hole_makes_no_x_ray_binary_under_the_arm() {
    assert_eq!(pinned_marks(&carved(), BLACK_HOLE_BESIDE_A_BE_STAR), None);
}

/// Under the arm a pair's classes at its segments' ages do not depend on the order they are asked
/// in: the pinned Be/X-ray binary's, whose context reads its history.
#[test]
fn a_pair_s_classes_under_the_arm_do_not_depend_on_their_order() {
    let member = pinned(&carved(), SPUN_UP_BE_STAR).expect("an X-ray binary under the arm");
    let timeline = member.timeline();
    let ages: Vec<Years> = timeline
        .segments()
        .iter()
        .flat_map(|s| {
            [
                s.start(),
                Years::new(f64::midpoint(s.start().value(), s.end().value())),
            ]
        })
        .collect();
    assert_order_independent(&ages, |&age| timeline.class_at(age));
}

/// A census of `records` records at `site` of the populations `whose` in `galaxy`, each band's in
/// its share of the systems, its [`CENSUS_PARTS`] parts joined in order, on [`CENSUS_THREADS`]
/// threads: its X-ray binaries' marks.
fn census(galaxy: &Galaxy, site: CensusSite, whose: CensusRecords, records: u64) -> Vec<XrayMarks> {
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
            site,
            whose,
            in_band.div_ceil(CENSUS_PARTS),
            CensusPart::new(k, CENSUS_PARTS),
        )
        .into_members()
    })
    .into_iter()
    .flatten()
    .filter(|m| m.class() == CarvedClass::XrayBinary)
    .map(|m| *m.xray().expect("an X-ray binary member's marks"))
    .collect()
}

/// The members of each kind in `marks`, in [`XrbHostKind::ALL`]'s order.
fn kinds(marks: &[XrayMarks]) -> [usize; 9] {
    std::array::from_fn(|k| {
        marks
            .iter()
            .filter(|m| m.member_kind() == Some(XrbHostKind::ALL[k]))
            .count()
    })
}

/// Whether `marks` is a low-mass X-ray binary fed through its inner Lagrangian point by a hydrogen
/// star: a catalogue low-mass kind's.
fn hydrogen_fed_low_mass(marks: &XrayMarks) -> bool {
    marks.host_kind().is_some_and(|kind| !kind.is_high_mass())
}

/// Slow: censuses of 10⁶ slab and 10⁶ young thin disc records under every 27 hold, and under every
/// 27 hold but this one. Under the arm no member is fed through its inner Lagrangian point under
/// the floor and every member has a kind; the members of each kind, the ultracompact ones at 80
/// minutes or longer, the hydrogen-fed low-mass X-ray binaries the floor takes, and the young
/// disc's Be/X share of its neutron-star high-mass hosts and its black holes' share of its
/// high-mass hosts are pinned as measured, against their cited figures.
#[test]
#[ignore = "slow: four censuses of 10⁶ records through the binary engine"]
fn no_census_member_is_fed_under_the_floor_and_every_member_has_a_kind() {
    // Built under each, since the displaced classes' table, from 26, is built under the galaxy's
    // stellar rules.
    let every_27 = GalaxyRules::every_27_hold();
    let [held, arm] = [
        GalaxyRules {
            stellar: StellarRules {
                xrb_class: XrbClassRules::Held,
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
    let sites = [
        ("slab", CensusSite::SOLAR_CIRCLE, CensusRecords::All),
        (
            "young thin disc",
            YOUNG_SITE,
            CensusRecords::Of(Population::YoungThinDisc),
        ),
    ];
    let mut young = Vec::new();
    let mut floored = Vec::new();
    let mut pinned_kinds = Vec::new();
    let mut long_ultracompacts = Vec::new();
    for (name, site, whose) in sites {
        let [before, after] = [&held, &arm].map(|g| census(g, site, whose, CENSUS_RECORDS));
        println!(
            "{name}: {} members held, {} under the arm; kinds (the catalogue's six, then \
             ultracompact, symbiotic, black hole beside a massive star) {:?} held, {:?} under \
             the arm",
            before.len(),
            after.len(),
            kinds(&before),
            kinds(&after)
        );
        for m in &after {
            assert!(m.member_kind().is_some(), "a member of no kind: {m:?}");
            if let Some(rate) = m.transfer_rate {
                assert!(rate >= XRB_MIN_TRANSFER_RATE, "fed under the floor: {m:?}");
            }
        }
        let long: Vec<&XrayMarks> = after
            .iter()
            .filter(|m| {
                m.member_kind() == Some(XrbHostKind::Ultracompact)
                    && m.period.is_none_or(|p| p >= ULTRACOMPACT_MAX_PERIOD)
            })
            .collect();
        println!("{name}: ultracompacts at 80 minutes or longer {long:?}");
        long_ultracompacts.push(long.len());
        pinned_kinds.push(kinds(&after));
        // The hydrogen-fed low-mass X-ray binaries the floor takes, fed under it.
        let under: Vec<&XrayMarks> = before
            .iter()
            .filter(|m| {
                hydrogen_fed_low_mass(m)
                    && m.transfer_rate
                        .is_some_and(|rate| rate < XRB_MIN_TRANSFER_RATE)
            })
            .collect();
        println!("{name}: hydrogen-fed members under the floor {under:?}");
        floored.push(under.len());
        if whose == CensusRecords::Of(Population::YoungThinDisc) {
            young = after;
        }
    }
    let count = |kind: XrbHostKind| {
        young
            .iter()
            .filter(|m| m.member_kind() == Some(kind))
            .count()
    };
    let (bex, supergiant, black_holes) = (
        count(XrbHostKind::BeX),
        count(XrbHostKind::Supergiant),
        count(XrbHostKind::BlackHoleHighMass),
    );
    #[expect(clippy::cast_precision_loss, reason = "a census's few hundred members")]
    let (bex_share, black_hole_share) = (
        bex as f64 / (bex + supergiant).max(1) as f64,
        black_holes as f64 / (bex + supergiant + black_holes).max(1) as f64,
    );
    println!(
        "young thin disc's high-mass hosts under every 27 hold: {bex} Be/X and {supergiant} \
         supergiant neutron stars, {black_holes} black holes; Be/X {bex_share:.3} of the neutron \
         stars' against Liu et al.'s {CITED_BEX_SHARE}, black holes {black_hole_share:.3} \
         against at most {CITED_BLACK_HOLE_SHARE}"
    );
    println!(
        "hydrogen-fed low-mass X-ray binaries under the floor, slab and young disc: {floored:?}"
    );
    assert_eq!(floored, PINNED_FLOORED_HYDROGEN);
    assert_eq!(pinned_kinds, PINNED_KINDS);
    assert_eq!(long_ultracompacts, PINNED_LONG_ULTRACOMPACTS);
    assert_eq!([bex, supergiant, black_holes], PINNED_HIGH_MASS);
}

/// A table of version 27's marks that draws the carving's three kinds alone, so that their marks
/// fill the catalogue's laws in the sample cells.
static NEW_KINDS: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    XRB_SAMPLER_27.marks(),
    &[
        SamplerPick::new("low_mass_kind", &[0.0, 0.0, 0.0, 0.0, 0.5, 0.5]),
        SamplerPick::new("high_mass_kind", &[0.0, 0.0, 0.5]),
    ],
);

/// The census's and the catalogue's kinds over the slab, and the new kinds' marks, that miss under
/// every 27 hold (findings): each a kind's share or a mark's name, as measured.
const PINNED_KIND_MISSES: [&str; 0] = [];

/// A new kind's mark as the catalogue's entries and the census's members give it: its name, its
/// kind, and its value in an entry's scan marks and in a member's marks.
type KindMark = (
    &'static str,
    XrbHostKind,
    fn(&XrbScanMarks) -> f64,
    fn(&XrayMarks) -> Option<f64>,
);

/// The new kinds' marks [`the_catalogue_draws_the_census_s_kinds`] compares: those the census holds
/// enough members of, the black-hole high-mass hosts' two too few.
const NEW_KIND_MARKS: [KindMark; 5] = [
    (
        "ultracompact period, minutes",
        XrbHostKind::Ultracompact,
        |s| s.period().value() / 60.0,
        |m| m.period.map(|p| p.value() / 60.0),
    ),
    (
        "ultracompact rate, log",
        XrbHostKind::Ultracompact,
        |s| {
            s.transfer_rate()
                .map_or(f64::NAN, |r| math::log10(r.value()))
        },
        |m| m.transfer_rate.map(|r| math::log10(r.value())),
    ),
    (
        "ultracompact accretor mass",
        XrbHostKind::Ultracompact,
        |s| s.accretor_mass().value(),
        |m| Some(m.accretor_mass.value()),
    ),
    (
        "symbiotic period, days",
        XrbHostKind::Symbiotic,
        |s| s.period().value() / 86_400.0,
        |m| m.period.map(|p| p.value() / 86_400.0),
    ),
    (
        "symbiotic accretor mass",
        XrbHostKind::Symbiotic,
        |s| s.accretor_mass().value(),
        |m| Some(m.accretor_mass.value()),
    ),
];

/// The chi-square per degree of freedom of `engine`'s values binned in the deciles of `drawn`'s,
/// ten bins of a tenth of the members each expected.
fn decile_chi2(drawn: &mut [f64], engine: &[f64]) -> f64 {
    assert!(
        drawn.len() >= 10 && !engine.is_empty(),
        "{} draws and {} members to compare",
        drawn.len(),
        engine.len()
    );
    drawn.sort_by(f64::total_cmp);
    let edges: Vec<f64> = (1..10).map(|k| drawn[k * drawn.len() / 10]).collect();
    let mut counts = [0_u32; 10];
    for v in engine {
        counts[edges.partition_point(|e| e <= v)] += 1;
    }
    #[expect(clippy::cast_precision_loss, reason = "a census's few hundred members")]
    let expected = engine.len() as f64 / 10.0;
    counts
        .iter()
        .map(|&c| (f64::from(c) - expected) * (f64::from(c) - expected) / expected)
        .sum::<f64>()
        / 9.0
}

/// The catalogue's entries of each kind in `galaxy` whose positions lie in the census's slab
/// (`CensusSite::SOLAR_CIRCLE`'s box), in [`XrbHostKind::ALL`]'s order.
fn slab_entries_by_kind(galaxy: &Galaxy) -> [u32; 9] {
    let process = XrayBinaries::new(galaxy);
    let (centre, half) = ([0.0, 26_000.0, 0.0], [12_000.0, 2_000.0, 1_500.0]);
    let inside = |p: &PointLy| {
        (p.x - centre[0]).abs() <= half[0]
            && (p.y - centre[1]).abs() <= half[1]
            && (p.z - centre[2]).abs() <= half[2]
    };
    let mut counts = [0_u32; 9];
    for x in -24..24 {
        for y in 46..55 {
            for z in -3..3 {
                let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, [x, y, z]).expect("a cell");
                for entry in CatalogueClassCell::generate(galaxy, &process, key).entries() {
                    let scan = entry.value().scan();
                    if inside(&PointLy::from(scan.position())) {
                        let k = XrbHostKind::ALL
                            .iter()
                            .position(|&each| each == scan.kind())
                            .expect("every kind is listed");
                        counts[k] += 1;
                    }
                }
            }
        }
    }
    counts
}

/// The kinds whose shares of `catalogue`'s entries and of `members` differ by more than four
/// standard errors of their difference.
fn kind_share_misses(catalogue: &[u32; 9], members: &[XrayMarks]) -> Vec<String> {
    let census = kinds(members);
    let n_cat = f64::from(catalogue.iter().sum::<u32>());
    assert!(
        n_cat > 0.0 && !members.is_empty(),
        "entries and members to compare"
    );
    let n_cen = f64::from(u32::try_from(members.len()).expect("a census's few hundred members"));
    let mut misses = Vec::new();
    for (k, kind) in XrbHostKind::ALL.iter().enumerate() {
        let c = f64::from(catalogue[k]);
        let e = f64::from(u32::try_from(census[k]).expect("a census's few hundred members"));
        let pooled = (c + e) / (n_cat + n_cen);
        let error = (pooled * (1.0 - pooled) * (1.0 / n_cat + 1.0 / n_cen)).sqrt();
        let difference = c / n_cat - e / n_cen;
        println!("  {kind:?}: {difference:+.4} against {error:.4}");
        if difference.abs() > 4.0 * error + 1e-12 {
            misses.push(format!("{kind:?} share"));
        }
    }
    misses
}

/// The scan marks of the entries `galaxy`'s class draws through [`NEW_KINDS`] about the centre and
/// across the inner disc, `xrb::kinds_tests`' sample cells.
fn new_kind_entries(galaxy: &Galaxy) -> Vec<XrbScanMarks> {
    let process = XrayBinaries::of_table(galaxy, &NEW_KINDS);
    (-4..4)
        .flat_map(|x| (-4..4).flat_map(move |y| [[x, y, 0], [x, y, -1]]))
        .chain((-12..12).flat_map(|x| (10..14).flat_map(move |y| [[x, y, 0], [x, y, -1]])))
        .flat_map(|cell| {
            let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, cell).expect("a cell");
            CatalogueClassCell::generate(galaxy, &process, key)
                .into_entries()
                .into_iter()
                .map(|entry| *entry.value().scan())
        })
        .collect()
}

/// Slow: the catalogue against the census under every 27 hold (P11.T8.b's fix). Over the slab, the
/// catalogue's entries of each of the nine kinds against the census's members of it, by their
/// shares, within four standard errors of their difference; and each new kind's marks, its entries
/// drawn through version 27's marks about the centre and the inner disc, against the census's
/// members of the kind in the slab and the young disc, binned in the deciles of the entries, by
/// chi-square per degree of freedom under 0.97 of the members compared (`tests/binary_xrb.rs`'s
/// scratch tolerance, which refuses only a mark whose members all fall into one tenth of the law).
/// A miss is a finding, pinned in [`PINNED_KIND_MISSES`].
#[test]
#[ignore = "slow: two censuses of 10⁶ records through the binary engine"]
fn the_catalogue_draws_the_census_s_kinds() {
    let arm = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_27_hold(),
    )
    .expect("the Milky Way fixture builds");
    let slab = census(
        &arm,
        CensusSite::SOLAR_CIRCLE,
        CensusRecords::All,
        CENSUS_RECORDS,
    );
    let young = census(
        &arm,
        YOUNG_SITE,
        CensusRecords::Of(Population::YoungThinDisc),
        CENSUS_RECORDS,
    );
    let catalogue = slab_entries_by_kind(&arm);
    println!(
        "the slab's kinds: catalogue {catalogue:?}, census {:?}",
        kinds(&slab)
    );
    let mut misses = kind_share_misses(&catalogue, &slab);
    let entries = new_kind_entries(&arm);
    let members: Vec<&XrayMarks> = slab.iter().chain(&young).collect();
    for (name, kind, of_entry, of_member) in NEW_KIND_MARKS {
        let mut drawn: Vec<f64> = entries
            .iter()
            .filter(|s| s.kind() == kind)
            .map(of_entry)
            .collect();
        let engine: Vec<f64> = members
            .iter()
            .filter(|m| m.member_kind() == Some(kind))
            .filter_map(|m| of_member(m))
            .collect();
        let chi2 = decile_chi2(&mut drawn, &engine);
        #[expect(clippy::cast_precision_loss, reason = "a census's few hundred members")]
        let bound = 0.97 * engine.len() as f64;
        println!(
            "  {name}: {} entries, {} members, chi-square per degree of freedom {chi2:.2} \
             against {bound:.2}",
            drawn.len(),
            engine.len()
        );
        if chi2 >= bound {
            misses.push(name.to_owned());
        }
    }
    assert_eq!(misses, PINNED_KIND_MISSES);
}
