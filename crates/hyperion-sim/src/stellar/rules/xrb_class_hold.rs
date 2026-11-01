//! The tests of P11.T5's and P11.T8.b's fix, [`XRB_CLASS_VERSION`]: what `classify` counts as an
//! X-ray binary, a floor on the rate of Roche-lobe overflow onto a compact star, Be donors spun up
//! by stable transfer and a disc for a wind-fed black hole, only under the rules that turn it on,
//! read through [`XrbClassRules::carves`] and each timeline's rules.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Under the arm alone three pinned grid systems move, one a rule, found by a census of 3 × 10⁵
//!   slab and 3 × 10⁵ young thin disc records: a neutron star fed 2.7 × 10⁻¹⁴ M☉ yr⁻¹ by a
//!   helium white dwarf is no X-ray binary; a 24.5 M☉ star's wind feeding a black hole 30 days out,
//!   far inside its Roche lobe, makes none; and a 12.3 M☉ main-sequence star that gained mass from
//!   its neutron star's progenitor is a Be star, its pair a Be/X-ray binary.
//! - The catalogue: under the arm and plan 15's fitted tables the scan refuses the candidates its
//!   sampler feeds under the floor, which the class's test will refuse at the bump, and keeps
//!   exactly the cells' entries; and a pair's classes at its ages do not depend on the order they
//!   are asked in.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's tests and the slow suite: the golden `sky/census_near_sun`
//! (the grid's carve near the Sun) and, slow, `tables_hold::the_x_ray_binary_sampler_against_the_engine`'s
//! pinned misses (its Be/X members); and this module's tests of the version's arm (2026-11-01).
//! Nothing moves at 24–26.

use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::{BinaryTableRules, XrayBinaries};
use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement;
use crate::galaxy::rules::GalaxyRules;
use crate::id::SystemId;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::scan::{XrbScanMarks, scan_cell};
use crate::stellar::binary::testing::{
    CensusPart, CensusRecords, CensusSite, ClassMember, XrayMarks, class_census_in,
};
use crate::stellar::binary::xrb_class::{SPUN_UP_GAINED_SHARE, XRB_MIN_TRANSFER_RATE};
use crate::stellar::binary::{BinaryClass, CarvedClass, Component, HmxbKind, SegmentKind, XrbKind};
use crate::units::Years;

/// The seed of the Milky Way fixture: `hyperion-fit`'s manifests', whose census found the pinned
/// systems.
const SEED: u64 = 0x0b17_15a0_0000_0001;

/// A layer-D record of the slab whose first attempt is a neutron star fed 2.7 × 10⁻¹⁴ M☉ yr⁻¹ by
/// a 0.0043 M☉ helium white dwarf on a 2.8-hour orbit.
const UNDER_THE_FLOOR: u64 = 0x621d_daa7_f800_0000;

/// A layer-E record of the young thin disc whose first attempt is a black hole fed by the wind of
/// a 24.5 M☉ main-sequence star 30.3 days out.
const DISCLESS_BLACK_HOLE: u64 = 0x816d_5b1f_f000_0012;

/// A layer-E record of the slab whose first attempt is a neutron star 70.5 days from a 12.3 M☉
/// main-sequence star that gained mass in stable transfer.
const SPUN_UP_BE_STAR: u64 = 0x81af_b360_0000_0006;

/// The bands a census runs, whose primaries can make a member.
const CENSUS_BANDS: [MassBand; 3] = [MassBand::C, MassBand::D, MassBand::E];

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
         `XrbClassRules` with its `Held` arm, `StellarRules::xrb_class`, `classify`'s three \
         calls of `carves` and the context's spun-up stars, this module and its line in \
         `stellar/rules.rs`"
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
/// binary: under the held arm it is a low-mass transient fed 2.7 × 10⁻¹⁴ M☉ yr⁻¹.
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
/// X-ray binaries' scan refuses exactly those of the held arm's hosts over 3,200 cells about the
/// centre, and keeps exactly the cells' entries in 128 of them, none fed under the floor.
#[test]
fn the_scan_refuses_a_host_fed_under_the_floor_under_the_arm() {
    let rules = held().rules();
    let fitted = GalaxyRules {
        binary_tables: BinaryTableRules::Fitted,
        ..rules
    };
    let [held_fitted, arm_fitted] = [version(), carved().rules().stellar]
        .map(|stellar| held().clone().with_rules(GalaxyRules { stellar, ..fitted }));
    let [held_process, arm_process] = [&held_fitted, &arm_fitted].map(XrayBinaries::new);
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
