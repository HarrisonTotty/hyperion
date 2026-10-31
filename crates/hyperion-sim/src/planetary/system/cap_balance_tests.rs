//! P14.T53.h's census over the record: the CO₂ cap's energy balance, under every 26 hold against
//! every 26 hold with the cap held, over P14.T53.c's thin-air census (the 400 sample systems, the
//! golden systems and LOOK72's systems).

use std::sync::OnceLock;
use std::time::Instant;

use super::tests::{
    GOLDEN_SYSTEMS_SEED, ThinAirBody, golden_systems, thin_air_census, thin_air_sets,
};
use super::*;
use crate::planetary::derive::atmosphere::{SurfaceState, settled_log};
use crate::planetary::hooks::surface::{CapBalanceRules, rotates, with_cap_balance};
use crate::planetary::testing::with_every_26_hold;

/// The records whose state or figures the cap moves over the census (the census at the tip,
/// 2026-10-31), by ID in the census's order: thick CO₂ airs that collapse or thin, each a fast
/// rotator (a ÷ `L_Ro` of 4.0–25 under the cap) whose cap is frost. Nine go from `Temperate` to
/// `Snowball`, three stay `Snowball` and one `Temperate` thins. The research's pilot, which read the
/// cap in the passes' steps alone with neither gate, moved 16; the gates leave LOOK72's three
/// (020e56ef20080000.0700, 020e56ef60070005.0100 and 020e66efa007000b.0400), and the cap
/// `partition_over` reads, which the passes' area mix reads too
/// ([`partition_over`](crate::planetary::hooks::figures::partition_over)), takes
/// 42002cb200000009.0700 to a 0.36 bar `Snowball` at 122 K, where that pilot thinned it to 14 bar.
const MOVERS: [&str; 13] = [
    "01fffb2be0000000.0500",
    "21ffd6587ff80000.0300",
    "41ffecb200000001.0300",
    "41ffecb200000000.e301",
    "02000b2b60020000.0400",
    "2200365a7ff80000.0400",
    "42002cb1ff800003.0a00",
    "41ffecb1ffc00000.0a00",
    "42006cba00000009.0400",
    "42002cb200000009.0700",
    "42002cb200000009.0800",
    "42002cb200000009.e401",
    "42002cb200000009.e501",
];

/// The records the cap moves in their last bits alone, within 10⁻⁶ ([`within_a_millionth`]), by
/// ID in the census's order: a step or a thermostat trial on the way read a cap, and the steps
/// settled a few ulps away.
const NUDGED: [&str; 2] = ["41ffecb600000007.0300", "2200365a7ff80000.0300"];

/// The census under every 26 hold, the cap held and in force, walked once a process, with the
/// solves the walk under the cap published ([`settled_log`]).
fn census() -> &'static Census {
    static CENSUS: OnceLock<Census> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let sets = thin_air_sets();
        let start = Instant::now();
        let held = with_every_26_hold(|| {
            with_cap_balance(CapBalanceRules::Held, || thin_air_census(&sets))
        });
        let walk_held = start.elapsed();
        let start = Instant::now();
        let (balanced, solves) =
            settled_log::capture(|| with_every_26_hold(|| thin_air_census(&sets)));
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
/// state, its last step's, moves its CO₂ by under 5%. It lists each failure with its greenhouse's
/// step count and does not pin them (main's, 2026-10-31); it asserts only that each mover's
/// published solve is found in the log. The census of 2026-10-31, the air term on the surface
/// pressure, lists ten, each at the steps' limit (33 evaluations): 01fffb2be0000000.0500,
/// 41ffecb200000001.0300, 41ffecb200000000.e301, 02000b2b60020000.0400, 2200365a7ff80000.0400,
/// 42002cb1ff800003.0a00, `solar_like`'s 42006cba00000009.0400 and `close_binary`'s
/// 42002cb200000009.0800, .e401 and .e501.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 115 s on a pod"]
fn under_every_26_hold_the_cap_s_movers_settle_or_are_listed() {
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
        eprintln!(
            "{} {}: CO2 {:.4e} Pa, again {:.4e} Pa ({:+.3e}), {} steps",
            if settled { "settled" } else { "UNSETTLED" },
            now.label,
            solve.carbon_dioxide,
            solve.again,
            solve.again / solve.carbon_dioxide - 1.0,
            solve.steps
        );
        if !settled {
            failures.push(format!("{id} ({} steps)", solve.steps));
        }
    }
    eprintln!("{} unsettled: {failures:?}", failures.len());
}

/// The share by which one more partition may move a mover's CO₂ for its steps to count as
/// settled: 5% (P14.T53.h's amendment).
const SETTLED_SHARE: f64 = 0.05;

/// P14.T53.h's D4: under every 26 hold the golden close binary, which holds movers of the cap,
/// generates the same records twice, bit for bit.
#[test]
fn under_every_26_hold_the_cap_s_close_binary_repeats() {
    let (_, ctx, _) = golden_systems()
        .iter()
        .find(|(name, ..)| *name == "close_binary")
        .expect("the golden close binary");
    let records = || {
        with_every_26_hold(|| {
            format!(
                "{:?}",
                generate(Seed::new(GOLDEN_SYSTEMS_SEED), ctx)
                    .snapshot_at(ctx, crate::time::UniverseTime::EPOCH)
            )
        })
    };
    assert_eq!(records(), records());
}
