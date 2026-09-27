//! Molecular clouds and dark nebulae: gas and dust, no members (plan 09, P09.T4.c; Design note
//! 19).
//!
//! The brainstorm gives clouds a count ("thousands") and a size (50–300 ly) and nothing else, so
//! the statistics here are parameters of the generator version:
//!
//! - **Mass function** `dN ÷ dM ∝ M^−1.7` over 10⁴–10^6.5 M☉ (Design note 19; Miville-Deschênes,
//!   Murray and Lee 2017, ApJ 834, 57, measure a slope near −1.6 to −1.7 for the Galaxy's 8,107
//!   clouds). Its mean is 1.1 × 10⁵ M☉.
//! - **Radius from a constant surface density**, `M = π R² Σ` with Σ = 170 M☉ pc⁻² (Solomon,
//!   Rivolo, Barrett and Yahil 1987, ApJ 319, 730, the normalisation of Larson's third law; other
//!   surveys give 40–150, so it is provisional). `R` is the cloud's half-mass radius.
//! - **Profile**, a Plummer ball `n_c (1 + r² ÷ a²)^(−5/2)` with `a = R ÷ 1.305`, whose half-mass
//!   radius is `R`, and `n_c = 3M ÷ (4π a³)` in hydrogen at 1.4 `m_H` per hydrogen: the form plan 07
//!   integrates in closed form ([`GasModifier::Cloud`](crate::galaxy::gas::modifiers::GasModifier)).
//! - **Dust** by the local dust-to-gas ratio, plan 07's `GasField::dust_per_hydrogen` at the site.
//!
//! The process's mass density is `ε n_n + 9 n_mol`: a share ε ([`CLOUD_NEUTRAL_SHARE`]) of the
//! smooth neutral gas, plus nine times the smooth molecular disc, which plan 07 keeps at a tenth of
//! the central molecular zone and expects the rest as clouds. In Design note 19's terms the
//! molecular weight is `w = 9 ÷ ε`, closed in the two components' masses: the expected mass of
//! clouds drawn from the molecular term is nine times the disc's.

use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use crate::galaxy::gas::MASS_PER_HYDROGEN_FACTOR;
use crate::math;
use crate::units::consts::{HYDROGEN_MASS_KG, METRES_PER_LIGHT_YEAR, SOLAR_MASS_KG};
use crate::units::{HydrogenPerCm3, LightYears, SolarMasses};

/// The clouds' mass function falls as `M^−CLOUD_MASS_SLOPE`.
pub const CLOUD_MASS_SLOPE: f64 = 1.7;

/// The least cloud mass, M☉.
pub const CLOUD_MASS_MIN: SolarMasses = SolarMasses::new(1e4);

/// The greatest cloud mass, 10^6.5 M☉.
pub const CLOUD_MASS_MAX: SolarMasses = SolarMasses::new(3_162_277.660_168_379_5);

/// The clouds' surface density Σ, M☉ per square parsec (Solomon et al. 1987; provisional).
pub const CLOUD_SURFACE_DENSITY: f64 = 170.0;

/// A Plummer ball's half-mass radius over its core radius, `1 ÷ √(2^(2/3) − 1)`.
pub const PLUMMER_HALF_MASS_RATIO: f64 = 1.304_766_372_624_786;

/// ε: the share of the smooth neutral gas that the clouds' process follows (module
/// documentation). 0.15 puts about 1 × 10⁹ M☉ in clouds at Milky Way values, the molecular mass
/// Miville-Deschênes et al. 2017 find in their clouds (1.2 × 10⁹ M☉), and so some ten thousand
/// clouds. Provisional: our reading, a parameter of the generator version.
pub const CLOUD_NEUTRAL_SHARE: f64 = 0.15;

/// How many times plan 07's smooth molecular disc the clouds of the central molecular zone hold:
/// 9, since plan 07 keeps a tenth of the zone's mass smooth (Design note 19).
pub const MOLECULAR_CLOUD_MULTIPLE: f64 = 9.0;

/// The mean cloud mass of the mass function, M☉: `(α − 1) ÷ (2 − α) × (hi^(2−α) − lo^(2−α)) ÷
/// (lo^(1−α) − hi^(1−α))`, about 1.1 × 10⁵.
#[must_use]
pub fn mean_cloud_mass() -> SolarMasses {
    let (a, lo, hi) = (
        CLOUD_MASS_SLOPE,
        CLOUD_MASS_MIN.value(),
        CLOUD_MASS_MAX.value(),
    );
    let upper = math::powf(hi, 2.0 - a) - math::powf(lo, 2.0 - a);
    let lower = math::powf(lo, 1.0 - a) - math::powf(hi, 1.0 - a);
    SolarMasses::new((a - 1.0) / (2.0 - a) * upper / lower)
}

/// A cloud's marks (P09.T4.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudMarks {
    mass: SolarMasses,
    dust_per_hydrogen: f64,
}

impl CloudMarks {
    /// A cloud of `mass` whose dust per hydrogen is `dust_per_hydrogen` times the solar ratio.
    #[must_use]
    pub(crate) const fn new(mass: SolarMasses, dust_per_hydrogen: f64) -> Self {
        Self {
            mass,
            dust_per_hydrogen,
        }
    }

    /// Its gas mass.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// Its half-mass radius, `√(M ÷ π Σ)`.
    #[must_use]
    pub fn radius(&self) -> LightYears {
        let parsecs = (self.mass.value() / (core::f64::consts::PI * CLOUD_SURFACE_DENSITY)).sqrt();
        LightYears::new(parsecs * LIGHT_YEARS_PER_PARSEC)
    }

    /// Its size: the diameter `2R`.
    #[must_use]
    pub fn size(&self) -> LightYears {
        self.radius() * 2.0
    }

    /// Its Plummer core radius, `R ÷ 1.305`.
    #[must_use]
    pub fn core_radius(&self) -> LightYears {
        self.radius() / PLUMMER_HALF_MASS_RATIO
    }

    /// Its central hydrogen density, `3M ÷ (4π a³)` over 1.4 `m_H`.
    #[must_use]
    pub fn central_density(&self) -> HydrogenPerCm3 {
        plummer_central_density(self.mass, self.core_radius())
    }

    /// Its dust per hydrogen nucleus, in units of the solar ratio.
    #[must_use]
    pub const fn dust_per_hydrogen(&self) -> f64 {
        self.dust_per_hydrogen
    }
}

/// The central hydrogen density of a Plummer ball of gas mass `mass` and core radius `core`:
/// `3M ÷ (4π a³)` over 1.4 `m_H` per hydrogen, cm⁻³.
#[must_use]
pub fn plummer_central_density(mass: SolarMasses, core: LightYears) -> HydrogenPerCm3 {
    let a = core.value() * METRES_PER_LIGHT_YEAR;
    let rho = 3.0 * mass.value() * SOLAR_MASS_KG / (4.0 * core::f64::consts::PI * a * a * a);
    HydrogenPerCm3::new(rho / (MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG) * 1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mean_mass_matches_a_fine_sum() {
        let n = 400_000;
        let (lo, hi) = (
            math::ln(CLOUD_MASS_MIN.value()),
            math::ln(CLOUD_MASS_MAX.value()),
        );
        let (mut number, mut mass) = (0.0, 0.0);
        for i in 0..n {
            let x = lo + (hi - lo) * (f64::from(i) + 0.5) / f64::from(n);
            let m = math::exp(x);
            let weight = math::powf(m, 1.0 - CLOUD_MASS_SLOPE);
            number += weight;
            mass += weight * m;
        }
        let mean = mean_cloud_mass().value();
        assert!((mean / (mass / number) - 1.0).abs() < 1e-6, "{mean}");
        assert!((1.0e5..1.2e5).contains(&mean), "{mean}");
    }

    #[test]
    fn sizes_and_densities_span_the_plan_ranges() {
        let light = CloudMarks::new(CLOUD_MASS_MIN, 1.0);
        let heavy = CloudMarks::new(CLOUD_MASS_MAX, 1.0);
        // Diameters of 28–500 ly; central densities of about 100–2,000 cm⁻³.
        assert!(
            (25.0..32.0).contains(&light.size().value()),
            "{:?}",
            light.size()
        );
        assert!(
            (450.0..550.0).contains(&heavy.size().value()),
            "{:?}",
            heavy.size()
        );
        for cloud in [light, heavy] {
            let n = cloud.central_density().value();
            assert!((1e2..1e6).contains(&n), "{n}");
        }
    }

    #[test]
    fn the_plummer_ball_holds_its_mass() {
        // ∫ 4π r² n_c (1 + r²/a²)^(−5/2) dr = (4π/3) a³ n_c.
        let cloud = CloudMarks::new(SolarMasses::new(2e5), 1.0);
        let a = cloud.core_radius().value() * METRES_PER_LIGHT_YEAR;
        let n = cloud.central_density().value() * 1e6;
        let mass = 4.0 / 3.0
            * core::f64::consts::PI
            * a
            * a
            * a
            * n
            * MASS_PER_HYDROGEN_FACTOR
            * HYDROGEN_MASS_KG
            / SOLAR_MASS_KG;
        assert!((mass / 2e5 - 1.0).abs() < 1e-12, "{mass}");
    }
}
