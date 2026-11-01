//! P14.T53.i's census over the record: the 100 Pa line read once by the steps and the record,
//! under every 26 hold against every 26 hold with the line's switch held, over P14.T53.c's
//! thin-air census (the 400 sample systems, the golden systems and LOOK72's systems), and count
//! (vii)'s two records by name.

use std::sync::OnceLock;
use std::time::Instant;

use super::cap_balance_tests::{id_of, published, same};
use super::tests::{SEED, ThinAirBody, sample, thin_air_census, thin_air_sets};
use super::*;
use crate::planetary::derive::atmosphere::{
    AIRLESS_PRESSURE, Gas, LineSwitchRules, SurfaceState, settled_log, with_line_switch,
};
use crate::planetary::derive::photometry::area_mix::{
    IcyRegolithRules, with_icy_regolith, with_scattering_held,
};
use crate::planetary::testing::with_every_26_hold;
use crate::units::Kelvin;
use hyperion_testkit::float::assert_same_bits;

/// The records the switch moves over the census, by ID in the census's order, each `Tenuous` on the
/// airless climate after a last step on the bulk one, and read once at its N₂'s saturation
/// pressure over its ice: item 33's world ([`ITEM_33_WORLD`](super::tests::ITEM_33_WORLD)), at
/// 1.68 Pa, ×49 (the census, 2026-10-31: the brief's two and this one, its lean), to 0.034 Pa; and
/// count (vii)'s two (decision-p14-t53c-collapse §2.2), at 100.000 Pa, ×1,137 and ×741, to about
/// 0.09 and 0.13 Pa.
const MOVERS: [&str; 3] = [
    "02000b2ca0000001.1100",
    "01ffeb2d20000000.e101",
    "02002b2d1ffe0000.e103",
];

/// Count (vii)'s two, the brief's movers: their record's N₂ over its ice with the switch held, and
/// read once their record's N₂ and their derivation's total, Pa, pinned natively (x86-64) so that
/// the WebAssembly suite checks the re-hold bit for bit; e101's under P14.T49.l's pore gas
/// (2026-11-01), its lid's ground reading its air, re-pinned at generator version 25, whose rules
/// move its coldest zone in the last bits.
const COUNT_VII: [(&str, f64, (f64, f64)); 2] = [
    (
        "01ffeb2d20000000.e101",
        1_137.0,
        (0.085_142_374_195_206_07, 0.102_698_785_631_132_33),
    ),
    (
        "02002b2d1ffe0000.e103",
        741.0,
        (0.129_739_417_588_150_6, 0.151_640_301_314_995_3),
    ),
];

/// The N₂ `Snowball`s at the line, which the switch leaves to the bit: the moon last.
const AT_THE_LINE: [&str; 4] = [
    "02000b2cdfff0000.0d00",
    "01fffb2bdfff0001.0600",
    "41ffecb600000001.0700",
    "41ffecb1ffc00007.0d01",
];

/// The census's two walks, one body each in the same order, and the solves each published.
struct Census {
    held: (Vec<ThinAirBody>, Vec<settled_log::Settled>),
    switched: (Vec<ThinAirBody>, Vec<settled_log::Settled>),
}

/// The census under every 26 hold, the line's switch held and in force, walked once a process,
/// with the solves each walk published ([`settled_log`]).
fn census() -> &'static Census {
    static CENSUS: OnceLock<Census> = OnceLock::new();
    CENSUS.get_or_init(|| {
        let sets = thin_air_sets();
        let start = Instant::now();
        // The icy regolith (P14.T49.m) is set back to the frost, and P14.T24.h.b's holds, as in
        // `named`: under the icy lag count (vii)'s two leave the line, and two airless bodies'
        // trace N₂ (01ffdb2ce0020000.0600 and 01ffeb2d1ffc0000.0800) moves by the switch, neither a
        // crossing; 26's bump restates this census.
        let frost = IcyRegolithRules::Frost;
        let held = settled_log::capture(|| {
            with_every_26_hold(|| {
                with_icy_regolith(frost, || {
                    with_scattering_held(|| {
                        with_line_switch(LineSwitchRules::Held, || thin_air_census(&sets))
                    })
                })
            })
        });
        let walk_held = start.elapsed();
        let start = Instant::now();
        let switched = settled_log::capture(|| {
            with_every_26_hold(|| {
                with_icy_regolith(frost, || with_scattering_held(|| thin_air_census(&sets)))
            })
        });
        eprintln!(
            "the walks: the line held {walk_held:?}, read once {:?}, {} bodies",
            start.elapsed(),
            switched.0.len()
        );
        assert_eq!(
            held.0.len(),
            switched.0.len(),
            "one walk's bodies, both rules"
        );
        Census { held, switched }
    })
}

/// The solve of `solves` that published the census body `body`, where one did: the last whose
/// temperature, total and CO₂ are its derivation's, bit for bit.
fn logged<'a>(
    solves: &'a [settled_log::Settled],
    body: &ThinAirBody,
) -> Option<&'a settled_log::Settled> {
    let bits = |x: f64, y: f64| x.total_cmp(&y).is_eq();
    solves.iter().rev().find(|s| {
        bits(s.t, body.air_t) && bits(s.total, body.air_p) && bits(s.carbon_dioxide, body.co2)
    })
}

/// Whether the solve `solve` crossed the line: its last step read the bulk climate while the total
/// it published lies under the line (P14.T53.i's movers), off a molten surface.
fn crossed(solve: &settled_log::Settled, body: &ThinAirBody) -> bool {
    solve.last_step_airless == Some(false)
        && solve.total < AIRLESS_PRESSURE.value()
        && body.state != SurfaceState::MagmaOcean
}

/// The converse: its last step read the airless climate while the total it published lies at or
/// above the line.
fn converse(solve: &settled_log::Settled) -> bool {
    solve.last_step_airless == Some(true) && solve.total >= AIRLESS_PRESSURE.value()
}

/// P14.T53.i over the census, both arms under every 26 hold: the records the switch moves are
/// exactly those whose last step read the bulk climate under a sub-line total with the switch
/// held, and they are [`MOVERS`], each printed both ways, a second hold over the airless span
/// repeating its partial pressures to the bit; none of them is `Temperate`,
/// `Snowball`, `RunawayGreenhouse` or `MagmaOcean` under either arm; the N₂ `Snowball`s at the line
/// ([`AT_THE_LINE`]) are the same to the bit; and the converse, a total at or above the line after
/// an airless last step, is counted under each arm and left as it is.
#[test]
#[ignore = "slow: the thin-air census twice under every 26 hold, about 60 s on a pod"]
fn under_every_26_hold_the_line_is_read_once_by_the_steps_and_the_record() {
    let Census { held, switched } = census();
    let (mut movers, mut crossings) = (Vec::new(), Vec::new());
    let (mut converse_held, mut converse_switched, mut unlogged) = (0_u32, 0_u32, 0_u32);
    let mut at_the_line = Vec::new();
    for (was, now) in held.0.iter().zip(&switched.0) {
        assert_eq!(was.label, now.label, "one walk's order");
        let id = id_of(now);
        let before = logged(&held.1, was);
        let after = logged(&switched.1, now);
        if before.is_none() {
            unlogged += 1;
        }
        converse_held += u32::from(before.is_some_and(converse));
        converse_switched += u32::from(after.is_some_and(converse));
        if before.is_some_and(|solve| crossed(solve, was)) {
            crossings.push(id.clone());
        }
        if AT_THE_LINE.contains(&id.as_str()) {
            assert!(
                same(was, now),
                "{}: {} against {}",
                was.label,
                was.line(),
                now.line()
            );
            assert_eq!(now.state, SurfaceState::Snowball, "{}", now.line());
            at_the_line.push(id);
            continue;
        }
        if same(was, now) {
            continue;
        }
        eprintln!(
            "moved {}\n  held:     {}; N2 {:.6e} Pa\n  one read: {}; N2 {:.6e} Pa; held once more, \
             again the same: {:?}",
            now.label,
            was.line(),
            was.nitrogen,
            now.line(),
            now.nitrogen,
            after.and_then(|solve| solve.greenhouse.switch),
        );
        for body in [was, now] {
            assert!(
                matches!(body.state, SurfaceState::Tenuous | SurfaceState::Airless),
                "{}: {:?}",
                body.label,
                body.state
            );
        }
        assert_eq!(
            after.and_then(|solve| solve.greenhouse.switch),
            Some(true),
            "{}: held once more, its partial pressures repeat",
            now.label
        );
        movers.push(id);
    }
    eprintln!(
        "{} movers; {} crossings with the switch held: {crossings:?}; the converse {converse_held} \
         held, {converse_switched} read once; {unlogged} bodies with no logged solve",
        movers.len(),
        crossings.len()
    );
    assert_eq!(
        at_the_line, AT_THE_LINE,
        "the N₂ Snowballs at the line are in the census"
    );
    assert_eq!(movers, crossings, "the movers are the crossings");
    assert_eq!(movers, MOVERS);
}

/// The census body `id` of the sample, its system generated afresh under every 26 hold with the
/// line's rules `rules`, and the solve that published it ([`settled_log`]).
/// The icy regolith (P14.T49.m) is set back to the frost: its lag moves both off the line, to a
/// `Snowball` under either rule
/// (`under_every_26_hold_the_icy_regolith_moves_count_vii_s_two_off_the_line`); and P14.T24.h.b's
/// holds are set back (`with_scattering_held`), whose adding rule moves their N₂.
fn named(id: &str, rules: LineSwitchRules) -> (ThinAirBody, settled_log::Settled) {
    named_under(id, rules, IcyRegolithRules::Frost)
}

/// [`named`] under the icy regolith's rules `icy`.
fn named_under(
    id: &str,
    rules: LineSwitchRules,
    icy: IcyRegolithRules,
) -> (ThinAirBody, settled_log::Settled) {
    let system = id.split('.').next().unwrap_or_default();
    let ctx = sample()
        .iter()
        .find(|ctx| ctx.id().to_string() == system)
        .unwrap_or_else(|| panic!("{id}'s system is in the sample"));
    let sets = [("sample", SEED, ctx.clone())];
    let (bodies, solves) = settled_log::capture(|| {
        with_every_26_hold(|| {
            with_icy_regolith(icy, || {
                with_scattering_held(|| with_line_switch(rules, || thin_air_census(&sets)))
            })
        })
    });
    let body = bodies
        .into_iter()
        .find(|body| id_of(body) == id)
        .unwrap_or_else(|| panic!("{id} is a census body"));
    let solve = *published(&solves, &body);
    (body, solve)
}

/// P14.T53.i's records by name, count (vii)'s two ([`COUNT_VII`]): with the switch held each is
/// `Tenuous` at 100.000 Pa, its last step on the bulk climate, its record's N₂ ×1,137 and ×741
/// over its ice at its coldest zone (P14.T53.c's seam, as today). Read once, each is `Tenuous` or
/// `Airless`, its record's dominant N₂ within 10⁻⁹ of its saturation pressure at its record's
/// coldest zone, about 0.09 and 0.13 Pa, and a second hold over the airless span repeats its
/// partial pressures to the bit.
#[test]
fn under_every_26_hold_count_vii_s_two_are_held_once_over_the_airless_span() {
    let saturation = |t: f64| {
        crate::substance::saturation_pressure(Gas::Nitrogen.substance(), Kelvin::new(t))
            .expect("N₂ has a vapour curve below its critical point")
            .value()
    };
    for (id, factor, (pinned_nitrogen, pinned_total)) in COUNT_VII {
        let (was, held) = named(id, LineSwitchRules::Held);
        let (now, read_once) = named(id, LineSwitchRules::OneSwitch);
        eprintln!(
            "{id}\n  held:     {}; N2 {:.6e} Pa, p_sat {:.6e} Pa at its coldest zone\n  read \
             once: {}; N2 {:.6e} Pa, p_sat {:.6e} Pa at its coldest zone",
            was.line(),
            was.nitrogen,
            saturation(was.coldest),
            now.line(),
            now.nitrogen,
            saturation(now.coldest),
        );
        assert_eq!(was.state, SurfaceState::Tenuous, "{}", was.line());
        assert!((99.999..100.0).contains(&was.p), "{}", was.line());
        assert_eq!(held.last_step_airless, Some(false), "{id}");
        assert_eq!(held.greenhouse.switch, None, "{id}");
        let over = was.factor().expect("its N₂ lies over its ice");
        assert!((over / factor - 1.0).abs() < 0.001, "{id}: ×{over:.1}");
        assert!(
            matches!(now.state, SurfaceState::Tenuous | SurfaceState::Airless),
            "{}",
            now.line()
        );
        let (gas, nitrogen, ..) = now.dominant.expect("its N₂ lies over its ice");
        assert_eq!(gas, Gas::Nitrogen, "{}", now.line());
        let p_sat = saturation(now.coldest);
        eprintln!("  read once, its record's N₂ {nitrogen:.12e} Pa against p_sat {p_sat:.12e} Pa");
        assert!(
            (nitrogen - p_sat).abs() <= 1e-9 * p_sat,
            "{id}: N₂ {nitrogen} Pa against {p_sat} Pa"
        );
        assert!((0.05..0.2).contains(&nitrogen), "{}", now.line());
        assert_same_bits(nitrogen, pinned_nitrogen);
        assert_same_bits(now.air_p, pinned_total);
        assert_eq!(read_once.greenhouse.switch, Some(true), "{id}: held twice");
    }
}

/// P14.T53.i's D4: under every 26 hold e101's system generates the same records twice, bit for
/// bit.
#[test]
fn under_every_26_hold_count_vii_s_first_system_repeats() {
    let ctx = sample()
        .iter()
        .find(|ctx| ctx.id().to_string() == "01ffeb2d20000000")
        .expect("e101's system is in the sample");
    let records = || {
        with_every_26_hold(|| {
            format!(
                "{:?}",
                generate(SEED, ctx).snapshot_at(ctx, crate::time::UniverseTime::EPOCH)
            )
        })
    };
    assert_eq!(records(), records());
}

/// P14.T49.m's movers among the line's bodies, stated: under every 26 hold the icy regolith's lag
/// moves count (vii)'s two, ice-rich belt members held at the 100 Pa line, to a `Snowball` under
/// either rule of the line (their regolith lagged while their pass is tenuous, their nitrogen then
/// a bulk air at the line, its coldest zone 45.9 K against 33.3–33.8 K under the frost), each
/// state printed under both rules.
#[test]
fn under_every_26_hold_the_icy_regolith_moves_count_vii_s_two_off_the_line() {
    for (id, ..) in COUNT_VII {
        for rules in [LineSwitchRules::Held, LineSwitchRules::OneSwitch] {
            let (frost, _) = named_under(id, rules, IcyRegolithRules::Frost);
            let (lagged, _) = named_under(id, rules, IcyRegolithRules::Mixture);
            eprintln!(
                "{id} ({rules:?}):\n  the frost: {}\n  the lag:   {}",
                frost.line(),
                lagged.line()
            );
            assert_eq!(frost.state, SurfaceState::Tenuous, "{id}");
            assert_eq!(lagged.state, SurfaceState::Snowball, "{}", lagged.line());
        }
    }
}
