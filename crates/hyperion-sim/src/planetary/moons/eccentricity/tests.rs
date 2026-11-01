//! P14.T55.j.a's checks: the hold, the budget at measured elements and the two rules' brackets.

use super::*;
use crate::planetary::derive::solar::SOLAR_AGE;
use crate::planetary::testing::{with_every_26_hold, with_every_27_hold};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// Saturn's and Jupiter's GM, km³ s⁻² (JPL's "Planet GM values", SAT441 and JUP365, read
/// 2026-10-16, as `tides`' checks read them), and their equatorial radii at 1 bar, km (NASA's
/// Saturn and Jupiter fact sheets, from memory), the reference radii of Lainey et al.'s k₂.
const SATURN: (f64, f64) = (37_931_206.23, 60_268.0);
const JUPITER: (f64, f64) = (126_686_531.9, 71_492.0);

/// A moon at its measured elements: its GM, km³ s⁻², its semi-major axis, km, and its sidereal
/// period, days, as `tides`' checks read them (JPL's satellite tables and NASA's fact sheets).
struct Measured {
    gm: f64,
    a_km: f64,
    period_days: f64,
}

const ENCELADUS: Measured = Measured {
    gm: 7.210_37,
    a_km: 238_020.0,
    period_days: 1.370_218,
};
const DIONE: Measured = Measured {
    gm: 73.116_07,
    a_km: 377_400.0,
    period_days: 2.736_915,
};
const IO: Measured = Measured {
    gm: 5_959.915_47,
    a_km: 421_800.0,
    period_days: 1.769_138,
};
const EUROPA: Measured = Measured {
    gm: 3_202.712_10,
    a_km: 671_100.0,
    period_days: 3.551_181,
};

impl Measured {
    fn orbit(&self) -> ResonantOrbit {
        ResonantOrbit {
            mass: kilograms(self.gm),
            a: Metres::new(self.a_km * 1e3),
            mean_motion_rad_per_s: core::f64::consts::TAU / (self.period_days * 86_400.0),
        }
    }
}

fn kilograms(gm_km3_s2: f64) -> Kilograms {
    Kilograms::new(gm_km3_s2 * 1e9 / GRAVITATIONAL_CONSTANT)
}

/// The Sun's age, s, at which the measured systems are read.
fn age() -> Seconds {
    Seconds::new(SOLAR_AGE.value() * SECONDS_PER_JULIAN_YEAR)
}

/// The planet `(gm, radius_km)`'s locking k₂ ÷ Q at the inner moon `inner`.
fn locking(planet: (f64, f64), inner: &Measured) -> f64 {
    locking_tidal_response(
        kilograms(planet.0),
        Metres::new(planet.1 * 1e3),
        &inner.orbit(),
        age(),
    )
}

/// P14.T55.j.a's hold: 26's bump leaves it, 27's deletes it; the tests' switch and 27's helper
/// reach the damping, and 26's helper does not.
#[test]
fn the_eccentricity_damping_is_held_only_until_the_27_bump() {
    use EccentricityDampingRules::{Damped, Held};
    assert!(
        crate::GENERATOR_VERSION < ECCENTRICITY_DAMPING_VERSION,
        "the 26 → 27 bump damps a regular moon's eccentricity to its age (P14.T55.j.a): delete \
         `ECCENTRICITY_DAMPING_VERSION`, `EccentricityDampingRules` with its `Held` arms in \
         `keep_inside` and `parent_at_epoch`, the tests' `with_eccentricity_damping`, its switch \
         in `with_every_27_hold`, and this test; re-bless the movers plan 14's record of \
         P14.T55.j.a lists"
    );
    assert_eq!(EccentricityDampingRules::in_force(), Held);
    assert_eq!(
        with_eccentricity_damping(Damped, EccentricityDampingRules::in_force),
        Damped
    );
    assert_eq!(
        with_every_27_hold(EccentricityDampingRules::in_force),
        Damped
    );
    assert_eq!(with_every_26_hold(EccentricityDampingRules::in_force), Held);
    assert_eq!(
        with_every_27_hold(|| with_eccentricity_damping(Held, EccentricityDampingRules::in_force)),
        Held
    );
    assert_eq!(EccentricityDampingRules::in_force(), Held);
}

/// The budget of a pair at a fixed Q is Meyer and Wisdom's (2007, Icarus 188, 535; from memory):
/// at Q = 18,000 and k₂ = 0.341 Enceladus and Dione can dissipate 1.1 GW. Here 1.0–1.3 GW.
#[test]
fn enceladus_and_dione_s_budget_at_meyer_and_wisdom_s_q_is_theirs() {
    let radius = Metres::new(SATURN.1 * 1e3);
    let budget = resonance_budget(
        radius,
        0.341 / 18_000.0,
        &[ENCELADUS.orbit(), DIONE.orbit()],
    );
    eprintln!(
        "Enceladus and Dione at Q 18,000, k2 0.341: {:.3} GW",
        budget.value() / 1e9
    );
    assert!(
        (1.0e9..1.3e9).contains(&budget.value()),
        "{budget:?}, against Meyer and Wisdom's 1.1 GW"
    );
}

/// The locking k₂ ÷ Q, at which the inner moon migrates over the Sun's age, lies within 2σ of the
/// astrometric values (from memory): Saturn's at Enceladus (1.59 ± 0.74) × 10⁻⁴ (Lainey et al.
/// 2017, Icarus 281, 286), 0.11–3.07 × 10⁻⁴, and Jupiter's at Io (1.102 ± 0.203) × 10⁻⁵ (Lainey et
/// al. 2009, Nature 459, 957), 0.70–1.51 × 10⁻⁵.
#[test]
fn the_locking_response_lies_within_two_sigma_of_the_astrometry() {
    let saturn = locking(SATURN, &ENCELADUS);
    let jupiter = locking(JUPITER, &IO);
    eprintln!("locking k2/Q: Saturn at Enceladus {saturn:.4e}, Jupiter at Io {jupiter:.4e}");
    assert!((0.11e-4..3.07e-4).contains(&saturn), "{saturn}");
    assert!((0.70e-5..1.51e-5).contains(&jupiter), "{jupiter}");
}

/// At the locking k₂ ÷ Q Enceladus and Dione can dissipate 10–20 GW, about the 15.8 ± 3.1 GW of
/// Enceladus's south polar terrain (Howett et al. 2011, JGR 116, E03003); Io and Europa's is
/// recorded, under Io's ~10¹⁴ W, as Lainey et al. (2009) find Io above equilibrium.
#[test]
fn enceladus_and_dione_s_budget_at_the_locking_response_is_ten_to_twenty_gw() {
    let saturn = Metres::new(SATURN.1 * 1e3);
    let budget = resonance_budget(
        saturn,
        locking(SATURN, &ENCELADUS),
        &[ENCELADUS.orbit(), DIONE.orbit()],
    );
    let jovian = resonance_budget(
        Metres::new(JUPITER.1 * 1e3),
        locking(JUPITER, &IO),
        &[IO.orbit(), EUROPA.orbit()],
    );
    eprintln!(
        "budgets at the locking k2/Q: Enceladus–Dione {:.2} GW, Io–Europa {:.3e} W",
        budget.value() / 1e9,
        jovian.value()
    );
    assert!((10e9..20e9).contains(&budget.value()), "{budget:?}");
    assert!(jovian.value() > 0.0 && jovian.value() < 1e14, "{jovian:?}");
}

/// The budget of one moon, or none, is zero, and the pair's is the chain's for two: a chain of
/// three reduces to the pair where its third moon is massless; and a heavy outer moon close in,
/// whose tide outruns the inner's, gives a budget that is not positive.
#[test]
fn the_budget_is_the_chain_s_and_turns_negative_where_the_outer_tide_outruns() {
    let radius = Metres::new(SATURN.1 * 1e3);
    let k = 1e-4;
    assert_eq!(resonance_budget(radius, k, &[]).value(), 0.0);
    assert_eq!(
        resonance_budget(radius, k, &[ENCELADUS.orbit()]).value(),
        0.0
    );
    let pair = resonance_budget(radius, k, &[ENCELADUS.orbit(), DIONE.orbit()]);
    let far = ResonantOrbit {
        mass: Kilograms::new(1e-30),
        a: Metres::new(DIONE.a_km * 1e3 * 1.587_401),
        mean_motion_rad_per_s: DIONE.orbit().mean_motion_rad_per_s / 2.0,
    };
    let chain = resonance_budget(radius, k, &[ENCELADUS.orbit(), DIONE.orbit(), far]);
    assert!(
        (chain.value() / pair.value() - 1.0).abs() < 1e-12,
        "{chain:?} {pair:?}"
    );
    let heavy = ResonantOrbit {
        mass: Kilograms::new(DIONE.orbit().mass.value() * 30.0),
        ..DIONE.orbit()
    };
    let outrun = resonance_budget(radius, k, &[ENCELADUS.orbit(), heavy]);
    assert!(outrun.value() <= 0.0, "{outrun:?}");
}

/// The damping time from the heat at a fixed Im k₂ is the circularisation time (2 ÷ 21)(m ÷
/// `M_p`)(a ÷ R)⁵ ÷ (n Im k₂ (1 − e²)), whatever e; a moon that dissipates nothing keeps its e.
#[test]
fn the_damping_time_is_the_circularisation_time_at_a_fixed_loss() {
    use crate::planetary::moons::regular::tidal_heating;
    let (planet, moon) = (kilograms(SATURN.0), ENCELADUS.orbit());
    let (radius, loss) = (Metres::new(252.1e3), 0.01);
    let n = moon.mean_motion_rad_per_s;
    for e in [1e-4, 0.0047, 0.04] {
        let heat = tidal_heating(planet, radius, loss, moon.a, e, n);
        let tau = damping_time(planet, moon.mass, moon.a, e, heat).value();
        let classical = 2.0 / 21.0
            * (moon.mass.value() / planet.value())
            * math::powi(moon.a.value() / radius.value(), 5)
            / (n * loss * (1.0 - e * e));
        assert!(
            (tau / classical - 1.0).abs() < 1e-12,
            "{e}: {tau} {classical}"
        );
    }
    let still = damping_time(planet, moon.mass, moon.a, 0.01, Watts::new(0.0));
    assert!(still.value().is_infinite());
}

/// The free rule keeps a drawn e that lasts the age, circularises one whose damping time falls
/// short at any e (a cold moon's fixed Im k₂), and otherwise lands on the age's crossing from
/// below, inside its bracket's resolution, where its damping time lasts.
#[test]
fn the_free_rule_keeps_a_lasting_e_circularises_a_short_one_and_finds_the_crossing() {
    let year = SECONDS_PER_JULIAN_YEAR;
    let age = Seconds::new(4.5e9 * year);
    assert_eq!(
        free_eccentricity(0.02, age, |_| Seconds::new(1e10 * year)),
        0.02
    );
    assert_eq!(
        free_eccentricity(0.02, age, |_| Seconds::new(1e8 * year)),
        0.0
    );
    assert_eq!(free_eccentricity(0.0, age, |_| Seconds::new(0.0)), 0.0);
    // An ocean that freezes below e* = 0.0031: a short time above it, a long one below.
    let crossing = 0.0031;
    let tau = |e: f64| Seconds::new(if e < crossing { 1e11 } else { 1e7 } * year);
    let mut reads = 0;
    let e = free_eccentricity(0.03, age, |e| {
        reads += 1;
        tau(e)
    });
    assert!(e < crossing && e > crossing * (1.0 - 1e-5), "{e}");
    assert!(tau(e).value() >= age.value());
    assert_eq!(reads, ECCENTRICITY_BISECTIONS + 2);
}

/// The forced scale is 1 within the budget; at H ∝ s² its first guess, √(B ÷ H(1)), is exact; it
/// steps down where the heat falls slower than s², and its heat never exceeds the budget.
#[test]
fn the_forced_scale_holds_the_chain_to_its_budget() {
    let watts = Watts::new;
    assert_eq!(forced_scale(watts(10.0), |s| watts(5.0 * s * s)), 1.0);
    let s = forced_scale(watts(1.0), |s| watts(16.0 * s * s));
    assert!((s - 0.25).abs() < 1e-15, "{s}");
    // A loss that rises as the eccentricity falls: H ∝ s² × s^(−1.5).
    let rising = |s: f64| watts(16.0 * s.sqrt());
    let s = forced_scale(watts(1.0), rising);
    assert!(rising(s).value() <= 1.0, "{s}");
    assert!((s / (1.0 / 256.0) - 1.0).abs() < 1e-5, "{s}");
    // A budget no scale reaches leaves circular orbits.
    assert_eq!(forced_scale(watts(1.0), |_| watts(2.0)), 0.0);
    assert_eq!(forced_scale(watts(0.0), watts), 0.0);
}

/// The budget's k₂ ÷ Q ceiling, k₂ ÷ 100 at a giant's 0.4 (research-leans-19): it does not bind for
/// Saturn at Enceladus or Jupiter at Io, and binds for a 10²⁰ kg inner moon at 15 Saturn radii,
/// whose locking value would be Q < 1.
#[test]
fn the_ceiling_binds_a_light_far_pair_and_neither_enceladus_nor_io() {
    use crate::planetary::params::GIANT_LOVE_NUMBER;
    let response = |planet: (f64, f64), inner: &ResonantOrbit| {
        budget_tidal_response(
            kilograms(planet.0),
            Metres::new(planet.1 * 1e3),
            GIANT_LOVE_NUMBER,
            inner,
            age(),
        )
    };
    let saturn = response(SATURN, &ENCELADUS.orbit());
    let jupiter = response(JUPITER, &IO.orbit());
    assert_eq!(saturn, BudgetResponse::Locking(locking(SATURN, &ENCELADUS)));
    assert_eq!(jupiter, BudgetResponse::Locking(locking(JUPITER, &IO)));
    let a = 15.0 * SATURN.1 * 1e3;
    let far = ResonantOrbit {
        mass: Kilograms::new(1e20),
        a: Metres::new(a),
        mean_motion_rad_per_s: (SATURN.0 * 1e9 / (a * a * a)).sqrt(),
    };
    let locked = locking_tidal_response(
        kilograms(SATURN.0),
        Metres::new(SATURN.1 * 1e3),
        &far,
        age(),
    );
    assert!(locked > GIANT_LOVE_NUMBER, "{locked}: Q < 1");
    assert_eq!(
        response(SATURN, &far),
        BudgetResponse::Ceiling(GIANT_LOVE_NUMBER / RESONANCE_LEAST_TIDAL_Q)
    );
}
