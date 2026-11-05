//! The tests of P11.T4.d's fix: a dynamically transferring pair of main-sequence stars merges with
//! both stars' masses, at one choke point, `Engine::merge_dynamically`. Its hold is deleted (the 27
//! fold); these are its tests of the new arm.
//!
//! - A pinned grid system: its pair of 0.454 and 0.138 M☉ merges dynamically and keeps both stars'
//!   masses (under the held arm it kept the accretor's). The same seed twice gives the same
//!   system.
//! - A pinned Hertzsprung-gap accretor (1.3 M☉, beside a 0.6 M☉ donor) takes the whole donor
//!   (under the held arm the thermal share): a pair built for it and run through the engine's own
//!   dynamical merger, since the grid's pairs seldom come to one (none of 6,000 engine mergers of
//!   the sampler test's streams, 401 of them with a star in the gap, as measured 2026-11-01; a
//!   low-mass donor fills its lobe beside a heavier gap star only once that star overfills its
//!   own).
//! - Slow: over the same 3,000 engine mergers the stellar merger class's sampler test draws
//!   (`testing::brute_force_merger_members_in`), every main-sequence merger keeps the pair's mass,
//!   held to the tracks' 150 M☉, under the version's rules and under every 26 hold; and the
//!   class's sampler against those mergers under every 26 hold.

use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{self, SystemRecord};
use crate::galaxy::rules::GalaxyRules;
use crate::id::{Layer, SystemId};
use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
use crate::rng::Seed;
use crate::stellar::binary::class_merger::secondary_dynamical_merger;
use crate::stellar::binary::testing::{MergerMember, brute_force_merger_members_in};
use crate::stellar::binary::{BinaryInput, BinaryTimeline, CarvedClass};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::sse::{self, Track};
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::stellar::{Composition, Phase};
use crate::time::UniverseTime;
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

/// The Milky Way fixture, under the version's rules.
fn galaxy() -> &'static Galaxy {
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
        GalaxyRules::of_version(crate::GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
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

/// The pinned system's pair's dynamical merger keeps both stars' 0.592 M☉, a main-sequence star at
/// BSE's equation 80's age (under the held arm it kept the accretor's 0.1375 M☉), and the system
/// today is that one star.
#[test]
fn the_pinned_system_s_merger_keeps_both_stars() {
    let id = SystemId::from_raw(PINNED).expect("a grid ID");
    let record = placement::resolve(galaxy(), id).expect("the pinned record resolves");
    assert_eq!(record.layer(), Layer::A);
    let stars = first_attempt(galaxy(), &record);
    let pair = stars.pairs()[0].timeline().clone();
    let t = pair.merger_age().expect("the pinned pair merges");
    let pre = pair.state_at(Years::new(t.value() * (1.0 - 1e-9)));
    let [donor, accretor] = pre.stars().map(|s| s.mass().value());
    assert!(pre.stars().iter().all(|s| s.phase() == Phase::MainSequence));
    let product = pair.state_at(t).stars()[0];
    println!(
        "{donor:.4} + {accretor:.4} M_sun at {:.4e} yr: {:.4} (tau {:.4})",
        t.value(),
        product.mass().value(),
        product.phase_fraction()
    );
    assert_eq!(product.phase(), Phase::MainSequence);
    let total = donor + accretor;
    assert!(
        (product.mass().value() / total - 1.0).abs() < 1e-9,
        "{} against {total}",
        product.mass().value()
    );
    // The system today: one star, the merged one.
    let now = stars
        .state_at(UniverseTime::EPOCH)
        .expect("the system exists")
        .stars()[0]
        .mass()
        .value();
    assert!((now / total - 1.0).abs() < 1e-6);
}

/// The pinned gap-accretor pair: a 0.6 M☉ low-mass main-sequence secondary at its lobe beside a
/// 1.3 M☉ primary halfway through the Hertzsprung gap, solar, at the median draws.
const GAP_PAIR: [f64; 2] = [1.3, 0.6];

/// The pinned gap-accretor merger's product, M☉, as measured (2026-11-01): both stars' 1.89978 M☉
/// (the pair less the 2.2 × 10⁻⁴ M☉ its stars' winds took before; the held arm kept the accretor's
/// 1.29978 M☉ at the merger and 2.6 × 10⁻⁵ M☉ of the donor).
const GAP_PRODUCT: f64 = 1.899_780_424_594_525_6;

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
/// `evolv2.f`, k 2–6, from 26): under the version's rules and under every 26 hold it takes the whole
/// donor, the pair less the product's own wind (none at the merger), and stays in the gap at the
/// accretor's place there, on its own mass's track (under the held arm it kept only the thermal
/// share of the donor).
#[test]
fn the_gap_accretor_takes_the_whole_donor() {
    let total = GAP_PAIR[0] + GAP_PAIR[1];
    for rules in [
        version(),
        StellarRules::of_version(crate::GENERATOR_VERSION),
    ] {
        let (input, age) = gap_pair(rules);
        let until = Years::new(age.value() + 1.0e3);
        let product = secondary_dynamical_merger(&input, age, until);
        println!(
            "{:?} {:.17} M_sun at {:.6} of the gap",
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
        let m = product.mass().value();
        assert!(
            m >= (1.0 - MERGER_MASS_TOLERANCE) * total,
            "{m} against {total}"
        );
        assert!(m <= total, "{m} against {total}");
        assert!((m - GAP_PRODUCT).abs() < 1e-12, "{m}");
    }
}

/// The same seed twice gives the same system.
#[test]
fn the_pinned_system_is_deterministic() {
    let id = SystemId::from_raw(PINNED).expect("a grid ID");
    let record = placement::resolve(galaxy(), id).expect("the pinned record resolves");
    assert_eq!(
        first_attempt(galaxy(), &record),
        first_attempt(galaxy(), &record)
    );
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

/// Whether `timeline`'s pair coalesces through a common envelope straight out of contact:
/// P11.T4.n's contact switch, whose primary leaves its main sequence at its pin's own end with the
/// pair in contact, so that the pair coalesces as a giant's common envelope does and its product
/// keeps the envelope left unbound (Hurley, Tout and Pols 2002, equation 77), not by the arm's
/// rule. Its segments run contact, a common envelope of no length, merged.
fn contact_switch(timeline: &BinaryTimeline) -> bool {
    use crate::stellar::binary::SegmentKind;
    timeline.segments().windows(3).any(|w| {
        w[0].kind() == SegmentKind::Contact
            && w[1].kind() == SegmentKind::CommonEnvelope
            && w[2].kind() == SegmentKind::Merged
    })
}

/// What [`least_kept`] found over the mergers it read.
#[derive(Debug, Clone, Copy)]
struct Kept {
    /// The least share the merged star keeps of the mass it can keep: the pair's, or the tracks'
    /// 150 M☉ (`sse::MAX_INITIAL_MASS`) where the pair passes it.
    least: f64,
    /// The main-sequence mergers read.
    n: usize,
    /// The contact switches left out ([`contact_switch`]).
    switched: usize,
    /// The mergers among `n` whose pair passes the tracks' 150 M☉.
    capped: usize,
}

/// The least the merged star keeps of the mass it can keep, over the main-sequence mergers of the
/// same grid records `drawn` run again in `galaxy` ([`Kept`]): each record's first attempt, every
/// pair that merges with two main-sequence stars just before. The mass it can keep is the pair's,
/// held to the tracks' 150 M☉ (`sse::MAX_INITIAL_MASS`), where `Engine::main_sequence_star` places
/// a heavier product, the excess lost at the merger (lean, 2026-11-05).
fn least_kept(galaxy: &Galaxy, drawn: &[MergerMember]) -> Kept {
    let mut least = f64::INFINITY;
    let (mut n, mut switched, mut capped) = (0, 0, 0);
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
            if contact_switch(timeline) {
                switched += 1;
                continue;
            }
            let total: f64 = pre.stars().iter().map(|s| s.mass().value()).sum();
            let keepable = total.min(sse::MAX_INITIAL_MASS.value());
            capped += usize::from(total > keepable);
            let product = timeline.state_at(t).stars()[0].mass().value();
            least = least.min(product / keepable);
            n += 1;
        }
    }
    Kept {
        least,
        n,
        switched,
        capped,
    }
}

/// The most of the pair's mass a main-sequence merger may lose: 2 × 10⁻³ of it, the
/// rule's own loss, which is none but the product's own main-sequence wind from its zero-age mass
/// to the age the rule places it at (`Engine::main_sequence_star`; the merger's other routes, a
/// contact that coalesces at the end of the main sequence or a collision, place it alike). A pair
/// over the tracks' 150 M☉ (`sse::MAX_INITIAL_MASS`) is held to 150 M☉ first: no track exists
/// above it, so its product is placed at 150 M☉ and the excess is lost at the merger (lean,
/// 2026-11-05), and the share is of the 150 M☉. As measured at generator version 28 (2026-11-05)
/// the least kept is 0.99897 of a 13.3 + 13.2 M☉ pair's 26.5 M☉ (record `0x821f_b120_0000_0002`,
/// [`LEAST_KEPT_UNDER_THE_ARM`]), and the one pair over 150 M☉, 129.48 + 109.43 M☉ (record
/// `0x81f7_b660_0000_0001`), keeps the whole 150 M☉. P11.T4.n's contact switches are left out
/// ([`contact_switch`]), which coalesce through a common envelope, not by the arm's rule: at
/// generator version 25 four of the records' pairs, three of them keeping under 0.998 of the pair
/// (the least 0.99558) and one all of it (research's review of round 7's bound, 2026-11-01).
const MERGER_MASS_TOLERANCE: f64 = 2e-3;

/// The least share of the pair's mass, held to the tracks' 150 M☉, that a main-sequence merger
/// keeps over the 3,000 records, under the version's rules and under every 26 hold, as measured
/// at generator version 28 (2026-11-05; 0.99893 before the bound, measured while the arm was
/// held, 2026-11-01), the contact switches left out.
const LEAST_KEPT_UNDER_THE_ARM: [f64; 2] = [0.998_971_164_310_089_8, 0.998_971_164_310_089_8];

/// Over the same 3,000 grid records whose engine mergers P11.T8.c's sampler test draws, every
/// main-sequence merger keeps the pair's mass, held to the tracks' 150 M☉
/// (`sse::MAX_INITIAL_MASS`, where the engine places a heavier product), less the rule's loss
/// (none but the product's own wind, [`MERGER_MASS_TOLERANCE`]), under the version's rules and
/// under every 26 hold, each record run again, P11.T4.n's contact switches left out
/// ([`contact_switch`]) (under the held arm 719 of 803 kept under 99%). It prints how many pairs
/// pass 150 M☉.
#[test]
#[ignore = "slow: thousands of grid pairs run through the binary engine, twice"]
fn every_main_sequence_merger_keeps_the_pair_s_mass() {
    let drawn = members(galaxy(), 3_000);
    let every_26 = every_26();
    let mut measured = [0.0; 2];
    for (k, (name, galaxy)) in [
        ("the version's rules", galaxy()),
        ("every 26 hold", &every_26),
    ]
    .iter()
    .enumerate()
    {
        let Kept {
            least,
            n,
            switched,
            capped,
        } = least_kept(galaxy, &drawn);
        println!(
            "{name}: {n} main-sequence mergers, the least keeps {least:e} of the pair held to \
             150 M_sun; {capped} pairs past 150 M_sun; {switched} contact switches left out"
        );
        assert!(n > 700, "{name}: {n}");
        assert!(capped > 0, "{name}: no pair past 150 M_sun to hold");
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
