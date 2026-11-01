//! The tests of P11.T4.d's fix, [`MERGER_MASS_VERSION`]: a dynamically transferring pair of
//! main-sequence stars merges with both stars' masses only under the rules that turn it on, read
//! at one choke point, `Engine::merge_dynamically`, through the engine's input's rules. Each fast
//! test turns this hold's arm on alone ([`pair_rules`]), so that the other 26 holds move nothing
//! it holds equal.
//!
//! - The version's rules keep the held arm, and the tripwire holds it only until the bump.
//! - Under the arm a pinned grid system moves: its pair of 0.454 and 0.138 M☉ merged dynamically
//!   and kept the accretor's mass, and keeps both; and a cell's systems whose pairs merge in no
//!   such way move under neither arm. The same seed twice under the arm gives the same system.
//! - Under the arm a pinned Hertzsprung-gap accretor (1.3 M☉, beside a 0.6 M☉ donor) takes the
//!   whole donor, where the held arm keeps the thermal share: a pair built for it and run through
//!   the engine's own dynamical merger, since the grid's pairs seldom come to one (none of 6,000
//!   engine mergers of the sampler test's streams, 401 of them with a star in the gap, as
//!   measured 2026-11-01; a low-mass donor fills its lobe beside a heavier gap star only once
//!   that star overfills its own).
//! - Slow: over the same 3,000 engine mergers the stellar merger class's sampler test draws
//!   (`testing::brute_force_merger_members_in`), every main-sequence merger keeps the pair's mass
//!   under the arm, and under every 26 hold; and the class's sampler against those mergers under
//!   every 26 hold.
//!
//! The movers under the arm, found by turning it on at the version (and back off), over the sim's,
//! the server's and the fitting crate's tests and the binaries' slow censuses: three goldens,
//! `stellar/binary_timelines` (`a_thousand_timelines_are_pinned`), `sky/census_near_sun` (one
//! layer-B white dwarf too cool for the photometry tables, the product of a pair that kept its
//! mass) and `sky/band_face_row` (two more stars listed within 100 ly). No census of the binaries
//! moves out of its bounds (`binary_system`, `binary_carve`, `binary_classes`, the pair-light and
//! reach tables' bounds), and no golden of the catalogue classes moves. The gap accretor's arm,
//! turned on alone at the version, moves no test and no slow census (2026-11-01).

use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{self, CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{Layer, SystemId};
use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
use crate::rng::Seed;
use crate::stellar::binary::class_merger::secondary_dynamical_merger;
use crate::stellar::binary::testing::{MergerMember, brute_force_merger_members_in};
use crate::stellar::binary::{BinaryInput, CarvedClass};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::sse::Track;
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::stellar::{Composition, Phase};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{GravitationalParameter, Metres, Radians, SolarMasses, Years};

/// The seed of the Milky Way fixture the tests share, P11.T8.c's slow sampler test's.
const SEED: u64 = 0x0b17_08c0_0000_0002;

/// The pinned grid system: a layer-A record of 0.454 M☉ whose first attempt's pair with a
/// 0.138 M☉ companion merges dynamically at 395 Myr (found among the first engine mergers of
/// stream 0 of the sampler test's members).
const PINNED: u64 = 0x0208_daff_2000_0000;

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's rules with this hold's arm on alone.
fn pair_rules() -> StellarRules {
    StellarRules {
        merger_mass: MergerMassRules::Pair,
        ..version()
    }
}

/// The Milky Way fixture, under the version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    })
}

/// The fixture built under every 26 hold: built so, since the field share and the carve-out are
/// read when a galaxy is built.
fn every_26() -> Galaxy {
    Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_26_hold(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The fixture under the version's rules with this hold's arm on alone.
fn with_pairs() -> Galaxy {
    let galaxy_rules = GalaxyRules::of_version(GENERATOR_VERSION);
    held().clone().with_rules(GalaxyRules {
        stellar: pair_rules(),
        ..galaxy_rules
    })
}

/// The first attempt's stars of `record` in `galaxy`, as the engine's members run them.
fn first_attempt(galaxy: &Galaxy, record: &SystemRecord) -> SystemStars {
    let primary = StarModel::new_under(
        record.primary_initial_mass(),
        draw_metallicity(galaxy, record),
        primary_draws(galaxy, record),
        record.age_at_epoch(),
        galaxy.rules().stellar,
    )
    .expect("a grid primary is a star with a finite age");
    SystemStars::at_attempt(
        galaxy,
        record,
        &primary,
        MultiplicityContext::Free,
        RedrawAttempt::FIRST,
    )
}

/// The version's rules keep the held arm, every version below 26 does and 26 takes the pair's
/// masses; the fitted tables hold only under the version's arm. The tripwire: the bump to 26
/// deletes this hold.
#[test]
fn the_merger_mass_is_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < MERGER_MASS_VERSION,
        "the bump to 26 deletes P11.T4.d's merger-mass hold: `MERGER_MASS_VERSION`, \
         `MergerMassRules` with its `ThermalLimit` arm, `StellarRules::merger_mass`, \
         `merge_dynamically`'s match on it, the brief's read of \
         `MergerMassRules::fitted_tables_hold`, this module and its line in `stellar/rules.rs`, \
         and refits the pair-light and reach tables"
    );
    assert_eq!(version().merger_mass, MergerMassRules::ThermalLimit);
    let at = |v: u32| StellarRules::of_version(crate::GeneratorVersion::new(v)).merger_mass;
    for v in [GENERATOR_VERSION.get(), 25] {
        assert_eq!(at(v), MergerMassRules::ThermalLimit, "version {v}");
    }
    assert_eq!(at(26), MergerMassRules::Pair);
    assert_eq!(
        StellarRules::every_26_hold().merger_mass,
        MergerMassRules::Pair
    );
    assert_eq!(
        GalaxyRules::every_26_hold().stellar.merger_mass,
        MergerMassRules::Pair
    );
    assert!(version().merger_mass.fitted_tables_hold());
    assert!(!MergerMassRules::Pair.fitted_tables_hold());
}

/// Under the arm alone the pinned system moves, and only from its merger: under the version's
/// rules its pair's dynamical merger keeps the accretor's 0.1375 M☉, under the arm both stars'
/// 0.592 M☉, a main-sequence star at BSE's equation 80's age; the merger's age and the pair
/// before it are the same under both arms, and so is the merged star's place, the primary's.
#[test]
fn the_pinned_system_s_merger_keeps_both_stars_under_the_arm() {
    let id = SystemId::from_raw(PINNED).expect("a grid ID");
    let record = placement::resolve(held(), id).expect("the pinned record resolves");
    assert_eq!(record.layer(), Layer::A);
    let arm = with_pairs();
    let [before, after] = [held(), &arm].map(|galaxy| {
        let stars = first_attempt(galaxy, &record);
        let pair = stars.pairs()[0].timeline().clone();
        (stars, pair)
    });
    let (held_pair, arm_pair) = (&before.1, &after.1);
    let t = held_pair.merger_age().expect("the pinned pair merges");
    assert_eq!(arm_pair.merger_age(), Some(t));
    let earlier = Years::new(0.5 * t.value());
    assert_eq!(held_pair.state_at(earlier), arm_pair.state_at(earlier));
    let pre = held_pair.state_at(Years::new(t.value() * (1.0 - 1e-9)));
    let [donor, accretor] = pre.stars().map(|s| s.mass().value());
    assert!(pre.stars().iter().all(|s| s.phase() == Phase::MainSequence));
    let held_product = held_pair.state_at(t).stars()[0];
    let arm_product = arm_pair.state_at(t).stars()[0];
    println!(
        "{donor:.4} + {accretor:.4} M_sun at {:.4e} yr: held {:.4}, under the arm {:.4} (tau {:.4})",
        t.value(),
        held_product.mass().value(),
        arm_product.mass().value(),
        arm_product.phase_fraction()
    );
    assert!(held_product.mass().value() < 0.95 * (donor + accretor));
    assert!((held_product.mass().value() / accretor - 1.0).abs() < 0.01);
    assert_eq!(arm_product.phase(), Phase::MainSequence);
    let total = donor + accretor;
    assert!(
        (arm_product.mass().value() / total - 1.0).abs() < 1e-9,
        "{} against {total}",
        arm_product.mass().value()
    );
    // The system today: one star, the merged one.
    let now = |stars: &SystemStars| {
        stars
            .state_at(UniverseTime::EPOCH)
            .expect("the system exists")
            .stars()[0]
            .mass()
            .value()
    };
    assert!((now(&after.0) / total - 1.0).abs() < 1e-6);
    assert!(now(&before.0) < 0.95 * total);
}

/// The pinned gap-accretor pair: a 0.6 M☉ low-mass main-sequence secondary at its lobe beside a
/// 1.3 M☉ primary halfway through the Hertzsprung gap, solar, at the median draws.
const GAP_PAIR: [f64; 2] = [1.3, 0.6];

/// The pinned gap-accretor merger's product under the held arm and under the arm, M☉, as measured
/// (2026-11-01): the accretor's 1.29978 M☉ at the merger and 2.6 × 10⁻⁵ M☉ of the donor, against
/// both stars' 1.89978 M☉ (the pair less the 2.2 × 10⁻⁴ M☉ its stars' winds took before).
const GAP_PRODUCT: [f64; 2] = [1.299_806_206_392_41, 1.899_780_424_594_525_6];

/// The gap-accretor pair of [`GAP_PAIR`] under `rules`, the secondary filling its lobe on a
/// circular orbit; and its age, years.
fn gap_pair(rules: StellarRules) -> (BinaryInput, Years) {
    let [m1, m2] = GAP_PAIR;
    let comp = Composition::SOLAR;
    let draws = [StarDraws::median(), StarDraws::median()];
    let full = Track::to_age(SolarMasses::new(m1), &comp, &draws[0], Years::new(2.0e10));
    let age = full
        .age_in_phase(Phase::HertzsprungGap, 0.5)
        .expect("the primary's track passes the gap");
    let donor = Track::to_age(SolarMasses::new(m2), &comp, &draws[1], Years::new(age))
        .state_at(Years::new(age));
    let lobe = roche_lobe_radius(m2 / m1, Metres::new(1.0)).value();
    let a = Metres::new(donor.radius().value() * SOLAR_RADIUS_M / lobe);
    let orbit = KeplerElements::from_semi_major_axis(
        a,
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).expect("zero angles"),
        Radians::ZERO,
    )
    .expect("a bound orbit");
    let input = BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        comp,
        orbit,
        draws,
        Years::new(age),
    )
    .expect("a valid pair")
    .with_rules(rules);
    (input, Years::new(age))
}

/// A Hertzsprung-gap accretor's dynamical merger with a low-mass main-sequence donor (BSE's
/// `evolv2.f`, k 2–6, from 26): under the held arm the gap star keeps only the thermal share of
/// the donor, the old figure; under the arm alone and under every 26 hold it takes the whole donor,
/// the pair less the product's own wind (none at the merger). Under each it stays in the gap at
/// the accretor's place there, on its own mass's track.
#[test]
fn the_gap_accretor_takes_the_whole_donor_under_the_arm() {
    let mut products = Vec::new();
    for rules in [version(), pair_rules(), StellarRules::every_26_hold()] {
        let (input, age) = gap_pair(rules);
        let until = Years::new(age.value() + 1.0e3);
        let product = secondary_dynamical_merger(&input, age, until);
        println!(
            "{:?}: {:?} {:.17} M_sun at {:.6} of the gap",
            rules.merger_mass,
            product.phase(),
            product.mass().value(),
            product.phase_fraction()
        );
        assert_eq!(product.phase(), Phase::HertzsprungGap);
        assert!(
            (product.phase_fraction() - 0.5).abs() < 1e-3,
            "{rules:?}: {}",
            product.phase_fraction()
        );
        products.push(product.mass().value());
    }
    let total = GAP_PAIR[0] + GAP_PAIR[1];
    let held = products[0];
    assert!(held < 0.95 * total, "{held}");
    assert!((held / GAP_PAIR[0] - 1.0).abs() < 1e-3, "{held}");
    for arm in &products[1..] {
        assert!(
            *arm >= (1.0 - MERGER_MASS_TOLERANCE) * total,
            "{arm} against {total}"
        );
        assert!(*arm <= total, "{arm} against {total}");
    }
    for (m, pinned) in products
        .iter()
        .zip([GAP_PRODUCT[0], GAP_PRODUCT[1], GAP_PRODUCT[1]])
    {
        assert!((m - pinned).abs() < 1e-12, "{products:?}");
    }
}

/// The same seed twice under the arm gives the same system.
#[test]
fn the_arm_is_deterministic() {
    let id = SystemId::from_raw(PINNED).expect("a grid ID");
    let record = placement::resolve(held(), id).expect("the pinned record resolves");
    let [a, b] = [with_pairs(), with_pairs()].map(|galaxy| first_attempt(&galaxy, &record));
    assert_eq!(a, b);
}

/// A cell's systems whose pairs merge with no main-sequence pair before the merger, or merge
/// not at all, are in the same states across the clock window under both arms.
#[test]
fn systems_without_a_main_sequence_merger_move_under_neither_arm() {
    let arm = with_pairs();
    let mut records = Vec::new();
    let mut cell = Vec::new();
    for x in 0..1_024 {
        let size = i32::try_from(Layer::B.cell_size_ly()).expect("a small cell size");
        let key = CellKey::new(Layer::B, [x, 26_000 / size, 0]).expect("a cell of the grid");
        generate_cell(held(), key, &mut cell);
        records.extend_from_slice(&cell);
        if records.len() >= 150 {
            break;
        }
    }
    let mut compared = 0;
    for record in records.iter().take(150) {
        let stars = first_attempt(held(), record);
        let main_sequence_merger = stars.pairs().iter().any(|pair| {
            let timeline = pair.timeline();
            timeline.merger_age().is_some_and(|t| {
                let pre = timeline.state_at(Years::new(t.value() * (1.0 - 1e-9)));
                pre.stars().iter().all(|s| s.phase() == Phase::MainSequence)
            })
        });
        if main_sequence_merger {
            continue;
        }
        // The systems carry their rules; their states, which the record and every reader see,
        // are the same.
        let other = first_attempt(&arm, record);
        for t in [ClockWindow::START, UniverseTime::EPOCH, ClockWindow::END] {
            assert_eq!(
                stars.state_at(t),
                other.state_at(t),
                "{:?} at {t}",
                record.id()
            );
        }
        compared += 1;
    }
    assert!(compared > 100, "{compared} of {}", records.len());
}

/// `n` engine mergers of the stellar merger class in `galaxy`, drawn on 16 streams at once and
/// joined in stream order, as `tests/common/binaries.rs` draws them.
fn members(galaxy: &Galaxy, n: usize) -> Vec<MergerMember> {
    let share = n.div_ceil(16);
    let mut out = Vec::with_capacity(n + 16);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16_u64)
            .map(|s| {
                scope.spawn(move || {
                    brute_force_merger_members_in(galaxy, CarvedClass::StellarMerger, share, s)
                })
            })
            .collect();
        for handle in handles {
            out.extend(handle.join().expect("a member draw finishes"));
        }
    });
    out.truncate(n);
    out
}

/// The pair's mass just before `member`'s merger (its state at T less a part in 10⁹, after any
/// transfer in the segment that ends at T) and the merged star's, or `None` if the pair then was
/// not two main-sequence stars.
fn main_sequence_merger(member: &MergerMember) -> Option<(f64, f64)> {
    let t = member.merger_age().value();
    let before = member.timeline().state_at(Years::new(t * (1.0 - 1e-9)));
    before
        .stars()
        .iter()
        .all(|s| s.phase() == Phase::MainSequence)
        .then(|| {
            let total = before.stars().iter().map(|s| s.mass().value()).sum();
            (total, member.product().mass().value())
        })
}

/// The share of main-sequence mergers in `members` whose merged star keeps under 99% of the pair,
/// and their count.
fn short_share(members: &[MergerMember]) -> (usize, usize) {
    let mergers: Vec<_> = members.iter().filter_map(main_sequence_merger).collect();
    let short = mergers
        .iter()
        .filter(|(total, product)| *product < 0.99 * total)
        .count();
    (short, mergers.len())
}

/// The least the merged star keeps of the pair's mass, over the main-sequence mergers of the same
/// grid records `drawn` run again in `galaxy`, and how many there were: each record's first
/// attempt, every pair that merges with two main-sequence stars just before.
fn least_kept(galaxy: &Galaxy, drawn: &[MergerMember]) -> (f64, usize) {
    let mut least = f64::INFINITY;
    let mut n = 0;
    for member in drawn {
        for pair in first_attempt(galaxy, member.record()).pairs() {
            let timeline = pair.timeline();
            let Some(t) = timeline.merger_age() else {
                continue;
            };
            let pre = timeline.state_at(Years::new(t.value() * (1.0 - 1e-9)));
            if !pre.stars().iter().all(|s| s.phase() == Phase::MainSequence) {
                continue;
            }
            let total: f64 = pre.stars().iter().map(|s| s.mass().value()).sum();
            let product = timeline.state_at(t).stars()[0].mass().value();
            least = least.min(product / total);
            n += 1;
        }
    }
    (least, n)
}

/// The most of the pair's mass a main-sequence merger may lose under the arm: 2 × 10⁻³ of it, the
/// rule's own loss, which is none but the product's own main-sequence wind from its zero-age mass
/// to the age the rule places it at (`Engine::main_sequence_star`; the merger's other routes, a
/// contact that coalesces at the end of the main sequence or a collision, place it alike). As
/// measured (2026-11-01) the least kept is 0.99893 of a 12.9 + 12.7 M☉ contact's 25.5 M☉
/// ([`LEAST_KEPT_UNDER_THE_ARM`]).
const MERGER_MASS_TOLERANCE: f64 = 2e-3;

/// The least share of the pair's mass a main-sequence merger keeps over the 3,000 records, under
/// the arm alone and under every 26 hold, as measured.
const LEAST_KEPT_UNDER_THE_ARM: [f64; 2] = [0.998_929_750_656_249_7, 0.998_929_750_656_249_7];

/// Over the same 3,000 grid records whose engine mergers P11.T8.c's sampler test draws, every
/// main-sequence merger keeps the pair's mass less the rule's loss (none but the product's own
/// wind, [`MERGER_MASS_TOLERANCE`]), under the arm alone and under every 26 hold, each record run
/// again; against 719 of 803 under 99% under the version's rules.
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine, three times"]
fn every_main_sequence_merger_keeps_the_pair_s_mass_under_the_arm() {
    let drawn = members(held(), 3_000);
    let (short, all) = short_share(&drawn);
    println!("version's rules: {short} of {all} main-sequence mergers under 99% of the pair");
    assert!(short > all / 2, "{short} of {all}");
    let every_26 = every_26();
    let mut measured = [0.0; 2];
    for (k, (name, galaxy)) in [("the arm alone", with_pairs()), ("every 26 hold", every_26)]
        .iter()
        .enumerate()
    {
        let (least, n) = least_kept(galaxy, &drawn);
        println!("{name}: {n} main-sequence mergers, the least keeps {least:e} of the pair");
        assert!(n > 700, "{name}: {n}");
        assert!(least >= 1.0 - MERGER_MASS_TOLERANCE, "{name}: {least}");
        measured[k] = least;
    }
    for (m, pinned) in measured.iter().zip(LEAST_KEPT_UNDER_THE_ARM) {
        assert!((m - pinned).abs() < 1e-12, "{measured:?}");
    }
}

/// The stellar merger class's sampler against the engine's mergers under every 26 hold: the pair's
/// total mass, mass ratio and primary's mass of 3,000 weighted engine mergers in the deciles of
/// 1,000 entries about the solar circle, by chi-square per degree of freedom under the scratch
/// tolerance of P11.T8.c's test (290), and the share in contact before the merger.
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine"]
fn the_merger_sampler_against_the_engine_under_every_26_hold() {
    use crate::galaxy::catalogue_classes::binary::StellarMergers;
    use crate::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
    use crate::math;
    use crate::stellar::binary::SegmentKind;
    let galaxy = every_26();
    let process = StellarMergers::new(&galaxy);
    let mut catalogue: Vec<[f64; 3]> = Vec::new();
    'cells: for x in [0, -1, 1, -2, 2, -3, 3] {
        for y in 5..=7 {
            for z in [0, -1] {
                if catalogue.len() >= 1_000 {
                    break 'cells;
                }
                let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, y, z]).unwrap();
                let cell = CatalogueClassCell::generate(&galaxy, &process, key);
                catalogue.extend(cell.entries().iter().map(|e| {
                    let scan = e.value().scan();
                    let (m1, m2) = (scan.primary_mass().value(), scan.secondary_mass().value());
                    [math::log10(m1 + m2), m2 / m1, math::log10(m1)]
                }));
            }
        }
    }
    catalogue.truncate(1_000);
    let engine: Vec<([f64; 3], f64, bool)> = members(&galaxy, 3_000)
        .iter()
        .map(|m| {
            let before = m.before();
            let [a, b] = before.stars().map(|s| s.mass().value());
            let marks = [
                math::log10(a + b),
                a.min(b) / a.max(b),
                math::log10(m.record().primary_initial_mass().value()),
            ];
            (marks, m.weight(), before.kind() == SegmentKind::Contact)
        })
        .collect();
    let total: f64 = engine.iter().map(|(_, w, _)| w).sum();
    let squares: f64 = engine.iter().map(|(_, w, _)| w * w).sum();
    let effective = total * total / squares;
    for (k, name) in ["log total mass", "mass ratio", "log primary mass"]
        .iter()
        .enumerate()
    {
        let mut sorted: Vec<f64> = catalogue.iter().map(|m| m[k]).collect();
        sorted.sort_by(f64::total_cmp);
        let edges: Vec<f64> = (1..10).map(|d| sorted[d * sorted.len() / 10]).collect();
        let mut weights = [0.0_f64; 10];
        for (marks, w, _) in &engine {
            weights[edges.partition_point(|&e| e <= marks[k])] += w;
        }
        let each = effective / 10.0;
        let chi2: f64 = weights
            .iter()
            .map(|w| {
                let o = w / total * effective;
                (o - each) * (o - each) / each
            })
            .sum();
        println!(
            "  {name}: chi2/dof {:.1} (effective {effective:.0} of 3000)",
            chi2 / 9.0
        );
        assert!(chi2 / 9.0 < 290.0, "{name}: {}", chi2 / 9.0);
    }
    let contact: f64 = engine
        .iter()
        .filter(|(_, _, c)| *c)
        .map(|(_, w, _)| w)
        .sum();
    println!(
        "  in contact before T: engine {:.3} weighted",
        contact / total
    );
}
