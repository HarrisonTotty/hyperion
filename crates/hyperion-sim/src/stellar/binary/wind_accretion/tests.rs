//! The closed form's numerics (P11.T12's fix): its steps against finer ones, and what it gives a
//! star against the engine's own integration of BSE equation 6 for the same star carried.

use std::sync::Arc;

use super::*;
use crate::stellar::binary::evolve::{Engine, LiveOrbit, arrival, own_members};
use crate::stellar::binary::tests::pair;
use crate::stellar::binary::{BinaryState, evolve};

/// The age the brief's halo pair is run to, years: 12 Gyr.
const UNTIL: f64 = 1.2e10;

/// The brief's halo pair under the arm: 0.85 and 0.80 M☉ at Z = 0.0003 on a circular orbit of
/// `period_days`.
fn halo_pair(period_days: f64) -> BinaryInput {
    pair(0.85, 0.80, period_days, 0.0, 0.0003).with_rules(StellarRules {
        wind_accretion: WindAccretionRules::BondiHoyle,
        ..StellarRules::of_version(crate::GENERATOR_VERSION)
    })
}

/// What the secondary of `input`, on its own track, takes of the primary's whole wind in `steps`
/// steps between two of the primary's mass knots, M☉.
fn taken_over_the_wind(input: &BinaryInput, steps: u32) -> f64 {
    let members = own_members(input, UNTIL, None);
    let walk = Walk {
        steps,
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
/// over the primary's whole wind.
#[test]
fn the_walk_has_converged() {
    let input = halo_pair(3_000.0);
    let coarse = taken_over_the_wind(&input, STEPS_PER_INTERVAL);
    let fine = taken_over_the_wind(&input, 8 * STEPS_PER_INTERVAL);
    let single = taken_over_the_wind(&input, 1);
    println!(
        "taken over the primary's wind: {coarse:.6e} M_sun at {STEPS_PER_INTERVAL} steps, \
         {fine:.6e} at {}, {single:.6e} at 1",
        8 * STEPS_PER_INTERVAL
    );
    assert!(fine > 1e-3, "{fine}");
    assert!(
        (coarse / fine - 1.0).abs() < 3e-3,
        "{coarse} against {fine}"
    );
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
