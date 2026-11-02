//! The closed form's numerics (P11.T12's fix): its steps against finer ones, what it gives a star
//! against the engine's own integration of BSE equation 6 for the same star carried, its screen
//! against its walk, and wind Roche-lobe overflow's efficiency against Abate et al. (2013).

use std::sync::Arc;

use super::*;
use crate::stellar::binary::evolve::{Engine, LiveOrbit, arrival, own_members};
use crate::stellar::binary::tests::pair;
use crate::stellar::binary::{BinaryState, evolve};

/// The age the brief's halo pair is run to, years: 12 Gyr.
const UNTIL: f64 = 1.2e10;

/// The brief's halo pair under the Bondi–Hoyle arm, the engine's own wind: 0.85 and 0.80 M☉ at
/// Z = 0.0003 on a circular orbit of `period_days`.
fn halo_pair(period_days: f64) -> BinaryInput {
    halo_pair_under(WindAccretionRules::BondiHoyle, period_days)
}

/// The brief's halo pair under `arm`, on a circular orbit of `period_days`.
fn halo_pair_under(arm: WindAccretionRules, period_days: f64) -> BinaryInput {
    pair(0.85, 0.80, period_days, 0.0, 0.0003).with_rules(StellarRules {
        wind_accretion: arm,
        ..StellarRules::of_version(crate::GENERATOR_VERSION)
    })
}

/// What the secondary of `input`, on its own track, takes of the primary's whole wind in `steps`
/// steps between two of the primary's mass knots, M☉.
fn taken_over_the_wind(input: &BinaryInput, steps: u32) -> f64 {
    let members = own_members(input, UNTIL, None);
    let walk = Walk {
        steps_per_interval: steps,
        ..Walk::of(input, &members, 1).expect("both on their own tracks")
    };
    let (start, end) = walk
        .span()
        .expect("the secondary arrives before the primary dies");
    walk.steps(start, end)
        .map(|(t0, t1)| walk.taken(t0, t1, walk.accretor.mass_at(t0)))
        .sum()
}

/// The primary's death, years.
fn primary_death(input: &BinaryInput) -> f64 {
    let members = own_members(input, UNTIL, None);
    let Member::Track { track, .. } = &members[0] else {
        panic!("the primary is on its own track");
    };
    track
        .lifetime()
        .expect("the primary dies by 12 Gyr")
        .value()
}

/// [`STEPS_PER_INTERVAL`]'s steps hold what the secondary takes to 0.3% of eight times as many,
/// over the primary's whole wind, of the Bondi–Hoyle share alone and with wind Roche-lobe overflow.
#[test]
fn the_walk_has_converged() {
    for arm in [
        WindAccretionRules::BondiHoyle,
        WindAccretionRules::WindRocheLobe,
    ] {
        let input = halo_pair_under(arm, 3_000.0);
        let coarse = taken_over_the_wind(&input, STEPS_PER_INTERVAL);
        let fine = taken_over_the_wind(&input, 8 * STEPS_PER_INTERVAL);
        let single = taken_over_the_wind(&input, 1);
        println!(
            "{arm:?}: taken over the primary's wind: {coarse:.6e} M_sun at {STEPS_PER_INTERVAL} \
             steps, {fine:.6e} at {}, {single:.6e} at 1",
            8 * STEPS_PER_INTERVAL
        );
        assert!(fine > 1e-3, "{arm:?}: {fine}");
        assert!(
            (coarse / fine - 1.0).abs() < 3e-3,
            "{arm:?}: {coarse} against {fine}"
        );
    }
}

/// The screen before the walk holds: under 27's arm, over the halo pair's periods from 10³ to 10⁸
/// days, its two bounds together are at least what the walk gives the secondary over the
/// primary's whole wind, it refuses only a pair the walk gives under [`LEAST_ACCRETED_MSUN`], and
/// it refuses every pair from 10⁶ days.
#[test]
fn the_screen_bounds_the_walk() {
    for days in [
        1.0e3, 3.0e3, 1.0e4, 3.0e4, 1.0e5, 3.0e5, 1.0e6, 1.0e7, 1.0e8,
    ] {
        let input = halo_pair_under(WindAccretionRules::WindRocheLobe, days);
        let members = own_members(&input, UNTIL, None);
        let walk = Walk::of(&input, &members, 1).expect("both on their own tracks");
        let (start, end) = walk
            .span()
            .expect("the secondary arrives before the primary dies");
        let taken = taken_over_the_wind(&input, STEPS_PER_INTERVAL);
        let (bondi_hoyle, overflow) =
            (walk.bondi_hoyle_bound(end), walk.overflow_bound(start, end));
        let refused = walk.takes_too_little(start, end);
        println!(
            "{days:.0e} d: taken {taken:.4e} M_sun, bounds {bondi_hoyle:.4e} (Bondi-Hoyle) and \
             {overflow:.4e} (overflow), refused {refused}"
        );
        assert!(taken <= bondi_hoyle + overflow, "{days} d: {taken}");
        assert!(!refused || taken < LEAST_ACCRETED_MSUN, "{days} d: {taken}");
        assert_eq!(refused, days >= 1.0e6, "{days} d");
    }
}

/// Wind Roche-lobe overflow's efficiency is Abate et al. (2013, equation 9)'s: at Mohamed and
/// Podsiadlowski's q of 0.6 and a dust radius at the lobe it is c₁ + c₂ + c₃ = 0.4; it is 0 under
/// the quadratic's lower root, x ≈ 0.279, at most 0.5, and held at its peak, x ≈ 1.616, beyond it,
/// never falling as q or x grows. The dust radius is half the star's at the condensation
/// temperature, and goes as `T_eff`^2.5.
#[test]
fn the_overflow_follows_abate_et_al() {
    assert!((wind_roche_lobe_efficiency(0.6, 1.0) - 0.4).abs() < 1e-12);
    assert!((WIND_ROCHE_LOBE_PEAK_X - 1.616_197).abs() < 1e-6);
    let [c1, c2, c3] = WIND_ROCHE_LOBE_COEFFICIENTS;
    let root = (-c2 + (c2 * c2 - 4.0 * c1 * c3).sqrt()) / (2.0 * c1);
    assert!((root - 0.278_9).abs() < 1e-3, "{root}");
    assert!(wind_roche_lobe_efficiency(1.0, root - 1e-3) <= 0.0);
    assert!(wind_roche_lobe_efficiency(1.0, root + 1e-2) > 0.0);
    assert!(wind_roche_lobe_efficiency(1.2, 0.1) <= 0.0);
    assert!((wind_roche_lobe_efficiency(1.2, 1.0) - WIND_ROCHE_LOBE_MAX_SHARE).abs() < 1e-15);
    let peak = wind_roche_lobe_efficiency(0.5, WIND_ROCHE_LOBE_PEAK_X);
    assert!(peak > 0.3 && peak < WIND_ROCHE_LOBE_MAX_SHARE, "{peak}");
    let mut last = 0.0;
    for x in [0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 5.0] {
        let at = wind_roche_lobe_efficiency(0.5, x);
        assert!(at >= last && at <= peak, "{x}");
        assert!(at <= wind_roche_lobe_efficiency(0.6, x), "{x}");
        last = at;
    }
    assert!((wind_roche_lobe_efficiency(0.5, 5.0) - peak).abs() < 1e-15);
    assert!((dust_radius(100.0, DUST_CONDENSATION_K) - 50.0).abs() < 1e-12);
    assert!((dust_radius(1.0, 2.0 * DUST_CONDENSATION_K) - 2.0 * 2f64.sqrt()).abs() < 1e-12);
}

/// What the closed form gives the secondary is what the engine's own integration of BSE equation 6
/// gives it carried from the engine's start (its tides, braking and the winds' own orbit), to 10%,
/// 2 Myr after the primary's death.
#[test]
fn the_closed_form_takes_what_the_engine_takes() {
    let input = halo_pair(3_000.0);
    let after = primary_death(&input) + 2.0e6;
    let closed = evolve(&input, Years::new(UNTIL));
    let members = own_members(&input, UNTIL, None);
    let Member::Track { track: own, .. } = &members[1] else {
        panic!("the secondary is on its own track");
    };
    let own_mass = own.mass_at(after);
    let k = input.orbit();
    let orbit = LiveOrbit::new(
        0.0,
        k.semi_major_axis().value() / SOLAR_RADIUS_M,
        k.eccentricity().value(),
        *k.orientation(),
        k.mean_anomaly_at_epoch(),
    );
    let start = arrival(&members, UNTIL);
    let restarted = members.clone().map(|m| m.restarted(start));
    let mut engine = Engine::new(
        Arc::new(Context::of(&input)),
        start,
        UNTIL,
        restarted,
        orbit,
        None,
    );
    engine.carry(1);
    engine.run();
    let carried = engine.finish();
    let gained = |state: BinaryState| state.stars()[1].mass().value() - own_mass;
    let (by_closed, by_engine) = (
        gained(closed.state_at(Years::new(after))),
        gained(carried.state_at(Years::new(after))),
    );
    println!(
        "gained by {:.3} Gyr: {by_closed:.6e} M_sun in closed form, {by_engine:.6e} by the \
         engine; {} and {} segments",
        after / 1e9,
        closed.segments().len(),
        carried.segments().len()
    );
    assert!(by_engine > 1e-3, "{by_engine}");
    assert!(
        (by_closed / by_engine - 1.0).abs() < 0.1,
        "{by_closed} against {by_engine}"
    );
}
