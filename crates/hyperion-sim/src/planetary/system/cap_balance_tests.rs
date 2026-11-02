//! P14.T53.h's census over the record: the CO₂ cap's energy balance, under every 26 hold against
//! every 26 hold with the cap held, over P14.T53.c's thin-air census (the 400 sample systems, the
//! golden systems and LOOK72's systems).

use std::sync::OnceLock;
use std::time::Instant;

use super::tests::{
    GOLDEN_SYSTEMS_SEED, ThinAirBody, golden_systems, thin_air_census, thin_air_sets,
};
use super::*;
use crate::planetary::derive::atmosphere::{Gas, SurfaceState, settled_log};
use crate::planetary::hooks::surface::{CapBalanceRules, rotates, with_cap_balance};

/// The records whose state or figures the cap moves over the census (the census at the tip,
/// 2026-10-31, the cap's scope read on CO₂'s inventory and a capped CO₂ held at its cap), by ID in
/// the census's order: CO₂ airs that collapse or thin, each a fast rotator whose cap is frost.
/// Against the gate alone, holding a capped CO₂ at its cap (d = 1) adds nine, among them LOOK72's
/// 020e56ef20080000.0700, `Temperate` at 273.15 K under 43.7 bar → `Snowball` at 170 K, a class
/// change; with no sea of its own (c = 0) it adds 41ffecb5ffc00001.0c00, `Temperate` at 273.15 K
/// under 42.2 bar at 0.198 S⊕ → `Snowball` at 163.6 K, the other. The cap `partition_over` reads, which the passes' area mix reads too
/// ([`partition_over`](crate::planetary::hooks::figures::partition_over)), takes
/// 42002cb200000009.0700 to a 0.36 bar `Snowball` at 122 K.
const MOVERS: [&str; 24] = [
    "01ffeb2c20000000.0600",
    "01fffb2be0000000.0500",
    "21ffd6587ff80000.0200",
    "21ffd6587ff80000.0300",
    "41ffecb200000001.0300",
    "41ffecb200000000.e301",
    "41ffecb600000007.0300",
    "41ffecb5ffc00000.0300",
    "02000b2b60020000.0400",
    "2200365a7ff80000.0300",
    "2200365a7ff80000.0400",
    "41ffecb5ffc00001.0c00",
    "42002cb1ff800003.0a00",
    "41ffecb1ffc00000.0a00",
    "42006cba00000009.0400",
    "42002cb200000009.0700",
    "42002cb200000009.0800",
    "42002cb200000009.e401",
    "42002cb200000009.e501",
    "42006c9a00000004.0504",
    "8200b2e00000000a.0200",
    "8200b2e00000000a.0300",
    "020e56ef20080000.0700",
    "820e6ee00000008d.0900",
];

/// The records the cap moves in their last bits alone, within 10⁻⁶ ([`within_a_millionth`]), by
/// ID in the census's order: a step or a thermostat trial on the way read a cap, and the steps
/// settled a few ulps away.
const NUDGED: [&str; 1] = ["820e6ee00000008d.e302"];

/// The records whose airborne CO₂ stands over its cap's saturation pressure by more than 1%, each
/// pinned at under twice it ([`OVER_THE_CAP_AT_MOST`]): 41ffecb600000007.0300, 1.6% over, whose
/// greenhouse ends at its steps' limit, one more partition moving its CO₂ by 1.4% to its cap's
/// saturation. Its partition at its record's span holds the cap's saturation to the bit.
const OVER_THE_CAP: [&str; 1] = ["41ffecb600000007.0300"];

/// The most an [`OVER_THE_CAP`] record's airborne CO₂ may stand over its cap's saturation, as a
/// ratio: 2.
const OVER_THE_CAP_AT_MOST: f64 = 2.0;

/// The census under every 26 hold, the cap held and in force, walked once a process, with the
/// solves the walk under the cap published ([`settled_log`]).
fn census() -> &'static Census {
    static CENSUS: OnceLock<Census> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let sets = thin_air_sets();
        let start = Instant::now();
        let held = { with_cap_balance(CapBalanceRules::Held, || thin_air_census(&sets)) };
        let walk_held = start.elapsed();
        let start = Instant::now();
        let (balanced, solves) = settled_log::capture(|| thin_air_census(&sets));
        eprintln!(
            "the walks: the cap held {walk_held:?}, balanced {:?}, {} bodies, {} published solves",
            start.elapsed(),
            balanced.len(),
            solves.len()
        );
        assert_eq!(held.len(), balanced.len(), "one walk's bodies, both rules");
        Census {
            held,
            balanced,
            solves,
        }
    })
}

/// The census's two walks, one body each in the same order, and the solves the walk under the cap
/// published.
struct Census {
    held: Vec<ThinAirBody>,
    balanced: Vec<ThinAirBody>,
    solves: Vec<settled_log::Settled>,
}

/// Whether `a` and `b`, one body under each rule, publish the same state and figures, bit for bit.
pub(super) fn same(a: &ThinAirBody, b: &ThinAirBody) -> bool {
    let bits = |x: f64, y: f64| x.total_cmp(&y).is_eq();
    a.state == b.state
        && bits(a.p, b.p)
        && bits(a.air_p, b.air_p)
        && bits(a.t_s, b.t_s)
        && bits(a.coldest, b.coldest)
        && a.gases.len() == b.gases.len()
        && a.gases
            .iter()
            .zip(&b.gases)
            .all(|(x, y)| x.0 == y.0 && bits(x.1, y.1))
}

/// Whether `a` and `b` publish the same state and gases in the same order, their pressures within
/// 10⁻⁶ of each other and their temperatures within 10⁻⁶ K.
fn within_a_millionth(a: &ThinAirBody, b: &ThinAirBody) -> bool {
    let relative = |x: f64, y: f64| (x - y).abs() <= 1e-6 * x.abs().max(y.abs());
    a.state == b.state
        && relative(a.p, b.p)
        && relative(a.air_p, b.air_p)
        && (a.t_s - b.t_s).abs() <= 1e-6
        && (a.coldest - b.coldest).abs() <= 1e-6
        && a.gases.iter().map(|g| g.0).eq(b.gases.iter().map(|g| g.0))
}

/// The ID of a census body, the last word of its label.
pub(super) fn id_of(body: &ThinAirBody) -> String {
    body.label.rsplit(' ').next().unwrap_or_default().to_owned()
}

/// The solve of `solves` that published the census body `body`: the last whose temperature, total
/// and CO₂ are its derivation's, bit for bit.
pub(super) fn published<'a>(
    solves: &'a [settled_log::Settled],
    body: &ThinAirBody,
) -> &'a settled_log::Settled {
    let bits = |x: f64, y: f64| x.total_cmp(&y).is_eq();
    solves
        .iter()
        .rev()
        .find(|s| {
            bits(s.t, body.air_t) && bits(s.total, body.air_p) && bits(s.carbon_dioxide, body.co2)
        })
        .unwrap_or_else(|| panic!("{}: its published solve is logged", body.label))
}

/// A temperature `t`, K, as a census line prints it: "none" where there is none.
fn kelvin(t: Option<f64>) -> String {
    t.map_or_else(|| "none".to_owned(), |t| format!("{t:.2} K"))
}

/// P14.T53.h over the census, both arms under every 26 hold: the records the cap moves are
/// [`MOVERS`], and those it moves in their last bits [`NUDGED`], each printed both ways with its
/// a ÷ `L_Ro`, its `T_c` and its light; none of them is a `MagmaOcean`, `Tenuous` or `Airless`
/// record under either arm, and none is a world locked 1:1. Its `T_c` is the balance's at its
/// published state's column, the one a next step would read, the gates aside
/// ([`settled_log::Greenhouse::figures`]), with the cap that column's span carries and its
/// record's.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 115 s on a pod"]
fn under_every_26_hold_the_cap_moves_the_census_s_thick_carbon_dioxide_airs() {
    let Census {
        held,
        balanced,
        solves,
    } = census();
    let (mut movers, mut nudged) = (Vec::new(), Vec::new());
    for (was, now) in held.iter().zip(balanced) {
        assert_eq!(was.label, now.label, "one walk's order");
        if same(was, now) {
            continue;
        }
        let near = within_a_millionth(was, now);
        let solve = published(solves, now);
        let (step_rossby, t_c) = solve.greenhouse.figures.map_or((f64::NAN, None), |f| {
            (f.rossby, f.temperature.map(crate::units::Kelvin::value))
        });
        eprintln!(
            "{} {}\n  held:     {}\n  balanced: {}\n  p {:+.3e}, T_s {:+.3e} K; a ÷ L_Ro {:.3} \
             held, {:.3} balanced, {step_rossby:.3} at its published column; T_c {} there, its \
             cap {} there and {} on its record; S ÷ S⊕ {:.4}",
            if near { "nudged" } else { "moved" },
            now.label,
            was.line(),
            now.line(),
            now.p / was.p - 1.0,
            now.t_s - was.t_s,
            was.rossby,
            now.rossby,
            kelvin(t_c),
            kelvin(solve.greenhouse.cap),
            kelvin(now.cap),
            now.flux_earths,
        );
        for body in [was, now] {
            assert!(
                !matches!(
                    body.state,
                    SurfaceState::MagmaOcean | SurfaceState::Tenuous | SurfaceState::Airless
                ),
                "{}: {:?}",
                body.label,
                body.state
            );
            assert!(rotates(&body.spin), "{} is locked", body.label);
        }
        if near {
            nudged.push(id_of(now));
        } else {
            movers.push(id_of(now));
        }
    }
    assert_eq!(movers, MOVERS);
    assert_eq!(nudged, NUDGED);
}

/// P14.T53.h's settled check over the census, under every 26 hold: for each record the cap moves
/// ([`MOVERS`] and [`NUDGED`]), one more partition of its inventory over the span at its published
/// state, its last step's, moves its CO₂ by under 5% ([`SETTLED_SHARE`]), each printed with its
/// greenhouse's step count. With the cap's scope read on CO₂'s airborne partial pressure, ten
/// movers cycled across that gate; read on CO₂'s inventory, every one settles. Each mover's CO₂
/// partial pressure x p is printed against the registry's saturation pressure at its column's
/// `T_c`, the gates aside.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 115 s on a pod"]
fn under_every_26_hold_the_cap_s_movers_settle() {
    let Census {
        balanced, solves, ..
    } = census();
    let mut failures = Vec::new();
    for now in balanced {
        let id = id_of(now);
        if !(MOVERS.contains(&id.as_str()) || NUDGED.contains(&id.as_str())) {
            continue;
        }
        let solve = &published(solves, now).greenhouse;
        let moved = if solve.carbon_dioxide > 0.0 {
            (solve.again - solve.carbon_dioxide).abs() / solve.carbon_dioxide
        } else {
            0.0
        };
        let settled = moved < SETTLED_SHARE;
        let t_c = solve.figures.and_then(|f| f.temperature);
        let saturation = t_c.and_then(carbon_dioxide_saturation);
        eprintln!(
            "{} {}: CO2 {:.4e} Pa, again {:.4e} Pa ({:+.3e}), {} steps; T_c {}, its CO2 {} its \
             saturation there",
            if settled { "settled" } else { "UNSETTLED" },
            now.label,
            solve.carbon_dioxide,
            solve.again,
            solve.again / solve.carbon_dioxide - 1.0,
            solve.steps,
            kelvin(t_c.map(crate::units::Kelvin::value)),
            saturation.map_or_else(
                || "against no".to_owned(),
                |p_sat| format!("×{:.3e}", now.co2_partial / p_sat)
            ),
        );
        if !settled {
            failures.push(format!("{id} ({} steps)", solve.steps));
        }
    }
    assert_eq!(failures, Vec::<String>::new(), "the unsettled movers");
}

/// The share by which one more partition may move a mover's CO₂ for its steps to count as
/// settled: 5% (P14.T53.h's amendment).
const SETTLED_SHARE: f64 = 0.05;

/// CO₂'s saturation pressure at `t` on the registry's curve, Pa.
fn carbon_dioxide_saturation(t: crate::units::Kelvin) -> Option<f64> {
    crate::substance::saturation_pressure(Gas::CarbonDioxide.substance(), t)
        .map(crate::units::Pascals::value)
}

/// P14.T53.h's contract at the cap, under every 26 hold: a permanent cap holds its air at its
/// saturation pressure (Leighton and Murray 1966). For each mover whose record holds a cap, its
/// CO₂ partial pressure x p lies within 1% of the registry's `p_sat` at its cold trap, the colder
/// of that cap and its coldest zone (a cap never warms a cold trap), or under it where all its CO₂
/// is airborne; [`OVER_THE_CAP`]'s record lies over it, under [`OVER_THE_CAP_AT_MOST`]. Each is
/// printed with its ratio.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 115 s on a pod"]
fn under_every_26_hold_each_cap_holds_its_air_at_its_saturation() {
    let Census { balanced, .. } = census();
    let mut over = Vec::new();
    for now in balanced {
        let id = id_of(now);
        let Some(cap) = now.cap.filter(|_| MOVERS.contains(&id.as_str())) else {
            continue;
        };
        let trap = if cap < now.coldest { cap } else { now.coldest };
        let p_sat = carbon_dioxide_saturation(crate::units::Kelvin::new(trap))
            .expect("CO₂ has a vapour curve at a frost cap");
        let ratio = now.co2_partial / p_sat;
        let all_airborne = now.co2.total_cmp(&now.co2_inventory).is_eq();
        eprintln!(
            "{}: cap {cap:.3} K, coldest zone {:.3} K, CO2 weight {:.6e} Pa, x p {:.4e} Pa against p_sat {p_sat:.4e} \
             Pa, ×{ratio:.4}{}",
            now.label,
            now.coldest,
            now.co2,
            now.co2_partial,
            if all_airborne {
                ", all of it airborne"
            } else {
                ""
            }
        );
        if (ratio - 1.0).abs() <= 0.01 || (ratio < 1.0 && all_airborne) {
            continue;
        }
        assert!(
            OVER_THE_CAP.contains(&id.as_str()),
            "{}: ×{ratio} over its cap's saturation",
            now.label
        );
        assert!(
            (1.0..OVER_THE_CAP_AT_MOST).contains(&ratio),
            "{}: ×{ratio}",
            now.label
        );
        over.push(id);
    }
    assert_eq!(over, OVER_THE_CAP);
}

/// P14.T53.h's D4: under every 26 hold the golden close binary, which holds movers of the cap,
/// generates the same records twice, bit for bit.
#[test]
fn under_every_26_hold_the_cap_s_close_binary_repeats() {
    let (_, ctx, _) = golden_systems()
        .iter()
        .find(|(name, ..)| *name == "close_binary")
        .expect("the golden close binary");
    let records = || {
        {
            format!(
                "{:?}",
                generate(Seed::new(GOLDEN_SYSTEMS_SEED), ctx)
                    .snapshot_at(ctx, crate::time::UniverseTime::EPOCH)
            )
        }
    };
    assert_eq!(records(), records());
}
