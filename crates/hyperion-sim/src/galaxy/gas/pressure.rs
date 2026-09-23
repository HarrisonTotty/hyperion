//! The gas's thermal pressure: a closed-form hydrostatic layer over the corona's floor (plan 07,
//! Design note 11).
//!
//! ```text
//! P(R, z) ÷ k = P_cor ÷ k + (1.4 m_H σ_P² ÷ k) × n̄_disc(R, 0) × exp(−|z| ÷ h_P)
//! ```
//!
//! with `n̄_disc(R, 0)` the azimuthal mean of the disc gas in the plane
//! ([`SmoothGas::plane_disc_mean`]), so that the lanes do not modulate it, `σ_P` a velocity
//! dispersion and `h_P` a height, both constants of the generator version
//! ([`GasParams::PRESSURE_SPEED`], [`GasParams::PRESSURE_HEIGHT`]), and `P_cor` the drawn floor. It
//! is the hydrostatic form "pressure is density times a velocity dispersion squared", closed-form,
//! falling with height to the floor. The noise does not enter it: the phases are in rough pressure
//! balance, which is what makes rarefied gas hot (Design note 12). The floor is what caps a
//! supernova shell's observable window at the brainstorm's 2–4 Myr, which P07.T12 pins.
//!
//! Pressures are P ÷ k in K cm⁻³ and densities hydrogen nuclei per cm³ (Design note 2); radii and
//! heights are bare `f64` light-years, as the module doc of [`gas`](super) says of hot paths.

use crate::galaxy::gas::MASS_PER_HYDROGEN_FACTOR;
use crate::galaxy::gas::params::GasParams;
use crate::galaxy::gas::smooth::SmoothGas;
use crate::math;
use crate::units::MetresPerSecond;
use crate::units::consts::{BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG};

/// The thermal pressure of one galaxy's gas.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::gas::params::GasParams;
/// use hyperion_sim::galaxy::gas::pressure::Pressure;
/// use hyperion_sim::galaxy::gas::smooth::SmoothGas;
///
/// let params = GasParams::milky_way_like();
/// let (gas, pressure) = (SmoothGas::new(&params), Pressure::new(&params));
/// // The measured thermal pressure of the solar neighbourhood's cold gas, about 3,800 K cm⁻³.
/// let plane = pressure.at(&gas, 26_000.0, 0.0);
/// assert!((3_400.0..=4_200.0).contains(&plane));
/// // Far above the disc it falls to the corona's floor.
/// let high = pressure.at(&gas, 26_000.0, 30_000.0);
/// assert!((high / params.pressure_floor().value() - 1.0).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pressure {
    /// `P_cor ÷ k`, K cm⁻³.
    floor: f64,
    /// `1.4 m_H σ_P² ÷ k`, K cm⁻³ per hydrogen nucleus per cm³.
    per_density: f64,
    /// `h_P`, ly.
    height: f64,
}

impl Pressure {
    /// The pressure of `params`' gas.
    #[must_use]
    pub fn new(params: &GasParams) -> Self {
        Self {
            floor: params.pressure_floor().value(),
            per_density: per_density(MetresPerSecond::from(params.pressure_speed())),
            height: params.pressure_height().value(),
        }
    }

    /// `P ÷ k` at cylindrical radius `r` and height `z` (ly) of the smooth gas `gas`, K cm⁻³.
    ///
    /// Never below the floor, bit for bit — the floor plus a term that is not negative — and never
    /// rising with `|z|`.
    #[must_use]
    pub fn at(&self, gas: &SmoothGas, r: f64, z: f64) -> f64 {
        self.over_floor(gas, r) * math::exp(-z.abs() / self.height) + self.floor
    }

    /// The part of the mid-plane pressure above the floor at radius `r` (ly), K cm⁻³: `1.4 m_H σ_P²
    /// ÷ k × n̄_disc(R, 0)`.
    #[must_use]
    pub fn over_floor(&self, gas: &SmoothGas, r: f64) -> f64 {
        self.per_density * gas.plane_disc_mean(r)
    }

    /// The floor `P_cor ÷ k`, K cm⁻³.
    #[must_use]
    pub fn floor(&self) -> f64 {
        self.floor
    }

    /// `1.4 m_H σ_P² ÷ k`, K cm⁻³ per hydrogen nucleus per cm³: the pressure one nucleus per cm³ of
    /// disc gas holds up in the plane.
    #[must_use]
    pub fn per_density(&self) -> f64 {
        self.per_density
    }
}

/// `1.4 m_H σ² ÷ k` for a dispersion `speed`, K cm⁻³ per hydrogen nucleus per cm³.
///
/// In SI it is kelvin per nucleus per m³ times m⁻³, and the density's cm⁻³ and the pressure's cm⁻³
/// cancel, so it needs no factor of 10⁶.
fn per_density(speed: MetresPerSecond) -> f64 {
    let speed = speed.value();
    MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * speed * speed / BOLTZMANN_CONSTANT
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::Seed;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;
    use crate::units::KilometresPerSecond;

    fn drawn(seed: Seed) -> GasParams {
        GasParams::from_galaxy(
            seed,
            &GalaxyParams::from_seed(seed, MassFunctionKind::default()),
        )
        .unwrap()
    }

    /// One nucleus per cm³ moving at 1 km/s holds up `1.4 m_H (10³ m/s)² ÷ k`, about 169.7 K cm⁻³.
    #[test]
    fn the_pressure_per_density_is_the_hydrostatic_constant() {
        let one = per_density(MetresPerSecond::from(KilometresPerSecond::new(1.0)));
        assert!((one / 169.699 - 1.0).abs() < 1e-5, "{one}");
    }

    /// The Milky Way fixture's mid-plane pressure at the Sun's radius is inside 3,400–4,200 K cm⁻³,
    /// around the measured 3,800 (Jenkins and Tripp 2011, ApJ 734, 65: log(P ÷ k) of 3.58 with a
    /// dispersion of at least 0.175 dex among the cold neutral medium's sight lines).
    #[test]
    fn the_fixtures_plane_pressure_is_the_measured_one() {
        let params = GasParams::milky_way_like();
        let (gas, pressure) = (SmoothGas::new(&params), Pressure::new(&params));
        let plane = pressure.at(&gas, 26_000.0, 0.0);
        assert!((3_400.0..=4_200.0).contains(&plane), "{plane} K cm⁻³");
    }

    /// The pressure never rises with `|z|`, is even in `z`, and at 26,000 ly is within 1% of the
    /// floor by eight pressure heights, for the fixture and drawn galaxies.
    #[test]
    fn the_pressure_falls_with_height_to_the_floor() {
        let mut all = vec![GasParams::milky_way_like()];
        all.extend((0..8).map(|n| drawn(Seed::new(0x0705_0000_0000_0000 | n))));
        for params in all {
            let (gas, pressure) = (SmoothGas::new(&params), Pressure::new(&params));
            for r in [100.0, 8_000.0, 26_000.0, 60_000.0] {
                let mut previous = f64::INFINITY;
                for i in 0..=400 {
                    let z = 100.0 * f64::from(i);
                    let at = pressure.at(&gas, r, z);
                    assert!(at <= previous, "the pressure rises to {at} at ({r}, {z})");
                    assert_same_bits(pressure.at(&gas, r, -z), at);
                    previous = at;
                }
            }
            let high = pressure.at(&gas, 26_000.0, 8.0 * params.pressure_height().value());
            assert!(
                high / pressure.floor() - 1.0 < 0.01,
                "{high} at eight heights over a floor of {}",
                pressure.floor()
            );
        }
    }

    /// The floor holds over 10⁴ random positions of the root cube for each of eight drawn
    /// galaxies: bit for bit, since the pressure is the floor plus a term that is not negative
    /// (P07.T5's fast counterpart; `tests/gas_statistics.rs` sweeps 100 seeds).
    #[test]
    fn the_pressure_never_falls_below_the_floor() {
        let mut lcg = Lcg::new(0x0705);
        for n in 0..8 {
            let params = drawn(Seed::new(0x0705_f100_0000_0000 | n));
            let (gas, pressure) = (SmoothGas::new(&params), Pressure::new(&params));
            for _ in 0..10_000 {
                let [x, y, z] = [0; 3].map(|_| 65_536.0 * (2.0 * lcg.next_f64() - 1.0));
                let at = pressure.at(&gas, (x * x + y * y).sqrt(), z);
                assert!(at >= pressure.floor(), "{at} below {}", pressure.floor());
            }
        }
    }
}
