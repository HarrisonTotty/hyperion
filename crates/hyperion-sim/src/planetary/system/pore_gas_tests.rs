//! P14.T49.l's census over the record: the regolith's pore gas and the outer system's ice, under
//! every 26 hold against every 26 hold with the pore gas held, over P14.T53.c's thin-air census
//! (the 400 sample systems, the golden systems and LOOK72's systems), with P14.T49.m's icy
//! regolith at the frost and P14.T24.h.b's holds set back in both walks, as `line_switch_tests`'s
//! census has them.

use std::sync::OnceLock;
use std::time::Instant;

use super::cap_balance_tests::{id_of, same};
use super::tests::{
    GOLDEN_SYSTEMS_SEED, ThinAirBody, golden_systems, thin_air_census, thin_air_sets,
};
use super::*;
use crate::planetary::derive::photometry::area_mix::{
    IcyRegolithRules, with_icy_regolith, with_scattering_held,
};
use crate::substance::thermal::{RegolithInertiaRules, with_regolith_inertia};

/// The records whose state the pore gas flips over the census, by ID in the census's order: one
/// from generator version 28, the sample's moon 41ffecb1ffc00007.0d01, `Tenuous` at 0.49 Pa with
/// the pore gas held and a `Snowball` at 101 Pa in force, its coldest zone 34.6 → 45.8 K and its
/// N₂ at its ice's saturation in both. P14.T9.d's placement margin moves its planet, 0d00, about
/// the pair 3 of 41ffecb1ffc00007, from 28; an N₂ ice world at 46 K, it lies at the jump that
/// P14.T51.l's secant root also finds there (`SECANT_OTHER_CROSSINGS`). To 27 none (the census at
/// the tip, 2026-11-01). With the outer system's ice on every water-ice lid, nine N₂ airs at or
/// under the 100 Pa line on icy worlds at 35–52 K went `Airless`, their lids' Γ taken from about
/// 56 to 3–5 J m⁻² K⁻¹ s^−½ and their coldest zones 5–23 K colder under their N₂'s cold trap; a
/// lid under an air (`hooks::surface::lid_air`) now keeps P14.T49.j's form.
const FLIPS: [&str; 1] = ["41ffecb1ffc00007.0d01"];

/// How many of the census's bodies the pore gas moves: 1,722 of 6,232 at generator version 28,
/// 1,622 of them airless and 1 tenuous, over 28's census of 71 bodies fewer than 27's (its
/// evaporation and placement margin); 1,721 of 6,303 at 27, 1,621 airless and 1 tenuous, over
/// 27's census of 344 bodies fewer (its galaxy's sample and golden systems; 1,723 of 6,647 at 26,
/// 2026-11-01, 1,622 airless and 1 tenuous; 1,729 of 6,664 at 25, 1,730 at 24).
const MOVED: usize = 1_722;

/// The census's two walks, one body each in the same order: the pore gas held, and in force.
struct Census {
    held: Vec<ThinAirBody>,
    aired: Vec<ThinAirBody>,
}

/// The census under every 26 hold, the pore gas held and in force, walked once a process.
///
/// Both walks set P14.T49.m's icy regolith back to the frost and P14.T24.h.b's holds back
/// (`with_scattering_held`): with the icy lag the pore gas moves 1,882 bodies (1,772 airless), and
/// with the scattering's four 1,875, against the [`MOVED`] it moves alone. 26's bump restates
/// this census.
fn census() -> &'static Census {
    static CENSUS: OnceLock<Census> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let sets = thin_air_sets();
        let frost = IcyRegolithRules::Frost;
        let start = Instant::now();
        let held = {
            with_icy_regolith(frost, || {
                with_scattering_held(|| {
                    with_regolith_inertia(RegolithInertiaRules::Vacuum, || thin_air_census(&sets))
                })
            })
        };
        let walk_held = start.elapsed();
        let start = Instant::now();
        let aired =
            { with_icy_regolith(frost, || with_scattering_held(|| thin_air_census(&sets))) };
        eprintln!(
            "the walks: the pore gas held {walk_held:?}, in force {:?}, {} bodies",
            start.elapsed(),
            aired.len()
        );
        assert_eq!(held.len(), aired.len(), "one walk's bodies, both rules");
        Census { held, aired }
    })
}

/// Under every 26 hold, P14.T49.l moves the census's records as listed ([`MOVED`], [`FLIPS`]):
/// every mover's figures finite, and its largest moves and the airless movers' coldest zones
/// printed.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 115 s on a pod"]
fn under_every_26_hold_the_pore_gas_moves_the_census_as_listed() {
    let census = census();
    let mut movers = Vec::new();
    let mut flips = Vec::new();
    for (held, aired) in census.held.iter().zip(&census.aired) {
        assert_eq!(held.label, aired.label, "one body each, in order");
        assert!(
            aired.t_s.is_finite() && aired.coldest.is_finite() && aired.p.is_finite(),
            "{}: {}",
            aired.label,
            aired.line()
        );
        if same(held, aired) {
            continue;
        }
        movers.push((held, aired));
        if held.state != aired.state {
            flips.push(id_of(aired));
            eprintln!(
                "FLIP {}\n  held  {}\n  aired {}",
                aired.label,
                held.line(),
                aired.line()
            );
        }
    }
    let largest = |figure: fn(&ThinAirBody) -> f64| {
        movers
            .iter()
            .map(|(h, a)| {
                (
                    (figure(a) - figure(h)).abs(),
                    id_of(a),
                    figure(h),
                    figure(a),
                )
            })
            .max_by(|x, y| x.0.total_cmp(&y.0))
    };
    eprintln!(
        "P14.T49.l's census: {} of {} bodies move, {} change state; the largest moves: mean T_s \
         {:?}, coldest zone {:?}, pressure {:?}",
        movers.len(),
        census.aired.len(),
        flips.len(),
        largest(|b| b.t_s),
        largest(|b| b.coldest),
        largest(|b| b.p),
    );
    let mut by_state = std::collections::BTreeMap::new();
    for (_, aired) in &movers {
        *by_state
            .entry(format!("{:?}", aired.state))
            .or_insert(0_u32) += 1;
    }
    let mut by_set = std::collections::BTreeMap::new();
    for (_, aired) in &movers {
        let set = aired.label.split(' ').next().unwrap_or_default().to_owned();
        *by_set.entry(set).or_insert(0_u32) += 1;
    }
    eprintln!("movers by state: {by_state:?}; by set: {by_set:?}");
    let airless: Vec<f64> = movers
        .iter()
        .filter(|(h, a)| {
            h.state == crate::planetary::derive::atmosphere::SurfaceState::Airless
                && a.state == h.state
        })
        .map(|(h, a)| a.coldest - h.coldest)
        .collect();
    let (colder, warmer) = airless
        .iter()
        .fold((0.0_f64, 0.0_f64), |(lo, hi), &d| (lo.min(d), hi.max(d)));
    eprintln!(
        "airless movers: {} coldest zones, from {colder:.2} K to {warmer:+.2} K",
        airless.len()
    );
    assert_eq!(flips, FLIPS);
    assert_eq!(movers.len(), MOVED);
}

/// Under every 26 hold with the pore gas in force, the golden systems generated twice give the
/// same records, bit for bit.
#[test]
fn under_every_26_hold_the_pore_gas_repeats() {
    let sets: Vec<(&str, Seed, SystemContext)> = golden_systems()
        .iter()
        .map(|(name, ctx, _)| (*name, Seed::new(GOLDEN_SYSTEMS_SEED), ctx.clone()))
        .collect();
    let first = thin_air_census(&sets);
    let second = thin_air_census(&sets);
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(&second) {
        assert!(same(a, b), "{}: {} against {}", a.label, a.line(), b.line());
    }
    assert!(!first.is_empty());
}
