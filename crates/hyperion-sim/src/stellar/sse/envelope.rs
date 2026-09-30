//! Small-envelope behaviour: the perturbation of luminosity and radius as a star's envelope
//! becomes small in mass (plan 06, P06.T10.d; Hurley, Pols and Tout 2000, MNRAS 315, 543, "HPT",
//! section 6.3, equations 97–105).
//!
//! As the envelope thins the star moves across the HR diagram towards the remnant its core would
//! become if the envelope went at once: a naked helium star, or a white dwarf. HPT write that
//! remnant's luminosity and radius Lc and Rc and, while the envelope measure µ is below 1, move L
//! and R towards them geometrically, L′ = Lc (L ÷ Lc)^s and R′ = Rc (R ÷ Rc)^r, with exponents that
//! fall to zero as µ does. The formulae of section 5 are therefore continuous with the helium
//! stars of section 6.1 and the white dwarfs of section 6.2 when the envelope reaches zero, which
//! without the perturbation they are not (a helium-burning star is then 0.13–1.75 dex brighter than
//! the helium star it becomes; ruling 29 of 2026-09-22). The published SSE code applies the same
//! functions in `hrdiag` (its `lpertf` and `rpertf`), from the Hertzsprung gap to the helium giant
//! branch, the helium main sequence excepted.
//!
//! Units are HPT's: M☉, L☉ and R☉.

use crate::math;
use crate::units::{SolarLuminosities, SolarMasses, SolarRadii};

use super::PhasePoint;

/// The luminosity and radius of the remnant a star's core would become if it lost its envelope
/// at once, Lc and Rc of HPT section 6.3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CoreRemnant {
    /// Lc, L☉, positive.
    pub(crate) luminosity: SolarLuminosities,
    /// Rc, R☉, positive.
    pub(crate) radius: SolarRadii,
}

/// µ of HPT equation 98 for a helium giant of current mass `mt` with a carbon–oxygen core of
/// `mc`, whose core stops growing at `mc_max`: 5 (`Mc,max` − Mc) ÷ `Mc,max`. The published SSE code
/// evaluates `Mc,max` = min(1.45 M − 0.31, M) at the current mass, as
/// [`HeliumStar::core_limit`](super::helium::HeliumStar::core_limit) does before its `Mc,SN` term.
///
/// # Panics
///
/// In debug builds, if `mc_max` is not positive.
#[must_use]
pub(crate) fn helium_giant_mu(mc: SolarMasses, mc_max: SolarMasses) -> f64 {
    debug_assert!(mc_max.value() > 0.0, "a core limit is positive: {mc_max:?}");
    5.0 * (mc_max.value() - mc.value()) / mc_max.value()
}

/// `point` as the small-envelope perturbation leaves it (HPT equations 99–105) for a star of
/// current mass `mt` whose envelope measure is `mu` (equation 97 or 98) and whose core would
/// become `remnant`; unchanged where `mu` is 1 or more. The core mass is not perturbed.
///
/// L′ = Lc (L ÷ Lc)^s with s = (1 + b³)(µ ÷ b)³ ÷ (1 + (µ ÷ b)³) and b = 0.002 max(1, 2.5 ÷ M)
/// (equations 99, 101 and 103). R′ = Rc (R ÷ Rc)^r with r = (1 + c³)(µ ÷ c)³ µ^(0.1 ÷ q) ÷ (1 +
/// (µ ÷ c)³), c = 0.006 max(1, 2.5 ÷ M) and q = ln(R ÷ Rc) (equations 100, 102, 104 and 105). A
/// star no larger than its remnant takes the remnant's radius, and an envelope measure at or
/// below zero the remnant's luminosity and radius, both as in the published SSE code; the paper
/// asks only that Rc ≤ R be checked. SSE also floors µ^(0.1 ÷ q) at 10⁻¹⁴, which changes R′ by
/// under 10⁻¹³ of itself and is left out.
///
/// # Panics
///
/// In debug builds, if `mt` or the remnant's luminosity or radius is not positive.
#[must_use]
pub(crate) fn perturb(
    point: PhasePoint,
    mt: SolarMasses,
    mu: f64,
    remnant: CoreRemnant,
) -> PhasePoint {
    if mu >= 1.0 {
        return point;
    }
    debug_assert!(
        mt.value() > 0.0 && remnant.luminosity.value() > 0.0 && remnant.radius.value() > 0.0,
        "the perturbation needs a mass and a remnant: {mt:?}, {remnant:?}"
    );
    let (lc, rc) = (remnant.luminosity.value(), remnant.radius.value());
    let (l, r) = (point.luminosity.value(), point.radius.value());
    let scale = 1.0_f64.max(2.5 / mt.value());
    let luminosity = if mu > 0.0 {
        lc * math::powf_positive(l / lc, exponent(mu, 0.002 * scale))
    } else {
        lc
    };
    let radius = if mu > 0.0 && r > rc {
        let q = math::ln(r / rc);
        let power = exponent(mu, 0.006 * scale) * math::powf_positive(mu, 0.1 / q);
        rc * math::powf_positive(r / rc, power)
    } else {
        rc
    };
    PhasePoint {
        luminosity: SolarLuminosities::new(luminosity),
        radius: SolarRadii::new(radius),
        core_mass: point.core_mass,
    }
}

/// (1 + k³)(µ ÷ k)³ ÷ (1 + (µ ÷ k)³), the common factor of HPT equations 101 and 102: 1 at µ = 1,
/// falling to zero with µ, and half of 1 + k³ at µ = k.
#[must_use]
fn exponent(mu: f64, k: f64) -> f64 {
    let ratio = mu / k;
    let cubed = ratio * ratio * ratio;
    (1.0 + k * k * k) * cubed / (1.0 + cubed)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn point(l: f64, r: f64) -> PhasePoint {
        PhasePoint {
            luminosity: SolarLuminosities::new(l),
            radius: SolarRadii::new(r),
            core_mass: SolarMasses::new(0.5),
        }
    }

    fn remnant(l: f64, r: f64) -> CoreRemnant {
        CoreRemnant {
            luminosity: SolarLuminosities::new(l),
            radius: SolarRadii::new(r),
        }
    }

    /// A full envelope leaves the point alone; a vanishing one takes it to the remnant; µ = 1 is
    /// continuous with the unperturbed point.
    #[test]
    fn the_perturbation_runs_from_the_star_to_its_remnant() {
        let (star, core) = (point(5_000.0, 200.0), remnant(20.0, 0.015));
        let mass = SolarMasses::new(0.6);
        assert_eq!(perturb(star, mass, 1.3, core), star);
        let at_one = perturb(star, mass, 1.0, core);
        assert_eq!(at_one, star);
        let just_below = perturb(star, mass, 1.0 - 1e-12, core);
        assert!((just_below.luminosity.value() / 5_000.0 - 1.0).abs() < 1e-9);
        assert!((just_below.radius.value() / 200.0 - 1.0).abs() < 1e-9);
        let gone = perturb(star, mass, 0.0, core);
        assert_same_bits(gone.luminosity.value(), 20.0);
        assert_same_bits(gone.radius.value(), 0.015);
        // Continuous as µ falls to zero, and monotone in µ.
        let mut last = (f64::INFINITY, f64::INFINITY);
        for i in (1..=4_000).rev() {
            let mu = f64::from(i) / 4_000.0;
            let moved = perturb(star, mass, mu, core);
            let (l, r) = (moved.luminosity.value(), moved.radius.value());
            assert!(l <= last.0 && r <= last.1, "not monotone at µ = {mu}");
            last = (l, r);
        }
        let tiny = perturb(star, mass, 1e-9, core);
        assert!((tiny.luminosity.value() / 20.0 - 1.0).abs() < 1e-12);
        assert!((tiny.radius.value() / 0.015 - 1.0).abs() < 1e-9);
    }

    /// The exponents against the paper's equations, evaluated by hand (Python's double
    /// precision): at M = 0.6 M☉, µ = 0.01, L = 5,000, R = 200, Lc = 20, Rc = 0.015.
    #[test]
    fn the_exponents_follow_equations_101_to_105() {
        let mass = 0.6;
        let b_l = 0.002 * 2.5 / mass;
        let c_r = 0.006 * 2.5 / mass;
        let mu: f64 = 0.01;
        let s_l =
            (1.0 + b_l * b_l * b_l) * math::powi(mu / b_l, 3) / (1.0 + math::powi(mu / b_l, 3));
        let q_r = math::ln(200.0 / 0.015);
        let r_exp = (1.0 + c_r * c_r * c_r) * math::powi(mu / c_r, 3) * math::powf(mu, 0.1 / q_r)
            / (1.0 + math::powi(mu / c_r, 3));
        let got = perturb(
            point(5_000.0, 200.0),
            SolarMasses::new(mass),
            mu,
            remnant(20.0, 0.015),
        );
        let luminosity = 20.0 * math::powf(5_000.0 / 20.0, s_l);
        let radius = 0.015 * math::powf(200.0 / 0.015, r_exp);
        assert!((got.luminosity.value() / luminosity - 1.0).abs() < 1e-14);
        assert!((got.radius.value() / radius - 1.0).abs() < 1e-14);
        // s ≈ 0.633 and r ≈ 0.057 here: the radius falls towards the remnant's first.
        assert!((s_l - 0.633_4).abs() < 1e-3, "{s_l}");
        assert!((r_exp - 0.057_3).abs() < 1e-3, "{r_exp}");
    }

    /// A star smaller than its remnant takes the remnant's radius, as SSE does, which is also the
    /// limit of R′ as R falls to Rc.
    #[test]
    fn a_star_smaller_than_its_remnant_takes_the_remnants_radius() {
        let mass = SolarMasses::new(2.0);
        let core = remnant(300.0, 0.5);
        let small = perturb(point(1_000.0, 0.4), mass, 0.5, core);
        assert_same_bits(small.radius.value(), 0.5);
        let near = perturb(point(1_000.0, 0.5 + 0.5e-9), mass, 0.5, core);
        assert!((near.radius.value() / 0.5 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_helium_giants_envelope_measure_is_five_times_its_remaining_growth() {
        let mu = helium_giant_mu(SolarMasses::new(0.9), SolarMasses::new(1.0));
        assert!((mu - 0.5).abs() < 1e-15);
        assert_same_bits(
            helium_giant_mu(SolarMasses::new(1.0), SolarMasses::new(1.0)),
            0.0,
        );
    }
}
