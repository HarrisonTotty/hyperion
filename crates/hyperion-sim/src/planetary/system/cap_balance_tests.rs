//! P14.T53.h's census over the record: the CO₂ cap's energy balance, under every 26 hold against
//! every 26 hold with the cap held, over P14.T53.c's thin-air census (the 400 sample systems, the
//! golden systems and LOOK72's systems).

use std::sync::OnceLock;
use std::time::Instant;

use super::tests::{
    GOLDEN_SYSTEMS_SEED, ThinAirBody, golden_systems, thin_air_census, thin_air_sets,
};
use super::*;
use crate::planetary::derive::atmosphere::SurfaceState;
use crate::planetary::derive::passes::Convergence;
use crate::planetary::hooks::surface::{CapBalanceRules, rotates, with_cap_balance};
use crate::planetary::testing::with_every_26_hold;

/// The records whose state or figures the cap moves over the census (the census, 2026-10-31), by
/// ID in the census's order: thick CO₂ airs that collapse or thin, each a fast rotator (a ÷ `L_Ro`
/// of 4.4–22 under the cap) whose cap is frost. The research's first pilot, which read the cap in
/// the passes' steps alone with neither gate, moved 16; the gates leave LOOK72's three
/// (020e56ef20080000.0700, 020e56ef60070005.0100 and 020e66efa007000b.0400), and the cap
/// `partition_over` reads, which the passes' area mix reads too
/// ([`partition_over`](crate::planetary::hooks::figures::partition_over)), keeps
/// 01fffb2be0000000.0500 at its 35.6 bar ([`NUDGED`]) and takes 42002cb200000009.0700 to a
/// 0.36 bar `Snowball` at 122 K, where that pilot thinned it to 14 bar.
const MOVERS: [&str; 12] = [
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
const NUDGED: [&str; 3] = [
    "01fffb2be0000000.0500",
    "21ffd6587ff80000.0200",
    "2200365a7ff80000.0300",
];

/// The census under every 26 hold, the cap held and in force, walked once a process.
fn census() -> &'static (Vec<ThinAirBody>, Vec<ThinAirBody>) {
    static CENSUS: OnceLock<(Vec<ThinAirBody>, Vec<ThinAirBody>)> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let sets = thin_air_sets();
        let start = Instant::now();
        let held = with_every_26_hold(|| {
            with_cap_balance(CapBalanceRules::Held, || thin_air_census(&sets))
        });
        let walk_held = start.elapsed();
        let start = Instant::now();
        let balanced = with_every_26_hold(|| thin_air_census(&sets));
        eprintln!(
            "the walks: the cap held {walk_held:?}, balanced {:?}, {} bodies",
            start.elapsed(),
            balanced.len()
        );
        assert_eq!(held.len(), balanced.len(), "one walk's bodies, both rules");
        (held, balanced)
    })
}

/// Whether `a` and `b`, one body under each rule, publish the same state and figures, bit for bit.
fn same(a: &ThinAirBody, b: &ThinAirBody) -> bool {
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

/// Whether a census body's passes settled within the plain passes.
fn settled(body: &ThinAirBody) -> bool {
    matches!(body.convergence, Convergence::Converged)
}

/// The ID of a census body, the last word of its label.
fn id_of(body: &ThinAirBody) -> String {
    body.label.rsplit(' ').next().unwrap_or_default().to_owned()
}

/// P14.T53.h over the census, both arms under every 26 hold: the records the cap moves are
/// [`MOVERS`], and those it moves in their last bits [`NUDGED`], each printed both ways with its
/// a ÷ `L_Ro`, its cap and its light; none of them is a `MagmaOcean`, `Tenuous` or `Airless`
/// record under either arm, and none is a world locked 1:1.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 115 s on a pod"]
fn under_every_26_hold_the_cap_moves_the_census_s_thick_carbon_dioxide_airs() {
    let (held, balanced) = census();
    let (mut movers, mut nudged) = (Vec::new(), Vec::new());
    for (was, now) in held.iter().zip(balanced) {
        assert_eq!(was.label, now.label, "one walk's order");
        if same(was, now) {
            continue;
        }
        let near = within_a_millionth(was, now);
        eprintln!(
            "{} {}\n  held:     {}\n  balanced: {}\n  p {:+.3e}, T_s {:+.3e} K; a ÷ L_Ro {:.3} \
             held, {:.3} balanced; the cap {:?} K; S ÷ S⊕ {:.4}",
            if near { "nudged" } else { "moved" },
            now.label,
            was.line(),
            now.line(),
            now.p / was.p - 1.0,
            now.t_s - was.t_s,
            was.rossby,
            now.rossby,
            now.cap,
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

/// P14.T53.h's settled check over the census, both arms under every 26 hold: it lists the records
/// whose passes do not settle within the plain passes under the cap (bracketed instead), against
/// the cap held, without pinning them, and asserts only that every record the cap unsettles is
/// one it moves ([`MOVERS`] or [`NUDGED`]). The census of 2026-10-31 lists six:
/// 01fffb2be0000000.0500, 41ffecb200000001.0300, 2200365a7ff80000.0400, 42002cb1ff800003.0a00,
/// `solar_like`'s 42006cba00000009.0400 and `close_binary`'s 42002cb200000009.e401.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 115 s on a pod"]
fn under_every_26_hold_the_cap_s_movers_settle_or_are_listed() {
    let (held, balanced) = census();
    let mut unsettled = Vec::new();
    for (was, now) in held.iter().zip(balanced) {
        if settled(now) {
            continue;
        }
        let id = id_of(now);
        eprintln!(
            "unsettled under the cap {} (held: {}): {}",
            now.label,
            if settled(was) { "settled" } else { "unsettled" },
            now.line()
        );
        if settled(was) {
            unsettled.push(id);
        }
    }
    eprintln!(
        "the cap unsettles {} records: {unsettled:?}",
        unsettled.len()
    );
    for id in &unsettled {
        assert!(
            MOVERS.contains(&id.as_str()) || NUDGED.contains(&id.as_str()),
            "{id} is unsettled by the cap but does not move"
        );
    }
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
