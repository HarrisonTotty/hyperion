//! Black holes (plan 06, P06.T22): mass and spin, dark, with the radii a readout shows.
//!
//! A black hole's mass is the remnant mass of P06.T18 (Mandel and Müller 2020). Its dimensionless
//! spin a = c J ÷ (G M²) comes from one draw, `star.bh.spin`: a single star's collapse leaves a
//! slowly spinning hole when angular momentum is carried efficiently from the core to the
//! envelope (Fuller and Ma 2019, ApJ 881, L1, who find a ≈ 10⁻²), so the default is a
//! half-normal of σ = 0.1 held below 0.998, the spin a thin disc spins a hole up to (Thorne 1974,
//! ApJ 191, 507). This keeps most spins small and leaves a tail; plan 11 owns the routes to high
//! spin, tidal spin-up and accretion (plan 06, design note 20; provisional, see its Risks).
//!
//! A black hole is dark unless something feeds it, and feeding is plan 11's, so its state has
//! zero luminosity and never changes. Its radius is the Schwarzschild radius 2GM ÷ c², which the
//! track's remnant stage already gives; the innermost stable circular orbit is Bardeen, Press and
//! Teukolsky's (1972, ApJ 178, 347, equation 2.21).

use crate::math;
use crate::stellar::draws::StarDraws;
use crate::units::consts::{GM_SUN, SPEED_OF_LIGHT};
use crate::units::{Metres, SolarMasses};

/// The spread of the spin draw: a half-normal of σ = 0.1 (plan 06, design note 20; Fuller and Ma
/// 2019 as the suggested source; provisional).
pub const SPIN_SIGMA: f64 = 0.1;

/// The spin a draw is held below: 0.998, Thorne's (1974) limit for a hole spun up by a thin disc.
pub const MAX_SPIN: f64 = 0.998;

/// A black hole: its gravitational mass and dimensionless spin (P06.T22).
///
/// # Examples
///
/// A non-spinning 10 M☉ hole has a Schwarzschild radius of 29.5 km and its innermost stable orbit
/// at three times that; a spinning one lets matter orbit closer in:
///
/// ```
/// use hyperion_sim::stellar::remnant::BlackHole;
/// use hyperion_sim::units::SolarMasses;
///
/// let still = BlackHole::new(SolarMasses::new(10.0), 0.0).ok_or("a valid spin")?;
/// assert!((still.schwarzschild_radius().value() / 29_532.0 - 1.0).abs() < 1e-3);
/// assert!((still.isco_radius().value() / still.schwarzschild_radius().value() - 3.0).abs() < 1e-12);
/// let spinning = BlackHole::new(SolarMasses::new(10.0), 0.9).ok_or("a valid spin")?;
/// assert!(spinning.isco_radius() < still.isco_radius());
/// # Ok::<(), &str>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlackHole {
    mass: SolarMasses,
    spin: f64,
}

impl BlackHole {
    /// The hole of `mass` and dimensionless `spin`, or `None` unless the mass is positive and
    /// finite and the spin lies in [0, 0.998).
    #[must_use]
    pub fn new(mass: SolarMasses, spin: f64) -> Option<Self> {
        (mass.value().is_finite() && mass.value() > 0.0 && (0.0..MAX_SPIN).contains(&spin))
            .then_some(Self { mass, spin })
    }

    /// The hole a star's draws make of a remnant of `mass`: spin 0.1 |z| from `star.bh.spin`,
    /// held below 0.998 (a draw beyond 9.98 σ, about once in 10²², takes the largest spin below
    /// it).
    ///
    /// # Panics
    ///
    /// If `mass` is not positive and finite, which no remnant of P06.T18 is.
    #[must_use]
    pub fn from_draws(mass: SolarMasses, draws: &StarDraws) -> Self {
        let spin =
            (SPIN_SIGMA * draws.bh_spin().value().abs()).min(MAX_SPIN * (1.0 - f64::EPSILON));
        Self::new(mass, spin).expect("a black hole from a collapse has a positive, finite mass")
    }

    /// The gravitational mass.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The dimensionless spin a = c J ÷ (G M²), in [0, 0.998).
    #[must_use]
    pub const fn spin(&self) -> f64 {
        self.spin
    }

    /// The Schwarzschild radius 2GM ÷ c², from the nominal GM☉ (2.953 km per M☉).
    #[must_use]
    pub fn schwarzschild_radius(&self) -> Metres {
        Metres::new(2.0 * self.gravitational_radius())
    }

    /// The radius of the innermost stable circular orbit, prograde (Bardeen, Press and Teukolsky
    /// 1972, equation 2.21): r = GM ÷ c² × (3 + Z₂ − √((3 − Z₁)(3 + Z₁ + 2Z₂))), with
    /// Z₁ = 1 + (1 − a²)^⅓ ((1 + a)^⅓ + (1 − a)^⅓) and Z₂ = √(3a² + Z₁²). It is 6GM ÷ c² at zero
    /// spin and falls towards GM ÷ c² as the spin approaches one.
    #[must_use]
    pub fn isco_radius(&self) -> Metres {
        let a = self.spin;
        let z1 = 1.0 + math::cbrt(1.0 - a * a) * (math::cbrt(1.0 + a) + math::cbrt(1.0 - a));
        let z2 = (3.0 * a * a + z1 * z1).sqrt();
        let r = 3.0 + z2 - ((3.0 - z1) * (3.0 + z1 + 2.0 * z2)).sqrt();
        Metres::new(r * self.gravitational_radius())
    }

    /// GM ÷ c², m.
    #[must_use]
    fn gravitational_radius(&self) -> f64 {
        GM_SUN * self.mass.value() / (SPEED_OF_LIGHT * SPEED_OF_LIGHT)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::stellar::draws::{StandardNormal, StarDrawsParts};

    fn from_z(z: f64) -> BlackHole {
        BlackHole::from_draws(
            SolarMasses::new(8.0),
            &StarDraws::from_parts(StarDrawsParts {
                bh_spin: StandardNormal::new(z).expect("finite"),
                ..StarDrawsParts::MEDIAN
            }),
        )
    }

    /// The plan's test: every spin lies in [0, 0.998), the tail included, and most are small.
    #[test]
    fn spins_lie_in_zero_to_the_thorne_limit() {
        let mut rng = Lcg::new(9);
        let mut small = 0_u32;
        let n = 10_000_u32;
        for _ in 0..n {
            let u = (rng.next_f64() + f64::EPSILON) / (1.0 + 2.0 * f64::EPSILON);
            let spin = from_z(math::normal_quantile(u)).spin();
            assert!((0.0..MAX_SPIN).contains(&spin), "{spin}");
            small += u32::from(spin < 0.1);
        }
        // A half-normal of σ 0.1 lies below 0.1 with probability 2Φ(1) − 1 = 0.683.
        let share = f64::from(small) / f64::from(n);
        assert!((share - 0.683).abs() < 0.02, "{share}");
        assert!(from_z(40.0).spin() < MAX_SPIN);
        assert!((from_z(-2.0).spin() - 0.2).abs() < 1e-15);
        assert_eq!(BlackHole::new(SolarMasses::new(8.0), MAX_SPIN), None);
        assert_eq!(BlackHole::new(SolarMasses::new(8.0), -0.01), None);
    }

    /// The plan's test: the innermost stable orbit is 6GM ÷ c² at zero spin, and falls with spin
    /// towards 1.24 GM ÷ c² at 0.998 (Thorne 1974).
    #[test]
    fn the_isco_is_six_gravitational_radii_at_zero_spin() {
        let hole = BlackHole::new(SolarMasses::new(10.0), 0.0).unwrap();
        let rg = GM_SUN * 10.0 / (SPEED_OF_LIGHT * SPEED_OF_LIGHT);
        assert!((hole.isco_radius().value() / (6.0 * rg) - 1.0).abs() < 1e-12);
        let mut last = hole.isco_radius().value();
        for i in 1..998 {
            let r = BlackHole::new(SolarMasses::new(10.0), f64::from(i) * 1e-3)
                .unwrap()
                .isco_radius()
                .value();
            assert!(r < last, "{i}");
            last = r;
        }
        // 1.24 GM ÷ c² at 0.998, and just above it at 0.997.
        assert!((1.24..1.35).contains(&(last / rg)), "{}", last / rg);
    }

    /// The Schwarzschild radius is 2.953 km per solar mass.
    #[test]
    fn the_schwarzschild_radius_is_three_kilometres_a_solar_mass() {
        let hole = BlackHole::new(SolarMasses::new(1.0), 0.0).unwrap();
        assert!((hole.schwarzschild_radius().value() - 2_953.25).abs() < 0.1);
    }
}
