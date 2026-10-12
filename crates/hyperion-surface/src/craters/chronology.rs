//! The lunar chronology, counted from a system's formation: the cumulative crater density N(>1 km)
//! a surface accumulates over its age, and its inverse (plan R09, Design note 12;
//! science-r09-craters.md item 5).
//!
//! Neukum, Ivanov and Hartmann 2001's curve, N(T) = 5.44 × 10⁻¹⁴ (e^(6.93 T) − 1) + 8.38 × 10⁻⁴ T
//! km⁻² with T in Gyr, takes T as time before the Solar System's present. Its exponential is the
//! early bombardment, which every system has at its own birth, so a surface of age S in a system
//! of age A is counted from the system's formation: with o = `A_☉` − A,
//!
//! N(S; A) = 5.44 × 10⁻¹⁴ (e^(6.93 (o + S)) − e^(6.93 o)) + 8.38 × 10⁻⁴ S, for 0 ≤ S ≤ A.
//!
//! At A = `A_☉` it is the published curve to the bit (o is 0, e^0 is 1). Taken as written, a system
//! older than the Sun would explode (S = 10 Gyr gives 6.8 × 10¹⁶ km⁻²) and a young one would get
//! the tail alone (S = 1 Gyr in a 1 Gyr system, 8.4 × 10⁻⁴ km⁻² where its early bombardment's
//! 3.0 belongs). Plan 14's belt scaling multiplies it (P14.T24.b), and the coarse pass and the
//! synthesis invert it for craters' ages ([`chronology_age`]; R09.T7.b).

use hyperion_base::math;
use hyperion_base::units::{Gigayears, PerSquareKilometre};

/// The Solar System's age, the chronology's clock: 4.567 Gyr, the age of its calcium–aluminium-rich
/// inclusions, Connelly et al. 2012's 4.5673 ± 0.0002 Gyr (Science 338, 651) rounded as
/// science-r09-craters.md item 5 has it.
pub const SOLAR_SYSTEM_AGE: Gigayears = Gigayears::new(4.567);

/// The exponential term's coefficient, km⁻²: 5.44 × 10⁻¹⁴ (Neukum, Ivanov and Hartmann 2001, eq. 5,
/// as Craterstats' "Moon" chronology lists it).
const EXPONENTIAL_COEFFICIENT: f64 = 5.44e-14;

/// The exponential term's rate, Gyr⁻¹: 6.93 (as [`EXPONENTIAL_COEFFICIENT`]).
const EXPONENTIAL_RATE: f64 = 6.93;

/// The constant flux, km⁻² Gyr⁻¹: 8.38 × 10⁻⁴, the published coefficient (as
/// [`EXPONENTIAL_COEFFICIENT`]; Cratermaker's 10^a0 = 8.17 × 10⁻⁴ is not adopted).
const LINEAR_RATE: f64 = 8.38e-4;

/// The bisection steps of [`chronology_age`]: 64 halvings of [0, A], below an f64's resolution of
/// any age.
const AGE_BISECTION_STEPS: u32 = 64;

/// Asserts `age` is finite and non-negative and returns it in Gyr.
#[must_use]
fn gyr(what: &str, age: Gigayears) -> f64 {
    let t = age.value();
    assert!(
        t.is_finite() && t >= 0.0,
        "{what} must be finite and non-negative, got {t} Gyr"
    );
    t
}

/// The cumulative crater density N(>1 km) per km² of a surface `surface_age` old in a system
/// `system_age` old, before plan 14's belt scaling.
///
/// It is the lunar chronology counted from the system's formation ([the module
/// documentation](self)): strictly increasing in the surface age, and the published curve itself
/// at [`SOLAR_SYSTEM_AGE`].
///
/// # Panics
///
/// If either age is not finite and non-negative, or the surface is older than its system.
#[must_use]
pub fn lunar_chronology(surface_age: Gigayears, system_age: Gigayears) -> PerSquareKilometre {
    let s = gyr("a surface age", surface_age);
    let a = gyr("a system age", system_age);
    assert!(
        s <= a,
        "a surface of {s} Gyr is older than its system of {a} Gyr"
    );
    let o = SOLAR_SYSTEM_AGE.value() - a;
    PerSquareKilometre::new(
        EXPONENTIAL_COEFFICIENT
            * (math::exp(EXPONENTIAL_RATE * (o + s)) - math::exp(EXPONENTIAL_RATE * o))
            + LINEAR_RATE * s,
    )
}

/// The surface age at which a system `system_age` old has accumulated the density `n_1km`, the
/// inverse of [`lunar_chronology`].
///
/// It bisects [0, A] with a fixed 64 halvings, the same work for every input on every target. It
/// is zero for a density of zero or less, and the system's age for one at or above the whole
/// system's.
///
/// # Panics
///
/// If the system age is not finite and non-negative, or the density is NaN.
#[must_use]
pub fn chronology_age(n_1km: PerSquareKilometre, system_age: Gigayears) -> Gigayears {
    let n = n_1km.value();
    assert!(!n.is_nan(), "a crater density must not be NaN");
    let a = gyr("a system age", system_age);
    if n <= 0.0 {
        return Gigayears::ZERO;
    }
    let (mut lo, mut hi) = (0.0, a);
    for _ in 0..AGE_BISECTION_STEPS {
        let mid = f64::midpoint(lo, hi);
        if lunar_chronology(Gigayears::new(mid), system_age).value() < n {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Gigayears::new(f64::midpoint(lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The published curve as R09.T10's reference worlds wrote it.
    fn published(t: f64) -> f64 {
        5.44e-14 * (math::exp(6.93 * t) - 1.0) + 8.38e-4 * t
    }

    #[test]
    fn the_solar_systems_chronology_is_the_published_curve_to_the_bit() {
        // Every reference world's age (0.1, 3.9, 4.4 and 4.0 Gyr, R09.T10) among them.
        for t in [0.0, 0.1, 0.5, 1.0, 3.2, 3.8, 3.9, 4.0, 4.4, 4.567] {
            assert_same_bits(
                lunar_chronology(Gigayears::new(t), SOLAR_SYSTEM_AGE).value(),
                published(t),
            );
        }
        // 1 Gyr is Neukum 1983's a0, 10^−3.0768 (science-r09-craters.md F3).
        let n = lunar_chronology(Gigayears::new(1.0), SOLAR_SYSTEM_AGE).value();
        assert!((math::log10(n) + 3.0768).abs() < 5e-5, "{n}");
    }

    #[test]
    fn the_chronology_rises_with_the_surface_age() {
        for system in [1.0, SOLAR_SYSTEM_AGE.value(), 10.0] {
            let mut previous = -1.0;
            for k in 0..=1_000 {
                let s = system * f64::from(k) / 1_000.0;
                let n = lunar_chronology(Gigayears::new(s), Gigayears::new(system)).value();
                assert!(n > previous, "{system} Gyr system at {s} Gyr");
                previous = n;
            }
        }
    }

    #[test]
    fn the_inverse_recovers_the_age() {
        for system in [0.5, 2.0, SOLAR_SYSTEM_AGE.value(), 10.0] {
            for k in 0..=100 {
                let s = system * f64::from(k) / 100.0;
                let a = Gigayears::new(system);
                let n = lunar_chronology(Gigayears::new(s), a);
                let back = chronology_age(n, a).value();
                assert!((back - s).abs() < 1e-12, "{system} Gyr, {s} Gyr: {back}");
            }
        }
        let a = SOLAR_SYSTEM_AGE;
        assert_same_bits(chronology_age(PerSquareKilometre::ZERO, a).value(), 0.0);
        assert_same_bits(
            chronology_age(PerSquareKilometre::new(-1.0), a).value(),
            0.0,
        );
        let whole = lunar_chronology(a, a).value();
        let saturated = chronology_age(PerSquareKilometre::new(2.0 * whole), a).value();
        assert!((saturated - a.value()).abs() < 1e-12, "{saturated}");
    }

    #[test]
    fn every_system_counts_from_its_own_formation() {
        // A 10-Gyr system's 10-Gyr surface holds what the Solar System's 4.567-Gyr one does,
        // 3.03 km⁻², to 1%; taken as written the curve would give 6.8 × 10¹⁶.
        let old = lunar_chronology(Gigayears::new(10.0), Gigayears::new(10.0)).value();
        let ours = lunar_chronology(SOLAR_SYSTEM_AGE, SOLAR_SYSTEM_AGE).value();
        assert!((old / 3.03 - 1.0).abs() < 0.01, "{old}");
        assert!((ours / 3.03 - 1.0).abs() < 0.01, "{ours}");
        // A young system's whole age includes its early bombardment, and an old system's young
        // surface only the tail: 1.7 × 10⁻³ at 2 Gyr, as in the Solar System.
        let young = lunar_chronology(Gigayears::new(1.0), Gigayears::new(1.0)).value();
        assert!((young / 3.0 - 1.0).abs() < 0.02, "{young}");
        let tail = lunar_chronology(Gigayears::new(2.0), Gigayears::new(10.0)).value();
        assert!((tail / 1.7e-3 - 1.0).abs() < 0.02, "{tail}");
    }
}
