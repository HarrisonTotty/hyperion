//! The tests of the stellar stage's rules (decision-galaxy-25-train §1.5–§1.6, carried to the
//! stellar stage): every reader of the kick law takes its table from its galaxy's rules, through
//! [`KickRankTable::of`].
//!
//! - **D3.** The version's rules are the default law's and a default galaxy's, value for value.
//! - **D2.** Under the test-only second table ([`StellarRules::probe`]), every reader of the
//!   ordinary mode's rank shows a mover. These are a single remnant's kick, the engine's pinned
//!   death and its companions' kicks, the class table's kick shares, and the feature and centre
//!   members' conditional draws.
//! - **D4.** The same seed twice under the second table gives the same output.
//! - **D1** is the goldens as they stand, which pin every reader: `stellar/kicks` (three pinned
//!   kicks), `stellar/binary_timelines` (the engine's supernovae) and `galaxy_class_table`. No
//!   version takes the second table, so nothing on the record can move by it.
//! - **D5 is not needed:** choosing a table adds no arithmetic, and the committed arm is
//!   [`StandardKickLaw::default`]'s law, value for value (D3).
//! - **C1** is [`census_of_the_second_table`], slow.
//!
//! Readers that cannot move under a second rank table:
//!
//! - the engine's accretion-induced collapse, which takes the low mode alone;
//! - the range rows' brief, whose exact primary is built under the galaxy's rules but which reads
//!   no kick.
//!
//! Readers that take no rules:
//!
//! - the centre's classes (`tabulated_progenitors`) and the features' retention
//!   (`retention_from_rows`) read only the law's parameters (the white dwarfs' σ), never its rank
//!   table: their ordinary mode is the fitted `cluster_retention`;
//! - `hyperion-fit`'s tasks (`node_shares`, `kick_defaults`, the reference population) do read the
//!   committed table, but they build no galaxy, so they take the version's rules by design.
//!
//! The fitted tables downstream of the committed kick table ([`STALE_BY_DESIGN`]) stay on it under
//! a second table until a wiring refits them.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::displaced::class_table::{ClassKey, ClassTable, FormTable};
use crate::galaxy::features::interior::ClassKind;
use crate::galaxy::features::members::remnant_fits;
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell, resolve};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{Layer, SystemId};
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::rng::Seed;
use crate::stellar::Composition;
use crate::stellar::binary::{BinaryInput, Component, SupernovaRecord};
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::{KickMode, KickRankTable, NatalKick, StandardKickLaw};
use crate::stellar::system::{StarModel, SystemStars};
use crate::time::UniverseTime;
use crate::units::{
    GravitationalParameter, KilometresPerSecond, Radians, Seconds, SolarMasses, Years,
};

/// The universe of the pinned kicks: `tests/stellar_kick.rs`' Milky Way fixture, whose
/// `stellar/kicks` golden pins the first three systems below.
const SYSTEMS_SEED: u64 = 0x0600_0029_b000_5eed;

/// A primary whose neutron star takes the ordinary mode (`stellar/kicks`).
const ORDINARY_NEUTRON_STAR: u64 = 0x8200_b2e0_0000_0000;
/// A primary whose neutron star takes the low mode (`stellar/kicks`).
const LOW_MODE_NEUTRON_STAR: u64 = 0x8200_b2e0_0000_000c;
/// A primary whose black hole takes the ordinary mode (`stellar/kicks`).
const KICKED_BLACK_HOLE: u64 = 0x8200_b2e0_0000_0005;
/// Two pairs of black holes, each bound after its supernovae, which the engine runs: the first
/// pair's primary falls back completely, with no kick, and its secondary's ordinary kick goes
/// through the engine's `companion_kick`; the second pair's primary takes an ordinary kick at its
/// pinned death. It is the third record of layer E that `tests/stellar_kick.rs`' search reads.
const PAIR: u64 = 0x8200_b2e0_0000_0002;

/// The fitted tables built from the committed kick table, which a second table leaves stale until a
/// wiring refits them: the retention's ordinary shares and the defaults' review from the law, and
/// the pair light and the sky's binary light from the engine's supernovae.
const STALE_BY_DESIGN: [&str; 4] = [
    "cluster_retention",
    "kick_defaults",
    "binary_pair_light",
    "sky_binary_light",
];

/// Every pinned system.
const PINNED: [u64; 4] = [
    ORDINARY_NEUTRON_STAR,
    LOW_MODE_NEUTRON_STAR,
    KICKED_BLACK_HOLE,
    PAIR,
];

/// The fixture under the generator version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// The version's galaxy rules with the second kick-rank table.
fn probe_rules() -> GalaxyRules {
    GalaxyRules {
        stellar: StellarRules::probe(),
        ..GalaxyRules::of_version(GENERATOR_VERSION)
    }
}

/// [`held`]'s galaxy built under the second kick-rank table: from 27 its displaced classes' table
/// reads the stellar rules, so `Galaxy::with_rules` keeps them and the galaxy is built under them.
fn probed() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SYSTEMS_SEED),
            GalaxyParams::milky_way_like(),
            probe_rules(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// The pinned system `id`'s stars in `galaxy`.
fn stars(galaxy: &Galaxy, id: u64) -> SystemStars {
    let id = SystemId::from_raw(id).expect("a pinned ID is well formed");
    let record = resolve(galaxy, id).expect("a pinned ID names a system");
    SystemStars::generate(galaxy, &record)
}

/// Every supernova of a system's pairs, in pair order.
fn supernovae(stars: &SystemStars) -> Vec<SupernovaRecord> {
    stars
        .pairs()
        .iter()
        .flat_map(|pair| pair.timeline().supernovae().iter().copied())
        .collect()
}

/// A kick's speed, km/s.
fn km_s(kick: &NatalKick) -> f64 {
    KilometresPerSecond::from(kick.speed()).value()
}

/// No version takes the second table: the rules of every version, 25's among them, read the
/// committed one. The first stellar hold adds its field's tripwire beside this.
#[test]
fn no_version_takes_the_second_table() {
    for v in 1..=64 {
        assert_eq!(
            StellarRules::of_version(GeneratorVersion::new(v)).kick_rank,
            KickRankRules::Committed,
            "version {v}"
        );
    }
    assert_eq!(
        StellarRules::of_version(GENERATOR_VERSION).kick_rank,
        KickRankRules::Committed
    );
    assert_ne!(
        StellarRules::probe(),
        StellarRules::of_version(GENERATOR_VERSION)
    );
}

/// D3: a default galaxy carries the version's stellar rules, under which the law is the default
/// law and the table the committed one, and the pinned systems' stars are those of a galaxy built
/// under them explicitly. Below the galaxy, a star and a pair built without one take them too.
#[test]
fn the_version_s_rules_are_the_default_law() {
    let rules = StellarRules::of_version(GENERATOR_VERSION);
    assert_eq!(held().rules().stellar, rules);
    assert_eq!(StandardKickLaw::under(rules), StandardKickLaw::default());
    assert_eq!(KickRankTable::of(rules), KickRankTable::generator());
    assert_ne!(
        KickRankTable::of(StellarRules::probe()),
        KickRankTable::generator()
    );
    let explicit = Galaxy::from_params_with_rules(
        Seed::new(SYSTEMS_SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::of_version(GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture builds");
    for id in PINNED {
        assert_eq!(stars(held(), id), stars(&explicit, id), "{id:#018x}");
    }
    let (m, composition, age) = (
        SolarMasses::new(15.0),
        Composition::SOLAR,
        Years::new(5.0e7),
    );
    assert_eq!(
        StarModel::new(m, composition, StarDraws::median(), age).expect("in range"),
        StarModel::new_under(m, composition, StarDraws::median(), age, rules).expect("in range")
    );
    assert_eq!(pair(15.0, 12.0, 300.0).rules(), rules);
}

/// D2, a single remnant: the primaries whose kicks take the ordinary mode move to the second
/// table's slower kick, with the same death, remnant, mode and direction; the low mode's reads no
/// rank table and stays.
#[test]
fn a_single_remnant_s_kick_takes_its_galaxy_s_table() {
    for id in [ORDINARY_NEUTRON_STAR, KICKED_BLACK_HOLE] {
        let (held, probed) = (stars(held(), id), stars(probed(), id));
        let (a, b) = (
            held.natal_kick().expect("a pinned primary has a kick"),
            probed.natal_kick().expect("a pinned primary has a kick"),
        );
        assert_eq!(
            (a.mode(), b.mode()),
            (KickMode::Ordinary, KickMode::Ordinary)
        );
        assert_eq!(a.direction(), b.direction(), "{id:#018x}");
        assert!(
            km_s(&b) < km_s(&a),
            "{id:#018x}: {} against {}",
            km_s(&b),
            km_s(&a)
        );
        assert_eq!(held.primary().death(), probed.primary().death());
        assert_eq!(held.primary().remnant(), probed.primary().remnant());
    }
    let (held, probed) = (
        stars(held(), LOW_MODE_NEUTRON_STAR),
        stars(probed(), LOW_MODE_NEUTRON_STAR),
    );
    assert_eq!(
        held.natal_kick().map(|k| k.mode()),
        Some(KickMode::Low),
        "the low-mode pin"
    );
    assert_eq!(held.natal_kick(), probed.natal_kick());
}

/// Every supernova of `a` and `b` paired in order, with the same star, remnant and mode: an
/// ordinary kick is slower under the second table and changes the stars' velocities after it,
/// and any other is the same. The count of ordinary kicks.
fn slower_ordinary_kicks(a: &[SupernovaRecord], b: &[SupernovaRecord]) -> usize {
    assert_eq!(a.len(), b.len());
    let mut ordinary = 0;
    for (x, y) in a.iter().zip(b) {
        assert_eq!(
            (x.component(), x.remnant().kind()),
            (y.component(), y.remnant().kind())
        );
        let (kx, ky) = (x.kick().expect("a kick"), y.kick().expect("a kick"));
        assert_eq!(kx.mode(), ky.mode());
        if kx.mode() == KickMode::Ordinary {
            ordinary += 1;
            assert!(
                km_s(&ky) < km_s(&kx),
                "{:?}: {} against {}",
                x.component(),
                km_s(&ky),
                km_s(&kx)
            );
            assert_ne!(x.velocities(), y.velocities(), "{:?}", x.component());
        } else {
            assert_eq!(kx, ky, "{:?}: no rank table", x.component());
        }
    }
    ordinary
}

/// D2, the engine in a galaxy: the pinned quadruple's ordinary kicks, one through
/// `companion_kick` and one at a pinned death, take the second table's slower kicks; its complete
/// fallback has none under either. A pair's pinned death stays its primary's own model's kick
/// under either table (design note 16), so the engine and the model read one table.
#[test]
fn the_engine_s_kicks_take_its_galaxy_s_table() {
    let (held, probed) = (stars(held(), PAIR), stars(probed(), PAIR));
    assert_eq!(
        slower_ordinary_kicks(&supernovae(&held), &supernovae(&probed)),
        2
    );
    for stars in [&held, &probed] {
        let first = stars.pairs().first().expect("the quadruple's inner pair");
        let pinned = first
            .timeline()
            .supernovae()
            .iter()
            .find(|r| r.component() == Component::Primary)
            .and_then(SupernovaRecord::kick);
        assert_eq!(pinned, stars.primary().natal_kick());
    }
}

/// A circular pair of `m1` and `m2` M☉ on a period of `days`, 50 Myr old, under the version's
/// rules.
fn pair(m1: f64, m2: f64, days: f64) -> BinaryInput {
    let orbit = KeplerElements::from_period(
        Seconds::new(days * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
            .expect("a valid orientation"),
        Radians::new(0.0),
    )
    .expect("a valid orbit");
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::SOLAR,
        orbit,
        [StarDraws::median(), StarDraws::median()],
        Years::new(5.0e7),
    )
    .expect("two stars in range with a finite age")
}

/// D2, the engine without a galaxy: `BinaryInput::with_rules` alone moves a pair's kicks, so the
/// engine reads its rules from its input. A 15 and 12 M☉ pair on 300 days explodes twice in the
/// ordinary mode, the primary at its pinned death and the secondary through `companion_kick`.
/// Which pairs a slower kick keeps bound, the census lists.
#[test]
fn a_pair_s_kicks_take_its_input_s_rules() {
    let until = Years::new(5.0e7);
    let runs = |input: &BinaryInput| {
        (
            crate::stellar::binary::evolve(input, until),
            crate::stellar::binary::evolve(&input.clone().with_rules(StellarRules::probe()), until),
        )
    };
    let (a, b) = runs(&pair(15.0, 12.0, 300.0));
    assert_eq!(slower_ordinary_kicks(a.supernovae(), b.supernovae()), 2);
}

/// D2, the class table: its kick shares, and so its classes' weights, take the galaxy's table.
#[test]
fn the_class_table_s_kick_shares_take_its_galaxy_s_table() {
    let forms = FormTable::committed();
    let (a, b) = (
        ClassTable::build(held(), &forms),
        ClassTable::build(probed(), &forms),
    );
    let moved = a
        .classes()
        .iter()
        .filter(|class| {
            let id = class.key().id();
            [MassBand::D, MassBand::E].into_iter().any(|band| {
                class
                    .weight(band)
                    .total_cmp(&b.class_weight(band, id))
                    .is_ne()
            })
        })
        .count();
    assert!(moved > 0, "no class moved");
    assert_ne!(a, b);
}

/// D2, the members' conditional draws: a feature's or the centre's neutron star is kept only if
/// its kick falls below the escape speed, so a slower table keeps more and refuses none the
/// committed one keeps.
#[test]
fn the_members_draws_judge_kicks_by_their_galaxy_s_table() {
    let system = SystemId::from_raw(ORDINARY_NEUTRON_STAR).expect("a pinned ID is well formed");
    let (composition, age) = (Composition::SOLAR, Years::new(5.0e7));
    let mut flipped = 0;
    for k in 0..20 {
        let mass = 9.0 + 0.5 * f64::from(k);
        for attempt in 0..5 {
            let fits = |rules| {
                remnant_fits(
                    ClassKind::NeutronStar,
                    mass,
                    &composition,
                    (Seed::new(SYSTEMS_SEED), system, attempt),
                    age,
                    (300.0, 1.0),
                    rules,
                )
            };
            let (a, b) = (
                fits(StellarRules::of_version(GENERATOR_VERSION)),
                fits(StellarRules::probe()),
            );
            assert!(!a || b, "{mass} M☉, attempt {attempt}");
            flipped += u32::from(a != b);
        }
    }
    assert!(flipped > 0, "no draw moved");
}

/// D4: a galaxy built under the second table, and its pinned systems, are the same twice, and the
/// same as [`probed`]'s.
#[test]
fn the_second_table_gives_the_same_output_twice() {
    let build = || {
        Galaxy::from_params_with_rules(
            Seed::new(SYSTEMS_SEED),
            GalaxyParams::milky_way_like(),
            probe_rules(),
        )
        .expect("the Milky Way fixture builds")
    };
    let (first, second) = (build(), build());
    assert_eq!(first, second);
    assert_eq!(&first, probed());
    for id in PINNED {
        let once = stars(&first, id);
        assert_eq!(once, stars(&second, id), "{id:#018x}");
        assert_eq!(once, stars(probed(), id), "{id:#018x}");
    }
}

/// The census's figures for one galaxy's sample under either table.
#[derive(Default)]
struct Census {
    systems: usize,
    moved: usize,
    largest: (f64, u64),
    changes: Vec<String>,
}

impl Census {
    /// Compares `id`'s stars under the committed table, `a`, with the second's, `b`.
    fn compare(&mut self, id: u64, a: &SystemStars, b: &SystemStars) {
        self.systems += 1;
        let kicks = |s: &SystemStars| {
            let mut kicks: Vec<NatalKick> = s.natal_kick().into_iter().collect();
            kicks.extend(supernovae(s).iter().filter_map(SupernovaRecord::kick));
            kicks
        };
        let (ka, kb) = (kicks(a), kicks(b));
        for k in ka.iter().chain(&kb) {
            assert!(km_s(k).is_finite(), "{id:#018x}: {k:?}");
        }
        if ka != kb {
            self.moved += 1;
        }
        for (x, y) in ka.iter().zip(&kb) {
            let change = (km_s(x) - km_s(y)).abs();
            if change > self.largest.0 {
                self.largest = (change, id);
            }
        }
        if ka.len() != kb.len() {
            self.changes.push(format!(
                "{id:#018x}: {} kicks against {}",
                ka.len(),
                kb.len()
            ));
        }
        let (sa, sb) = (
            a.summary_at(UniverseTime::EPOCH),
            b.summary_at(UniverseTime::EPOCH),
        );
        if sa.existence() != sb.existence() {
            self.changes.push(format!("{id:#018x}: existence"));
        }
        for (i, (x, y)) in sa.stars().iter().zip(sb.stars()).enumerate() {
            if x.kind() != y.kind() || x.classification() != y.classification() {
                self.changes.push(format!(
                    "{id:#018x} star {i}: {:?} {} against {:?} {}",
                    x.kind(),
                    x.classification(),
                    y.kind(),
                    y.classification()
                ));
            }
        }
        let bound = |s: &SystemStars| {
            supernovae(s)
                .iter()
                .map(SupernovaRecord::bound)
                .collect::<Vec<_>>()
        };
        if bound(a) != bound(b) {
            self.changes.push(format!(
                "{id:#018x}: bound after each supernova {:?} against {:?}",
                bound(a),
                bound(b)
            ));
        }
    }
}

/// The class weights per band, kind (the thin disc, or an old source) and speed bin, summed over
/// the thin disc's ages.
fn weights_by_bin(table: &ClassTable) -> BTreeMap<(&'static str, String, usize), f64> {
    let mut bins = BTreeMap::new();
    for class in table.classes() {
        let (kind, speed) = match class.key() {
            ClassKey::Thin { speed, .. } => ("thin disc".to_owned(), speed.index()),
            ClassKey::Old { source, speed } => (format!("{source:?}"), speed.index()),
            ClassKey::Hypervelocity => continue,
        };
        for (name, band) in [("D", MassBand::D), ("E", MassBand::E)] {
            *bins.entry((name, kind.clone(), speed)).or_insert(0.0) += class.weight(band);
        }
    }
    bins
}

/// C1, the census, field on against off: 300 systems of layer E near the solar circle in each of
/// three drawn galaxies, and their class tables. It reports the systems whose kicks move, the
/// largest change of a kick speed, every change of existence, class or a pair's survival by ID,
/// the class weights per band, kind and speed bin, and the tables stale by design. Every kick is
/// finite. Nothing on the record moves
/// by the second table; the census shows the carriers reach every reader, and is the harness the
/// first stellar hold's census reruns on its own field.
#[test]
#[ignore = "slow: 900 systems of layer E and six class tables under either kick-rank table"]
fn census_of_the_second_table() {
    let forms = FormTable::committed();
    for seed in [SYSTEMS_SEED, 0x0600_0029_b000_0001, 0x0600_0029_b000_0002] {
        let held = Galaxy::new(Seed::new(seed));
        let probed = Galaxy::from_params_with_rules(
            Seed::new(seed),
            GalaxyParams::from_seed(
                Seed::new(seed),
                crate::galaxy::imf::MassFunctionKind::default(),
            ),
            probe_rules(),
        )
        .expect("the census galaxy builds");
        let size = i32::try_from(Layer::E.cell_size_ly()).expect("a small cell size");
        let mut census = Census::default();
        let mut cell: Vec<SystemRecord> = Vec::new();
        'cells: for step in 0..10_000 {
            let key = CellKey::new(Layer::E, [step, 26_000 / size, 0]).expect("a cell of the grid");
            generate_cell(&held, key, &mut cell);
            for record in cell.drain(..) {
                let a = SystemStars::generate(&held, &record);
                let b = SystemStars::generate(&probed, &record);
                census.compare(record.id().raw(), &a, &b);
                if census.systems == 300 {
                    break 'cells;
                }
            }
        }
        eprintln!(
            "seed {seed:#018x}: {} systems, {} with a kick moved; largest change {:.3} km/s \
             ({:#018x})",
            census.systems, census.moved, census.largest.0, census.largest.1
        );
        for change in &census.changes {
            eprintln!("  {change}");
        }
        let (wa, wb) = (
            weights_by_bin(&ClassTable::build(&held, &forms)),
            weights_by_bin(&ClassTable::build(&probed, &forms)),
        );
        for ((band, kind, bin), a) in &wa {
            let b = wb[&(*band, kind.clone(), *bin)];
            if a.total_cmp(&b).is_ne() {
                eprintln!(
                    "  class weight, band {band}, {kind}, bin {bin}: {a:.6e} against {b:.6e}"
                );
            }
        }
        assert!(census.moved > 0, "seed {seed:#018x}: no kick moved");
    }
    eprintln!("stale by design under a second table: {STALE_BY_DESIGN:?}");
}
